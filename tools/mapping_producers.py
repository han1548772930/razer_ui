"""Static current mapping producer extraction for the workspace workflow.

Only a small, explicit subset of JavaScript AST expressions is interpreted as
source data. Vendor JavaScript is never loaded or executed.
"""
from __future__ import annotations

import hashlib
import concurrent.futures
import json
import sys
from pathlib import Path

import analysis
import families

ROOT = Path(__file__).resolve().parent.parent
INPUT = ".analysis/inputs/typed-tables/razer-pages/mouse_button_state_current_data.json"
REPORT = ".analysis/results/frontend/mapping-producers-current.json"
MISSING = object()


def walk(node):
    yield node
    for child in node.named_children:
        yield from walk(child)


class Returned(Exception):
    def __init__(self, value):
        self.value = value


class SourceObject:
    def __init__(self, owner, node):
        self.owner = owner
        self.node = node

    def get(self, key):
        found = MISSING
        for pair in self.node.named_children:
            if pair.type == "pair":
                name = pair.child_by_field_name("key")
                name = self.owner.literal(name) if name.type in ("string", "number") else self.owner.text(name)
                if name == key:
                    found = pair.child_by_field_name("value")
            elif pair.type == "spread_element":
                spread = self.owner.value(pair.named_children[0])
                if isinstance(spread, SourceObject):
                    value = spread.get(key)
                    if value is not MISSING:
                        found = value
                else:
                    raise ValueError("Unresolved source object spread")
        return found

    def field(self, key):
        node = self.get(key)
        return MISSING if node is MISSING else self.owner.value(node)


