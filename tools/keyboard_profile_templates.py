"""Recover complete keyboard profile recipes from their current AST identities.

This is a maintenance input to typed Rust constructors. It never executes vendor
JavaScript, and it resolves each shared factory once through the identity index.
"""
from __future__ import annotations

import hashlib
import json
import copy
from pathlib import Path
import sqlite3
import sys

import code_identity as ci

ROOT = Path(__file__).resolve().parent.parent
TEMPLATES = ".analysis/results/reverse-pending/review-sources/7d0b7b150902ac6353c866630cb95fc8084c3bc264ee65c1b4cdc0a383cefc9e.json"
DOCUMENTS = ".analysis/results/reverse-pending/review-sources/cf69c18bfcc02812787a804a74ac27ec7ba48cd416188037fc96d76e1f06c0f6.json"
OUTPUT = "crates/razer-device/src/keyboard/keyboard_local_profile_templates.rs"


def walk(node):
    yield node
    for child in node.named_children:
        yield from walk(child)


class Module:
    def __init__(self, index, product_id, record, original):
        self.index, self.product_id, self.record = index, product_id, record
        self.original = original
        raw = index.original(original)
        body = raw[record["start_byte"]:record["end_byte"]]
        self.factory = None
        cached = index.syntax.get(record["normalized_sha"])
        if cached is None:
            for prefix, suffix in [(b"(", b")"), (b"({", b"})")]:
                tree = index.parser.parse(prefix + body + suffix)
                if tree.root_node.has_error:
                    continue
                node = tree.root_node.descendant_for_byte_range(len(prefix), len(prefix) + len(body) - 1)
                if (node.start_byte == len(prefix) and node.end_byte == len(prefix) + len(body)
                        and ci.normalized_ast(node, factory=True) == record["normalized_sha"]):
                    cached = tree, node, len(prefix), body
                    index.syntax[record["normalized_sha"]] = cached
                    break
        if cached is not None:
            self.tree, self.factory, prefix_length, self.source_body = cached
            name_delta = 0
            if body != self.source_body:
                name = self.factory.child_by_field_name("name") if self.factory.type == "method_definition" else None
                if name is None:
                    raise ValueError("Equivalent keyboard factory needs an indexed owner-offset mapping")
                start, end = name.start_byte - prefix_length, name.end_byte - prefix_length
                replacement = record["module_id"].encode()
                expected = self.source_body[:start] + replacement + self.source_body[end:]
                if body != expected:
                    raise ValueError("Equivalent keyboard factory needs an indexed owner-offset mapping")
                name_delta = len(replacement) - (end - start)
            self.offset = record["start_byte"] - prefix_length + name_delta
        if self.factory is None:
            raise ValueError("Keyboard profile factory normalized identity differs")
        self.bindings, self.exports, self.namespaces = {}, {}, {}
        self.guid_functions, self.dynamic_guids = set(), {}
        self.require = self.factory.child_by_field_name("parameters").named_children[2].text
        self.export_object = self.factory.child_by_field_name("parameters").named_children[1].text
        self.body = self.factory.child_by_field_name("body")
        for statement in self.body.named_children:
            if statement.type in {"lexical_declaration", "variable_declaration"}:
                for node in statement.named_children:
                    if node.type == "variable_declarator":
                        name = node.child_by_field_name("name")
                        if name.type == "identifier":
                            self.bindings[name.text.decode()] = node.child_by_field_name("value")
            if statement.type == "function_declaration":
                self.bindings[statement.child_by_field_name("name").text.decode()] = statement
        for node in walk(self.body):
            if node.type != "call_expression":
                continue
            function = node.child_by_field_name("function")
            arguments = node.child_by_field_name("arguments").named_children
            if (function.type == "member_expression"
                    and function.child_by_field_name("object").text == self.require
                    and function.child_by_field_name("property").text == b"d"
                    and len(arguments) == 2 and arguments[1].type == "object"):
                target = arguments[0].text
                exports = (self.exports if target == self.export_object else
                           self.namespaces.setdefault(target.decode(), {}))
                for pair in arguments[1].named_children:
                    key, getter = pair.child_by_field_name("key"), pair.child_by_field_name("value")
                    body = getter.child_by_field_name("body")
                    if body.type == "statement_block":
                        statements = [n for n in body.named_children if n.type != "comment"]
                        if len(statements) != 1 or statements[0].type != "return_statement":
                            raise ValueError("Keyboard profile export getter has non-return behavior")
                        body = statements[0].named_children[0]
                    exports[self.key(key)] = body

    def key(self, node):
        if node.type == "string":
            return self.scalar(node)
        if node.type in {"property_identifier", "identifier", "number"}:
            return node.text.decode()
        raise ValueError("Dynamic keyboard profile property: " + node.type)

    @staticmethod
    def scalar(node):
        import json5
        value = json5.loads(node.text.decode())
        return int(value) if isinstance(value, float) and value.is_integer() else value

    def enum(self, name):
        matches = []
        for call in walk(self.body):
            if call.type != "call_expression":
                continue
            args = call.child_by_field_name("arguments").named_children
            if len(args) != 1 or args[0].type != "binary_expression":
                continue
            left, right = args[0].child_by_field_name("left"), args[0].child_by_field_name("right")
            if left.type != "identifier" or left.text.decode() != name:
                continue
            function = call.child_by_field_name("function")
            while function.type in {"parenthesized_expression", "unary_expression"}:
                function = function.named_children[-1]
            if function.type not in {"function_expression", "function", "arrow_function"}:
                continue
            parameters = function.child_by_field_name("parameters").named_children
            if len(parameters) != 1:
                continue
            parameter = parameters[0].text
            entries = {}
            for node in walk(function.child_by_field_name("body")):
                if node.type != "assignment_expression":
                    continue
                target = node.child_by_field_name("left")
                if target.type != "member_expression" or target.child_by_field_name("object").text != parameter:
                    continue
                entries[self.key(target.child_by_field_name("property"))] = self.value(node.child_by_field_name("right"))
            if entries:
                matches.append(entries)
        if len(matches) != 1:
            raise ValueError("Unresolved keyboard profile enum: " + name)
        return matches[0]

    def value(self, node, seen=(), *, profile=False):
        if node is None:
            raise ValueError("Keyboard profile binding has no initializer")
        kind = node.type
        if kind in {"string", "number", "true", "false", "null"}:
            return self.scalar(node)
        if kind == "identifier":
            name = node.text.decode()
            if name in seen or name not in self.bindings:
                raise ValueError("Unresolved/cyclic keyboard profile binding: " + name)
            binding = self.bindings[name]
            if name in self.namespaces:
                if binding is None or binding.type != "object" or binding.named_children:
                    raise ValueError("Keyboard namespace has an unexpected initializer")
                return Namespace(self, self.namespaces[name])
            return self.enum(name) if binding is None else self.value(binding, (*seen, name), profile=profile)
        if kind == "parenthesized_expression":
            return self.value(node.named_children[0], seen, profile=profile)
        if kind == "sequence_expression":
            for preceding in node.named_children[:-1]:
                if (preceding.type != "call_expression"
                        or preceding.child_by_field_name("function").text != self.require
                        or len(preceding.child_by_field_name("arguments").named_children) != 1
                        or preceding.child_by_field_name("arguments").named_children[0].type != "number"):
                    raise ValueError("Keyboard profile comma expression has non-import preceding work")
            return self.value(node.named_children[-1], seen)
        if kind == "template_string":
            parts = []
            for child in node.named_children:
                if child.type == "string_fragment":
                    parts.append(child.text.decode())
                elif child.type == "template_substitution":
                    value = self.value(child.named_children[0], seen)
                    if not isinstance(value, (str, int, float)) or isinstance(value, bool):
                        raise ValueError("Keyboard profile interpolation is not a static name")
                    parts.append(str(value))
                else:
                    raise ValueError("Keyboard profile template name has an unsupported escape")
            return "".join(parts)
        if kind == "unary_expression":
            argument = node.child_by_field_name("argument")
            operator = node.text[:argument.start_byte - node.start_byte].strip()
            value = self.value(argument, seen)
            if operator == b"!":
                return not value
            if operator == b"-":
                return -value
        if kind == "array":
            values = []
            for child in node.named_children:
                if child.type == "comment":
                    continue
                spread = child.type == "spread_element"
                value = self.value(child.named_children[0] if spread else child, seen)
                if spread:
                    if not isinstance(value, list):
                        raise ValueError("Keyboard profile array spread is not an array")
                    values.extend(value)
                else:
                    values.append(value)
            return values
        if kind == "object":
            values = {}
            for child in node.named_children:
                if child.type == "comment":
                    continue
                if child.type == "spread_element":
                    values.update(self.value(child.named_children[0], seen, profile=profile))
                    continue
                if child.type != "pair":
                    raise ValueError("Keyboard profile object has an unresolved member")
                key = self.key(child.child_by_field_name("key"))
                # The reviewed profile generators replace both fields before
                # publishing the recipe. Runtime supplies the actual values.
                values[key] = "" if profile and key in {"guid", "name"} else self.value(child.child_by_field_name("value"), seen)
            return values
        if kind in {"member_expression", "subscript_expression"}:
            value = self.value(node.child_by_field_name("object"), seen)
            key = (self.key(node.child_by_field_name("property")) if kind == "member_expression"
                   else self.value(node.child_by_field_name("index"), seen))
            if isinstance(value, Module):
                if key not in value.exports:
                    raise ValueError("Keyboard profile export is absent: " + str(key))
                return value.value(value.exports[key])
            if isinstance(value, Namespace):
                return value.module.value(value.exports[key])
            return value[key]
        if kind == "call_expression":
            function = node.child_by_field_name("function")
            args = node.child_by_field_name("arguments").named_children
            if not args and self.callable_name(function) in self.guid_functions:
                key = node.start_byte
                if key not in self.dynamic_guids:
                    self.dynamic_guids[key] = len(self.dynamic_guids)
                return {"runtime_guid": self.dynamic_guids[key]}
            if function.type == "identifier" and function.text == self.require and len(args) == 1 and args[0].type == "number":
                return self.index.module(self.product_id, str(self.scalar(args[0])))
            if function.text == b"Array.from" and len(args) == 2:
                size = self.value(args[0], seen)
                callback = args[1]
                while callback.type == "parenthesized_expression":
                    callback = callback.named_children[0]
                if (not isinstance(size, dict) or set(size) != {"length"}
                        or type(size["length"]) is not int or not 0 <= size["length"] <= 64
                        or callback.type != "arrow_function"
                        or callback.child_by_field_name("parameters").named_children):
                    raise ValueError("Keyboard Array.from is not a bounded constant recipe")
                value = self.value(callback.child_by_field_name("body"), seen)
                return [copy.deepcopy(value) for _ in range(size["length"])]
        raise ValueError("Unresolved keyboard profile expression: " + kind + " " + node.text.decode()[:120])

    @staticmethod
    def callable_name(node):
        while node.type in {"parenthesized_expression", "sequence_expression"}:
            node = node.named_children[-1]
        return node.text.decode()

    def owner_node(self, owner):
        start, end = owner["byte_start"] - self.offset, owner["byte_end"] - self.offset
        node = self.tree.root_node.descendant_for_byte_range(start, end - 1)
        if node.start_byte != start or node.end_byte != end:
            raise ValueError("Keyboard template owner boundary differs")
        return node

    def admit_guid_source(self, owner):
        node = self.owner_node(owner)
        for child in walk(node):
            if child.type == "assignment_expression":
                target, value = child.child_by_field_name("left"), child.child_by_field_name("right")
                key = target.child_by_field_name("property") if target.type == "member_expression" else None
            elif child.type == "pair":
                key, value = child.child_by_field_name("key"), child.child_by_field_name("value")
            else:
                continue
            if key is not None and key.text == b"guid" and value.type == "call_expression":
                if value.child_by_field_name("arguments").named_children:
                    raise ValueError("Keyboard generated GUID source takes unexpected arguments")
                self.guid_functions.add(self.callable_name(value.child_by_field_name("function")))
        if len(self.guid_functions) != 1:
            raise ValueError("Keyboard profile generator has no unique GUID provider")

    def owner_value(self, owner, *, profile=False):
        node = self.owner_node(owner)
        if node.type == "variable_declarator":
            node = node.child_by_field_name("value")
        return self.value(node, profile=profile)


