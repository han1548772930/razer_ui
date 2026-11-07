"""Bind retained review scopes to current code identities and queue unique variants.

An owner records reviewed source scope. Even a complete factory source owner
does not prove that every behavior in that factory was reviewed. Propagation
therefore preserves code evidence without promoting an entire capability.
"""
from collections import defaultdict
from contextlib import closing
import hashlib
import json
from pathlib import Path

import analysis as a
import code_identity as ci

REPORT = a.ROOT / ".analysis/results/code-identity-propagation.json"
QUEUE = a.ROOT / ".analysis/results/variant-work-queue.json"


def assign_capabilities(db_path, requests):
    """Claim complete capability candidate sets, with one owner per shared variant."""
    claims = [request.split(",", 2) for request in requests]
    if not 1 <= len(claims) <= 3 or any(len(c) != 3 for c in claims) or len({c[0] for c in claims}) != len(claims):
        raise ValueError("Assign one to three distinct owner,family,capability requests")
    with closing(ci.connect(db_path)) as db:
        signature = source_signature(db)
        if dict(db.execute("SELECT key,value FROM metadata")).get("review_source_signature") != signature:
            raise ValueError("Assignments require current propagated identities")
        db.executescript("""
            CREATE TABLE IF NOT EXISTS variant_assignments(normalized_sha TEXT PRIMARY KEY,
                owner TEXT NOT NULL, family TEXT NOT NULL, capability TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS capability_assignments(owner TEXT PRIMARY KEY, family TEXT,
                capability TEXT, source_signature TEXT, variants_sha TEXT, products TEXT);
        """)
        summaries = []
        with db:
            for owner, family, capability in claims:
                variants = [r[0] for r in db.execute("""SELECT DISTINCT c.normalized_sha
                    FROM capability_candidates c JOIN capability_scope s USING(product_id,capability)
                    WHERE s.family=? AND c.capability=? ORDER BY c.normalized_sha""", (family, capability))]
                products = [r[0] for r in db.execute("SELECT product_id FROM capability_scope WHERE family=? AND capability=? ORDER BY product_id", (family, capability))]
                if not variants or not owner:
                    raise ValueError("Assignment has no verified capability candidate scope")
                values = (owner, family, capability, signature, ci.digest(variants), json.dumps(products))
                previous = db.execute("SELECT * FROM capability_assignments WHERE owner=?", (owner,)).fetchone()
                if previous is not None and (tuple(previous)[1:4] != values[1:4] or previous[5] != values[5]):
                    raise ValueError("An existing owner has a different reserved scope")
                db.execute("INSERT OR REPLACE INTO capability_assignments VALUES(?,?,?,?,?,?)", values)
                shared, owned, data = {}, [], []
                for variant in variants:
                    db.execute("INSERT OR IGNORE INTO variant_assignments VALUES(?,?,?,?)", (variant, owner, family, capability))
                    actual = db.execute("SELECT owner FROM variant_assignments WHERE normalized_sha=?", (variant,)).fetchone()[0]
                    state = db.execute("SELECT state FROM variant_queue WHERE normalized_sha=?", (variant,)).fetchone()
                    if state and state[0] == "verified_data_dependency":
                        data.append(variant)
                    elif actual == owner:
                        owned.append(variant)
                    else:
                        shared.setdefault(actual, []).append(variant)
                summaries.append({"owner": owner, "family": family, "capability": capability,
                    "products": products, "candidate_variants": len(variants), "variant_set_sha256": ci.digest(variants),
                    "owned_variants": owned, "shared_dependencies": shared, "verified_data_dependencies": data,
                    "acceptance": "Current application entry, actual capability consumers and full local dependency chain; candidate membership alone is not closure"})
    import reverse_pending
    prior = QUEUE.read_bytes()
    retained = reverse_pending.retain_review_json(raw=prior)
    queue = json.loads(prior)
    queue.update(assignments=summaries, assignment_source_signature=signature, prior_queue=retained)
    a.save(QUEUE, queue)
    return {"queue": a.rel(QUEUE), "sha256": a.sha(QUEUE), "assignments": [
        {k:v for k,v in row.items() if k not in {"owned_variants", "shared_dependencies", "products", "verified_data_dependencies"}} |
        {"products": len(row["products"]), "owned_variants": len(row["owned_variants"]),
         "verified_data_dependencies": len(row["verified_data_dependencies"]),
         "shared_variants": sum(len(v) for v in row["shared_dependencies"].values())} for row in summaries]}


