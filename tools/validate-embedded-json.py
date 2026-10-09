"""Validate embedded JSON data files against the Rust structs that read them.

Every `include_str!("*.json")` file is parsed at runtime, so a missing required
field is a startup panic rather than a compile error. This check statically
parses the struct definitions in the same Rust file and verifies that each
embedded document satisfies at least one of them, including nested `Vec<Struct>`
and struct fields. Fields marked `#[serde(default)]`, `Option<...>` or
`#[serde(flatten)]` are not required.

This tool only reads our own sources and the embedded JSON; it never runs the
application, tests or downloaded JavaScript.
"""

from __future__ import annotations

import json
import importlib
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
FIELD = re.compile(r"(?P<attributes>(?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?(?P<name>\w+)\s*:\s*(?P<type>[^,]+),")
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
SKIP_TYPES = ("Value", "String", "str", "bool", "u8", "u16", "u32", "u64", "usize", "i32", "i64", "f32", "f64")
TYPE_ALIAS = re.compile(r"\btype\s+(\w+)\s*=\s*([^;]+);")


def check_alias_type(document, type_name, aliases, lookup, path, errors, seen=()):
    """Validate local map aliases before looking up same-named global structs.

    A feature's `type Catalog = BTreeMap<...>` must not resolve to another
    feature's `struct Catalog`. Nested map keys and translation values are
    checked rather than treating the alias as an unvalidated JSON document.
    """
    type_name = type_name.strip()
    if type_name in aliases:
        if type_name in seen:
            errors.append(f"{path}: cyclic type alias `{type_name}`")
        else:
            check_alias_type(document, aliases[type_name], aliases, lookup, path, errors, (*seen, type_name))
        return
    generic = re.fullmatch(r"(\w+)\s*<(.+)>", type_name, re.S)
    if generic:
        outer, inner = generic.groups()
        if outer in ("BTreeMap", "HashMap"):
            depth = 0
            separator = None
            for index, char in enumerate(inner):
                depth += (char == "<") - (char == ">")
                if char == "," and depth == 0:
                    separator = index
                    break
            if separator is None or not isinstance(document, dict):
                errors.append(f"{path}: expected a map for `{type_name}`")
                return
            key_type, value_type = inner[:separator].strip(), inner[separator + 1:].strip()
            for key, value in document.items():
                if re.fullmatch(r"u(?:8|16|32|64)", key_type):
                    if not re.fullmatch(r"[0-9]+", key) or int(key) >= 2 ** int(key_type[1:]):
                        errors.append(f"{path}: invalid `{key_type}` key {key!r}")
                elif key_type != "String":
                    errors.append(f"{path}: unsupported map key `{key_type}`")
                check_alias_type(value, value_type, aliases, lookup, f"{path}[{key!r}]", errors, seen)
            return
        if outer in ("Option", "Box"):
            if outer != "Option" or document is not None:
                check_alias_type(document, inner, aliases, lookup, path, errors, seen)
            return
        if outer == "Vec" and isinstance(document, list):
            for index, value in enumerate(document):
                check_alias_type(value, inner, aliases, lookup, f"{path}[{index}]", errors, seen)
            return
    if type_name == "String":
        if not isinstance(document, str):
            errors.append(f"{path}: expected a string")
        return
    fields = lookup(type_name)
    if fields is None or not isinstance(document, dict):
        errors.append(f"{path}: cannot validate `{type_name}` as an object")
        return
    for name, field in fields.items():
        key = field["rename"] or name
        if field["flatten"]:
            errors.append(f"{path}: flattened aliases require an explicit schema")
        elif key in document:
            check_alias_type(document[key], field["type"], aliases, lookup, f"{path}.{key}", errors, seen)
        elif not field["optional"]:
            errors.append(f"{path}: missing field `{key}`")


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
        for field in FIELD.finditer(match.group("body")):
            attributes = field.group("attributes")
            fields[field.group("name")] = {
                "type": field.group("type").strip(),
                "optional": bool(SERDE_DEFAULT.search(attributes)) or "Option<" in field.group("type"),
                "flatten": "#[serde(flatten" in attributes,
                "rename": (re.search(r'rename\s*=\s*"([^"]+)"', attributes) or [None, None])[1]
                or rename_field(field.group("name"), rename_all),
            }
        structs[match.group("name")] = fields
    return structs


def struct_in(type_name: str, lookup) -> dict | None:
    """Return the fields of the struct a Rust type refers to, if any."""
    cleaned = re.sub(r"^(Option|Vec|Box)\s*<(.+)>$", r"\2", type_name.strip())
    cleaned = cleaned.split("<")[0].strip().rstrip("?").strip()
    return lookup(cleaned)


def check_document(document, fields: dict, lookup, path: str, errors: list[str]) -> None:
    if not isinstance(document, dict):
        errors.append(f"{path}: expected an object for this struct")
        return
    for name, field in fields.items():
        key = field["rename"] or name
        if key not in document:
            if not field["optional"] and not field["flatten"]:
                errors.append(f"{path}: missing field `{key}`")
            continue
        nested = struct_in(field["type"], lookup)
        if not nested:
            continue
        value = document[key]
        # Serde represents Option<Struct>::None as null unless its producer
        # elects to omit it. A defaulted non-Option struct still rejects null.
        if value is None and re.match(r"^Option\s*<", field["type"]):
            continue
        if isinstance(value, list):
            # A list of structs: every element must satisfy the struct.
            for index, element in enumerate(value):
                check_document(element, nested, lookup, f"{path}.{key}[{index}]", errors)
        else:
            check_document(value, nested, lookup, f"{path}.{key}", errors)


def accepts(document, structs: dict, lookup) -> list[str]:
    """Structs that the document satisfies without errors."""
    accepted = []
    for name, fields in structs.items():
        errors: list[str] = []
        if isinstance(document, list):
            for index, element in enumerate(document):
                check_document(element, fields, lookup, f"[{index}]", errors)
        else:
            check_document(document, fields, lookup, "", errors)
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
        for name, fields in load_structs(source.read_text(encoding="utf-8")).items():
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
        aliases = dict(TYPE_ALIAS.findall(text))
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
            function = text.rfind("\nfn ", 0, match.start())
            if function < 0:
                function = text.rfind("\npub", 0, match.start())
            declaration = text[max(0, function) : match.start()]
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
                        check_document(element, fields, lookup, f"[{index}]", errors)
                else:
                    check_document(document, fields, lookup, "", errors)
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
            if not accepts(document, structs, lookup):
                # Report the closest struct's errors to make the cause obvious.
                best: tuple[int, str, list[str]] | None = None
                for name, candidate in structs.items():
                    errors = []
                    if isinstance(document, list):
                        for index, element in enumerate(document):
                            check_document(element, candidate, lookup, f"[{index}]", errors)
                    else:
                        check_document(document, candidate, lookup, "", errors)
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
