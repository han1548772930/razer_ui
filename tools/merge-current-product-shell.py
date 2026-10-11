"""Merge independently reviewed current shell receipts, without executing vendor code.

Run audit-profile-menu-product-current.cjs and audit-product-shell-current.cjs
once per selected product first. The Help spacing adapter is deliberately
restricted to the four mounted callers independently reviewed in this batch.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PRODUCTS = (190, 678, 679, 688)


def read(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def write(path, data):
    (ROOT / path).write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


receipts = []
menus = read("crates/razer-pages/src/features/source_profile_menu_data.json")
by_id = {row["product_id"]: row for row in menus["products"]}
helps = read("crates/razer-pages/src/features/source_help_data.json")
for pid in PRODUCTS:
    evidence = read(f".work/product-shell-current/{pid}-evidence.json")
    menu = read(f".work/profile-menu-current/{pid}-data.json")
    source = evidence["source"]
    expected_prefix = (
        f"local-ui-reverse/source/official/apps.razer.com/synapse/products/{pid}/ui/"
    )
    assert source["path"].startswith(expected_prefix)
    for receipt in (source, evidence["css"], menu["source"]):
        assert receipt["path"].startswith(expected_prefix)
        assert hashlib.sha256((ROOT / receipt["path"]).read_bytes()).hexdigest() == receipt["sha256"]
    shell = evidence["profile_bar"]
    assert not shell["ancestor_conditions"]
    pages = shell["sync_exclusions"][0]["pages"]
    assert all(row["pages"] == pages for row in shell["sync_exclusions"])
    menu["profile_shell"] = dict(
        always_mounted=True,
        sync_excluded_pages=pages,
        evidence="docs/re/product-shell-current-evidence.json",
    )
    by_id[pid] = menu

    # These flags were independently re-audited in the current mounted caller,
    # defaultProps and connect map; do not derive them from product category.
    help_receipt = evidence["help"][0]
    assert len(evidence["help"]) == 1 and len(help_receipt["mounts"]) == 1
    assert help_receipt["default_props"]["value"] == {"resetTitle": "FACTORY_RESET_PROFILES"}
    info = evidence["device_info"]["value"]
    help_row = next(row for row in helps if row["product_id"] == pid)
    assert len(help_row["pages"]) == 1
    help_row.update(
        support=info.get("supportPage"), guide=info.get("masterGuideURL"),
        source=source["path"], evidence=source["sha256"],
    )
    page = help_row["pages"][0]
    page.update(
        source=source["path"], class_source=source["path"],
        class_offset=help_receipt["component"]["offset"],
        class_offset_unit=evidence["offset_unit"],
        mounted_component_offset=help_receipt["mounts"][0]["offset"],
        firmware=True, view_more=True, serial=True, registration=True,
        reset=True, oled_reset=False, reset_title="FACTORY_RESET_PROFILES",
        obm=info["isOBMDevice"], firmware_reset=None,
        system_info=False, tutorial=False, thx_instructions=False, camo=False,
        inline_confirmation=True, obm_reset_during_ble=False, plain_columns=True,
        spacing_evidence="docs/re/product-shell-current-evidence.json",
    )
    # Retain the navigation identity offset consumed by the existing catalog;
    # AST source addresses have a separately declared UTF-16 offset unit.
    receipts.append(evidence)

menus["products"] = sorted(by_id.values(), key=lambda row: row["product_id"])
menus["uncovered"] = [row for row in menus.get("uncovered", []) if row["product_id"] not in PRODUCTS]
write("crates/razer-pages/src/features/source_profile_menu_data.json", menus)
write("crates/razer-pages/src/features/source_help_data.json", helps)
write("docs/re/product-shell-current-evidence.json", dict(
    schema_version=1,
    method="Current per-product Acorn lexical/module resolution and static CSS parsing; vendor code never evaluated",
    products=receipts,
))
print("Merged source-verified profile shells and mounted Help callers:", list(PRODUCTS))