def schema(db):
    db.executescript("""
        CREATE TABLE IF NOT EXISTS review_sections(
            id TEXT PRIMARY KEY, receipt TEXT, pointer TEXT, canonical_sha TEXT,
            state TEXT, scope TEXT, product_ids TEXT, capabilities TEXT, owner_count INTEGER,
            verification_errors TEXT);
        CREATE TABLE IF NOT EXISTS review_owners(
            section_id TEXT, owner_id TEXT, source_path TEXT, source_sha TEXT,
            start_byte INTEGER, end_byte INTEGER, body_sha TEXT, normalized_sha TEXT,
            source_coverage TEXT, status TEXT, detail TEXT,
            PRIMARY KEY(section_id,owner_id));
        CREATE INDEX IF NOT EXISTS review_variant ON review_owners(normalized_sha);
        CREATE TABLE IF NOT EXISTS review_matches(
            section_id TEXT, owner_id TEXT, product_id INTEGER, normalized_sha TEXT,
            PRIMARY KEY(section_id,owner_id,product_id,normalized_sha));
        CREATE INDEX IF NOT EXISTS review_match_product ON review_matches(product_id,section_id);
        CREATE TABLE IF NOT EXISTS capability_scope(
            product_id INTEGER, family TEXT, capability TEXT, source_state TEXT,
            PRIMARY KEY(product_id,capability));
        CREATE TABLE IF NOT EXISTS capability_roots(
            product_id INTEGER, capability TEXT, normalized_sha TEXT, basis TEXT,
            PRIMARY KEY(product_id,capability,normalized_sha));
        CREATE TABLE IF NOT EXISTS capability_candidates(
            product_id INTEGER, capability TEXT, normalized_sha TEXT, basis TEXT,
            PRIMARY KEY(product_id,capability,normalized_sha,basis));
        CREATE INDEX IF NOT EXISTS capability_variant ON capability_candidates(normalized_sha);
        CREATE TABLE IF NOT EXISTS variant_queue(
            normalized_sha TEXT PRIMARY KEY, state TEXT, product_count INTEGER,
            primary_capability TEXT, families TEXT, capabilities TEXT);
        CREATE TABLE IF NOT EXISTS variant_roles(
            normalized_sha TEXT PRIMARY KEY, role TEXT NOT NULL, pure_data INTEGER NOT NULL,
            receipt TEXT NOT NULL, receipt_sha TEXT NOT NULL, proof_sha TEXT NOT NULL);
    """)


def source_signature(db):
    sources = [list(r) for r in db.execute("""SELECT d.product_id,d.family,d.layer,d.acquisition_sha,
        d.failure,s.sha256,p.normalization,p.failure FROM deployments d
        LEFT JOIN deployment_sources ds USING(product_id,layer) LEFT JOIN sources s USING(path)
        LEFT JOIN parsed_sources p ON p.sha256=s.sha256
        ORDER BY d.product_id,d.layer,ds.path""")]
    extraction = dict(db.execute("SELECT key,value FROM metadata WHERE key IN ('normalization','factory_extraction_version')"))
    shared = [list(r) for r in db.execute("""SELECT d.name,d.entry_url,d.acquisition_sha,
        d.failure,s.sha256,p.normalization,p.failure FROM shared_deployments d
        LEFT JOIN shared_sources ds USING(name) LEFT JOIN sources s USING(path)
        LEFT JOIN parsed_sources p ON p.sha256=s.sha256 ORDER BY d.name,ds.path""")]
    return ci.digest({"sources": sources, "shared_sources": shared, "extraction": extraction})


def navigation_signature(db):
    if not db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='navigation_products'").fetchone():
        return None
    digest = hashlib.sha256()
    for row in db.execute("SELECT product_id,receipt FROM navigation_products ORDER BY product_id"):
        digest.update(ci.digest(list(row)).encode())
    return digest.hexdigest()


def bind_navigation(db):
    """Bind current lexical mount evidence without inventing unresolved roots."""
    if navigation_signature(db) is None:
        return {}, {"products": 0, "implementation_roots": 0, "stale_products": []}
    current = {r[0]: r[1] for r in db.execute("SELECT product_id,acquisition_sha FROM deployments WHERE layer='ui' AND failure IS NULL")}
    reports, sources, owners, stale = {}, {}, {}, []
    for row in db.execute("SELECT product_id,receipt FROM navigation_products ORDER BY product_id"):
        report = json.loads(row[1])
        if report.get("acquisition_sha256") != current.get(row[0]) or report.get("target_execution") is not False:
            stale.append(row[0])
            continue
        reports[row[0]] = report
        for owner_id, owner in report["owners"].items():
            original = owner["original"]
            identity = {"path": original["path"], "bytes": original["bytes"], "sha256": original["sha256"]}
            if identity["path"] in sources and sources[identity["path"]] != identity:
                raise ValueError("Navigation owners disagree on source identity")
            sources[identity["path"]] = identity
            fields = {k: owner[k] for k in ("start_byte", "end_byte", "syntax", "raw_sha256", "normalized_sha256")}
            item = {"original": identity, **fields}
            if owner_id in owners:
                previous = owners[owner_id]
                if (any(previous[k] != item[k] for k in fields)
                        or any(previous["original"][k] != identity[k] for k in ("sha256", "bytes"))):
                    raise ValueError("Navigation owner identity has conflicting spans")
            else:
                owners[owner_id] = item
        for page in report["pages"]:
            required = [page["component"], *page.get("mount_path", []), *page.get("props", {}).values()]
            required.extend(page[k] for k in ("row", "name_owner", "lookup_predicate") if k in page)
            required.extend(page.get("navigation_consumers", []))
            required.extend(c["owner"] for c in page.get("conditions", []) if "owner" in c)
            if any(owner_id not in report["owners"] for owner_id in required):
                raise ValueError("Navigation page has an unbound source owner")
    by_source = defaultdict(list)
    for owner in owners.values():
        by_source[owner["original"]["path"]].append(owner)
    for name, original in sources.items():
        raw = a.resolve(name, ".analysis/inputs").read_bytes()
        if len(raw) != original["bytes"] or hashlib.sha256(raw).hexdigest() != original["sha256"]:
            raise ValueError("Current navigation source changed: " + name)
        for owner in by_source[name]:
            start, end = owner["start_byte"], owner["end_byte"]
            if (type(start) is not int or type(end) is not int or not 0 <= start < end <= len(raw)
                    or hashlib.sha256(memoryview(raw)[start:end]).hexdigest() != owner["raw_sha256"]):
                raise ValueError("Current navigation AST owner changed")
    db.execute("DELETE FROM factories WHERE kind='mounted_component'")
    roots = set()
    for report in reports.values():
        for page in report["pages"]:
            owner = owners[page["component"]]
            if page.get("binding_resolution") != "implementation_root" or owner["syntax"] not in {
                    "class", "class_declaration", "function_expression", "function_declaration", "arrow_function"}:
                continue
            start, end = owner["start_byte"], owner["end_byte"]
            db.execute("INSERT OR IGNORE INTO factories VALUES(?,?,?,?,?,?,?,?)", (
                owner["original"]["sha256"], start, end, "component@" + str(start), owner["normalized_sha256"],
                owner["raw_sha256"], "mounted_component", '["page_root_dependency_scope_pending"]'))
            roots.add(owner["normalized_sha256"])
    return reports, {"products": len(reports), "implementation_roots": len(roots), "stale_products": stale,
                     "source_files_verified": len(sources),
                     "products_with_unresolved_scope": sum(not r.get("source_scope_complete") for r in reports.values())}


