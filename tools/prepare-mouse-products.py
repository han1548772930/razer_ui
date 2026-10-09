"""Prepare native mouse data from current CONFIG and mounted-component receipts."""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def load(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))

configs = load("docs/re/mouse-product-source.json")
# Cobra must include both the CONFIG and button-group receipts from the
# maintained extractor. Falling back to CONFIG alone loses its mapping inputs.
cobra = next(p for p in configs['products'] if p['product_id'] == 162)
assert cobra['groups'] and cobra['group_source']
configs['products'].sort(key=lambda p: p['product_id'])
pages = {p["product_id"]: p["pages"] for p in load("docs/re/mouse-page-source.json")["products"]}
artwork = {p['product_id']: p for p in load('docs/re/mouse-product-assets.json')}
output = []
for product in configs["products"]:
    pid = product["product_id"]
    exports = product["config"]["exports"]
    info = exports["DeviceInfo"]
    group_file = ROOT / f'assets/synapse/mouse-products/configs/{pid}.json'
    groups = product.get('groups', [])
    if groups:
        receipt = product['group_source']
        group_source = (ROOT / receipt['path']).read_text(encoding='utf8')
        fragment = group_source.encode('utf-16-le')[receipt['offset'] * 2:receipt['end'] * 2].decode('utf-16-le')
        assert hashlib.sha256(fragment.encode()).hexdigest() == receipt['sha256']
    elif group_file.exists():
        group_receipt = json.loads(group_file.read_text(encoding='utf8'))
        for receipt in group_receipt['source_files']:
            assert hashlib.sha256((ROOT/receipt['path']).read_bytes()).hexdigest() == receipt['sha256']
        assert group_receipt['default_groups'], pid
        groups = [group['group'] for group in group_receipt['default_groups']['groups']]
    source = (ROOT / product["config"]["path"]).read_text(encoding="utf-8")
    assert hashlib.sha256(source.encode()).hexdigest() == product["config"]["sha256"]
    mounted = {p["key"]: "\n".join(c["source"] for c in p["components"]) for p in pages[pid]}
    profile = exports["DEFAULTPROFILE"]
    def normalize_literals(value):
        if isinstance(value, list):
            return [normalize_literals(item) for item in value]
        if isinstance(value, dict):
            if "expression" in value:
                expression = value["expression"]
                if re.fullmatch(r"[\w$]+\.[\w$]+\.SW_[A-Z_]+", expression):
                    symbol = expression.split(".")[-1]
                    assert re.search(re.escape(symbol) + r':"' + re.escape(symbol) + '"', source), (pid, expression)
                    return symbol
                if 'computerName' in expression and '"Default"' in expression:
                    return "Default"
                raise ValueError((pid, expression))
            return {key: normalize_literals(item) for key, item in value.items()}
        return value
    profile = normalize_literals(profile)
    step = info.get("dpiStep")
    if step is None:
        assert "dpiStep||50" in mounted["TAB_PERFORMANCE"], pid
        step = 50
    rates = {name: [int(v["content"]) for v in value] for name, value in exports.items()
             if (name.startswith("POLLING_RATE") or name.startswith("HYPER_POLLING")) and isinstance(value, list)}
    effects = []
    for item in exports.get("QUICK_EFFECTS", []):
        if isinstance(item['id'], int):
            effects.append({'key': item['name'], 'id': item['id']})
            continue
        enum = item["id"].get("expression", "").split(".")[-1] if isinstance(item["id"], dict) else None
        if enum:
            values = set(re.findall(r"\b" + re.escape(enum) + r":(\d+)", source))
            assert len(values) == 1, (pid, enum, values)
            effects.append({"key": enum, "id": int(next(iter(values)))})
    cal = mounted.get("TAB_CALIBRATION", "")
    calibration = "none"
    if "smartOnlyCalibrationReducer" in cal:
        calibration = "smart_only"
    elif "smartTracking" in cal:
        calibration = "smart_preset"
    elif "usingDefaultMat" in cal:
        calibration = "surface"
    smart = re.search(r"smartTracking:\{isAsymmetric:!(\d),trackingDistance:(\d+),liftOffDistance:(\d+),landingDistance:(\d+)\}", source)
    if smart and calibration != "none":
        profile["smartTracking"] = dict(isAsymmetric=smart[1] == "0", trackingDistance=int(smart[2]), liftOffDistance=int(smart[3]), landingDistance=int(smart[4]))
    advanced = mounted.get("ADVANCED", "")
    if "rotationReducer" in advanced:
        rotation = re.search(r"rotation:\{isEnabled:!(\d),value:(-?\d+)\}", source)
        assert rotation, pid
        profile.setdefault("rotation", dict(isEnabled=rotation[1] == "0", value=int(rotation[2])))
    dynamic = "dynamicSensitivityReducer" in advanced
    if dynamic:
        assert "dynamicSensitivity:{state:0,mode:0,customSensorAccelerations:[],points:[],templateId:0}" in source, pid
        profile.setdefault("dynamicSensitivity", dict(state=0, mode=0, customSensorAccelerations=[], points=[], templateId=0))
    power = mounted.get("TAB_POWER", "")
    performance = mounted.get("TAB_PERFORMANCE", "")
    if "lowPowerMode" in power and "min:5,max:100,step:5" in power and "lowPowerMode" not in profile:
        values = set(re.findall(r"lowPowerMode:(\d+)", source))
        if len(values) == 1:
            profile["lowPowerMode"] = int(next(iter(values)))
    if calibration == "surface":
        values = set(re.findall(r"liftOffRangeValue:(\d+)", source))
        assert len(values) == 1, (pid, values)
        profile["calibration"] = {"liftOffRangeValue": int(next(iter(values)))}
    output.append(dict(
        product_id=pid, name=product["name"], source=product["config"]["path"],
        source_sha256=product["config"]["sha256"], info=info, profile=profile,
        min_dpi=info["minDPI"], max_dpi=info["maxDPI"], dpi_step=step,
        support_xy=info.get("supportXYDPI", False), rates=rates,
        effects=effects, buttons=exports.get("BUTTON_LIST", []), dkm=exports.get("DKMKEYS", []),
        mats=exports.get("DEVICE_SUPPORTED_MATS", []), calibration=calibration,
        smart_lift_max=26 if "maxLiftOffDistance||26" in cal else 3,
        smart_landing_max=25 if "maxLandingDistance||25" in cal else 2,
        power_slider="powerSavingValue" in power and "min:1,max:15" in power,
        performance_power="powerSavingValue" in performance and "min:1,max:15" in performance,
        low_power_slider="lowPowerMode" in power and "min:5,max:100,step:5" in power,
        low_battery_slider="lowBatteryEffects" in power and "min:5,max:100" in power,
        rotation="rotationReducer" in advanced, dynamic=dynamic,
        image=artwork[pid]['output'].removeprefix('assets/') if artwork.get(pid,{}).get('status') == 'prepared' else None,
        groups=groups,
        pages=list(mounted), mounted_component_count=sum(len(p["components"]) for p in pages[pid]),
    ))
(ROOT / "crates/razer-pages/src/features/mouse_products_data.json").write_text(json.dumps(output, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
print(f"Prepared {len(output)} current mouse specifications.")