class Namespace:
    def __init__(self, module, exports):
        self.module, self.exports = module, exports


class Index:
    def __init__(self, database):
        sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
        from tree_sitter import Parser, Language
        import tree_sitter_javascript
        self.parser = Parser(Language(tree_sitter_javascript.language()))
        self.database = database
        self.modules, self.sources, self.syntax = {}, {}, {}

    def original(self, original):
        path = original["path"]
        if path not in self.sources:
            raw = (ROOT / path).read_bytes()
            if len(raw) != original["bytes"] or hashlib.sha256(raw).hexdigest() != original["sha256"]:
                raise ValueError("Keyboard template original bytes differ")
            self.sources[path] = raw
        return self.sources[path]

    def module(self, product_id, module_id):
        rows = self.database.execute("""SELECT DISTINCT f.*,s.path,s.bytes FROM deployment_sources ds
            JOIN sources s ON s.path=ds.path JOIN factories f ON f.source_sha=s.sha256
            WHERE ds.product_id=? AND ds.layer='mw' AND f.module_id=? AND f.kind='webpack_factory'""",
            (product_id, module_id)).fetchall()
        if len({row["normalized_sha"] for row in rows}) != 1:
            raise ValueError(f"Keyboard template module {product_id}/{module_id} is absent or ambiguous")
        row = dict(rows[0])
        key = product_id, row["module_id"], row["normalized_sha"]
        if key not in self.modules:
            original = {"path": row["path"], "bytes": row["bytes"], "sha256": row["source_sha"]}
            self.modules[key] = Module(self, product_id, row, original)
        return self.modules[key]

    def owner(self, product_id, owner):
        raw = self.original(owner["original"])
        if hashlib.sha256(raw[owner["byte_start"]:owner["byte_end"]]).hexdigest() != owner["body_sha256"]:
            raise ValueError("Keyboard template owner bytes differ")
        row = self.database.execute("""SELECT * FROM factories WHERE source_sha=? AND start_byte<=?
            AND end_byte>=? AND kind='webpack_factory'""", (owner["original"]["sha256"],
            owner["byte_start"], owner["byte_end"])).fetchone()
        if row is None:
            raise ValueError("Keyboard template owner has no indexed factory")
        if self.module(product_id, row["module_id"]).record["normalized_sha"] != row["normalized_sha"]:
            raise ValueError("Keyboard template owner is not the product's current factory")
        key = product_id, row["module_id"], row["normalized_sha"]
        if key not in self.modules:
            self.modules[key] = Module(self, product_id, dict(row), owner["original"])
        module = self.modules[key]
        # Identical factories at another byte offset reuse the parsed AST.
        owner = {**owner, "byte_start": module.record["start_byte"] + owner["byte_start"] - row["start_byte"],
                 "byte_end": module.record["start_byte"] + owner["byte_end"] - row["start_byte"]}
        return module, owner