def populate_scope(db, manifest, product_doc, navigation=None):
    pages = {row["product_id"]: row for row in product_doc["products"]}
    navigation = navigation or {}
    for family, spec in manifest["families"].items():
        for pid in spec["product_ids"]:
            registered = db.execute("""SELECT r.* FROM deployments d
                JOIN deployment_sources ds USING(product_id,layer) JOIN sources s USING(path)
                JOIN registrations r ON r.source_sha=s.sha256
                WHERE d.product_id=? AND d.layer='mw' AND d.failure IS NULL
                AND r.feature='genericFeature' ORDER BY ds.path,r.start_byte""", (pid,))
            for row in registered:
                for key in json.loads(row["generic_keys"]):
                    db.execute("INSERT OR IGNORE INTO capability_scope VALUES(?,?,?,?)",
                               (pid, family, "feature:" + key, "current_registration"))
                    db.execute("""INSERT OR IGNORE INTO capability_roots
                        SELECT ?,?,normalized_sha,'current_feature_registration_factory' FROM factories
                        WHERE source_sha=? AND start_byte<=? AND end_byte>=?""",
                               (pid, "feature:" + key, row["source_sha"], row["start_byte"], row["end_byte"]))
            page = pages.get(pid)
            mounted_report = navigation.get(pid)
            if mounted_report is not None:
                complete = mounted_report.get("source_scope_complete") is True and not mounted_report.get("failures")
                mounted_kinds = set()
                for mounted in mounted_report["pages"]:
                    capability = "page:" + mounted["kind"]
                    mounted_kinds.add(mounted["kind"])
                    implementation = mounted.get("binding_resolution") == "implementation_root"
                    resolved = implementation and mounted.get("selection_status") in {"first_match_candidate", "standalone_consumer", "keyed_selection_candidate"}
                    state = ("current_navigation_complete" if complete and resolved else
                             "current_navigation_partial" if resolved else
                             "current_navigation_selection_unresolved" if implementation else "current_navigation_wrapper_unresolved")
                    existing = db.execute("SELECT source_state FROM capability_scope WHERE product_id=? AND capability=?", (pid, capability)).fetchone()
                    if existing is None or resolved:
                        db.execute("INSERT OR REPLACE INTO capability_scope VALUES(?,?,?,?)", (pid, family, capability, state))
                    if implementation:
                        owner = mounted_report["owners"][mounted["component"]]
                        db.execute("INSERT OR IGNORE INTO capability_candidates VALUES(?,?,?,?)", (
                            pid, capability, owner["normalized_sha256"], "current_navigation_declared_implementation"))
                        if resolved:
                            db.execute("INSERT OR IGNORE INTO capability_roots VALUES(?,?,?,?)", (
                                pid, capability, owner["normalized_sha256"], "current_navigation_implementation_root"))
                if complete:
                    continue
            if page is None:
                continue
            current = {r[0] for r in db.execute("""SELECT s.sha256 FROM deployments d
                JOIN deployment_sources ds USING(product_id,layer) JOIN sources s USING(path)
                WHERE d.product_id=? AND d.layer='ui' AND d.failure IS NULL""", (pid,))}
            originals = {s["path"]: s["sha256"] for s in page.get("source_inputs", [])}
            mounts = {navigation["source"] for navigation in page.get("navigations", [])
                      if isinstance(navigation.get("source"), str)}
            verified = bool(mounts) and all(originals.get(path) in current for path in mounts)
            for kind in page.get("current_page_kinds", []):
                db.execute("INSERT OR IGNORE INTO capability_scope VALUES(?,?,?,?)",
                           (pid, family, "page:" + kind,
                            "current_mount_source_identity" if verified else "mount_source_identity_unmatched"))
            if verified:
                for legacy_navigation in page.get("navigations", []):
                    for mounted in legacy_navigation.get("pages", []):
                        component = mounted.get("component")
                        if not isinstance(component, dict):
                            continue
                        source_sha = originals.get(component.get("source"))
                        if source_sha not in current:
                            continue
                        db.execute("""INSERT OR IGNORE INTO capability_roots
                            SELECT ?,?,normalized_sha,'current_mounted_component_factory' FROM factories
                            WHERE source_sha=? AND start_byte<=? AND end_byte>=?""",
                                   (pid, "page:" + mounted["kind"], source_sha,
                                    component.get("start_byte"), component.get("end_byte")))


