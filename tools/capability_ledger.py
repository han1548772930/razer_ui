"""First-tier product x capability ledger: the single progress denominator.

Capabilities come from the current official deployment of each product: every
middleware ``genericFeature`` ``featureKey`` (other ``useFeature`` names are kept
as injector configuration keys) parsed as complete AST call nodes from the newest verified
acquisition, plus the mounted UI page kinds recorded by the product structure
receipt. Stage status is bound to published scopes through the cross-product
SQLite identity index; partial source evidence never establishes whole-capability
closure on a different product.
"""
import hashlib
import json
from contextlib import closing

import analysis as a

MANIFEST = a.ROOT / "tools/capability-ledger.json"
OUTPUT = a.ROOT / ".analysis/results/capability-ledger.json"
STAGES = ("static", "rust", "ui_backend", "official_comparison")


def _digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                     separators=(",", ":")).encode()).hexdigest()


def _string(node):
    if node is None or node.type != "string":
        return None
    text = node.text.decode("utf-8", "replace")
    return text[1:-1] if len(text) >= 2 else None


def _walk(node):
    stack = [node]
    while stack:
        current = stack.pop()
        yield current
        stack.extend(reversed(current.children))


def _registrations(audit, item, *, tree=None):
    """Return ordered (feature, generic featureKeys) pairs from one original."""
    raw = audit.read(item)
    if b"useFeature" not in raw:
        return []
    tree = tree if tree is not None else audit.parser.parse(raw)
    found = []
    for node in _walk(tree.root_node):
        if node.type != "call_expression":
            continue
        callee = node.child_by_field_name("function")
        prop = callee.child_by_field_name("property") if callee is not None and callee.type == "member_expression" else None
        if prop is None or prop.text != b"useFeature":
            continue
        args = [child for child in node.child_by_field_name("arguments").named_children]
        name = _string(args[0]) if args else None
        if name is None:
            continue
        keys = []
        if name == "genericFeature" and len(args) >= 3 and args[2].type == "object":
            for pair in args[2].named_children:
                key = pair.child_by_field_name("key") if pair.type == "pair" else None
                if key is None or key.text.strip(b"\"'") != b"features":
                    continue
                value = pair.child_by_field_name("value")
                for entry in value.named_children if value is not None and value.type == "array" else []:
                    for field in entry.named_children if entry.type == "object" else []:
                        fkey = field.child_by_field_name("key") if field.type == "pair" else None
                        if fkey is not None and fkey.text.strip(b"\"'") == b"featureKey":
                            text = _string(field.child_by_field_name("value"))
                            if text is not None:
                                keys.append(text)
        found.append({"feature": name, "generic_keys": keys,
                      "owner": {"path": item["path"], "start_byte": node.start_byte, "end_byte": node.end_byte}})
    return found


def _static_state(sections):
    direct = [row for row in sections if row["basis"] == "explicit_product_scope"]
    if direct and all(row["state"] == "closed" and row["current_product_sources_bound"] for row in direct):
        return "closed"
    if any(row["matched_owner_count"] and row["state"] != "invalid" for row in sections):
        return "partial"
    if any(row["state"] == "invalid" for row in sections):
        return "invalid_evidence"
    return "evidence_unmatched" if direct else "not_started"