def recover():
    templates = json.loads((ROOT / TEMPLATES).read_bytes())
    documents = json.loads((ROOT / DOCUMENTS).read_bytes())
    owners = {row["id"]: row for source in (templates, documents) for row in source["complete_source_owners"]}
    bases = {row["product_id"]: row for row in documents["default_document_sources"]}
    result, failures, versions = [], [], {}
    with sqlite3.connect("file:" + (ROOT / ".analysis/results/code-identity.sqlite3").as_posix() + "?mode=ro", uri=True) as database:
        database.row_factory = sqlite3.Row
        index = Index(database)
        for product in templates["default_lighting_templates"]:
            pid = product["product_id"]
            try:
                generator = owners[bases[pid]["profile_generator_owner"]]
                module, adjusted_generator = index.owner(pid, generator)
                module.admit_guid_source(adjusted_generator)
                variants = []
                for template in product.get("templates", []) if pid != 708 else []:
                    owner = owners[template["owner"]]
                    module, adjusted = index.owner(pid, owner)
                    variants.append({"edition": template["edition"], "profile": module.owner_value(adjusted, profile=True),
                                     "owner": owner["id"]})
                owner = owners[bases[pid]["device_base_owner"]]
                module, adjusted = index.owner(pid, owner)
                base = module.owner_value(adjusted)
                if pid == 708:
                    source = module.owner_node(adjusted_generator)
                    arrays = [node for node in walk(source) if node.type == "member_expression"
                              and node.child_by_field_name("property").text == b"DEFAULTPROFILE"]
                    if len(arrays) != 1:
                        raise ValueError("708 application profile array is absent or ambiguous")
                    profiles = module.value(arrays[0])
                    if not isinstance(profiles, list) or len(profiles) != 13:
                        raise ValueError("708 complete application profile recipes differ")
                    variants = [{"edition": None, "profiles": profiles, "owner": generator["id"]}]
                result.append({"product_id": pid, "templates": variants, "device": base,
                               "dynamic_guids": len(module.dynamic_guids)})
            except (ValueError, KeyError, IndexError, TypeError, AttributeError) as error:
                failures.append({"product_id": pid, "error": str(error)})
        for cohort in documents["setup_variants"]:
            owner = owners[cohort["owner_id"]]
            for pid in cohort["product_ids"]:
                try:
                    module, _ = index.owner(pid, owner)
                    assignments = []
                    for node in walk(module.body):
                        if node.type != "assignment_expression":
                            continue
                        left = node.child_by_field_name("left")
                        if left.type == "member_expression" and left.child_by_field_name("property").text == b"schemaVersion":
                            assignments.append(node.child_by_field_name("right"))
                    if len(assignments) != 1:
                        raise ValueError("Keyboard seed schema assignment is absent or ambiguous")
                    value = module.value(assignments[0])
                    if type(value) is not int or value not in {12, 13, 14}:
                        raise ValueError("Keyboard current schema needs a verified migration contract")
                    versions[pid] = value
                except (ValueError, KeyError, IndexError, TypeError, AttributeError) as error:
                    failures.append({"product_id": pid, "schema_error": str(error)})
        return {"products": result, "schema_versions": versions, "failures": failures,
                "parsed_factories": len(index.syntax)}


