"""Incremental identities of byte-verified current deployments, without execution.

AST identities preserve identifiers, literals, operators and tree structure.
Only comments, whitespace and the enclosing webpack module key are omitted.
Equal identities establish code equality, not equal runtime inputs or callees.
"""
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor
from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
import threading

import analysis as a

DATABASE = a.ROOT / ".analysis/results/code-identity.sqlite3"
REPORT = a.ROOT / ".analysis/results/code-identity.json"
NORMALIZATION = "tree-sitter-js-structure-preserve-bindings-v1"
_worker = threading.local()


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                    separators=(",", ":")).encode()).hexdigest()


def stream_sections(path):
    """Bound memory to one top-level section, including older large receipts."""
    sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
    import ijson
    with Path(path).open("rb") as source:
        for key, value in ijson.kvitems(source, ""):
            yield key, _json_numbers(value)


def _json_numbers(value):
    """Keep unbounded JSON integers and the standard decoder's float semantics."""
    from decimal import Decimal
    if isinstance(value, Decimal):
        return float(value)
    pending = [value] if isinstance(value, (dict, list)) else []
    while pending:
        current = pending.pop()
        items = current.items() if isinstance(current, dict) else enumerate(current)
        for key, child in items:
            if isinstance(child, Decimal):
                current[key] = float(child)
            elif isinstance(child, (dict, list)):
                pending.append(child)
    return value


def connect(path=DATABASE):
    db = sqlite3.connect(path, timeout=60)
    db.row_factory = sqlite3.Row
    db.execute("PRAGMA journal_mode=WAL")
    db.execute("PRAGMA foreign_keys=ON")
    db.executescript("""
        CREATE TABLE IF NOT EXISTS metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS deployments(product_id INTEGER, family TEXT, layer TEXT,
            acquisition TEXT, acquisition_sha TEXT, failure TEXT, PRIMARY KEY(product_id,layer));
        CREATE TABLE IF NOT EXISTS sources(path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, bytes INTEGER NOT NULL);
        CREATE INDEX IF NOT EXISTS source_identity ON sources(sha256);
        CREATE TABLE IF NOT EXISTS deployment_sources(product_id INTEGER, layer TEXT, path TEXT, url TEXT,
            PRIMARY KEY(product_id,layer,path));
        CREATE INDEX IF NOT EXISTS deployment_source_path ON deployment_sources(path);
        CREATE TABLE IF NOT EXISTS parsed_sources(sha256 TEXT PRIMARY KEY, normalization TEXT, failure TEXT);
        CREATE TABLE IF NOT EXISTS factories(source_sha TEXT, start_byte INTEGER, end_byte INTEGER,
            module_id TEXT, normalized_sha TEXT, raw_sha TEXT, kind TEXT, dependencies TEXT,
            PRIMARY KEY(source_sha,start_byte,end_byte));
        CREATE INDEX IF NOT EXISTS factory_identity ON factories(normalized_sha);
        CREATE TABLE IF NOT EXISTS markers(source_sha TEXT, start_byte INTEGER, end_byte INTEGER,
            token TEXT, PRIMARY KEY(source_sha,start_byte,end_byte,token));
        CREATE TABLE IF NOT EXISTS registrations(source_sha TEXT, start_byte INTEGER, end_byte INTEGER,
            feature TEXT, generic_keys TEXT, PRIMARY KEY(source_sha,start_byte,end_byte));
        CREATE TABLE IF NOT EXISTS shared_deployments(name TEXT PRIMARY KEY, entry_url TEXT,
            acquisition TEXT, acquisition_sha TEXT, failure TEXT);
        CREATE TABLE IF NOT EXISTS shared_sources(name TEXT, path TEXT, url TEXT, PRIMARY KEY(name,path));
        CREATE VIEW IF NOT EXISTS shared_variants AS
            SELECT DISTINCT d.name,f.normalized_sha,f.kind FROM shared_deployments d
            JOIN shared_sources ds USING(name) JOIN sources s USING(path)
            JOIN factories f ON f.source_sha=s.sha256 WHERE d.failure IS NULL;
        CREATE VIEW IF NOT EXISTS product_variants AS
            SELECT DISTINCT d.product_id,d.family,d.layer,f.normalized_sha,f.kind
            FROM deployments d JOIN deployment_sources ds USING(product_id,layer)
            JOIN sources s ON s.path=ds.path JOIN factories f ON f.source_sha=s.sha256
            WHERE d.failure IS NULL;
    """)
    return db


