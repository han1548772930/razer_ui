"""Validate bundled resources against the local reference, without third-party modules."""
import base64
import hashlib
import importlib
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

# Current 96689 category paths have a separate generated include so resource
# preparation can remain independent from the main manifest writer.
service_evidence = json.loads((ROOT / "docs/re/module-service-current-evidence.json").read_text(encoding="utf-8"))
service_include = (directory / "module-service-embedded.rs").read_text(encoding="utf-8")
service_keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', service_include)
assert len(service_keys) == len(service_evidence["entries"])
assert len({key for key, _ in service_keys}) == len(service_keys)
for entry in service_evidence["entries"]:
    assert entry["source"].startswith(".ref/applications/synapse/dashboard/")
    source = ROOT / entry["source"]
    target = ROOT / "assets" / entry["asset"]
    assert (entry["asset"], target.name) in service_keys
    assert hashlib.sha256(source.read_bytes()).hexdigest() == entry["source_sha256"]
    assert hashlib.sha256(target.read_bytes()).hexdigest() == entry["output_sha256"]
    assert ET.fromstring(target.read_bytes()).tag.endswith("svg")

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
                                     ".ref/host-4.0.821/", ".work/latest-source-check/host-4.0.821/")), value
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
            assert len(payload) == 10
            dimensions = (u24(payload[4:7]) + 1, u24(payload[7:10]) + 1)
        elif tag == b"VP8L":
            assert len(payload) >= 5 and payload[0] == 0x2f
            header = int.from_bytes(payload[1:5], "little")
            decoded = ((header & 0x3fff) + 1, ((header >> 14) & 0x3fff) + 1)
            assert dimensions is None or dimensions == decoded
            dimensions = decoded
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
    assert offset == len(data) and dimensions
    assert bool(delays) == (loop is not None)
    return dimensions, loop, delays


for entry in entries:
    for path_key, hash_key in (("source", "source_sha256"), ("output", "sha256")):
        path = ROOT / entry[path_key]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == entry[hash_key], path
    path = ROOT / entry["output"]
    expected.add(path.relative_to(directory).as_posix())
    if path.suffix == ".rgba":
        assert entry["format"] == "rgba8", path
        assert len(path.read_bytes()) == entry["width"] * entry["height"] * 4, path
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
assert expected.isdisjoint({filename for _, filename in service_keys}), "Duplicate service asset registration"
expected.update(filename for _, filename in service_keys)

