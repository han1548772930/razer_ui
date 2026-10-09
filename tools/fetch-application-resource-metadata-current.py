"""Fetch source-linked JSON/XML metadata only; never follow package enclosures."""
import concurrent.futures
import argparse
import hashlib
import json
from pathlib import Path
import urllib.error
import urllib.request
import urllib.parse
from datetime import datetime, timezone
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/re/application-resource-metadata-2026-10-09"
LIMIT = 2 * 1024 * 1024
URLS = [
    ("background-installer", "https://apps.razer.com/background-manager/installer-manifest.json"),
    ("synapse-assets-index", "https://apps.razer.com/synapse/assets/index.json?filter=synapseAssets.json"),
    ("alisha-appcast", "https://appcasts.razer.com/alisha/appcast.xml"),
    ("lite-appcast", "https://appcasts.razer.com/sophie-lite/appcast.xml"),
    ("lite-thx", "https://appcasts.razer.com/sophie-lite/appcast-thx.xml"),
    ("lite-sparkle", "https://appcasts.razer.com/sophie-lite/appcast-rzsparkle.xml"),
    ("sophie-appcast", "https://appcasts.razer.com/sophie/appcast.xml"),
    ("sophie-thx", "https://appcasts.razer.com/sophie/appcast-thx.xml"),
    ("natalie-appcast", "https://appcasts.razer.com/natalie/appcast.xml"),
    ("natalie-dll", "https://appcasts.razer.com/natalie/appcast-dll.xml"),
    ("natalie-sparkle", "https://appcasts.razer.com/natalie/appcast-rzsparkle.xml"),
]


def validate_metadata_url(url):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "https" or parsed.hostname not in {"apps.razer.com", "appcasts.razer.com"} or not parsed.path.endswith((".json", ".xml")):
        raise ValueError("only source-linked official JSON/XML endpoints are permitted")


class MetadataRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        validate_metadata_url(newurl)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def fetch_metadata(item):
    name, url = item
    validate_metadata_url(url)
    if not name.replace("-", "").isalnum():
        raise ValueError("unsafe receipt name")
    row = {"id": name, "url": url, "queried_utc": datetime.now(timezone.utc).isoformat()}
    req = urllib.request.Request(url, headers={"Cache-Control": "no-cache", "User-Agent": "StaticSourceAudit/1.0"})
    try:
        try:
            response = urllib.request.build_opener(MetadataRedirectHandler()).open(req, timeout=35)
        except urllib.error.HTTPError as err:
            response = err
        with response:
            row.update(status=response.code, final_url=response.geturl(), headers=dict(response.headers.items()))
            content = response.read(LIMIT + 1)
            if len(content) > LIMIT:
                raise ValueError("metadata size limit exceeded")
            row.update(size=len(content), sha256=hashlib.sha256(content).hexdigest())
            suffix = ".json" if ".json" in url else ".xml"
            file = OUT / (name + suffix)
            file.write_bytes(content)
            row["body_path"] = file.relative_to(ROOT).as_posix()
            row["parse_status"] = "not_parsed"
            if row["status"] == 200:
                if suffix == ".json":
                    json.loads(content)
                else:
                    import xml.etree.ElementTree as ET
                    ET.fromstring(content)
                row["parse_status"] = "valid_data"
    except Exception as err:
        row["error"] = str(err)
    return row


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("name", nargs="?", help="Receipt identifier for one source-proven metadata endpoint")
    parser.add_argument("url", nargs="?", help="Exact official HTTPS JSON/XML URL")
    parser.add_argument("--summarize", action="store_true", help="Parse saved XML receipts only, without network access")
    args = parser.parse_args()
    if bool(args.name) != bool(args.url):
        parser.error("name and url must be supplied together")
    if args.summarize and args.name:
        parser.error("--summarize does not accept a network endpoint")
    OUT.mkdir(parents=True, exist_ok=True)
    if args.summarize:
        rows = []
        saved = json.loads((OUT / "http-receipts.json").read_text(encoding="utf-8"))
        for receipt in saved["receipts"]:
            if receipt["status"] != 200 or not receipt["body_path"].endswith(".xml"):
                continue
            raw = (ROOT / receipt["body_path"]).read_bytes()
            if hashlib.sha256(raw).hexdigest() != receipt["sha256"]:
                raise ValueError("XML receipt hash mismatch")
            root = ET.fromstring(raw)
            channel = root.find("channel")
            rows.append({"id": receipt["id"], "file": receipt["body_path"], "sha256": receipt["sha256"], "channel_link": channel.findtext("link"), "items": [{"title": item.findtext("title"), "pubDate": item.findtext("pubDate"), "releaseNotesLink": item.findtext("{http://www.andymatuschak.org/xml-namespaces/sparkle}releaseNotesLink"), "enclosure_attributes": dict(item.find("enclosure").attrib)} for item in channel.findall("item")]})
        (OUT / "appcast-items.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"saved_xml_appcasts": len(rows), "items": sum(len(row["items"]) for row in rows)}))
        sys.exit(0)
    # Extra requests must be exact metadata URLs obtained from the saved first-stage manifest.
    items = URLS if args.name is None else [(args.name, args.url)]
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        receipts = list(pool.map(fetch_metadata, items))
    receipt_path = OUT / ("http-receipts.json" if args.name is None else args.name + "-http-receipt.json")
    receipt_path.write_text(json.dumps({"policy": "JSON/XML bodies only; referenced binary URLs are recorded, never fetched", "receipts": receipts}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for row in receipts:
        print(json.dumps({k: row.get(k) for k in ["id", "status", "size", "parse_status", "error"]}))