def normalized_ast(node, *, factory=False):
    parts, stack = [], [(node, False)]
    ignored = node.child_by_field_name("name") if factory and node.type == "method_definition" else None
    while stack:
        current, end = stack.pop()
        if current.type == "comment" or current == ignored:
            continue
        if end:
            parts.append(b")")
            continue
        parts.extend((b"(", current.type.encode(), b":"))
        if current.child_count:
            stack.append((current, True))
            stack.extend((child, False) for child in reversed(current.children))
        else:
            value = current.text
            parts.extend((str(len(value)).encode(), b":", value, b")"))
    return hashlib.sha256(b"".join(parts)).hexdigest()


def parse_source(row, capability_tokens):
    import frontend
    import host
    import capability_ledger
    if not hasattr(_worker, "audit"):
        _worker.audit = host.HostLocalAudit()
    audit = _worker.audit
    audit.raw.clear()
    audit.sources.clear()
    audit.artifacts.clear()
    raw = audit.read(row)
    tree = audit.parser.parse(raw)
    if tree.root_node.has_error:
        raise ValueError("JavaScript AST has syntax errors: " + row["path"])
    segments = frontend.recovery_segments(tree.root_node, include_standalone=True)
    owners = {(f["factory_range"][0], f["factory_range"][1]): f
              for segment in segments for f in segment["factories"]}
    factories, markers = [], []
    for (start, end), owner in owners.items():
        node = tree.root_node.descendant_for_byte_range(start, end - 1)
        if (node.start_byte, node.end_byte) != (start, end):
            raise ValueError("Factory does not match a complete AST node")
        requires = owner["parameters"][2:3]
        dependencies = set()
        for child in frontend.nodes(node):
            if child.type in {"identifier", "property_identifier", "string"}:
                token = child.text.decode("utf-8")
                if child.type == "string":
                    token = capability_ledger._string(child)
                if token in capability_tokens:
                    markers.append((row["sha256"], start, end, token))
            if child.type == "call_expression":
                callee = child.child_by_field_name("function")
                args = child.child_by_field_name("arguments")
                if callee is not None and callee.type == "identifier" and callee.text.decode() in requires:
                    args = args.named_children if args else []
                    if len(args) == 1 and args[0].type == "number":
                        dependencies.add(args[0].text.decode())
                    else:
                        dependencies.add("unresolved_dynamic_require")
        factories.append((row["sha256"], start, end, str(owner["module_id"]),
                          normalized_ast(node, factory=True), hashlib.sha256(raw[start:end]).hexdigest(),
                          "webpack_factory", json.dumps(sorted(dependencies))))
    if not factories:
        factories.append((row["sha256"], 0, len(raw), "script", normalized_ast(tree.root_node),
                          row["sha256"], "non_factory_script", "[]"))
    registrations = capability_ledger._registrations(audit, row, tree=tree)
    return factories, list(set(markers)), [(row["sha256"], r["owner"]["start_byte"],
        r["owner"]["end_byte"], r["feature"], json.dumps(r["generic_keys"])) for r in registrations]


