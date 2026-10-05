"""Parse the current, fingerprint-recovered SVG as XML; never execute JavaScript."""
import argparse
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from decimal import Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SVG = "{http://www.w3.org/2000/svg}"
XLINK = "{http://www.w3.org/1999/xlink}"
SOURCE = ".ref/applications/synapse/dashboard/static/media/gamer_room_hotspot_animation.53dd5566.svg"
RECOVERY = "docs/re/current-media-offline-recovery.json"
OUTPUT = "src/shell/gamer_room_hotspot_data.json"
EVIDENCE = "docs/re/gamer-room-hotspot-current-evidence.json"


def digest(value):
    return hashlib.sha256(value).hexdigest()


def prepare():
    recovery = json.loads((ROOT / RECOVERY).read_text(encoding="utf-8"))
    recovered = next(item for item in recovery["recovered"] if item["target"] == SOURCE)
    raw = (ROOT / SOURCE).read_bytes()
    assert digest(raw) == recovered["sha256"], "Recovered current SVG changed"
    manifest_path = ROOT / recovery["manifest"]["path"]
    assert digest(manifest_path.read_bytes()) == recovery["manifest"]["sha256"]
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["files"][recovered["key"]] == "./" + SOURCE.split("dashboard/", 1)[1]
    tree = ET.fromstring(raw)
    assert tree.tag == SVG + "svg"
    assert tree.get("preserveAspectRatio") == "xMidYMid meet"
    dimensions = [float(tree.get("width")), float(tree.get("height"))]
    view_box = [float(value) for value in tree.get("viewBox").split()]
    assert dimensions == [36, 36] and view_box == [0, 0, 36, 36]
    parents = {child: parent for parent in tree.iter() for child in parent}
    shapes = tree.findall(f".//{SVG}ellipse")
    assert len(shapes) == 2
    invisible, visible = shapes
    assert invisible.get("fill-opacity") == "0" and invisible.get("stroke") is None
    assert visible.get("fill") == "none" and visible.get("stroke") == "#ffffff"
    assert visible.get("stroke-opacity") == "1"
    assert visible.get("stroke-linecap") == "round" and visible.get("stroke-linejoin") == "round"
    center = [Decimal(visible.get("cx")), Decimal(visible.get("cy"))]
    transforms = []
    element = visible
    while element in parents:
        element = parents[element]
        transform = element.get("transform", "")
        if not transform:
            continue
        pieces = re.findall(r"translate\(\s*(-?[\d.]+)[ ,]+(-?[\d.]+)\s*\)", transform)
        assert re.sub(r"translate\([^)]*\)", "", transform).strip() == "" and pieces
        transforms.append({"id": element.get("id"), "transform": transform})
        for x, y in pieces:
            center[0] += Decimal(x)
            center[1] += Decimal(y)
    assert center == [18, 18]
    animations = tree.findall(f"{SVG}defs/{SVG}animate")
    tracks = {}
    for animation in animations:
        if animation.get(XLINK + "href") != "#" + visible.get("id"):
            continue
        assert animation.get("repeatCount") == "indefinite"
        assert animation.get("begin") == "0s" and animation.get("fill") == "freeze"
        assert animation.get("calcMode") == "spline"
        duration = animation.get("dur")
        assert re.fullmatch(r"\d+(?:\.\d+)?s", duration)
        times = [float(value) for value in animation.get("keyTimes").split(";")]
        values = [float(value) for value in animation.get("values").split(";")]
        splines = [[float(value) for value in segment.split()] for segment in animation.get("keySplines").split(";")]
        assert times == [0, 0.25, 0.625, 1]
        assert len(values) == len(times) and len(splines) == len(times) - 1
        assert all(len(segment) == 4 for segment in splines)
        tracks[animation.get("attributeName")] = {
            "duration_seconds": float(duration[:-1]), "key_times": times,
            "values": values, "key_splines": splines,
        }
    assert set(tracks) == {"rx", "ry", "opacity", "stroke-width"}
    assert tracks["rx"] == tracks["ry"]
    empty_group = tree.find(f"{SVG}g[@id='time_group']")
    assert empty_group is not None and len(empty_group) == 0
    payload = {
        "source": SOURCE, "source_sha256": digest(raw),
        "recovery_receipt": RECOVERY, "recovery_method": recovery["method"],
        "width": dimensions[0], "height": dimensions[1], "view_box": view_box,
        "center": [float(value) for value in center],
        "stroke_rgb": [255, 255, 255], "stroke_opacity": float(visible.get("stroke-opacity")),
        "radius": tracks["rx"], "stroke_width": tracks["stroke-width"], "opacity": tracks["opacity"],
    }
    evidence = {
        "verification": "Static XML parsing and source/resource hash validation only.",
        "source": {"path": SOURCE, "sha256": digest(raw), "xml": raw.decode("utf-8")},
        "recovery": {"path": RECOVERY, "method": recovery["method"], "manifest": recovery["manifest"], "recovered": recovered},
        "transforms": transforms,
        "animations": [{key.replace(XLINK, "xlink:"): value for key, value in animation.attrib.items()} for animation in animations],
        "inert_nodes": [
            {"id": invisible.get("id"), "reason": "fill-opacity stays 0 and there is no stroke; the animated fill ellipse is invisible"},
            {"id": "time_group", "reason": "The opacity animation targets an empty group, not the visible ellipse"},
        ],
        "data": payload,
    }
    return payload, evidence


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    payload, evidence = prepare()
    for file, value in [(OUTPUT, payload), (EVIDENCE, evidence)]:
        text = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
        destination = ROOT / file
        if args.check:
            assert destination.read_text(encoding="utf-8") == text, f"Stale hotspot receipt: {file}"
        else:
            destination.write_text(text, encoding="utf-8")
    print(f"{'Checked' if args.check else 'Extracted'} 8 SMIL nodes, 4 visible-ellipse tracks; intrinsic size 36 x 36")
