"""Scope inventory only. Does not turn historical implementation counts into review passes."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
source = ROOT / "docs/re/native-product-coverage.json"
catalog = json.loads(source.read_text(encoding="utf-8"))
reviewed = {
    (164, "TAB_CUSTOMIZE"): "pairing observations/local intents and source callbacks reviewed; partial",
    (164, "TAB_LIGHTING"): "normal quick-effects branch/brightness/idle subtree reviewed; missing effect editors",
    (164, "HELP"): "current reset confirmation reviewed and local intent repaired; partial Help",
    (179, "TAB_CUSTOMIZE"): "pairing operation generations/progress/cancel reviewed; parent conditions remain",
    (179, "HELP"): "current reset confirmation reviewed and local intent repaired; partial Help",
    (241, "TAB_PAIRING"): "pairing observations/local intents and source callbacks reviewed; partial",
    (241, "TAB_LIGHTING"): "normal quick-effects branch/brightness/idle subtree reviewed; missing effect editors",
    (241, "HELP"): "current reset confirmation reviewed and local intent repaired; partial Help",
    (3858, "TAB_GAMING"): "gaming presets/numeric/discrete slider events reviewed; disabled wrapper and live observations remain",
    (3880, "TAB_GAMING"): "extended gamma/gamut and preset editing reviewed; disabled wrapper/reducer combinations remain",
    (3858, "TAB_COLOR"): "color preset/RGB branches and source sync mount reviewed; observation and disabled wrapper remain",
    (3880, "TAB_COLOR"): "color profile selection/restrictions/path match repaired; observations and complete dropdown geometry remain",
    (3858, "TAB_DISPLAY"): "source icons/confirmation/PIP/FPS/Adaptive Sync reviewed and repaired; loading/global outside click remain",
    (3880, "TAB_DISPLAY"): "input assets/refresh rate restriction/local intentions/FPS repaired; live observation and exact layout remain",
}
products = []
for product in catalog["products"]:
    if product["family"] not in {"source_controls", "system", "accessory_system"}:
        continue
    pages = []
    for page in product["pages"]:
        record = {key: page[key] for key in ("page_id", "key", "offset", "route") if key in page}
        scope = reviewed.get((product["product_id"], page["key"]))
        record["review_status"] = "partial_subtree_review" if scope else "pending_review"
        record["review_scope"] = scope or "Inventory only; current source and implemented UI still require independent review."
        if scope:
            record["report"] = (
                "docs/re/monitor-ui-current.md"
                if product["product_id"] in {3858, 3880}
                else "docs/re/receiver-ui-current.md"
            )
        pages.append(record)
    products.append({
        "product_id": product["product_id"], "name": product["name"], "family": product["family"],
        "pages": pages,
        "independent_modes": [dict(mode, review_status="pending_review") for mode in product.get("independent_modes", [])],
    })
result = {
    "date": "2026-10-07",
    "method": "Assigned non-audio group inventory, with only explicitly reviewed subtrees marked partial; no full UI passes.",
    "inventory": {"path": str(source.relative_to(ROOT)).replace("\\", "/"), "sha256": hashlib.sha256(source.read_bytes()).hexdigest()},
    "products": products,
    "summary": {"products": len(products), "pages": sum(len(p["pages"]) for p in products),
        "partial_subtree_reviews": sum(p["review_status"] == "partial_subtree_review" for product in products for p in product["pages"]),
        "complete_ui_reviews": 0},
}
target = ROOT / "docs/re/accessory-system-review-queue-2026-10-07.json"
content = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
if "--check" in sys.argv:
    assert target.read_text(encoding="utf-8") == content, "Stale group queue"
else:
    target.write_text(content, encoding="utf-8")
print(result["summary"])
