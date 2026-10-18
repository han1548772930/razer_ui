"""Emit Rust domain constructors from audited inputs and their actual Rust schema.

This maintenance module is never imported by Cargo or the product. Typed
constructors live in their domain owners; large UI catalogs use SQLite readers.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
INPUTS = ".analysis/inputs/typed-tables"
EMOJI_SEARCH_INPUT = INPUTS + "/razer-catalog/emoji_search.json"


def emoji_search_input():
    raw = (ROOT / EMOJI_SEARCH_INPUT).read_bytes()
    value = json.loads(raw)
    if (type(value) is not dict or set(value) != {"schema_version", "search"}
            or type(value["schema_version"]) is not int or value["schema_version"] != 1):
        raise ValueError("Invalid shared emoji search schema")
    validate_emoji_search(value["search"])
    return raw, value["search"]


def validate_emoji_search(rows):
    if type(rows) is not list or not rows:
        raise ValueError("Emoji search must be a nonempty source-ordered array")
    for index, row in enumerate(rows):
        if (type(row) is not dict or set(row) != {"emo", "name"}
                or any(type(row[key]) is not str for key in ("emo", "name"))):
            raise ValueError(f"Emoji search entry {index} must contain emo and name strings")


def verify_emoji_search_sources(sources, expected):
    """Compare original JS literals without executing the source or callbacks."""
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from importlib.metadata import version
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    import json5

    parser = Parser(Language(tree_sitter_javascript.language()))
    reports = []
    for source in sources:
        path = (ROOT / source).resolve()
        if not path.is_relative_to(ROOT / ".analysis/inputs/frontend") or path.suffix != ".js":
            raise ValueError("Emoji evidence must be a retained frontend JS original")
        raw = path.read_bytes()
        receipt = next((parent / "acquisition.json" for parent in path.parents
                        if parent.is_relative_to(ROOT / ".analysis/inputs/frontend")
                        and (parent / "acquisition.json").is_file()), None)
        if receipt is None:
            raise ValueError("Emoji source has no acquisition receipt")
        acquisition = json.loads(receipt.read_bytes())
        relative = path.relative_to(ROOT).as_posix()
        acquired = [row for row in acquisition["files"] if row.get("path") == relative]
        digest = hashlib.sha256(raw).hexdigest()
        if (len(acquired) != 1 or acquired[0].get("sha256") != digest
                or acquired[0].get("bytes") != len(raw)
                or not acquired[0].get("url", "").startswith("https://apps.razer.com/")):
            raise ValueError("Emoji original differs from its official acquisition identity")
        tree = parser.parse(raw)
        if tree.root_node.has_error:
            raise ValueError("Emoji JS original contains unresolved syntax")
        literals, pending = [], [tree.root_node]
        while pending:
            node = pending.pop()
            if node.type == "string":
                text = node.text.decode("utf-8")
                if "Grinning Face,😃 Grinning Face with Big Eyes," in text:
                    literal = json5.loads(text)
                    rows = []
                    for item in literal.split(","):
                        parts = item.split(" ")
                        rows.append({"emo": parts[0], "name": " ".join(parts[1:])})
                    if rows != expected:
                        raise ValueError("Complete ordered emoji search differs from original JS: " + relative)
                    literals.append(node)
            pending.extend(node.named_children)
        if len(literals) != 1:
            raise ValueError("Emoji source search literal is not unique: " + relative)
        literal = literals[0]
        owner = next((node for node in ancestors(literal) if node.type in {
            "arrow_function", "function_expression", "function_declaration"}), None)
        if owner is None:
            raise ValueError("Emoji literal lacks a complete callable owner")
        body = owner.text
        reports.append({"path": relative, "bytes": len(raw), "sha256": digest,
                        "acquisition": receipt.relative_to(ROOT).as_posix(),
                        "acquisition_sha256": hashlib.sha256(receipt.read_bytes()).hexdigest(),
                        "acquisition_kind": acquired[0].get("kind"),
                        "source_current_during_acquisition": acquisition.get("source_current_during_acquisition"),
                        "deployment_current_verified": acquired[0].get("deployment_current_verified"),
                        "ordered_entries_verified": len(expected),
                        "literal": {"byte_start": literal.start_byte, "byte_end": literal.end_byte,
                                    "sha256": hashlib.sha256(literal.text).hexdigest()},
                        "owner": {"node_type": owner.type, "byte_start": owner.start_byte,
                                  "byte_end": owner.end_byte, "sha256": hashlib.sha256(body).hexdigest(),
                                  "body": body.decode("utf-8")}})
    return {"analyzers": {name: version(name) for name in ("tree-sitter", "tree-sitter-javascript", "json5")},
            "originals": reports, "target_execution": False,
            "limits": ["Literal equality does not establish deployment stability or complete product dependency closure"]}


def prepare_emoji_search(*, check=False, sources=None):
    import resource_database
    return resource_database.prepare(check=check, sources=sources)


def ancestors(node):
    parent = node.parent
    while parent is not None:
        yield parent
        parent = parent.parent


def imported_paths(node, prefix=()):
    """Resolve explicit Rust use trees; strings/comments never declare an owner."""
    def text(child):
        return child.text.decode("utf-8") if child is not None else ""
    if node.type == "use_declaration":
        yield from imported_paths(node.child_by_field_name("argument"), prefix)
    elif node.type == "scoped_use_list":
        path = prefix + tuple(text(node.child_by_field_name("path")).split("::"))
        yield from imported_paths(node.child_by_field_name("list"), path)
    elif node.type == "use_list":
        for child in node.named_children:
            yield from imported_paths(child, prefix)
    elif node.type == "use_as_clause":
        yield text(node.child_by_field_name("alias")), prefix + tuple(text(node.child_by_field_name("path")).split("::"))
    elif node.type == "self" and prefix:
        yield prefix[-1], prefix
    elif node.type in {"identifier", "scoped_identifier"}:
        path = prefix + tuple(text(node).split("::"))
        yield path[-1], path


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
    value = value.removeprefix("r#")
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


def json_boundary(value, render=None):
    """Construct an explicitly allowed interchange value without recursive macros."""
    render = render or json_boundary
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
        number = str(value) + kind
        if value < 0:
            number = "(" + number + ")"
        return "serde_json::Value::Number(" + number + ".into())"
    if type(value) is float:
        if not math.isfinite(value):
            raise ValueError("Interchange number must be finite")
        return ("serde_json::Value::Number(serde_json::Number::from_f64("
                + repr(value) + "f64).expect(\"finite interchange number\"))")
    if type(value) is list:
        return "serde_json::Value::Array(vec![" + ",".join(render(item) for item in value) + "])"
    if type(value) is dict:
        return ("serde_json::Value::Object([" + ",".join(
            "(" + rust_string(key) + ".into()," + render(item) + ")"
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
        sys.path.insert(0, str(Path(__file__).resolve().parent / "analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_rust

        self.crate_root = Path(crate_root).resolve()
        self.parser = Parser(Language(tree_sitter_rust.language()))
        self.definitions = {}
        self.aliases = {}
        self.excluded = set()
        self.value_fields = set()
        self.external_roots = set()
        self.expression_pool = None
        self.shared_constructors = []
        self.module_sources = {}
        self.module_imports = {}
        self.resolved_definitions = {}
        self.shared_inputs = {}
        for path in sorted((self.crate_root / "src").rglob("*.rs")):
            if "/src/data/" in path.as_posix() and not path.name.endswith("_schema.rs"):
                continue
            self._parse(path)

    def _parse(self, path):
        raw = path.read_bytes()
        root = self.parser.parse(raw).root_node
        self.module_imports[path] = {alias: target for node in root.named_children
                                     if node.type == "use_declaration" for alias, target in imported_paths(node)}

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
        key = (name, module)
        if key in self.resolved_definitions:
            return self.resolved_definitions[key]
        source = self.module_sources.get(module)
        if source is None:
            source = module.read_text(encoding="utf-8")
            self.module_sources[module] = source
        imports = self.module_imports.get(module, {})
        first = name.split("::", 1)[0]
        imported = imports.get(first)
        namespace = imported[0] if imported else first if "::" in name else None
        if namespace:
            external = ROOT / "crates" / namespace.replace("_", "-")
            if external.is_dir() and external != self.crate_root and external not in self.external_roots:
                self.external_roots.add(external)
                for path in sorted((external / "src").rglob("*.rs")):
                    if "/src/data/" not in path.as_posix():
                        self._parse(path)
        actual_name = imported[-1] if imported and "::" not in name else name.rsplit("::", 1)[-1]
        candidates = self.definitions.get(actual_name, [])
        if imported:
            imported_root = ROOT / "crates" / imported[0].replace("_", "-")
            if imported_root.is_dir():
                candidates = [item for item in candidates if item.module.is_relative_to(imported_root)]
        local = [item for item in candidates if item.module == module]
        if len(local) == 1:
            self.resolved_definitions[key] = local[0]
            return local[0]
        candidates = local or candidates
        if len(candidates) == 1:
            self.resolved_definitions[key] = candidates[0]
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
        imported = self.module_imports.get(module, {}).get(typ)
        if owner != self.crate_root.name and imported is not None:
            return "::".join(imported)
        return owner.replace("-", "_") + "::" + typ if owner != self.crate_root.name and "::" not in typ else typ

    def _render(self, value, typ, module, path, allow_value):
        if typ.strip() == "&'static EmojiSearchTable":
            return self._render_inner(value, typ, module, path, allow_value)
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
        if short in {"EmojiData", "SourceEmojiData", "TextSource"}:
            import resource_database
            profile = resource_database.emoji_dataset_identity(value)
            for input_path in resource_database.INPUTS.values():
                self.shared_inputs[input_path] = hashlib.sha256((ROOT / input_path).read_bytes()).hexdigest()
            return typ + "::from_catalog(razer_catalog::emoji::DatasetId::" + profile + ")"
        if typ == "&'static EmojiSearchTable":
            validate_emoji_search(value)
            raw, shared = emoji_search_input()
            if value != shared:
                raise ValueError(f"{path}: source emoji search differs from the shared vocabulary")
            self.shared_inputs[EMOJI_SEARCH_INPUT] = hashlib.sha256(raw).hexdigest()
            return "razer_catalog::emoji::search()"
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
        if typ.startswith("&'static [") and typ.endswith("]"):
            return "&" + self._render(value, typ[len("&'static "):], module, path, allow_value)
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
            return self._boundary(value)
        definition = self.definition(typ, module, value)
        constructor = self.constructor_type(typ, module, value)
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

    def _boundary(self, value):
        if self.expression_pool is None or not isinstance(value, (dict, list)):
            return json_boundary(value)
        encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
        if len(encoded) < 256:
            return json_boundary(value)
        key = ("interchange", hashlib.sha256(encoded.encode("utf-8")).digest())
        cached = self.expression_pool.get(key)
        if cached is not None:
            return cached + "()"
        expression = json_boundary(value, self._boundary)
        name = "source_table_" + str(len(self.shared_constructors))
        self.shared_constructors.append((name, "serde_json::Value", expression))
        self.expression_pool[key] = name
        return name + "()"


MAPPING_ENGINE_SOURCE = ".analysis/results/frontend/mapping-engine-current-data.json"
MAPPING_ENGINE_MODULE = "crates/razer-device/src/mapping/mapping_engine_data.rs"
MAPPING_ENGINE_OUTPUT = "crates/razer-device/src/mapping/mapping_engine_data_generated.rs"


def mapping_engine_tables(data, *, schema=None):
    """Validate every source field against its actual static Rust owner."""
    if type(data) is not dict or not data:
        raise ValueError("Mapping engine source must contain product branches")
    module = MAPPING_ENGINE_MODULE
    schema = schema or RustData(ROOT / "crates/razer-device")
    tables, products = [], []
    if any(type(product) is not str or not re.fullmatch(r"[1-9][0-9]*", product)
           or int(product) > 0xffffffff for product in data):
        raise ValueError("Mapping engine product identity or fields differ")
    for product, row in sorted(data.items(), key=lambda item: int(item[0])):
        if type(row) is not dict or set(row) != {"keys", "default_turbo_ids", "turbo_events", "constants"}:
            raise ValueError("Mapping engine product identity or fields differ")
        ids, events, constants = row["default_turbo_ids"], row["turbo_events"], row["constants"]
        if (type(ids) is not list or not ids or any(type(value) is not str or not value for value in ids)
                or len(set(ids)) != len(ids) or type(events) is not dict or set(events) != set(ids)
                or type(constants) is not dict or set(constants) != {
                    "SINGLE_STATE_INPUT_ID", "SCROLL_INPUT_ID", "SCROLL_TURBO_DELAY_MS"}
                or type(row["keys"]) is not list or not row["keys"]):
            raise ValueError("Mapping engine ordered keys, turbo events or constants are incomplete")
        def non_null(value):
            if value is None:
                raise ValueError("Mapping engine fixed fields must not contain null")
            for child in value.values() if isinstance(value, dict) else value if isinstance(value, list) else []:
                non_null(child)
        non_null(row)
        value = {"keys": row["keys"], "default_turbos": [{"id": key, "events": events[key]} for key in ids],
                 "single_state_input_ids": constants["SINGLE_STATE_INPUT_ID"],
                 "scroll_input_ids": constants["SCROLL_INPUT_ID"],
                 "scroll_turbo_delay_ms": constants["SCROLL_TURBO_DELAY_MS"]}
        expression = schema.render(value, "EngineData", module)
        if schema.excluded or schema.value_fields:
            raise ValueError("Mapping engine source has fields outside the typed Rust schema: " + str(sorted(schema.excluded)))
        index = next((index for index, previous in enumerate(tables) if previous == expression), len(tables))
        if index == len(tables):
            tables.append(expression)
        products.append((int(product), index))
    return tables, products


def prepare_mapping_engine_data(data, *, check=False):
    import subprocess
    tables, products = mapping_engine_tables(data)
    text = "// Generated by tools/workspace.py audit-family-function --query mapping-engine --prepare.\n"
    text += "// Complete original array order and independently decoded product branches are retained.\n\n"
    text += "\n\n".join(f"static DATA_{index}: EngineData = {body};" for index, body in enumerate(tables))
    text += "\n\npub(super) const PRODUCTS: &[ProductData] = &[\n" + "\n".join(
        f"ProductData {{ product_id: {product}, data: &DATA_{index} }}," for product, index in products) + "\n];\n"
    formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], input=text.encode("utf-8"),
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if formatted.returncode:
        raise ValueError("Mapping engine Rust formatting failed: " + formatted.stderr.decode("utf-8", errors="replace"))
    output = ROOT / MAPPING_ENGINE_OUTPUT
    if check:
        if output.read_bytes() != formatted.stdout:
            raise ValueError("Mapping engine generated Rust differs from complete typed source")
    else:
        output.write_bytes(formatted.stdout)
    summary = prepare_mapping_engine_summary(data, check=check)
    return {"output": output.relative_to(ROOT).as_posix(), "sha256": hashlib.sha256(formatted.stdout).hexdigest(),
            "products": [product for product, _ in products], "shared_tables": len(tables), "checked": check,
            "source_summary": summary}


def check_mapping_engine_data():
    source = ROOT / MAPPING_ENGINE_SOURCE
    raw = source.read_bytes()
    result = prepare_mapping_engine_data(json.loads(raw), check=True)
    return {"source": MAPPING_ENGINE_SOURCE, "source_sha256": hashlib.sha256(raw).hexdigest(),
            **result}


def mapping_source_md5(value):
    """Independently encode the closed integer/string source-table projection."""
    def canonical(item):
        if type(item) is dict:
            if any(type(key) is not str for key in item):
                raise ValueError("Mapping summary keys must be strings")
            return {key: canonical(item[key]) for key in sorted(item, key=lambda key: key.encode("utf-16-be"))}
        if type(item) is list:
            return [canonical(child) for child in item]
        if type(item) is int:
            if abs(item) > 9_007_199_254_740_991:
                raise ValueError("Mapping summary integer exceeds exact JavaScript range")
        elif item is not None and type(item) not in {str, bool}:
            raise ValueError("Mapping fixed summary requires integer/string source fields")
        return item
    encoded = json.dumps(canonical(value), ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003C")
    return hashlib.md5(encoded.encode("utf-8")).hexdigest()


def mapping_engine_summary(data):
    """Keep source-derived digests and behavioral counts without copying tables."""
    tables, products = [], []
    for product, source in sorted(data.items(), key=lambda item: int(item[0])):
        key_events = []
        for key in source["keys"]:
            if "inputID" in key:
                flag = key.get("outputFlag", key["flag"])
                key_events.append({"inputID": key["inputID"], "events": [
                    {"type": "keyboard", "scancode": key["scancode"], "flag": flag + release}
                    for release in (0, 1)]})
        table = {"key_count": len(source["keys"]),
            "keys_without_input_id": sum("inputID" not in key for key in source["keys"]),
            "keys_md5": mapping_source_md5(source["keys"]),
            "key_events_md5": mapping_source_md5(key_events),
            "constants_md5": mapping_source_md5(source["constants"]),
            "default_turbos": [{"id": name, "event_count": len(source["turbo_events"][name]),
                "events_md5": mapping_source_md5(source["turbo_events"][name]),
                "source_hash": mapping_source_md5({"events": source["turbo_events"][name]})}
                for name in source["default_turbo_ids"]]}
        index = next((index for index, previous in enumerate(tables) if previous == table), len(tables))
        if index == len(tables):
            tables.append(table)
        products.append({"product_id": int(product), "table": index})
    return {"schema_version": 1, "products": products, "tables": tables}


def prepare_mapping_engine_summary(data, *, check=False):
    summary = mapping_engine_summary(data)
    raw = (json.dumps(summary, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    output = ROOT / "crates/razer-device/testdata/re/mapping-engine-source-summary.json"
    if check:
        if not output.is_file() or output.read_bytes() != raw:
            raise ValueError("Mapping engine source summary differs from independently decoded source")
    else:
        output.write_bytes(raw)
    if json.loads(output.read_bytes()) != summary:
        raise ValueError("Mapping engine source summary ordered readback differs")
    return {"output": output.relative_to(ROOT).as_posix(), "bytes": len(raw),
            "sha256": hashlib.sha256(raw).hexdigest(), "checked": check}


def magnetic_source_expressions(products, *, schema=None):
    """Render each fixed magnetic branch against the real capability owner."""
    module = "crates/razer-device/src/keyboard/keyboard_magnetic_source.rs"
    schema = schema or RustData(ROOT / "crates/razer-device")
    if set(products) != {740, 746}:
        raise ValueError("Magnetic fixed tables require both complete product branches")
    shared = ("DEFAULTPROFILE", "preset_guids", "preset_actuation", "preset_rapid_trigger", "defaultActuation", "defaultRapidTrigger", "guest_profile_owner", "onboard_preset_guids", "macro_scancodes", "bottom_deadzone")
    if any(products[740][key] != products[746][key] for key in shared):
        raise ValueError("Magnetic shared defaults differ across product branches")
    constants, functions, counts = {}, {}, {}
    def render(value, typ):
        result = schema.render(value, typ, module)
        if schema.excluded or schema.value_fields:
            raise ValueError("Magnetic fixed source has untyped or excluded fields: " + str(sorted(schema.excluded)))
        return result
    aliases = {"FACTORY_DEFAULT": "FACTORY", "FPS_RAPID_TRIGGER": "FPS", "ANALOG_WASD": "ANALOG_WASD",
               "RACING": "RACING", "HIGH_SENSITIVITY": "HIGH", "GENERAL": "GENERAL"}
    if set(products[740]["preset_guids"]) != set(aliases):
        raise ValueError("Magnetic preset identities differ")
    constants.update({aliases[key]: rust_string(guid) for key, guid in products[740]["preset_guids"].items()})
    constants["GUEST_PROFILE_OWNER"] = rust_string(products[740]["guest_profile_owner"])
    preset_names = {guid: aliases[key] for key, guid in products[740]["preset_guids"].items()}
    onboard = products[740]["onboard_preset_guids"]
    if len(onboard) != len(set(onboard)) or not set(onboard) < set(preset_names) or set(preset_names) - set(onboard) != {products[740]["preset_guids"]["GENERAL"]}:
        raise ValueError("Magnetic onboard preset predicate differs from the current registration branch")
    functions["is_onboard_preset"] = "matches!(guid," + "|".join(preset_names[guid] for guid in onboard) + ")"
    scans, seen = products[740]["macro_scancodes"], set()
    for row in scans:
        if (type(row) is not dict or set(row) != {"hid", "scancode"} or type(row["hid"]) is not int
                or type(row["scancode"]) is not int or not 0 <= row["hid"] <= 65535
                or not 0 <= row["scancode"] <= 0xffffffff or row["hid"] in seen):
            raise ValueError("Magnetic macro HID conversion fields or identities differ")
        seen.add(row["hid"])
    functions["macro_scancode"] = "match hid{" + ",".join(f"{row['hid']}=>Some({row['scancode']})" for row in scans) + ",_=>None}"
    deadzone = products[740]["bottom_deadzone"]
    if (type(deadzone) is not dict or set(deadzone) != {"responsive", "medium", "stable"}
            or any(type(value) is not int or not 0 <= value <= 255 for value in deadzone.values())):
        raise ValueError("Magnetic bottom deadzone fields or values differ")
    functions["bottom_deadzone"] = "match value{" + ",".join(rust_string(key) + "=>" + str(value)
        for key, value in deadzone.items() if key != "medium") + ",_=>" + str(deadzone["medium"]) + "}"
    profile_source = dict(products[740]["DEFAULTPROFILE"])
    polling = profile_source["pollingRate"]
    if type(polling) is float and math.isfinite(polling) and polling.is_integer():
        profile_source["pollingRate"] = int(polling)
    profile = render(profile_source, "ProfileDefaults")
    functions["default_profile"] = 'serde_json::to_value(' + profile + ').expect("typed magnetic profile defaults")'
    def mapping(row):
        kind = row["outputType"]
        required = {"inputID", "inputType", "isHyperShift", "outputType"}
        if kind not in {"otfmGroup", "gameModeGroup"}:
            required.add(kind)
        if set(row) != required or row["inputType"] != "KeyInput" or row["isHyperShift"] is not True:
            raise ValueError("Magnetic default mapping fields or branch differ")
        group = row.get(kind)
        if kind == "keyboardGroup":
            if group == {"key": "KEY_PRINT_SCREEN", "keyAssignment": "Prnt Scrn", "modifiers": []}:
                output = "DefaultOutput::PrintScreen"
            elif type(group) is dict and set(group) == {"key"} and type(group["key"]) is str:
                output = "DefaultOutput::Keyboard(" + rust_string(group["key"]) + ")"
            else:
                raise ValueError("Magnetic keyboard default output differs")
        elif kind == "multimediaGroup":
            if set(group) not in ({"multimediaAssignment"}, {"multimediaAssignment", "useTurbo"}) or group.get("useTurbo", True) is not True:
                raise ValueError("Magnetic media default fields differ")
            output = "DefaultOutput::Media{action:" + rust_string(group["multimediaAssignment"]) + ",turbo:" + str(group.get("useTurbo", False)).lower() + "}"
        elif kind in {"systemGroup", "backlightGroup"}:
            field = "systemAssignment" if kind == "systemGroup" else "backlightAssignment"
            if set(group) != {field}:
                raise ValueError("Magnetic named default output differs")
            output = "DefaultOutput::" + ("System" if kind == "systemGroup" else "Backlight") + "(" + rust_string(group[field]) + ")"
        elif kind == "profileNavigationGroup":
            if set(group) not in ({"profileNavigationAssignment"}, {"profileNavigationAssignment", "guid"}):
                raise ValueError("Magnetic profile default output differs")
            output = "DefaultOutput::Profile{action:" + rust_string(group["profileNavigationAssignment"]) + ",guid:" + (
                "Some(" + rust_string(group["guid"]) + ")" if "guid" in group else "None") + "}"
        elif kind in {"otfmGroup", "gameModeGroup"}:
            output = "DefaultOutput::" + ("Otfm" if kind == "otfmGroup" else "GameMode")
        elif kind == "snapTapGroup" and group == {"snapTapAssignment": "toggle"}:
            output = "DefaultOutput::SnapTap"
        else:
            raise ValueError("Magnetic default output is unresolved: " + str(kind))
        return "DefaultMapping{input:" + rust_string(row["inputID"]) + ",output:" + output + "}"
    for pid, source in products.items():
        if source["DEFAULT_FIRMWARE_MAPPINGS"] != []:
            raise ValueError("Magnetic firmware default override requires an explicit owner")
        for suffix, key in [("", "DEFAULTMAPPINGS"), ("_JP", "DEFAULTMAPPINGS_JPN"), ("_UK", "DEFAULTMAPPINGS_UK")]:
            if key in source:
                constants[f"DEFAULT_{pid}{suffix}"] = "&[" + ",".join(mapping(row) for row in source[key]) + "]"
        for suffix, key in [("", "BUTTON_LIST"), ("_JP", "BUTTON_LIST_JP"), ("_UK", "BUTTON_LIST_UK")]:
            if key not in source:
                continue
            # The list intentionally contains repeated source rows; preserve them
            # in encoded defaults while OBM physical identity keeps first order.
            constants[f"BUTTONS_{pid}{suffix}"] = render(list(dict.fromkeys(row["inputID"] for row in source[key])), "&'static [&'static str]")
            defaults = source["obm_default_buttons"][key]
            constants[f"OBM_DEFAULT_{pid}{suffix}"] = render(defaults, "&'static [DefaultButton]")
            counts[f"OBM_DEFAULT_{pid}{suffix}"] = len(defaults)
        firmware = [{"input": name, "id": row["fwID"]} for name, row in source["firmware_keyboard"]["NormalMode"].items()
                    if type(row.get("fwID")) is int and row["fwID"] != 255]
        constants[f"FIRMWARE_{pid}"] = render(firmware, "&'static [FirmwareKey]")
        full = []
        for row in source["FULLMAPPINGLIST"]:
            if (set(row) != {"inputID", "inputType", "isHyperShift", "outputType", "disableGroup"}
                    or row["inputType"] not in {"KeyInput", "AnalogInput"} or row["outputType"] != "disableGroup"
                    or row["disableGroup"] != {"disableAssignment": "Disable"}):
                raise ValueError("Magnetic full input fields or disabled output differ")
            full.append({"input": row["inputID"], "analog": row["inputType"] == "AnalogInput", "hypershift": row["isHyperShift"]})
        constants[f"FULL_{pid}"] = render(full, "&'static [FullInput]")
    return constants, functions, counts


def prepare_magnetic_source(products, *, check=False):
    """Update only the AST-owned fixed expressions; keep lifecycle/API code."""
    import subprocess
    constants, functions, counts = magnetic_source_expressions(products)
    path = ROOT / "crates/razer-device/src/keyboard/keyboard_magnetic_source.rs"
    raw = path.read_bytes()
    schema = RustData(ROOT / "crates/razer-device")
    tree = schema.parser.parse(raw)
    replacements, found = [], set()
    for node in tree.root_node.named_children:
        if node.type not in {"const_item", "function_item"}:
            continue
        name = node.child_by_field_name("name").text.decode("utf-8")
        if node.type == "const_item" and name in constants:
            value = node.child_by_field_name("value")
            replacements.append((value.start_byte, value.end_byte, constants[name].encode("utf-8")))
            found.add(name)
        if node.type == "function_item" and name in functions:
            body = node.child_by_field_name("body")
            replacements.append((body.start_byte, body.end_byte, ("{" + functions[name] + "}").encode("utf-8")))
            found.add(name)
    if found != set(constants) | set(functions):
        raise ValueError("Magnetic generated expressions are missing their Rust owner: " + str(sorted((set(constants) | set(functions)) - found)))
    for start, end, replacement in sorted(replacements, reverse=True):
        raw = raw[:start] + replacement + raw[end:]
    formatted = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], input=raw,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if formatted.returncode:
        raise ValueError("Magnetic typed source formatting failed: " + formatted.stderr.decode("utf-8", errors="replace"))
    if check:
        if path.read_bytes().replace(b"\r\n", b"\n") != formatted.stdout.replace(b"\r\n", b"\n"):
            raise ValueError("Magnetic generated Rust differs from complete typed source")
    else:
        path.write_bytes(formatted.stdout)
    return {"output": path.relative_to(ROOT).as_posix(), "sha256": hashlib.sha256(formatted.stdout).hexdigest(),
            "products": sorted(products), "fixed_expressions": len(found), "ordered_default_buttons": counts, "checked": check}


def owned_constructor(schema, module, marker, function, fragment):
    """Keep a generated function and its private schema in the same module."""
    import textwrap
    path = ROOT / module
    raw = path.read_bytes().replace(b"\r\n", b"\n")
    source = raw.decode("utf-8")
    changes, declarations = [], {}
    def visit(node):
        if node.type in {"struct_item", "enum_item"}:
            name = node.child_by_field_name("name").text.decode("utf-8")
            owner = node
            while owner.parent.type != "source_file":
                owner = owner.parent
            if owner != node and re.search(r"\b" + re.escape(name) + r"\b", fragment):
                start, previous = node.start_byte, node.prev_named_sibling
                while previous is not None and previous.type == "attribute_item":
                    start, previous = previous.start_byte, previous.prev_named_sibling
                start = raw.rfind(b"\n", 0, start) + 1
                end = raw.find(b"\n", node.end_byte) + 1
                changes.append((start, end, b""))
                declarations.setdefault(owner.start_byte, []).append(textwrap.dedent(raw[start:end].decode()).strip() + "\n\n")
        for child in node.named_children:
            visit(child)
    visit(schema.parser.parse(raw).root_node)
    for at, blocks in declarations.items():
        changes.append((at, at, "".join(blocks).encode("utf-8")))
    for start, end, value in sorted(changes, reverse=True):
        raw = raw[:start] + value + raw[end:]
    source = raw.decode("utf-8")
    if marker in source:
        source, count = re.subn(re.escape(marker) + r".*?\n}\n", lambda _: fragment, source, flags=re.S)
        if count != 1:
            raise ValueError("Repeated generated constructor identity")
    else:
        if re.search(r"\bfn\s+" + re.escape(function) + r"\b", source):
            raise ValueError("Generated constructor conflicts with handwritten code")
        source = source.rstrip() + "\n\n" + fragment
    return source.encode("utf-8")


def build_table(crate, symbol, module, root_type, *, source=None, check=False,
                macro_name=None, allow_value=False, product_factory=False, schema=None):
    import asset_release
    macro_name = macro_name or symbol.lower()
    registry_path = ROOT / "tools/typed-data.json"
    recipes = json.loads(registry_path.read_bytes()) if registry_path.is_file() else {"tables": []}
    registered = next((row for row in recipes["tables"]
                       if (row["crate"], row["macro"]) == (crate, macro_name)), None)
    if registered and registered.get("storage") in {"sqlite", "sqlite-profile"}:
        import catalog_data
        import resource_database
        maintained = ROOT / registered["source"]
        if source:
            original = ROOT / source
            raw = original.read_bytes()
            if registered["storage"] == "sqlite":
                catalog_data.Codec(ROOT / "crates" / crate).document(registered, json.loads(raw))
            else:
                resource_database.emoji_dataset_identity(json.loads(raw))
            if check and maintained.read_bytes() != raw:
                raise ValueError("SQLite catalog maintenance input differs")
            if not check and maintained.read_bytes() != raw:
                maintained.write_bytes(raw)
        report = resource_database.prepare(check=check)
        return next(row for row in report["typed_catalogs"] if row["id"] == registered["catalog_id"]) if registered["storage"] == "sqlite" else registered
    crate_root = ROOT / "crates" / crate
    original = Path(source) if source else asset_release.builtin_data_path(crate, symbol)
    original = original if original.is_absolute() else ROOT / original
    raw = original.read_bytes()
    value = json.loads(raw)
    schema = schema or RustData(crate_root)
    schema.expression_pool = {} if len(raw) > 1024 * 1024 else None
    schema.shared_constructors = []
    schema.shared_inputs = {}
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
    output = ROOT / module
    maintained = ROOT / INPUTS / crate / (macro_name + ".json")
    function = "make_" + macro_name.removesuffix("_data") + "_table"
    marker = "// Generated typed table: " + crate + "/" + macro_name + ".\n"
    helpers = "".join("    fn " + name + "() -> " + typ + " {\n"
                      + pretty_expression(body, indent=8) + "\n    }\n"
                      for name, typ, body in schema.shared_constructors)
    return_type = "Vec<(u32, fn() -> " + match[1] + ")>" if product_factory else root_type
    content = (marker + "#[rustfmt::skip]\nfn " + function + "() -> " + return_type + " {\n"
               + helpers + pretty_expression(expression, indent=4) + "\n}\n").encode("utf-8")
    body = owned_constructor(schema, module, marker, function, content.decode("utf-8"))
    pattern = r"crate::data::(?:" + re.escape(macro_name) + r"::)?" + re.escape(macro_name) + r"!\(\s*\)"
    body = re.sub(pattern, function + "()", body.decode("utf-8")).encode("utf-8")
    outputs = {output: content}
    stale_root = crate_root / "src/data"
    stale = [stale_root / (macro_name + ".rs"), *stale_root.glob(macro_name + "_part_*.rs")]
    if check:
        if output.read_bytes() != body or any(path.exists() for path in stale):
            raise ValueError("Typed Rust constructor differs from its schema or maintenance input")
    else:
        maintained.parent.mkdir(parents=True, exist_ok=True)
        if maintained.exists() and maintained.read_bytes() != raw:
            raise ValueError("Maintenance input already exists with different bytes")
        if not maintained.is_file():
            maintained.write_bytes(raw)
        output.write_bytes(body)
        for path in stale:
            if path.is_file():
                if path.resolve().parent != stale_root.resolve() or not path.read_bytes().startswith(b"// Generated"):
                    raise ValueError("Superseded constructor is not an owned generated file")
                path.unlink()
        registry = crate_root / "src/data.rs"
        if registry.is_file():
            text = registry.read_text(encoding="utf-8")
            text = re.sub(r'^include!\("data/' + re.escape(macro_name) + r'\.rs"\);\n', '', text, flags=re.M)
            text = re.sub(r'^#\[path = "data/' + re.escape(macro_name) + r'\.rs"\]\npub\(crate\) mod ' + re.escape(macro_name) + r';\n', '', text, flags=re.M)
            registry.write_text(text, encoding="utf-8", newline="\n")
        if stale_root.is_dir() and not any(stale_root.iterdir()):
            stale_root.rmdir()
            registry.unlink(missing_ok=True)
            lib = crate_root / "src/lib.rs"
            lib.write_bytes(lib.read_bytes().replace(b"mod data;\r\n", b"").replace(b"mod data;\n", b""))
    report = {"crate": crate, "symbol": symbol, "module": str(Path(module)).replace("\\", "/"),
            "root_type": root_type, "macro": macro_name, "storage": "rust", "reader": function,
            "source": maintained.relative_to(ROOT).as_posix(),
            "source_sha256": hashlib.sha256(raw).hexdigest(),
            "output": output.relative_to(ROOT).as_posix(),
            "outputs": [{"path": path.relative_to(ROOT).as_posix(), "reader": function, "sha256": hashlib.sha256(body).hexdigest()}
                        for path, body in outputs.items()],
            "excluded_evidence_fields": sorted(schema.excluded),
            "external_json_boundaries": sorted(schema.value_fields), "allow_value": allow_value,
            "product_factory": product_factory, "passed": True}
    if schema.shared_inputs:
        report["shared_inputs"] = [{"path": path, "sha256": digest}
                                   for path, digest in sorted(schema.shared_inputs.items())]
    if not check:
        import analysis
        registry = ROOT / "tools/typed-data.json"
        with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
            recipes = json.loads(registry.read_bytes()) if registry.exists() else {"schema_version": 1, "tables": []}
            previous = next((item for item in recipes["tables"]
                             if (item["crate"], item["macro"]) == (crate, macro_name)), None)
            if previous and "maintenance_recovery" in previous:
                report["maintenance_recovery"] = previous["maintenance_recovery"]
            if previous and "source_evidence" in previous:
                report["source_evidence"] = previous["source_evidence"]
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
    registry = json.loads((ROOT / "tools/typed-data.json").read_bytes())
    owners = {item["module"] for item in registry["tables"] if item["crate"] == crate and item["symbol"] == symbol}
    if len(owners) != 1:
        raise ValueError("A domain schema requires one explicit owning module")
    output = (ROOT / owners.pop()).with_name(symbol.lower() + "_schema.rs")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("// Explicit domain schema for source-derived fixed data.\n" + "\n".join(definitions),
                      encoding="utf-8", newline="\n")
    return output


def add_commands(commands):
    parser = commands.add_parser("prepare-typed-data", help="Regenerate code-owned Rust tables from explicit maintenance inputs")
    parser.add_argument("--crate", action="append", help="Restrict to owning crates")
    parser.add_argument("--symbol", action="append", help="Restrict to registered source symbols")
    parser.add_argument("--check", action="store_true", help="Compare without modifying tables")
    parser.add_argument("--recover-maintenance", action="store_true", help="Recover missing maintained inputs from complete current Rust constructors and verify AST equivalence")
    parser.add_argument("--record-verification", action="store_true", help="With an unfiltered --check, record complete input/output identities and index maintenance bytes without claiming vendor provenance")
    emoji = commands.add_parser("prepare-emoji-search", help="Share identical emoji entries while preserving the complete source order")
    emoji.add_argument("--check", action="store_true", help="Compare without modifying the shared table")
    emoji.add_argument("--source", action="append", help="Retained original JS; verify the entire ordered vocabulary and register its byte identity")


def run(args):
    if args.command == "prepare-emoji-search":
        return prepare_emoji_search(check=args.check, sources=args.source)
    registry = json.loads((ROOT / "tools/typed-data.json").read_bytes())
    record = getattr(args, "record_verification", False)
    if record and (not args.check or args.crate or args.symbol or args.recover_maintenance):
        raise ValueError("Recorded maintenance verification requires an unfiltered --check")
    snapshots = maintenance_verification_inputs(registry) if record else None
    rows = [item for item in registry["tables"]
            if (not args.crate or item["crate"] in args.crate)
            and (not args.symbol or item["symbol"] in args.symbol)]
    missing = set(args.symbol or ()) - {item["symbol"] for item in rows}
    if not rows or missing:
        raise ValueError("No matching typed tables for requested crate/symbol filters: " + ", ".join(sorted(missing)))
    sqlite_rows = [item for item in rows if item.get("storage") in {"sqlite", "sqlite-profile"}]
    rows = [item for item in rows if item.get("storage") not in {"sqlite", "sqlite-profile"}]
    if args.recover_maintenance:
        if args.check:
            raise ValueError("Maintenance recovery cannot run in check-only mode")
        import analysis
        import catalog_data
        missing_rows = [item for item in rows if not item.get("maintenance_recovery")]
        for item in missing_rows:
            item["source"] = INPUTS + "/" + item["crate"] + "/" + item["macro"] + ".json"
        catalog_data.recover_inputs(missing_rows)
        analysis.save(ROOT / "tools/typed-data.json", registry)
        fingerprints = {}
        for item in rows:
            if item.get("maintenance_recovery", {}).get("full_constructor_fingerprint"):
                continue
            fingerprint = fingerprints.get(item["crate"])
            if fingerprint is None:
                fingerprint = fingerprints[item["crate"]] = catalog_data.Fingerprint(ROOT / "crates" / item["crate"])
            value = json.loads((ROOT / item["source"]).read_bytes())
            item["maintenance_recovery"]["full_constructor_fingerprint"] = catalog_data.verify_constructors(item, value, fingerprint)
        analysis.save(ROOT / "tools/typed-data.json", registry)
    if sqlite_rows:
        import resource_database
        resource_database.prepare(check=args.check)
    schemas = {crate: RustData(ROOT / "crates" / crate) for crate in {item["crate"] for item in rows}}
    reports = [build_table(item["crate"], item["symbol"], item["module"], item["root_type"],
                           source=item["source"], macro_name=item["macro"], check=args.check,
                           allow_value=item.get("allow_value", False),
                           product_factory=item.get("product_factory", False), schema=schemas[item["crate"]]) for item in rows]
    specialized = {}
    if args.check and not args.symbol and (not args.crate or "razer-device" in args.crate):
        specialized["mapping_engine"] = check_mapping_engine_data()
        import typed_tables
        specialized["receiver_battery_peers"] = typed_tables.prepare_receiver_battery_peers(check=True, record=False)
        specialized["receiver_sender_tables"] = typed_tables.prepare_receiver_sender_tables(check=True, record=False)
    if args.check and not args.symbol and (not args.crate or "razer-storage" in args.crate):
        import typed_tables
        specialized["receiver_reset_storage"] = typed_tables.check_receiver_reset_storage()
    if record:
        record_maintenance_verification(registry, snapshots, len(reports) + len(sqlite_rows), specialized)
    return {"tables": len(reports) + len(sqlite_rows), "specialized_tables": specialized,
            "check_only": args.check, "passed": True}


def maintenance_verification_inputs(registry):
    """Snapshot regeneration inputs and outputs; retained source claims stay explicit."""
    import analysis
    import resource_database
    import typed_tables
    paths = {"Cargo.toml", "Cargo.lock", "tools/typed-data.json", "tools/rust_data.py", "tools/catalog_data.py", "tools/resource_database.py",
             "tools/analysis.py", "tools/asset_release.py", "tools/asset-release.json", "tools/resource-mappings.json",
             "tools/typed_tables.py", "tools/analyzers/python-static/acquisition.json",
             resource_database.INSTALLATION, resource_database.SCHEMA, resource_database.OUTPUT,
              resource_database.PRODUCT_REGISTRY, resource_database.PRODUCT_HELP, EMOJI_SEARCH_INPUT,
              MAPPING_ENGINE_SOURCE, "tools/families.py", typed_tables.RESET_INPUT}
    _, battery_peer_inputs = typed_tables.receiver_battery_peer_inputs()
    paths.update(item["path"] for item in battery_peer_inputs)
    paths.add(typed_tables.BATTERY_PEER_OUTPUT)
    import host
    import families
    sender_audit = host.HostLocalAudit()
    for pid in (164, 179, 241):
        families.receiver_sender_sources(sender_audit, pid)
    paths.update(item["path"] for item in sender_audit.artifacts.values() if item["path"] != ".analysis/inputs.json")
    paths.update((typed_tables.QUERY_OUTPUT, typed_tables.PAIR_CAP_OUTPUT))
    for recipe in registry["tables"]:
        source = analysis.resolve(recipe["source"], INPUTS)
        if analysis.sha(source) != recipe["source_sha256"]:
            raise ValueError("Maintenance input identity differs: " + recipe["source"])
        paths.add(recipe["source"])
        paths.add(recipe["module"])
        paths.update(row["path"] for row in recipe.get("shared_inputs", []))
    for source in registry.get("emoji_search", {}).get("original_sources", []):
        name = source if isinstance(source, str) else source["path"]
        paths.add(name)
        path = (ROOT / name).resolve()
        receipt = next((parent / "acquisition.json" for parent in path.parents
                        if parent.is_relative_to(ROOT / ".analysis/inputs/frontend") and (parent / "acquisition.json").is_file()), None)
        if receipt is None:
            raise ValueError("Emoji source has no acquisition receipt: " + name)
        paths.add(receipt.relative_to(ROOT).as_posix())
    paths.update(path.relative_to(ROOT).as_posix() for path in (ROOT / "crates").glob("*/Cargo.toml"))
    paths.update(path.relative_to(ROOT).as_posix() for path in (ROOT / "crates").rglob("*.rs"))
    paths.update(path.relative_to(ROOT).as_posix() for directory in (ROOT / "crates").glob("*/testdata")
                 for path in directory.rglob("*") if path.is_file())
    paths.update(path.relative_to(ROOT).as_posix() for path in (ROOT / "src").rglob("*.rs"))
    return [{"path": name, "bytes": (ROOT / name).stat().st_size, "sha256": analysis.sha(ROOT / name)}
            for name in sorted(paths)]


def record_maintenance_verification(registry, snapshots, tables, specialized_tables=None):
    import analysis
    from importlib.metadata import version
    import sqlite3
    import subprocess
    if snapshots != maintenance_verification_inputs(registry):
        raise ValueError("Maintenance inputs changed during complete verification")
    indexed = []
    for name in sorted({row["source"] for row in registry["tables"]}):
        item = next(row for row in snapshots if row["path"] == name)
        indexed.append({**item, "kind": "typed_table_maintenance_input", "maintenance_only": True,
                        "vendor_source_provenance_established": False})
    analysis.verify_source_rows(indexed)
    analysis.index_files(indexed)
    path = ROOT / ".analysis/results/typed-data.json"
    report = json.loads(path.read_bytes()) if path.is_file() else {"schema_version": 1}
    report.pop("maintenance_verification", None)
    report["maintenance_inputs"] = {
        "recorded_at_utc": analysis.stamp(), "command": "python tools/workspace.py prepare-typed-data --check --record-verification",
        "tables": tables, "specialized_tables": specialized_tables or {},
        "inputs": snapshots, "maintenance_inputs": indexed,
        "tools": {"python": sys.version.split()[0], "sqlite": sqlite3.sqlite_version,
                  "rustfmt": subprocess.run(["rustfmt", "--version"], cwd=ROOT, capture_output=True,
                                            text=True, check=True).stdout.strip(),
                  "static_analyzers": {name: version(name) for name in ("tree-sitter", "tree-sitter-javascript", "tree-sitter-rust", "json5")}},
        "regenerated_rust_and_complete_sqlite_bytes_equal": True,
        "vendor_source_provenance_established": False,
        "scope": "Every registered maintenance input, full typed schema, generated constructor/reader and packaged database was compared without product writes; constructor projections are not vendor originals",
        "target_execution": False, "passed": True}
    analysis.save(path, report)