class SourceModule:
    def __init__(self, bundle, row):
        from tree_sitter import Language, Parser
        import tree_sitter_javascript

        self.bundle = bundle
        self.row = row
        self.prefix = b"const SOURCE_MODULE={" + (b"0:" if isinstance(row["module_id"], str) else b"")
        self.raw = self.prefix + row["body"].encode("utf-8") + b"};"
        self.tree = Parser(Language(tree_sitter_javascript.language())).parse(self.raw)
        if self.tree.root_node.has_error:
            raise ValueError("Source producer factory AST failed")
        obj = self.tree.root_node.named_children[0].named_children[0].child_by_field_name("value")
        factory = obj.named_children[0].child_by_field_name("value")
        self.statements = factory.child_by_field_name("body")
        self.bindings, self.exports, self.destructured, self.retained = {}, {}, {}, {}
        for statement in self.statements.named_children:
            if statement.type in ("lexical_declaration", "variable_declaration"):
                for declaration in statement.named_children:
                    if declaration.type != "variable_declarator":
                        continue
                    name = declaration.child_by_field_name("name")
                    if name.type == "identifier":
                        self.bindings[self.text(name)] = declaration.child_by_field_name("value")
                    elif name.type == "object_pattern":
                        for item in name.named_children:
                            if item.type == "object_assignment_pattern":
                                identifier = item.child_by_field_name("left")
                                self.destructured[self.text(identifier)] = (
                                    declaration.child_by_field_name("value"), item.child_by_field_name("right"))
            if statement.type == "expression_statement":
                for node in walk(statement):
                    if node.type != "call_expression":
                        continue
                    function = node.child_by_field_name("function")
                    args = node.child_by_field_name("arguments").named_children
                    if (function.type == "member_expression" and self.text(function.child_by_field_name("property")) == "d"
                            and len(args) == 2 and args[1].type == "object"):
                        for pair in args[1].named_children:
                            if pair.type != "pair":
                                continue
                            getter = pair.child_by_field_name("value")
                            if getter.type == "arrow_function":
                                result = getter.child_by_field_name("body")
                                if result.type == "identifier":
                                    self.exports[self.text(pair.child_by_field_name("key"))] = self.text(result)

    def text(self, node):
        return self.raw[node.start_byte:node.end_byte].decode("utf-8")

    def retain(self, node, role):
        body = self.raw[node.start_byte:node.end_byte]
        key = (node.start_byte, node.end_byte)
        self.retained[key] = {"role": role, "module_id": self.row["module_id"],
            "byte_start": node.start_byte - len(self.prefix), "byte_end": node.end_byte - len(self.prefix),
            "body": body.decode("utf-8"), "sha256": hashlib.sha256(body).hexdigest()}

    def literal(self, node):
        import json5
        return json5.loads(self.text(node))

    def exported_node(self, name):
        binding = self.exports.get(name)
        if binding not in self.bindings:
            return MISSING
        node = self.bindings[binding]
        self.retain(node, "export " + name)
        return node

    def export(self, name):
        node = self.exported_node(name)
        return MISSING if node is MISSING else self.value(node)

    def value(self, node, local=None):
        local = {} if local is None else local
        if node is None or node is MISSING:
            return MISSING
        kind = node.type
        if kind in ("string", "number", "true", "false", "null"):
            return self.literal(node)
        if kind in ("parenthesized_expression", "spread_element"):
            return self.value(node.named_children[0], local)
        if kind == "identifier":
            name = self.text(node)
            if name in local:
                return local[name]
            if name in ("undefined",):
                return MISSING
            if name in self.destructured:
                source, fallback = self.destructured[name]
                obj = self.value(source)
                value = obj.field(name) if isinstance(obj, SourceObject) else MISSING
                return self.value(fallback) if value is MISSING else value
            if name not in self.bindings:
                raise ValueError("Unresolved producer binding " + name)
            target = self.bindings[name]
            self.retain(target, "binding " + name)
            return self.value(target)
        if kind == "object":
            return SourceObject(self, node)
        if kind == "assignment_expression" and node.child_by_field_name("left").type == "identifier":
            value = self.value(node.child_by_field_name("right"), local)
            local[self.text(node.child_by_field_name("left"))] = value
            return value
        if kind == "binary_expression":
            left, right = node.child_by_field_name("left"), node.child_by_field_name("right")
            operator = self.raw[left.end_byte:right.start_byte].decode().strip()
            value = self.value(left, local)
            if operator == "&&":
                return self.value(right, local) if truthy(value) else value
            if operator in ("||", "??"):
                empty = not truthy(value) if operator == "||" else value is MISSING or value is None
                return self.value(right, local) if empty else value
            other = self.value(right, local)
            if operator in ("===", "!=="):
                equal = type(value) is type(other) and value == other
                return equal if operator == "===" else not equal
        if kind == "array":
            result = []
            for item in node.named_children:
                if item.type == "comment":
                    continue
                value = self.value(item, local)
                if item.type == "spread_element":
                    if not isinstance(value, list):
                        raise ValueError("Source array spread is not an array")
                    result.extend(value)
                else:
                    result.append(value)
            return result
        if kind == "member_expression":
            obj = self.value(node.child_by_field_name("object"), local)
            key = self.text(node.child_by_field_name("property"))
            if isinstance(obj, SourceObject):
                return obj.field(key)
            if isinstance(obj, SourceModule):
                return obj.export(key)
            if obj is MISSING or obj is None:
                return MISSING
            if isinstance(obj, list) and key == "length":
                return len(obj)
            raise ValueError("Unresolved producer member " + self.text(node))
        if kind == "unary_expression":
            argument = node.child_by_field_name("argument")
            operator = self.text(node)[:argument.start_byte - node.start_byte].strip()
            if operator == "!":
                return not truthy(self.value(argument, local))
            if operator == "void":
                return MISSING
            if operator == "-":
                return -self.value(argument, local)
        if kind == "ternary_expression":
            return self.value(node.child_by_field_name("consequence") if truthy(
                self.value(node.child_by_field_name("condition"), local)) else node.child_by_field_name("alternative"), local)
        if kind == "call_expression":
            function = node.child_by_field_name("function")
            args = node.child_by_field_name("arguments").named_children
            if function.type == "identifier" and len(args) == 1 and args[0].type == "number":
                pid = self.literal(args[0])
                if pid in self.bundle.rows:
                    return self.bundle.module(pid)
            if function.type == "identifier" and self.text(function) == "structuredClone" and len(args) == 1:
                return self.value(args[0], local)
            if function.type == "member_expression":
                obj = function.child_by_field_name("object")
                field = self.text(function.child_by_field_name("property"))
                if self.text(obj) == "Array" and field == "isArray":
                    return isinstance(self.value(args[0], local), list)
                if field == "concat":
                    initial = self.value(obj, local)
                    if not isinstance(initial, list):
                        raise ValueError("Source concat receiver is not an array")
                    result = initial.copy()
                    for arg in args:
                        additional = self.value(arg, local)
                        result.extend(additional if isinstance(additional, list) else [additional])
                    return result
            if function.type == "parenthesized_expression" and function.named_children[0].type == "arrow_function" and not args:
                return self.call_literal_function(function.named_children[0])
        raise ValueError("Unresolved static producer expression " + kind + ": " + self.text(node)[:160])

    def execute(self, statement, local):
        if statement.type == "statement_block":
            for item in statement.named_children:
                self.execute(item, local)
        elif statement.type in ("lexical_declaration", "variable_declaration"):
            for declaration in statement.named_children:
                if declaration.type == "variable_declarator":
                    local[self.text(declaration.child_by_field_name("name"))] = self.value(declaration.child_by_field_name("value"), local)
        elif statement.type == "if_statement":
            branch = "consequence" if truthy(self.value(statement.child_by_field_name("condition"), local)) else "alternative"
            selected = statement.child_by_field_name(branch)
            if selected is not None:
                self.execute(selected, local)
        elif statement.type == "expression_statement":
            expression = statement.named_children[0]
            if expression.type != "assignment_expression" or expression.child_by_field_name("left").type != "identifier":
                raise ValueError("Unexpected statement in a source data helper")
            local[self.text(expression.child_by_field_name("left"))] = self.value(expression.child_by_field_name("right"), local)
        elif statement.type == "return_statement":
            raise Returned(self.value(statement.named_children[0], local) if statement.named_children else MISSING)
        elif statement.type != "comment":
            raise ValueError("Unexpected static source statement " + statement.type)

    def call_literal_function(self, function):
        try:
            self.execute(function.child_by_field_name("body"), {})
        except Returned as result:
            return result.value
        raise ValueError("Source literal function did not return")

    def lookup(self, exported):
        function = self.exported_node(exported)
        if function is MISSING:
            raise ValueError("Helper export is missing: " + exported)
        local = {}
        body = function.child_by_field_name("body")
        if not any(node.type == "call_expression"
                and node.child_by_field_name("function").type == "member_expression"
                and self.text(node.child_by_field_name("function").child_by_field_name("property")) == "find"
                for node in walk(body)):
            return None
        for statement in body.named_children:
            finds = [node for node in walk(statement) if node.type == "call_expression"
                and node.child_by_field_name("function").type == "member_expression"
                and self.text(node.child_by_field_name("function").child_by_field_name("property")) == "find"]
            if finds:
                tables = []
                for found in finds:
                    table = self.value(found.child_by_field_name("function").child_by_field_name("object"), local)
                    if not isinstance(table, list):
                        raise ValueError("Source helper find receiver is not an array")
                    tables.extend(table)
                result = {}
                for row in tables:
                    if not isinstance(row, SourceObject):
                        raise ValueError("Source lookup row is not an object")
                    identifier, content = row.field("id"), row.field("content")
                    if not isinstance(identifier, str) or not isinstance(content, str):
                        raise ValueError("Source lookup has a nonstring identity/content")
                    result.setdefault(identifier, content)
                return result
            self.execute(statement, local)
        return None


