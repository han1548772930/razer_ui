"""Compare complete JS ASTs with explicit local-binding correspondence.

This is a review aid, not a behavioral-equivalence test. Import substitutions
remain separate evidence obligations; globals, property keys and literal values
are never silently renamed. Existing lexical resolution owns scope handling.
"""
from dataclasses import dataclass
import hashlib

import evidence
import frontend


@dataclass
class SourceContext:
    raw: bytes
    tree: object
    root: object
    bindings: object
    offset: int = 0

    def select(self, start_byte, end_byte):
        """Select one complete owner while retaining its lexical environment."""
        if (type(start_byte) is not int or type(end_byte) is not int
                or not 0 <= start_byte < end_byte <= len(self.raw) - 2 * self.offset):
            raise ValueError("Comparison selection is outside the supplied source")
        start, end = start_byte + self.offset, end_byte + self.offset
        node = self.tree.root_node.descendant_for_byte_range(start, end - 1)
        if (node.start_byte, node.end_byte) != (start, end):
            raise ValueError("Comparison selection is not one complete AST node")
        return SourceContext(self.raw, self.tree, node, self.bindings, self.offset)

    @classmethod
    def parse(cls, parser, raw, *, factory=False):
        source = b"(" + raw + b")" if factory else raw
        tree = parser.parse(source)
        if tree.root_node.has_error:
            raise ValueError("Comparison input must be a complete valid JavaScript AST")
        root = tree.root_node
        if factory:
            if len(root.named_children) != 1:
                raise ValueError("Factory input has multiple roots")
            root = root.named_children[0].named_children[0]
            root = frontend.unparenthesized(root)
            if root.type not in {"arrow_function", "function_expression"}:
                raise ValueError("Factory input is not a complete function value")
        return cls(source, tree, root, evidence.JavascriptBindings(source, tree), int(factory))


def _key(node):
    return node.start_byte, node.end_byte


def _public_name(node):
    """Identifier tokens that also expose a JavaScript property/export name."""
    parent = node.parent
    if parent is None:
        return False
    if parent.type == "export_specifier":
        alias = parent.child_by_field_name("alias")
        return alias is None or node == alias
    if parent.type in {"class_declaration", "function_declaration"}:
        return (node == parent.child_by_field_name("name") and parent.parent is not None
                and parent.parent.type == "export_statement")
    cursor = parent
    while cursor is not None and cursor.type in {
            "object_pattern", "array_pattern", "pair_pattern", "assignment_pattern", "rest_pattern"}:
        cursor = cursor.parent
    if cursor is not None and cursor.type == "variable_declarator":
        binding = cursor.child_by_field_name("name")
        declaration = cursor.parent
        return (binding is not None and binding.start_byte <= node.start_byte < node.end_byte <= binding.end_byte
                and declaration is not None and declaration.parent is not None
                and declaration.parent.type == "export_statement")
    return False


def _brief(context, node):
    result = {"syntax": node.type, "start_byte": node.start_byte - context.offset,
              "end_byte": node.end_byte - context.offset,
              "sha256": hashlib.sha256(node.text).hexdigest()}
    if len(node.text) <= 160:
        result["token"] = node.text.decode("utf-8")
    return result


def _require_parameter(context):
    root = context.root
    parameters = root.child_by_field_name("parameters")
    if root.type not in {"arrow_function", "function_expression"} or parameters is None:
        return None
    parameters = [p for p in parameters.named_children if p.type != "comment"]
    return parameters[2] if len(parameters) == 3 and parameters[2].type == "identifier" else None


def _dependency_id(context, node):
    if node.type not in {"number", "string"}:
        return None
    arguments = node.parent
    call = arguments.parent if arguments is not None else None
    if (arguments is None or arguments.type != "arguments" or arguments.named_children != [node]
            or call is None or call.type != "call_expression"):
        return None
    callee = call.child_by_field_name("function")
    parameter = _require_parameter(context)
    if parameter is None or callee.type != "identifier":
        return None
    binding = context.bindings.resolve(callee.text.decode(), callee.start_byte)
    if (binding.get("start_byte"), binding.get("end_byte")) != _key(parameter):
        return None
    return frontend.webpack_module_id(node)


