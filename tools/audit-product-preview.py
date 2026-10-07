"""Validate local preview IDs against current product manifests as static data."""
import hashlib
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]


def read(name):
    return (ROOT / name).read_text(encoding="utf-8")


def digest(name):
    return hashlib.sha256((ROOT / name).read_bytes()).hexdigest()


catalog = json.loads(read("docs/re/product-catalog.json"))
products = [row for row in catalog["products"] if row["ui_entry_found"]]
registry = {
    int(pid): json.loads(f"[{editions}]")
    for pid, editions in re.findall(
        r"RegisteredProduct \{ id: (\d+),[^\n]*?edition_ids: &\[([^]]*)\]",
        read("src/product/registry_data.rs"),
    )
}
assert set(registry) == {row["product_id"] for row in products}
receipts = []
for product in products:
    pid = product["product_id"]
    declared = set()
    manifests = []
    for part in ("ui", "mw"):
        path = f".ref/discovery/products/{pid}/{part}/manifest.json"
        sha256 = digest(path)
        assert sha256 == product["endpoints"][f"{part}/manifest.json"]["sha256"], path
        manifest = json.loads(read(path))
        translations = manifest.get("productTranslations", {})
        declared.update(
            int(key.split("_")[1]) for key in translations
            if re.fullmatch(fr"{pid}_\d+", key)
        )
        editions = manifest.get("editionId", [])
        declared.update([editions] if isinstance(editions, int) else editions)
        manifests.append({"path": path, "sha256": sha256})
    assert sorted(declared) == product["edition_ids"] == registry[pid], pid
    receipts.append({"product_id": pid, "edition_ids": sorted(declared), "manifests": manifests})

layout_path = "assets/synapse/keyboard-653-customize-layouts.json"
layouts = json.loads(read(layout_path))
rendered = json.loads(read("assets/synapse/keyboard-653-layouts.json"))
assert [row["layout_id"] for row in layouts["layouts"]] == [row["layout_id"] for row in rendered]
inventory = json.loads(read(".ref/discovery/interfaces.json"))
source = next(
    file for row in inventory["products"] if row["product_id"] == 653
    for file in row["files"] if file["path"] == layouts["source"]
)
assert digest(layouts["source"]) == source["sha256"]
report = {
    "scope": "Local product selection only; no complete UI or hardware capability claim.",
    "method": "Static JSON manifest hashes and edition IDs; source registry set; layout IDs and current source hash. Run generate-product-registry.cjs --check and extract-keyboard.cjs --check for the corresponding AST checks.",
    "registered_products": len(receipts),
    "catalog_editions": sum(len(row["edition_ids"]) for row in receipts),
    "default_only_products": [row["product_id"] for row in receipts if not row["edition_ids"]],
    "products": receipts,
    "keyboard_653": {
        "path": layouts["source"], "sha256": source["sha256"],
        "layout_enum": layouts["layout_enum"],
        "layout_resolver": layouts["layout_resolver"],
        "layouts": [{key: row[key] for key in ("layout_id", "layout_name", "module", "symbol", "source_range")} for row in layouts["layouts"]],
    },
    "limitations": [
        "The complete catalogue is a local preview entry, not an official Settings control.",
        "Layout variants are offered only for 653; other renderers retain their own default layout.",
        "Edition/layout identity is not evidence of connected hardware or implemented variant behavior.",
        "No application, build, tests, vendor JavaScript or DLLs were executed.",
    ],
}
target = ROOT / "docs/re/product-preview-current-evidence.json"
output = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
if "--check" in sys.argv:
    assert target.read_text(encoding="utf-8") == output, "Product preview source receipt drifted"
else:
    target.write_text(output, encoding="utf-8")
print(f"Preview source: {len(receipts)} products, {report['catalog_editions']} editions, {len(layouts['layouts'])} layouts for 653")
