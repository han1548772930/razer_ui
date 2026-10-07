"""Publish compact, reviewable discovery evidence from saved source snapshots."""
from __future__ import annotations

import hashlib
import json
from collections import Counter
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]
DISCOVERY = ROOT / ".ref/discovery"


def implementation(pid):
    return "not_assessed_by_source_catalog"


def read(path, fallback=None):
    return json.loads(path.read_text(encoding="utf-8-sig")) if path.is_file() else fallback


def display_name(product):
    for name in product["names"]:
        value = name["value"]
        if isinstance(value, dict):
            value = value.get("en") or value.get("zh-cn")
        if isinstance(value, str) and value:
            return value
    for version in product["versions"].values():
        if version.get("short_name"):
            return version["short_name"]
    return "未提供名称"


def escape(value):
    return str(value).replace("|", "\\|").replace("\n", " ")


def navigation_name(item):
    name = item["name"]
    value = name.get("value")
    return str(value) if value is not None else name.get("expression") or "未解析名称"


def main():
    discovery = read(DISCOVERY / "products.json")
    interfaces = {p["product_id"]: p for p in read(DISCOVERY / "interfaces.json", {"products": []})["products"]}
    records, families = [], Counter()
    for product in discovery["products"]:
        pid = product["product_id"]
        code_files = [r for r in product["resource_entries"] if urlparse(r["url"]).path.endswith((".js", ".css"))]
        acquired = 0
        for resource in code_files:
            url = urlparse(resource["url"])
            prefix = f"/synapse/products/{pid}/ui/"
            if not url.path.startswith(prefix):
                continue
            path = ROOT / ".ref/devices" / str(pid) / url.path[len(prefix):]
            receipt = read(path.with_name(path.name + ".http.json"), {})
            if (receipt.get("result") == "ok" and receipt.get("source_url") == resource["url"]
                    and path.is_file() and path.stat().st_size == receipt.get("bytes")
                    and hashlib.sha256(path.read_bytes()).hexdigest() == receipt.get("sha256")):
                acquired += 1
        navigations = []
        source_files = interfaces.get(pid, {}).get("files", [])
        source_paths = {source["path"]: source for source in source_files}
        def ancestry(source, seen=None):
            seen = set() if seen is None else seen
            parent = source.get("reachable_from")
            if not parent or parent["path"] in seen:
                return parent
            seen.add(parent["path"])
            return {**parent, "parent": ancestry(source_paths.get(parent["path"], {}), seen)}
        for source in source_files:
            for navigation in source.get("mounted_navigation", []):
                navigations.append({"source": source["path"], "sha256": source["sha256"],
                    "owner": navigation["owner"], "offset": navigation["navs_offset"],
                    "items": navigation["items"],
                    "evidence": navigation.get("evidence"), "owner_offset": navigation.get("class_offset"),
                    "usage_offset": navigation.get("state_offset"),
                    "reachable_from": ancestry(source),
                    "display_mode": source.get("reachable_from", {}).get("display_mode", "entry")})
        names = tuple(tuple(navigation_name(item)
                            for item in nav["items"]) for nav in navigations)
        if names:
            families[names] += 1
        records.append({"product_id": pid, "name": display_name(product),
            "catalogs": sorted({row["catalog"] for row in product["catalog_sources"]}),
            "catalog_entries": product["catalog_sources"], "edition_ids": product["edition_ids"],
            "connection_aliases": product["connection_aliases"], "referenced_by": product.get("referenced_by", []),
            "categories": sorted({r["category"] for r in product["classifications"]}),
            "endpoints": {key: {k: value.get(k) for k in ("source_url", "final_url", "http_status",
                "result", "fetched_at_utc", "sha256")} for key, value in product["endpoints"].items()},
            "ui_entry_found": product["ui_exists"], "code_files_expected": len(code_files),
            "code_files_acquired": acquired, "code_complete": bool(code_files) and acquired == len(code_files),
            "navigation": navigations,
            "implementation": implementation(pid)})
    summary = {**discovery["summary"], "code_complete_products": sum(p["code_complete"] for p in records),
        "code_files_acquired": sum(p["code_files_acquired"] for p in records),
        "products_with_navigation_arrays": sum(bool(p["navigation"]) for p in records),
        "navigation_signatures": len(families),
        "unresolved_navigation_names": sum(item["name"].get("value") is None
            for product in records for nav in product["navigation"] for item in nav["items"])}
    report = {"schema_version": 1, "date": "2026-10-02", "summary": summary,
        "source_bundle": discovery["source_bundle"], "catalog_snapshots": discovery["catalog_snapshots"],
        "source_inventory_sha256": hashlib.sha256((DISCOVERY / "products.json").read_bytes()).hexdigest(),
        "scope": "Source-defined catalogs, historical responses and explicit aliases/manifest declarations. Navigation signatures are not interchangeable product capabilities or proof of all conditional screens.",
        "products": records}
    target = ROOT / "docs/re/product-catalog.json"
    target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    text = ["# 产品源码目录", "",
        "本目录仅记录官方来源、别名、入口及已取得的代码，不能推算本地 UI 完成度。", "",
        "[产品源码记录](product-catalog.json)保留各目录、manifest、连接别名与实际挂载位置；[产品注册](product-registration-audit.md)说明当前可用产品身份。", "",
        "本地实现状态单独由 [页面覆盖](native-product-coverage.md)与[当前缺口](remaining-ui-work.md)维护，不使用固定型号白名单判断已适配。", "",
        "维护工具为 `tools/report-razer-discovery.py`，只读取已保存的源元数据。取得源码、成功解析、路由存在和实际界面完成是不同事项。", ""]
    (ROOT / "docs/re/16-product-catalog.md").write_text("\n".join(text), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == "__main__":
    main()
