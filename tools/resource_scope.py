"""Check the ownership of resource caches across GPUI overlay boundaries."""
from __future__ import annotations

import hashlib
from pathlib import Path
import sys


OWNER = "crates/razer-assets/src/remote/image_cache/scope.rs"
_COMMENTS = {"line_comment", "block_comment"}


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
    edges, violations, tests = [], [], 0

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

    def is_test(node):
        path = Path(owner)
        if path.stem.endswith(("_test", "_tests")) or any(part in ("tests", "testdata") for part in path.parts):
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
    return {"edges": edges, "violations": violations, "test_edges": tests}


def audit_sources(root, sources=None):
    root = Path(root).resolve()
    rust_parser = parser(root)
    if sources is None:
        sources = sorted({*root.glob("app/**/*.rs"), *root.glob("crates/**/*.rs")})
    report = {"owner": OWNER, "inputs": [], "edges": [], "violations": [], "test_edges": 0,
              "scope": "Rust call AST, import aliases, inline overlay callbacks and explicit macro diagnostics",
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
        report["violations"].extend(checked["violations"])
        report["test_edges"] += checked["test_edges"]
    report["inputs_unchanged"] = all(hashlib.sha256((root / row["path"]).read_bytes()).hexdigest() == row["sha256"]
                                     for row in report["inputs"])
    report["passed"] = report["inputs_unchanged"] and not report["violations"]
    report["summary"] = {"source_files": len(report["inputs"]), "edges": len(report["edges"]),
                         "violations": len(report["violations"]), "test_edges": report["test_edges"]}
    return report
