"""Validate bundled resources against the local reference, without third-party modules."""
import hashlib
import json
import math
import re
import struct
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.parse import unquote, urljoin, urlparse
from keyboard_geometry import geometry_for

ROOT = Path(__file__).resolve().parent.parent
directory = ROOT / "assets/synapse"
entries = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))["entries"]
embedded = (directory / "embedded.rs").read_text(encoding="utf-8")
expected = set()

# Reject obsolete provenance before opening any referenced source. A passing hash
# from an old snapshot must never certify an asset as current.
def validate_provenance(value):
    if isinstance(value, dict):
        for nested in value.values():
            validate_provenance(nested)
    elif isinstance(value, list):
        for nested in value:
            validate_provenance(nested)
    elif isinstance(value, str):
        assert not value.startswith((".ref/frontend/", ".ref/synapse-asar/",
                                     ".ref/host-4.0.821/")), value
        prefix = ".ref/applications/synapse/dashboard/"
        if value.startswith(prefix + "static/"):
            assert value[len(prefix):] in dashboard_assets, value


dashboard_assets = set(json.loads((ROOT / ".ref/applications/synapse/dashboard/asset-manifest.json")
    .read_text(encoding="utf-8"))["files"].values())
dashboard_assets = {name.removeprefix("./") for name in dashboard_assets}
for manifest in directory.glob("*.json"):
    validate_provenance(json.loads(manifest.read_text(encoding="utf-8")))

for entry in json.loads((directory / "tutorial-media-manifest.json").read_text(encoding="utf-8"))["entries"]:
    if "resource_manifest" not in entry:
        continue
    manifest_path = entry["resource_manifest"]
    prefix = ".ref/applications/"
    assert manifest_path.startswith(prefix), manifest_path
    route = manifest_path[len(prefix):].removesuffix("asset-manifest.json")
    manifest = json.loads((ROOT / manifest_path).read_text(encoding="utf-8"))
    declared_urls = {unquote(urljoin("https://apps.razer.com/" + route, value))
                     for value in manifest["files"].values()}
    assert unquote(entry["source_url"]) in declared_urls, entry["source_url"]
    assert Path(unquote(urlparse(entry["source_url"]).path)).name == Path(entry["source"]).name


def webp_metadata(data):
    """Audit the actual native animation container, without a decoder dependency."""
    assert data[:4] == b"RIFF" and data[8:12] == b"WEBP"
    assert int.from_bytes(data[4:8], "little") + 8 == len(data)
    dimensions, loop, delays = None, None, []
    offset = 12
    u24 = lambda value: int.from_bytes(value, "little")
    while offset < len(data):
        tag = data[offset:offset + 4]
        length = int.from_bytes(data[offset + 4:offset + 8], "little")
        start, end = offset + 8, offset + 8 + length
        assert end <= len(data)
        payload = data[start:end]
        if tag == b"VP8X":
            assert len(payload) == 10 and payload[0] & 2
            dimensions = (u24(payload[4:7]) + 1, u24(payload[7:10]) + 1)
        elif tag == b"ANIM":
            assert len(payload) == 6
            loop = int.from_bytes(payload[4:6], "little")
        elif tag == b"ANMF":
            assert dimensions and len(payload) > 16
            x, y = u24(payload[:3]) * 2, u24(payload[3:6]) * 2
            width, height = u24(payload[6:9]) + 1, u24(payload[9:12]) + 1
            assert x + width <= dimensions[0] and y + height <= dimensions[1]
            delay = u24(payload[12:15])
            assert delay > 0
            delays.append(delay)
        offset = end + (length & 1)
    assert offset == len(data) and delays
    return dimensions, loop, delays


