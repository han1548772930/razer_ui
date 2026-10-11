"""Acquire current mapping middleware entry scripts as inert official bytes."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from html.parser import HTMLParser
import argparse
import hashlib
import json
from pathlib import Path
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent.parent


class Entry(HTMLParser):
    def __init__(self):
        super().__init__()
        self.scripts = []

    def handle_starttag(self, tag, attrs):
        if tag == "script" and dict(attrs).get("src"):
            self.scripts.append(dict(attrs)["src"])


def product(pid, declared=None):
    base = f"https://apps.razer.com/synapse/products/{pid}/mw/"
    directory = ROOT / f"local-ui-reverse/source/official/apps.razer.com/synapse/products/{pid}/mw"
    receipts = []

    def fetch(relative):
        url = urllib.parse.urljoin(base, relative)
        assert url.startswith(base) and ".." not in relative.split("/")
        request = urllib.request.Request(url, headers={"User-Agent": "RazerStaticSourceAudit/1.0"})
        with urllib.request.urlopen(request, timeout=35) as response:
            raw = response.read(16 * 1024 * 1024 + 1)
            assert response.status == 200 and response.url == url
            assert len(raw) <= 16 * 1024 * 1024
        target = directory / url[len(base):]
        target.parent.mkdir(parents=True, exist_ok=True)
        digest = hashlib.sha256(raw).hexdigest()
        if target.exists() and target.read_bytes() != raw:
            raise ValueError(f"Existing pinned source differs: {target}")
        target.write_bytes(raw)
        receipt = {"path": target.relative_to(ROOT).as_posix(), "url": url,
                   "retrieved_utc": datetime.now(timezone.utc).isoformat(),
                   "sha256": digest, "bytes": len(raw), "http_status": 200}
        target.with_name(target.name + ".http.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        receipts.append(receipt)
        return raw

    shared = []
    if declared is None:
        parser = Entry()
        parser.feed(fetch("index.html").decode("utf-8"))
        assert parser.scripts
        for script in parser.scripts:
            if urllib.parse.urljoin(base, script).startswith(base):
                fetch(script)
            else:
                shared.append(urllib.parse.urljoin(base, script))
    else:
        import re
        source = ROOT / declared["source"]
        assert hashlib.sha256(source.read_bytes()).hexdigest() == declared["sha256"]
        for chunk in declared["chunks"]:
            assert re.fullmatch(r"[A-Za-z0-9_-]+\.[0-9a-f]{20}\.js", chunk["name"])
            fetch(chunk["name"])
    result = {"product_id": pid, "method": "Current official HTML and declared scripts fetched as data; never executed",
              "receipts": receipts, "shared_script_urls": shared}
    suffix = "chunks" if declared is not None else "entry"
    (ROOT / f"docs/re/mapping-submission-{pid}-current-{suffix}-acquisition.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return {"pid": pid, "scripts": len(receipts) - (declared is None), "bytes": sum(r["bytes"] for r in receipts)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--declared-chunks", action="store_true")
    args = parser.parse_args()
    declarations = json.loads((ROOT / "docs/re/mapping-submission-current-chunk-declarations.json").read_text(encoding="utf-8")) if args.declared_chunks else None
    with ThreadPoolExecutor(max_workers=2) as pool:
        jobs = [pool.submit(product, item["pid"], item) for item in declarations] if declarations else [pool.submit(product, pid) for pid in [190, 678, 679, 688]]
        for job in jobs:
            result = job.result()
            print(json.dumps(result), flush=True)
