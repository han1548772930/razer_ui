"""Prepare local-preview names from current manifests; never execute vendor code."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
CHECK = "--check" in sys.argv


def read(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


catalog = read("docs/re/product-catalog.json")
names = {}
receipts = []
for row in catalog["products"]:
    if not row["ui_entry_found"]:
        continue
    pid = row["product_id"]
    translations = {}
    for part in ("ui", "mw"):
        path = f".ref/discovery/products/{pid}/{part}/manifest.json"
        sha256 = digest(path)
        assert sha256 == row["endpoints"][f"{part}/manifest.json"]["sha256"], path
        manifest = read(path)
        receipts.append({"path": path, "sha256": sha256})
        for key, locales in manifest.get("productTranslations", {}).items():
            if key.split("_")[0] != str(pid):
                continue
            target = translations.setdefault(key, {})
            for locale, values in locales.items():
                target.setdefault(locale.lower(), {}).update(values)
    editions = {}
    for edition in row["edition_ids"] or [0]:
        values = {}
        for locale, fields in translations.get(f"{pid}_{edition}", {}).items():
            product = fields.get("PRODUCT", "")
            dashboard = fields.get("DASHBOARD_NAME", "")
            label = {"product": product or dashboard}
            if dashboard and dashboard != label["product"]:
                label["dashboard"] = dashboard
            if fields.get("DASHBOARD_EDITION"):
                label["edition"] = fields["DASHBOARD_EDITION"]
            if label["product"]:
                values[locale] = label
        # The runtime falls back to English, so identical translations need
        # not be duplicated. Different source translations remain verbatim.
        editions[str(edition)] = {
            locale: value for locale, value in sorted(values.items())
            if locale == "en" or value != values.get("en")
        }
    names[str(pid)] = editions

resource = "crates/razer-model/src/demo/preview_names.json"
output = json.dumps(names, ensure_ascii=False, indent=2) + "\n"
if CHECK:
    assert (ROOT / resource).read_text(encoding="utf-8") == output, resource
else:
    (ROOT / resource).parent.mkdir(parents=True, exist_ok=True)
    (ROOT / resource).write_text(output, encoding="utf-8")

dashboard = [entry for entry in read("docs/re/dashboard-device-current-evidence.json")["contracts"]
             if entry["module"] == 22534 and entry["name"] in ("G", "V", "K")]
assert len(dashboard) == 3
for entry in dashboard:
    assert entry["path"].startswith(".ref/applications/synapse/dashboard/")
    assert digest(entry["path"]) == entry["sha256"]
    source = (ROOT / entry["path"]).read_text(encoding="utf-8").encode("utf-16-le")
    assert source[entry["offset"] * 2:entry["end"] * 2].decode("utf-16-le") == entry["source"]

report = {
    "method": "Static current manifest hashes and productTranslations. UI fields are merged before MW fields; duplicate English translations are omitted without changing fallback results.",
    "scope": "Names and editions for explicit local previews only; no connected-device or completed-UI claim.",
    "products": len(names),
    "variants": sum(len(editions) for editions in names.values()),
    "resource": {"path": resource, "sha256": digest(resource)},
    "dashboard_names": dashboard,
    "implementation": [{"path": path, "sha256": digest(path)} for path in (
        "crates/razer-model/src/demo.rs", "crates/razer-model/src/demo/preview_catalog.rs", "crates/razer-settings/src/settings_page.rs",
        "tools/prepare-preview-product-names.py",
    )],
    "sources": receipts,
}
target = ROOT / "docs/re/preview-product-names-current-evidence.json"
output = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
if CHECK:
    assert target.read_text(encoding="utf-8") == output, target
else:
    target.write_text(output, encoding="utf-8")
print(f"Current preview names: {report['products']} products, {report['variants']} variants")
