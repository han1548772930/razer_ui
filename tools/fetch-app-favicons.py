"""Fetch only icons named by current local app HTML and asset manifests.

No JavaScript execution or image decoder is used. Existing source bytes are
never overwritten; --check validates the receipt without accessing the network.
"""
import argparse
import hashlib
import json
from datetime import datetime, timezone
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urljoin, urlparse
from urllib.request import Request, urlopen
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
ROUTES = ["synapse/dashboard", "synapse/alexa", "synapse/armory", "synapse/profiles", "feedback",
          "chroma-app/dashboard", "synapse/introduction-tour"]
RECEIPT = ROOT / "docs/re/app-favicons-current-fetch.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


class Icons(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.icons = []
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "link" and "icon" in attrs.get("rel", "").split():
            href = attrs.get("href")
            if href and href not in self.icons:
                self.icons.append(href)


def declared(route):
    directory = ROOT / ".ref/applications" / route
    html = (directory / "index.html").read_bytes()
    manifest = (directory / "asset-manifest.json").read_bytes()
    urls = {urljoin(f"https://apps.razer.com/{route}/", value)
            for value in json.loads(manifest)["files"].values()}
    links = Icons(html.decode("utf8")).icons
    if len(links) != 1:
        raise ValueError(f"Ambiguous favicon for {route}: {links}")
    url = urljoin(f"https://apps.razer.com/{route}/", links[0])
    parsed = urlparse(url)
    if url not in urls or parsed.netloc != "apps.razer.com":
        raise ValueError(f"Icon not declared in current manifest: {url}")
    expected = f"/{route}/"
    if not parsed.path.startswith(expected):
        raise ValueError(f"Icon outside its current app: {url}")
    target = directory / parsed.path[len(expected):]
    if target.suffix not in (".svg", ".ico") or target.resolve().parent != directory.resolve():
        raise ValueError(f"Unexpected icon target: {target}")
    return dict(route=route, url=url, path=target.relative_to(ROOT).as_posix(),
                html_sha256=sha(html), manifest_sha256=sha(manifest))


def validate(data, target):
    if target.endswith(".svg"):
        root = ET.fromstring(data)
        if root.tag.rsplit("}", 1)[-1] != "svg":
            raise ValueError("Not an SVG")
        if any(el.tag.rsplit("}", 1)[-1] == "script" for el in root.iter()):
            raise ValueError("Unexpected script in SVG")
    elif data[:4] != b"\x00\x00\x01\x00":
        raise ValueError("Not an ICO")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    previous = json.loads(RECEIPT.read_text(encoding="utf8")) if RECEIPT.exists() else {}
    entries = {entry["route"]: entry for entry in previous.get("entries", [])}
    for route in ROUTES:
        entry = declared(route)
        target = ROOT / entry["path"]
        if args.check:
            old = entries[route]
            if any(old[key] != value for key, value in entry.items()):
                raise ValueError(f"Source drift: {route}")
            data = target.read_bytes()
            validate(data, entry["path"])
            if old["sha256"] != sha(data):
                raise ValueError(f"Icon drift: {route}")
        elif args.fetch:
            with urlopen(Request(entry["url"], headers={"User-Agent": "SynapseSourceAudit/1.0"}), timeout=12) as response:
                data = response.read(2 * 1024 * 1024 + 1)
                if len(data) > 2 * 1024 * 1024:
                    raise ValueError("Oversized icon")
                entry.update(final_url=response.geturl(), content_type=response.headers.get("Content-Type"),
                             etag=response.headers.get("ETag"))
            validate(data, entry["path"])
            if target.exists() and target.read_bytes() != data:
                raise ValueError(f"Existing current source differs; review before replacing {target}")
            target.write_bytes(data)
            entry.update(sha256=sha(data), bytes=len(data), fetched_utc=datetime.now(timezone.utc).isoformat())
            entries[route] = entry
            RECEIPT.write_text(json.dumps(dict(method="Exact current HTML + manifest icon URLs; live asset download only; no code execution", entries=list(entries.values())), indent=2) + "\n", encoding="utf8")
        print(route, entry["url"], flush=True)


if __name__ == "__main__":
    main()
