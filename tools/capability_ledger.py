"""First-tier product x capability ledger: the single progress denominator.

Capabilities come from the current official deployment of each product: every
middleware ``genericFeature`` ``featureKey`` (other ``useFeature`` names are kept
as injector configuration keys) parsed as complete AST call nodes from the newest verified
acquisition, plus the mounted UI page kinds recorded by the product structure
receipt. Stage status is read from published canonical sections mapped in
``tools/capability-ledger.json``; nothing is inferred from names.
"""
import hashlib
import json
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

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


def _registrations(audit, item):
    """Return ordered (feature, generic featureKeys) pairs from one original."""
    raw = audit.read(item)
    if b"useFeature" not in raw:
        return []
    tree = audit.parser.parse(raw)
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


def _product(audit_factory, family, layer_family, pid):
    import frontend
    audit = audit_factory()
    route = "https://apps.razer.com/synapse/products/%d/mw/" % pid
    candidates = sorted((a.ROOT / ".analysis/inputs/frontend").glob("%s-mw-%d-*/acquisition.json" % (layer_family, pid)))
    if not candidates:
        return {"product_id": pid, "family": family, "mw_source": None,
                "failure": "no_retained_mw_acquisition", "registrations": []}
    try:
        selected = frontend.current_source_selection(route, audit, candidates=candidates, verify_capture=False)
    except Exception as error:  # Selection failures are evidence, not skipped products.
        return {"product_id": pid, "family": family, "mw_source": None,
                "failure": "source_selection: " + str(error), "registrations": []}
    registrations = []
    for item in selected["originals"]:
        if item["path"].endswith(".js") and "/products/%d/" % pid in item["path"]:
            registrations.extend(_registrations(audit, item))
    return {"product_id": pid, "family": family, "mw_source": selected["acquisition"],
            "failure": None if registrations else "no_useFeature_registration_found",
            "registrations": registrations, "changed_during_query": audit.final_snapshot()}


def _section_status(documents, section_ref):
    receipt, name = section_ref["receipt"], section_ref["section"]
    document = documents.setdefault(receipt, json.loads((a.ROOT / receipt).read_bytes()))
    section = document.get(name)
    if not isinstance(section, dict):
        return {"receipt": receipt, "section": name, "state": "missing"}
    complete = section.get("source_chain_complete") is True
    clean = (section.get("target_execution") is False and section.get("failures") == []
             and section.get("changed_during_query") == [])
    return {"receipt": receipt, "section": name, "canonical_sha256": _digest(section),
            "state": "closed" if complete and clean else "partial" if clean else "invalid",
            "open_static_items": len(section.get("remaining_chains") or [])}


def build(workers=4):
    import host
    if workers < 2:
        raise ValueError("Capability ledger requires at least two workers")
    manifest = json.loads(MANIFEST.read_bytes())
    product_doc = json.loads((a.ROOT / ".analysis/results/product-documentation.json").read_bytes())
    pages = {row["product_id"]: row for row in product_doc["products"]}
    scope = []
    for family, spec in manifest["families"].items():
        for pid in spec["product_ids"]:
            scope.append((family, spec["source_family"], pid))
    with ThreadPoolExecutor(max_workers=workers) as pool:
        products = list(pool.map(lambda row: _product(host.HostLocalAudit, *row), scope))
    documents = {}
    mapped = {}
    for entry in manifest["capability_sections"]:
        status = [_section_status(documents, ref) for ref in entry["sections"]]
        for pid in entry["product_ids"]:
            for capability in entry["capabilities"]:
                mapped.setdefault((pid, capability), []).extend(status)
    stage_notes = {(row["product_id"], row["capability"]): row for row in manifest.get("stage_evidence", [])}
    rows, totals = [], {}
    for product in products:
        pid = product["product_id"]
        capabilities = []
        for reg in product["registrations"]:
            if reg["feature"] == "genericFeature":
                capabilities.extend("feature:" + key for key in reg["generic_keys"])
            # Other useFeature names are injector configuration for the registered
            # features and pages, not separately deliverable capabilities.
        page_row = pages.get(pid)
        capabilities.extend("page:" + kind for kind in (page_row or {}).get("current_page_kinds", []))
        unique = list(dict.fromkeys(capabilities))
        entries = []
        for capability in unique:
            sections = mapped.get((pid, capability), [])
            states = {row["state"] for row in sections}
            static = ("closed" if sections and states == {"closed"} else
                      "partial" if sections and "invalid" not in states and "missing" not in states else
                      "invalid_evidence" if sections else "not_started")
            note = stage_notes.get((pid, capability), {})
            stages = {"static": static}
            for stage in STAGES[1:]:
                stages[stage] = note.get(stage, "not_started")
            entries.append({"capability": capability, "stages": stages, "sections": sections})
            for stage, value in stages.items():
                totals.setdefault(product["family"], {}).setdefault(stage, {}).setdefault(value, 0)
                totals[product["family"]][stage][value] += 1
        rows.append({**{k: product[k] for k in ("product_id", "family", "mw_source", "failure")},
                     "page_source": {"receipt": ".analysis/results/product-documentation.json",
                                     "recorded_at_utc": product_doc.get("recorded_at_utc"),
                                     "present": page_row is not None},
                     "registration_owners": [r["owner"] | {"feature": r["feature"]} for r in product["registrations"]],
                     "configuration_keys": sorted({r["feature"] for r in product["registrations"] if r["feature"] != "genericFeature"}),
                     "changed_during_query": product.get("changed_during_query", []),
                     "capabilities": entries})
    result = {"schema_version": 1, "manifest": {"path": a.rel(MANIFEST), "sha256": hashlib.sha256(MANIFEST.read_bytes()).hexdigest()},
              "denominator": "first-tier products x (current MW genericFeature featureKeys + mounted UI page kinds); other useFeature names are configuration_keys",
              "stages": list(STAGES), "target_execution": False,
              "totals": totals, "products": rows,
              "failures": [{"product_id": r["product_id"], "failure": r["failure"]} for r in rows if r["failure"]]}
    a.save(OUTPUT, result)
    return {"products": len(rows), "failures": len(result["failures"]), "totals": totals}
