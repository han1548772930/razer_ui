"""Acquire source-defined application entries and their declared JS/CSS as data."""
from __future__ import annotations

import argparse
import concurrent.futures
import importlib.util
import json
import re
from collections import Counter
from pathlib import Path
from urllib.parse import unquote, urljoin, urlparse

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("discovery", Path(__file__).with_name("discover-razer-products.py"))
discovery = importlib.util.module_from_spec(spec)
spec.loader.exec_module(discovery)
SOURCES = (
    ".ref/host-4.0.827/electron/constants.js",
    ".ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js",
    ".ref/applications/rz-app-menu/static/js/main.83ced465.js",
    ".ref/settings/static/js/720.1e5d1c8f.chunk.js",
)
ROUTE_PATH = r'/(?:synapse|chroma-app|natalie|alisha|sophie|sophie-lite|settings|cortex|rz-app-menu|rz-user-profile-menu|systray|release-patch-note|profile-migration|background-manager|feedback)[^"\x27`<>\s$]*'
PATH_LITERAL = re.compile(r'''["']((?:https://apps\.razer\.com)?''' + ROUTE_PATH + r''')["']''')
# Only the origin is interpolated: preserve the static route without evaluating JS.
ORIGIN_TEMPLATE = re.compile(r'`\$\{window\.location\.origin\}(' + ROUTE_PATH + r')`')
# A query-only interpolation does not affect the static application directory.
# Keep the literal prefix as evidence; never evaluate the template expression.
QUERY_TEMPLATE = re.compile(r'`(' + ROUTE_PATH + r'\?[^"\x27`<>\s$]*?)\$\{')


def route_seeds():
    routes = {}
    for name in SOURCES:
        path = ROOT / name
        if not path.is_file():
            continue
        source = path.read_text(encoding="utf-8")
        matches = sorted(
            (match for pattern in (PATH_LITERAL, ORIGIN_TEMPLATE, QUERY_TEMPLATE) for match in pattern.finditer(source)),
            key=lambda match: match.start(1),
        )
        for match in matches:
            route = urlparse(match[1]).path
            if "/assets" in route or "/products/" in route:
                continue
            if Path(route).suffix and not route.endswith("/index.html"):
                continue
            route = route.removesuffix("index.html").rstrip("/") + "/"
            routes.setdefault(route, []).append({"source": name, "offset": match.start(1),
                "literal": match[1], "sha256": discovery.sha256(path.read_bytes())})
    return routes


