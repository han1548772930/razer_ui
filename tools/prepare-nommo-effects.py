"""Fetch only manifest-declared Nommo SVG resources and validate existing assets."""
import concurrent.futures
import hashlib
import json
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = {
    "chroma_sync_v3_static": "chroma-sync.svg",
    "logo_chromastudio": "hue-logo_chromastudio.svg",
    "icon_direction_cw": "direction-cw.svg",
    "icon_direction_cw_1": "direction-cw-active.svg",
    "icon_direction_ccw": "direction-ccw.svg",
    "icon_direction_ccw_1": "direction-ccw-active.svg",
    "icon_direction_outward_999": "direction-out.svg",
    "icon_direction_outward": "direction-out-active.svg",
    "icon_direction_inward_999": "direction-in.svg",
    "icon_direction_inward": "direction-in-active.svg",
    "stepper_up": "stepper-up.svg",
    "stepper_down": "stepper-down.svg",
}


def acquire(job):
    pid, relative, output = job
    target = ROOT / ".ref" / "devices" / str(pid) / relative
    url = f"https://apps.razer.com/synapse/products/{pid}/ui/{relative}"
    if target.exists():
        data = target.read_bytes()
    else:
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read()
        if b"<svg" not in data[:1000] or b"<html" in data[:1000].lower():
            raise ValueError(f"Not SVG: {url}")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    asset = ROOT / "assets" / "synapse" / output
    if data != asset.read_bytes():
        raise ValueError(f"Prepared asset differs from current product resource: {asset}")
    return {"product_id": pid, "source_url": url, "source": str(target.relative_to(ROOT)),
            "asset": str(asset.relative_to(ROOT)), "sha256": hashlib.sha256(data).hexdigest(),
            "bytes": len(data), "matches_prepared_asset": True}


def main():
    jobs = []
    for pid in (1303, 1304):
        manifest = json.loads((ROOT / f".ref/devices/{pid}/asset-manifest.json").read_text(encoding="utf8"))
        resources = set(manifest["files"].values())
        for stem, output in ASSETS.items():
            found = [p for p in resources if Path(p).name.startswith(stem + ".") and p.endswith(".svg")]
            if len(found) != 1:
                raise ValueError(f"Ambiguous resource {pid}:{stem}")
            jobs.append((pid, found[0].removeprefix("./"), output))
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        receipts = list(pool.map(acquire, jobs))
    (ROOT / "docs/re/nommo-effects-assets.json").write_text(json.dumps(receipts, indent=2) + "\n", encoding="utf8")
    print(f"Validated {len(receipts)} current Nommo SVG resources against prepared assets.")


if __name__ == "__main__":
    main()
