"""Index current declared native resources and retained IDA function identities.

Only verified deployment manifests and indexed container relationships bind a
product to a binary. Instruction identities are conservative: literal bytes and
external reference targets remain significant. They are code identities, not a
claim that a caller's product, firmware or lifecycle contract is interchangeable.
"""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
from urllib.parse import urlsplit

import analysis as a


VERSION = "native-instructions-v1"


def _digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def _checked(path, digest, size=None):
    path = a.resolve(path, ".analysis")
    before = path.stat()
    raw = path.read_bytes()
    after = path.stat()
    if (hashlib.sha256(raw).hexdigest() != digest or size is not None and len(raw) != size
            or (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns)):
        raise ValueError("Native identity input changed: " + a.rel(path))
    return raw


def _schema(db):
    db.executescript("""
        CREATE TABLE IF NOT EXISTS native_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS native_declarations(
            product_id INTEGER, layer TEXT, sha256 TEXT, manifest TEXT,
            manifest_sha TEXT, resource_name TEXT, resource_version TEXT,
            resource_url TEXT, bytes INTEGER,
            PRIMARY KEY(product_id,layer,sha256,manifest,resource_name));
        CREATE TABLE IF NOT EXISTS native_inputs(
            path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, bytes INTEGER NOT NULL,
            container_sha TEXT, input_pointer TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS native_inputs_sha ON native_inputs(sha256);
        CREATE INDEX IF NOT EXISTS native_inputs_container ON native_inputs(container_sha);
        CREATE TABLE IF NOT EXISTS native_bindings(
            product_id INTEGER, sha256 TEXT, root_sha TEXT, binding_kind TEXT,
            PRIMARY KEY(product_id,sha256,root_sha));
        CREATE TABLE IF NOT EXISTS native_modules(
            sha256 TEXT PRIMARY KEY, source_path TEXT, source_bytes INTEGER,
            capture_path TEXT, capture_sha TEXT, image_base TEXT, architecture TEXT,
            ida_version TEXT, functions_total INTEGER, functions_indexed INTEGER,
            status TEXT NOT NULL, failure TEXT);
        CREATE TABLE IF NOT EXISTS native_functions(
            source_sha TEXT, rva TEXT, name TEXT, end_rva TEXT, chunks TEXT,
            function_path TEXT, function_sha TEXT, instruction_sha TEXT,
            normalized_sha TEXT, instructions INTEGER, code_bytes INTEGER,
            external_references INTEGER, pseudocode_captured INTEGER,
            uncovered_bytes INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY(source_sha,rva));
        CREATE INDEX IF NOT EXISTS native_functions_identity ON native_functions(normalized_sha);
        CREATE TABLE IF NOT EXISTS native_failures(
            owner TEXT PRIMARY KEY, kind TEXT NOT NULL, detail TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS native_review_sections(
            section_id TEXT PRIMARY KEY, canonical_sha TEXT, products TEXT,
            capabilities TEXT, status TEXT, detail TEXT);
        CREATE TABLE IF NOT EXISTS native_review_owners(
            section_id TEXT, evidence_path TEXT, evidence_sha TEXT, selection_pointer TEXT,
            source_sha TEXT, rva TEXT, function_path TEXT, function_sha TEXT,
            normalized_sha TEXT, status TEXT, detail TEXT,
            PRIMARY KEY(section_id,evidence_path,selection_pointer));
        CREATE TABLE IF NOT EXISTS native_review_dependencies(
            section_id TEXT, product_id INTEGER, source_sha TEXT,
            source_path TEXT, source_bytes INTEGER,
            PRIMARY KEY(section_id,product_id,source_sha));
        CREATE TABLE IF NOT EXISTS native_review_matches(
            section_id TEXT, product_id INTEGER, normalized_sha TEXT,
            source_sha TEXT, rva TEXT, proof_source_sha TEXT, proof_rva TEXT,
            basis TEXT NOT NULL,
            PRIMARY KEY(section_id,product_id,source_sha,rva,proof_source_sha,proof_rva));
        CREATE TABLE IF NOT EXISTS native_review_code_candidates(
            section_id TEXT, product_id INTEGER, normalized_sha TEXT,
            source_sha TEXT, rva TEXT, proof_source_sha TEXT, proof_rva TEXT,
            basis TEXT NOT NULL,
            PRIMARY KEY(section_id,product_id,source_sha,rva,proof_source_sha,proof_rva));
    """)
    old = db.execute("SELECT value FROM native_meta WHERE key='normalization'").fetchone()
    if old and old[0] != VERSION:
        raise ValueError("Native normalization changed; explicit index migration required")
    db.execute("INSERT OR REPLACE INTO native_meta VALUES('normalization',?)", (VERSION,))
    if "binding_kind" not in {row[1] for row in db.execute("PRAGMA table_info(native_bindings)")}:
        db.execute("ALTER TABLE native_bindings ADD COLUMN binding_kind TEXT")
    if "uncovered_bytes" not in {row[1] for row in db.execute("PRAGMA table_info(native_functions)")}:
        db.execute("ALTER TABLE native_functions ADD COLUMN uncovered_bytes INTEGER NOT NULL DEFAULT 0")
    db.commit()