for entry in entries:
    for path_key, hash_key in (("source", "source_sha256"), ("output", "sha256")):
        path = ROOT / entry[path_key]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == entry[hash_key], path
    path = ROOT / entry["output"]
    expected.add(path.relative_to(directory).as_posix())
    if path.suffix == ".png":
        data = path.read_bytes()
        assert data[:8] == b"\x89PNG\r\n\x1a\n", path
        assert struct.unpack(">II", data[16:24]) == (entry["width"], entry["height"]), path
    if path.suffix == ".svg":
        svg = ET.parse(path).getroot()
        assert svg.tag == "{http://www.w3.org/2000/svg}svg", path
        if "fragment" in entry:
            source = ET.parse(ROOT / entry["source"]).getroot()
            view = source.find(f"{{http://www.w3.org/2000/svg}}view[@id='{entry['fragment']}']")
            assert svg.attrib["viewBox"] == view.attrib["viewBox"], path
    if path.suffix == ".webp":
        dimensions, loop, delays = webp_metadata(path.read_bytes())
        assert dimensions == (entry["width"], entry["height"]), path
        assert loop == entry["loop"] == 0, path
        assert len(delays) == entry["frames"], path
        assert delays == entry["frame_durations_ms"], path
assert set(re.findall(r'include_bytes!\("([^"]+)"\)', embedded)) == expected
assert len(expected) == len(entries), "Duplicate output keys"
image_map = json.loads((directory / "product-image-map.json").read_text(encoding="utf-8"))
sources = {entry["output"]: entry for entry in entries}
source_text = {}
for request in image_map["requests"]:
    for file_key, hash_key in (("context", "context_sha256"), ("module_file", "module_sha256")):
        path = ROOT / request[file_key]
        if path not in source_text:
            data = path.read_bytes()
            source_text[path] = (hashlib.sha256(data).hexdigest(), data.decode("utf-8"))
        assert source_text[path][0] == request[hash_key], path
    context = source_text[ROOT / request["context"]][1]
    module = str(request["module"])
    assert re.search(re.escape(json.dumps(request["request"])) + r':\[' + module + r',', context), request
    module_source = source_text[ROOT / request["module_file"]][1]
    media = "static/media/" + Path(request["source"]).name
    assert re.search(r'(?<![\w])' + module + r':\([^)]*\)=>\{(?:"use strict";)?'
                     + r'\w+\.exports=\w+\.p\+' + re.escape(json.dumps(media)) + r';?\}', module_source), request
    assert sources[request["output"]]["source"] == request["source"], request
resolved = image_map["resolved"]
assert image_map["requests"] and resolved, "Empty product image map"
identities = {(item["product_id"], item["edition_id"], item["layout_id"], item["part"]) for item in resolved}
assert len(identities) == len(resolved), "Duplicate product variants"
assert all(item["asset"].removeprefix("synapse/") in expected for item in resolved)
rust_rows = {
    (int(pid), int(edition), int(layout), part, asset)
    for pid, edition, layout, part, asset in re.findall(
        r'\((\d+), (\d+), (\d+), DeviceImage::(\w+), "([^"]+)"\)',
        (directory / "product-images.rs").read_text(encoding="utf-8"))
}
assert rust_rows == {
    (item["product_id"], item["edition_id"], item["layout_id"], item["part"], item["asset"])
    for item in resolved
}, "Rust product image index differs from source map"

# PluginImages are a separate family: a valid Customize request does not prove
# that a Dashboard/pairing image is available. Audit each downloaded identity,
# and prove that content-deduplicated outputs still match its exact AVIF bytes.
dashboard_map = json.loads((directory / "dashboard-image-map.json").read_text(encoding="utf-8"))
dashboard_requests = dashboard_map["requests"]
dashboard_context = (ROOT / dashboard_map["url_rule_source"]).read_text(encoding="utf-8")
assert "_dashboard3x.${s}" in dashboard_context and "/PluginImages`}" in dashboard_context
dashboard_rows = {
    (int(pid), int(edition), int(layout), asset)
    for pid, edition, layout, asset in re.findall(
        r'\((\d+), (\d+), (\d+), "([^"]+)"\)',
        (directory / "dashboard-images.rs").read_text(encoding="utf-8"))
}
assert dashboard_rows == {
    (item["product_id"], item["edition_id"], item["layout_id"], item["asset"])
    for item in dashboard_requests
}, "Rust Dashboard image index differs from downloaded source map"
assert len(dashboard_rows) == len(dashboard_requests), "Duplicate Dashboard identity"
for request in dashboard_requests:
    pid, edition, layout = (request[key] for key in ("product_id", "edition_id", "layout_id"))
    assert (pid, edition, layout, "Product") in identities, request
    assert request["source_url"] == (
        f"https://apps.razer.com/synapse/products/{pid}/ui/{pid}_{edition}/PluginImages/"
        f"{pid}_{edition}_{layout}_dashboard3x.avif"), request
    assert hashlib.sha256((ROOT / request["source"]).read_bytes()).hexdigest() == request["source_sha256"]
    assert sources[request["output"]]["source_sha256"] == request["source_sha256"], request
    assert request["asset"] == "synapse/" + Path(request["output"]).name
    assert Path(request["output"]).name in expected