def truthy(value):
    return value is not MISSING and value is not None and value is not False and value != 0 and value != ""


class Bundle:
    def __init__(self, row):
        self.path = ROOT / row["path"]
        if not families.valid(row):
            raise ValueError("Current mapping producer input bytes differ")
        modules = families.keyboard_source_modules(self.path.read_text(encoding="utf-8"))
        self.rows = {module["module_id"]: module for module in modules}
        self.modules = {}

    def module(self, identity):
        if identity not in self.modules:
            self.modules[identity] = SourceModule(self, self.rows[identity])
        return self.modules[identity]


METHOD_GROUPS = {
    "assignMouseGroup":"mouseGroup", "assignSensitivityGroup":"sensitivityGroup",
    "assignScrollingGroup":"scrollingGroup", "assignProfileGroup":"profileNavigationGroup",
    "assignMultimediaGroup":"multimediaGroup", "assignWindowsShortcutGroup":"win8ShortcutsGroup",
    "assignBacklightGroup":"backlightGroup", "assignBrightnessGlobalGroup":"brightnessGlobalGroup",
    "assignAudioGroup":"audioGroup", "assignKnobGroup":"knobGroup", "assignKeyboardGroup":"keyboardGroup",
    "assignDynamicKeyStrokeGroup":"dynamicKeyStrokeGroup", "assignControllerGroup":"controllerGroup",
    "assignJoystickGroup":"joystickGroup", "assignKeymapGroup":"keymapGroup",
    "assignMacroGroup":"macroGroup", "assignInterDeviceGroup":"interDeviceGroup",
    "assignLightpacGroup":"lightPacGroup", "assignHyperShiftGroup":"hyperShiftGroup",
    "assignLaunchGroup":"launchGroup", "assignAILauncherGroup":"aiLauncherGroup",
    "assignTextGroup":"textBlockGroup", "assignDisableGroup":"disableGroup",
    "assignAppSpecificGroup":"appSpecificGroup",
}


