"""Inventory statically reachable resource consumers; never starts the app.

This is a repository-specific audit, not a Rust call-graph analyzer. Literal
paths come from the production module tree; dynamic families below name their
audited consumers explicitly. Conditional branches count as reachable, not as
proof of a connected device, installed service, or a rendered frame.
"""
import collections
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets/synapse"
entries = json.loads((ASSETS / "manifest.json").read_text(encoding="utf-8"))["entries"]
modules = set()


def visit(path):
    path = path.resolve()
    if path in modules:
        return
    modules.add(path)
    source = path.read_text(encoding="utf-8")
    for match in re.finditer(
        r'(?P<attrs>(?:#\[[^\]]+\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?mod\s+(?P<name>\w+)\s*;', source
    ):
        attrs = match["attrs"]
        if "cfg(test)" in attrs:
            continue
        override = re.search(r'path\s*=\s*"([^"]+)"', attrs)
        if override:
            child = path.parent / override[1]
        else:
            base = path.parent if path.stem in ("main", "mod") else path.parent / path.stem
            child = base / (match["name"] + ".rs")
            if not child.exists():
                child = base / match["name"] / "mod.rs"
        assert child.exists(), child
        visit(child)


visit(ROOT / "crates/razer-app/src/lib.rs")
direct = collections.defaultdict(set)
for path in sorted(modules):
    # Current inline test modules are terminal; exclude their asserted paths.
    source = path.read_text(encoding="utf-8").split("#[cfg(test)]\nmod tests")[0]
    for asset in re.findall(r'"(synapse/[\w./-]+\.(?:png|svg))"', source):
        direct[asset].add(path.relative_to(ROOT).as_posix())

mapping_names = "brightness default keyboard mouse sensitivity macro interdevice profile hypershift launch multimedia windows text disable".split()
dynamic_controls = {f"synapse/mapping-{name}{state}.svg" for name in mapping_names for state in ("", "-active")}
dynamic_controls.update(f"synapse/mapping-lighting{state}.png" for state in ("", "-active"))
dynamic_controls.update(f"synapse/direction-{direction}{state}.svg"
                        for direction in ("left", "right", "cw", "ccw", "out", "in")
                        for state in ("", "-active"))
dynamic_controls.update(f"synapse/stage-{stage}.svg" for stage in range(1, 6))
dynamic_controls.update(f"synapse/macro/{kind}.svg" for kind in
                        ("delay", "keyboard", "mouse", "macro", "launch", "command", "text", "loop"))
products = json.loads((ASSETS / "product-image-map.json").read_text(encoding="utf-8"))["resolved"]
product_images = {item["asset"] for item in products if item["part"] in ("Product", "MouseBottom")}
dashboard = json.loads((ASSETS / "dashboard-image-map.json").read_text(encoding="utf-8"))["requests"]
dashboard_images = {item["asset"] for item in dashboard}
images = {"synapse/" + (ROOT / entry["output"]).relative_to(ASSETS).as_posix()
          for entry in entries if Path(entry["output"]).suffix in (".png", ".svg")}
used_images = set(direct) | dynamic_controls | product_images | dashboard_images
assert used_images <= images, sorted(used_images - images)
layouts = json.loads((ASSETS / "keyboard-653-customize-layouts.json").read_text(encoding="utf-8"))["layouts"]
runtime_json = {item["output"] for item in layouts} | {"assets/synapse/keyboard-653-layouts.json"}
fonts = {item["output"] for item in entries if item["output"].endswith(".ttf")}
other_media = {item["output"] for item in entries
               if Path(item["output"]).suffix not in (".png", ".svg", ".json", ".ttf")}
unused_images = images - used_images
unused_data = {item["output"] for item in entries if item["output"].endswith(".json")} - runtime_json
assert len(used_images) + len(unused_images) + len(runtime_json) + len(unused_data) + len(fonts) + len(other_media) == len(entries)

print(json.dumps({
    "manifest_entries": len(entries),
    "formats": dict(collections.Counter(Path(item["output"]).suffix for item in entries)),
    "production_modules": len(modules),
    "additional_binary_media_not_traced": sorted(other_media),
    "consumers": {
        "direct_images": len(direct),
        "dynamic_controls_excluding_direct": len(dynamic_controls - direct.keys()),
        "product_images_excluding_previous": len(product_images - direct.keys() - dynamic_controls),
        "dashboard_images_excluding_previous": len(dashboard_images - direct.keys() - dynamic_controls - product_images),
        "fonts": len(fonts),
        "runtime_json": len(runtime_json),
        "total": len(used_images) + len(fonts) + len(runtime_json),
    },
    "without_consumers": {
        "images": sorted(unused_images),
        "data": sorted(unused_data),
        "total": len(unused_images) + len(unused_data),
    },
    "direct_references": {asset: sorted(paths) for asset, paths in sorted(direct.items())},
}, indent=2))