def rust_renderer():
    import rust_data
    current = ROOT / "crates/razer-device/src/keyboard/keyboard_current_profile.rs"
    local = ROOT / "crates/razer-device/src/keyboard/keyboard_local_profile.rs"
    high_speed = ROOT / "crates/razer-device/src/keyboard/keyboard_high_speed_profile.rs"

    class KeyboardData(rust_data.RustData):
        def definition(self, name, module, value):
            short = name.rsplit("::", 1)[-1]
            target = None
            if name.startswith("super::"):
                target = high_speed if "::high_speed::" in name else current
            elif name.startswith("local::"):
                target = local
            else:
                target = module
            candidates = [item for item in self.definitions.get(short, []) if item.module == target]
            if len(candidates) == 1:
                return candidates[0]
            return super().definition(name, module, value)

        def constructor_type(self, typ, module, value):
            short = typ.rsplit("::", 1)[-1]
            if short in self.definitions and "<" not in typ:
                definition = self.definition(typ, module, value)
                prefix = {current: "crate::keyboard_current_profile::",
                          local: "crate::keyboard_current_profile::local::",
                          high_speed: "crate::keyboard_current_profile::high_speed::"}.get(definition.module)
                if prefix:
                    return prefix + definition.name
            return super().constructor_type(typ, module, value)

        def _render_inner(self, value, typ, module, path, allow_value):
            short = typ.rsplit("::", 1)[-1]
            if short in {"Extensions", "EmptyObject"}:
                if value != {}:
                    raise ValueError("Fixed keyboard profile data needs a named field: " + path)
                return "Default::default()"
            if typ == "String" and isinstance(value, dict) and set(value) == {"runtime_guid"}:
                index = value["runtime_guid"]
                if type(index) is not int or not 0 <= index < 16:
                    raise ValueError("Keyboard generated GUID slot differs")
                return f"ids.get({index}).to_owned()"
            if short == "ActuationPoint":
                if not isinstance(value, dict) or set(value) != {"0", "1"}:
                    raise ValueError("Keyboard default actuation shape differs")
                values = [super()._render_inner(value[key], "f64", module, path + "." + key, False) for key in ("0", "1")]
                return ("crate::keyboard_current_profile::ActuationPoint::Named{make:" + values[0]
                        + ",release:" + values[1] + ",extensions:Default::default()}")
            return super()._render_inner(value, typ, module, path, allow_value)

    return KeyboardData(ROOT / "crates/razer-device")