def select_deployment(spec):
    import frontend
    import host
    family, source_family, pid, layer = spec
    audit = host.HostLocalAudit()
    prefix = f"{source_family}-{'mw-' if layer == 'mw' else ''}{pid}-*"
    try:
        selected = frontend.current_source_selection(f"https://apps.razer.com/synapse/products/{pid}/{layer}/",
            audit, candidates=sorted((a.ROOT / ".analysis/inputs/frontend").glob(prefix + "/acquisition.json")),
            verify_capture=False)
        changed = audit.final_snapshot()
        if changed:
            raise ValueError("Sources changed during verification: " + str(changed))
        acquired = selected["acquisition"]
        return (pid, family, layer, acquired["path"], acquired["sha256"], None), selected["originals"]
    except (OSError, ValueError, KeyError) as error:
        return (pid, family, layer, None, None, str(error)), []


def parse_pending(db, pending, tokens, workers):
    with ProcessPoolExecutor(max_workers=workers) as pool:
        for base in range(0, len(pending), workers):
            batch = pending[base:base + workers]
            futures = [pool.submit(parse_source, row, tokens) for row in batch]
            for row, future in zip(batch, futures):
                try:
                    factories, markers, registrations = future.result()
                    failure = None
                except Exception as error:
                    factories, markers, registrations, failure = [], [], [], str(error)
                with db:
                    for table in ("factories", "markers", "registrations"):
                        db.execute(f"DELETE FROM {table} WHERE source_sha=?", (row["sha256"],))
                    db.executemany("INSERT INTO factories VALUES(?,?,?,?,?,?,?,?)", factories)
                    db.executemany("INSERT OR IGNORE INTO markers VALUES(?,?,?,?)", markers)
                    db.executemany("INSERT INTO registrations VALUES(?,?,?,?,?)", registrations)
                    db.execute("INSERT OR REPLACE INTO parsed_sources VALUES(?,?,?)", (row["sha256"], NORMALIZATION, failure))
            if base % (workers * 10) == 0:
                print(json.dumps({"parsed_unique_sources": min(base + workers, len(pending)), "total": len(pending)}), flush=True)


def build_shared(name, workers=4, db_path=DATABASE):
    """Index an explicitly required shared app without fabricating product use."""
    import frontend
    import host
    if not name or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-_" for c in name):
        raise ValueError("Invalid shared source name")
    candidates = sorted((a.ROOT / ".analysis/inputs/frontend").glob(name + "-*/acquisition.json"))
    routes = {frontend.canonical_frontend_entry(json.loads(p.read_bytes())["entry_url"]) for p in candidates}
    if len(routes) != 1:
        raise ValueError("Shared source requires one retained official route")
    audit = host.HostLocalAudit()
    selected = frontend.current_source_selection(next(iter(routes)), audit, candidates=candidates, verify_capture=False)
    if audit.final_snapshot():
        raise ValueError("Shared source changed during verification")
    with closing(connect(db_path)) as db:
        with db:
            db.execute("DELETE FROM shared_sources WHERE name=?", (name,))
            db.execute("INSERT OR REPLACE INTO shared_deployments VALUES(?,?,?,?,NULL)",
                       (name, selected["canonical_entry_url"], selected["acquisition"]["path"], selected["acquisition"]["sha256"]))
            for original in selected["originals"]:
                url = original.get("url", original["path"])
                if not url.split("?", 1)[0].endswith(".js"):
                    continue
                db.execute("INSERT OR REPLACE INTO sources VALUES(?,?,?)", (original["path"], original["sha256"], original["bytes"]))
                db.execute("INSERT INTO shared_sources VALUES(?,?,?)", (name, original["path"], url))
        pending = [dict(row) for row in db.execute("""SELECT s.* FROM shared_sources ds JOIN sources s USING(path)
            LEFT JOIN parsed_sources p ON p.sha256=s.sha256 WHERE ds.name=? AND
            (p.sha256 IS NULL OR p.normalization<>? OR p.failure IS NOT NULL) GROUP BY s.sha256""", (name, NORMALIZATION))]
        has_scope = db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='capability_scope'").fetchone()
        tokens = {r[0].split(":", 1)[1] for r in db.execute("SELECT DISTINCT capability FROM capability_scope")} if has_scope else set()
        parse_pending(db, pending, tokens, workers)
        report = {"schema_version": 1, "name": name, "acquisition": selected["acquisition"],
                  "entry_url": selected["canonical_entry_url"], "target_execution": False,
                  "newly_parsed_sources": len(pending),
                  "source_files": db.execute("SELECT COUNT(*) FROM shared_sources WHERE name=?", (name,)).fetchone()[0],
                  "code_variants": db.execute("SELECT COUNT(*) FROM shared_variants WHERE name=?", (name,)).fetchone()[0],
                  "parser_failures": [dict(r) for r in db.execute("""SELECT DISTINCT s.path,p.failure
                      FROM shared_sources ds JOIN sources s USING(path) JOIN parsed_sources p ON p.sha256=s.sha256
                      WHERE ds.name=? AND p.failure IS NOT NULL""", (name,))],
                  "product_binding": "Not inferred; actual callers must establish capability dependency scope"}
        a.save(a.ROOT / ".analysis/results" / ("code-identity-" + name + ".json"), report)
        return report


