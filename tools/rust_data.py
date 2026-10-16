"""Emit Rust domain constructors from audited inputs and their actual Rust schema.

This maintenance module is never imported by Cargo or the product. Generated
macros expand at the owning consumer, where private domain types are visible.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import re

from tree_sitter import Language, Parser
import tree_sitter_rust

ROOT = Path(__file__).resolve().parent.parent
INPUTS = ".analysis/inputs/typed-tables"


def ancestors(node):
    parent = node.parent
    while parent is not None:
        yield parent
        parent = parent.parent


def rust_string(value):
    text = json.dumps(value, ensure_ascii=False)
    text = text.replace("\\b", "\\u{8}").replace("\\f", "\\u{c}")
    return re.sub(r"\\u([0-9a-fA-F]{4})", lambda match: "\\u{" + match[1] + "}", text)


def split_types(text):
    parts, start, depth = [], 0, 0
    for ix, char in enumerate(text):
        if char in "<([":
            depth += 1
        elif char in ">)]":
            depth -= 1
        elif char in ",;" and depth == 0:
            parts.append(text[start:ix].strip())
            start = ix + 1
    if text[start:].strip():
        parts.append(text[start:].strip())
    return parts


def rename(value, mode):
    words = re.findall(r"[A-Z]+(?=[A-Z][a-z]|$)|[A-Z]?[a-z]+|[0-9]+", value)
    if mode == "snake_case":
        return "_".join(word.lower() for word in words)
    if mode == "SCREAMING_SNAKE_CASE":
        return "_".join(word.upper() for word in words)
    if mode == "kebab-case":
        return "-".join(word.lower() for word in words)
    if mode == "camelCase":
        return words[0].lower() + "".join(word.title() for word in words[1:])
    if mode == "PascalCase":
        return "".join(word.title() for word in words)
    if mode == "lowercase":
        return value.lower()
    if mode == "UPPERCASE":
        return value.upper()
    if mode:
        raise ValueError("Unsupported serde rename mode: " + mode)
    return value


def attribute(attrs, name):
    match = re.search(r"\b" + re.escape(name) + r'\s*=\s*"([^"\\]+)"', attrs)
    return match[1] if match else None


def json_boundary(value):
    """Construct an explicitly allowed interchange value without recursive macros."""
    if value is None:
        return "serde_json::Value::Null"
    if type(value) is bool:
        return "serde_json::Value::Bool(" + str(value).lower() + ")"
    if type(value) is str:
        return "serde_json::Value::String(" + rust_string(value) + ".into())"
    if type(value) is int:
        kind = "i64" if value < 0 else "u64"
        low, high = (-2**63, 2**63 - 1) if value < 0 else (0, 2**64 - 1)
        if not low <= value <= high:
            raise ValueError("Interchange integer is outside serde_json::Number")
        return "serde_json::Value::Number(" + str(value) + kind + ".into())"
    if type(value) is float:
        if not math.isfinite(value):
            raise ValueError("Interchange number must be finite")
        return ("serde_json::Value::Number(serde_json::Number::from_f64("
                + repr(value) + "f64).expect(\"finite interchange number\"))")
    if type(value) is list:
        return "serde_json::Value::Array(vec![" + ",".join(json_boundary(item) for item in value) + "])"
    if type(value) is dict:
        return ("serde_json::Value::Object([" + ",".join(
            "(" + rust_string(key) + ".into()," + json_boundary(item) + ")"
            for key, item in value.items()) + "].into_iter().collect())")
    raise ValueError("Invalid interchange value")


def pretty_expression(expression, *, indent=0, width=100):
    """Format generated constructors with bounded memory and string-safe tokens."""
    pairs = {"(": ")", "[": "]", "{": "}"}

    def compact_end(start, budget):
        stack = []
        stop = min(len(expression), start + max(0, budget))
        position = start
        while position < stop:
            char = expression[position]
            if char == '"':
                position += 1
                while position < stop and expression[position] != '"':
                    position += 2 if expression[position] == "\\" else 1
                if position >= stop:
                    return None
            elif char in pairs:
                stack.append(pairs[char])
            elif char in ")]}":
                if not stack or stack.pop() != char:
                    raise ValueError("Unbalanced generated Rust expression")
                if not stack:
                    return position + 1
            position += 1
        return None

    chunks, stack, position, column = [" " * indent], [], 0, indent
    tokens = re.finditer(r'"(?:[^"\\]|\\.)*"|[()\[\]{},]', expression)

    def append(value):
        nonlocal column
        chunks.append(value)
        column = len(value.rsplit("\n", 1)[-1]) if "\n" in value else column + len(value)

    def newline(level):
        nonlocal column
        while chunks and chunks[-1] == "":
            chunks.pop()
        if chunks and chunks[-1].startswith("\n") and not chunks[-1].strip():
            chunks[-1] = "\n" + " " * level
            column = level
        else:
            append("\n" + " " * level)

    for match in tokens:
        if match.start() < position:
            continue
        append(re.sub(r"(?<!:):(?!:)", ": ", expression[position:match.start()]))
        token = match[0]
        position = match.end()
        if token.startswith('"'):
            append(token)
        elif token in pairs:
            end = compact_end(match.start(), width - column)
            if end is not None:
                append(expression[match.start():end])
                position = end
                continue
            append(token)
            stack.append((pairs[token], indent))
            indent += 4
            newline(indent)
        elif token in ")]}":
            if not stack or stack[-1][0] != token:
                raise ValueError("Unbalanced generated Rust expression")
            _, indent = stack.pop()
            newline(indent)
            append(token)
        else:
            append(",")
            newline(indent)
    append(expression[position:])
    if stack:
        raise ValueError("Unclosed generated Rust expression")
    return "".join(chunks)


@dataclass
class Definition:
    name: str
    module: Path
    kind: str
    attrs: str
    fields: list


class RustData:
    def __init__(self, crate_root):
        self.crate_root = Path(crate_root).resolve()
        self.parser = Parser(Language(tree_sitter_rust.language()))
        self.definitions = {}
        self.aliases = {}
        self.excluded = set()
        self.value_fields = set()
        self.external_roots = set()
        self.expression_pool = None
        self.shared_constructors = []
        for path in sorted((self.crate_root / "src").rglob("*.rs")):
            if "/src/data/" in path.as_posix() and not path.name.endswith("_schema.rs"):
                continue
            self._parse(path)

    def _parse(self, path):
        raw = path.read_bytes()
        root = self.parser.parse(raw).root_node

        def scan(parent):
            attrs = []
            for node in parent.named_children:
                if node.type == "attribute_item":
                    attrs.append(node.text.decode("utf-8"))
                    continue
                metadata = " ".join(attrs)
                attrs = []
                if node.type in ("struct_item", "enum_item"):
                    name = node.child_by_field_name("name").text.decode()
                    body = node.child_by_field_name("body")
                    fields, field_attrs = [], []
                    if body:
                        for field in body.named_children:
                            if field.type == "attribute_item":
                                field_attrs.append(field.text.decode("utf-8"))
                                continue
                            if field.type == "field_declaration":
                                fields.append((field.child_by_field_name("name").text.decode(),
                                               field.child_by_field_name("type").text.decode(),
                                               " ".join(field_attrs)))
                            elif field.type == "enum_variant":
                                fields.append((field.child_by_field_name("name").text.decode(),
                                               field.child_by_field_name("body"), " ".join(field_attrs)))
                            field_attrs = []
                    definition = Definition(name, path, node.type, metadata, fields)
                    self.definitions.setdefault(name, []).append(definition)
                elif node.type == "type_item" and parent.type in ("source_file", "declaration_list") and not any(ancestor.type in ("impl_item", "trait_item") for ancestor in ancestors(node)):
                    name, value = node.child_by_field_name("name"), node.child_by_field_name("type")
                    if name and value:
                        self.aliases[(path, name.text.decode())] = value.text.decode()
                if node.type not in ("struct_item", "enum_item", "macro_definition"):
                    scan(node)
        scan(root)

    def definition(self, name, module, value):
        namespace = name.split("::", 1)[0] if "::" in name else None
        if namespace is None:
            imported = re.search(r"use\s+([\w_]+)::" + re.escape(name) + r"\s*;", module.read_text(encoding="utf-8"))
            if imported:
                namespace = imported[1]
        if namespace:
            original_namespace = namespace
            source = module.read_text(encoding="utf-8")
            alias = re.search(r"use\s+([\w_]+)\s+as\s+" + re.escape(namespace) + r"\s*;", source)
            if alias:
                namespace = alias[1]
            external = ROOT / "crates" / namespace.replace("_", "-")
            if external.is_dir() and external != self.crate_root and external not in self.external_roots:
                self.external_roots.add(external)
                for path in sorted((external / "src").rglob("*.rs")):
                    if "/src/data/" not in path.as_posix():
                        self._parse(path)
        candidates = self.definitions.get(name.rsplit("::", 1)[-1], [])
        local = [item for item in candidates if item.module == module]
        if len(local) == 1:
            return local[0]
        candidates = local or candidates
        if len(candidates) == 1:
            return candidates[0]
        if isinstance(value, dict) and candidates:
            scored = [(len(set(value) & {attribute(f[2], "rename") or f[0] for f in item.fields}), ix, item)
                      for ix, item in enumerate(candidates)]
            scored.sort(key=lambda item: item[:2], reverse=True)
            if len(scored) == 1 or scored[0][0] > scored[1][0]:
                return scored[0][2]
        raise ValueError(f"Ambiguous or missing Rust type {name} in {module.relative_to(ROOT)}")

    def render(self, value, root_type, module_path, *, allow_value=False):
        module = Path(module_path)
        module = module.resolve() if module.is_absolute() else (ROOT / module).resolve()
        self.excluded.clear()
        self.value_fields.clear()
        return self._render(value, root_type, module, "$", allow_value)

    def constructor_type(self, typ, module, value):
        typ = self.aliases.get((module, typ.strip()), typ.strip())
        generic = re.fullmatch(r"([\w:]+)\s*<(.*)>", typ, re.S)
        if generic:
            base = generic[1].rsplit("::", 1)[-1]
            args = split_types(generic[2])
            owners = {"Vec": "std::vec", "Option": "std::option", "Box": "std::boxed",
                      "Arc": "std::sync", "BTreeMap": "std::collections", "HashMap": "std::collections",
                      "BTreeSet": "std::collections", "HashSet": "std::collections"}
            name = owners.get(base)
            head = name + "::" + base if name else generic[1]
            return head + "<" + ",".join(self.constructor_type(arg, module, None) for arg in args) + ">"
        if typ in ("Value", "serde_json::Value"):
            return "serde_json::Value"
        if typ in ("String", "std::string::String", "bool", "()", "&str", "&'static str") or re.fullmatch(r"[uif](8|16|32|64|128|size)", typ):
            return typ
        if typ == "SharedString":
            return "gpui_kit::SharedString"
        if typ.startswith("[") or typ.startswith("("):
            return typ
        definition = self.definition(typ, module, value)
        owner = definition.module.relative_to(ROOT / "crates").parts[0]
        return owner.replace("-", "_") + "::" + typ if owner != self.crate_root.name and "::" not in typ else typ

    def _render(self, value, typ, module, path, allow_value):
        if self.expression_pool is None or not isinstance(value, (dict, list, str)):
            return self._render_inner(value, typ, module, path, allow_value)
        encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
        if len(encoded) < 1024:
            return self._render_inner(value, typ, module, path, allow_value)
        key = (str(module), typ, hashlib.sha256(encoded.encode("utf-8")).digest(), allow_value)
        cached = self.expression_pool.get(key)
        if cached is not None:
            name, excluded, boundaries = cached
            self.excluded.update(path + suffix for suffix in excluded)
            self.value_fields.update(path + suffix for suffix in boundaries)
            return name + "()"
        outer_excluded, outer_boundaries = self.excluded, self.value_fields
        self.excluded, self.value_fields = set(), set()
        try:
            expression = self._render_inner(value, typ, module, path, allow_value)
            excluded = {item[len(path):] for item in self.excluded}
            boundaries = {item[len(path):] for item in self.value_fields}
        finally:
            outer_excluded.update(self.excluded)
            outer_boundaries.update(self.value_fields)
            self.excluded, self.value_fields = outer_excluded, outer_boundaries
        name = "source_table_" + str(len(self.shared_constructors))
        self.shared_constructors.append((name, self.constructor_type(typ, module, value), expression))
        self.expression_pool[key] = (name, excluded, boundaries)
        return name + "()"

    def _render_inner(self, value, typ, module, path, allow_value):
        typ = typ.strip()
        typ = self.aliases.get((module, typ), typ)
        short = typ.rsplit("::", 1)[-1]
        if typ == "()":
            if value is not None:
                raise ValueError(f"{path}: expected null")
            return "()"
        if typ in ("String", "std::string::String", "SharedString"):
            if not isinstance(value, str):
                raise ValueError(f"{path}: expected {typ}")
            return rust_string(value) + ".into()"
        if typ in ("&str", "&'static str"):
            if not isinstance(value, str):
                raise ValueError(f"{path}: expected string")
            return rust_string(value)
        if typ == "bool":
            if type(value) is not bool:
                raise ValueError(f"{path}: expected bool")
            return str(value).lower()
        if re.fullmatch(r"[ui](8|16|32|64|128|size)", typ):
            if type(value) is not int:
                raise ValueError(f"{path}: expected {typ}")
            bits = 64 if typ.endswith("size") else int(typ[1:])
            low, high = (0, 2 ** bits - 1) if typ[0] == "u" else (-2 ** (bits - 1), 2 ** (bits - 1) - 1)
            if not low <= value <= high:
                raise ValueError(f"{path}: {value} is outside {typ}")
            return str(value) + typ
        if typ in ("f32", "f64"):
            if type(value) not in (int, float):
                raise ValueError(f"{path}: expected {typ}")
            if not math.isfinite(value):
                raise ValueError(f"{path}: expected a finite {typ}")
            return repr(float(value)) + typ
        generic = re.fullmatch(r"([\w:]+)\s*<(.*)>", typ, re.S)
        if generic:
            base = generic[1].rsplit("::", 1)[-1]
            args = split_types(generic[2])
            if base == "Option":
                if value is None and args[0] != "Value" and args[0] in self.definitions:
                    inner = self.definition(args[0], module, value)
                    if "untagged" in inner.attrs and any(body is not None and body.type == "ordered_field_declaration_list" and body.text.decode().strip() == "(())" for _, body, _ in inner.fields):
                        return "Some(" + self._render(value, args[0], module, path, allow_value) + ")"
                return "None" if value is None else "Some(" + self._render(value, args[0], module, path, allow_value) + ")"
            if base in ("Vec", "HashSet", "BTreeSet"):
                if not isinstance(value, list):
                    raise ValueError(f"{path}: expected array")
                entries = [self._render(v, args[0], module, path + "[]", allow_value) for v in value]
                sequence = "vec![" + ",".join(entries) + "]"
                return sequence if base == "Vec" else sequence + ".into_iter().collect()"
            if base in ("HashMap", "BTreeMap", "IndexMap", "Map"):
                if not isinstance(value, dict):
                    raise ValueError(f"{path}: expected object")
                entries = []
                for key, item in value.items():
                    k = int(key) if re.fullmatch(r"[ui](8|16|32|64|128|size)", args[0]) else key
                    entries.append("(" + self._render(k, args[0], module, path, allow_value) + "," + self._render(item, args[1], module, path + ".*", allow_value) + ")")
                return "vec![" + ",".join(entries) + "].into_iter().collect()"
            if base in ("Box", "Arc"):
                return base + "::new(" + self._render(value, args[0], module, path, allow_value) + ")"
            if base == "Cow" and args[-1] == "str":
                return "std::borrow::Cow::Borrowed(" + rust_string(value) + ")"
        if typ.startswith("[") and typ.endswith("]"):
            args = split_types(typ[1:-1])
            if len(args) == 2 and len(value) != int(args[1]):
                raise ValueError(f"{path}: fixed array length differs")
            return "[" + ",".join(self._render(v, args[0], module, path + "[]", allow_value) for v in value) + "]"
        if typ.startswith("(") and typ.endswith(")"):
            args = split_types(typ[1:-1])
            if len(args) != len(value):
                raise ValueError(f"{path}: tuple length differs")
            return "(" + ",".join(self._render(v, arg, module, path, allow_value) for v, arg in zip(value, args)) + ",)"
        if typ in ("Value", "serde_json::Value"):
            if not allow_value:
                raise ValueError(f"{path}: serde_json::Value needs an explicit external-format boundary")
            self.value_fields.add(path)
            return json_boundary(value)
        definition = self.definition(typ, module, value)
        constructor = typ
        owner = definition.module.relative_to(ROOT / "crates").parts[0]
        if owner != self.crate_root.name and "::" not in typ:
            constructor = owner.replace("-", "_") + "::" + typ
        mode = attribute(definition.attrs, "rename_all")
        if definition.kind == "enum_item":
            tag, content = attribute(definition.attrs, "tag"), attribute(definition.attrs, "content")
            if "untagged" in definition.attrs:
                variants = definition.fields
                if type(value) is int:
                    variants = sorted(
                        variants,
                        key=lambda item: bool(
                            item[1] is not None
                            and re.fullmatch(r"\(f(?:32|64)\)", item[1].text.decode())
                        ),
                    )
                for name, body, _ in variants:
                    if body is not None and body.type == "ordered_field_declaration_list" and len(body.named_children) == 1:
                        try:
                            argument = self._render(value, body.named_children[0].text.decode(), definition.module, path, allow_value)
                            return constructor + "::" + name + "(" + argument + ")"
                        except ValueError:
                            pass
                raise ValueError(f"{path}: no matching untagged {typ} variant")
            for name, body, attrs in definition.fields:
                serialized = attribute(attrs, "rename") or rename(name, mode)
                names = [serialized, *re.findall(r'\balias\s*=\s*"([^"\\]+)"', attrs)]
                matches = value.get(tag) in names if tag and isinstance(value, dict) else value in names
                payload = value.get(content) if content and isinstance(value, dict) else value
                if not matches:
                    continue
                if body is None:
                    return constructor + "::" + name
                if body.type == "ordered_field_declaration_list":
                    args = [child.text.decode() for child in body.named_children if child.type != "attribute_item"]
                    values = [payload] if len(args) == 1 else payload
                    return constructor + "::" + name + "(" + ",".join(self._render(item, arg, definition.module, path + "." + name, allow_value) for item, arg in zip(values, args)) + ")"
                if body.type == "field_declaration_list":
                    entries = []
                    for field in body.named_children:
                        if field.type != "field_declaration":
                            continue
                        field_name = field.child_by_field_name("name").text.decode()
                        field_type = field.child_by_field_name("type").text.decode()
                        entries.append(field_name + ":" + self._render(payload[field_name], field_type, definition.module, path + "." + field_name, allow_value))
                    return constructor + "::" + name + "{" + ",".join(entries) + "}"
            raise ValueError(f"{path}: unsupported enum {typ} value {value!r}")
        if not isinstance(value, dict):
            raise ValueError(f"{path}: expected {typ} object")
        fields, consumed = [], set()
        named_keys = {key for name, _, attrs in definition.fields if "flatten" not in attrs
                      for key in [attribute(attrs, "rename") or rename(name, mode),
                                  *re.findall(r'\balias\s*=\s*"([^"\\]+)"', attrs)]}
        for name, field_type, attrs in definition.fields:
            key = attribute(attrs, "rename") or rename(name, mode)
            accepted = [key, *re.findall(r'\balias\s*=\s*"([^"\\]+)"', attrs)]
            if sum(candidate in value for candidate in accepted) > 1:
                raise ValueError(f"{path}: duplicate field aliases for {typ}.{key}")
            if key not in value:
                key = next((alias for alias in accepted[1:] if alias in value), key)
            if re.search(r"\b(skip|skip_deserializing)\b", attrs):
                expression = "Default::default()"
            elif key in value:
                consumed.add(key)
                expression = self._render(value[key], field_type, definition.module, path + "." + key, allow_value)
            elif re.search(r"\bflatten\b", attrs):
                remainder = {key: item for key, item in value.items() if key not in named_keys}
                expression = self._render(remainder, field_type, definition.module, path + ".*", allow_value)
                consumed.update(remainder)
            elif field_type.startswith("Option<"):
                expression = "None"
            elif re.search(r"\bdefault\b", attrs) or re.search(r"\bdefault\b", definition.attrs):
                function = attribute(attrs, "default")
                expression = (function + "()") if function else "Default::default()"
            else:
                raise ValueError(f"{path}: missing required {typ}.{key}")
            fields.append(name + ":" + expression)
        unknown = set(value) - consumed
        if unknown and "deny_unknown_fields" in definition.attrs:
            raise ValueError(f"{path}: unknown fields for {typ}: {sorted(unknown)}")
        self.excluded.update(path + "." + name for name in unknown)
        return constructor + "{" + ",".join(fields) + "}"


def build_table(crate, symbol, module, root_type, *, source=None, check=False,
                macro_name=None, allow_value=False, product_factory=False, schema=None):
    import asset_release
    crate_root = ROOT / "crates" / crate
    original = Path(source) if source else asset_release.builtin_data_path(crate, symbol)
    original = original if original.is_absolute() else ROOT / original
    raw = original.read_bytes()
    value = json.loads(raw)
    schema = schema or RustData(crate_root)
    schema.expression_pool = {} if len(raw) > 1024 * 1024 else None
    schema.shared_constructors = []
    if product_factory:
        match = re.fullmatch(r"Vec<(.*)>", root_type)
        if not match or not isinstance(value, list):
            raise ValueError("A product factory requires Vec<T> source rows")
        entries, excluded, boundaries, products = [], set(), set(), set()
        for row in value:
            product = row.get("product_id", row.get("pid"))
            if type(product) is not int or product <= 0:
                raise ValueError("Product factory row has no stable product ID")
            if product in products:
                raise ValueError("Product factory has a duplicate product ID")
            products.add(product)
            body = schema.render(row, match[1], module, allow_value=allow_value)
            entries.append(f"({product}u32, (|| {body}) as fn() -> {match[1]})")
            excluded.update(schema.excluded)
            boundaries.update(schema.value_fields)
        expression = "vec![" + ",".join(entries) + "]"
        schema.excluded, schema.value_fields = excluded, boundaries
    else:
        expression = schema.render(value, root_type, module, allow_value=allow_value)
    macro_name = macro_name or symbol.lower()
    if not re.fullmatch(r"[a-z][a-z0-9_]*", macro_name):
        raise ValueError("Invalid Rust table macro name")
    output = crate_root / "src/data" / (macro_name + ".rs")
    maintained = ROOT / INPUTS / crate / (symbol.lower() + ".json")
    helpers = "".join("        fn " + name + "() -> " + typ + " {\n"
                      + pretty_expression(body, indent=12) + "\n        }\n"
                      for name, typ, body in schema.shared_constructors)
    content = ("// Generated by tools/workspace.py prepare-typed-data.\n"
               "#[rustfmt::skip]\n"
               f"macro_rules! {macro_name} {{\n    () => {{{{\n"
               + helpers + pretty_expression(expression, indent=8) + "\n    }};\n}\n"
               f"pub(crate) use {macro_name};\n").encode("utf-8")
    if check:
        if output.read_bytes() != content:
            raise ValueError("Typed Rust table differs from its schema or maintenance input")
    else:
        maintained.parent.mkdir(parents=True, exist_ok=True)
        if maintained.exists() and maintained.read_bytes() != raw:
            raise ValueError("Maintenance input already exists with different bytes")
        maintained.write_bytes(raw)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(content)
    report = {"crate": crate, "symbol": symbol, "module": str(Path(module)).replace("\\", "/"),
            "root_type": root_type, "macro": macro_name,
            "source": maintained.relative_to(ROOT).as_posix(),
            "source_sha256": hashlib.sha256(raw).hexdigest(),
            "output": output.relative_to(ROOT).as_posix(),
            "excluded_evidence_fields": sorted(schema.excluded),
            "external_json_boundaries": sorted(schema.value_fields), "allow_value": allow_value,
            "product_factory": product_factory, "passed": True}
    if not check:
        import analysis
        registry = ROOT / "tools/typed-data.json"
        with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
            recipes = json.loads(registry.read_bytes()) if registry.exists() else {"schema_version": 1, "tables": []}
            recipes["tables"] = [item for item in recipes["tables"]
                                 if (item["crate"], item["macro"]) != (crate, macro_name)] + [report]
            recipes["tables"].sort(key=lambda item: (item["crate"], item["macro"]))
            analysis.save(registry, recipes)
    return report


def infer_domain_schema(crate, symbol, root_name, *, map_paths=()):
    """Materialize a complete explicit schema for a fixed, heterogeneous table.

    Callers review the resulting Rust types and choose map paths by domain
    identity. No generic JSON tree or opaque source string enters the product.
    """
    import asset_release
    source = asset_release.builtin_data_path(crate, symbol)
    value = json.loads(source.read_bytes())
    definitions, names = [], set()
    maps = set(map_paths)

    def name_for(path):
        stem = root_name + "".join(part.title().replace("_", "") for part in re.findall(r"[A-Za-z0-9_]+", path))
        name, counter = stem, 2
        while name in names:
            name, counter = stem + str(counter), counter + 1
        names.add(name)
        return name

    def enum_type(samples, path, branches):
        name = name_for(path + "Choice")
        variants = []
        for label, items in branches:
            variants.append(f"    {label}({infer(items, path + label)}),")
        definitions.append("#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]\n#[serde(untagged)]\n"
                           + f"pub(super) enum {name} {{\n" + "\n".join(variants) + "\n}\n")
        return name

    def infer(samples, path, force_nullable=False):
        if not samples:
            return "String"
        categories = {}
        for sample in samples:
            category = {type(None): "Null", bool: "Boolean", int: "Integer", float: "Decimal",
                        str: "Text", dict: "Object", list: "Array"}.get(type(sample))
            if category is None:
                raise ValueError("Unsupported maintenance value at " + path)
            categories.setdefault(category, []).append(sample)
        if len(categories) > 1 or force_nullable:
            return enum_type(samples, path, sorted(categories.items()))
        category = next(iter(categories))
        if category == "Null":
            return "()"
        if category == "Boolean":
            return "bool"
        if category == "Integer":
            return "i64" if any(item < 0 for item in samples) else "u64"
        if category == "Decimal":
            return "f64"
        if category == "Text":
            return "String"
        if category == "Array":
            if samples and all(len(item) == 2 and isinstance(item[0], str) and not isinstance(item[1], str) for item in samples):
                return "(String," + infer([item[1] for item in samples], path + "Payload") + ")"
            return "Vec<" + infer([item for sequence in samples for item in sequence], path + "Item") + ">"
        if path in maps or all(not item for item in samples):
            return "std::collections::BTreeMap<String," + infer([item for obj in samples for item in obj.values()], path + "Entry") + ">"
        name = name_for(path)
        fields = []
        keys = list(dict.fromkeys(key for sample in samples for key in sample))
        for key in keys:
            present = [sample[key] for sample in samples if key in sample]
            optional = len(present) != len(samples)
            child = infer(present, path + "_" + key, optional and all(item is None for item in present))
            field_name = rename(key, "snake_case")
            field_name = re.sub(r"[^A-Za-z0-9_]", "_", field_name)
            if not field_name or field_name[0].isdigit():
                field_name = "field_" + field_name
            if field_name in {"type", "macro", "self", "super", "crate", "ref", "match", "move", "fn", "mod", "use", "loop", "where", "async", "await", "in", "const", "static", "default"}:
                field_name = "r#" + field_name
            attrs = f"    #[serde(rename = {rust_string(key)}"
            if optional:
                attrs += ', default, skip_serializing_if = "Option::is_none"'
                child = "Option<" + child + ">"
            attrs += ")]"
            fields.extend([attrs, f"    pub(super) {field_name}: {child},"])
        definitions.append("#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]\n#[serde(deny_unknown_fields)]\n"
                           + f"pub(super) struct {name} {{\n" + "\n".join(fields) + "\n}\n")
        return name

    generated_root = infer([value], "")
    if generated_root != root_name:
        raise ValueError("Domain root must be an object")
    output = ROOT / "crates" / crate / "src/data" / (symbol.lower() + "_schema.rs")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("// Explicit domain schema for source-derived fixed data.\n" + "\n".join(definitions),
                      encoding="utf-8", newline="\n")
    return output


def add_commands(commands):
    parser = commands.add_parser("prepare-typed-data", help="Regenerate code-owned Rust tables from explicit maintenance inputs")
    parser.add_argument("--crate", action="append", help="Restrict to owning crates")
    parser.add_argument("--check", action="store_true", help="Compare without modifying tables")


def run(args):
    registry = json.loads((ROOT / "tools/typed-data.json").read_bytes())
    rows = [item for item in registry["tables"] if not args.crate or item["crate"] in args.crate]
    schemas = {crate: RustData(ROOT / "crates" / crate) for crate in {item["crate"] for item in rows}}
    reports = [build_table(item["crate"], item["symbol"], item["module"], item["root_type"],
                           source=item["source"], macro_name=item["macro"], check=args.check,
                           allow_value=item.get("allow_value", False),
                           product_factory=item.get("product_factory", False), schema=schemas[item["crate"]]) for item in rows]
    return {"tables": len(reports), "check_only": args.check, "passed": True}