class OwnerVerifier:
    """Keep only one original body in memory while reusing verified identities."""
    def __init__(self, db):
        self.db = db
        self.current_sha = None
        self.current_raw = None
        self.verified = {}

    def raw(self, original):
        path = a.resolve(original["path"], ".analysis/inputs")
        expected = original["sha256"]
        raw = path.read_bytes()
        if len(raw) != original["bytes"] or hashlib.sha256(raw).hexdigest() != expected:
            raise ValueError("Original source byte identity differs")
        self.verified[original["path"]] = (original["bytes"], expected)
        self.current_sha, self.current_raw = expected, raw
        return raw

    def verify(self, owner):
        original = owner.get("original")
        if not isinstance(original, dict) or not {"path", "sha256", "bytes"}.issubset(original):
            return {"status": "unhandled_owner_shape", "detail": "Owner has no original source identity"}
        start, end = owner.get("byte_start"), owner.get("byte_end")
        if type(start) is not int or type(end) is not int or not 0 <= start < end <= original["bytes"]:
            return {"status": "invalid_owner", "detail": "Owner byte range is invalid"}
        raw = self.current_raw if self.current_sha == original["sha256"] else self.raw(original)
        # A second path claiming the same SHA must still be verified independently.
        if original["path"] not in self.verified:
            raw = self.raw(original)
        body = raw[start:end]
        body_sha = hashlib.sha256(body).hexdigest()
        if body_sha != owner.get("body_sha256") or ("body" in owner and body.decode("utf-8") != owner["body"]):
            return {"status": "invalid_owner", "detail": "Retained owner body differs from its original bytes"}
        factories = list(self.db.execute("""SELECT * FROM factories WHERE source_sha=?
            AND start_byte<=? AND end_byte>=? ORDER BY end_byte-start_byte""", (original["sha256"], start, end)))
        wrapped_factory = False
        if not factories and owner.get("syntax") == "pair":
            import frontend
            import host
            if not hasattr(self, "parser"):
                self.parser = host.HostLocalAudit().parser
            tree = self.parser.parse(b"({" + body + b"})")
            if not tree.root_node.has_error:
                pairs = [node for node in frontend.nodes(tree.root_node)
                         if node.type == "pair" and node.start_byte == 2 and node.end_byte == len(body) + 2]
                if len(pairs) == 1:
                    pair = pairs[0]
                    value = pair.child_by_field_name("value")
                    module_id = frontend.webpack_module_id(pair.child_by_field_name("key"))
                    if module_id is not None and value is not None:
                        factories = list(self.db.execute("""SELECT * FROM factories WHERE source_sha=?
                            AND start_byte=? AND end_byte=? AND module_id=? AND normalized_sha=?""",
                            (original["sha256"], start + value.start_byte - 2, start + value.end_byte - 2,
                             str(module_id), ci.normalized_ast(value, factory=True))))
                        wrapped_factory = bool(factories)
        if not factories:
            current = self.db.execute("""SELECT 1 FROM sources s JOIN deployment_sources ds USING(path)
                JOIN deployments d USING(product_id,layer) WHERE s.sha256=? AND d.failure IS NULL
                UNION ALL SELECT 1 FROM sources s JOIN shared_sources ds USING(path)
                JOIN shared_deployments d USING(name) WHERE s.sha256=? AND d.failure IS NULL LIMIT 1""",
                                      (original["sha256"], original["sha256"])).fetchone()
            return {"status": "verified_current_non_factory_scope" if current else "source_not_in_current_factory_index",
                    "detail": "Original bytes verified; owner lies outside indexed factories" if current else
                              "Verified original owner has no current indexed source or containing factory"}
        factory = factories[0]
        coverage = "whole_factory_source" if wrapped_factory or (factory["start_byte"], factory["end_byte"]) == (start, end) else "source_slice"
        return {"status": "matched", "detail": "Source identity only; dependency bindings and runtime inputs retain their own scope",
                "normalized_sha": factory["normalized_sha"], "source_coverage": coverage}

    def final_snapshot(self):
        changed = []
        for name, (size, sha) in self.verified.items():
            path = a.resolve(name, ".analysis/inputs")
            if path.stat().st_size != size or a.sha(path) != sha:
                changed.append(name)
        return changed


def section_state(section):
    clean = (section.get("target_execution") is False and section.get("failures") == []
             and section.get("changed_during_query") == [])
    return "closed" if clean and section.get("source_chain_complete") is True else "partial" if clean else "invalid"