# Without SMIL the source hotspot's ellipses have zero radii. Its bundled
# fallback must remain a visible snapshot of the original 0.25 keyframe.
hotspot = ET.parse(directory / "gr-hotspot.svg").getroot()
assert not hotspot.findall(".//{http://www.w3.org/2000/svg}animate")
ellipses = hotspot.findall(".//{http://www.w3.org/2000/svg}ellipse")
assert len(ellipses) == 2
assert all(item.get("rx") == "10" and item.get("ry") == "10" and item.get("opacity") == "0.2"
           for item in ellipses)
assert ellipses[1].get("stroke-width") == "4"

# Literal image paths cover only direct consumers; dynamic mapping, direction,
# DPI and variant consumers are audited separately instead of called unused.
for source_path in (ROOT / "src").rglob("*.rs"):
    for asset in re.findall(r'"(synapse/[\w.-]+\.(?:png|svg|webp))"', source_path.read_text(encoding="utf-8")):
        assert asset.removeprefix("synapse/") in expected, (source_path, asset)
# The original Customize caller supplies layout one when old device data has no
# layout (`layoutId || 1`). The app also uses these same embedded images for an
# explicitly marked preview if a nonzero layout is unknown; no synthetic layout
# requests should be added to the audited Webpack index for that UI behavior.
keyboard_main = next((ROOT / ".ref/devices/653/static/js").glob("main.*.js"))
assert "layoutId:this.props.layoutId||1" in source_text[keyboard_main][1]
keyboard_products = [item for item in resolved if item["product_id"] == 653 and item["part"] == "Product"]
keyboard_editions = {item["edition_id"] for item in keyboard_products}
assert keyboard_editions == {0, 128, 129, 130}
assert all((653, edition, 1, "Product") in identities for edition in keyboard_editions)
assert not any(item["layout_id"] in (0, 999) for item in keyboard_products), "Synthetic source layout"
keys = json.loads((directory / "keyboard-653-keys.json").read_text(encoding="utf-8"))
assert len({key["id"] for key in keys}) == len(keys)
assert all(key["bounds"][2] > 0 and key["bounds"][3] > 0 for key in keys)

# The source table and native shapes must follow the same original resolver.
# `node tools/extract-keyboard.cjs --check` additionally repeats AST extraction
# and compares every source literal without changing the generated files.
layout_sources = json.loads((directory / "keyboard-653-customize-layouts.json").read_text(encoding="utf-8"))
layouts = json.loads((directory / "keyboard-653-layouts.json").read_text(encoding="utf-8"))
expected_layouts = {1,2,3,4,5,6,7,8,9,10,11,12,15,16,17,18}
assert {layout["layout_id"] for layout in layouts} == expected_layouts
assert len(layouts) == len(expected_layouts)
assert {item["layout_id"] for item in keyboard_products} == expected_layouts, \
    "Product layouts and Customize input layouts differ"
assert {layout["layout_id"] for layout in layout_sources["layouts"]} == expected_layouts
assert layout_sources["view_box"] == [0,0,730,340]
assert layouts[0]["layout_id"] == 1 and layouts[0]["keys"] == keys
source_rows = {
    (int(layout), filename)
    for layout, filename in re.findall(r'\((\d+), include_str!\("([^"]+)"\)\)',
                                      (directory / "keyboard-sources.rs").read_text(encoding="utf-8"))
}
assert source_rows == {(layout["layout_id"], Path(layout["output"]).name)
                       for layout in layout_sources["layouts"]}

