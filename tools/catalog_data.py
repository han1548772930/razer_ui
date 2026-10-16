"""Maintain offline typed resource documents without compiling their literals.

The recovery reader accepts only the constructors emitted by rust_data. It
never evaluates Rust, JavaScript, a target binary or an arbitrary expression.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import sys

import rust_data

ROOT = rust_data.ROOT
FORMAT = 1
MAX_DOCUMENT = 64 * 1024 * 1024


def selected(row):
    # Device protocol, pairing and restore tables remain code-owned.
    return row["crate"] != "razer-device" and not (
        row["crate"] == "razer-model" and row["symbol"] != "CATALOG"
    ) and row["symbol"] not in {"TEXT_DATA", "KEYBOARD_MAPPING_TEXT_EMOJI_DATA"}


def parse_expression(parser, expression):
    raw = ("fn __catalog_expression() {" + expression + "}").encode("utf-8")
    tree = parser.parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Unsupported constructor syntax: " + expression[:150])
    body = tree.root_node.named_children[0].child_by_field_name("body")
    nodes = [n for n in body.named_children if n.type not in {"line_comment", "block_comment"}]
    if len(nodes) != 1:
        raise ValueError("Constructor must contain one expression")
    node = nodes[0]
    return node.named_children[0] if node.type == "expression_statement" else node


def text(node):
    return node.text.decode("utf-8")


def unstring(raw):
    raw = re.sub(r"\\u\{([0-9a-fA-F]+)\}", lambda m: chr(int(m[1], 16)), raw)
    return json.loads(raw)


class Recovery:
    def __init__(self, row, schema):
        self.row, self.schema = row, schema
        self.helpers, self.cache, self.inputs = {}, {}, []
        self.root = None
        pattern = re.compile(r"        fn (source_table_\d+)\(\) -> ([^\n{]+) \{\n(.*?)\n        \}\n", re.S)
        for item in row["outputs"]:
            path = ROOT / item["path"]
            raw = path.read_bytes()
            source = raw.decode("utf-8").replace("\r\n", "\n")
            self.inputs.append({"path": item["path"], "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
            for match in pattern.finditer(source):
                if match[1] in self.helpers:
                    raise ValueError("Duplicate constructor helper")
                self.helpers[match[1]] = (match[2].strip(), match[3])
            if item["path"] == row["output"]:
                source = pattern.sub("", source)
                start = source.index("() => {{") + len("() => {{")
                stop = source.rindex("    }};")
                self.root = re.sub(r"\$crate::data::\w+!\(\);", "", source[start:stop]).strip()
        if self.root is None:
            raise ValueError("No complete constructor root")

    def recover(self):
        node = parse_expression(self.schema.parser, self.root)
        if self.row.get("product_factory"):
            rows = []
            for entry in self.sequence(node):
                product, factory = entry.named_children
                pid = self.literal(product)
                # (|| Product{...}) as fn() -> Product
                while factory.type in {"type_cast_expression", "parenthesized_expression"}:
                    factory = factory.named_children[0]
                if factory.type != "closure_expression":
                    raise ValueError("Product factory must be a constructor closure")
                body = factory.child_by_field_name("body")
                value = self.value(body, self.row["root_type"][4:-1], ROOT / self.row["module"])
                if value.get("product_id", value.get("pid")) != pid:
                    raise ValueError("Factory identity differs from its product")
                rows.append(value)
            return rows
        return self.value(node, self.row["root_type"], ROOT / self.row["module"])

    def fingerprint(self):
        cache = {}
        def visit(node):
            if node.type == "call_expression":
                name = text(node.child_by_field_name("function"))
                if name in self.helpers:
                    if name not in cache:
                        cache[name] = visit(parse_expression(self.schema.parser, self.helpers[name][1]))
                    return cache[name]
            return expression_fingerprint(node, visit)
        return visit(parse_expression(self.schema.parser, self.root))

    def literal(self, node):
        source = text(node)
        if node.type == "string_literal":
            return unstring(source)
        if node.type == "boolean_literal":
            return source == "true"
        if node.type in {"integer_literal", "float_literal"}:
            source = re.sub(r"(?:[ui](?:8|16|32|64|128|size)|f(?:32|64))$", "", source).replace("_", "")
            return float(source) if node.type == "float_literal" else int(source, 0) if source.startswith("0x") else int(source)
        if node.type == "unary_expression" and source.startswith("-"):
            return -self.literal(node.named_children[0])
        if node.type == "parenthesized_expression":
            return self.literal(node.named_children[0])
        raise ValueError("Unsupported scalar constructor: " + source[:150])

    def sequence(self, node):
        if node.type == "macro_invocation" and text(node.child_by_field_name("macro")) == "vec":
            token = node.named_children[-1]
            parsed = parse_expression(self.schema.parser, "[" + text(token)[1:-1] + "]")
            return parsed.named_children
        if node.type in {"array_expression", "tuple_expression"}:
            return node.named_children
        raise ValueError("Unsupported collection constructor: " + text(node)[:150])

    def value(self, node, typ, module):
        typ = self.schema.aliases.get((module, typ.strip()), typ.strip())
        source = text(node)
        if node.type == "parenthesized_expression":
            return self.value(node.named_children[0], typ, module)
        if node.type == "call_expression":
            function, arguments = node.child_by_field_name("function"), node.child_by_field_name("arguments")
            name = text(function)
            if name in self.helpers:
                key = (name, typ, str(module))
                if key not in self.cache:
                    _, expression = self.helpers[name]
                    self.cache[key] = self.value(parse_expression(self.schema.parser, expression), typ, module)
                return self.cache[key]
            if name in {"Default::default", "default_max_pan_tilt"}:
                return _DEFAULT
            if function.type == "field_expression":
                method = text(function.child_by_field_name("field"))
                owner = function.child_by_field_name("value")
                if method in {"into", "as_slice"}:
                    return self.value(owner, typ, module)
                if method == "collect":
                    if owner.type == "call_expression":
                        iteration = owner.child_by_field_name("function")
                        if iteration.type == "field_expression" and text(iteration.child_by_field_name("field")) == "into_iter":
                            owner = iteration.child_by_field_name("value")
                    args = rust_data.split_types(re.fullmatch(r"[\w:]+<(.*)>", typ, re.S)[1])
                    values = {str(self.value(pair.named_children[0], args[0], module)):
                              self.value(pair.named_children[1], args[1], module) for pair in self.sequence(owner)}
                    return {key: None if child is _ABSENT else child for key, child in values.items()}
        generic = re.fullmatch(r"([\w:]+)\s*<(.*)>", typ, re.S)
        if generic:
            base, args = generic[1].rsplit("::", 1)[-1], rust_data.split_types(generic[2])
            if base == "Option":
                if source == "None":
                    return _ABSENT
                if node.type == "call_expression" and text(node.child_by_field_name("function")) == "Some":
                    return self.value(node.child_by_field_name("arguments").named_children[0], args[0], module)
            if base in {"Vec", "BTreeSet", "HashSet"}:
                return [self.value(child, args[0], module) for child in self.sequence(node)]
            if base in {"Box", "Arc", "Cow"}:
                return self.value(node.child_by_field_name("arguments").named_children[0], args[-1], module)
        if typ == "()":
            return None
        if typ.startswith("[") or typ.startswith("("):
            args = rust_data.split_types(typ[1:-1])
            if typ.startswith("["):
                return [self.value(child, args[0], module) for child in self.sequence(node)]
            return [self.value(child, arg, module) for child, arg in zip(self.sequence(node), args)]
        if typ in {"String", "std::string::String", "SharedString", "&str", "&'static str", "bool"} or re.fullmatch(r"[uif](8|16|32|64|128|size)", typ):
            return self.literal(node)
        if typ in {"Value", "serde_json::Value"}:
            return self.json_value(node, module)
        observed = None
        if node.type == "struct_expression":
            observed = {text(f.child_by_field_name("field")): None for f in node.child_by_field_name("body").named_children if f.type == "field_initializer"}
        definition = self.schema.definition(typ, module, observed)
        mode = rust_data.attribute(definition.attrs, "rename_all")
        if definition.kind == "enum_item":
            function = text(node.child_by_field_name("name")) if node.type == "struct_expression" else text(node.child_by_field_name("function")) if node.type == "call_expression" else source
            variant = function.rsplit("::", 1)[-1]
            fields = next((row for row in definition.fields if row[0] == variant), None)
            if fields is None:
                raise ValueError("Unknown enum constructor: " + source[:100])
            name, body, attrs = fields
            label = rust_data.attribute(attrs, "rename") or rust_data.rename(name, mode)
            tag, content = rust_data.attribute(definition.attrs, "tag"), rust_data.attribute(definition.attrs, "content")
            if body is None:
                return {tag: label} if tag else label
            if body.type == "ordered_field_declaration_list":
                args = [text(c) for c in body.named_children if c.type != "attribute_item"]
                values = [self.value(child, arg, definition.module) for child, arg in zip(node.child_by_field_name("arguments").named_children, args)]
                payload = values[0] if len(args) == 1 else values
            else:
                raw_fields = {text(f.child_by_field_name("field")): f.child_by_field_name("value") for f in node.child_by_field_name("body").named_children}
                payload = {text(f.child_by_field_name("name")): self.value(raw_fields[text(f.child_by_field_name("name"))], text(f.child_by_field_name("type")), definition.module) for f in body.named_children if f.type == "field_declaration"}
            if "untagged" in definition.attrs:
                return payload
            return {tag: label, content: payload} if content else {tag: label, **payload}
        if node.type != "struct_expression":
            raise ValueError("Expected record constructor " + typ + ": " + source[:150])
        raw_fields = {text(f.child_by_field_name("field")): f.child_by_field_name("value") for f in node.child_by_field_name("body").named_children if f.type == "field_initializer"}
        if set(raw_fields) != {name for name, _, _ in definition.fields}:
            raise ValueError("Incomplete constructor fields for " + typ)
        value = {}
        for name, field_type, attrs in definition.fields:
            default = rust_data.attribute(attrs, "default")
            if default and text(raw_fields[name]) == default + "()":
                continue
            child = self.value(raw_fields[name], field_type, definition.module)
            if child is _ABSENT or child is _DEFAULT:
                continue
            if "flatten" in attrs:
                value.update(child)
            else:
                value[rust_data.attribute(attrs, "rename") or rust_data.rename(name.removeprefix("r#"), mode)] = child
        return value

    def json_value(self, node, module):
        source = text(node)
        if source == "serde_json::Value::Null":
            return None
        if node.type == "call_expression":
            function = text(node.child_by_field_name("function"))
            args = node.child_by_field_name("arguments").named_children
            if function in self.helpers:
                return self.value(node, "serde_json::Value", module)
            if function == "serde_json::Value::Array":
                return [self.value(child, "serde_json::Value", module) for child in self.sequence(args[0])]
            if function == "serde_json::Value::Object":
                return self.value(args[0], "std::collections::BTreeMap<String,serde_json::Value>", module)
            if function in {"serde_json::Value::Bool", "serde_json::Value::String", "serde_json::Value::Number"}:
                node = args[0]
                while node.type == "call_expression":
                    fn = node.child_by_field_name("function")
                    if fn.type == "field_expression":
                        method = text(fn.child_by_field_name("field"))
                        node = fn.child_by_field_name("value")
                        if method == "expect":
                            continue
                    elif text(fn) == "serde_json::Number::from_f64":
                        node = node.child_by_field_name("arguments").named_children[0]
                    else:
                        break
                return self.literal(node)
        raise ValueError("Unsupported external value constructor: " + source[:150])


_ABSENT, _DEFAULT = object(), object()


def expression_fingerprint(node, visit):
    if node.type == "parenthesized_expression":
        return visit(node.named_children[0])
    # vec! is a token tree in tree-sitter; parse its actual ordered expressions.
    if node.type == "macro_invocation" and text(node.child_by_field_name("macro")) == "vec":
        token = node.named_children[-1]
        parser = _FINGERPRINT_PARSER
        children = parse_expression(parser, "[" + text(token)[1:-1] + "]").named_children
        return hashlib.sha256(b"vec" + b"".join(visit(child) for child in children)).digest()
    children = node.named_children
    body = b"".join(visit(child) for child in children) if children else node.text if node.type in {"string_literal", "string_content"} else re.sub(rb"\s+", b"", node.text)
    return hashlib.sha256(node.type.encode("ascii") + b"\0" + body).digest()


_FINGERPRINT_PARSER = None


class Fingerprint(rust_data.RustData):
    def _render(self, value, typ, module, path, allow_value):
        expression = super()._render_inner(value, typ, module, path, allow_value)
        return self.hash_expression(expression)

    def _boundary(self, value):
        return self.hash_expression(rust_data.json_boundary(value, self._boundary))

    def hash_expression(self, expression):
        def visit(node):
            if node.type == "identifier" and text(node).startswith("__catalog_hash_"):
                return bytes.fromhex(text(node).removeprefix("__catalog_hash_"))
            return expression_fingerprint(node, visit)
        return "__catalog_hash_" + visit(parse_expression(self.parser, expression)).hex()


class Codec(rust_data.RustData):
    """Generate typed constructors once per shape; values live in a deduplicated DAG."""
    def begin(self):
        self.expression_pool = None
        self.decoders, self.frames, self.nodes, self.pool = {}, [], [], {}
        self.excluded, self.value_fields, self.shared_inputs = set(), set(), {}

    def _render(self, value, typ, module, path, allow_value):
        typ = self.aliases.get((module, typ.strip()), typ.strip())
        key = (str(module), typ)
        decoder = self.decoders.get(key)
        if decoder is None:
            decoder = self.decoders[key] = {"name": "decode_" + str(len(self.decoders)),
                                           "type": self.constructor_type(typ, module, value), "cases": []}
        frame = {"bytes": bytearray(), "children": []}
        self.frames.append(frame)
        try:
            template = self.template(value, typ, module, path, allow_value)
        finally:
            self.frames.pop()
        if template not in decoder["cases"]:
            decoder["cases"].append(template)
        alternative = decoder["cases"].index(template)
        scalar = struct.pack("<H", alternative) + bytes(frame["bytes"])
        node_key = (key, scalar, tuple(frame["children"]))
        node_id = self.pool.get(node_key)
        if node_id is None:
            node_id = len(self.nodes)
            self.pool[node_key] = node_id
            self.nodes.append((scalar, frame["children"]))
        if self.frames:
            self.frames[-1]["children"].append(node_id)
        self.last_node = node_id
        return decoder["name"]

    def template(self, value, typ, module, path, allow_value):
        data = self.frames[-1]["bytes"]
        formats = {"u8": "B", "u16": "H", "u32": "I", "u64": "Q", "usize": "Q",
                   "i8": "b", "i16": "h", "i32": "i", "i64": "q", "isize": "q", "f32": "f", "f64": "d"}
        if typ in formats:
            # Reuse the strict source/range validation before encoding.
            super()._render_inner(value, typ, module, path, allow_value)
            data.extend(struct.pack("<" + formats[typ], value))
            return "r." + typ + "()?"
        if typ == "bool":
            super()._render_inner(value, typ, module, path, allow_value)
            data.append(int(value))
            return "r.boolean()?"
        if typ in {"String", "std::string::String", "SharedString"}:
            super()._render_inner(value, typ, module, path, allow_value)
            data.extend(value.encode("utf-8"))
            return "r.string()?.into()"
        generic = re.fullmatch(r"([\w:]+)\s*<(.*)>", typ, re.S)
        if generic and generic[1].rsplit("::", 1)[-1] in {"Vec", "BTreeSet", "HashSet", "BTreeMap", "HashMap"}:
            base, args = generic[1].rsplit("::", 1)[-1], rust_data.split_types(generic[2])
            if base in {"BTreeMap", "HashMap"}:
                if type(value) is not dict:
                    raise ValueError("Expected map at " + path)
                names = []
                for k, v in value.items():
                    k = int(k) if re.fullmatch(r"[ui](8|16|32|64|128|size)", args[0]) else k
                    pair = [self._render(k, args[0], module, path + ".key", allow_value),
                            self._render(v, args[1], module, path + ".*", allow_value)]
                    if names and names != pair:
                        raise ValueError("Inconsistent map type")
                    names = pair
                if not names:
                    return "Default::default()"
                return "r.map(" + ",".join(names) + ")?"
            if type(value) is not list:
                raise ValueError("Expected ordered list at " + path)
            names = [self._render(v, args[0], module, path + "[]", allow_value) for v in value]
            if not names:
                return "Default::default()"
            if len(set(names)) != 1:
                raise ValueError("Inconsistent sequence type")
            result = "r.sequence(" + names[0] + ")?"
            return result if base == "Vec" else result + ".into_iter().collect()"
        if typ != "()" and typ.startswith("[") and typ.endswith("]"):
            args = rust_data.split_types(typ[1:-1])
            if len(args) != 2 or len(value) != int(args[1]):
                raise ValueError("Invalid fixed array")
            names = [self._render(v, args[0], module, path + "[]", allow_value) for v in value]
            return "r.array::<_," + args[1] + ">(" + names[0] + ")?"
        if typ in {"Value", "serde_json::Value"}:
            if not allow_value:
                raise ValueError("External value requires an explicit boundary")
            self.value_fields.add(path)
            self.encode_value(value, data)
            return "r.external_value()?"
        # Existing schema rules select exact enum variants, defaults and optional
        # values. Child calls consume individual nodes, so those choices are not
        # inferred again by an untagged serde deserializer at runtime.
        original = super()._render_inner(value, typ, module, path, allow_value)
        return re.sub(r"\b(decode_\d+)\b(?!\()", r"r.child(\1)?", original)

    def encode_value(self, value, data):
        if value is None:
            data.append(0)
        elif type(value) is bool:
            data.extend((1, int(value)))
        elif type(value) is int:
            data.append(2 if value < 0 else 3)
            data.extend(struct.pack("<q" if value < 0 else "<Q", value))
        elif type(value) is float:
            if not __import__("math").isfinite(value):
                raise ValueError("Non-finite external number")
            data.append(4)
            data.extend(struct.pack("<d", value))
        elif type(value) is str:
            raw = value.encode("utf-8")
            data.append(5)
            data.extend(struct.pack("<I", len(raw)))
            data.extend(raw)
        elif type(value) is list:
            data.append(6)
            data.extend(struct.pack("<I", len(value)))
            for item in value:
                self.encode_value(item, data)
        elif type(value) is dict:
            data.append(7)
            data.extend(struct.pack("<I", len(value)))
            for key, item in value.items():
                self.encode_value(key, data)
                self.encode_value(item, data)
        else:
            raise ValueError("Invalid external interchange value")

    def document(self, row, value):
        self.begin()
        module = ROOT / row["module"]
        roots = []
        if row.get("product_factory"):
            typ = re.fullmatch(r"Vec<(.*)>", row["root_type"])[1]
            seen = set()
            for item in value:
                pid = item.get("product_id", item.get("pid"))
                if type(pid) is not int or not 0 < pid < 2**32 or pid in seen:
                    raise ValueError("Invalid product identity")
                seen.add(pid)
                decoder = self._render(item, typ, module, "$", row.get("allow_value", False))
                roots.append((pid, self.pool_last()))
        else:
            typ = row["root_type"]
            decoder = self._render(value, typ, module, "$", row.get("allow_value", False))
            roots.append((0, self.pool_last()))
        code = []
        for definition in self.decoders.values():
            cases = ",\n".join(str(i) + " => {" + expression + "}" for i, expression in enumerate(definition["cases"]))
            code.append("fn " + definition["name"] + "(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<" + definition["type"] + "> {\n"
                        + "let value = match r.tag()? {" + cases + ', _ => anyhow::bail!("Invalid typed catalog alternative")};\n'
                        + "r.finish()?; Ok(value)\n}\n")
        code = "".join(code)
        codec = hashlib.sha256(code.encode("utf-8")).hexdigest()
        raw = bytearray(b"RZTDAG01")
        raw.extend(struct.pack("<II", len(self.nodes), len(roots)))
        for pid, node in roots:
            raw.extend(struct.pack("<II", pid, node))
        for scalar, children in self.nodes:
            raw.extend(struct.pack("<II", len(scalar), len(children)))
            raw.extend(scalar)
            for child in children:
                raw.extend(struct.pack("<I", child))
        if len(raw) > MAX_DOCUMENT:
            raise ValueError("Typed catalog document exceeds the bounded reader")
        return {"id": row["crate"] + "/" + row["macro"], "codec": codec, "raw": bytes(raw), "code": code,
                "decoder": decoder, "type": typ, "roots": roots, "nodes": len(self.nodes)}

    def pool_last(self):
        # A root may reuse an earlier node; track the lookup rather than assuming
        # the last newly allocated node is the result.
        return self.last_node


def recover_inputs(rows):
    schemas, reports = {}, []
    for row in rows:
        path = ROOT / row["source"]
        if path.is_file() and row.get("maintenance_recovery"):
            continue
        schema = schemas.setdefault(row["crate"], None)
        if schema is None:
            schema = schemas[row["crate"]] = rust_data.RustData(ROOT / "crates" / row["crate"])
        recovery = Recovery(row, schema)
        value = recovery.recover()
        raw = (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.is_file() and path.read_bytes() != raw:
            raise ValueError("Existing maintenance input differs from the complete constructor projection: " + row["source"])
        path.write_bytes(raw)
        row["maintenance_recovery"] = {"kind": "complete_current_rust_constructor_projection", "original_source_sha256": row["source_sha256"], "inputs": recovery.inputs,
                                       "limits": "Excluded evidence fields are not recovered; this is a storage equivalence input, not newly acquired vendor evidence."}
        reports.append({"crate": row["crate"], "symbol": row["symbol"], "bytes": len(raw)})
        print(json.dumps({"recovered": row["crate"] + "/" + row["macro"], "bytes": len(raw)}), flush=True)
    return reports


def verify_constructors(row, value, schema):
    global _FINGERPRINT_PARSER
    _FINGERPRINT_PARSER = schema.parser
    before = Recovery(row, schema).fingerprint()
    if row.get("product_factory"):
        typ = re.fullmatch(r"Vec<(.*)>", row["root_type"])[1]
        expressions = []
        for item in value:
            pid = item.get("product_id", item.get("pid"))
            body = schema.render(item, typ, row["module"], allow_value=row.get("allow_value", False))
            expressions.append(f"({pid}u32, (|| {body}) as fn() -> {typ})")
        after = schema.hash_expression("vec![" + ",".join(expressions) + "]")
    else:
        after = schema.render(value, row["root_type"], row["module"], allow_value=row.get("allow_value", False))
    if before.hex() != after.removeprefix("__catalog_hash_"):
        raise ValueError("Recovered input does not reproduce every typed constructor: " + row["crate"] + "/" + row["macro"])
    return before.hex()


def constant(row):
    return (row["crate"] + "_" + row["macro"]).upper().replace("-", "_")


def reader_name(row):
    return "read_" + row["macro"].removesuffix("_data") + "_catalog"


def format_reader(row, body, return_type):
    marker = "// Generated catalog reader: " + row["crate"] + "/" + row["macro"] + ".\n"
    source = marker + "fn " + reader_name(row) + "() -> " + return_type + " {\n" + body + "\n}\n"
    test = "packaged_catalog_" + row["macro"] + "_decodes"
    verification = ("for (_, factory) in super::" + reader_name(row) + "() { let _ = factory(); }") if row.get("product_factory") else ("let _ = super::" + reader_name(row) + "();")
    source += ("\n#[cfg(test)]\nmod " + row["macro"] + "_catalog_tests {\n"
               + "#[test]\nfn " + test + "() { " + verification + " }\n}\n")
    source += "// End generated catalog reader: " + row["crate"] + "/" + row["macro"] + ".\n"
    formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"],
                               input=source.encode("utf-8"), stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=ROOT, check=False)
    if formatted.returncode:
        raise ValueError("Cannot format typed catalog reader: " + formatted.stderr.decode("utf-8", errors="replace"))
    return formatted.stdout.replace(b"\r\n", b"\n")


def place_reader(artifacts, row, body):
    path = ROOT / row["module"]
    source = artifacts.get(path, path.read_bytes()).decode("utf-8").replace("\r\n", "\n")
    marker = "// Generated catalog reader: " + row["crate"] + "/" + row["macro"] + ".\n"
    ending = "// End generated catalog reader: " + row["crate"] + "/" + row["macro"] + ".\n"
    pattern = re.compile(re.escape(marker) + (r".*?" + re.escape(ending) if ending in source else r".*?\n}\n"), re.S)
    if marker in source:
        source, count = pattern.subn(lambda _: body.decode("utf-8"), source)
        if count != 1:
            raise ValueError("Catalog reader identity is repeated")
    else:
        if re.search(r"\bfn\s+" + re.escape(reader_name(row)) + r"\b", source):
            raise ValueError("Catalog reader conflicts with a handwritten function")
        source = source.rstrip() + "\n\n" + body.decode("utf-8")
    artifacts[path] = source.encode("utf-8")


def replace_macro_calls(artifacts, rows):
    by_crate = {}
    for row in rows:
        by_crate.setdefault(row["crate"], []).append(row)
    for crate, readers in by_crate.items():
        for path in (ROOT / "crates" / crate / "src").rglob("*.rs"):
            if "/src/data/" in path.as_posix():
                continue
            source = artifacts.get(path, path.read_bytes()).decode("utf-8").replace("\r\n", "\n")
            changed = False
            for row in readers:
                expression = (r"crate::data::(?:" + re.escape(row["macro"]) + r"::)?"
                              + re.escape(row["macro"]) + r"!\(\s*\)")
                replacement = ("" if path == ROOT / row["module"] else "super::") + reader_name(row) + "()"
                source, count = re.subn(expression, replacement, source)
                changed |= count > 0
            if changed:
                artifacts[path] = source.encode("utf-8")


def adapter(row, document):
    load = ('razer_resource_db::read_document(razer_resource_db::catalogs::' + constant(row)
            + ', ROOT_ID, "' + document["codec"] + '", ' + document["decoder"]
            + ').expect("Packaged typed catalog must match its domain schema")')
    code = document["code"]
    if row.get("product_factory"):
        entries = []
        for pid, _ in document["roots"]:
            function = "product_" + str(pid)
            code += "fn " + function + "() -> " + document["type"] + " {" + load.replace("ROOT_ID", str(pid) + "u32") + "}\n"
            entries.append("(" + str(pid) + "u32," + function + " as fn() -> " + document["type"] + ")")
        result = "vec![" + ",".join(entries) + "]"
    else:
        result = load.replace("ROOT_ID", "0")
    return_type = "Vec<(u32, fn() -> " + document["type"] + ")>" if row.get("product_factory") else row["root_type"]
    return format_reader(row, code + result, return_type)


def prepare_documents(recipes, *, migrate=False, check=False):
    rows = [row for row in recipes["tables"] if selected(row)]
    if migrate:
        for row in rows:
            row["source"] = rust_data.INPUTS + "/" + row["crate"] + "/" + row["macro"] + ".json"
        recover_inputs(rows)
    documents, artifacts, reports, codecs, fingerprints = [], {}, [], {}, {}
    for row in rows:
        source = ROOT / row["source"]
        raw = source.read_bytes()
        value = json.loads(raw)
        if row.get("storage") != "sqlite":
            if not migrate:
                raise ValueError("Migrate complete typed catalogs with prepare-resource-database --migrate-data")
            fingerprint = fingerprints.get(row["crate"])
            if fingerprint is None:
                fingerprint = fingerprints[row["crate"]] = Fingerprint(ROOT / "crates" / row["crate"])
            constructor_hash = verify_constructors(row, value, fingerprint)
            if "maintenance_recovery" not in row:
                row["maintenance_recovery"] = {"kind": "current_rust_constructor_projection", "original_source_sha256": row["source_sha256"],
                                                "inputs": Recovery(row, fingerprint).inputs,
                                                "limits": "Excluded evidence fields are not recovered; no new vendor-source provenance is implied."}
            row["maintenance_recovery"]["full_constructor_fingerprint"] = constructor_hash
        codec = codecs.get(row["crate"])
        if codec is None:
            codec = codecs[row["crate"]] = Codec(ROOT / "crates" / row["crate"])
        document = codec.document(row, value)
        body = adapter(row, document)
        place_reader(artifacts, row, body)
        documents.append(document)
        row.update(storage="sqlite", output=row["module"], reader=reader_name(row), source_sha256=hashlib.sha256(raw).hexdigest(),
                   outputs=[{"path": row["module"], "reader": reader_name(row), "sha256": hashlib.sha256(body).hexdigest()}],
                   catalog_id=document["id"], codec=document["codec"], passed=True)
        reports.append({"id": document["id"], "source": row["source"], "source_sha256": row["source_sha256"],
                        "payload_bytes": len(document["raw"]), "nodes": document["nodes"], "roots": len(document["roots"]),
                        "adapter_bytes": len(body), "codec": document["codec"],
                        "constructor_fingerprint": row["maintenance_recovery"]["full_constructor_fingerprint"]})
        print(json.dumps({"typed_catalog": document["id"], "bytes": len(document["raw"]), "adapter_bytes": len(body)}), flush=True)
    ids = "// Generated stable packaged catalog identities.\nuse crate::CatalogId;\n"
    ids += "".join("pub const " + constant(row) + ': CatalogId = CatalogId("' + row["catalog_id"] + '");\n' for row in rows)
    artifacts[ROOT / "crates/razer-resource-db/src/catalogs.rs"] = ids.encode("utf-8")
    profiles = [row for row in recipes["tables"] if row["symbol"] in {"TEXT_DATA", "KEYBOARD_MAPPING_TEXT_EMOJI_DATA"}]
    for row in profiles:
        profile = "Macro" if row["symbol"] == "TEXT_DATA" else "Mapping"
        body = format_reader(row, row["root_type"] + "::from_catalog(razer_catalog::emoji::DatasetId::" + profile + ")", row["root_type"])
        place_reader(artifacts, row, body)
        row.update(storage="sqlite-profile", output=row["module"], reader=reader_name(row),
                   outputs=[{"path": row["module"], "reader": reader_name(row), "sha256": hashlib.sha256(body).hexdigest()}])
    replace_macro_calls(artifacts, rows + profiles)
    for path, body in artifacts.items():
        if check and path.read_bytes() != body:
            raise ValueError("Typed SQLite adapter differs from maintained schema: " + str(path.relative_to(ROOT)))
    return documents, artifacts, reports


def publish_adapters(artifacts, recipes, *, check=False):
    for path, body in artifacts.items():
        if not check and (not path.is_file() or path.read_bytes() != body):
            path.write_bytes(body)
    for row in recipes["tables"]:
        if row.get("storage") not in {"sqlite", "sqlite-profile"}:
            continue
        directory = ROOT / "crates" / row["crate"] / "src/data"
        for stale in [directory / (row["macro"] + ".rs"), *directory.glob(row["macro"] + "_part_*.rs")]:
            if not stale.exists():
                continue
            if check:
                raise ValueError("Superseded catalog macro is still present: " + stale.relative_to(ROOT).as_posix())
            if stale.resolve().parent != directory.resolve() or not stale.read_bytes().startswith(b"// Generated"):
                raise ValueError("Catalog cleanup target is not an owned generated file")
            stale.unlink()
    for crate in {row["crate"] for row in recipes["tables"] if row.get("storage") in {"sqlite", "sqlite-profile"}}:
        source = ROOT / "crates" / crate / "src"
        directory = source / "data"
        registry = source / "data.rs"
        if directory.exists() and not any(directory.iterdir()):
            if check:
                raise ValueError("Empty superseded catalog directory remains")
            directory.rmdir()
            registry.unlink()
            lib = source / "lib.rs"
            lib.write_bytes(lib.read_bytes().replace(b"mod data;\r\n", b"").replace(b"mod data;\n", b""))
        elif registry.is_file():
            raw = registry.read_text(encoding="utf-8")
            for row in recipes["tables"]:
                if row["crate"] == crate and row.get("storage") in {"sqlite", "sqlite-profile"}:
                    raw = re.sub(r'^include!\("data/' + re.escape(row["macro"]) + r'\.rs"\);\n', '', raw, flags=re.M)
            if check and registry.read_text(encoding="utf-8") != raw:
                raise ValueError("Superseded catalog registry entry remains: " + registry.relative_to(ROOT).as_posix())
            if not check and registry.read_text(encoding="utf-8") != raw:
                registry.write_text(raw, encoding="utf-8", newline="\n")