def render_recipes(recovered=None):
    import rust_data
    recovered = recover() if recovered is None else recovered
    if recovered["failures"] or len(recovered["products"]) != 73:
        raise ValueError("Keyboard default recipe recovery is incomplete: " + repr(recovered["failures"]))
    schema = rust_renderer()
    module = "crates/razer-device/src/keyboard/keyboard_local_profile.rs"
    profiles, devices, applications = {}, {}, []
    def render(value, typename):
        expression = schema.render(value, typename, module, allow_value=False)
        if schema.excluded or schema.value_fields:
            raise ValueError("Keyboard default recipe lost typed fields: " + repr(schema.excluded | schema.value_fields))
        # Missing optional fields remain None. Express that structural absence
        # once instead of duplicating every absent field in every recipe.
        raw = ("const RECIPE: () = " + expression + ";").encode()
        tree = schema.parser.parse(raw)
        if tree.root_node.has_error:
            raise ValueError("Generated keyboard constructor is not valid Rust syntax")
        node = tree.root_node.named_children[0].child_by_field_name("value")
        body = node.child_by_field_name("body")
        fields = []
        for field in body.named_children:
            name, val = field.child_by_field_name("field"), field.child_by_field_name("value")
            if name is None or val is None:
                raise ValueError("Generated keyboard constructor has an unexpected field")
            if val.text == b"None" or name.text == b"extensions" and val.text == b"Default::default()":
                continue
            fields.append(field.text.decode())
        expression = node.child_by_field_name("name").text.decode() + "{" + ",".join(fields) + ",..Default::default()}"
        return rust_data.pretty_expression(expression)
    for product in recovered["products"]:
        pid = product["product_id"]
        base = dict(product["device"])
        base.pop("profiles", None)
        # Product identity is supplied by the generated exact match arm.
        base["productId"] = 0
        device = render(base, "DeviceFields")
        devices.setdefault(device, []).append(pid)
        for variant in product["templates"]:
            if pid == 708:
                applications = [render(profile, "Profile") for profile in variant["profiles"]]
                continue
            expression = render(variant["profile"], "Profile")
            profiles.setdefault(expression, []).append((pid, int(variant["edition"])))
    return {"profile_constructors": profiles, "device_constructors": devices,
            "application_constructors": applications, "recovery": recovered}


