"""Check the ownership of resource caches across GPUI overlay boundaries."""
from __future__ import annotations

import hashlib
from pathlib import Path
import sys


OWNER = "crates/razer-assets/src/image_cache/scope.rs"
_COMMENTS = {"line_comment", "block_comment"}

# GPUI's default `img(Resource::Embedded(..))` path is intentionally owned by
# the per-window ResourceImageCache.  These APIs bypass that owner and are
# therefore only valid for the cache implementation itself or for the one
# local user-import path which has its own keyed lease and release cleanup.
_ASSET_OWNER_PREFIX = "crates/razer-assets/src/"
_LOCAL_IMPORTED_ASSET = "crates/razer-pages/src/features/workspace/source_controls/oled_crop.rs"
_SHARED_CUSTOM_IMAGE = "crates/razer-app-pages/src/services/module_service_rows.rs"


def parser(root):
    sys.path.insert(0, str(root / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_rust
    return Parser(Language(tree_sitter_rust.language()))


def inspect_source(raw, *, owner, rust_parser):
    """Inspect calls structurally, retaining original byte offsets and ownership."""
    tree = rust_parser.parse(raw)
    offset = 0
    if tree.root_node.has_error:
        prefix = b"const RESOURCE_SCOPE_EXPRESSION: () = "
        tree = rust_parser.parse(prefix + raw + b";")
        if tree.root_node.has_error:
            return {"edges": [], "violations": [{"owner": owner, "kind": "rust_parse_error"}],
                    "test_edges": 0}
        raw, offset = prefix + raw + b";", len(prefix)

    aliases = {name: name for name in ("deferred", "deferred_resource", "Popup", "Popover", "Dialog", "BasePopup", "BaseDialog",
                                      "resource_scope", "ResourceLayer")}
    edges, violations, asset_edges, tests = [], [], [], 0

    def text(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8") if node is not None else ""

    def children(node):
        return [child for child in node.named_children if child.type not in _COMMENTS]

    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)

    for node in walk(tree.root_node):
        if node.type == "use_as_clause":
            original = text(node.child_by_field_name("path")).split("::")[-1]
            if original in aliases:
                aliases[text(node.child_by_field_name("alias"))] = aliases[original]

    def method(node):
        if node.type != "call_expression":
            return None
        function = node.child_by_field_name("function")
        if function is None or function.type != "field_expression":
            return None
        return (text(function.child_by_field_name("field")),
                function.child_by_field_name("value"), children(node.child_by_field_name("arguments")))

    def receiver_root(node):
        while (member := method(node)) is not None:
            node = member[1]
        return node

    def constructor(node):
        node = receiver_root(node)
        if node.type != "call_expression":
            return ""
        function = text(node.child_by_field_name("function")).split("::")
        if len(function) < 2 or function[-1] != "new":
            return ""
        return aliases.get(function[-2], "")

    def scoped(node):
        if node.type == "block":
            values = children(node)
            return bool(values) and values[-1].type != "expression_statement" and scoped(values[-1])
        if node.type == "parenthesized_expression":
            return len(children(node)) == 1 and scoped(children(node)[0])
        if node.type != "call_expression":
            return False
        function = text(node.child_by_field_name("function")).split("::")[-1]
        return aliases.get(function) == "resource_scope"

    def early_returns(node):
        # A nested closure/function owns its own returns.
        if node.type in ("closure_expression", "function_item", "async_block"):
            return
        if node.type == "return_expression":
            yield node
        for child in node.named_children:
            yield from early_returns(child)

    def context(node):
        while node is not None:
            if node.type == "function_item":
                return {"name": text(node.child_by_field_name("name")),
                        "byte_start": node.start_byte - offset, "byte_end": node.end_byte - offset,
                        "sha256": hashlib.sha256(raw[node.start_byte:node.end_byte]).hexdigest()}
            node = node.parent
        return None

    def edge(node, kind, valid, detail=None):
        row = {"owner": owner, "line": raw[:node.start_byte].count(b"\n") + 1,
               "byte_start": node.start_byte - offset, "byte_end": node.end_byte - offset,
               "kind": kind, "resource_owner": OWNER, "valid": valid,
               "function": context(node)}
        if detail:
            row["detail"] = detail
        edges.append(row)
        if not valid:
            violations.append(row)

    def asset_edge(node, kind, valid, detail=None):
        row = {"owner": owner, "line": raw[:node.start_byte].count(b"\n") + 1,
               "byte_start": node.start_byte - offset, "byte_end": node.end_byte - offset,
               "kind": kind, "resource_owner": OWNER, "valid": valid,
               "function": context(node)}
        if detail:
            row["detail"] = detail
        asset_edges.append(row)
        if not valid:
            violations.append(row)

    def is_test(node):
        path = Path(owner)
        if path.stem in ("test", "tests") or path.stem.endswith(("_test", "_tests")) or any(part in ("tests", "testdata") for part in path.parts):
            return True
        while node is not None:
            sibling = node.prev_named_sibling
            while sibling is not None and sibling.type in (*_COMMENTS, "attribute_item"):
                if sibling.type == "attribute_item":
                    attribute = "".join(text(sibling).split())
                    if attribute.startswith(("#[test]", "#[gpui_kit::test]")) or (
                            attribute == "#[cfg(test)]" or attribute.startswith("#[cfg(all(test,")):
                        return True
                sibling = sibling.prev_named_sibling
            node = node.parent
        return False

    for node in walk(tree.root_node):
        if node.type != "call_expression":
            continue
        member = method(node)
        function = node.child_by_field_name("function")
        name = text(function).split("::")[-1] if member is None else member[0]
        if is_test(node):
            if name in ("deferred", "defer_draw", "popup", "content", "open_dialog", "open_sheet"):
                tests += 1
            continue

        # `Window::use_asset` is the GPUI asset-cache escape hatch.  The
        # resource owner may use it to implement the shared cache, while the
        # OLED cropper uses a separate keyed local-data lease.  Every other
        # production call would reintroduce the sticky GPUI AssetLogger cache
        # that this guard is intended to prevent.
        if function is not None and function.type == "generic_function":
            base = function.child_by_field_name("function")
            base_text = text(base)
            if base_text.split(".")[-1] == "use_asset":
                type_arguments = function.child_by_field_name("type_arguments")
                type_text = text(type_arguments) if type_arguments is not None else ""
                asset_type = type_text.removeprefix("<").removesuffix(">").strip()
                if owner.startswith(_ASSET_OWNER_PREFIX):
                    valid, detail = True, "resource-owner implementation"
                elif owner == _LOCAL_IMPORTED_ASSET and asset_type == "ImportedImageAsset":
                    valid, detail = True, "keyed local-data import with release cleanup"
                else:
                    valid, detail = False, (
                        f"direct use_asset::<{asset_type or '?'}> must use load_resource_image"
                    )
                asset_edge(node, "direct_use_asset", valid, detail)

        # `Svg::external_path` invokes gpui-fast's SvgAsset loader, whose
        # completed error is cached by GPUI.  The application has no valid
        # external-path owner; keeping this check explicit prevents a future
        # remote alias from silently bypassing ResourceImageCache.  Ordinary
        # `svg().path(...)` intentionally remains allowed: gpui-fast resolves
        # it through the configured AssetSource and runtime refresh events.
        if member is not None and member[0] == "external_path":
            asset_edge(node, "external_svg_asset", owner.startswith(_ASSET_OWNER_PREFIX),
                       "external_path uses gpui-fast SvgAsset; use svg().path or the shared loader")

        # Custom image sources are valid only when their closure is visibly
        # connected to the shared loader, or when they are the documented OLED
        # local import.  A file-level helper check is deliberately conservative
        # and keeps this guard independent of macro/type expansion.
        if function is not None and text(function).endswith("ImageSource::Custom"):
            if owner == _LOCAL_IMPORTED_ASSET and b"ImportedImageAsset" in raw:
                valid, detail = True, "keyed local-data import"
            elif owner == _SHARED_CUSTOM_IMAGE and b"load_resource_image" in raw:
                valid, detail = True, "shared per-window resource loader"
            else:
                valid, detail = False, "custom image source must call load_resource_image"
            asset_edge(node, "custom_image_source", valid, detail)

        if member is None and aliases.get(name) == "deferred":
            edge(node, "raw_deferred", owner == OWNER)
        if member is None and aliases.get(name) == "deferred_resource":
            edge(node, "resource_deferred", True)
        if member is None and constructor(node) == "ResourceLayer":
            edge(node, "retained_resource_layer", True)
        if member is None:
            continue
        name, receiver, arguments = member
        receiver_name = text(receiver)
        if name == "defer_draw":
            edge(node, "custom_deferred_draw", owner == OWNER)
        if name == "popup" and constructor(receiver) in ("Dialog", "BaseDialog") and len(arguments) == 1:
            edge(node, "dialog_popup", scoped(arguments[0]))
        if name == "content" and constructor(receiver) in ("Popup", "BasePopup", "Popover"):
            if len(arguments) != 1 or arguments[0].type != "closure_expression":
                edge(node, "overlay_content", False, "Overlay content must have an inspectable closure")
                continue
            body = arguments[0].child_by_field_name("body")
            edge(node, "overlay_content", scoped(body))
            for early in early_returns(body):
                values = children(early)
                edge(early, "overlay_early_return", len(values) == 1 and scoped(values[0]))
        if name in ("open_dialog", "open_alert_dialog", "open_sheet") and arguments:
            callback = arguments[-1]
            if callback.type != "closure_expression":
                edge(node, "window_overlay_callback", False, "Callback requires an explicit static audit")
                continue
            parameters = children(callback.child_by_field_name("parameters"))
            if not parameters:
                edge(node, "window_overlay_callback", False, "Missing overlay builder parameter")
                continue
            builder = text(parameters[0])
            for child in walk(callback.child_by_field_name("body")):
                part = method(child)
                if part is None:
                    continue
                part_name, part_receiver, values = part
                if text(receiver_root(part_receiver)) != builder:
                    continue
                if part_name in ("child", "title", "footer", "description") and len(values) == 1:
                    edge(child, "window_overlay_content", scoped(values[0]))
                elif part_name == "children":
                    edge(child, "window_overlay_collection", False, "Scope every collection element explicitly")

    # Macro token trees do not have expression ASTs. Fail visibly for overlay
    # syntax there rather than pretending those expansions were inspected.
    candidates = {name for name, original in aliases.items()
                  if original in ("deferred", "Popup", "BasePopup", "Popover", "Dialog", "BaseDialog")}
    candidates.add("defer_draw")
    for node in walk(tree.root_node):
        if node.type not in ("macro_definition", "macro_invocation") or is_test(node):
            continue
        tokens = [n for n in walk(node) if n.type == "identifier" and text(n) in candidates]
        if tokens:
            edge(node, "overlay_macro_requires_audit", False, sorted(set(text(n) for n in tokens)))
    return {"edges": edges, "asset_edges": asset_edges, "violations": violations,
            "test_edges": tests}


def audit_sources(root, sources=None):
    root = Path(root).resolve()
    rust_parser = parser(root)
    if sources is None:
        sources = sorted({*root.glob("app/**/*.rs"), *root.glob("crates/**/*.rs")})
    report = {"owner": OWNER, "inputs": [], "edges": [], "asset_edges": [], "violations": [],
              "test_edges": 0,
              "scope": "Rust call AST, overlay ownership, and image-loader bypass diagnostics",
              "limitations": ["No Rust type checking or macro expansion; compilation remains separate",
                              "Indirect callback or builder indirection requires an explicit audit"],
              "target_execution": False}
    for path in sources:
        path = Path(path).resolve()
        if not path.is_relative_to(root):
            raise ValueError("Resource scope source is outside the workspace")
        raw = path.read_bytes()
        name = path.relative_to(root).as_posix()
        report["inputs"].append({"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        checked = inspect_source(raw, owner=name, rust_parser=rust_parser)
        report["edges"].extend(checked["edges"])
        report["asset_edges"].extend(checked["asset_edges"])
        report["violations"].extend(checked["violations"])
        report["test_edges"] += checked["test_edges"]
    report["inputs_unchanged"] = all(hashlib.sha256((root / row["path"]).read_bytes()).hexdigest() == row["sha256"]
                                     for row in report["inputs"])
    report["passed"] = report["inputs_unchanged"] and not report["violations"]
    report["summary"] = {"source_files": len(report["inputs"]), "edges": len(report["edges"]),
                         "asset_edges": len(report["asset_edges"]),
                         "violations": len(report["violations"]), "test_edges": report["test_edges"]}
    return report


if __name__ == "__main__":
    import argparse
    import json

    command = argparse.ArgumentParser(description="Audit GPUI resource ownership and image-loader bypasses")
    command.add_argument("root", nargs="?", default=Path(__file__).resolve().parent.parent,
                         type=Path, help="workspace root (defaults to the repository containing tools/)")
    command.add_argument("--json", action="store_true", help="emit the complete machine-readable report")
    args = command.parse_args()
    result = audit_sources(args.root)
    payload = result if args.json else {"passed": result["passed"], "summary": result["summary"],
                                        "violations": result["violations"]}
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    raise SystemExit(0 if result["passed"] else 1)