def build_frontend(workers=4, db_path=DATABASE, resume=False):
    manifest = json.loads((a.ROOT / "tools/capability-ledger.json").read_bytes())
    previous = json.loads((a.ROOT / ".analysis/results/capability-ledger.json").read_bytes())
    tokens = {c["capability"].split(":", 1)[1] for p in previous["products"] for c in p["capabilities"]}
    scope = [(family, spec["source_family"], pid, layer) for family, spec in manifest["families"].items()
             for pid in spec["product_ids"] for layer in ("mw", "ui")]
    with closing(connect(db_path)) as db:
        if resume:
            if db.execute("SELECT COUNT(*) FROM deployments").fetchone()[0] != len(scope):
                raise ValueError("Cannot resume an incomplete deployment selection")
            for row in db.execute("SELECT acquisition,acquisition_sha FROM deployments WHERE failure IS NULL"):
                if a.sha(a.ROOT / row[0]) != row[1]:
                    raise ValueError("Selected acquisition changed; rebuild selection")
        with db:
            if not resume:
                _select_all(db, scope, workers)
            version = db.execute("SELECT value FROM metadata WHERE key='factory_extraction_version'").fetchone()
            if version is None or version[0] != "2":
                db.execute("""UPDATE parsed_sources SET failure='pending_factory_syntax_recheck'
                    WHERE sha256 IN (SELECT source_sha FROM factories WHERE kind='non_factory_script')""")
        print(json.dumps({"deployment_selection_complete": len(scope)}), flush=True)
        pending = [dict(row) for row in db.execute("""SELECT s.* FROM sources s
            JOIN deployment_sources ds ON ds.path=s.path LEFT JOIN parsed_sources p ON p.sha256=s.sha256
            WHERE p.sha256 IS NULL OR p.normalization<>? OR p.failure IS NOT NULL GROUP BY s.sha256""", (NORMALIZATION,))]
        parse_pending(db, pending, tokens, workers)
        with db:
            db.execute("INSERT OR REPLACE INTO metadata VALUES('normalization',?)", (NORMALIZATION,))
            db.execute("INSERT OR REPLACE INTO metadata VALUES('scope_sha256',?)", (digest(manifest["families"]),))
            db.execute("INSERT OR REPLACE INTO metadata VALUES('factory_extraction_version','2')")
        report = summary(db)
        a.save(REPORT, report)
        return report


def _select_all(db, scope, workers):
    db.execute("DELETE FROM deployment_sources")
    db.execute("DELETE FROM deployments")
    with ThreadPoolExecutor(max_workers=workers) as pool:
        for count, (deployment, sources) in enumerate(pool.map(select_deployment, scope), 1):
            db.execute("INSERT INTO deployments VALUES(?,?,?,?,?,?)", deployment)
            for source in sources:
                # Blobs can have no suffix: MIME ownership comes from the verified source URL.
                url = source.get("url", source["path"])
                if not url.split("?", 1)[0].endswith(".js"):
                    continue
                db.execute("INSERT OR REPLACE INTO sources VALUES(?,?,?)",
                           (source["path"], source["sha256"], source["bytes"]))
                db.execute("INSERT OR REPLACE INTO deployment_sources VALUES(?,?,?,?)",
                           (deployment[0], deployment[2], source["path"], url))
            if count % 25 == 0:
                print(json.dumps({"selected_deployments": count, "total": len(scope)}), flush=True)


