"""Acquire source text from the recorded product manifests; never execute it.

Downloads entrypoints first, then all manifest-listed JS/CSS. Every response has
its own URL, HTTP status and SHA-256 receipt. Resume trusts verified bytes only.
Images, source maps, drivers and installers are outside this acquisition step.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import importlib.util
import json
import urllib.parse
from collections import Counter
from pathlib import Path

spec = importlib.util.spec_from_file_location("discovery", Path(__file__).with_name("discover-razer-products.py"))
discovery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(discovery)


def local_path(root, pid, url):
    parsed = urllib.parse.urlparse(url)
    prefix = f"/synapse/products/{pid}/ui/"
    if parsed.scheme != "https" or parsed.netloc != "apps.razer.com" or not parsed.path.startswith(prefix):
        raise ValueError(f"Unexpected product source URL: {url}")
    relative = urllib.parse.unquote(parsed.path[len(prefix):])
    if any(part in ("", ".", "..") or ":" in part or "\\" in part for part in relative.split("/")):
        raise ValueError(f"Unsafe source path: {url}")
    target = (root / ".ref/devices" / str(pid) / relative).resolve()
    if (root / ".ref/devices" / str(pid)).resolve() not in target.parents:
        raise ValueError(f"Source path escapes product: {url}")
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--entrypoints-only", action="store_true")
    parser.add_argument("--workers", type=int, default=12)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--attempts", type=int, default=3)
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--retry-errors", action="store_true")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--products", type=int, nargs="+")
    options = parser.parse_args()
    options.keepalive = True
    if options.workers < 1 or options.attempts < 1 or options.timeout <= 0:
        parser.error("workers, attempts and timeout must be positive")
    if options.offline and options.refresh:
        parser.error("offline and refresh are mutually exclusive")
    root = Path(__file__).resolve().parents[1]
    catalog = discovery.read_json(root / ".ref/discovery/products.json")
    if not catalog:
        parser.error("Run discover-razer-products.py first")
    started = discovery.utc_now()
    jobs = {}
    products = []
    for product in catalog["products"]:
        pid = product["product_id"]
        if options.products and pid not in options.products:
            continue
        if not product["ui_exists"]:
            continue
        products.append(pid)
        base = product["endpoints"]["ui/index.html"]["final_url"]
        entrypoints = {urllib.parse.urljoin(base, ref) for ref in product["asset_entrypoints"]}
        for endpoint in ("ui/index.html", "ui/manifest.json", "ui/asset-manifest.json"):
            receipt = product["endpoints"][endpoint]
            if receipt["result"] != "ok":
                continue
            source = root / ".ref/discovery/products" / str(pid) / endpoint
            if discovery.sha256(source.read_bytes()) != receipt["sha256"]:
                raise ValueError(f"Changed discovery snapshot: {source}")
            destination = root / ".ref/devices" / str(pid) / source.name
            discovery.write_bytes(destination, source.read_bytes())
            discovery.write_json(destination.with_name(destination.name + ".http.json"), receipt)
        for resource in product["resource_entries"]:
            url = resource["url"]
            suffix = Path(urllib.parse.urlparse(url).path).suffix
            if suffix not in (".js", ".css") or (options.entrypoints_only and url not in entrypoints):
                continue
            path = local_path(root, pid, url)
            jobs[url] = (pid, path, suffix[1:], url in entrypoints)
    ordered = sorted(jobs.items(), key=lambda row: (not row[1][3], row[1][0], row[0]))
    counts = Counter()
    results = {pid: [] for pid in products}
    print(f"Source acquisition: {len(products)} products, {len(jobs)} JS/CSS URLs", flush=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=options.workers) as pool:
        futures = {pool.submit(discovery.fetch_snapshot, url, path, kind, options, pid):
                   (url, pid, path, entry) for url, (pid, path, kind, entry) in ordered}
        for index, future in enumerate(concurrent.futures.as_completed(futures), 1):
            url, pid, path, entry = futures[future]
            receipt = future.result()
            counts[receipt["result"]] += 1
            results[pid].append({"path": path.relative_to(root).as_posix(), "url": url,
                "entrypoint": entry, "result": receipt["result"], "sha256": receipt["sha256"],
                "bytes": receipt["bytes"]})
            if index % 100 == 0 or index == len(futures):
                print(f"Sources {index}/{len(futures)}: {dict(counts)}", flush=True)
    output = {"schema_version": 1, "started_at_utc": started, "generated_at_utc": discovery.utc_now(),
              "entrypoints_only": options.entrypoints_only, "selected_products": options.products,
              "summary": dict(counts), "products": [{"product_id": pid,
                "files": sorted(files, key=lambda item: item["path"]),
                "selected_files_complete": all(item["result"] == "ok" for item in files)}
                for pid, files in sorted(results.items())]}
    name = "entrypoint-sources.json" if options.entrypoints_only else "code-sources.json"
    if options.products:
        name = name.replace(".json", "-selected.json")
    discovery.write_json(root / ".ref/discovery" / name, output)
    print(f"Wrote {name}; {dict(counts)}", flush=True)
    if counts["ok"] != len(jobs):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