def owner_records(section):
    """Use the retained full-owner pointer without copying source into new receipts."""
    if "additional_complete_owner_references" in section:
        references = section["additional_complete_owner_references"]
        if not isinstance(references, list) or any(not isinstance(ref, dict) for ref in references):
            raise ValueError("Additional complete owner references must be a list of references")
        primary = {key: value for key, value in section.items()
                   if key != "additional_complete_owner_references"}
        streams = [owner_records(primary)]
        streams.extend(owner_records({"complete_owner_reference": ref}) for ref in references)
        seen = {}
        for records in streams:
            for owner in records:
                if not isinstance(owner, dict) or not isinstance(owner.get("original"), dict):
                    raise ValueError("Invalid retained owner record")
                owner_id = owner.get("id") or ci.digest(owner)
                if not isinstance(owner_id, str):
                    raise ValueError("Retained owner identity must be a string")
                original = owner["original"]
                identity = (original.get("sha256"), original.get("bytes"),
                            owner.get("byte_start"), owner.get("byte_end"), owner.get("body_sha256"))
                if owner_id in seen:
                    if seen[owner_id] != identity:
                        raise ValueError("Conflicting retained owner identity: " + owner_id)
                    continue
                seen[owner_id] = identity
                yield owner
        return
    owners = section.get("complete_source_owners", section.get("complete_owners"))
    if owners is not None:
        yield from owners.values() if isinstance(owners, dict) else owners
        reused = section.get("reused_owners")
        if isinstance(reused, dict) and "reference" in reused:
            selected = {item["owner_id"]: item for item in reused["owners"]}
            seen = set()
            for owner in owner_records({"complete_owner_reference": reused["reference"]}):
                expected = selected.get(owner["id"])
                if expected is None:
                    continue
                if ("original_sha256" in expected and owner["original"]["sha256"] != expected["original_sha256"]
                        or owner["body_sha256"] != expected["body_sha256"]):
                    raise ValueError("Selected reused owner differs from its retained reference")
                seen.add(owner["id"])
                yield owner
            if seen != set(selected):
                raise ValueError("Selected reused owner is absent from its retained reference")
        if not section.get("reused_owner_verification"):
            return
        originals = {}
        for source in section.get("reused_source_identities", []):
            original = source["original"]
            previous = originals.setdefault(original["sha256"], original)
            if previous["bytes"] != original["bytes"]:
                raise ValueError("Reused source identities disagree on byte length")
        proofs = {row["pointer"][1:] for row in section.get("upstream_sections", [])
                  if isinstance(row, dict) and isinstance(row.get("pointer"), str)
                  and row["pointer"].startswith("/")}
        for owner in section.get("reused_owner_verification", []):
            original = originals.get(owner["original_sha256"])
            if original is None or not owner.get("proof_sections") or not set(owner["proof_sections"]) <= proofs:
                raise ValueError("Reused owner has no declared source or upstream proof")
            start, end = owner["byte_range"]
            yield {"id": owner["owner_id"], "original": original,
                   "byte_start": start, "byte_end": end,
                   "body_sha256": owner["body_sha256"]}
        return
    reference = section.get("complete_owner_reference", section.get("complete_source_owners_reference"))
    if ("complete_owner_reference" in section and "complete_source_owners_reference" in section
            and section["complete_owner_reference"] != section["complete_source_owners_reference"]):
        raise ValueError("Complete owner references disagree")
    if not isinstance(reference, dict):
        return
    path = a.resolve(reference["path"], ".analysis/results")
    if path.stat().st_size != reference["bytes"] or a.sha(path) != reference["sha256"]:
        raise ValueError("Retained review-source byte identity differs")
    pointer = reference.get("pointer", "/complete_owners")
    if not isinstance(pointer, str) or pointer.count("/") != 1:
        raise ValueError("Unsupported retained owner pointer: " + str(pointer))
    for key, value in ci.stream_sections(path):
        if key == pointer[1:]:
            yield from value.values() if isinstance(value, dict) else value
            if path.stat().st_size != reference["bytes"] or a.sha(path) != reference["sha256"]:
                raise ValueError("Retained review-source changed during propagation")
            return
    raise ValueError("Retained owner pointer is absent")


def review_section(db, verifier, receipt, name, section, mapping):
    canonical = ci.digest(section)
    section_id = receipt + "#/" + name
    state = section_state(section)
    owners = 0
    errors = []
    if state != "invalid":
        try:
            for owner in owner_records(section):
                owners += 1
                if not isinstance(owner, dict):
                    raise ValueError("Invalid retained owner record")
                original = owner.get("original")
                original = original if isinstance(original, dict) else {}
                owner_id = owner.get("id") or ci.digest(owner)
                try:
                    match = verifier.verify(owner)
                except (OSError, ValueError, KeyError, UnicodeError) as error:
                    match = {"status": "invalid_owner", "detail": str(error)}
                if match["status"] == "invalid_owner":
                    errors.append({"owner": owner_id, "error": match["detail"]})
                db.execute("INSERT OR REPLACE INTO review_owners VALUES(?,?,?,?,?,?,?,?,?,?,?)", (
                    section_id, owner_id, original.get("path"), original.get("sha256"),
                    owner.get("byte_start"), owner.get("byte_end"), owner.get("body_sha256"),
                    match.get("normalized_sha"), match.get("source_coverage"), match["status"], match["detail"]))
                if match["status"] == "matched":
                    db.execute("""INSERT OR IGNORE INTO review_matches
                        SELECT ?,?,product_id,normalized_sha FROM product_variants WHERE normalized_sha=?""",
                               (section_id, owner_id, match["normalized_sha"]))
        except (OSError, ValueError, KeyError) as error:
            errors.append({"error": str(error)})
    if errors:
        state = "invalid"
        db.execute("DELETE FROM review_matches WHERE section_id=?", (section_id,))
    db.execute("INSERT OR REPLACE INTO review_sections VALUES(?,?,?,?,?,?,?,?,?,?)", (
        section_id, receipt, "/" + name, canonical, state, str(section.get("scope", "")),
        json.dumps(sorted(mapping["product_ids"])), json.dumps(sorted(mapping["capabilities"])),
        owners, json.dumps(errors)))