def summary(db):
    import importlib.metadata
    sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
    count = lambda query: db.execute(query).fetchone()[0]
    return {"schema_version": 1, "database": a.rel(DATABASE), "normalization": NORMALIZATION,
        "recorded_at_utc": a.stamp(),
        "parser_versions": {name: importlib.metadata.version(name) for name in ("tree-sitter", "tree-sitter-javascript")},
        "extractor_sources": {name: a.sha(a.ROOT / name) for name in ("tools/code_identity.py", "tools/frontend.py")},
        "target_execution": False, "scope": "first-tier latest retained byte-verified MW/UI deployments",
        "deployments": count("SELECT COUNT(*) FROM deployments"),
        "deployment_failures": [dict(r) for r in db.execute("SELECT * FROM deployments WHERE failure IS NOT NULL")],
        "source_occurrences": count("SELECT COUNT(*) FROM deployment_sources"),
        "unique_source_files": count("SELECT COUNT(DISTINCT s.sha256) FROM sources s JOIN deployment_sources ds USING(path)"),
        "unique_source_logical_bytes": count("SELECT SUM(bytes) FROM (SELECT s.bytes FROM sources s JOIN deployment_sources ds USING(path) GROUP BY s.sha256)"),
        "factory_occurrences": count("SELECT COUNT(*) FROM deployment_sources ds JOIN sources s USING(path) JOIN factories f ON f.source_sha=s.sha256 WHERE kind='webpack_factory'"),
        "unique_factory_variants": count("SELECT COUNT(DISTINCT normalized_sha) FROM product_variants WHERE kind='webpack_factory'"),
        "non_factory_variants": count("SELECT COUNT(DISTINCT normalized_sha) FROM product_variants WHERE kind='non_factory_script'"),
        "parser_failures": [dict(r) for r in db.execute("SELECT * FROM parsed_sources WHERE failure IS NOT NULL")],
        "limits": ["AST equality preserves local identifiers and literals; alpha-renaming is deliberately not assumed.",
                   "Module equality does not prove equal dependency bindings, product configuration or native semantics.",
                   "Capability token sites are candidates for review; they are not a proven complete call graph."]}


def run(args):
    result = {}
    products = getattr(args, "product", None)
    if products and args.phase != "navigation":
        raise ValueError("--product is supported only for a targeted navigation repair")
    if args.phase == "assign":
        import code_identity_reviews
        return code_identity_reviews.assign_capabilities(DATABASE, args.assignment)
    if args.phase in ("all", "frontend"):
        result["frontend"] = build_frontend(args.workers, resume=args.resume)
    if args.phase in ("all", "shared"):
        names = getattr(args, "shared_source", None)
        if not names:
            with closing(connect()) as db:
                names = [row[0] for row in db.execute("SELECT name FROM shared_deployments ORDER BY name")]
        if args.phase == "shared" and not names:
            raise ValueError("Select an explicitly required --shared-source")
        result["shared"] = [build_shared(name, args.workers) for name in names]
    if args.phase in ("all", "native"):
        import code_identity_native
        result["native"] = code_identity_native.build(DATABASE)
    if args.phase in ("all", "navigation"):
        import code_identity_navigation
        result["navigation"] = code_identity_navigation.build(DATABASE, min(args.workers, 4), products=products, refresh=bool(products))
    if args.phase in ("all", "propagate"):
        import code_identity_reviews
        result["propagation"] = code_identity_reviews.build(DATABASE)
    return result