def compare(left, right, *, import_substitutions=()):
    """Return binding-structure correspondence and all unaccepted differences.

    Coordinates are relative to the bytes supplied to SourceContext.parse.
    An import pair allows that numeric argument to differ only in an actual
    call to the factory's third parameter. It does not verify either provider.
    """
    allowed = set()
    for pair in import_substitutions:
        if not isinstance(pair, (tuple, list)) or len(pair) != 2 or any(type(value) is not int for value in pair):
            raise ValueError("Import substitutions must be explicit integer ID pairs")
        allowed.add(tuple(pair))
    node_pairs, positions, differences = [], {}, []
    pending = [(left.root, right.root)]
    while pending:
        a, b = pending.pop()
        positions[_key(a)] = _key(b)
        if a.type != b.type:
            differences.append({"reason": "syntax", "left": _brief(left, a), "right": _brief(right, b)})
            continue
        achildren = [n for n in a.children if n.type != "comment"]
        bchildren = [n for n in b.children if n.type != "comment"]
        if len(achildren) != len(bchildren):
            differences.append({"reason": "child_count", "left": _brief(left, a), "right": _brief(right, b)})
            continue
        if achildren:
            pending.extend(reversed(list(zip(achildren, bchildren))))
        else:
            node_pairs.append((a, b))
    right_positions = set(positions.values())
    forward, backward, renamed, dependencies, external = {}, {}, {}, [], {}
    local_resolutions = {"declaration", "parameter_requires_caller_analysis", "destructuring_requires_value_analysis"}
    for a, b in node_pairs:
        if a.type == "identifier" and not (_public_name(a) or _public_name(b)):
            x = left.bindings.resolve(a.text.decode(), a.start_byte)
            y = right.bindings.resolve(b.text.decode(), b.start_byte)
            xr = (x.get("start_byte"), x.get("end_byte"))
            yr = (y.get("start_byte"), y.get("end_byte"))
            xlocal = x.get("resolution") in local_resolutions and xr in positions
            ylocal = y.get("resolution") in local_resolutions and yr in right_positions
            if xlocal and ylocal and positions[xr] == yr and x.get("resolution") == y.get("resolution"):
                xkey, ykey = (*xr, a.text.decode()), (*yr, b.text.decode())
                if (xkey in forward and forward[xkey] != ykey) or (ykey in backward and backward[ykey] != xkey):
                    reason = "binding_bijection"
                else:
                    forward[xkey], backward[ykey] = ykey, xkey
                    if a.text != b.text:
                        renamed[xkey] = {"left_name": a.text.decode(), "right_name": b.text.decode(),
                            "left_declaration": [v - left.offset for v in xr],
                            "right_declaration": [v - right.offset for v in yr], "resolution": x["resolution"]}
                    continue
            elif not xlocal and not ylocal and a.text == b.text and x.get("resolution") == y.get("resolution"):
                external[a.text.decode()] = {"name": a.text.decode(), "resolution": x.get("resolution"),
                    "left_declaration": [v - left.offset for v in xr] if None not in xr else None,
                    "right_declaration": [v - right.offset for v in yr] if None not in yr else None,
                    "provider_equivalence": "not_inferred"}
                continue
            else:
                reason = "binding_target_or_global"
            differences.append({"reason": reason, "left": _brief(left, a), "right": _brief(right, b)})
            continue
        if a.text == b.text:
            continue
        aid, bid = _dependency_id(left, a), _dependency_id(right, b)
        if aid is not None and bid is not None and (aid, bid) in allowed:
            dependencies.append({"left_module_id": aid, "right_module_id": bid,
                                 "left": _brief(left, a), "right": _brief(right, b),
                                 "provider_equivalence": "not_inferred"})
        else:
            differences.append({"reason": "property_operator_or_literal", "left": _brief(left, a), "right": _brief(right, b)})
    return {"schema_version": 1, "structure_corresponds": not differences,
            "behavior_equivalence": "not_inferred", "target_execution": False,
            "left_root": _brief(left, left.root), "right_root": _brief(right, right.root),
            "left_sha256": hashlib.sha256(left.raw[left.offset:len(left.raw)-left.offset] if left.offset else left.raw).hexdigest(),
            "right_sha256": hashlib.sha256(right.raw[right.offset:len(right.raw)-right.offset] if right.offset else right.raw).hexdigest(),
            "renamed_bindings": list(renamed.values()), "dependency_changes": dependencies,
            "external_references": list(external.values()), "differences": differences,
            "limits": ["Only AST and binding structure is compared. Name reflection, eval and function-source observations need their own semantic review.",
                       "Changed dependency IDs and declarations outside a selected owner are explicit obligations, not proofs that providers are interchangeable."]}
