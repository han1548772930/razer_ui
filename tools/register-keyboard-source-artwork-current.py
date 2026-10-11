"""Validate acquired source images and register the lossless display adapters."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
RECEIPT = "docs/re/keyboard-679-source-artwork-current.json"
images = json.loads((ROOT / "assets/synapse/keyboard679-current-manifest.json").read_text(encoding="utf-8"))
manifest_path = ROOT / "assets/synapse/manifest.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
prepared = []
for image in images:
    assert image["source"].startswith("local-ui-reverse/source/official/apps.razer.com/")
    assert image["output"].startswith("assets/synapse/keyboard679-current/")
    source = (ROOT / image["source"]).read_bytes()
    output = (ROOT / image["output"]).read_bytes()
    assert hashlib.sha256(source).hexdigest() == image["source_sha256"]
    assert hashlib.sha256(output).hexdigest() == image["output_sha256"]
    prepared.append({
        "source": image["source"], "output": image["output"],
        "source_sha256": image["source_sha256"], "sha256": image["output_sha256"],
        "source_bytes": len(source), "pixel_size": image["pixel_size"],
        "preparation": image["conversion"], "evidence": RECEIPT,
    })
entries = manifest["entries"]
for image in prepared:
    matches = [i for i, entry in enumerate(entries) if entry["output"] == image["output"]]
    assert len(matches) <= 1, image["output"]
    if matches:
        entries[matches[0]] = image
    else:
        entries.append(image)
manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"source_and_output_hashes_verified": len(prepared)}))
