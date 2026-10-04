"""Fetch selected media from a cached application or product asset manifest.

Only reads manifests and media bytes; never executes downloaded source. Uses the
same HTTP/hash receipts and offline validation as the source discovery tools.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import importlib.util
import json
import re
from pathlib import Path
from urllib.parse import quote, unquote, urljoin, urlparse

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "discovery", Path(__file__).with_name("discover-razer-products.py"))
discovery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(discovery)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--route", help="Application route below apps.razer.com")
    source.add_argument("--product", type=int, help="Current Synapse product ID")
    parser.add_argument("--assets", nargs="+", default=[],
                        help="Exact static/media/... keys in asset-manifest.json")
    parser.add_argument("--shared-app-icons", nargs="+", default=[],
                        help="Exact SVG filenames under /synapse/assets/imgs/apps/")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--retry-errors", action="store_true")
    parser.add_argument("--workers", type=int, default=6)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--attempts", type=int, default=3)
    options = parser.parse_args()
    options.keepalive = True
    if options.product is not None:
        if options.product <= 0:
            parser.error("product must be a positive ID")
        route = f"synapse/products/{options.product}/ui"
        directory = ROOT / ".ref/devices" / str(options.product)
    else:
        route = options.route.strip("/")
        if not re.fullmatch(r"[a-z0-9-]+(?:/[a-z0-9-]+)*", route):
            parser.error("route must name an application below apps.razer.com")
        directory = ROOT / ".ref/applications" / route
    if options.workers < 1 or options.timeout <= 0 or options.attempts < 1:
        parser.error("workers, timeout and attempts must be positive")
    if options.offline and options.refresh:
        parser.error("offline and refresh are mutually exclusive")
    if not options.assets and not options.shared_app_icons:
        parser.error("specify assets or shared app icons")
    manifest = json.loads((directory / "asset-manifest.json").read_text(encoding="utf-8-sig"))
    base = f"https://apps.razer.com/{route}/"
    jobs = []
    for request in sorted(set(options.assets)):
        if not re.fullmatch(r"static/media/[^/\\]+\.(?:svg|avif|png|webp|jpg|jpeg|gif|mp4)", request):
            parser.error(f"unsupported media request: {request}")
        if request not in manifest["files"]:
            parser.error(f"asset absent from manifest: {request}")
        url = urljoin(base, quote(manifest["files"][request], safe="/%:._-"))
        prefix = f"/{route}/static/media/"
        parsed = urlparse(url)
        filename = unquote(parsed.path.removeprefix(prefix))
        if (parsed.scheme != "https" or parsed.netloc != "apps.razer.com"
                or not parsed.path.startswith(prefix) or "/" in filename
                or "\\" in filename or filename in ("", ".", "..")):
            parser.error(f"unexpected media URL: {url}")
        path = directory / "static/media" / filename
        jobs.append((url, path, "svg" if path.suffix == ".svg" else "media"))
    for name in sorted(set(options.shared_app_icons)):
        if not re.fullmatch(r"[a-zA-Z0-9_-]+\.svg", name):
            parser.error(f"unsupported shared icon filename: {name}")
        jobs.append((f"https://apps.razer.com/synapse/assets/imgs/apps/{name}",
                     ROOT / ".ref/applications/synapse/dashboard/shared-apps" / name, "svg"))
    with concurrent.futures.ThreadPoolExecutor(max_workers=options.workers) as pool:
        replies = list(pool.map(
            lambda job: discovery.fetch_snapshot(*job, options), jobs))
    for (url, _, _), reply in zip(jobs, replies):
        print(f"{reply['result']}: {url}", flush=True)
    if any(reply["result"] != "ok" for reply in replies):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