def extract(product):
    bundle = Bundle(product["main"])
    candidates = [identity for identity, row in bundle.rows.items()
        if "assignKeyboardGroup" in row["body"] and "getKeyValue" in row["body"] and "clearButtonValues" in row["body"]]
    candidates = [identity for identity in candidates if not any(
        bundle.rows[other]["byte_start"] >= bundle.rows[identity]["byte_start"]
        and bundle.rows[other]["byte_end"] <= bundle.rows[identity]["byte_end"] and other != identity
        for other in candidates)]
    if len(candidates) != 1:
        raise ValueError("Expected one current producer factory: " + repr(candidates))
    owner = bundle.module(candidates[0])
    labels, lookups, methods, helpers = {}, {}, [], []
    for node in walk(owner.statements):
        if node.type != "assignment_expression":
            continue
        left, function = node.child_by_field_name("left"), node.child_by_field_name("right")
        if (left.type != "member_expression" or owner.text(left.child_by_field_name("object")) != "this"
                or function.type != "arrow_function"):
            continue
        method = owner.text(left.child_by_field_name("property"))
        if method not in METHOD_GROUPS and method not in ("getKeyValue", "assignMappingGroup", "clearButtonValues", "updateButtonWithMappings", "updateMappingsToButtons", "updateMappingsToButtonsNew"):
            continue
        owner.retain(function, method)
        methods.append(method)
        group = METHOD_GROUPS.get(method)
        if not group:
            continue
        params = function.child_by_field_name("parameters").named_children
        if len(params) < 2:
            raise ValueError("Unexpected producer parameters")
        prefix = owner.text(params[-1])
        for item in walk(function.child_by_field_name("body")):
            if item.type == "assignment_expression":
                target, value = item.child_by_field_name("left"), item.child_by_field_name("right")
                if target.type == "subscript_expression" and owner.text(target.child_by_field_name("index")) == prefix:
                    label = owner.value(value)
                    if isinstance(label, str):
                        if group in labels and labels[group] != label:
                            labels[group + ":advanced"] = label
                        else:
                            labels[group] = label
            if item.type != "call_expression":
                continue
            helper = item.child_by_field_name("function")
            if helper.type != "member_expression":
                continue
            receiver = helper.child_by_field_name("object")
            if receiver.type != "identifier" or owner.text(receiver) not in owner.bindings:
                continue
            imported = owner.value(receiver)
            if not isinstance(imported, SourceModule):
                continue
            name = owner.text(helper.child_by_field_name("property"))
            if name not in imported.exports:
                continue
            key = group
            table = imported.lookup(name)
            helpers.append({"method":method, "module_id":imported.row["module_id"], "export":name,
                "table_membership_projected":table is not None})
            if table is not None:
                if key in lookups and table != lookups[key]:
                    key += ":" + name
                lookups[key] = table
    return {"product_id": product["product_id"], "assignment_labels":labels, "lookups":lookups}, {
        "main":product["main"], "producer_factory":bundle.rows[candidates[0]], "methods":methods, "helpers":helpers,
        "retained": [retained for module in bundle.modules.values() for retained in module.retained.values()]}


def prepare(*, check=False, workers=4):
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Mapping producer workers must be between two and sixteen")
    capabilities = json.loads((ROOT / ".analysis/results/typed-data.json").read_bytes())["current_mouse_mapping_capabilities"]
    def extract_product(product):
        try:
            projection, receipt = extract(product)
            return projection, receipt, None
        except Exception as error:
            return None, None, {"product_id": product["product_id"], "error": str(error)}

    products, failures, evidence = [], [], []
    selected = capabilities["products"]
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(2, min(workers, len(selected)))) as pool:
        for projection, receipt, failure in pool.map(extract_product, selected):
            if failure is not None:
                failures.append(failure)
            else:
                products.append(projection)
                evidence.append(receipt)
    report = {"tool": families.identity(Path(__file__)), "products":products, "evidence":evidence,
        "failures":failures, "vendor_execution":False, "application_execution":False,
        "passed":not failures, "check_only":check}
    analysis.save(ROOT / REPORT, report)
    return {"report":REPORT, "products":len(products), "failures":failures, "passed":not failures}