# OLED media has its own generated table so its preparer never races the shared
# manifest. Validate the original AST receipt before trusting inline image data.
def validate_oled_family(family, receipt_path):
    oled_entries = json.loads((directory / f"audio-oled-{family}-assets.json").read_text(encoding="utf-8"))
    oled_include = (directory / f"audio-oled-{family}-embedded.rs").read_text(encoding="utf-8")
    resource_code = (ROOT / "crates/razer-assets/src/lib.rs").read_text(encoding="utf-8")
    assert f'include!("../../../assets/synapse/audio-oled-{family}-embedded.rs")' in resource_code
    assert resource_code.count(f'.chain(AUDIO_OLED_{family.upper()}_ASSETS)') == 2, family
    family_keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', oled_include)
    assert len(family_keys) == len(oled_entries)
    assert len({key for key, _ in family_keys}) == len(family_keys)
    assert {(entry["output"].removeprefix("assets/"), Path(entry["output"]).name)
            for entry in oled_entries} == set(family_keys)
    oled_receipt = json.loads((ROOT / receipt_path).read_text(encoding="utf-8"))
    oled_manifest_path = ROOT / ".ref/devices/1383/asset-manifest.json"
    assert oled_receipt["manifest"]["path"] == oled_manifest_path.relative_to(ROOT).as_posix()
    assert hashlib.sha256(oled_manifest_path.read_bytes()).hexdigest() == oled_receipt["manifest"]["sha256"]
    oled_declared = {
        name[name.index("static/"):]
        for name in json.loads(oled_manifest_path.read_text(encoding="utf-8"))["files"].values()
        if "static/" in name
    }
    oled_origins = {item["output"]: item for item in oled_receipt["assets"] + oled_receipt["icons"]}
    assert len(oled_origins) == len(oled_entries)
    oled_source_cache = {}
    def verify_receipt(receipt):
        assert receipt["path"].startswith(".ref/devices/1383/")
        assert receipt["path"].removeprefix(".ref/devices/1383/") in oled_declared
        if receipt["path"] not in oled_source_cache:
            raw = (ROOT / receipt["path"]).read_bytes()
            oled_source_cache[receipt["path"]] = (hashlib.sha256(raw).hexdigest(), raw.decode("utf-8").encode("utf-16-le"))
        source_hash, utf16 = oled_source_cache[receipt["path"]]
        assert source_hash == receipt["sha256"]
        assert utf16[receipt["offset"] * 2:receipt["end"] * 2].decode("utf-16-le") == receipt["source"]

    for entry in oled_entries:
        source = ROOT / entry["source"]
        target = ROOT / entry["output"]
        asset = entry["output"].removeprefix("assets/")
        assert entry["source"].startswith(".ref/devices/1383/")
        assert entry["source"].removeprefix(".ref/devices/1383/") in oled_declared
        assert (asset, target.name) in family_keys
        assert hashlib.sha256(source.read_bytes()).hexdigest() == entry["source_sha256"], source
        data = target.read_bytes()
        assert hashlib.sha256(data).hexdigest() == entry["output_sha256"], target
        origin = oled_origins[asset]
        receipt = origin["receipt"]
        verify_receipt(receipt)
        if "loader" in origin:
            verify_receipt(origin["loader"])
        if "src" in origin:
            if origin["src"].startswith("data:image/"):
                assert origin["src"] in receipt["source"]
                assert entry["source"] == receipt["path"]
                inline = base64.b64decode(origin["src"].split(",", 1)[1], validate=True)
                assert hashlib.sha256(inline).hexdigest() == entry["inline_sha256"]
            else:
                assert entry["source"] == ".ref/devices/1383/" + origin["src"]
        else:
            assert entry["source"] == receipt["path"]
            assert data == origin["svg"].encode("utf-8")
        if target.suffix == ".png":
            assert data[:8] == b"\x89PNG\r\n\x1a\n"
            assert struct.unpack(">II", data[16:24]) == (entry["width"], entry["height"])
        elif target.suffix == ".webp":
            dimensions, loop, delays = webp_metadata(data)
            assert dimensions == (entry["width"], entry["height"])
            if delays:
                assert loop == entry["loop"]
                if "frame_durations_ms" in entry:
                    assert delays == entry["frame_durations_ms"]
                assert sum(delays) == sum(entry["source_frame_durations_ms"])
            else:
                assert len(entry["source_frame_durations_ms"]) == 1
        else:
            assert target.suffix == ".svg" and ET.fromstring(data).tag == "{http://www.w3.org/2000/svg}svg"
    assert expected.isdisjoint({filename for _, filename in family_keys}), "Duplicate OLED asset registration"
    expected.update(filename for _, filename in family_keys)
    return family_keys


oled_keys = []
for family, receipt_path in [
    ("home", "docs/re/audio-oled-home-source.json"),
    ("artwork", "docs/re/audio-oled-artwork-current-evidence.json"),
    ("banner", "docs/re/audio-oled-banner-current-evidence.json"),
    ("system", "docs/re/audio-oled-system-source.json"),
]:
    oled_keys.extend(validate_oled_family(family, receipt_path))