def _sections(db, pid, capability):
    sections = []
    native_present = db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='native_review_matches'").fetchone()
    for row in db.execute("SELECT * FROM review_sections ORDER BY receipt,pointer"):
        if capability not in json.loads(row["capabilities"]):
            continue
        direct = pid in json.loads(row["product_ids"])
        js_candidates = db.execute("SELECT COUNT(*) FROM review_matches WHERE section_id=? AND product_id=?",
                                   (row["id"], pid)).fetchone()[0]
        js_count = js_candidates if direct else db.execute("""SELECT COUNT(*) FROM review_matches m
            JOIN capability_roots cr ON cr.product_id=m.product_id AND cr.normalized_sha=m.normalized_sha
            WHERE m.section_id=? AND m.product_id=? AND cr.capability=?""", (row["id"], pid, capability)).fetchone()[0]
        non_factory = db.execute("""SELECT COUNT(*) FROM review_owners o WHERE o.section_id=?
            AND o.status='verified_current_non_factory_scope' AND EXISTS(
                SELECT 1 FROM sources s JOIN deployment_sources ds USING(path) JOIN deployments d USING(product_id,layer)
                WHERE s.sha256=o.source_sha AND ds.product_id=? AND d.failure IS NULL)""", (row["id"], pid)).fetchone()[0]
        if direct:
            js_count += non_factory
        native_count = db.execute("""SELECT COUNT(DISTINCT normalized_sha) FROM native_review_matches
            WHERE section_id=? AND product_id=?""", (row["id"], pid)).fetchone()[0] if native_present else 0
        count = js_count + native_count
        if not direct and not count and not js_candidates:
            continue
        missing = db.execute("""SELECT COUNT(*) FROM review_owners o WHERE section_id=?
            AND source_path LIKE ? AND NOT EXISTS(SELECT 1 FROM sources s
                JOIN deployment_sources ds USING(path) JOIN deployments d USING(product_id,layer)
                WHERE s.sha256=o.source_sha AND ds.product_id=? AND d.failure IS NULL)""",
                            (row["id"], f"%/products/{pid}/%", pid)).fetchone()[0]
        bound = db.execute("""SELECT COUNT(*) FROM review_owners o WHERE o.section_id=?
            AND o.source_path LIKE ? AND o.status IN ('matched','verified_current_non_factory_scope') AND EXISTS(
                SELECT 1 FROM sources s JOIN deployment_sources ds USING(path) JOIN deployments d USING(product_id,layer)
                WHERE s.sha256=o.source_sha AND ds.product_id=? AND d.failure IS NULL)""",
                           (row["id"], f"%/products/{pid}/%", pid)).fetchone()[0]
        sections.append({"receipt": row["receipt"], "section": row["pointer"][1:],
                         "canonical_sha256": row["canonical_sha"], "state": row["state"],
                         "basis": "explicit_product_scope" if direct else "identical_code_scope" if count else "reusable_code_candidate",
                         "matched_owner_count": count,
                         "matched_js_owner_count": js_count, "matched_native_function_count": native_count,
                         "candidate_js_owner_count": js_candidates,
                         "current_non_factory_owner_count": non_factory,
                         "current_product_sources_bound": bool(bound) and not missing,
                         "scope": row["scope"]})
    return sections


