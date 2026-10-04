"""Audit current Kiyo pan/tilt media and mounted sharpness/gain flags statically."""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = {
    "icon_pan_top.svg": "camera-pan-top.svg",
    "icon_pan_bottom.svg": "camera-pan-bottom.svg",
    "icon_pan_center.svg": "camera-pan-center.svg",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    products = json.loads((ROOT / "docs/re/source-product-configs.json").read_text(
        encoding="utf-8"))["products"]
    rows = []
    for pid in (3592, 3594, 3595, 3596):
        directory = ROOT / ".ref/devices" / str(pid)
        manifest = json.loads((directory / "asset-manifest.json").read_text(encoding="utf-8"))
        media = []
        rules = []
        for css_path in sorted((directory / "static/css").glob("*.css")):
            text = css_path.read_text(encoding="utf-8")
            matches = [m for m in re.finditer(r"([^{}]+)\{([^{}]*)\}", text)
                       if "pan-and-tilt-container" in m[1]]
            if matches:
                rules.append({"path": css_path.relative_to(ROOT).as_posix(),
                              "sha256": digest(css_path),
                              "rules": [m[0] for m in matches]})
        css = "\n".join(rule for row in rules for rule in row["rules"])
        assert "background-size:10px 10px" in css, pid
        assert "background-size:20px 20px" in css, pid
        for key, output in ASSETS.items():
            relative = manifest["files"]["static/media/" + key].removeprefix("./")
            assert relative.startswith("static/media/") and ".." not in relative.split("/")
            source = directory / relative
            receipt = json.loads(source.with_name(source.name + ".http.json").read_text(
                encoding="utf-8"))
            url = f"https://apps.razer.com/synapse/products/{pid}/ui/{relative}"
            assert receipt["result"] == "ok" and receipt["sha256"] == digest(source), url
            prepared = ROOT / "assets/synapse" / output
            assert digest(prepared) == digest(source), prepared
            assert source.name in css, (pid, source.name)
            media.append({"manifest_key": "static/media/" + key,
                          "source_path": source.relative_to(ROOT).as_posix(),
                          "source_url": url, "sha256": digest(source),
                          "bytes": source.stat().st_size,
                          "prepared_path": prepared.relative_to(ROOT).as_posix()})
        product = next(p for p in products if p["product_id"] == pid)
        navigation = next(n for n in product["navigation"]
                          if any(i.get("name", {}).get("value") == "IMAGE" for i in n["items"]))
        mount = next(i for i in navigation["items"] if i.get("name", {}).get("value") == "IMAGE")
        assert digest(ROOT / navigation["source"]) == navigation["sha256"], pid
        component = mount["component"]
        assert "supportSharpness:!0" not in component and "supportGain:!0" not in component
        if pid != 3592:
            assert "supportSharpness:!1" in component and "supportGain:!1" in component
        else:
            assert component.endswith("{} )".replace(" ", "")), component
        rows.append({"product_id": pid, "assets": media, "css": rules,
                     "image_mount": {"source": navigation["source"],
                                     "sha256": navigation["sha256"], "component": component,
                                     "sharpness_gain_rendered": False}})
    native = (ROOT / "src/features/source_controls.rs").read_text(encoding="utf-8")
    for output in ASSETS.values():
        assert "synapse/" + output in native, output
    report = {"schema_version": 1,
              "method": "Current product manifest, HTTP hashes, CSS and audited mount data; no downloaded code execution",
              "products": rows}
    rendered = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    target = ROOT / "docs/re/camera-pan-assets-current-evidence.json"
    if options.check:
        assert target.read_text(encoding="utf-8") == rendered, "Stale camera pan asset evidence"
    else:
        target.write_text(rendered, encoding="utf-8")
    print("Kiyo pan assets: 12 verified source files, 3 byte-identical prepared SVGs, 4 disabled sharpness/gain mounts")


if __name__ == "__main__":
    main()
