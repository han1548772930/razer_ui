"""Read current device HTML favicons; optionally fetch their exact static SVGs.

No reference JavaScript is evaluated. The host displays the first page favicon,
not a guessed device category. Relative icons stay scoped to their product.
"""
import argparse
import concurrent.futures
import hashlib
import json
from html.parser import HTMLParser
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urljoin
from urllib.request import Request, urlopen
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.icons = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "link" and "icon" in attrs.get("rel", "").split():
            self.icons.append(attrs["href"])


def digest(data):
    return hashlib.sha256(data).hexdigest()


def collect():
    entries = []
    for source in sorted((ROOT / ".ref/devices").glob("*/index.html")):
        if not source.parent.name.isdigit():
            continue
        data = source.read_bytes()
        links = Links()
        links.feed(data.decode("utf8"))
        if not links.icons:
            continue
        if len(links.icons) != 1:
            raise ValueError(f"Re-audit multiple favicons: {source}")
        pid = int(source.parent.name)
        href = links.icons[0]
        if href.startswith("/synapse/assets/imgs/favicon/"):
            original = ROOT / ".ref/applications/synapse/dashboard/shared-favicon" / href.split("/")[-1]
            output = "host-category-" + original.stem.lower() + ".svg"
        elif href == "./productCategoryIcon.svg":
            original = source.parent / "productCategoryIcon.svg"
            output = f"host-device-{pid}-favicon.svg"
        else:
            raise ValueError(f"Unreviewed favicon {pid}: {href}")
        entries.append(dict(product_id=pid, html=source.relative_to(ROOT).as_posix(),
                            html_sha256=digest(data), href=href,
                            source=original.relative_to(ROOT).as_posix(),
                            source_url=urljoin(f"https://apps.razer.com/synapse/products/{pid}/ui/", href),
                            output=f"assets/synapse/{output}"))
    return entries


def fetch(entry):
    target = ROOT / entry["source"]
    if target.exists():
        return
    request = Request(entry["source_url"], headers={"User-Agent": "SynapseStaticSourceAudit/1.0"})
    try:
        with urlopen(request, timeout=25) as response:
            body = response.read()
            status = response.status
            headers = dict(response.headers.items())
    except HTTPError as error:
        body, status, headers = error.read(), error.code, dict(error.headers.items())
    receipt = dict(source_url=entry["source_url"], status=status,
                   sha256=digest(body), size=len(body), headers=headers)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.with_suffix(target.suffix + ".http.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf8")
    if status != 200:
        target.with_suffix(target.suffix + ".response").write_bytes(body)
        return
    if ET.fromstring(body).tag.rsplit("}", 1)[-1] != "svg":
        raise ValueError(f"Expected SVG: {entry['source_url']}")
    target.write_bytes(body)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    entries = collect()
    if args.fetch:
        unique = {entry["source"]: entry for entry in entries}
        with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
            list(pool.map(fetch, unique.values()))
    mapping = {}
    for entry in entries:
        original = ROOT / entry["source"]
        if original.exists():
            entry["source_sha256"] = digest(original.read_bytes())
            mapping[str(entry["product_id"])] = entry["output"].removeprefix("assets/")
        else:
            receipt = original.with_suffix(original.suffix + ".http.json")
            entry["unavailable"] = json.loads(receipt.read_text(encoding="utf8")).get("status") if receipt.exists() else "not fetched"
            mapping[str(entry["product_id"])] = None
    products = [("src/shell/host_device_favicons.json", mapping),
                ("docs/re/host-device-favicons-current-evidence.json", dict(generator_sha256=digest(Path(__file__).read_bytes()),
                 method="Current HTML shortcut icon to current host TabUI.changeTabIcon; static SVG only", products=entries))]
    for filename, value in products:
        output = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
        target = ROOT / filename
        if args.check:
            if target.read_text(encoding="utf8") != output:
                raise ValueError(f"Stale {filename}")
        else:
            target.write_text(output, encoding="utf8")
    print(f"Audited {len(entries)} product favicons; {sum(v is None for v in mapping.values())} source URLs unavailable.")


if __name__ == "__main__":
    main()
