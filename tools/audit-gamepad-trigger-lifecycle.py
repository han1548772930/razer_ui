"""Revalidate current trigger source receipts without executing JavaScript.

This consumes existing AST ranges, verifies the current source bytes and UTF-16
slices, and adds lexical receipts for the exact current bindings and CSS rules.
It does not run the app, tests, vendor JS, or DLLs.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
PRODUCTS = (2629, 2636, 2647, 2676, 2684, 4133, 4144)
OUTPUT = ROOT / "docs/re/gamepad-trigger-lifecycle-current-evidence.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def u16(text):
    return len(text.encode("utf-16-le")) // 2


def lexical_receipt(path, source, start, end, **extra):
    return {
        "path": path,
        "sha256": sha((ROOT / path).read_bytes()),
        "offset": u16(source[:start]),
        "end": u16(source[:end]),
        "source": source[start:end],
        **extra,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    base_path = ROOT / "docs/re/gamepad-product-evidence.json"
    existing = json.loads(base_path.read_text(encoding="utf-8"))
    native = json.loads((ROOT / "src/features/gamepad_products_data.json").read_text(encoding="utf-8"))
    native = {p["product_id"]: p for p in native if "TRIGGERS" in p["pages"]}
    assert set(native) == set(PRODUCTS), "Re-audit the affected TRIGGERS products"
    result = []
    for pid in PRODUCTS:
        product = next(p for p in existing["products"] if p["product_id"] == pid)
        texts = {}
        for file in product["source_files"]:
            assert file["path"].startswith(f".ref/devices/{pid}/")
            raw = (ROOT / file["path"]).read_bytes()
            assert sha(raw) == file["sha256"], f"Current source changed: {file['path']}"
            texts[file["path"]] = raw.decode("utf-8")
        page = next(p for p in product["pages"] if p["key"] == "TRIGGERS")
        components = page["components"]
        ranges = [c for c in components if 'className:"rangeSlider "' in c["source"]]
        panels = [c for c in components if "TriggerType" in c["source"] and "minValue:r.startRange" in c["source"]]
        if not panels:
            panels = [c for c in components if "TriggerType" in c["source"] and "minValue:" in c["source"] and "maxValue:" in c["source"]]
        roots = [c for c in components if c["source"].startswith("class ") and ".TRIGGER_TYPE.LEFT" in c["source"] and ".TRIGGER_TYPE.RIGHT" in c["source"]]
        assert len(ranges) == len(panels) == len(roots) == 1, f"Ambiguous mounted trigger chain: {pid}"
        receipts = []
        for kind, item in [("range", ranges[0]), ("panel", panels[0]), ("root", roots[0])]:
            text = texts[item["path"]]
            encoded = text.encode("utf-16-le")
            assert encoded[item["offset"] * 2:item["end"] * 2].decode("utf-16-le") == item["source"]
            prefix = encoded[max(0, item["offset"] - 100) * 2:item["offset"] * 2].decode("utf-16-le")
            binding = re.search(r"([\w$]+)=$", prefix)
            klass = re.match(r"class ([\w$]+) ", item["source"])
            symbol = binding.group(1) if binding else klass.group(1) if klass else None
            assert symbol, f"No current binding: {pid}/{kind}"
            receipts.append({"kind": kind, "symbol": symbol, "path": item["path"],
                             "sha256": sha((ROOT / item["path"]).read_bytes()),
                             "offset": item["offset"], "end": item["end"], "source": item["source"]})
        range_source = ranges[0]["source"]
        assert range_source.count('type:"range",min:"0",max:"100"') == 2
        assert range_source.count("onMouseUp:()=>{e.changeValue(") == 2
        assert range_source.count("useEffect") == 2
        assert "Math.min(Number(e.target.value)" in range_source
        assert "Math.max(Number(e.target.value)" in range_source
        assert receipts[0]["symbol"] in panels[0]["source"]

        main_path = product["source"]
        main = texts[main_path]
        export = re.search(r"getDefaultControllerActuation:\(\)=>([\w$]+)", main)
        assert export
        name = re.escape(export.group(1))
        helper = re.search(name + r"=\(\)=>\(\{isRapidTrigger:!1,actuationPoint:(\d+),startRange:(\d+),endRange:(\d+)\}\)", main[export.start():export.start() + 30000])
        assert helper, f"Missing reset helper: {pid}"
        helper_start = export.start() + helper.start()
        reset = {"isRapidTrigger": False, "actuationPoint": int(helper[1]), "startRange": int(helper[2]), "endRange": int(helper[3])}
        assert reset == native[pid]["trigger_reset"], f"Native reset differs: {pid}"
        reset_receipt = lexical_receipt(main_path, main, helper_start, export.start() + helper.end(), value=reset)
        mode_export = re.search(r"ACTUATION_MODE:\(\)=>([\w$]+)", main)
        assert mode_export
        mode = re.search(re.escape(mode_export[1]) + r"=\{ANALOG:0,DIGITAL:1\}", main[mode_export.start():mode_export.start() + 30000])
        assert mode and native[pid]["analog_mode"] == 0 and native[pid]["digital_mode"] == 1
        mode_receipt = lexical_receipt(main_path, main, mode_export.start() + mode.start(), mode_export.start() + mode.end())

        manifest_path = f".ref/devices/{pid}/asset-manifest.json"
        manifest_bytes = (ROOT / manifest_path).read_bytes()
        manifest = json.loads(manifest_bytes)
        css = []
        for relative in dict.fromkeys(manifest["files"].values()):
            if not relative.endswith(".css"):
                continue
            path = f".ref/devices/{pid}/" + relative.removeprefix("./")
            text = (ROOT / path).read_text(encoding="utf-8")
            for match in re.finditer(r"([^{}]+)\{[^{}]*\}", text):
                selector = match[1]
                if ".rangeSlider" in selector or ".sliderTipBar" in selector:
                    css.append(lexical_receipt(path, text, match.start(), match.end(), selector=selector))
        assert any("pointer-events:none" in r["source"] for r in css)
        assert any("pointer-events:auto" in r["source"] for r in css)
        result.append({"product_id": pid, "source_files": product["source_files"],
                       "manifest": {"path": manifest_path, "sha256": sha(manifest_bytes)},
                       "receipts": receipts, "reset": reset_receipt, "modes": mode_receipt,
                       "css_lexical_receipts": css})
    output = {
        "method": "Current source hashes and existing AST range revalidation; lexical helper/CSS receipts. UTF-16 offsets. No JavaScript execution.",
        "input_ast_receipts_sha256": sha(base_path.read_bytes()),
        "products": result,
    }
    serialized = json.dumps(output, ensure_ascii=False, indent=2) + "\n"
    if args.check:
        assert OUTPUT.read_text(encoding="utf-8") == serialized, "Stale trigger lifecycle receipts"
    else:
        OUTPUT.write_text(serialized, encoding="utf-8")
    print(f"Trigger lifecycle: {len(result)} products, {sum(len(p['receipts']) for p in result)} current AST slices, {sum(len(p['css_lexical_receipts']) for p in result)} lexical CSS receipts; reset/mode helpers verified.")


if __name__ == "__main__":
    main()