def _declarations(db):
    rows, failures = [], []
    for pid, layer, receipt, digest in db.execute(
            "SELECT product_id,layer,acquisition,acquisition_sha FROM deployments WHERE failure IS NULL"):
        try:
            deployment_rows = []
            document = json.loads(_checked(receipt, digest))
            for source in document["files"]:
                name = source.get("path", "")
                url_path = urlsplit(source.get("url", "")).path
                if not name or not url_path.endswith(".json") or "manifest" not in Path(url_path).name:
                    continue
                manifest = json.loads(_checked(name, source["sha256"], source["bytes"]))
                resource_manifest = manifest.get("resourceManifest", {})
                resources = resource_manifest.get("resources", [])
                for resource in resources:
                    sha = resource["sha256"].lower()
                    if len(sha) != 64 or any(c not in "0123456789abcdef" for c in sha):
                        raise ValueError("Native manifest resource has no SHA-256")
                    deployment_rows.append((pid, layer, sha, name, source["sha256"],
                                            resource["resourceName"], resource.get("resourceVersion"),
                                            resource["url"], resource["size"]))
            rows.extend(deployment_rows)
        except (OSError, ValueError, KeyError, TypeError) as error:
            failures.append((receipt, "current_manifest_resource_failure", str(error)))
    with db:
        db.execute("DELETE FROM native_failures WHERE kind='current_manifest_resource_failure'")
        db.execute("DELETE FROM native_declarations")
        db.executemany("INSERT OR REPLACE INTO native_declarations VALUES(?,?,?,?,?,?,?,?,?)", rows)
        db.executemany("INSERT OR REPLACE INTO native_failures VALUES(?,?,?)", failures)


def _inputs(db):
    """Stream metadata once, retaining only native/container lineage candidates."""
    sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
    import ijson
    path = a.ROOT / ".analysis/inputs.json"
    before = path.stat()
    digest = a.sha(path)
    roots = {row[0] for row in db.execute("SELECT DISTINCT sha256 FROM native_declarations")}
    with db:
        db.execute("DELETE FROM native_inputs")
    pending = []
    with path.open("rb") as stream:
        for index, row in enumerate(ijson.items(stream, "inputs.item")):
            if (row.get("container_extraction_complete") is False
                    or row.get("source_bytes_verified") is False):
                continue
            if not row.get("container") and row.get("sha256") not in roots:
                continue
            pending.append((row["path"], row["sha256"], row["bytes"],
                            row.get("container_sha256"), "/inputs/" + str(index)))
            if len(pending) >= 2000:
                with db:
                    db.executemany("INSERT OR REPLACE INTO native_inputs VALUES(?,?,?,?,?)", pending)
                pending.clear()
    with db:
        db.executemany("INSERT OR REPLACE INTO native_inputs VALUES(?,?,?,?,?)", pending)
        db.execute("INSERT OR REPLACE INTO native_meta VALUES('inputs_sha256',?)", (digest,))
    after = path.stat()
    if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
        raise ValueError("Native input lineage index changed during query")
    with db:
        db.execute("DELETE FROM native_bindings")
        db.execute("""INSERT OR IGNORE INTO native_bindings
            WITH RECURSIVE lineage(product_id,sha256,root_sha) AS (
                SELECT DISTINCT d.product_id,d.sha256,d.sha256 FROM native_declarations d
                WHERE NOT EXISTS(SELECT 1 FROM native_inputs i WHERE i.sha256=d.sha256)
                   OR EXISTS(SELECT 1 FROM native_inputs i WHERE i.sha256=d.sha256 AND i.bytes=d.bytes)
                UNION
                SELECT l.product_id,i.sha256,l.root_sha FROM lineage l
                JOIN native_inputs i ON i.container_sha=l.sha256
            ) SELECT product_id,sha256,root_sha,
                CASE WHEN sha256=root_sha THEN 'declared_resource' ELSE 'contained_candidate' END
                FROM lineage""")
        db.execute("""INSERT OR IGNORE INTO native_bindings
            SELECT product_id,source_sha,source_sha,'reviewed_dependency_basis'
            FROM native_review_dependencies""")