# Independently embedded current roots must have the same provenance and key
# checks as the original shared table, rather than bypassing the literal scan.
root_keys = []
for family in ("tray-account", "settings-window", "chroma-studio", "chroma-studio-color"):
    payload = json.loads((directory / f"{family}-assets.json").read_text(encoding="utf-8"))
    rows = payload["entries"] if isinstance(payload, dict) else payload
    include = (directory / f"{family}-embedded.rs").read_text(encoding="utf-8")
    keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', include)
    assert len(keys) == len(rows) and len(set(keys)) == len(keys), family
    for row in rows:
        assert row["source"].startswith(".ref/applications/"), row["source"]
        source = (ROOT / row["source"]).read_bytes()
        output = ROOT / row["output"]
        data = output.read_bytes()
        assert hashlib.sha256(source).hexdigest() == row["source_sha256"]
        assert hashlib.sha256(data).hexdigest() == row.get("output_sha256", row.get("sha256"))
        assert ET.fromstring(data).tag.endswith("svg"), output
        assert (row["output"].removeprefix("assets/"), output.name) in keys
        if Path(row["source"]).suffix == ".svg":
            assert data == source, output
    assert expected.isdisjoint(filename for _, filename in keys), family
    expected.update(filename for _, filename in keys)
    root_keys.extend(keys)

studio_host = json.loads((ROOT / "docs/re/chroma-studio-window-source.json").read_text(encoding="utf-8"))
favicon = studio_host["favicon"]
assert favicon["source"] == ".ref/applications/synapse/chroma-studio/favicon.svg"
icon_bytes = (ROOT / favicon["source"]).read_bytes()
assert icon_bytes == (ROOT / favicon["output"]).read_bytes()
assert hashlib.sha256(icon_bytes).hexdigest() == favicon["sha256"]
assert ET.fromstring(icon_bytes).tag.endswith("svg")
assert 'href="./favicon.svg"' in (ROOT / ".ref/applications/synapse/chroma-studio/index.html").read_text(encoding="utf-8")
favicon_key = favicon["output"].removeprefix("assets/")
assert favicon_key in (ROOT / "crates/razer-assets/src/lib.rs").read_text(encoding="utf-8")
assert Path(favicon_key).name not in expected
expected.add(Path(favicon_key).name)
root_keys.append((favicon_key, Path(favicon_key).name))

# Product 4115 has its own current dynamic SVG context, not an application root.
kitsune = json.loads((ROOT / "docs/re/kitsune-current-evidence.json").read_text(encoding="utf-8"))
kitsune_include = (directory / "kitsune-embedded.rs").read_text(encoding="utf-8")
kitsune_keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', kitsune_include)
assert len(kitsune_keys) == len(kitsune["assets"]) == 2
assert len(set(kitsune_keys)) == 2
for row in kitsune["assets"]:
    assert row["source"].startswith(".ref/devices/4115/")
    source = (ROOT / row["source"]).read_bytes()
    output = ROOT / row["output"]
    assert hashlib.sha256(source).hexdigest() == row["sha256"]
    assert output.read_bytes() == source
    assert ET.fromstring(source).tag.endswith("svg")
    assert (row["output"].removeprefix("assets/"), output.name) in kitsune_keys
    assert output.name not in expected
    expected.add(output.name)
resource_source = (ROOT / "crates/razer-assets/src/lib.rs").read_text(encoding="utf-8")
assert resource_source.count(".chain(KITSUNE_ASSETS)") == 2

gamepad_dialog = json.loads((ROOT / "docs/re/gamepad-2636-dialog-current-evidence.json").read_text(encoding="utf-8"))
gamepad_keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', (directory / "gamepad-2636-embedded.rs").read_text(encoding="utf-8"))
assert len(gamepad_keys) == len(set(gamepad_keys)) == 2
for row in gamepad_dialog["assets"]:
    output = ROOT / row["output"]
    data = output.read_bytes()
    assert hashlib.sha256(data).hexdigest() == row["sha256"]
    assert ET.fromstring(data).tag.endswith("svg")
    if "module" not in row:
        assert data == (ROOT / row["source"]).read_bytes()
    if output.name.startswith("gamepad-2636-"):
        assert (row["output"].removeprefix("assets/"), output.name) in gamepad_keys
        assert output.name not in expected
        expected.add(output.name)
assert resource_source.count(".chain(GAMEPAD_DIALOG_ASSETS)") == 2