def populate_candidates(db):
    db.execute("""INSERT OR IGNORE INTO capability_candidates
        SELECT product_id,capability,normalized_sha,basis FROM capability_roots""")
    db.execute("""INSERT OR IGNORE INTO capability_candidates
        SELECT cs.product_id,cs.capability,f.normalized_sha,'ast_token_candidate'
        FROM capability_scope cs JOIN deployment_sources ds ON ds.product_id=cs.product_id
        JOIN deployments d ON d.product_id=ds.product_id AND d.layer=ds.layer
        JOIN sources s USING(path) JOIN markers m ON m.source_sha=s.sha256
        JOIN factories f ON f.source_sha=m.source_sha AND f.start_byte=m.start_byte AND f.end_byte=m.end_byte
        WHERE d.failure IS NULL AND m.token=substr(cs.capability,instr(cs.capability,':')+1)""")
    for section in db.execute("SELECT id,capabilities FROM review_sections WHERE state<>'invalid'").fetchall():
        for capability in json.loads(section["capabilities"]):
            db.execute("""INSERT OR IGNORE INTO capability_candidates
                SELECT m.product_id,cs.capability,m.normalized_sha,'retained_review_scope'
                FROM review_matches m JOIN capability_scope cs ON cs.product_id=m.product_id
                WHERE m.section_id=? AND cs.capability=?""", (section["id"], capability))


def populate_queue(db):
    variants = db.execute("SELECT DISTINCT normalized_sha FROM capability_candidates ORDER BY normalized_sha").fetchall()
    for (variant,) in variants:
        grouped = db.execute("""SELECT cc.capability,COUNT(DISTINCT cc.product_id) count
            FROM capability_candidates cc WHERE cc.normalized_sha=? GROUP BY cc.capability
            ORDER BY count DESC,cc.capability""", (variant,)).fetchall()
        products = db.execute("""SELECT DISTINCT cc.product_id,cs.family FROM capability_candidates cc
            JOIN capability_scope cs USING(product_id,capability) WHERE cc.normalized_sha=?""", (variant,)).fetchall()
        reviewed = db.execute("SELECT 1 FROM review_matches WHERE normalized_sha=? LIMIT 1", (variant,)).fetchone()
        role = db.execute("SELECT pure_data FROM variant_roles WHERE normalized_sha=?", (variant,)).fetchone()
        state = "verified_data_dependency" if role and role[0] else "partial_review_scope" if reviewed else "unreviewed"
        db.execute("INSERT INTO variant_queue VALUES(?,?,?,?,?,?)", (
            variant, state, len(products), grouped[0]["capability"],
            json.dumps(sorted({r["family"] for r in products})), json.dumps([r["capability"] for r in grouped])))


def import_roles(db, references):
    """Carry proven literal-export roles; executable runtimes stay in the queue."""
    rows, sources, seen, unmatched, scopes = {}, {}, set(), [], []
    for reference in references:
        path = a.resolve(reference["path"], ".analysis/results")
        raw = path.read_bytes()
        if hashlib.sha256(raw).hexdigest() != reference["sha256"]:
            raise ValueError("Variant role receipt changed: " + reference["path"])
        report = json.loads(raw)
        if (report.get("schema_version") != 1 or report.get("kind") != "assigned_code_variant_roles" or report.get("target_execution") is not False
                or report.get("failures") != []):
            raise ValueError("Role receipt is not a successful static inspection")
        variants = report["variants"]
        identities = [r["normalized_sha256"] for r in variants]
        actual_set = ci.digest(sorted(identities))
        if len(identities) != len(set(identities)) or len(identities) != report["variant_count"]:
            raise ValueError("Role receipt lost its complete selected variant set")
        if "selected_variant_set_sha256" in report:
            if actual_set != report["selected_variant_set_sha256"]:
                raise ValueError("Role selected variant identity differs")
            scope_basis = "declared_set_sha256"
        else:
            actual_counts = defaultdict(int)
            for row in variants:
                actual_counts[row["role"]] += 1
            if "reserved_variant_count" in report or dict(actual_counts) != report.get("role_counts"):
                raise ValueError("Legacy role receipt has an incomplete file-bound inventory")
            scope_basis = "immutable_receipt_bytes_and_complete_inventory"
        scopes.append({"receipt": reference["path"], "basis": scope_basis, "variant_count": len(identities),
                       "computed_set_sha256": actual_set})
        for row in variants:
            role, pure = row["role"], row.get("pure_data", False)
            if (role not in {"static_string_exports", "locale_runtime_with_dictionaries", "unclassified"}
                    or type(pure) is not bool or pure != (role == "static_string_exports")):
                raise ValueError("Role classification has conflicting executable/data semantics")
            original = row["original"]
            identity = (original["sha256"], row["start_byte"], row["end_byte"], str(row["module_id"]), row["normalized_sha256"])
            witness = db.execute("""SELECT 1 FROM factories f
                WHERE f.source_sha=? AND f.start_byte=? AND f.end_byte=? AND f.module_id=?
                AND f.normalized_sha=? LIMIT 1""", identity).fetchone()
            current = db.execute("SELECT 1 FROM product_variants WHERE normalized_sha=? LIMIT 1", (row["normalized_sha256"],)).fetchone()
            if witness is None or current is None:
                unmatched.append({"receipt": reference["path"], "normalized_sha": row["normalized_sha256"],
                                  "reason": "owner_not_indexed" if witness is None else "variant_not_current"})
                continue
            source_key = original["path"]
            claim = (original["bytes"], original["sha256"])
            if source_key in sources and sources[source_key] != claim:
                raise ValueError("Role receipts disagree on original byte identity")
            sources[source_key] = claim
            variant = row["normalized_sha256"]
            if variant in rows and rows[variant][1:3] != (role, int(pure)):
                raise ValueError("Conflicting role proofs for one code identity")
            rows[variant] = (variant, role, int(pure), reference["path"], reference["sha256"], ci.digest(row))
        seen.add(reference["path"])
    if len(seen) != len(references):
        raise ValueError("Duplicate variant role receipts")
    for name, (size, sha) in sources.items():
        path = a.resolve(name, ".analysis/inputs")
        if path.stat().st_size != size or a.sha(path) != sha:
            raise ValueError("Variant role original changed: " + name)
    db.execute("DELETE FROM variant_roles")
    db.executemany("INSERT INTO variant_roles VALUES(?,?,?,?,?,?)", rows.values())
    db.execute("INSERT OR REPLACE INTO metadata VALUES('review_role_evidence',?)", (json.dumps(references),))
    db.execute("INSERT OR REPLACE INTO metadata VALUES('review_unmatched_roles',?)", (json.dumps(unmatched),))
    db.execute("INSERT OR REPLACE INTO metadata VALUES('review_role_scopes',?)", (json.dumps(scopes),))
    return {row[0]: row[1] for row in db.execute("SELECT role,COUNT(*) FROM variant_roles GROUP BY role")}