def source_path(url):
    parsed = urlparse(url)
    if parsed.scheme != "https" or parsed.netloc != "apps.razer.com":
        raise ValueError(f"Unexpected application source host: {url}")
    relative = unquote(parsed.path).lstrip("/")
    if any(part in ("", ".", "..") or ":" in part or "\\" in part for part in relative.split("/")):
        raise ValueError(f"Unsafe application source path: {url}")
    base = (ROOT / ".ref/applications").resolve()
    path = (base / relative).resolve()
    if base not in path.parents:
        raise ValueError(f"Source path escapes application directory: {url}")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workers", type=int, default=12)
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--attempts", type=int, default=3)
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--retry-errors", action="store_true")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--routes", nargs="+", help="Acquire only these source-defined routes; preserve other catalog records")
    options = parser.parse_args()
    options.keepalive = True
    if options.workers < 1 or options.attempts < 1 or options.timeout <= 0:
        parser.error("workers, attempts and timeout must be positive")
    if options.offline and options.refresh:
        parser.error("offline and refresh are mutually exclusive")
    seeds = route_seeds()
    selected_routes = {"/" + value.strip("/") + "/" for value in options.routes or []}
    if selected_routes - seeds.keys():
        parser.error("routes absent from current source literals: " + ", ".join(sorted(selected_routes - seeds.keys())))
    previous = discovery.read_json(ROOT / ".ref/discovery/applications.json") or {}
    records = list(previous.get("applications", [])) if selected_routes else []
    for route, evidence in sorted(seeds.items()):
        if selected_routes and route not in selected_routes:
            continue
        base = "https://apps.razer.com" + route
        endpoints = {}
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
            jobs = {pool.submit(discovery.fetch_snapshot, urljoin(base, name), source_path(urljoin(base, name)), kind, options): name
                    for name, kind in (("index.html", "app_html"), ("asset-manifest.json", "assets"), ("manifest.json", "manifest"))}
            for future in concurrent.futures.as_completed(jobs):
                endpoints[jobs[future]] = future.result()
        resources = set()
        html = endpoints["index.html"]
        if html["result"] == "ok":
            for reference in html["validation"]["scripts"] + html["validation"]["styles"]:
                resources.add(urljoin(html["final_url"], reference))
        assets = endpoints["asset-manifest.json"]
        if assets["result"] == "ok":
            manifest = discovery.read_json(source_path(urljoin(base, "asset-manifest.json")))
            resources.update(urljoin(base, ref) for ref in manifest["files"].values())
        selected, skipped = [], []
        for url in sorted(resources):
            if Path(urlparse(url).path).suffix not in (".js", ".css"):
                continue
            try:
                selected.append((url, source_path(url)))
            except ValueError as error:
                skipped.append({"url": url, "reason": str(error)})
        files = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=options.workers) as pool:
            jobs = {pool.submit(discovery.fetch_snapshot, url, path, path.suffix[1:], options): (url, path)
                    for url, path in selected}
            for future in concurrent.futures.as_completed(jobs):
                url, path = jobs[future]
                receipt = future.result()
                files.append({"url": url, "path": path.relative_to(ROOT).as_posix(),
                    "result": receipt["result"], "sha256": receipt["sha256"], "bytes": receipt["bytes"]})
        record = {"route": route, "evidence": evidence, "endpoints": endpoints,
            "files": sorted(files, key=lambda f: f["url"]), "skipped": skipped,
            "manifest_code_complete": html["result"] == "ok" and assets["result"] == "ok"
                and bool(files) and not skipped and all(f["result"] == "ok" for f in files)}
        records = [row for row in records if row["route"] != route] + [record]
        print(f"{route}: HTML {html['result']}, JS/CSS {sum(f['result'] == 'ok' for f in files)}/{len(files)}", flush=True)
        # Preserve finished applications if a later source is interrupted.
        discovery.write_json(ROOT / ".ref/discovery/applications.json", {"applications": records})
    records.sort(key=lambda row: row["route"])
    summary = {"routes": len(records), "html_found": sum(r["endpoints"]["index.html"]["result"] == "ok" for r in records),
        "manifest_code_complete": sum(r["manifest_code_complete"] for r in records),
        "code_results": dict(Counter(f["result"] for r in records for f in r["files"]))}
    result = {"schema_version": 1, "generated_at_utc": discovery.utc_now(), "summary": summary,
        "scope": "Source-defined route literals. Routes can share an app. Manifest completeness excludes dynamically composed resources and is not UI implementation coverage.",
        "applications": records}
    discovery.write_json(ROOT / ".ref/discovery/applications.json", result)
    discovery.write_json(ROOT / "docs/re/application-catalog.json", result)
    lines = ["# 独立应用与模块入口", "", "根据宿主、主前端和 Settings 的路径字面量逐项取得；未执行下载的 JS。路由不等于独立产品或已完成的界面。", "",
        "原始响应及哈希：[application-catalog.json](application-catalog.json)。源文件、偏移及 SHA-256 保留在各项 evidence 中。", "",
        "| 路由 | HTML | 清单中的 JS/CSS | JS/CSS 取得 |", "| --- | --- | --- | ---: |"]
    for row in records:
        lines.append(f"| [{row['route']}](https://apps.razer.com{row['route']}) | {row['endpoints']['index.html']['result']} | {'齐备' if row['manifest_code_complete'] else '待追踪'} | {sum(f['result'] == 'ok' for f in row['files'])}/{len(row['files'])} |")
    lines += ["", "没有 asset-manifest 的入口仅能确认 HTML 声明的脚本；其动态 import、条件路由和原生服务仍须追踪。404 仅代表记录时该端点不可用。", "",
        "`/rz-app-menu/` 来自主前端 `App.72827d47.chunk.js` 的 `${window.location.origin}/rz-app-menu/` 模板。`/feedback/` 来自当前 Dashboard、App Menu 和 Settings 中查询参数插值之前的固定路径。发现脚本只提取静态路径，不执行模板或下载的代码。`--routes` 仅准备已在源码登记的应用，保留其他已取得的目录记录。弹层结构、安装条件和 Alexa 启动路径见[更多应用规格](../screens/19-app-picker.md)。", ""]
    (ROOT / "docs/re/17-application-catalog.md").write_text("\n".join(lines), encoding="utf-8")
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
