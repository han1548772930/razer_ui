"""Statically validate the refactored feature capabilities; never run the app or tests."""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
FEATURES = ROOT / "src/features"


def require(condition, message):
    if not condition:
        raise SystemExit(message)


numbered = [path.name for path in FEATURES.rglob("*.rs") if re.search(r"_\d+(?:_|\.)", path.name)]
require(not numbered, f"Product-numbered implementation modules remain: {numbered}")
for path in FEATURES.rglob("*.rs"):
    text = path.read_text(encoding="utf-8")
    require(not re.search(r'(?:path\s*=|include_str!\()\s*"mouse_\d+_', text),
            f"Obsolete single-product include: {path.relative_to(ROOT)}")

refactored = ["mouse_products.rs", "mouse_scroll_wheel.rs", "mouse_dpi_grid.rs",
              "mouse_dpi_rows.rs", "mouse_properties.rs", "macro_inputs.rs"]
for name in refactored:
    source = (FEATURES / name).read_text(encoding="utf-8")
    # Identity lookup against data remains valid. Product-number comparisons in
    # this shared production layer must move to a reviewed capability strategy.
    production = source.split("#[cfg(test)]\nmod tests", 1)[0]
    require(not re.search(r"\b(?:product_id|pid)\s*(?:==|!=)\s*\d", production),
            f"Literal product-number branch: {name}")
    require(not re.search(r"matches!\([^\n,]*(?:product_id|pid)\s*,\s*\d", production),
            f"Literal product-number match: {name}")

products = {int(pid) for pid in re.findall(r"RegisteredProduct \{ id: (\d+)",
            (ROOT / "crates/razer-catalog/src/registry_data.rs").read_text(encoding="utf-8"))}
catalog_names = ["mouse_scroll_wheel_data.json", "mouse_dpi_grid_data.json",
                 "mouse_dpi_rows_data.json", "mouse_properties_data.json"]
count = 0
for name in catalog_names:
    specs = json.loads((FEATURES / name).read_text(encoding="utf-8"))
    ids = [spec["product_id"] for spec in specs]
    require(len(ids) == len(set(ids)) and set(ids) <= products, f"Invalid capability identities: {name}")
    count += len(ids)
    for spec in specs:
        if name == "mouse_scroll_wheel_data.json":
            modes = [mode["id"] for mode in spec["modes"]]
            require(len(modes) == len(set(modes)), "Duplicate scroll modes")
            presentation = spec.get("presentation", "three_modes")
            require(presentation in ["three_modes", "two_mode_levels", "two_mode_switches"],
                    "Unknown scroll presentation")
            require((not spec["locking_mode"] or spec["locking_mode"] in modes)
                    and spec["defaults"]["scrollMode"] in modes,
                    "Scroll lock/default missing from modes")
            require(0 <= spec["max_disabled_modes"] < len(modes), "Invalid disabled-mode limit")
            if presentation != "three_modes":
                require(len(modes) == 2 and not spec["locking_mode"]
                        and spec["max_disabled_modes"] == 0
                        and "disabledModes" not in spec["defaults"], "Invalid two-mode controls")
                require("two_mode_style" in spec, "Missing source two-mode CSS parameters")
            if presentation == "two_mode_switches":
                require(all(spec[key] == 0 for key in ["level_min", "level_max", "level_step"])
                        and all(key not in spec["defaults"] for key in
                                ["accelerationLevel", "smartReelLevel"]), "Unexpected legacy levels")
            else:
                require(0 <= spec["level_min"] < spec["level_max"] <= 255 and spec["level_step"] > 0,
                        "Invalid scroll level range")
                for key in ["accelerationLevel", "smartReelLevel"]:
                    require(spec["level_min"] <= spec["defaults"][key] <= spec["level_max"],
                            f"Out-of-range default: {key}")
        if name == "mouse_dpi_rows_data.json":
            profile = next(row["profile"] for row in json.loads(
                (FEATURES / "mouse_products_data.json").read_text(encoding="utf-8"))
                if row["product_id"] == spec["product_id"])
            for path_key in ["stages_path", "active_path", "enabled_path"]:
                value = profile
                for key in spec[path_key].strip("/").split("/"):
                    require(key in value, f"Unknown DPI profile field: {spec[path_key]}")
                    value = value[key]
                if path_key == "stages_path":
                    for stage in value:
                        require(all(spec[key] in stage for key in
                                    ["x_key", "y_key", "independent_key", "visible_key"]),
                                "DPI stage capability does not match stored profile")
        if name == "mouse_dpi_grid_data.json":
            segments = spec["segments"]
            require(segments and segments[0]["from"] == 0 and segments[-1]["to"] == 100,
                    "DPI segments must cover the percentage domain")
            require(segments[0]["fromValue"] == spec["min"] and segments[-1]["toValue"] == spec["max"],
                    "DPI segments must cover the product range")
            require(all(a["to"] == b["from"] and a["toValue"] == b["fromValue"]
                        for a, b in zip(segments, segments[1:])), "Discontinuous DPI segments")

print(f"Feature capabilities: no numbered Rust modules or literal PID branches in {len(refactored)} refactored files; {count} entries validated.")
