"""Register only byte-verified assets from the current floating-controller receipt."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
RECEIPT = "docs/re/keyboard-analog-floating-controller-current-source.json"
record = json.loads((ROOT / RECEIPT).read_text(encoding="utf-8"))
manifest_path = ROOT / "assets/synapse/manifest.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
outputs = {}
for product in record["records"]:
    for asset in product["resources"]:
        source = asset["path"]
        assert source.startswith("local-ui-reverse/source/official/apps.razer.com/")
        output = "assets/synapse/keyboard-floating-" + asset["name"]
        original = (ROOT / source).read_bytes()
        prepared = (ROOT / output).read_bytes()
        digest = hashlib.sha256(original).hexdigest()
        assert prepared == original and digest == asset["sha256"], source
        provenance = {
            "product_id": product["product_id"],
            "source": source,
            "source_sha256": digest,
        }
        if output not in outputs:
            outputs[output] = {
                "source": source,
                "output": output,
                "source_sha256": digest,
                "sha256": digest,
                "source_bytes": len(original),
                "preparation": "Exact current source bytes; shared only after independent per-product byte comparison.",
                "evidence": RECEIPT,
                "current_product_sources": [],
            }
        assert outputs[output]["sha256"] == digest, output
        outputs[output]["current_product_sources"].append(provenance)

# Validate every output before touching the manifest; repeated runs are idempotent.
entries = manifest["entries"]
for output, entry in outputs.items():
    matches = [i for i, existing in enumerate(entries) if existing["output"] == output]
    assert len(matches) <= 1, output
    if matches:
        entries[matches[0]] = entry
    else:
        entries.append(entry)
manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"byte_verified_assets_registered": len(outputs), "products": [p["product_id"] for p in record["records"]]}))