mixer = json.loads((ROOT / "docs/re/stream-mixer-current-evidence.json").read_text(encoding="utf-8"))
mixer_keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', (directory / "stream-mixer-embedded.rs").read_text(encoding="utf-8"))
assert len(mixer_keys) == len(set(mixer_keys)) == 2
assert [p["product_id"] for p in mixer["products"]] == [3334, 3337]
for product in mixer["products"]:
    manifest_path = product["manifest"]["path"]
    manifest_bytes = (ROOT / manifest_path).read_bytes()
    assert hashlib.sha256(manifest_bytes).hexdigest() == product["manifest"]["sha256"]
    declared = {str(Path(manifest_path).parent / value).replace("\\", "/")
                for value in json.loads(manifest_bytes)["files"].values()}
    for row in product["assets"]:
        assert row["source"].startswith(f'.ref/devices/{product["product_id"]}/')
        assert row["source"] in declared
        source = (ROOT / row["source"]).read_bytes()
        output = ROOT / row["output"]
        assert hashlib.sha256(source).hexdigest() == row["sha256"]
        assert output.read_bytes() == source
        assert ET.fromstring(source).tag.endswith("svg")
        assert (row["output"].removeprefix("assets/"), output.name) in mixer_keys
for key, filename in mixer_keys:
    assert filename not in expected
    expected.add(filename)
assert resource_source.count(".chain(STREAM_MIXER_ASSETS)") == 2

snap = json.loads((ROOT / "docs/re/snap-tap-current-evidence.json").read_text(encoding="utf-8"))
snap_keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', (directory / "snap-tap-embedded.rs").read_text(encoding="utf-8"))
for row in snap["assets"]:
    assert row["source"].startswith(".ref/devices/515/")
    source = (ROOT / row["source"]).read_bytes()
    output = ROOT / row["output"]
    assert hashlib.sha256(source).hexdigest() == row["sha256"]
    assert output.read_bytes() == source
    assert ET.fromstring(source).tag.endswith("svg")
    assert (row["output"].removeprefix("assets/"), output.name) in snap_keys
for key, filename in snap_keys:
    assert filename not in expected
    expected.add(filename)
assert resource_source.count(".chain(SNAP_TAP_ASSETS)") == 2

properties = json.loads((ROOT / "docs/re/keyboard-properties-current-evidence.json").read_text(encoding="utf-8"))
properties_keys = re.findall(r'\("([^"]+)", include_bytes!\("([^"]+)"\)', (directory / "keyboard-properties-embedded.rs").read_text(encoding="utf-8"))
for row in properties["assets"]:
    assert row["source"].startswith(".ref/devices/515/")
    source = (ROOT / row["source"]).read_bytes()
    output = ROOT / row["output"]
    assert hashlib.sha256(source).hexdigest() == row["sha256"]
    assert output.read_bytes() == source
    assert ET.fromstring(source).tag.endswith("svg")
    assert (row["output"].removeprefix("assets/"), output.name) in properties_keys
for key, filename in properties_keys:
    assert filename not in expected
    expected.add(filename)
assert resource_source.count(".chain(KEYBOARD_PROPERTIES_ASSETS)") == 2

runtime_evidence = json.loads((ROOT / "docs/re/audio-oled-runtime-current-evidence.json").read_text(encoding="utf-8"))
for icon in runtime_evidence["icons"]:
    receipt = icon["receipt"]
    assert receipt["path"].startswith(".ref/devices/1383/")
    source = (ROOT / receipt["path"]).read_bytes()
    assert hashlib.sha256(source).hexdigest() == receipt["sha256"]
    utf16 = source.decode("utf-8").encode("utf-16-le")
    assert utf16[receipt["offset"] * 2:receipt["end"] * 2].decode("utf-16-le") == receipt["source"]
    output = ROOT / "assets" / icon["output"]
    assert output.read_text(encoding="utf-8") == icon["svg"]
    assert ET.fromstring(icon["svg"]).tag.endswith("svg")
    assert icon["output"] in (ROOT / "crates/razer-assets/src/lib.rs").read_text(encoding="utf-8")
    assert output.name not in expected
    expected.add(output.name)

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

