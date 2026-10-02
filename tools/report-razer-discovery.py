"""Publish compact, reviewable discovery evidence from saved source snapshots."""
from __future__ import annotations

import hashlib
import json
from collections import Counter
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]
DISCOVERY = ROOT / ".ref/discovery"


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
            "implementation": "已适配，服务及部分条件界面仍有缺口" if pid in (182, 653, 777) else "未适配"})
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
    text = ["# 官方目录、连接别名与产品界面清单", "", "更新日期：2026-10-02。由保存的 HTTP 响应、资源清单和入口 AST 生成；没有执行下载的 JS、驱动或安装器。", "",
        f"六份官方目录合计 {summary['official_primary_ids']} 个主产品 ID，与旧记录合并为 {summary['seed_product_ids']} 个主候选；沿连接别名及 manifest 中的明确声明共检查 {summary['fetched_product_ids']} 个 ID。其中 {summary['ui_html_found']} 个有 UI 入口。别名仍保留父产品关系，不计为独立型号。", "",
        f"已取得 {summary['code_files_acquired']} 份清单所列 JS/CSS，{summary['code_complete_products']} 个产品清单齐备；{summary['products_with_navigation_arrays']} 个产品找到明确写入 state.navs 或作为 JSX navs 传入的数组，包括入口、displayMode 和 setupStatus 延迟根组件。记录保留每条解析依据、条件表达式、调用偏移、根入口和 chunk/module ID；Babel 类及 useMemo 分支也按实际绑定追踪。未解析的名称和条件界面继续保留，不能按产品类别猜页面。", "",
        "机器可读记录：[product-catalog.json](product-catalog.json)。原始响应及错误体在 `.ref/discovery`，代码在 `.ref/devices`。资源哈希是本次下载指纹，不是发布方数字签名。", "",
        "## 来源", "", "| 目录 | 原始条目数 | HTTP | SHA-256 |", "| --- | ---: | ---: | --- |"]
    for name, receipt in sorted(discovery["catalog_snapshots"].items()):
        rows = read(DISCOVERY / "catalogs" / name, [])
        text.append(f"| [{name}]({receipt['source_url']}) | {len(rows)} | {receipt['http_status']} | `{receipt['sha256']}` |")
    text += ["", "## 全部候选及实际入口", "", "同一导航组合不表示控件、弹窗、服务、配色或布局相同。下表只列明确挂载的导航；附属界面和业务完成度仍以逐页审计为准。404 表示记录时此路径不可用，不表示产品不存在。", "",
        "| ID | 原名称 | 来源目录／别名 | UI | JS/CSS | 实际导航 | Rust |", "| ---: | --- | --- | --- | ---: | --- | --- |"]
    for row in records:
        navigation = "；".join(" / ".join(navigation_name(item) for item in nav["items"]) for nav in row["navigation"])
        source = ", ".join(n.replace("Devices.json", "").replace(".json", "") for n in row["catalogs"])
        if not source:
            source = "连接／声明引用" if row["referenced_by"] else "历史探测"
        ui = "有入口" if row["ui_entry_found"] else row["endpoints"]["ui/index.html"]["result"]
        text.append(f"| {row['product_id']} | {escape(row['name'])} | {escape(source)} | {ui} | {row['code_files_acquired']}/{row['code_files_expected']} | {escape(navigation or '待追踪' if row['ui_entry_found'] else '—')} | {'已有适配' if row['product_id'] in (182, 653, 777) else '未适配'} |")
    text += ["", "## 尚未完成", "", f"- 导航中仍有 {summary['unresolved_navigation_names']} 个名称表达式未化简；保留原表达式与源码偏移。条件弹窗、子应用和各显示模式继续逐项追踪。", "- 此表不是应用实现清单；目前 Rust 产品适配仍只有 182/653/777，不能把下载完成计作功能完成。", "- 图像、视频、字体、source map 和原生服务不在 JS/CSS 齐备统计内。", "- 目录是已记录版本的官方来源，不能证明未来或未公开产品的完整性。", ""]
    (ROOT / "docs/re/16-product-catalog.md").write_text("\n".join(text), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == "__main__":
    main()
