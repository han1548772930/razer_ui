"""Validate embedded JSON data files against the Rust structs that read them.

Every `include_str!("*.json")` file is parsed at runtime, so a missing required
field is a startup panic rather than a compile error. This check statically
parses the struct definitions in the same Rust file and verifies that each
embedded document satisfies its resolved type, including primitive shape/range,
balanced generic maps, nested vectors, arrays, tuples and struct fields.
Serde unit enums with explicit variant names or standard rename rules are
validated too. Tagged/data-bearing enums and custom deserialize implementations
remain outside this tool's schema scope. Fields marked `#[serde(default)]`, `Option<...>` or
`#[serde(flatten)]` are not required.

This tool only reads our own sources and the embedded JSON; it never runs the
application, tests or downloaded JavaScript.
"""

from __future__ import annotations

import json
import importlib
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
production_modules = importlib.import_module("audit-assets-current").production_modules
INCLUDE = re.compile(r'include_str!\(\s*"([^"]+\.json)"\s*\)')
STRUCT = re.compile(
    r"#\[derive\((?P<derives>[^)]*)\)\]\s*(?P<attributes>(?:#\[[^\]]*\]\s*)*)"
    r"(?:pub(?:\([^)]*\))?\s+)?struct\s+(?P<name>\w+)\s*(?:<[^>]*>)?\s*\{(?P<body>[^}]*)\}",
    re.S,
)
UNIT_ENUM = re.compile(STRUCT.pattern.replace(r"struct\s+", r"enum\s+"), re.S)
ENUM_VARIANTS = "__serde_unit_enum_variants__"
FIELD = re.compile(r"(?P<attributes>(?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?(?P<name>\w+)\s*:\s*(?P<type>.+)", re.S)
# `#[serde(default)]` and named defaults such as `#[serde(rename = "x", default)]`.
SERDE_DEFAULT = re.compile(r"serde\([^)]*\bdefault\b")
# The include site declares its target type through a OnceLock or a turbofish.
DECLARED = (
    re.compile(r"let\s+\w+\s*:\s*(\w+)\s*=\s*serde_json::from_str\s*\($"),
    re.compile(r"from_str::<Vec<(\w+)>>\s*\($"),
    re.compile(r"from_str::<(\w+)>\s*\($"),
    re.compile(r"OnceLock<Vec<(\w+)>>"),
    re.compile(r"OnceLock<(\w+)>"),
    re.compile(r"from_str::<Vec<(\w+)>>"),
    re.compile(r"from_str::<(\w+)>"),
)
FUNCTION_START = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+\w+",
    re.M,
)
TYPE_ALIAS = re.compile(r"\btype\s+(\w+)\s*=\s*([^;]+);")


def split_types(text: str, separator=",") -> list[str]:
    """Split only outside Rust generic, tuple, array and attribute delimiters."""
    parts, stack = [], []
    start = 0
    quoted = escaped = False
    for index, char in enumerate(text):
        if quoted:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
            continue
        if char == '"':
            quoted = True
        elif char in "<([{":
            stack.append(char)
        elif char in ">)]}":
            if char == ">" and index and text[index - 1] == "-":
                continue  # Rust function return arrow, not a generic delimiter.
            if not stack or stack.pop() != {">": "<", ")": "(", "]": "[", "}": "{"}[char]:
                raise ValueError(f"Unbalanced Rust type delimiters: {text!r}")
        elif char == separator and not stack:
            parts.append(text[start:index].strip())
            start = index + 1
    if stack or quoted:
        raise ValueError(f"Unbalanced Rust type delimiters: {text!r}")
    parts.append(text[start:].strip())
    return parts


def strip_comments(text: str) -> str:
    """Preserve attribute strings while removing Rust line and block comments."""
    output, index = [], 0
    quoted = escaped = False
    while index < len(text):
        char = text[index]
        if quoted:
            output.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
            index += 1
        elif char == '"':
            quoted = True
            output.append(char)
            index += 1
        elif text.startswith("//", index):
            end = text.find("\n", index)
            index = len(text) if end < 0 else end
        elif text.startswith("/*", index):
            depth = 1
            index += 2
            while index < len(text) and depth:
                if text.startswith("/*", index):
                    depth += 1
                    index += 2
                elif text.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            if depth:
                raise ValueError("Unterminated Rust block comment")
            output.append(" ")
        else:
            output.append(char)
            index += 1
    return "".join(output)


