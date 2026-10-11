"""Compare current per-product resource bytes without evaluating vendor code."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRODUCTS = (678, 679, 688)
ASSETS = (
    "chroma_sync_v3_static.svg", "controller-fill-green-icon.svg",
    "controller-fill-grey-icon.svg", "icon_expand.svg", "controller_icon_qe.svg",
    "controller_icon_wasd.svg", "icon_sidepanel_a.svg", "icon_sidepanel.svg",
    "xbox-trigger-left.svg", "xbox-trigger-right.svg",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def relative(path):
    return path.relative_to(ROOT).as_posix()


rows = []
for asset in ASSETS:
    output = ROOT / "assets/synapse" / asset
    prepared = output.read_bytes()
    sources = []
    for pid in PRODUCTS:
        base = ROOT / f"local-ui-reverse/source/official/apps.razer.com/synapse/products/{pid}/ui"
        manifest = base / "asset-manifest.json"
        raw_manifest = manifest.read_bytes()
        files = json.loads(raw_manifest)["files"]
        key = f"static/media/{asset}"
        source = base / files[key].removeprefix("./")
        original = source.read_bytes()
        assert original == prepared, (pid, asset, "resource bytes differ")
        references = []
        for main_key in ("main.js", "main.css"):
            context = base / files[main_key].removeprefix("./")
            data = context.read_bytes()
            text = data.decode("utf-8")
            needle = source.name
            start = text.find(needle)
            if start < 0:
                continue
            lo = max(0, start - 120)
            hi = min(len(text), start + len(needle) + 120)
            snippet = text[lo:hi]
            references.append({
                "file": relative(context), "file_sha256": digest(data),
                "utf8_byte_range": [len(text[:lo].encode()), len(text[:hi].encode())],
                "utf16_range": [len(text[:lo].encode("utf-16-le")) // 2,
                                len(text[:hi].encode("utf-16-le")) // 2],
                "slice_sha256": digest(snippet.encode()), "source": snippet,
            })
        assert references, (pid, asset, "no main JS/CSS reference")
        sources.append({
            "product_id": pid, "manifest": relative(manifest),
            "manifest_sha256": digest(raw_manifest), "manifest_key": key,
            "source": relative(source), "source_sha256": digest(original),
            "source_bytes": len(original), "reference_slices": references,
        })
    rows.append({"output": relative(output), "sha256": digest(prepared), "sources": sources})

receipt = {
    "date": "2026-10-11", "products": PRODUCTS,
    "method": "Per-product current manifest resolution, byte comparison and inert JS/CSS reference slices",
    "scope": "Resource identity only; surrounding reference slices do not prove mounted caller equivalence",
    "vendor_code_executed": False, "resources": rows,
}
path = ROOT / "docs/re/keyboard-analog-shared-assets-current-source.json"
path.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
manifest = ROOT / "assets/synapse/manifest.json"
data = json.loads(manifest.read_text(encoding="utf-8-sig"))
by_output = {row["output"]: row for row in rows}
for entry in data["entries"]:
    if entry["output"] in by_output:
        entry["current_product_sources"] = by_output[entry["output"]]["sources"]
        entry["shared_resource_evidence"] = relative(path)
manifest.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"products": PRODUCTS, "resources": len(rows), "receipt": relative(path)}))