def queue_summary(db):
    groups = []
    for row in db.execute("""SELECT primary_capability,COUNT(*) variants,
        SUM(CASE WHEN state='unreviewed' THEN 1 ELSE 0 END) unreviewed,
        MAX(product_count) maximum_shared_products FROM variant_queue WHERE state<>'verified_data_dependency'
        GROUP BY primary_capability ORDER BY maximum_shared_products DESC,variants DESC,primary_capability"""):
        groups.append(dict(row))
    previous = json.loads(QUEUE.read_bytes()) if QUEUE.exists() else {}
    reservations = {key: previous[key] for key in ("assignments", "assignment_source_signature", "prior_queue", "role_evidence") if key in previous}
    if reservations:
        reservations["assignment_source_is_current"] = previous.get("assignment_source_signature") == source_signature(db)
    return {"schema_version": 1, "database": a.rel(ci.DATABASE), **reservations,
        "queue_table": "variant_queue", "assignment_key": "normalized_sha",
        "groups": groups,
        "verified_data_dependencies": db.execute("SELECT COUNT(*) FROM variant_queue WHERE state='verified_data_dependency'").fetchone()[0],
        "limits": [
            "Each variant is assigned once; primary_capability is a scheduling owner, not proof of exclusive ownership.",
            "Groups combine retained review scopes with AST token candidates; confirm full capability chains before closure.",
            "Partially reviewed factory source remains queued. Unclassified factories are not counted as remaining capability work.",
            "Proven literal exports remain source dependencies but are excluded from behavioral work groups; locale runtimes remain queued."]}