def verify_rust_recipes(binary, recovered):
    import os
    import subprocess
    expected = {}
    def instantiate(value):
        if isinstance(value, dict):
            if set(value) == {"runtime_guid"}:
                return f"dial-{value['runtime_guid']}"
            return {key: instantiate(child) for key, child in value.items()}
        if isinstance(value, list):
            return [instantiate(child) for child in value]
        return value
    for product in recovered["products"]:
        pid = product["product_id"]
        for variant in product["templates"]:
            edition = 0 if variant["edition"] is None else int(variant["edition"])
            device = dict(product["device"])
            device.pop("profiles", None)
            expected[pid, edition] = {"product_id": pid, "edition": edition, "device": instantiate(device),
                                     "profiles": instantiate(variant.get("profiles", [variant.get("profile")]))}
    environment = os.environ.copy()
    environment["RAZER_KEYBOARD_TEMPLATE_REQUESTS"] = json.dumps(list(expected))
    command = [str(Path(binary).resolve()), "keyboard_local_recipe_complete_original_comparison", "--ignored", "--nocapture"]
    result = subprocess.run(command, env=environment, capture_output=True, text=True, encoding="utf-8")
    if result.returncode:
        raise ValueError("Rust keyboard template execution failed: " + result.stdout + result.stderr)
    actual = {}
    for line in result.stdout.splitlines():
        if not line.startswith("KEYBOARD_RECIPE_COMPARE "):
            continue
        value = json.loads(line.removeprefix("KEYBOARD_RECIPE_COMPARE "))
        key = value["product_id"], value["edition"]
        if key in actual:
            raise ValueError("Rust keyboard template output repeats a product edition")
        actual[key] = value
    if set(actual) != set(expected):
        raise ValueError("Rust keyboard template comparison omitted product editions")
    differences = [key for key in expected if ci.digest(actual[key]) != ci.digest(expected[key])]
    if differences:
        raise ValueError("Rust keyboard complete profile fields differ: " + repr(differences))
    return {"command": command, "exit_code": 0, "compared_product_editions": len(expected),
            "actual_records_sha256": ci.digest([actual[key] for key in sorted(actual)]),
            "expected_records_sha256": ci.digest([expected[key] for key in sorted(expected)]),
            "device_io": False, "official_dll_execution": False}