def function_identity(body, image_base, source_sha, architecture, read):
    """Validate whole chunks and instructions, preserving external target identity."""
    rva = int(body["rva"], 0)
    start = image_base + rva
    if int(body["address"], 0) != start:
        raise ValueError("Native function address and RVA disagree")
    chunks = []
    for chunk in body["chunks"]:
        begin, end = int(chunk["start"], 0), int(chunk["end"], 0)
        raw = bytes.fromhex(chunk["raw_hex"])
        if (end <= begin or end - begin != len(raw)
                or hashlib.sha256(raw).hexdigest() != chunk["sha256"]
                or read(begin - image_base, len(raw)) != raw):
            raise ValueError("Native function chunk differs from original bytes")
        chunks.append((begin, end, chunk["sha256"]))
    chunks.sort()
    if (not chunks or not any(begin == start for begin, _, _ in chunks)
            or any(left[1] > right[0] for left, right in zip(chunks, chunks[1:]))):
        raise ValueError("Native function chunks are missing, repeated or overlapping")
    observed = {begin: [] for begin, _, _ in chunks}
    instructions, normalized, external = [], [], 0
    for ins in body["assembly"]:
        address = int(ins["address"], 0)
        raw = bytes.fromhex(ins["bytes"])
        matches = [(begin, end) for begin, end, _ in chunks if begin <= address < end]
        if (len(matches) != 1 or not raw or int(ins["rva"], 0) != address - image_base
                or address + len(raw) > matches[0][1]
                or read(address - image_base, len(raw)) != raw):
            raise ValueError("Native instruction bytes do not cover complete chunks")
        observed[matches[0][0]].append((address, address + len(raw)))
        refs = []
        for ref in ins["xrefs_from"]:
            target = int(ref["to"], 0)
            if int(ref["from"], 0) != address:
                raise ValueError("Native reference belongs to a different instruction")
            internal = any(begin <= target < end for begin, end, _ in chunks)
            refs.append((ref["type"], bool(ref["code"]),
                         ["internal", target - start] if internal else
                         ["external", source_sha, target - image_base]))
            external += not internal
        item = (address - start, raw.hex())
        instructions.append(item)
        normalized.append((*item, sorted(refs)))
    gaps = []
    for begin, end, _ in chunks:
        cursor = begin
        for ins_begin, ins_end in sorted(observed[begin]):
            if ins_begin < cursor:
                raise ValueError("Native instruction ranges overlap")
            if ins_begin > cursor:
                gaps.append((cursor - start, read(cursor - image_base, ins_begin - cursor).hex()))
            cursor = ins_end
        if cursor < end:
            gaps.append((cursor - start, read(cursor - image_base, end - cursor).hex()))
    end_rva = int(body["end"], 0) - image_base
    if not any(begin == start and end - image_base == end_rva for begin, end, _ in chunks):
        raise ValueError("Native function end differs from its entry chunk boundary")
    return {"rva": hex(rva), "end_rva": hex(end_rva),
            "chunks": [[hex(begin - image_base), hex(end - image_base), sha] for begin, end, sha in chunks],
            "instruction_sha": _digest([VERSION, architecture, instructions]),
            "normalized_sha": _digest([VERSION, architecture, normalized] + ([{"unclassified_bytes": gaps}] if gaps else [])),
            "instructions": len(instructions), "code_bytes": sum(end - begin for begin, end, _ in chunks),
            "external_references": external, "uncovered_bytes": sum(len(raw) // 2 for _, raw in gaps)}


def _module(db, source_sha):
    from host import inspect_pe
    candidates = list(db.execute("SELECT path,bytes FROM native_inputs WHERE sha256=? ORDER BY path", (source_sha,)))
    capture_path = a.ROOT / ".analysis/results/native" / source_sha / "capture.json"
    if not candidates:
        return {"sha256": source_sha, "status": "missing_original", "failure": "Declared bytes are absent from inputs index"}
    source_path, source_bytes = candidates[0]
    row = {"sha256": source_sha, "source_path": source_path, "source_bytes": source_bytes}
    with a.resolve(source_path, ".analysis/inputs").open("rb") as stream:
        prefix = stream.read(2)
    if prefix != b"MZ":
        return {**row, "status": "container_or_non_pe"}
    source = _checked(source_path, source_sha, source_bytes)
    for pid, manifest, name, expected in db.execute("""SELECT product_id,manifest,resource_name,bytes
            FROM native_declarations WHERE sha256=? AND bytes<>?""", (source_sha, source_bytes)):
        with db:
            db.execute("INSERT OR REPLACE INTO native_failures VALUES(?,?,?)", (
                manifest + "#" + name, "native_resource_declaration_length_mismatch",
                json.dumps({"product_id": pid, "sha256": source_sha, "declared_bytes": expected,
                            "actual_bytes": source_bytes, "source_path": source_path}, sort_keys=True)))
    pe = inspect_pe(source)
    row.update(image_base=pe["image_base"], architecture=pe["architecture"])
    if not capture_path.is_file():
        return {**row, "status": "missing_capture", "failure": "No retained IDA base function inventory"}
    raw_capture = capture_path.read_bytes()
    capture_sha = hashlib.sha256(raw_capture).hexdigest()
    capture = json.loads(raw_capture)
    if (capture.get("input_sha256") != source_sha or capture.get("input_bytes") != len(source)
            or capture.get("capture_complete") is not True or capture.get("target_execution") is not False
            or capture.get("newly_discovered_addresses") or capture.get("removed_function_boundaries")):
        raise ValueError("Native capture has incomplete or different source identity")
    records = capture["functions"]
    if (len(records) != capture["functions_total"] or len(records) != capture["functions_captured"]
            or len({record["rva"] for record in records}) != len(records)):
        raise ValueError("Native capture has incomplete or repeated function coordinates")
    previous = db.execute("SELECT * FROM native_modules WHERE sha256=?", (source_sha,)).fetchone()
    if previous and previous[4] == capture_sha and previous[10] == "indexed":
        indexed_records = dict(db.execute("SELECT rva,function_sha FROM native_functions WHERE source_sha=?", (source_sha,)))
        if indexed_records == {record["rva"]: record["sha256"] for record in records}:
            def check_cached(record):
                _checked(a.rel(capture_path.parent / record["path"]), record["sha256"])
            with ThreadPoolExecutor(max_workers=4) as pool:
                for offset in range(0, len(records), 256):
                    list(pool.map(check_cached, records[offset:offset + 256]))
            if a.sha(capture_path) != capture_sha:
                raise ValueError("Native capture changed during cached identity verification")
            columns = ("sha256", "source_path", "source_bytes", "capture_path", "capture_sha", "image_base",
                       "architecture", "ida_version", "functions_total", "functions_indexed", "status", "failure")
            return dict(zip(columns, previous))
    base = int(pe["image_base"], 0)
    if base != int(capture["image_base"], 0):
        raise ValueError("Native capture image base differs from original")

    def read(rva, length):
        matches = [s for s in pe["sections"] if s["rva"] <= rva
                   and rva + length <= s["rva"] + s["bytes"]]
        if len(matches) != 1 or length <= 0:
            raise ValueError("Native instruction is not backed by one original PE section")
        offset = matches[0]["file_offset"] + rva - matches[0]["rva"]
        return source[offset:offset + length]

    def one(record):
        path = capture_path.parent / record["path"]
        try:
            if path.parent != capture_path.parent / "functions" or record.get("assembly_captured") is not True:
                raise ValueError("Native function is outside complete base inventory")
            body = json.loads(_checked(a.rel(path), record["sha256"]))
            if body["rva"] != record["rva"] or body["address"] != record["address"]:
                raise ValueError("Native function coordinate differs from capture manifest")
            identity = function_identity(body, base, source_sha, pe["architecture"], read)
            return (source_sha, identity["rva"], body["name"], identity["end_rva"],
                    json.dumps(identity["chunks"], separators=(",", ":")), a.rel(path), record["sha256"],
                    identity["instruction_sha"], identity["normalized_sha"], identity["instructions"],
                    identity["code_bytes"], identity["external_references"], int(record.get("pseudocode_captured") is True),
                    identity["uncovered_bytes"]), None
        except (OSError, ValueError, KeyError, TypeError) as error:
            return None, (a.rel(path), "native_function_identity_failure", str(error))

    indexed, failed = 0, 0
    with db:
        db.execute("DELETE FROM native_functions WHERE source_sha=?", (source_sha,))
    # A bounded worker batch prevents Executor.map from retaining the entire corpus.
    with ThreadPoolExecutor(max_workers=4) as pool:
        for offset in range(0, len(records), 256):
            values, failures = [], []
            for value, failure in pool.map(one, records[offset:offset + 256]):
                if failure:
                    failures.append(failure)
                else:
                    values.append(value)
            with db:
                db.executemany("INSERT INTO native_functions VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)", values)
                db.executemany("INSERT OR REPLACE INTO native_failures VALUES(?,?,?)", failures)
            indexed += len(values)
            failed += len(failures)
    if a.sha(capture_path) != capture_sha:
        raise ValueError("Native capture changed during identity build")
    row.update(capture_path=a.rel(capture_path), capture_sha=capture_sha, image_base=hex(base),
               architecture=pe["architecture"], ida_version=capture["ida_version"],
               functions_total=len(records), functions_indexed=indexed,
               status="indexed" if not failed else "partial", failure=str(failed) + " function identities failed" if failed else None)
    return row


def build(db_path, *, include_candidates=False):
    """Update native tables in the frontend identity database without target execution."""
    with sqlite3.connect(db_path, timeout=60) as db:
        db.execute("PRAGMA busy_timeout=60000")
        _schema(db)
        with db:
            db.execute("DELETE FROM native_failures")
        _declarations(db)
        _inputs(db)
        captures = {p.parent.name for p in (a.ROOT / ".analysis/results/native").glob("*/capture.json")}
        roots = {row[0] for row in db.execute("SELECT sha256 FROM native_declarations")}
        candidates = {sha for sha, path in db.execute("""SELECT b.sha256,i.path FROM native_bindings b
            LEFT JOIN native_inputs i ON i.sha256=b.sha256""")
            if sha in captures or sha in roots or path and Path(path).suffix.lower() in
            {".exe", ".dll", ".sys", ".node", ".ocx", ".cpl"}}
        sources = sorted(roots) + (sorted(candidates - roots) if include_candidates else [])
        columns = ("sha256", "source_path", "source_bytes", "capture_path", "capture_sha", "image_base",
                   "architecture", "ida_version", "functions_total", "functions_indexed", "status", "failure")
        for index, source_sha in enumerate(sources):
            try:
                row = _module(db, source_sha)
            except (OSError, ValueError, KeyError, TypeError) as error:
                row = {"sha256": source_sha, "status": "invalid", "failure": str(error)}
                with db:
                    db.execute("DELETE FROM native_functions WHERE source_sha=?", (source_sha,))
            with db:
                db.execute("INSERT OR REPLACE INTO native_modules VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                           tuple(row.get(column) for column in columns))
            if (index + 1) % 50 == 0:
                print(json.dumps({"native_sources_processed": index + 1, "total": len(sources)}), flush=True)
        with db:
            db.execute("DELETE FROM native_modules WHERE sha256 NOT IN (SELECT sha256 FROM native_bindings)")
            db.execute("DELETE FROM native_functions WHERE source_sha NOT IN (SELECT sha256 FROM native_modules)")
            db.execute("INSERT OR REPLACE INTO native_meta VALUES('scope',?)", (
                "Latest verified first-tier MW/UI declared resources and byte-indexed extraction descendants; implicit shared host loading is not inferred",))
        counts = dict(db.execute("SELECT status,count(*) FROM native_modules GROUP BY status"))
        functions, variants = db.execute("SELECT count(*),count(DISTINCT normalized_sha) FROM native_functions").fetchone()
        summary = {"normalization": VERSION, "modules": counts, "functions": functions, "instruction_variants": variants,
                "declared_resources": len(roots), "contained_native_candidates": len(candidates - roots),
                "contained_candidates_indexed_this_run": include_candidates,
                "products_with_declarations": db.execute("SELECT count(DISTINCT product_id) FROM native_declarations").fetchone()[0],
                "function_failures": db.execute("SELECT count(*) FROM native_failures").fetchone()[0],
                "functions_with_raw_unclassified_bytes": db.execute("SELECT count(*) FROM native_functions WHERE uncovered_bytes>0").fetchone()[0],
                "scope": "current declared resources and verified indexed extraction lineage",
                "implicit_host_bindings": "not established by deployment resource declarations",
                "semantic_review_propagation": False, "target_execution": False}
        with db:
            db.execute("INSERT OR REPLACE INTO native_meta VALUES('summary',?)", (json.dumps(summary, sort_keys=True),))
        return summary


def publish(db_path):
    """Publish compact native-table evidence while the frontend index can grow."""
    with sqlite3.connect(db_path, timeout=60) as db:
        db.execute("BEGIN")
        snapshot = hashlib.sha256()
        for table, query in (
                ("declarations", "SELECT * FROM native_declarations ORDER BY product_id,layer,sha256,manifest,resource_name"),
                ("modules", "SELECT * FROM native_modules ORDER BY sha256"),
                ("functions", "SELECT source_sha,rva,function_sha,normalized_sha,uncovered_bytes FROM native_functions ORDER BY source_sha,rva")):
            snapshot.update(table.encode())
            for row in db.execute(query):
                snapshot.update(json.dumps(row, separators=(",", ":"), ensure_ascii=False).encode())
                snapshot.update(b"\n")
        functions, variants, unclassified = db.execute("""SELECT count(*),count(DISTINCT normalized_sha),
            sum(uncovered_bytes>0) FROM native_functions""").fetchone()
        module_columns = ("sha256", "source_path", "source_bytes", "capture_path", "capture_sha", "image_base",
                          "architecture", "ida_version", "functions_total", "functions_indexed", "status", "failure")
        unresolved = [dict(zip(module_columns, row)) for row in db.execute(
            "SELECT * FROM native_modules WHERE status<>'indexed' ORDER BY sha256")]
        for row in unresolved:
            row["declarations"] = [{"product_id": pid, "resource_name": name, "resource_url": url,
                                    "manifest": manifest, "manifest_sha256": sha}
                                   for pid, name, url, manifest, sha in db.execute("""SELECT DISTINCT product_id,
                                       resource_name,resource_url,manifest,manifest_sha FROM native_declarations
                                       WHERE sha256=? ORDER BY product_id,manifest,resource_name""", (row["sha256"],))]
        captures = {p.parent.name for p in (a.ROOT / ".analysis/results/native").glob("*/capture.json")}
        contained = {sha for sha, path in db.execute("""SELECT DISTINCT b.sha256,i.path
            FROM native_bindings b JOIN native_inputs i ON i.sha256=b.sha256
            WHERE b.binding_kind='contained_candidate'
              AND b.sha256 NOT IN(SELECT sha256 FROM native_declarations)""")
            if sha in captures or Path(path).suffix.lower() in {".exe", ".dll", ".sys", ".node", ".ocx", ".cpl"}}
        report = {"schema_version": 1, "normalization": VERSION, "target_execution": False,
                  "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
                  "tool": {"path": a.rel(Path(__file__)), "sha256": a.sha(Path(__file__))},
                  "database": a.rel(Path(db_path)), "native_tables_snapshot_sha256": snapshot.hexdigest(),
                  "inputs_index_sha256": db.execute("SELECT value FROM native_meta WHERE key='inputs_sha256'").fetchone()[0],
                  "normalization_scope": {"instruction_sha": "architecture + exact instruction bytes and relative positions; comparison candidates only",
                                          "normalized_sha": "instruction bytes + internal relative references + source-scoped external targets + all unclassified chunk bytes",
                                          "semantic_equivalence": False},
                  "products_with_declarations": db.execute("SELECT count(DISTINCT product_id) FROM native_declarations").fetchone()[0],
                  "declared_resource_identities": db.execute("SELECT count(DISTINCT sha256) FROM native_declarations").fetchone()[0],
                  "contained_native_candidates": len(contained),
                  "contained_candidates_are_actual_loading_proof": False,
                  "missing_capture_architectures": dict(db.execute("SELECT architecture,count(*) FROM native_modules WHERE status='missing_capture' GROUP BY architecture")),
                  "modules": dict(db.execute("SELECT status,count(*) FROM native_modules GROUP BY status")),
                  "functions": functions, "normalized_code_variants": variants,
                  "functions_with_unclassified_raw_bytes": unclassified,
                  "unresolved_modules": unresolved,
                  "failures": [{"owner": owner, "kind": kind, "detail": detail} for owner, kind, detail in db.execute(
                      "SELECT * FROM native_failures ORDER BY owner")],
                  "remaining_scope": ["Contained payload bindings are candidates, not demonstrated product loading paths",
                                      "Implicit shared host dependency loading and capability ownership require existing/new call-chain evidence",
                                      "Missing original bytes and PE captures remain explicit input boundaries; source-map/library noise is not counted as capability work",
                                      "Instruction indexing does not establish reviewed semantics or Rust/UI/runtime completion"]}
    output = a.ROOT / ".analysis/results/code-identity-native.json"
    a.save(output, report)
    return {"receipt": a.rel(output), "sha256": a.sha(output), "functions": functions,
            "normalized_code_variants": variants, "modules": report["modules"], "failures": len(report["failures"])}


def _selected_native(value, pointer=""):
    """Select explicit reviewed input lists, never artifact or capture inventories."""
    if not isinstance(value, dict):
        return
    source = value.get("original") or value.get("input")
    capture = value.get("capture_receipt") or value.get("capture")
    seeds = {hex(int(rva, 0)) for rva in value.get("explicit_seed_rvas", [])}
    seeds.update(hex(int(row["rva"], 0)) for row in value.get("exports", [])
                 if isinstance(row, dict) and isinstance(row.get("rva"), str))
    for key in ("complete_selected_functions", "complete_functions"):
        for index, row in enumerate(value.get(key, [])):
            rva = row.get("rva") or row.get("capture", {}).get("rva")
            artifact = row.get("artifact")
            if (not isinstance(rva, str) or not isinstance(artifact, dict)
                    or row.get("input_chunks_verified") is not True
                    or key == "complete_functions" and hex(int(rva, 0)) not in seeds):
                continue
            yield {"source": source, "capture": capture, "artifact": artifact,
                   "rva": hex(int(rva, 0)), "pointer": pointer + "/" + key + "/" + str(index)}
    ignored = {"complete_selected_functions", "complete_functions", "assembly", "pseudocode",
               "verified_artifacts", "complete_source_owners", "complete_owners", "source_selection", "host_release"}
    for key, child in value.items():
        if key in ignored:
            continue
        path = pointer + "/" + key.replace("~", "~0").replace("/", "~1")
        if isinstance(child, dict):
            yield from _selected_native(child, path)
        elif isinstance(child, list):
            for index, entry in enumerate(child):
                if isinstance(entry, dict):
                    yield from _selected_native(entry, path + "/" + str(index))


def _review_identity(reference, verified):
    if not isinstance(reference, dict) or not {"path", "sha256", "bytes"}.issubset(reference):
        raise ValueError("Selected native owner lacks an exact retained identity")
    key = (reference["path"], reference["sha256"], reference["bytes"])
    if key not in verified:
        path = a.resolve(reference["path"], ".analysis")
        before = path.stat()
        if before.st_size != reference["bytes"] or a.sha(path) != reference["sha256"]:
            raise ValueError("Selected native review identity changed: " + reference["path"])
        after = path.stat()
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            raise ValueError("Selected native review changed during verification")
        verified[key] = (after.st_size, after.st_mtime_ns)


def _clean_review(section):
    return (section.get("target_execution") is False and section.get("failures") == []
            and section.get("changed_during_query") == [] and section.get("source_slice_complete") is True
            and (bool(section.get("semantics")) or section.get("selected_static_semantics_reviewed") is True
                 or section.get("selected_native_response_consumers_reviewed") is True))


def prepare_review_dependencies(db_path):
    """Index only shared native modules explicitly owned by currently mapped reviews."""
    import code_identity
    manifest_path = a.ROOT / "tools/capability-ledger.json"
    manifest = json.loads(manifest_path.read_bytes())
    mapped = {}
    for entry in manifest["capability_sections"]:
        for ref in entry["sections"]:
            mapped.setdefault(ref["receipt"], {}).setdefault(ref["section"], set()).update(entry["capabilities"])
    verified, selections, module_references = {}, [], {}
    with sqlite3.connect(db_path, timeout=60) as db:
        _schema(db)
        pids = {row[0] for row in db.execute("SELECT DISTINCT product_id FROM deployments WHERE failure IS NULL")}
        with db:
            for table in ("native_review_matches", "native_review_code_candidates", "native_review_owners", "native_review_dependencies", "native_review_sections"):
                db.execute("DELETE FROM " + table)
            db.execute("DELETE FROM native_bindings WHERE binding_kind='reviewed_dependency_basis'")
        for receipt, names in sorted(mapped.items()):
            path = a.resolve(receipt, ".analysis/results")
            identity = {"path": receipt, "sha256": a.sha(path), "bytes": path.stat().st_size}
            _review_identity(identity, verified)
            for name, section in code_identity.stream_sections(path):
                if name not in names:
                    continue
                pointer = "/" + name.replace("~", "~0").replace("/", "~1")
                section_id = receipt + "#" + pointer
                products = {pid for pid in section.get("product_ids", []) if type(pid) is int}
                if type(section.get("product_id")) is int:
                    products.add(section["product_id"])
                products &= pids
                status = "selection_pending" if _clean_review(section) else "no_eligible_native_scope"
                with db:
                    db.execute("INSERT INTO native_review_sections VALUES(?,?,?,?,?,?)", (
                        section_id, code_identity.digest(section), json.dumps(sorted(products)),
                        json.dumps(sorted(names[name])), status, None))
                if status != "selection_pending":
                    continue
                owned = [(identity, row) for row in _selected_native(section, pointer)]
                review = section.get("review_source")
                if review is not None:
                    try:
                        _review_identity(review, verified)
                        review_selected, flags = [], {}
                        for key, value in code_identity.stream_sections(a.ROOT / review["path"]):
                            if key in {"target_execution", "failures", "changed_during_query", "source_slice_complete",
                                       "selected_static_semantics_reviewed", "selected_native_response_consumers_reviewed"}:
                                flags[key] = value
                            if key == "semantics":
                                flags[key] = bool(value)
                            if isinstance(value, dict) and key not in {"verified_artifacts", "complete_owners"}:
                                review_selected.extend(_selected_native(value, "/" + key))
                        if review_selected and not _clean_review(flags):
                            raise ValueError("Referenced native review has no clean selected semantic scope")
                        owned.extend((review, row) for row in review_selected)
                    except (OSError, ValueError, KeyError, TypeError) as error:
                        with db:
                            db.execute("UPDATE native_review_sections SET status='invalid_review',detail=? WHERE section_id=?",
                                       (str(error), section_id))
                        continue
                if not owned:
                    with db:
                        db.execute("UPDATE native_review_sections SET status='no_explicit_native_selection' WHERE section_id=?", (section_id,))
                    continue
                for evidence, row in owned:
                    source, capture, artifact = row["source"], row["capture"], row["artifact"]
                    source_sha = source.get("sha256") if isinstance(source, dict) else None
                    failure = None
                    try:
                        for ref in (source, capture, artifact):
                            _review_identity(ref, verified)
                        base = ".analysis/results/native/" + source_sha + "/"
                        if (capture["path"] != base + "capture.json"
                                or artifact["path"] != base + "functions/" + row["rva"][2:] + ".json"):
                            raise ValueError("Selected function is not the exact indexed base owner")
                        if not products:
                            raise ValueError("Selected native owner has no explicit current product scope")
                        module_references[source_sha] = source
                        with db:
                            db.executemany("INSERT OR IGNORE INTO native_review_dependencies VALUES(?,?,?,?,?)", [
                                (section_id, pid, source_sha, source["path"], source["bytes"]) for pid in products])
                    except (OSError, ValueError, KeyError, TypeError) as error:
                        failure = str(error)
                    selections.append((section_id, evidence["path"], evidence["sha256"], row["pointer"],
                                       source_sha, row["rva"], artifact["path"], artifact["sha256"],
                                       None, "invalid_owner" if failure else "identity_pending", failure))
                with db:
                    db.execute("UPDATE native_review_sections SET status='explicit_selection' WHERE section_id=?", (section_id,))
            if a.sha(path) != identity["sha256"]:
                raise ValueError("Published review changed during native selection indexing")
        with db:
            db.executemany("INSERT OR REPLACE INTO native_review_owners VALUES(?,?,?,?,?,?,?,?,?,?,?)", selections)
            db.execute("""INSERT OR IGNORE INTO native_bindings
                SELECT product_id,source_sha,source_sha,'reviewed_dependency_basis' FROM native_review_dependencies""")
        columns = ("sha256", "source_path", "source_bytes", "capture_path", "capture_sha", "image_base",
                   "architecture", "ida_version", "functions_total", "functions_indexed", "status", "failure")
        for source_sha, source in sorted(module_references.items()):
            if not db.execute("SELECT 1 FROM native_inputs WHERE sha256=?", (source_sha,)).fetchone():
                with db:
                    db.execute("INSERT INTO native_inputs VALUES(?,?,?,?,?)", (
                        source["path"], source_sha, source["bytes"], None, "explicit_review_dependency"))
            try:
                module = _module(db, source_sha)
            except (OSError, ValueError, KeyError, TypeError) as error:
                module = {"sha256": source_sha, "status": "invalid", "failure": str(error)}
                with db:
                    db.execute("DELETE FROM native_functions WHERE source_sha=?", (source_sha,))
            with db:
                db.execute("INSERT OR REPLACE INTO native_modules VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
                           tuple(module.get(column) for column in columns))
        _bind_selected_owners(db)
        return {"explicit_shared_modules": len(module_references),
                "selected_owners": dict(db.execute("SELECT status,count(*) FROM native_review_owners GROUP BY status")),
                "reviewed_dependency_products": db.execute("SELECT count(DISTINCT product_id) FROM native_review_dependencies").fetchone()[0]}


def _bind_selected_owners(db):
    with db:
        db.execute("""UPDATE native_review_owners SET normalized_sha=(SELECT f.normalized_sha FROM native_functions f
            WHERE f.source_sha=native_review_owners.source_sha AND f.rva=native_review_owners.rva
              AND f.function_path=native_review_owners.function_path AND f.function_sha=native_review_owners.function_sha)
            WHERE status='identity_pending'""")
        db.execute("""UPDATE native_review_owners SET status=CASE WHEN normalized_sha IS NULL THEN 'indexed_owner_unavailable'
            ELSE 'selected_code_scope' END WHERE status='identity_pending'""")


def _match_reviews(db):
    with db:
        db.execute("DELETE FROM native_review_matches")
        db.execute("DELETE FROM native_review_code_candidates")
        db.execute("""INSERT OR IGNORE INTO native_review_code_candidates
            SELECT r.section_id,b.product_id,f.normalized_sha,f.source_sha,f.rva,r.source_sha,r.rva,'code_identity_candidate'
            FROM native_review_owners r JOIN native_functions f ON f.normalized_sha=r.normalized_sha
            JOIN native_bindings b ON b.sha256=f.source_sha
            WHERE r.status='selected_code_scope' AND b.binding_kind IN('declared_resource','reviewed_dependency_basis')""")
        db.execute("""INSERT OR IGNORE INTO native_review_matches
            SELECT c.section_id,c.product_id,c.normalized_sha,c.source_sha,c.rva,c.proof_source_sha,c.proof_rva,'code_scope_only'
            FROM native_review_code_candidates c
            JOIN native_review_dependencies d ON d.product_id=c.product_id AND d.source_sha=c.source_sha
            JOIN native_review_sections source_section ON source_section.section_id=c.section_id
            JOIN native_review_sections dependency_section ON dependency_section.section_id=d.section_id
            WHERE EXISTS(SELECT 1 FROM json_each(source_section.capabilities) left_cap
                JOIN json_each(dependency_section.capabilities) right_cap ON left_cap.value=right_cap.value)""")
        db.execute("""DELETE FROM native_review_code_candidates WHERE EXISTS(
            SELECT 1 FROM native_review_matches m
            WHERE m.section_id=native_review_code_candidates.section_id
              AND m.product_id=native_review_code_candidates.product_id
              AND m.source_sha=native_review_code_candidates.source_sha AND m.rva=native_review_code_candidates.rva
              AND m.proof_source_sha=native_review_code_candidates.proof_source_sha
              AND m.proof_rva=native_review_code_candidates.proof_rva)""")


def propagate_reviews(db_path):
    """Carry selected native code scope; never mark entire functions/capabilities closed."""
    preparation = prepare_review_dependencies(db_path)
    with sqlite3.connect(db_path, timeout=60) as db:
        _match_reviews(db)
        snapshot = hashlib.sha256()
        for table in ("native_review_sections", "native_review_owners", "native_review_dependencies", "native_review_matches", "native_review_code_candidates"):
            snapshot.update(table.encode())
            for row in db.execute("SELECT * FROM " + table + " ORDER BY 1,2,3,4"):
                snapshot.update(json.dumps(row, separators=(",", ":"), ensure_ascii=False).encode())
                snapshot.update(b"\n")
        summary = {"schema_version": 1, "phase": "selected_native_review_propagation", "target_execution": False,
                   "database": a.rel(Path(db_path)), "normalization": VERSION,
                   "tool": {"path": a.rel(Path(__file__)), "sha256": a.sha(Path(__file__))},
                   "initial_identity_receipt": {"path": ".analysis/results/code-identity-native.json",
                                                "sha256": a.sha(a.ROOT / ".analysis/results/code-identity-native.json")},
                   "native_review_tables_sha256": snapshot.hexdigest(), **preparation,
                   "indexed_functions_after_shared_dependencies": db.execute("SELECT count(*) FROM native_functions").fetchone()[0],
                   "native_code_variants_after_shared_dependencies": db.execute("SELECT count(DISTINCT normalized_sha) FROM native_functions").fetchone()[0],
                   "matched_products": db.execute("SELECT count(DISTINCT product_id) FROM native_review_matches").fetchone()[0],
                   "matched_sections": db.execute("SELECT count(DISTINCT section_id) FROM native_review_matches").fetchone()[0],
                   "matched_code_scopes": db.execute("SELECT count(*) FROM native_review_matches").fetchone()[0],
                   "unproven_capability_code_candidates": db.execute("SELECT count(*) FROM native_review_code_candidates").fetchone()[0],
                   "reviewed_code_identities": db.execute("SELECT count(DISTINCT normalized_sha) FROM native_review_owners WHERE status='selected_code_scope'").fetchone()[0],
                   "basis": "code_scope_only", "whole_function_closed": False, "whole_capability_closed": False,
                   "limits": ["Only explicit selected owner lists attached to clean mapped semantic reviews are carried",
                              "Capture/artifact inventories and unproven container loading never confer reviewed scope",
                              "Identical native code without an explicitly reviewed dependency in the same capability remains a code candidate",
                              "Identical native code does not establish equal product configuration, callers or runtime behavior"]}
        with db:
            db.execute("INSERT OR REPLACE INTO native_meta VALUES('review_summary',?)", (json.dumps(summary, sort_keys=True),))
        return summary
