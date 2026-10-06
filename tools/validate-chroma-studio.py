"""Validate current Studio receipts and extract its SVG grid as inert JSON.

No vendor code is imported or evaluated. JS offsets are UTF-16 code units.
"""
import argparse
import hashlib
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--check", action="store_true")
args = parser.parse_args()


def read_json(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def sha(data):
    return hashlib.sha256(data).hexdigest()


audit = read_json("docs/re/chroma-studio-source.json")
cache = {}


def current(path, digest):
    if path not in cache:
        assert path.startswith(".ref/applications/synapse/chroma-studio/"), path
        cache[path] = (ROOT / path).read_bytes()
    assert sha(cache[path]) == digest, path
    return cache[path]


current(audit["manifest"]["path"], audit["manifest"]["sha256"])
for receipt in audit["receipts"]:
    data = current(receipt["path"], receipt["sha256"])
    source = data.decode("utf-8")
    if receipt["symbol"] != "locale exports":
        # Locale receipts contain a composed export inventory, not one range.
        excerpt = source.encode("utf-16-le")[receipt["offset"] * 2:receipt["end"] * 2].decode("utf-16-le")
        assert excerpt == receipt["source"], (receipt["path"], receipt["symbol"])
for rule in audit["css"]:
    current(rule["path"], rule["sha256"])
for asset in read_json("assets/synapse/chroma-studio-assets.json"):
    current(asset["source"], asset["source_sha256"])
    assert sha((ROOT / asset["output"]).read_bytes()) == asset["output_sha256"], asset["output"]

canvas = next(r for r in audit["receipts"] if r["symbol"] == "vt")
source = current(canvas["path"], canvas["sha256"]).decode("utf-8")
start = source.index("ke=e=>")
end = source.index("Re=o().memo(ke)", start)
snippet = source[start:end]
rects = []
for item in re.finditer(r'\("rect",\{([^{}]+)\}\)', snippet):
    values = dict(re.findall(r'(x|y|width|height|className):"([^"]+)"', item.group(1)))
    assert set(values) == {"x", "y", "width", "height", "className"}, values
    rects.append({"x": float(values["x"]), "y": float(values["y"]),
                  "width": float(values["width"]), "height": float(values["height"]),
                  "major": values["className"] == "canvas-grid-2"})
assert len(rects) >= 144, len(rects)
grid = {"rects": rects}
evidence = {"path": canvas["path"], "sha256": canvas["sha256"], "symbol": "ke",
            "offset": len(source[:start].encode("utf-16-le")) // 2,
            "end": len(source[:end].encode("utf-16-le")) // 2, "source": snippet,
            "rectangles": len(rects), "method": "Static literal SVG rectangle extraction; no JavaScript evaluation"}
for path, value in [("src/features/chroma_studio_grid.json", grid),
                    ("docs/re/chroma-studio-grid-source.json", evidence)]:
    encoded = (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    target = ROOT / path
    if args.check:
        assert target.read_bytes() == encoded, path
    else:
        target.write_bytes(encoded)
print(f"Validated {len(audit['receipts'])} Studio receipts, assets and {len(rects)} SVG grid rectangles")