def prepare(*, check=False, rust_test_binary=None):
    import subprocess
    rendered = render_recipes()
    lines = ["//! Complete current keyboard profile and device recipes.",
             "//! Generated by the workspace typed-data workflow; values are local configuration.",
             "use super::{DeviceFields, Profile, RecipeIds};", "",
             "pub(super) fn profile(product: u32, edition: u8, ids: &RecipeIds) -> Option<Profile> {",
             "    Some(match (product, edition) {"]
    for expression, variants in rendered["profile_constructors"].items():
        patterns = [f"({pid}, _)" if pid == 691 else f"({pid}, {edition})" for pid, edition in variants]
        patterns = list(dict.fromkeys(patterns))
        lines.append(" | ".join(patterns) + " => " + expression + ",")
    lines += ["_ => return None,", "})", "}", "",
              "pub(super) fn device(product: u32, ids: &RecipeIds) -> Option<DeviceFields> {",
              "let mut fields = match product {"]
    for expression, products in rendered["device_constructors"].items():
        lines.append(" | ".join(map(str, products)) + " => " + expression + ",")
    lines += ["_ => return None,", "};", "fields.product_id = product;", "Some(fields)", "}", "",
              "pub(super) fn applications() -> Vec<Profile> {", "vec![",
              *[expression + "," for expression in rendered["application_constructors"]], "]", "}", ""]
    versions = rendered["recovery"]["schema_versions"]
    if set(versions) != {row["product_id"] for row in rendered["recovery"]["products"]}:
        raise ValueError("Keyboard recipe schema-version scope differs")
    lines += ["pub(super) fn schema_version(product: u32) -> Option<u32> {", "match product {"]
    for version in sorted(set(versions.values())):
        lines.append(" | ".join(str(pid) for pid in sorted(versions) if versions[pid] == version) + f" => Some({version}),")
    lines += ["_ => None,", "}", "}", "",
              "pub(super) fn dial_guid_count(product: u32) -> usize {", "match product {"]
    for row in rendered["recovery"]["products"]:
        if row["dynamic_guids"]:
            lines.append(f"{row['product_id']} => {row['dynamic_guids']},")
    lines += ["_ => 0,", "}", "}", ""]
    lines += ["pub(super) fn category(product: u32) -> Option<super::Category> {", "match product {"]
    for category, name in [("KEYBOARD", "Keyboard"), ("KEYPAD", "Keypad")]:
        products = [str(row["product_id"]) for row in rendered["recovery"]["products"] if row["device"]["category"] == category]
        lines.append(" | ".join(products) + f" => Some(super::Category::{name}),")
    lines += ["_ => None,", "}", "}", ""]
    raw = "\n".join(lines).encode()
    formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], input=raw,
                               capture_output=True, check=True).stdout
    destination = ROOT / OUTPUT
    if check:
        if not destination.is_file() or destination.read_bytes() != formatted:
            raise ValueError("Current keyboard typed profile recipes differ from their complete original inputs")
    else:
        destination.write_bytes(formatted)
    comparison = verify_rust_recipes(rust_test_binary, rendered["recovery"]) if rust_test_binary else None
    return {"output": OUTPUT, "bytes": len(formatted), "sha256": hashlib.sha256(formatted).hexdigest(),
            "products": 73, "profile_variants": len(rendered["profile_constructors"]),
            "device_variants": len(rendered["device_constructors"]), "application_profiles": 13,
            "check": check, "rust_comparison": comparison}