def integer_range(type_name: str) -> tuple[int, int] | None:
    match = re.fullmatch(r"([ui])(8|16|32|64|128|size)", type_name)
    if not match:
        return None
    sign, size = match.groups()
    bits = sys.maxsize.bit_length() + 1 if size == "size" else int(size)
    return (0, 2 ** bits - 1) if sign == "u" else (-(2 ** (bits - 1)), 2 ** (bits - 1) - 1)


def load_aliases(text: str) -> dict[str, str]:
    """Only module-level aliases are visible to the module's data structs.

    An associated `impl Visitor { type Value = String; }` must never shadow
    the serde_json::Value imported by an unrelated struct in the same file.
    """
    text = strip_comments(text)
    candidates = {match.start(): match for match in TYPE_ALIAS.finditer(text)}
    aliases, depth, index = {}, 0, 0
    while index < len(text):
        if depth == 0 and index in candidates:
            match = candidates[index]
            aliases[match.group(1)] = match.group(2).strip()
        raw = re.match(r'r(#{0,255})"', text[index:]) if text[index] == "r" else None
        if raw:
            ending = '"' + raw.group(1)
            end = text.find(ending, index + raw.end())
            if end < 0:
                raise ValueError("Unterminated Rust raw string")
            index = end + len(ending)
            continue
        if text[index] == '"':
            index += 1
            while index < len(text):
                if text[index] == "\\":
                    index += 2
                elif text[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            continue
        char = re.match(r"'(?:\\.|[^'\\\r\n])'", text[index:]) if text[index] == "'" else None
        if char:
            index += char.end()
            continue
        depth += (text[index] == "{") - (text[index] == "}")
        index += 1
    return aliases


def check_alias_type(document, type_name, aliases, lookup, path, errors, seen=(), strict_unknown=True):
    """Validate complete types; a local alias wins over unrelated global structs.

    Struct fields and aliases share this checker, so primitive fields cannot
    silently bypass validation. Unknown external enums retain the existing
    struct-checking scope; generic containers never resolve to unrelated structs.
    """
    type_name = type_name.strip()
    if type_name in aliases:
        if type_name in seen:
            errors.append(f"{path}: cyclic type alias `{type_name}`")
        else:
            check_alias_type(document, aliases[type_name], aliases, lookup, path, errors, (*seen, type_name), strict_unknown)
        return
    simple = type_name.rsplit("::", 1)[-1]
    bounds = integer_range(simple)
    if bounds is not None:
        if type(document) is not int:
            errors.append(f"{path}: expected an integer for `{type_name}`")
        elif not bounds[0] <= document <= bounds[1]:
            errors.append(f"{path}: integer outside `{type_name}` range {bounds[0]}..{bounds[1]}")
        return
    if simple == "bool":
        if type(document) is not bool:
            errors.append(f"{path}: expected a boolean")
        return
    if simple in ("f32", "f64"):
        valid = type(document) in (int, float)
        try:
            valid = valid and math.isfinite(document) and (simple != "f32" or abs(document) <= 3.4028234663852886e38)
        except OverflowError:
            valid = False
        if not valid:
            errors.append(f"{path}: expected a finite number within `{type_name}` range")
        return
    if simple in ("String", "str"):
        if not isinstance(document, str):
            errors.append(f"{path}: expected a string")
        return
    if type_name in ("Value", "serde_json::Value"):
        return
    generic = re.fullmatch(r"([\w:]+)\s*<(.+)>", type_name, re.S)
    if generic:
        outer, inner = generic.groups()
        outer = outer.rsplit("::", 1)[-1]
        arguments = split_types(inner)
        if arguments[-1] == "":
            arguments.pop()  # Rust allows a trailing generic-argument comma.
        if outer in ("BTreeMap", "HashMap"):
            if len(arguments) != 2 or not isinstance(document, dict):
                errors.append(f"{path}: expected a map for `{type_name}`")
                return
            key_type, value_type = arguments
            for key, value in document.items():
                key_bounds = integer_range(key_type)
                if key_bounds is not None:
                    key_shape = r"[+]?[0-9]+" if key_type.startswith("u") else r"[+-]?[0-9]+"
                    if not re.fullmatch(key_shape, key) or not key_bounds[0] <= int(key) <= key_bounds[1]:
                        errors.append(f"{path}: invalid `{key_type}` key {key!r}")
                elif key_type != "String":
                    errors.append(f"{path}: unsupported map key `{key_type}`")
                check_alias_type(value, value_type, aliases, lookup, f"{path}[{key!r}]", errors, seen, strict_unknown)
            return
        if outer in ("Option", "Box"):
            if len(arguments) != 1:
                errors.append(f"{path}: invalid `{type_name}` type arguments")
                return
            if outer != "Option" or document is not None:
                check_alias_type(document, arguments[0], aliases, lookup, path, errors, seen, strict_unknown)
            return
        if outer in ("Vec", "BTreeSet", "HashSet"):
            if len(arguments) != 1 or not isinstance(document, list):
                errors.append(f"{path}: expected an array for `{type_name}`")
                return
            for index, value in enumerate(document):
                check_alias_type(value, arguments[0], aliases, lookup, f"{path}[{index}]", errors, seen, strict_unknown)
            return
        errors.append(f"{path}: unsupported generic type `{type_name}`")
        return
    if type_name.startswith("[") and type_name.endswith("]"):
        arguments = split_types(type_name[1:-1], ";")
        if len(arguments) != 2 or not arguments[1].isdigit():
            errors.append(f"{path}: unsupported array type `{type_name}`")
            return
        length = int(arguments[1])
        if not isinstance(document, list) or len(document) != length:
            errors.append(f"{path}: expected an array of length {length} for `{type_name}`")
            return
        for index, value in enumerate(document):
            check_alias_type(value, arguments[0], aliases, lookup, f"{path}[{index}]", errors, seen, strict_unknown)
        return
    if type_name.startswith("(") and type_name.endswith(")"):
        arguments = [argument for argument in split_types(type_name[1:-1]) if argument]
        if not isinstance(document, list) or len(document) != len(arguments):
            errors.append(f"{path}: expected a tuple array of length {len(arguments)} for `{type_name}`")
            return
        for index, (value, argument) in enumerate(zip(document, arguments)):
            check_alias_type(value, argument, aliases, lookup, f"{path}[{index}]", errors, seen, strict_unknown)
        return
    fields = lookup(type_name)
    if fields is None:
        if strict_unknown:
            errors.append(f"{path}: cannot resolve `{type_name}`")
        return
    check_document(document, fields, lookup, path, errors, aliases, seen, strict_unknown)


def rename_field(name: str, rename_all: str | None) -> str:
    """Apply the struct's `rename_all` rule to a field name."""
    if rename_all == "camelCase":
        head, *tail = name.split("_")
        return head + "".join(part.title() for part in tail)
    if rename_all == "PascalCase":
        return "".join(part.title() for part in name.split("_"))
    if rename_all == "kebab-case":
        return name.replace("_", "-")
    if rename_all == "SCREAMING_SNAKE_CASE":
        return name.upper()
    if rename_all == "lowercase":
        return name.lower()
    if rename_all == "UPPERCASE":
        return name.upper()
    return name


def load_structs(text: str) -> dict[str, dict]:
    structs: dict[str, dict] = {}
    for match in STRUCT.finditer(text):
        if "Deserialize" not in match.group("derives"):
            continue
        rename_all = (re.search(r'rename_all\s*=\s*"([^"]+)"', match.group("attributes")) or [None, None])[1]
        fields: dict[str, dict] = {}
        # A doc line such as `/// Source parser byte: ordinary ...` is not
        # a field declaration. Keep serde attributes and string values intact.
        body = strip_comments(match.group("body"))
        for declaration in split_types(body):
            if not declaration:
                continue
            field = FIELD.fullmatch(declaration)
            if field is None:
                raise ValueError(f"Cannot parse field in `{match.group('name')}`: {declaration!r}")
            attributes = field.group("attributes")
            fields[field.group("name")] = {
                "type": field.group("type").strip(),
                "optional": bool(SERDE_DEFAULT.search(attributes)) or bool(re.match(r"^(?:std::option::)?Option\s*<", field.group("type"))),
                "flatten": "#[serde(flatten" in attributes,
                "rename": (re.search(r'rename\s*=\s*"([^"]+)"', attributes) or [None, None])[1]
                or rename_field(field.group("name"), rename_all),
            }
        structs[match.group("name")] = fields
    return structs


def load_unit_enums(text: str) -> dict[str, dict]:
    """Only accept all-unit serde enums; never guess tagged/payload schemas."""
    result = {}
    for match in UNIT_ENUM.finditer(strip_comments(text)):
        attributes = match.group("attributes")
        if "Deserialize" not in match.group("derives") or re.search(
            r"\b(?:tag|content|untagged|from|try_from)\b", attributes
        ):
            continue
        rule = (re.search(r'rename_all\s*=\s*"([^"]+)"', attributes) or [None, None])[1]
        if rule not in (None, "snake_case", "SCREAMING_SNAKE_CASE", "kebab-case",
                        "SCREAMING-KEBAB-CASE", "camelCase", "PascalCase", "lowercase", "UPPERCASE"):
            continue
        variants = []
        for declaration in split_types(match.group("body")):
            if not declaration:
                continue
            unit = re.fullmatch(r'(?P<attrs>(?:#\[[^\]]*\]\s*)*)(?P<name>\w+)', declaration)
            if unit is None or re.search(r'\b(?:other|skip|skip_deserializing)\b', unit.group("attrs")):
                break
            name = unit.group("name")
            renamed = re.search(r'rename\s*=\s*"([^"]+)"', unit.group("attrs"))
            if renamed:
                name = renamed.group(1)
            elif rule in ("snake_case", "SCREAMING_SNAKE_CASE", "kebab-case", "SCREAMING-KEBAB-CASE", "camelCase"):
                # Serde's enum rule inserts a separator before each uppercase
                # letter after the first (including acronym letters).
                snake = re.sub(r'(?<!^)([A-Z])', r'_\1', name).lower()
                if rule == "camelCase":
                    name = name[:1].lower() + name[1:]
                elif rule == "snake_case":
                    name = snake
                elif rule == "SCREAMING_SNAKE_CASE":
                    name = snake.upper()
                elif rule == "kebab-case":
                    name = snake.replace("_", "-")
                else:
                    name = snake.upper().replace("_", "-")
            elif rule == "lowercase":
                name = name.lower()
            elif rule == "UPPERCASE":
                name = name.upper()
            variants.append(name)
            variants.extend(re.findall(r'alias\s*=\s*"([^"]+)"', unit.group("attrs")))
        else:
            result[match.group("name")] = {ENUM_VARIANTS: frozenset(variants)}
    return result


def check_document(document, fields: dict, lookup, path: str, errors: list[str], aliases=None, seen=(), strict_unknown=False) -> None:
    if ENUM_VARIANTS in fields:
        if not isinstance(document, str) or document not in fields[ENUM_VARIANTS]:
            errors.append(f"{path}: unknown unit enum variant {document!r}; expected {sorted(fields[ENUM_VARIANTS])}")
        return
    if not isinstance(document, dict):
        errors.append(f"{path}: expected an object for this struct")
        return
    for name, field in fields.items():
        key = field["rename"] or name
        if field["flatten"]:
            if strict_unknown:
                errors.append(f"{path}: flattened aliases require an explicit schema")
            continue
        if key not in document:
            if not field["optional"] and not field["flatten"]:
                errors.append(f"{path}: missing field `{key}`")
            continue
        check_alias_type(document[key], field["type"], aliases or {}, lookup,
                         f"{path}.{key}", errors, seen, strict_unknown)


def accepts(document, structs: dict, lookup, aliases=None) -> list[str]:
    """Structs that the document satisfies without errors."""
    accepted = []
    for name, fields in structs.items():
        errors: list[str] = []
        if isinstance(document, list):
            for index, element in enumerate(document):
                check_document(element, fields, lookup, f"[{index}]", errors, aliases)
        else:
            check_document(document, fields, lookup, "", errors, aliases)
        if not errors:
            accepted.append(name)
    return accepted


def main() -> int:
    failures: list[str] = []
    skipped: list[str] = []
    checked = 0
    # Struct definitions can live in another module of the same feature, so the
    # declared type at the include site is resolved against every scanned file.
    # Names are reused across features (`Spec` appears in many), so the same file
    # wins and an ambiguous name is reported instead of guessed.
    sources = production_modules()
    defined: dict[str, list[tuple[Path, dict]]] = {}
    for source in sources:
        text = source.read_text(encoding="utf-8")
        for name, fields in {**load_structs(text), **load_unit_enums(text)}.items():
            defined.setdefault(name, []).append((source, fields))

    def resolve(name: str | None, source: Path) -> tuple[str | None, dict | None]:
        if not name or name not in defined:
            return None, None
        candidates = defined[name]
        for file, fields in candidates:
            if file == source:
                return name, fields
        same_crate = [(file, fields) for file, fields in candidates
                      if file.relative_to(ROOT).parts[:2] == source.relative_to(ROOT).parts[:2]]
        if len(same_crate) == 1:
            return name, same_crate[0][1]
        if len(candidates) == 1:
            return name, candidates[0][1]
        return name, None

    for source in sources:
        text = source.read_text(encoding="utf-8").split("#[cfg(test)]\nmod tests")[0]
        structs = load_structs(text)
        aliases = load_aliases(text)
        for match in INCLUDE.finditer(text):
            target = (source.parent / match.group(1)).resolve()
            relative = target.relative_to(ROOT).as_posix()
            if not target.exists():
                failures.append(f"{relative}: included file does not exist")
                continue
            try:
                document = json.loads(target.read_text(encoding="utf-8"))
            except json.JSONDecodeError as error:
                failures.append(f"{relative}: invalid JSON ({error})")
                continue
            checked += 1
            # Prefer the type the include site declares; only fall back to the
            # file's own structs when the declaration cannot be resolved.
            # Scope the search to the enclosing function so a neighbouring
            # declaration cannot be mistaken for this one.
            functions = list(FUNCTION_START.finditer(text, 0, match.start()))
            function = functions[-1].start() if functions else 0
            declaration = text[function : match.start()]
            declared = next(
                (found.group(1) for pattern in DECLARED for found in [pattern.search(declaration)] if found),
                None,
            )
            if declared in aliases:
                errors = []
                check_alias_type(document, declared, aliases, lambda name: resolve(name, source)[1], "", errors)
                if errors:
                    failures.append(f"{relative}: does not satisfy local alias `{declared}` ({'; '.join(errors[:4])})")
                continue
            resolved, fields = resolve(declared, source)
            if resolved and fields is None:
                skipped.append(
                    f"{relative}: `{resolved}` is defined in several files ({', '.join(file.relative_to(ROOT).as_posix() for file, _ in defined[resolved])})"
                )
                continue
            if fields is not None:
                lookup = lambda name: resolve(name, source)[1]
                errors: list[str] = []
                if isinstance(document, list):
                    for index, element in enumerate(document):
                        check_document(element, fields, lookup, f"[{index}]", errors, aliases)
                else:
                    check_document(document, fields, lookup, "", errors, aliases)
                if errors:
                    failures.append(
                        f"{relative}: does not satisfy `{declared}` ({'; '.join(errors[:4])})"
                    )
                continue
            if not structs:
                skipped.append(
                    f"{relative}: no Deserialize struct in {source.name}"
                    + (f" and `{declared}` is not resolved in production modules" if declared else "")
                )
                continue
            lookup = lambda name: resolve(name, source)[1]
            if not accepts(document, structs, lookup, aliases):
                # Report the closest struct's errors to make the cause obvious.
                best: tuple[int, str, list[str]] | None = None
                for name, candidate in structs.items():
                    errors = []
                    if isinstance(document, list):
                        for index, element in enumerate(document):
                            check_document(element, candidate, lookup, f"[{index}]", errors, aliases)
                    else:
                        check_document(document, candidate, lookup, "", errors, aliases)
                    if best is None or len(errors) < best[0]:
                        best = (len(errors), name, errors)
                assert best is not None
                detail = "; ".join(best[2][:4]) or "no matching struct"
                failures.append(
                    f"{relative}: no struct in {source.name} accepts it (closest `{best[1]}`: {detail})"
                )
    for line in skipped:
        print(f"skipped: {line}")
    for line in failures:
        print(f"error: {line}")
    print(f"Checked {checked} embedded JSON documents; {len(failures)} failed, {len(skipped)} skipped.")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