def build(db_path=ci.DATABASE):
    manifest_path = a.ROOT / "tools/capability-ledger.json"
    manifest_sha = a.sha(manifest_path)
    manifest = json.loads(manifest_path.read_bytes())
    product_doc_path = a.ROOT / ".analysis/results/product-documentation.json"
    product_doc_sha = a.sha(product_doc_path)
    product_doc = json.loads(product_doc_path.read_bytes())
    mapped = defaultdict(lambda: defaultdict(lambda: {"product_ids": set(), "capabilities": set()}))
    for entry in manifest["capability_sections"]:
        for ref in entry["sections"]:
            mapped[ref["receipt"]][ref["section"]]["product_ids"].update(entry["product_ids"])
            mapped[ref["receipt"]][ref["section"]]["capabilities"].update(entry["capabilities"])
    with closing(ci.connect(db_path)) as db:
        schema(db)
        pending = db.execute("""SELECT COUNT(DISTINCT s.sha256) FROM deployment_sources ds
            JOIN sources s USING(path) LEFT JOIN parsed_sources p ON p.sha256=s.sha256
            WHERE p.sha256 IS NULL OR p.normalization<>?""", (ci.NORMALIZATION,)).fetchone()[0]
        if pending:
            raise ValueError(f"Current frontend index still has {pending} unparsed source identities")
        signature = source_signature(db)
        nav_signature = navigation_signature(db)
        verifier = OwnerVerifier(db)
        receipt_identities = []
        unmapped_sections = []
        role_references = json.loads(QUEUE.read_bytes()).get("role_evidence", []) if QUEUE.exists() else []
        with db:
            for table in ("review_sections", "review_owners", "review_matches", "capability_scope", "capability_roots", "capability_candidates", "variant_queue"):
                db.execute(f"DELETE FROM {table}")
            navigation, navigation_scope = bind_navigation(db)
            populate_scope(db, manifest, product_doc, navigation)
            for receipt, sections in mapped.items():
                path = a.resolve(receipt, ".analysis/results")
                before = a.sha(path)
                found = set()
                for name, section in ci.stream_sections(path):
                    if name not in sections:
                        if isinstance(section, dict) and "source_chain_complete" in section:
                            unmapped_sections.append({"receipt": receipt, "pointer": "/" + name,
                                "canonical_sha256": ci.digest(section), "state": section_state(section),
                                "scope": str(section.get("scope", ""))})
                        continue
                    found.add(name)
                    review_section(db, verifier, receipt, name, section, sections[name])
                if found != set(sections):
                    raise ValueError("Mapped formal sections are missing: " + str(sorted(set(sections) - found)))
                if a.sha(path) != before:
                    raise ValueError("Formal review changed during propagation: " + receipt)
                receipt_identities.append({"path": receipt, "sha256": before})
            changed = verifier.final_snapshot()
            if changed:
                raise ValueError("Originals changed during propagation: " + str(changed))
            if a.sha(manifest_path) != manifest_sha or a.sha(product_doc_path) != product_doc_sha:
                raise ValueError("Capability or product scope changed during propagation")
            if navigation_signature(db) != nav_signature:
                raise ValueError("Mounted navigation changed during propagation")
            populate_candidates(db)
            role_counts = import_roles(db, role_references)
            populate_queue(db)
            db.execute("INSERT OR REPLACE INTO metadata VALUES('review_source_signature',?)", (signature,))
            db.execute("INSERT OR REPLACE INTO metadata VALUES('review_manifest_sha',?)", (manifest_sha,))
            db.execute("INSERT OR REPLACE INTO metadata VALUES('review_product_doc_sha',?)", (product_doc_sha,))
            db.execute("INSERT OR REPLACE INTO metadata VALUES('review_receipts',?)", (json.dumps(receipt_identities),))
            db.execute("INSERT OR REPLACE INTO metadata VALUES('review_navigation_signature',?)", (json.dumps(nav_signature),))
        scalar = lambda sql: db.execute(sql).fetchone()[0]
        native_present = db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='native_functions'").fetchone()
        report = {"schema_version": 1, "database": a.rel(ci.DATABASE), "target_execution": False,
            "source_signature": signature, "review_receipts": receipt_identities,
            "navigation_scope": navigation_scope, "navigation_signature": nav_signature,
            "review_sections": scalar("SELECT COUNT(*) FROM review_sections"),
            "review_owners": scalar("SELECT COUNT(*) FROM review_owners"),
            "owner_statuses": {r[0]: r[1] for r in db.execute("SELECT status,COUNT(*) FROM review_owners GROUP BY status")},
            "matched_variants": scalar("SELECT COUNT(DISTINCT normalized_sha) FROM review_matches"),
            "product_variant_matches": scalar("SELECT COUNT(*) FROM (SELECT DISTINCT product_id,normalized_sha FROM review_matches)"),
            "capability_candidate_variants": scalar("SELECT COUNT(*) FROM variant_queue WHERE state<>'verified_data_dependency'"),
            "candidate_review_states": {r[0]: r[1] for r in db.execute("SELECT state,COUNT(*) FROM variant_queue WHERE state<>'verified_data_dependency' GROUP BY state")},
            "verified_data_dependencies": scalar("SELECT COUNT(*) FROM variant_queue WHERE state='verified_data_dependency'"),
            "role_counts": role_counts, "role_evidence": role_references,
            "role_scope_identities": json.loads(db.execute("SELECT value FROM metadata WHERE key='review_role_scopes'").fetchone()[0]),
            "unmatched_role_scopes": json.loads(db.execute("SELECT value FROM metadata WHERE key='review_unmatched_roles'").fetchone()[0]),
            "unclassified_current_factory_variants": scalar("""SELECT COUNT(DISTINCT pv.normalized_sha)
                FROM product_variants pv LEFT JOIN variant_queue q USING(normalized_sha)
                WHERE pv.kind='webpack_factory' AND q.normalized_sha IS NULL"""),
            "capabilities_without_candidates": scalar("""SELECT COUNT(*) FROM capability_scope cs
                WHERE NOT EXISTS(SELECT 1 FROM capability_candidates cc WHERE cc.product_id=cs.product_id AND cc.capability=cs.capability)"""),
            "invalid_sections": [dict(r) for r in db.execute("SELECT id,verification_errors FROM review_sections WHERE state='invalid'")],
            "unmapped_formal_sections": unmapped_sections,
            "native_review_scope": {"indexed_functions": scalar("SELECT COUNT(*) FROM native_functions") if native_present else None,
                                    "review_propagation": False,
                                    "reason": "Native captures require explicit reviewed source-SHA/RVA/function-SHA bindings; capture completeness is not semantic review"},
            "full_remaining_variant_count_known": False,
            "limits": ["Matching source scopes reuse the published observations; dependency and product-input equivalence are not inferred.",
                       "Legacy owners do not assert whole-factory semantic completion. All candidate variants retain unreviewed or partial status.",
                       "Only mapped complete capability sections establish existing capability closure; old remaining_chains are not requeued.",
                       "Unclassified current factories include third-party/runtime code and unresolved capability ownership; their count is not a progress denominator."]}
        a.save(QUEUE, queue_summary(db))
    if native_present:
        import code_identity_native
        report["native_review_scope"] = code_identity_native.propagate_reviews(db_path)
    a.save(REPORT, report)
    return report
