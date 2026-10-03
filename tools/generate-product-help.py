"""Merge independently extracted DeviceInfo links; do not infer URLs from IDs."""
import json
import hashlib
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
def read(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))

records = {}
def add(pid, info, source, evidence, exports=None):
    if not isinstance(info, dict):
        return
    assert hashlib.sha256((ROOT / source).read_bytes()).hexdigest() == evidence, source
    support = info.get("supportPage")
    guide = info.get("masterGuideURL")
    records[pid] = dict(product_id=pid,
                        support=support if isinstance(support, str) else None,
                        guide=guide if isinstance(guide, str) else None,
                        source=source, evidence=evidence, device_info=info,
                        guide_overrides=(exports or {}).get("MASTER_GUIDE_EXTERNAL_LINKS"))

for row in read("docs/re/mouse-product-source.json")["products"]:
    config = row["config"]
    add(row["product_id"], config["exports"]["DeviceInfo"], config["path"], config["sha256"], config["exports"])
for path in sorted((ROOT / "assets/synapse/keyboard-products").glob("*.json")):
    row = json.loads(path.read_text(encoding="utf-8"))
    if "config" not in row:
        continue
    source = row["source_config"]["path"]
    digest = next(f["sha256"] for f in row["source_files"] if f["path"] == source)
    add(row["product_id"], row["config"].get("DeviceInfo"), source, digest, row["config"])
for filename in ["docs/re/source-product-configs.json", "docs/re/audio-product-configs.json"]:
    if not (ROOT / filename).exists():
        continue
    for row in read(filename)["products"]:
        config = row.get("config")
        if config:
            add(row["product_id"], config["exports"].get("DeviceInfo"), config["path"], config["sha256"], config["exports"])

products = read("docs/re/unimplemented-products.json")["products"]
pending = {p["product_id"] for p in products}
help_evidence = read("docs/re/source-help-evidence.json")
assert hashlib.sha256((ROOT / "tools/source-help-ast.cjs").read_bytes()).hexdigest() == help_evidence["scanner_sha256"]
capabilities = {}
source_hashes = {}
for product in help_evidence["products"]:
    pid = product["product_id"]
    for receipt in product["source_files"]:
        path = receipt["path"]
        if path not in source_hashes:
            source_hashes[path] = hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
        assert source_hashes[path] == receipt["sha256"], path
    pages = []
    for page in product["pages"]:
        candidates = [c for c in page["components"] if "copyDeviceSerial=" in c["source"]]
        if not candidates:
            raise ValueError(f"Unresolved mounted Help class {pid}/{page['offset']}")
        component = min(candidates, key=lambda c:len(c["source"]))
        source = component["source"]
        if component["render"]:
            assert len(component["render"]) == 1
            render = component["render"][0]["source"]
        else:
            # Older current products have a Babel class method table.
            assert 'key:"render",value:function()' in source, pid
            render = source.split('key:"render",value:function()', 1)[1]
        assert 'this.props.masterGuide).concat(' in render and '\".pdf\"' in render, pid
        assert 'https://support.razer.com' in render and 'https://www.razer.com/product-registration' in render, pid
        props = {}
        for default in page["default_props"]:
            if "resetTitle" in default["values"]: props.update(default["values"])
        # Only literal values passed by mounted JSX ancestors activate an
        # optional panel. Shared Help method/translation presence is not proof.
        for ancestor in page["components"]:
            if ancestor is component: continue
            for jsx in ancestor["jsx"]:
                for key in ("hasSystemInfo", "hasSystemInfoImage", "hasTutorial", "hasTHXPartialAudio", "hasCamoStudio", "resetTitle", "firmwareResetProps"):
                    if key in jsx["props"]: props[key] = jsx["props"][key]
        info = records.get(pid, {}).get("device_info", {})
        pages.append(dict(offset=page["offset"], source=page["path"],
            firmware="this.props.currentFWVersion?" in render,
            view_more="this.state.viewMore?" in render,
            serial=True, registration=True, support=True,
            reset=not ("isNonSupportFactoryReset" in render and info.get("isNonSupportFactoryReset", False)),
            oled_reset="isSupportResetOLED" in render and info.get("isSupportResetOLED", False),
            reset_title=props.get("resetTitle", "FACTORY_RESET_PROFILES"),
            obm=info.get("isOBMDevice", False),
            firmware_reset=props.get("firmwareResetProps"),
            system_info=props.get("hasSystemInfo", False),
            tutorial=props.get("hasTutorial", False),
            thx_instructions=props.get("hasTHXPartialAudio", False),
            camo=props.get("hasCamoStudio", False),
            class_source=component["path"], class_offset=component["offset"]))
    capabilities[pid] = pages

assert pending == capabilities.keys(), f"Missing Help products: {pending-capabilities.keys()}"
assert pending <= records.keys(), f"Missing DeviceInfo: {pending-records.keys()}"
data = []
for pid in sorted(pending):
    row = records[pid]
    row.pop("device_info")
    row["pages"] = capabilities[pid]
    assert not row["guide_overrides"], f"Edition guide overrides need explicit support: {pid}"
    data.append(row)
(ROOT / "src/features/source_help_data.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"Prepared {len(data)} products, {sum(len(r['pages']) for r in data)} mounted Help pages.")
