"""Snapshot published Razer product metadata without executing or downloading JS.

The six source-defined device catalogs are the primary seeds. Historical probes
are additional evidence, not a complete catalog. USB/BLE/console aliases remain
relationships rather than becoming independent product seeds.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import datetime as dt
import hashlib
import json
import re
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import Counter
from contextlib import contextmanager
from html.parser import HTMLParser
from pathlib import Path


CATALOGS = (
    "AvailableDevices.json", "Windows10Devices.json", "Synapse2Devices.json",
    "XboxHeadsets.json", "XboxDevices.json", "inDevelopmentDevices.json",
)
ALIASES = ("dongleId", "bleId", "xBoxId", "wiredId", "psModeIds", "monitorPortIds", "repId")
ENDPOINTS = ("ui/index.html", "ui/manifest.json", "ui/asset-manifest.json", "mw/manifest.json")
SOURCE_BUNDLE = ".ref/background-manager/assets/index-8d39b3d5.js"
HTTP_LOCAL = threading.local()


@contextmanager
def open_reply(request, redirect, timeout, keepalive=False):
    # Large source acquisitions reuse TLS connections. Keep metadata discovery
    # usable with only Python's standard library when requests is unavailable.
    try:
        if not keepalive:
            raise ImportError
        import requests
    except ImportError:
        with urllib.request.build_opener(redirect).open(request, timeout=timeout) as response:
            yield response
        return
    if not hasattr(HTTP_LOCAL, "session"):
        HTTP_LOCAL.session = requests.Session()
    try:
        with HTTP_LOCAL.session.get(request.full_url, headers=dict(request.header_items()),
                                    timeout=timeout, stream=True) as response:
            for previous in response.history:
                redirect.chain.append({"from": previous.url,
                    "to": urllib.parse.urljoin(previous.url, previous.headers.get("Location", "")),
                    "status": previous.status_code})
            class Reply:
                status = response.status_code
                url = response.url
                headers = response.headers

                def read(self):
                    chunks, length = [], 0
                    for chunk in response.iter_content(128 * 1024):
                        length += len(chunk)
                        if length > 64 * 1024 * 1024:
                            raise OSError("Source response exceeds 64 MiB")
                        chunks.append(chunk)
                    return b"".join(chunks)
            yield Reply()
    except requests.RequestException as error:
        raise urllib.error.URLError(str(error)) from error


def utc_now():
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    write_bytes(path, (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))


def write_bytes(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".part")
    temporary.write_bytes(data)
    temporary.replace(path)


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return None


class PageReferences(HTMLParser):
    def __init__(self):
        super().__init__()
        self.scripts = []
        self.styles = []
        self.manifests = []
        self.has_html = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "html":
            self.has_html = True
        if tag == "script" and attrs.get("src"):
            self.scripts.append(attrs["src"])
        if tag == "link" and attrs.get("href"):
            if attrs.get("rel") == "stylesheet":
                self.styles.append(attrs["href"])
            if attrs.get("rel") == "manifest":
                self.manifests.append(attrs["href"])


def validate_payload(kind, data, pid=None):
    try:
        if kind == "svg":
            import xml.etree.ElementTree as ET
            try:
                valid = ET.fromstring(data).tag == "{http://www.w3.org/2000/svg}svg"
            except ET.ParseError:
                valid = False
            return {"valid": valid, "kind": kind, "reason": None if valid else "Expected SVG document"}
        if kind == "media":
            # The selective asset fetcher also accepts the original PNG/JPEG/GIF
            # requests. Check their file signatures without executing source.
            valid = len(data) > 12 and (
                data[4:8] == b"ftyp" or data[:4] == b"RIFF"
                or data.startswith(b"\x89PNG\r\n\x1a\n")
                or data.startswith(b"\xff\xd8\xff")
                or data[:6] in (b"GIF87a", b"GIF89a"))
            return {"valid": valid, "kind": kind,
                    "reason": None if valid else "Expected MP4/AVIF/WebP/PNG/JPEG/GIF media container"}
        text = data.decode("utf-8-sig")
        if kind in ("js", "css"):
            valid = bool(text.strip()) and not re.match(r"\s*<(?:!doctype|html|\?xml)", text, re.I)
            return {"valid": valid, "kind": kind,
                    "reason": None if text.strip() and valid else "Empty code response or HTML fallback"}
        if kind in ("html", "app_html"):
            parser = PageReferences()
            parser.feed(text)
            own_scripts = [s for s in parser.scripts if "/synapse/assets/" not in s]
            valid = parser.has_html and bool(own_scripts) and (kind == "app_html" or bool(parser.manifests))
            return {"valid": valid, "kind": kind,
                    "reason": None if valid else "Expected HTML app entry with own script and manifest",
                    "scripts": parser.scripts, "styles": parser.styles, "manifests": parser.manifests}
        value = json.loads(text)
        if kind == "catalog":
            valid = isinstance(value, list) and all(
                isinstance(row, dict) and isinstance(row.get("productId"), int) for row in value)
        elif kind == "assets":
            valid = isinstance(value, dict) and isinstance(value.get("files"), dict) and all(
                isinstance(k, str) and isinstance(v, str) for k, v in value["files"].items())
        else:
            # Several published UI manifests contain only deployment metadata;
            # product translations live in the paired middleware manifest.
            valid = isinstance(value, dict) and bool(value.get("name") or value.get("short_name")) and (
                "productTranslations" in value or "deviceName" in value or
                ("version" in value and ("gitMetadata" in value or "resourceManifest" in value)) or
                ("start_url" in value and "icons" in value))
        result = {"valid": valid, "kind": kind,
                  "reason": None if valid else f"Unexpected {kind} JSON structure"}
        if valid and kind == "manifest":
            translated_pids = sorted({int(key.split("_")[0]) for key in value.get("productTranslations", {})
                                     if re.fullmatch(r"\d+_\d+", key)})
            result["declared_product_ids"] = translated_pids
            result["requested_product_id_declared"] = pid in translated_pids if translated_pids else None
            result["identity_warning"] = (
                "Manifest translations declare other product IDs; inspect shared application or alias"
                if translated_pids and pid not in translated_pids else None)
            result["application_name"] = value.get("short_name", value.get("name"))
        return result
    except (UnicodeError, ValueError, TypeError) as error:
        return {"valid": False, "kind": kind, "reason": str(error)}


class RedirectRecorder(urllib.request.HTTPRedirectHandler):
    def __init__(self):
        super().__init__()
        self.chain = []

    def redirect_request(self, request, fp, code, message, headers, newurl):
        self.chain.append({"from": request.full_url, "to": newurl, "status": code})
        return super().redirect_request(request, fp, code, message, headers, newurl)


def fetch_snapshot(url, path, kind, options, pid=None):
    metadata_path = path.with_name(path.name + ".http.json")
    previous = read_json(metadata_path)
    if not options.refresh and isinstance(previous, dict) and previous.get("source_url") == url:
        cached_body = path.parent / previous.get("body_file", path.name)
        cache_status = previous.get("result")
        reusable = cache_status in ("ok", "not_found", "invalid_payload") or (
            not options.retry_errors and cache_status == "http_error")
        if reusable and cached_body.is_file() and sha256(cached_body.read_bytes()) == previous.get("sha256"):
            if previous.get("http_status") == 200:
                previous["validation"] = validate_payload(kind, cached_body.read_bytes(), pid)
                previous["result"] = "ok" if previous["validation"]["valid"] else "invalid_payload"
                write_json(metadata_path, previous)
            return previous
    if getattr(options, "offline", False):
        raise RuntimeError(f"Verified snapshot unavailable in offline mode: {url}")
    attempts = []
    body = b""
    metadata = None
    for attempt in range(1, options.attempts + 1):
        redirect = RedirectRecorder()
        request = urllib.request.Request(url, headers={
            "User-Agent": "RazerSourceCatalog/1.0 (read-only metadata discovery)",
            "Accept": "*/*" if kind in ("html", "js", "css") else "application/json",
            "Cache-Control": "no-cache",
        })
        started = utc_now()
        status = None
        final_url = url
        content_type = None
        last_modified = None
        etag = None
        error = None
        try:
            with open_reply(request, redirect, options.timeout, getattr(options, "keepalive", False)) as response:
                status = response.status
                final_url = response.url
                content_type = response.headers.get("Content-Type")
                last_modified = response.headers.get("Last-Modified")
                etag = response.headers.get("ETag")
                body = response.read()
        except urllib.error.HTTPError as exc:
            status = exc.code
            final_url = exc.url
            content_type = exc.headers.get("Content-Type")
            body = exc.read()
            error = str(exc)
        except (OSError, urllib.error.URLError, TimeoutError) as exc:
            body = b""
            error = f"{type(exc).__name__}: {exc}"
        validation = validate_payload(kind, body, pid) if status == 200 else None
        if status == 200:
            result = "ok" if validation["valid"] else "invalid_payload"
        elif status == 404:
            result = "not_found"
        elif status is None:
            result = "network_error"
        else:
            result = "http_error"
        attempts.append({"attempt": attempt, "started_at_utc": started, "completed_at_utc": utc_now(),
                         "status": status, "result": result, "error": error,
                         "final_url": final_url, "redirect_chain": redirect.chain})
        metadata = {"source_url": url, "final_url": final_url, "redirected": final_url != url,
                    "http_status": status, "fetched_at_utc": utc_now(), "content_type": content_type,
                    "last_modified": last_modified, "etag": etag, "result": result,
                    "sha256": sha256(body), "bytes": len(body), "validation": validation,
                    "attempts": attempts, "error": error}
        transient = status is None or status in (408, 425, 429, 500, 502, 503, 504) or result == "invalid_payload"
        if not transient or attempt == options.attempts:
            break
        time.sleep(min(0.5 * 2 ** (attempt - 1), 4.0))
    body_path = path if metadata["http_status"] == 200 else path.with_name(path.name + ".response")
    metadata["body_file"] = body_path.name
    write_bytes(body_path, body)
    write_json(metadata_path, metadata)
    return metadata


def load_success(path, metadata):
    return read_json(path) if metadata and metadata.get("result") == "ok" else None


def numeric_values(value):
    if isinstance(value, int):
        return [value]
    return [v for v in value if isinstance(v, int)] if isinstance(value, list) else []


def historical_products(root):
    products = {}
    snapshots = []
    for path in sorted((root / ".ref/notes").glob("razer-products*.txt")):
        body = path.read_bytes()
        snapshots.append({"path": str(path.relative_to(root)).replace("\\", "/"),
                          "sha256": sha256(body), "bytes": len(body)})
        for line in body.decode("utf-8-sig").splitlines():
            values = line.split("\t")
            if not values or not values[0].isdigit():
                continue
            pid = int(values[0])
            products.setdefault(pid, []).append({"path": snapshots[-1]["path"],
                "device_name": values[1] if len(values) > 1 else "",
                "application_name": values[2] if len(values) > 2 else "",
                "icon": values[3] if len(values) > 3 else ""})
    return products, snapshots


def product_summary(pid, output, seeds, history, snapshots):
    manifests = {part: load_success(output / "products" / str(pid) / f"{part}/manifest.json",
                                   snapshots.get(f"{part}/manifest.json")) for part in ("ui", "mw")}
    names = []
    classifications = []
    editions = set()
    declared_pids = set()
    versions = {}
    for part, manifest in manifests.items():
        if not manifest:
            continue
        if manifest.get("deviceName"):
            names.append({"source": part, "locale": "deviceName", "value": manifest["deviceName"]})
        for icon in manifest.get("icons", []):
            if isinstance(icon, dict) and icon.get("src"):
                category = Path(urllib.parse.urlparse(icon["src"]).path).stem
                classifications.append({"source": f"{part}/manifest.json:icons", "category": category,
                                        "icon": icon["src"]})
        for key, locales in manifest.get("productTranslations", {}).items():
            match = re.fullmatch(r"(\d+)_(\d+)", key)
            if not match:
                continue
            declared_pids.add(int(match[1]))
            if int(match[1]) != pid:
                continue
            editions.add(int(match[2]))
            for locale in ("en", "zh-cn", "zh-CN"):
                values = locales.get(locale, {}) if isinstance(locales, dict) else {}
                name = values.get("DASHBOARD_NAME") or values.get("PRODUCT")
                if name:
                    names.append({"source": part, "locale": locale, "edition": int(match[2]),
                                  "value": name, "edition_name": values.get("DASHBOARD_EDITION", "")})
        editions.update(numeric_values(manifest.get("editionId")))
        versions[part] = {key: manifest.get(key) for key in
                          ("name", "short_name", "version", "buildVersion", "gitMetadata")}
    for source in seeds.get(pid, []):
        row = source["entry"]
        raw_name = row.get("productName", row.get("deviceName"))
        if raw_name:
            names.append({"source": source["catalog"], "value": raw_name})
        editions.update(numeric_values(row.get("editionId")))
    if not names:
        names = [{"source": row["path"], "value": row["device_name"]}
                 for row in history.get(pid, []) if row["device_name"]]
    asset_path = output / "products" / str(pid) / "ui/asset-manifest.json"
    assets = load_success(asset_path, snapshots.get("ui/asset-manifest.json")) or {}
    asset_files = assets.get("files", {})
    base_url = snapshots["ui/asset-manifest.json"]["final_url"]
    resources = [{"name": name, "reference": value, "url": urllib.parse.urljoin(base_url, value)}
                 for name, value in asset_files.items()]
    js = [r for r in resources if urllib.parse.urlparse(r["url"]).path.endswith(".js")]
    css = [r for r in resources if urllib.parse.urlparse(r["url"]).path.endswith(".css")]
    connections = [{"source": source["catalog"], "kind": field, "ids": numeric_values(source["entry"][field])}
                   for source in seeds.get(pid, []) for field in ALIASES if field in source["entry"]]
    return {"product_id": pid, "catalog_sources": seeds.get(pid, []),
            "historical_sources": history.get(pid, []), "connection_aliases": connections,
            "names": names, "classifications": classifications, "edition_ids": sorted(editions),
            "manifest_declared_product_ids": sorted(declared_pids), "versions": versions,
            "endpoints": snapshots, "ui_exists": snapshots["ui/index.html"]["result"] == "ok",
            "ui_manifest_exists": snapshots["ui/manifest.json"]["result"] == "ok",
            "mw_manifest_exists": snapshots["mw/manifest.json"]["result"] == "ok",
            "js_count": len(js), "css_count": len(css),
            "asset_entrypoints": assets.get("entrypoints", []), "resource_entries": resources,
            "html_resources": snapshots["ui/index.html"].get("validation")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path, default=Path(".ref/discovery"))
    parser.add_argument("--base-url", default="https://apps.razer.com")
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--timeout", type=float, default=20)
    parser.add_argument("--attempts", type=int, default=3)
    parser.add_argument("--refresh", action="store_true", help="Refetch even verified cached snapshots")
    parser.add_argument("--retry-errors", action="store_true", help="Also retry cached terminal HTTP errors")
    parser.add_argument("--offline", action="store_true", help="Revalidate saved hashes and schemas without network")
    parser.add_argument("--limit", type=int, help="Fetch only the first N seed PIDs; report catalog coverage separately")
    options = parser.parse_args()
    if options.workers < 1 or options.attempts < 1 or options.timeout <= 0:
        parser.error("workers, attempts and timeout must be positive")
    if options.offline and options.refresh:
        parser.error("offline and refresh are mutually exclusive")
    root = options.root.resolve()
    output = (root / options.output).resolve()
    if root not in output.parents or output.name != "discovery":
        parser.error("output must be a discovery directory inside the workspace")
    started_at = utc_now()
    catalog_snapshots = {}
    seeds = {}
    print(f"Catalog discovery started {started_at}; workers={options.workers}", flush=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=options.workers) as pool:
        future_catalogs = {pool.submit(fetch_snapshot,
            f"{options.base_url}/synapse/dashboard/{name}", output / "catalogs" / name,
            "catalog", options): name for name in CATALOGS}
        for future in concurrent.futures.as_completed(future_catalogs):
            name = future_catalogs[future]
            metadata = future.result()
            catalog_snapshots[name] = metadata
            rows = load_success(output / "catalogs" / name, metadata)
            if rows is None:
                raise SystemExit(f"Catalog {name} unavailable ({metadata['result']}); do not declare complete seeds")
            for row in rows:
                seeds.setdefault(row["productId"], []).append({"catalog": name, "entry": row})
            print(f"Catalog {name}: {len(rows)} entries; HTTP {metadata['http_status']}", flush=True)
        history, historical_snapshots = historical_products(root)
        all_pids = sorted(set(seeds) | set(history))
        selected_pids = all_pids[:options.limit] if options.limit is not None else list(all_pids)
        print(f"Seeds: {len(seeds)} official primary IDs + {len(history)} historical IDs = {len(all_pids)}; fetching {len(selected_pids)}", flush=True)
        related = {}
        for pid, sources in seeds.items():
            for source in sources:
                for field in ALIASES:
                    for alias in numeric_values(source["entry"].get(field)):
                        related.setdefault(alias, []).append({"product_id": pid,
                            "source": source["catalog"], "relation": field})
        if options.limit is None:
            selected_pids = sorted(set(selected_pids) | set(related))
        endpoint_snapshots = {}
        counts = Counter()
        pending = selected_pids
        while pending:
            print(f"Checking {len(pending)} product/alias IDs", flush=True)
            futures = {}
            for pid in pending:
                endpoint_snapshots[pid] = {}
                for endpoint in ENDPOINTS:
                    kind = "html" if endpoint.endswith(".html") else "assets" if "asset-manifest" in endpoint else "manifest"
                    url = f"{options.base_url}/synapse/products/{pid}/{endpoint}"
                    futures[pool.submit(fetch_snapshot, url, output / "products" / str(pid) / endpoint,
                                        kind, options, pid)] = (pid, endpoint)
            for completed, future in enumerate(concurrent.futures.as_completed(futures), 1):
                pid, endpoint = futures[future]
                metadata = future.result()
                endpoint_snapshots[pid][endpoint] = metadata
                counts[metadata["result"]] += 1
                if completed % 100 == 0 or completed == len(futures):
                    print(f"Endpoints {completed}/{len(futures)}: {dict(counts)}", flush=True)
            extra = set()
            for pid in pending:
                for endpoint, metadata in endpoint_snapshots[pid].items():
                    for declared in (metadata.get("validation") or {}).get("declared_product_ids", []):
                        if declared != pid:
                            related.setdefault(declared, []).append({"product_id": pid,
                                "source": endpoint, "relation": "manifest_translation"})
                        if declared not in endpoint_snapshots:
                            extra.add(declared)
            pending = sorted(extra) if options.limit is None else []
        selected_pids = sorted(endpoint_snapshots)
    products = [product_summary(pid, output, seeds, history, endpoint_snapshots[pid]) for pid in selected_pids]
    for product in products:
        product["referenced_by"] = related.get(product["product_id"], [])
    alias_ids = {alias for rows in seeds.values() for source in rows for field in ALIASES
                 for alias in numeric_values(source["entry"].get(field))}
    categories = Counter()
    for product in products:
        product_categories = {v["category"] for v in product["classifications"]}
        categories.update(product_categories or ["UNKNOWN"])
    summary = {"official_primary_ids": len(seeds), "historical_primary_ids": len(history),
               "seed_product_ids": len(all_pids), "fetched_product_ids": len(products),
               "all_seeds_processed": set(all_pids).issubset(selected_pids),
               "related_only_ids": len(set(selected_pids) - set(all_pids)),
               "alias_ids": len(alias_ids), "alias_ids_without_primary": len(alias_ids - set(seeds)),
               "historical_only_ids": sorted(set(history) - set(seeds)),
               "ui_html_found": sum(p["ui_exists"] for p in products),
               "ui_manifests_found": sum(p["ui_manifest_exists"] for p in products),
               "mw_manifests_found": sum(p["mw_manifest_exists"] for p in products),
               "endpoints": dict(counts), "categories": dict(categories),
               "js_references": sum(p["js_count"] for p in products),
               "css_references": sum(p["css_count"] for p in products)}
    source_bundle = root / SOURCE_BUNDLE
    write_json(output / "products.json", {
        "schema_version": 1, "started_at_utc": started_at, "generated_at_utc": utc_now(),
        "scope": "Published catalogs and historical probes at recorded fetch times; not a claim about future or unpublished products",
        "source_bundle": {"path": SOURCE_BUNDLE, "sha256": sha256(source_bundle.read_bytes())},
        "catalog_snapshots": catalog_snapshots, "historical_snapshots": historical_snapshots,
        "summary": summary, "seed_product_ids": all_pids, "candidate_product_ids": selected_pids, "products": products,
    })
    print(json.dumps(summary, ensure_ascii=True, indent=2), flush=True)
    print(f"Wrote {output / 'products.json'}", flush=True)


if __name__ == "__main__":
    main()
