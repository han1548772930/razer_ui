"""AST ownership for expression-bodied createElement SVG producers."""
import hashlib

import frontend
import host


def expression_svg(original_text, anchor, render_legacy):
    raw = original_text.encode("utf-8")
    parser = host.HostLocalAudit().parser
    tree = parser.parse(raw)
    if tree.root_node.has_error:
        raise ValueError("SVG original has invalid JavaScript syntax")
    declarations = [node for node in frontend.nodes(tree.root_node)
                    if node.type == "variable_declarator" and node.text.startswith(anchor.encode("utf-8"))]
    if len(declarations) != 1:
        raise ValueError("SVG expression producer is absent or ambiguous")
    declaration = declarations[0]
    value = frontend.unparenthesized(declaration.child_by_field_name("value"))
    if value.type != "arrow_function":
        raise ValueError("SVG expression producer must be an arrow function")
    expression = frontend.unparenthesized(value.child_by_field_name("body"))
    if expression.type != "call_expression":
        raise ValueError("SVG expression producer has no literal returned call")
    callee = expression.child_by_field_name("function")
    if (callee.type != "member_expression"
            or callee.child_by_field_name("property").text != b"createElement"):
        raise ValueError("SVG expression does not use createElement")
    call_name = callee.text

    def scalar(node):
        return node.type in {"string", "number"} or (
            node.type == "unary_expression" and node.children[0].type == "-"
            and node.named_children[-1].type == "number")

    def validate(node, *, root=False):
        node = frontend.unparenthesized(node)
        if node.type == "binary_expression" and node.child_by_field_name("operator").text == b"||":
            left = node.child_by_field_name("left")
            assignment = frontend.unparenthesized(node.child_by_field_name("right"))
            if (left.type != "identifier" or assignment.type != "assignment_expression"
                    or assignment.child_by_field_name("left").text != left.text):
                raise ValueError("SVG cache is not a literal element assignment")
            return validate(assignment.child_by_field_name("right"))
        if node.type == "ternary_expression":
            child = node.child_by_field_name("consequence")
            if (node.child_by_field_name("condition").type != "identifier"
                    or node.child_by_field_name("alternative").type != "null"
                    or child.type != "call_expression"
                    or child.child_by_field_name("function").text != call_name
                    or child.child_by_field_name("arguments").named_children[0].text != b'"title"'):
                raise ValueError("SVG has a conditional visual child")
            return
        if node.type != "call_expression" or node.child_by_field_name("function").text != call_name:
            raise ValueError("SVG child is not a literal createElement call")
        args = node.child_by_field_name("arguments").named_children
        if len(args) < 2 or args[0].type != "string":
            raise ValueError("SVG call has no literal tag and properties")
        props = args[1]
        merged = root and props.type == "call_expression"
        if merged:
            values = props.child_by_field_name("arguments").named_children
            if (props.child_by_field_name("function").type != "identifier" or len(values) != 2
                    or values[0].type != "object" or values[1].type != "identifier"):
                raise ValueError("SVG forwarded root properties are unresolved")
            props = values[0]
        if props.type not in {"object", "null"}:
            raise ValueError("SVG properties are not a literal object")
        for prop in props.named_children:
            if prop.type != "pair":
                raise ValueError("SVG property is not a literal pair")
            key, value = prop.child_by_field_name("key"), prop.child_by_field_name("value")
            if merged and key.text.strip(b'\"\'') in {b"ref", b"aria-labelledby"} and value.type == "identifier":
                continue
            if key.text == b"style" and value.type == "object":
                if all(p.type == "pair" and scalar(p.child_by_field_name("value")) for p in value.named_children):
                    continue
            if not scalar(value):
                raise ValueError("SVG attribute is not a scalar literal")
        for child in args[2:]:
            if child.type != "string":
                validate(child)

    validate(expression, root=True)
    # The existing restricted literal renderer owns XML escaping and React
    # attribute conversion. Only a validated AST expression is adapted here;
    # the synthetic function is never executed or recorded as an original.
    synthetic = "const Svg=()=>{return " + expression.text.decode("utf-8") + "}"
    content, receipt = render_legacy(synthetic, "Svg=()=>{")
    start = len(raw[:declaration.start_byte].decode("utf-8"))
    body = declaration.text.decode("utf-8")
    receipt["source_regions"] = [{"character_start": start, "character_end": start + len(body),
                                  "body": body, "body_sha256": hashlib.sha256(declaration.text).hexdigest()}]
    receipt["module"] = {"syntax": declaration.type, "byte_start": declaration.start_byte,
                         "byte_end": declaration.end_byte, "identity_scope": "complete_ast_producer"}
    receipt["tool"] = "tree-sitter JavaScript AST and existing restricted XML literal renderer"
    return content, receipt
