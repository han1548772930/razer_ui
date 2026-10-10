"""Acquire current lighting analysis inputs; never load or execute vendor code."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
import pefile

ROOT = Path(__file__).resolve().parent.parent
METADATA = ROOT / "docs/re/application-resource-metadata-2026-10-09/background-resources.json"
NAMES = {"LightingDriverDLL", "LightingEngineDLL"}
OUTPUT = ROOT / "docs/re/lighting-native-current-acquisition.json"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--acquire", action="store_true")
    args = parser.parse_args()
    metadata = json.loads(METADATA.read_text(encoding="utf-8"))
    selected = [r for r in metadata["resources"] if r["resourceName"] in NAMES]
    assert {r["resourceName"] for r in selected} == NAMES
    receipts = []
    for resource in selected:
        digest = resource["sha256"].lower()
        filename = resource["url"].rsplit("/", 1)[-1]
        path = ROOT / ".ref/native-libraries" / digest / filename
        if args.acquire and not path.exists():
            request = urllib.request.Request(resource["url"], headers={"User-Agent": "RazerStaticSourceAudit/1.0"})
            with urllib.request.urlopen(request, timeout=60) as response:
                data = response.read()
            assert len(data) == resource["size"], filename
            assert hashlib.sha256(data).hexdigest() == digest, filename
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        data = path.read_bytes()
        assert len(data) == resource["size"], filename
        assert hashlib.sha256(data).hexdigest() == digest, filename
        receipts.append({"resource_name": resource["resourceName"], "version": resource["resourceVersion"],
                         "metadata": str(METADATA.relative_to(ROOT)).replace("\\", "/"),
                         "url": resource["url"], "sha256": digest, "bytes": len(data),
                         "input": str(path.relative_to(ROOT)).replace("\\", "/"),
                         "file": filename, "md5": hashlib.md5(data).hexdigest(),
                         "machine": pefile.PE(data=data).FILE_HEADER.Machine,
                         "exports": [{"name": export.name.decode("ascii") if export.name else None,
                                      "rva": export.address,
                                      "forwarder": export.forwarder.decode("ascii") if export.forwarder else None}
                                     for export in pefile.PE(data=data).DIRECTORY_ENTRY_EXPORT.symbols]})
    result = {"schema_version": 1, "method": "official current resource bytes, metadata SHA-256 and size verified; static input only; not executed", "resources": receipts}
    serialized = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.acquire:
        OUTPUT.write_text(serialized, encoding="utf-8")
    else:
        assert OUTPUT.read_text(encoding="utf-8") == serialized
    print(f"Verified {len(receipts)} current lighting inputs; no vendor execution")


if __name__ == "__main__":
    main()