def geometry_bounds(geometry):
    if geometry["kind"] == "circle":
        x,y = geometry["center"]
        r = geometry["radius"]
        return [x-r,y-r,2*r,2*r]
    if geometry["kind"] == "rect":
        return geometry["origin"] + geometry["size"]
    points = []
    current = start = [0,0]
    for command in geometry["commands"]:
        op = command["op"]
        if op in ("move", "line"):
            current = command["points"]
            if op == "move": start = current
            points.append(current)
        elif op == "close":
            current = start
        elif op == "curve":
            p0,p1,p2,p3 = current,*command["points"]
            extrema = {0,1}
            for axis in (0,1):
                a = -p0[axis]+3*p1[axis]-3*p2[axis]+p3[axis]
                b = 2*(p0[axis]-2*p1[axis]+p2[axis])
                c = p1[axis]-p0[axis]
                if abs(a) < 1e-12:
                    if abs(b) >= 1e-12: extrema.add(-c/b)
                elif (discriminant := b*b-4*a*c) >= 0:
                    extrema.update(((-b-math.sqrt(discriminant))/(2*a), (-b+math.sqrt(discriminant))/(2*a)))
            for t in extrema:
                if 0 <= t <= 1:
                    u=1-t
                    points.append([u**3*p0[axis]+3*u*u*t*p1[axis]+3*u*t*t*p2[axis]+t**3*p3[axis]
                                   for axis in (0,1)])
            current=p3
        else: raise AssertionError(f"Unsupported path command {op}")
    x,y = [min(point[axis] for point in points) for axis in (0,1)]
    right,bottom = [max(point[axis] for point in points) for axis in (0,1)]
    return [x,y,right-x,bottom-y]

for layout in layout_sources["layouts"]:
    raw_path = ROOT / layout["output"]
    groups = json.loads(raw_path.read_text(encoding="utf-8"))
    inputs = [key for group in groups for key in group["group"]["buttonList"]]
    assert len({key["inputID"] for key in inputs}) == len(inputs), \
        f"Duplicate source input identity in layout {layout['layout_id']}"
    source_keys = [key for key in inputs if any(field in key for field in ("d", "r", "x"))]
    target = next(item["keys"] for item in layouts if item["layout_id"] == layout["layout_id"])
    assert len(source_keys) == len(target)
    assert len({key["id"] for key in target}) == len(target)
    assert len(inputs) == len(target)+4, "Invisible dial sub-inputs must remain in source data"
    assert raw_path.name in expected
    assert sources[layout["output"]]["module"] == layout["module"]
    for key,source in zip(target,source_keys):
        assert key["id"] == source["inputID"]
        assert key["label"] == str(source["counter"])
        assert key["enabled"] == source.get("isEnabled",True)
        assert key["functions"] == source.get("functionList",[])
        assert key["geometry"] == geometry_for(source), (layout["layout_id"],key["id"])
        if key["geometry"]["kind"] == "path":
            commands = key["geometry"]["commands"]
            assert commands[0]["op"] == "move" and commands[-1]["op"] == "close", \
                (layout["layout_id"], key["id"], "Audit changed SVG subpath closure")
        bounds=geometry_bounds(key["geometry"])
        assert all(math.isfinite(value) for value in key["bounds"])
        assert all(abs(a-b)<1e-5 for a,b in zip(bounds,key["bounds"])), (key["id"],bounds,key["bounds"])
        assert bounds[2]>0 and bounds[3]>0
print(f"Validated {len(entries)} source/output hashes, image formats and embedded keys; "
      f"{len(image_map['requests'])} Webpack requests, {len(resolved)} product variants; "
      f"{len(dashboard_requests)} Dashboard variants; "
      f"{len(layouts)} keyboard layouts / {sum(len(layout['keys']) for layout in layouts)} input shapes")
