"""Acquire exact Webpack product images, PluginImages cards and manifest favicons.

Run after fetch-razer-sources.py. Only parse downloaded source text; never run it.
Raw image bytes and HTTP/hash receipts are separate from prepare-resources.py's
offline conversion. --products explicitly selects the products to acquire.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import importlib.util
import json
import re
from collections import Counter
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "discovery", Path(__file__).with_name("discover-razer-products.py"))
discovery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(discovery)

MODULE_EXPORT = re.compile(
    r'(?<![\w])(?P<id>\d+):\([^)]*\)=>\{(?:"use strict";)?'
    r'\w+\.exports=\w+\.p\+"(?P<path>static/media/[^"\\]+)";?\}'
)
INLINE_EXPORT = re.compile(
    r'(?<![\w])(?P<id>\d+):(?:\([^)]*\)|\w+)=>\{(?:"use strict";)?'
    r'\w+\.exports="data:image/(?:png|avif);base64,[A-Za-z0-9+/=]+";?\}'
)


def source_jobs(root, pid):
    directory = root / ".ref/devices" / str(pid)
    scripts = sorted((directory / "static/js").glob("*.js"))
    main = next(path for path in scripts if path.name.startswith("main."))
    modules = {}
    for script in scripts:
        text = script.read_text(encoding="utf-8")
        for match in MODULE_EXPORT.finditer(text):
            module_id, media = int(match["id"]), match["path"]
            if module_id in modules and modules[module_id] != media:
                raise ValueError(f"Conflicting media module {pid}:{module_id}")
            modules[module_id] = media
        for match in INLINE_EXPORT.finditer(text):
            # This image is already a literal inside the manifest-declared JS.
            # Static resource preparation decodes it; there is no media URL.
            module_id = int(match['id'])
            if module_id in modules and modules[module_id] is not None:
                raise ValueError(f'Conflicting inline media module {pid}:{module_id}')
            modules[module_id] = None
    requests = set(re.findall(
        r'"\./' + str(pid) + r'_(\d+)/img_prods/([^"\\]+)":\[(\d+),[^\]]+\]',
        main.read_text(encoding="utf-8")))
    if not requests:
        raise ValueError(f"No product image contexts for {pid}")
    base = f"https://apps.razer.com/synapse/products/{pid}/ui/"
    jobs = []
    variants = set()
    for edition, name, module_id in sorted(requests):
        media = modules[int(module_id)]
        if media is not None and not re.fullmatch(r"static/media/[A-Za-z0-9_.@-]+\.avif", media):
            raise ValueError(f"Unexpected product image export: {pid}:{media}")
        if media is not None:
            jobs.append((base + media, directory / media, "media"))
        if name == "prd-3x.png":
            variants.add((int(edition), 0))
        elif re.fullmatch(r"\d+-3x\.png", name):
            variants.add((int(edition), int(name.split("-")[0])))
    if not variants:
        raise ValueError(f"No Dashboard product identities for {pid}")
    for edition, layout in sorted(variants):
        url = (base + f"{pid}_{edition}/PluginImages/"
               f"{pid}_{edition}_{layout}_dashboard3x.avif")
        path = root / "assets/synapse" / f"dashboard-{pid}-{edition}-{layout}-source.avif"
        jobs.append((url, path, "media"))
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    for icon in manifest.get("icons", []):
        relative = icon.get("src", "")
        if not re.fullmatch(r"/synapse/assets/imgs/favicon/[A-Z_]+\.svg", relative):
            raise ValueError(f"Unexpected manifest favicon: {pid}:{relative}")
        jobs.append(("https://apps.razer.com" + relative,
                     root / ".ref/applications/synapse/dashboard/shared-favicon" / Path(relative).name, "svg"))
    return jobs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--products", type=int, nargs="+", required=True)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--attempts", type=int, default=3)
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--retry-errors", action="store_true")
    parser.add_argument("--offline", action="store_true")
    options = parser.parse_args()
    options.keepalive = True
    if options.workers < 1 or options.attempts < 1 or options.timeout <= 0:
        parser.error("workers, attempts and timeout must be positive")
    if options.offline and options.refresh:
        parser.error("offline and refresh are mutually exclusive")
    root = Path(__file__).resolve().parents[1]
    jobs = {}
    for pid in sorted(set(options.products)):
        for url, path, kind in source_jobs(root, pid):
            jobs[url] = (path, kind)
    print(f"Product image acquisition: {len(set(options.products))} products, "
          f"{len(jobs)} image URLs", flush=True)
    counts = Counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=options.workers) as pool:
        pending = {pool.submit(discovery.fetch_snapshot, url, path, kind, options): url
                   for url, (path, kind) in sorted(jobs.items())}
        for index, future in enumerate(concurrent.futures.as_completed(pending), 1):
            receipt = future.result()
            counts[receipt["result"]] += 1
            if receipt["result"] != "ok":
                print(f"{receipt['result']}: {pending[future]}", flush=True)
            if index % 10 == 0 or index == len(pending):
                print(f"Images {index}/{len(pending)}: {dict(counts)}", flush=True)
    if counts["ok"] != len(jobs):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
