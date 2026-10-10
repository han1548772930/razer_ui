"""Acquire official lighting-engine JS/CSS as inert static analysis input."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
BASE = "https://apps.razer.com/synapse/lighting-engine/"
DIRECTORY = ROOT / ".ref/applications/synapse/lighting-engine"
INDEX = ROOT / "docs/re/lighting-engine-current-source-acquisition.json"


def acquire(relative):
    assert relative and not relative.startswith("/") and ".." not in relative.split("/")
    path = DIRECTORY / relative
    request = urllib.request.Request(BASE + relative, headers={"User-Agent": "RazerStaticSourceAudit/1.0"})
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read()
        receipt = {"source_url": BASE + relative, "final_url": response.url, "http_status": response.status,
                   "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    assert receipt["http_status"] == 200 and receipt["final_url"] == receipt["source_url"]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    path.with_name(path.name + ".http.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return {"path": str(path.relative_to(ROOT)).replace("\\", "/"), **receipt}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--acquire", action="store_true")
    parser.add_argument("--verify-live", action="store_true", help="Independently compare entry, manifest and main bytes without replacing inputs")
    args = parser.parse_args()
    if args.acquire:
        receipts = [acquire(name) for name in ["index.html", "asset-manifest.json", "manifest.json"]]
        manifest = json.loads((DIRECTORY / "asset-manifest.json").read_text(encoding="utf-8"))
        resources = sorted({value.removeprefix("./") for value in manifest["files"].values() if value.endswith((".js", ".css"))})
        with ThreadPoolExecutor(max_workers=6) as pool:
            receipts.extend(pool.map(acquire, resources))
        result = {"schema_version": 1, "method": "Official current HTML, manifests and declared JS/CSS fetched as inert data; no downloaded code execution", "source_route": BASE, "receipts": receipts}
        INDEX.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    else:
        result = json.loads(INDEX.read_text(encoding="utf-8"))
        for receipt in result["receipts"]:
            data = (ROOT / receipt["path"]).read_bytes()
            assert len(data) == receipt["bytes"]
            assert hashlib.sha256(data).hexdigest() == receipt["sha256"]
            actual = json.loads((ROOT / (receipt["path"] + ".http.json")).read_text(encoding="utf-8"))
            assert actual == {key: value for key, value in receipt.items() if key != "path"}
        if args.verify_live:
            main = json.loads((DIRECTORY / "asset-manifest.json").read_text(encoding="utf-8"))["files"]["main.js"].removeprefix("./")
            for relative in ["index.html", "asset-manifest.json", "manifest.json", main]:
                with urllib.request.urlopen(BASE + relative, timeout=60) as response:
                    assert response.status == 200 and response.url == BASE + relative
                    assert response.read() == (DIRECTORY / relative).read_bytes(), relative
    print(f"Verified {len(result['receipts'])} current lighting-engine source inputs; no vendor execution")


if __name__ == "__main__":
    main()