# Later resource families use independent receipts instead of the shared table.
# Verify their output hashes and current-source hashes before extending coverage.
supplemental_outputs = {}
def supplemental_receipts(value):
    if isinstance(value, list):
        for row in value:
            supplemental_receipts(row)
    elif isinstance(value, dict):
        name = value.get("output", value.get("path"))
        sha = value.get("output_sha256", value.get("sha256"))
        if isinstance(name, str) and name.startswith("assets/") and sha:
            assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == sha, name
            supplemental_outputs[name] = sha
        source = value.get("source", value.get("path"))
        source_sha = value.get("source_sha256")
        if source_sha is None and "output" not in value:
            source_sha = value.get("sha256")
        if isinstance(source, str) and source.startswith(".ref/") and source_sha:
            validate_provenance(source)
            assert hashlib.sha256((ROOT / source).read_bytes()).hexdigest() == source_sha, source
        for row in value.values():
            if isinstance(row, (dict, list)):
                supplemental_receipts(row)

for receipt in (
    "chroma-settings-current-evidence", "gamepad-2636-calibration-current-evidence",
    "profiles-transfer-current-evidence", "monitor-pages-review-current-evidence",
    "receiver-pairing-current-evidence", "mouse-polling-model-current-evidence",
):
    supplemental_receipts(json.loads((ROOT / f"docs/re/{receipt}.json").read_text("utf-8")))
supplemental_receipts(json.loads((directory / "tray-widget-assets.json").read_text("utf-8")))
supplemental_receipts(json.loads((directory / "audio-demo-manifest.json").read_text("utf-8")))
actuation = json.loads((ROOT / "docs/re/keyboard-actuation-sync-current-evidence.json").read_text("utf-8"))
for product in actuation["products"]:
    icon = product.get("icon", product.get("sync_icon"))
    if icon is None:
        continue
    validate_provenance(icon["path"])
    data = (ROOT / icon["path"]).read_bytes()
    assert hashlib.sha256(data).hexdigest() == icon["sha256"]
    output = "assets/synapse/keyboard-actuation-sync.svg"
    assert (ROOT / output).read_bytes() == data
    supplemental_outputs[output] = icon["sha256"]

asset_audit = importlib.import_module("audit-assets-current").audit()
assert not asset_audit["errors"], asset_audit["errors"]
for key, rows in asset_audit["registrations"].items():
    for row in rows:
        target = ROOT / row["file"]
        filename = target.relative_to(directory).as_posix()
        if filename in expected:
            continue
        assert row["file"] in supplemental_outputs, (key, "Missing supplemental output receipt")
        data = target.read_bytes()
        if target.suffix == ".svg":
            assert ET.fromstring(data).tag.endswith("svg"), target
        elif target.suffix == ".mov":
            assert data[4:8] == b"ftyp", target
        elif target.suffix == ".png":
            assert data[:8] == b"\x89PNG\r\n\x1a\n", target
            assert all(size > 0 for size in struct.unpack(">II", data[16:24])), target
        elif target.suffix == ".webp":
            webp_metadata(data)
        else:
            raise AssertionError((target, "Supplemental format needs explicit validation"))
        expected.add(filename)

# Literal image paths cover only direct consumers; dynamic mapping, direction,
# DPI and variant consumers are audited separately instead of called unused.
for source_path in (ROOT / "crates").rglob("*.rs"):
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
      f"{len(service_keys)} current service SVGs; "
      f"{len(oled_keys)} OLED media assets; "
      f"{len(root_keys)} independent-root SVGs / {len(runtime_evidence['icons'])} OLED runtime icons; "
      f"{len(kitsune_keys)} Kitsune SVGs; "
      f"{len(gamepad_keys)} gamepad dialog SVGs; "
      f"{len(mixer_keys)} Stream Mixer SVGs; "
      f"{len(snap_keys)} Snap Tap SVGs; {len(properties_keys)} Keyboard Properties SVGs; "
      f"{len(image_map['requests'])} Webpack requests, {len(resolved)} product variants; "
      f"{len(dashboard_requests)} Dashboard variants; "
      f"{len(layouts)} keyboard layouts / {sum(len(layout['keys']) for layout in layouts)} input shapes; "
      f"{len(supplemental_outputs)} supplemental output receipts / {asset_audit['summary']['runtime_keys']} total runtime keys")
