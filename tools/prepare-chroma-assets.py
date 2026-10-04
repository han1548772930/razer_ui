"""Fetch current Chroma manifest resources as data and prepare native assets."""
import argparse
import base64
import datetime
import hashlib
import io
import json
import pathlib
import re
import urllib.request
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASE = ROOT / ".ref/applications/chroma-app/dashboard"
ASSETS = ROOT / "assets/synapse"
parser = argparse.ArgumentParser()
parser.add_argument("--offline", action="store_true")
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
manifest = json.loads((BASE / "asset-manifest.json").read_text(encoding="utf-8"))["files"]
NAMES = ["icon_chromaapps_apps", "icon_chromaapps_games", "icon_chromaapps_profiles",
         "dashboard_spectrumcycling", "dashboard_static_green", "dashboard_breathing_green",
         "dashboard_wave", "dashboard_fire", "dashboard_starlight", "dashboard_wheel",
         "dashboard_chromaapps", "big_synapse_4", "split_arrow", "icon_device_brightness_100"]
items = [(name, manifest[f"static/media/{name}.svg"]) for name in NAMES]
items.append(("introduction_background", next(v for v in manifest.values()
    if re.search(r"/introduction_background\.[0-9a-f]+\.avif$", v))))
for name in ["services_store_placeholder", "services_support_placeholder", "services_community_placeholder", "service_gold_and_silver"]:
    items.append((name, next(v for v in manifest.values() if re.search(r"/" + name + r"\.[0-9a-f]+\.avif$", v))))

def sha(data):
    return hashlib.sha256(data).hexdigest()

def output(path, data):
    if args.check:
        if path.read_bytes() != data:
            raise ValueError(f"Stale Chroma resource: {path}")
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

records = []
for name, source_path in items:
    relative = source_path.removeprefix("/chroma-app/dashboard/")
    target = BASE / relative
    receipt_path = target.with_suffix(target.suffix + ".chroma.http.json")
    url = "https://apps.razer.com" + source_path
    if not target.exists():
        if args.offline or args.check:
            raise ValueError(f"Missing resource {target}")
        with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "Static-Source-Audit/1.0"}), timeout=45) as response:
            data = response.read(16 * 1024 * 1024 + 1)
            if len(data) > 16 * 1024 * 1024 or response.status != 200:
                raise ValueError(f"Invalid resource response {url}")
            receipt = {"url": url, "status": response.status,
                       "captured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                       "bytes": len(data), "sha256": sha(data)}
        output(target, data)
        output(receipt_path, (json.dumps(receipt, indent=2) + "\n").encode())
    data = target.read_bytes()
    # Only receipts captured by this helper establish its HTTP claim.
    receipt = json.loads(receipt_path.read_text()) if receipt_path.exists() else None
    if receipt and (receipt["url"] != url or receipt["status"] != 200
                    or receipt["sha256"] != sha(data) or receipt["bytes"] != len(data)):
        raise ValueError(f"Invalid resource receipt {target}")
    if target.suffix == ".svg":
        svg = ET.fromstring(data)
        if svg.tag.split("}")[-1] != "svg":
            raise ValueError(f"Not an SVG: {target}")
        if any(e.tag.split("}")[-1] in ("script", "foreignObject") for e in svg.iter()):
            raise ValueError(f"Active SVG element: {target}")
        prepared = data
        extension = "svg"
    else:
        from PIL import Image
        image = Image.open(io.BytesIO(data))
        if image.width * image.height > 32 * 1024 * 1024:
            raise ValueError("Chroma resource dimensions too large")
        out = io.BytesIO()
        image.convert("RGBA").save(out, format="PNG")
        prepared, extension = out.getvalue(), "png"
    filename = f"chroma-{name}.{extension}"
    output(ASSETS / filename, prepared)
    records.append({"source": str(target.relative_to(ROOT)).replace("\\", "/"),
                    "url": url, "source_sha256": sha(data), "http_receipt": receipt,
                    "asset": filename, "sha256": sha(prepared)})

# The current introduction logo is an embedded PNG in the mounted stylesheet.
css_path = BASE / "static/css/9700.ff0493ac.chunk.css"
css = css_path.read_text(encoding="utf-8")
match = re.search(r"\.big-chroma-studio-icon\{background-image:url\(data:image/png;base64,([A-Za-z0-9+/=]+)\)", css)
if not match:
    raise ValueError("Current Chroma introduction logo is missing")
data = base64.b64decode(match[1], validate=True)
filename = "chroma-introduction-logo.png"
output(ASSETS / filename, data)
records.append({"source": str(css_path.relative_to(ROOT)).replace("\\", "/"),
                "offset": match.start(), "source_sha256": sha(css.encode()),
                "asset": filename, "sha256": sha(data)})
embedded = ASSETS / "embedded.rs"
content = embedded.read_text(encoding="utf-8")
for record in records:
    name = record["asset"]
    if f'"synapse/{name}"' not in content:
        if args.check:
            raise ValueError(f"Missing embedded Chroma asset {name}")
        at = content.rfind("]")
        content = content[:at] + f'    ("synapse/{name}", include_bytes!("{name}") as &[u8]),\n' + content[at:]
if not args.check:
    embedded.write_text(content, encoding="utf-8", newline="\n")
output(ROOT / "docs/re/chroma-assets-current-evidence.json", (json.dumps(records, indent=2) + "\n").encode())
manifest_records = [{"source": r["source"], "source_sha256": r["source_sha256"],
    "url": r.get("url"), "output": "assets/synapse/" + r["asset"], "output_sha256": r["sha256"]}
    for r in records]
output(ASSETS / "chroma-app-manifest.json", (json.dumps(manifest_records, indent=2) + "\n").encode())
print(f"Chroma resources: {len(records)} current resources prepared and validated as data.")