def build(workers=4, *, db_path=None):
    import code_identity as ci
    import code_identity_reviews as reviews
    manifest = json.loads(MANIFEST.read_bytes())
    stage_notes = {(row["product_id"], row["capability"]): row for row in manifest.get("stage_evidence", [])}
    prior = json.loads(OUTPUT.read_bytes()) if OUTPUT.is_file() else {"products": []}
    previous_stages = {(row["product_id"], entry["capability"]): entry["stages"]
                       for row in prior["products"] for entry in row["capabilities"]}
    rows, totals = [], {}
    with closing(ci.connect(db_path or ci.DATABASE)) as db:
        metadata = dict(db.execute("SELECT key,value FROM metadata"))
        if metadata.get("review_source_signature") != reviews.source_signature(db):
            raise ValueError("Code identities changed or propagation is absent; run code-identity --phase propagate")
        if json.loads(metadata.get("review_navigation_signature", "null")) != reviews.navigation_signature(db):
            raise ValueError("Mounted navigation changed; rebuild identity propagation")
        if metadata.get("review_manifest_sha") != a.sha(MANIFEST):
            raise ValueError("Capability scope mapping changed; rebuild identity propagation")
        product_doc = a.ROOT / ".analysis/results/product-documentation.json"
        if metadata.get("review_product_doc_sha") != a.sha(product_doc):
            raise ValueError("Mounted product scope changed; rebuild identity propagation")
        for receipt in json.loads(metadata.get("review_receipts", "[]")):
            if a.sha(a.resolve(receipt["path"], ".analysis/results")) != receipt["sha256"]:
                raise ValueError("Formal review changed; rebuild identity propagation: " + receipt["path"])
        for receipt in json.loads(metadata.get("review_role_evidence", "[]")):
            if a.sha(a.resolve(receipt["path"], ".analysis/results")) != receipt["sha256"]:
                raise ValueError("Variant role evidence changed; rebuild identity propagation: " + receipt["path"])
        for family, spec in manifest["families"].items():
            for pid in spec["product_ids"]:
                deployments = {row["layer"]: dict(row) for row in db.execute("SELECT * FROM deployments WHERE product_id=?", (pid,))}
                registrations = [dict(row) for row in db.execute("""SELECT DISTINCT s.path,r.start_byte,r.end_byte,r.feature
                    FROM deployment_sources ds JOIN sources s USING(path) JOIN registrations r ON r.source_sha=s.sha256
                    WHERE ds.product_id=? AND ds.layer='mw' ORDER BY s.path,r.start_byte""", (pid,))]
                failures = [r["failure"] for r in deployments.values() if r["failure"]]
                if not registrations:
                    failures.append("no_current_registration_identity")
                entries = []
                for scope in db.execute("SELECT * FROM capability_scope WHERE product_id=? ORDER BY capability", (pid,)):
                    capability = scope["capability"]
                    sections = _sections(db, pid, capability)
                    stages = {"static": _static_state(sections)}
                    note = stage_notes.get((pid, capability), {})
                    previous = previous_stages.get((pid, capability), {})
                    for stage in STAGES[1:]:
                        stages[stage] = note.get(stage, previous.get(stage, "not_started"))
                    variants = db.execute("""SELECT COUNT(DISTINCT normalized_sha) FROM capability_candidates
                        WHERE product_id=? AND capability=?""", (pid, capability)).fetchone()[0]
                    entries.append({"capability": capability, "stages": stages, "source_state": scope["source_state"],
                                    "known_candidate_variants": variants, "complete_variant_scope_proven": False,
                                    "sections": sections, "stage_evidence": note})
                    for stage, value in stages.items():
                        totals.setdefault(family, {}).setdefault(stage, {}).setdefault(value, 0)
                        totals[family][stage][value] += 1
                rows.append({"product_id": pid, "family": family,
                             "mw_source": deployments.get("mw", {}).get("acquisition"),
                             "ui_source": deployments.get("ui", {}).get("acquisition"),
                             "failure": failures or None, "registration_owners": registrations,
                             "configuration_keys": sorted({r["feature"] for r in registrations if r["feature"] != "genericFeature"}),
                             "capabilities": entries})
        variant_totals = {r[0]: r[1] for r in db.execute("SELECT state,COUNT(*) FROM variant_queue WHERE state<>'verified_data_dependency' GROUP BY state")}
        data_dependencies = db.execute("SELECT COUNT(*) FROM variant_queue WHERE state='verified_data_dependency'").fetchone()[0]
        unknown_sources = [dict(r) for r in db.execute("SELECT source_state,COUNT(*) AS capabilities FROM capability_scope GROUP BY source_state")]
    result = {"schema_version": 2, "manifest": {"path": a.rel(MANIFEST), "sha256": hashlib.sha256(MANIFEST.read_bytes()).hexdigest()},
              "denominator": "first-tier retained product x capability inventory: current MW genericFeature featureKeys plus retained UI page kinds; unresolved current mount identities are recorded separately",
              "current_capability_inventory_complete": (not any(r["failure"] for r in rows) and
                  all(r["source_state"] in {"current_registration", "current_mount_source_identity", "current_navigation_complete"} for r in unknown_sources)),
              "stages": list(STAGES), "target_execution": False,
              "totals": totals, "products": rows,
              "code_identity": {"database": a.rel(ci.DATABASE), "source_signature": metadata["review_source_signature"],
                                "known_candidate_variant_states": variant_totals, "full_remaining_variant_count_known": False,
                                "verified_data_dependencies": data_dependencies,
                                "source_scope_states": unknown_sources},
              "failures": [{"product_id": r["product_id"], "failure": r["failure"]} for r in rows if r["failure"]]}
    a.save(OUTPUT, result)
    return {"products": len(rows), "failures": len(result["failures"]), "totals": totals,
            "known_candidate_variant_states": variant_totals}
