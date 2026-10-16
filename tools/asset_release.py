"""Raw Hugging Face resources and code-owned runtime metadata.

workspace.py owns the CLI. Cargo uses committed metadata without hydrating the
resource payload; the product fetches individual files through its asset service.
"""
from __future__ import annotations

import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import urllib.parse
import urllib.request

import analysis

ROOT = Path(__file__).resolve().parent.parent
LOCK = "tools/asset-release.json"
OUTPUT = ".work/asset-releases"
NATIVE_ICONS = (
    "a64b8a64b721fb74-tray-native.rgba", "f979021796dfd342-tray-app.rgba",
    "81851a9004a803d3-tray-icon-24.rgba", "71b354f660adb5db-tray-icon.rgba",
    "f8fd980e3bec82fe-tray-icon-40.rgba", "5e5df98271550aca-tray-icon-48.rgba",
    "afb1350c23ce2416-tray-icon-64.rgba", "7747f51f284ded65-tray-gear-dark.rgba",
    "33892a0b56b91958-tray-gear-light.rgba",
)
GIT_ICONS = {"assets/synapse/shared/pending-source-audit/" + n for n in NATIVE_ICONS}
META_FILES = {".gitattributes", "README.md", "release-index.json"}
LFS_ATTRIBUTES = (b"* -filter -diff -merge -text\n"
                  b"assets/** filter=lfs diff=lfs merge=lfs -text\n")
RUNTIME_METADATA = "assets/synapse/resource-catalog.sqlite3"
REMOTE_FONT_PATHS = tuple("assets/synapse/shared/fonts/" + name for name in (
    "104d76669a1810f2-Roboto-Light.ttf", "202d2e67f0ed60f1-Roboto-LightItalic.ttf",
    "179ca811a216df59-Roboto-Regular.ttf", "4bd8affd7aecd848-Roboto-Italic.ttf",
    "b5356da237055228-Roboto-Medium.ttf", "f9bfc6d616e97b51-Roboto-MediumItalic.ttf",
    "fc8488b1de752bbd-Roboto-Bold.ttf", "1c6453f110bf3eef-Roboto-BoldItalic.ttf",
    "0fa0aefa2bf1e00b-RazerF5-Thin.ttf", "ba98bf5a0ea8d551-RazerF5-Regular.ttf",
    "64293d40f66cda7e-RazerF5-SemiBold.ttf", "fa85dc64f3408613-RazerF5-Bold.ttf",
    "c79c356dc2b997ea-RazerF5-RegItalic.ttf", "85a911b833545640-RazerF5-BoldItalic.ttf",
))
# Original PID 1352 icon_external_link_sprite.0b292d50.svg <view> elements.
# These are presentation values; the lock separately identifies the original
# bytes used for integrity checking, including both fragment aliases.
SPRITE_VIEWS = (("link", "0 0 20 20"), ("hover", "0 20 20 20"))


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def published_manifest(lock):
    # A commit cannot contain its own hash; only the source lock pins storage.
    return {k: v for k, v in lock.items() if k != "storage"}


def content_hash(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                    separators=(",", ":")).encode("utf-8")).hexdigest()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def canonical(name):
    if (not isinstance(name, str) or not name.startswith("assets/")
            or any(c in name for c in '\\:<>"|?*') or any(ord(c) < 32 for c in name)
            or any(p in ("", ".", "..") or p.endswith((" ", ".")) for p in name.split("/"))
            or any(re.fullmatch(r"(?i)(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?", p)
                   for p in name.split("/"))):
        raise ValueError("Noncanonical asset path: " + repr(name))
    return name


def source_path(name, root=None):
    canonical(name)
    root = (root or ROOT).resolve()
    path, current = root / name, root / name
    while current != root:
        if (current.is_symlink() or (current.exists()
                and getattr(current.lstat(), "st_file_attributes", 0) & 0x400)):
            raise ValueError("Asset path traverses a link: " + name)
        current = current.parent
    if not path.resolve().is_relative_to(root / "assets"):
        raise ValueError("Asset path escaped assets/: " + name)
    return path


def in_git(name):
    return (name.startswith("assets/synapse/tables/") and name.endswith(".rs")) or name in GIT_ICONS


def identity(name):
    path = source_path(name)
    if not path.is_file():
        raise ValueError("Missing asset source: " + name)
    size = path.stat().st_size
    if size < 1024 and path.read_bytes().startswith(b"version https://git-lfs.github.com/spec/v1\n"):
        raise ValueError("Hydrate the Git LFS pointer first: " + name)
    return {"path": name, "bytes": size, "sha256": digest(path)}


def group_name(name):
    match = re.match(r"assets/synapse/products/([1-9][0-9]*)/", name)
    if name.startswith("assets/synapse/products/") and not match:
        raise ValueError("Asset product directory has no canonical PID: " + name)
    return "product-" + match[1] if match else "shared"


def inventory():
    paths = sorted(p.relative_to(ROOT).as_posix() for p in (ROOT / "assets").rglob("*")
                   if p.is_file() and p != ROOT / "assets/synapse.pack"
                   and p not in (ROOT / RUNTIME_METADATA, ROOT / (RUNTIME_METADATA + ".partial"))
                   and not (p.parent == ROOT / "assets" and p.name.startswith("synapse.pack.")
                            and p.name.endswith(".tmp")))
    if not paths or len({p.casefold() for p in paths}) != len(paths):
        raise ValueError("Asset inventory is empty or contains Windows path collisions")
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        rows = list(executor.map(identity, paths))
    groups, retained = {"shared": []}, []
    for row in rows:
        if in_git(row["path"]):
            retained.append(row)
        else:
            groups.setdefault(group_name(row["path"]), []).append(row)
    result = []
    for name, files in sorted(groups.items()):
        required = set() if name == "shared" else {"shared"}
        if name != "shared":
            pid = int(name.removeprefix("product-"))
            index = ROOT / f"assets/synapse/products/{pid}/resources.json"
            if index.is_file():
                data = json.loads(index.read_bytes())
                if data.get("product_id") != pid:
                    raise ValueError("Product resource index identity differs: " + str(pid))
                for item in data.get("resources", []):
                    dependency = group_name(canonical(item["physical_path"]))
                    if dependency != name:
                        required.add(dependency)
        if not required <= groups.keys():
            raise ValueError("Missing cross-product resource dependency: " + name)
        result.append({"name": name, "requires": sorted(required), "files": files})
    return result, retained


def repository_name(value):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", value or ""):
        raise ValueError("Expected a Hugging Face owner/dataset")
    return value


def rows_for(lock, groups=None):
    return [r for g in (lock["groups"] if groups is None else groups) for r in g["files"]]


def matches(row, root=None):
    path = source_path(row["path"], root)
    return path.is_file() and path.stat().st_size == row["bytes"] and digest(path) == row["sha256"]


def copy_verified(row, source_root, destination_root):
    source, target = source_path(row["path"], source_root), source_path(row["path"], destination_root)
    if not source.is_file():
        raise ValueError("Missing snapshot source: " + row["path"])
    target.parent.mkdir(parents=True, exist_ok=True)
    count, h = 0, hashlib.sha256()
    with source.open("rb") as stream, target.open("xb") as output:
        while block := stream.read(min(1024 * 1024, row["bytes"] - count + 1)):
            count += len(block)
            if count > row["bytes"]:
                raise ValueError("Snapshot file exceeds its locked length: " + row["path"])
            h.update(block)
            output.write(block)
    if count != row["bytes"] or h.hexdigest() != row["sha256"]:
        raise ValueError("Snapshot file SHA-256 differs: " + row["path"])


def prepare(repository=None):
    old = json.loads((ROOT / LOCK).read_bytes()) if (ROOT / LOCK).is_file() else {}
    repository = repository_name(repository or old.get("storage", {}).get("repository"))
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        groups, retained = inventory()
        source_hash = content_hash({"groups": groups, "git_files": retained})
        lock = {"schema_version": 2, "format": "files", "tag": "assets-" + source_hash[:20],
                "source_sha256": source_hash, "git_files": retained, "groups": groups,
                "storage": {"provider": "huggingface", "repository": repository, "revision": None}}
        if published_manifest(old) == published_manifest(lock) and old.get("storage", {}).get("repository") == repository:
            lock["storage"] = old["storage"]
        directory = ROOT / OUTPUT / lock["tag"]
        directory.mkdir(parents=True, exist_ok=True)
        for row in rows_for(lock):
            if not matches(row, directory):
                if source_path(row["path"], directory).exists():
                    raise ValueError("Prepared snapshot was edited: " + row["path"])
                copy_verified(row, ROOT, directory)
        (directory / "release-index.json").write_bytes(encoded(published_manifest(lock)))
        (directory / ".gitattributes").write_bytes(LFS_ATTRIBUTES)
        (directory / "README.md").write_text(
            "---\npretty_name: Razer UI local product resources\ntags:\n- desktop-ui\n---\n\n"
            f"Raw files for `{lock['tag']}`. Products retain `assets/synapse/products/<PID>/`; "
            "shared files retain their original paths. Resource payloads use Git LFS. No ZIP archives.\n\n"
            "The application's committed resource index pins this dataset to an immutable revision. "
            "It fetches individual files as screens use them, verifies their length and SHA-256, and "
            "retains them in its installation cache for offline reuse. Cargo needs no downloaded payload. "
            "`tools/workspace.py fetch-asset-release` is an explicit source-maintenance operation only.\n\n"
            "Vendor artwork, fonts and binaries retain their original rights. This dataset grants no license "
            "to vendor content.\n", encoding="utf-8", newline="\n")
        if inventory() != (groups, retained):
            raise ValueError("Asset sources changed during preparation; retry after writers finish")
        analysis.save(ROOT / LOCK, lock)
    return {"tag": lock["tag"], "directory": directory.relative_to(ROOT).as_posix(),
            "products": len(groups) - 1, "source_files": len(rows_for(lock)),
            "source_bytes": sum(r["bytes"] for r in rows_for(lock)), "git_files": len(retained), "passed": True}


def validate_lock(lock):
    storage = lock.get("storage", {})
    if storage.get("provider") != "huggingface":
        raise ValueError("Asset storage provider must be Hugging Face")
    repository_name(storage.get("repository"))
    if storage.get("revision") is not None and not re.fullmatch(r"[0-9a-f]{40}", storage["revision"]):
        raise ValueError("Asset downloads require a complete immutable HF commit")
    if (lock.get("schema_version") != 2 or lock.get("format") != "files"
            or not re.fullmatch(r"assets-[a-f0-9]{20}", lock.get("tag", ""))):
        raise ValueError("Invalid raw asset snapshot lock")
    seen, groups = set(), {}
    for group in lock.get("groups", []):
        name = group.get("name", "")
        if not re.fullmatch(r"shared|product-[1-9][0-9]*", name) or name in groups:
            raise ValueError("Invalid or repeated resource group")
        groups[name] = group
        for row in group.get("files", []):
            path = canonical(row["path"])
            if in_git(path) or path == "assets/synapse.pack" or group_name(path) != name:
                raise ValueError("Invalid asset group member: " + path)
    if "shared" not in groups or not 0 < len(rows_for(lock)) < 100000:
        raise ValueError("Invalid resource file set")
    for name, group in groups.items():
        requires = group.get("requires", [])
        if (not isinstance(requires, list) or len(requires) != len(set(requires))
                or not set(requires) <= groups.keys() or name in requires
                or (name != "shared" and "shared" not in requires)):
            raise ValueError("Invalid resource group dependencies")
    for row in lock.get("git_files", []):
        if not in_git(canonical(row["path"])):
            raise ValueError("Invalid retained Git resource")
    for row in rows_for(lock) + lock.get("git_files", []):
        if row["path"] == RUNTIME_METADATA:
            raise ValueError("The generated local catalog must not become a remote resource input")
        path = row["path"]
        if (path.casefold() in seen or type(row.get("bytes")) is not int or row["bytes"] < 0
                or not re.fullmatch(r"[a-f0-9]{64}", row.get("sha256", ""))):
            raise ValueError("Invalid or repeated resource identity: " + path)
        seen.add(path.casefold())
    actual = content_hash({"groups": lock["groups"], "git_files": lock["git_files"]})
    if actual != lock.get("source_sha256") or lock["tag"] != "assets-" + actual[:20]:
        raise ValueError("Resource snapshot identity differs from its lock")
    return lock


def load_lock():
    return validate_lock(json.loads((ROOT / LOCK).read_bytes()))


def builtin_data_path(crate, symbol, *, root=None):
    """Read the owning crate's data.rs instead of duplicating data paths."""
    if (not re.fullmatch(r"razer-[a-z][a-z0-9-]*", crate)
            or not re.fullmatch(r"[A-Z][A-Z0-9_]*", symbol)):
        raise ValueError("Invalid compiled-data owner or symbol")
    root = (root or ROOT).resolve()
    source = root / "crates" / crate / "src"
    registry = source / "data.rs"
    text = registry.read_text(encoding="utf-8")
    pattern = (r'pub\(crate\)\s+const\s+' + re.escape(symbol)
               + r':\s*&str\s*=\s*include_str!\(\s*"([^"\\]+)"\s*\)\s*;')
    bindings = re.findall(pattern, text)
    if not bindings:
        import typed_tables
        fixed = typed_tables.fixed_tables().get((crate, symbol))
        if fixed is not None:
            return root / fixed[0]
    if not bindings and (root / "tools/typed-data.json").is_file():
        recipes = json.loads((root / "tools/typed-data.json").read_bytes())["tables"]
        paths = {item["source"] for item in recipes if item["crate"] == crate and item["symbol"] == symbol}
        if len(paths) == 1:
            target = root / paths.pop()
            if not target.resolve().is_relative_to((root / ".analysis/inputs/typed-tables").resolve()):
                raise ValueError("Typed-data maintenance input escaped its owning directory")
            return target
    if len(bindings) != 1:
        raise ValueError("Compiled-data binding is missing or repeated: " + crate + "::" + symbol)
    target = Path(os.path.abspath(source / bindings[0]))
    if (not target.resolve().is_relative_to(source.resolve())
            or any(part in ("", ".", "..") for part in bindings[0].split("/"))
            or "\\" in bindings[0]):
        raise ValueError("Compiled-data path escaped its owning crate")
    current = target
    while current != source.parent:
        if (current.is_symlink() or (current.exists()
                and getattr(current.lstat(), "st_file_attributes", 0) & 0x400)):
            raise ValueError("Compiled-data path traverses a link")
        current = current.parent
    return target


def save_builtin_data(symbol, value, *, root=None):
    """Regenerate all explicit owners of a shared compiled data binding."""
    root = (root or ROOT).resolve()
    targets = []
    for registry in sorted((root / "crates").glob("*/src/data.rs")):
        if re.search(r'pub\(crate\)\s+const\s+' + re.escape(symbol) + r'\b', registry.read_text(encoding="utf-8")):
            targets.append(builtin_data_path(registry.parent.parent.name, symbol, root=root))
    registry = root / "tools/typed-data.json"
    recipes = json.loads(registry.read_bytes())["tables"] if registry.is_file() else []
    typed = [item for item in recipes if item["symbol"] == symbol]
    for item in typed:
        target = root / item["source"]
        if target not in targets:
            targets.append(target)
    import typed_tables
    targets.extend(root / item[0] for (owner, name), item in typed_tables.fixed_tables().items()
                   if name == symbol and (root / item[0]).is_file() and root / item[0] not in targets)
    if not targets:
        raise ValueError("Compiled-data binding has no owner: " + symbol)
    for target in targets:
        analysis.save(target, value)
    return targets


def regenerate_builtin_path(path, *, root=None):
    """Keep generated Rust synchronized when a maintenance workflow saves input."""
    root = (root or ROOT).resolve()
    import typed_tables
    typed_tables.regenerate_input(path, root=root)
    registry = root / "tools/typed-data.json"
    recipes = json.loads(registry.read_bytes())["tables"] if registry.is_file() else []
    matches = [item for item in recipes if (root / item["source"]).resolve() == Path(path).resolve()]
    if matches:
        import rust_data
        for item in matches:
            rust_data.build_table(item["crate"], item["symbol"], item["module"], item["root_type"],
                                 source=item["source"], macro_name=item["macro"],
                                 allow_value=item.get("allow_value", False),
                                 product_factory=item.get("product_factory", False))


def rust_array(text, start):
    """Read one literal Rust array without executing or compiling its source."""
    if text[start] != "[":
        raise ValueError("Expected a Rust array")
    depth, position = 0, start
    while position < len(text):
        token = text[position]
        if token == '"':
            position += 1
            while position < len(text) and text[position] != '"':
                position += 2 if text[position] == "\\" else 1
            if position >= len(text):
                raise ValueError("Unterminated string in Rust resource table")
        elif text.startswith("//", position):
            newline = text.find("\n", position)
            position = len(text) if newline == -1 else newline
            continue
        elif text.startswith("/*", position):
            closing = text.find("*/", position + 2)
            if closing == -1:
                raise ValueError("Unterminated comment in Rust resource table")
            position = closing + 2
            continue
        elif token == "[":
            depth += 1
        elif token == "]":
            depth -= 1
            if depth == 0:
                return text[start:position + 1]
        position += 1
    raise ValueError("Unterminated Rust resource array")


def runtime_metadata(lock=None, *, check_local=False, input_rows=None):
    """Resolve current alias semantics from source and immutable file identities.

    This does not consume analysis reports or require ignored payload files.
    check_local additionally verifies original bytes and sprite viewports when
    preparing a release from a fully hydrated developer workspace.
    """
    lock = load_lock() if lock is None else validate_lock(lock)
    files = {row["path"]: dict(row) for row in [*rows_for(lock), *lock["git_files"]]
             if not row["path"].startswith("assets/synapse/tables/")}
    def table_source(name):
        raw = source_path(name).read_bytes()
        if input_rows is not None:
            input_rows.append({"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        return raw.decode("utf-8")

    tables_path = "assets/synapse/tables/asset-tables-source.rs"
    table_text = table_source(tables_path)
    order = re.search(r'static ASSET_TABLES:[^=]+=\s*&\[', table_text)
    if order is None:
        raise ValueError("AssetSource table order is unresolved")
    names = re.findall(r'\b[A-Z][A-Z_0-9]+\b', rust_array(table_text, order.end() - 1))
    if not names or len(set(names)) != len(names):
        raise ValueError("Missing or repeated AssetSource table")

    def relative_input(owner, literal):
        path = Path(os.path.abspath(ROOT / owner / ".." / literal))
        try:
            name = path.relative_to(ROOT).as_posix()
        except ValueError as error:
            raise ValueError("Resource table input escaped the workspace") from error
        source_path(name)  # Validate spelling and links even when payload is absent.
        return name

    def row_for(name):
        if name not in files:
            raise ValueError("Runtime asset is absent from the resource lock: " + name)
        return dict(files[name])

    def entry(key, owner, literal):
        if (not key.startswith("synapse/") or any(ord(c) < 32 for c in key)
                or "\\" in key or any(p in ("", ".", "..") for p in key.split("/"))):
            raise ValueError("Invalid runtime asset alias: " + key)
        return {"key": key, **row_for(relative_input(owner, literal))}

    definitions = {}
    pattern = r'const ([A-Z][A-Z_0-9]+):\s*&\[\(&str,\s*&\[u8\]\)\]\s*=\s*'
    for match in re.finditer(pattern, table_text):
        expression = table_text[match.end():]
        if expression.startswith("include!"):
            include = re.match(r'include!\("([^"\\]+)"\)', expression)
            if include is None:
                raise ValueError("Nonliteral asset table include")
            name = relative_input(tables_path, include[1])
            definitions[match[1]] = (name, table_source(name))
        elif expression.startswith("&["):
            definitions[match[1]] = (tables_path, rust_array(table_text, match.end() + 1))
        else:
            raise ValueError("Unresolved AssetSource table: " + match[1])
    tuples = re.compile(r'\(\s*"([^"\\]+)"\s*,\s*include_bytes!\(\s*"([^"\\]+)"\s*\)\s*(?:as\s*&\[u8\]|\.as_slice\(\))?\s*,?\s*\)')
    table_entries = []
    for name in names:
        if name not in definitions:
            raise ValueError("Missing AssetSource table: " + name)
        owner, body = definitions[name]
        clean = re.sub(r'(?m)^\s*//[^\n]*', '', body)
        found_tuples = list(tuples.finditer(clean))
        if tuples.sub('', clean).strip("&[], \t\r\n"):
            raise ValueError("Unparsed AssetSource table content: " + name)
        table_entries.extend(entry(match[1], owner, match[2]) for match in found_tuples)

    demo_path = "assets/synapse/tables/audio-demo-controls-embedded.rs"
    sprite_path = "assets/synapse/tables/audio-volume-sprite-embedded.rs"
    demo_body = table_source(demo_path)
    demo_entries = [entry(match[1], demo_path, match[2]) for match in re.finditer(
        r'"([^"\\]+)"\s*=>\s*Some\(include_bytes!\("([^"\\]+)"\)\)', demo_body)]
    if {row["key"] for row in demo_entries} != {
            "synapse/audio-demo-control-play.svg", "synapse/audio-demo-control-pause.svg",
            "synapse/audio-demo-control-fullscreen.svg", "synapse/audio-demo-control-fullscreen-exit.svg"}:
        raise ValueError("Audio demo AssetSource aliases require a source audit")
    sprite_body = table_source(sprite_path)
    sprite_input = re.search(r'const AUDIO_VOLUME_SPRITE: &str = include_str!\("([^"\\]+)"\);', sprite_body)
    sprite_alias = re.search(r'const AUDIO_VOLUME_SPRITE_PATH: &str = "([^"\\]+)";', sprite_body)
    if sprite_input is None or sprite_alias is None:
        raise ValueError("Audio sprite AssetSource requires a source audit")
    sprite = entry(sprite_alias[1], sprite_path, sprite_input[1])
    sprite_entries = [sprite] + [
        {**sprite, "key": sprite["key"] + "#" + view, "view_box": viewport, "width": 20, "height": 20}
        for view, viewport in SPRITE_VIEWS]
    if check_local:
        checked = {row["path"] for row in [*table_entries, *demo_entries, sprite]}
        checked.update(REMOTE_FONT_PATHS)
        for name in checked:
            if not matches(row_for(name)):
                raise ValueError("Runtime source length or SHA-256 differs: " + name)
        sprite_text = source_path(sprite["path"]).read_text(encoding="utf-8")
        for view, viewport in SPRITE_VIEWS:
            if f'<view id="{view}" viewBox="{viewport}"' not in sprite_text:
                raise ValueError("Original sprite viewport changed: " + view)
    return {"schema_version": 1, "storage": dict(lock["storage"]),
            "entries": [*sprite_entries, *demo_entries, *table_entries],
            "list_entries": [*({"key": row["key"], "provider": "audio_demo_controls"} for row in demo_entries),
                             *({"key": row["key"], "provider": "audio_volume_sprite"} for row in sprite_entries),
                             *({"key": row["key"], "provider": "tables"} for row in table_entries)],
            "fonts": [row_for(name) for name in REMOTE_FONT_PATHS],
            "files": list(files.values())}


def prepare_runtime_metadata(*, check_local=False):
    import resource_database
    return resource_database.prepare(check_local=check_local)


def verify_runtime_metadata(*, check_local=False):
    import resource_database
    return resource_database.prepare(check=True, check_local=check_local)


def runtime_metadata_summary(value):
    return {"path": RUNTIME_METADATA, "aliases": len(value["entries"]),
            "list_entries": len(value["list_entries"]), "fonts": len(value["fonts"]),
            "files": len(value["files"]), "revision": value["storage"]["revision"], "passed": True}


def selected_groups(lock, products=None):
    if not products:
        return lock["groups"]
    groups = {g["name"]: g for g in lock["groups"]}
    selected = {"shared"} | {"product-" + str(p) for p in products}
    if not selected <= groups.keys():
        raise ValueError("Unknown asset PID: " + ", ".join(sorted(selected - groups.keys())))
    while True:
        before = set(selected)
        for name in before:
            selected.update(groups[name]["requires"])
        if selected == before:
            return [g for g in lock["groups"] if g["name"] in selected]


def verify(directory=None, check_local=False):
    lock = load_lock()
    directory = Path(directory).resolve() if directory else ROOT / OUTPUT / lock["tag"]
    if (directory / "release-index.json").read_bytes() != encoded(published_manifest(lock)):
        raise ValueError("Snapshot index differs from the checked-in lock")
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        results = list(executor.map(lambda r: matches(r, directory), rows_for(lock)))
    if not all(results):
        raise ValueError("Snapshot source SHA-256 or length differs")
    if check_local:
        groups, retained = inventory()
        if content_hash({"groups": groups, "git_files": retained}) != lock["source_sha256"]:
            raise ValueError("Local asset sources differ from the snapshot")
    return {"tag": lock["tag"], "source_files_verified": len(results),
            "local_sources_verified": check_local, "passed": True}


def git(*args, cwd=None):
    env = dict(os.environ, GIT_LFS_SKIP_SMUDGE="1", GIT_TERMINAL_PROMPT="0", GCM_INTERACTIVE="never")
    result = subprocess.run(["git", *args], cwd=cwd or ROOT, env=env, capture_output=True,
                            text=True, encoding="utf-8", errors="replace")
    if result.returncode:
        message = re.sub(r"hf_[A-Za-z0-9]+", "[redacted]", result.stderr.strip() or result.stdout.strip())
        raise ValueError("Git resource operation failed: " + message[-5000:])
    return result.stdout


def hf_headers():
    token = os.environ.get("HF_TOKEN")
    if not token:
        env = dict(os.environ, GIT_TERMINAL_PROMPT="0", GCM_INTERACTIVE="never")
        result = subprocess.run(["git", "credential", "fill"], cwd=ROOT, env=env,
                                input="protocol=https\nhost=huggingface.co\n\n", capture_output=True,
                                text=True, encoding="utf-8", errors="replace")
        credentials = dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)
        token = credentials.get("password")
    if not token:
        raise ValueError("Configure Hugging Face authentication in Git Credential Manager or HF_TOKEN")
    return {"Authorization": "Bearer " + token, "User-Agent": "razer-ui-asset-maintenance"}


def remote_tree(repository, revision):
    url = f"https://huggingface.co/api/datasets/{repository}/tree/{revision}?recursive=true&limit=1000"
    headers, files, pages = hf_headers(), {}, set()
    while url:
        if (urllib.parse.urlsplit(url).netloc != "huggingface.co" or not url.startswith("https://")
                or url in pages):
            raise ValueError("Unexpected HF pagination URL")
        pages.add(url)
        with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=120) as response:
            data = json.load(response)
            link = response.headers.get("Link", "")
        for item in data:
            if item["type"] == "file":
                if item["path"] in files:
                    raise ValueError("Repeated remote tree file")
                files[item["path"]] = item
        next_link = re.search(r'<([^>]+)>;\s*rel="next"', link)
        url = next_link[1] if next_link else None
    return files


def verify_remote_files(lock, tree):
    expected = {r["path"]: r for r in rows_for(lock)}
    retained = {r["path"]: r for r in lock["git_files"]}
    if not expected.keys() <= tree.keys() or tree.keys() - expected.keys() - retained.keys() - META_FILES:
        raise ValueError("Remote snapshot file set differs from its lock")
    verified = dict(expected)
    for name, row in verified.items():
        actual, lfs = tree[name], tree[name].get("lfs")
        if actual.get("size") != row["bytes"]:
            raise ValueError("Remote resource length differs: " + name)
        if lfs:
            if lfs.get("oid") != row["sha256"] or lfs.get("size") != row["bytes"]:
                raise ValueError("Remote LFS SHA-256 differs: " + name)
        else:
            roots = (ROOT, ROOT / OUTPUT / lock["tag"])
            evidence = next((root for root in roots if matches(row, root)), None)
            if evidence is None:
                raise ValueError("Local SHA-256 evidence is required to verify a remote Git blob: " + name)
            blob = source_path(name, evidence).read_bytes()
            oid = hashlib.sha1(b"blob " + str(len(blob)).encode("ascii") + b"\0" + blob).hexdigest()
            if actual.get("oid") != oid:
                raise ValueError("Remote Git blob differs from SHA-256-verified source: " + name)


def verify_remote(lock=None):
    lock = lock or load_lock()
    storage = lock["storage"]
    if not storage.get("revision"):
        raise ValueError("Resource snapshot has no verified immutable HF commit")
    tree = remote_tree(storage["repository"], storage["revision"])
    verify_remote_files(lock, tree)
    if "release-index.json" in tree:
        raw_expected = encoded(published_manifest(lock))
        url = f"https://huggingface.co/datasets/{storage['repository']}/resolve/{storage['revision']}/release-index.json"
        with urllib.request.urlopen(urllib.request.Request(url, headers=hf_headers()), timeout=120) as response:
            raw = response.read(len(raw_expected) + 1)
        if raw != raw_expected:
            raise ValueError("Remote manifest differs from the checked-in lock")
    return {"repository": storage["repository"], "revision": storage["revision"],
            "source_files_verified": len(rows_for(lock)),
            "lfs_files_verified": sum(bool(tree[r["path"]].get("lfs")) for r in rows_for(lock)),
            "git_blob_files_verified": sum(not bool(tree[r["path"]].get("lfs")) for r in rows_for(lock)),
            "retained_files_present": len(tree.keys() & {r["path"] for r in lock["git_files"]}),
            "remote_manifest_present": "release-index.json" in tree, "passed": True}


def pin_remote(revision):
    if not re.fullmatch(r"[a-f0-9]{40}", revision):
        raise ValueError("An immutable HF commit is required")
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        lock = load_lock()
        lock["storage"]["revision"] = revision
        report = verify_remote(lock)
        analysis.save(ROOT / LOCK, lock)
    return report


def checkout(lock, groups):
    storage = lock["storage"]
    if not storage.get("revision"):
        raise ValueError("Snapshot is not pinned; use --from-dir for offline raw files")
    directory = ROOT / OUTPUT / "downloads" / storage["revision"]
    directory.parent.mkdir(parents=True, exist_ok=True)
    url = "https://huggingface.co/datasets/" + storage["repository"]
    if not directory.exists():
        git("clone", "--no-checkout", url, str(directory))
    if git("remote", "get-url", "origin", cwd=directory).strip() != url:
        raise ValueError("Resource cache has an unexpected Git origin")
    git("config", "core.longpaths", "true", cwd=directory)
    git("lfs", "install", "--local", cwd=directory)
    git("fetch", "--depth=1", "origin", storage["revision"], cwd=directory)
    git("-c", "core.autocrlf=false", "checkout", "--detach", "FETCH_HEAD", cwd=directory)
    if (directory / "release-index.json").read_bytes() != encoded(published_manifest(lock)):
        raise ValueError("Downloaded snapshot manifest differs")
    if len(groups) == len(lock["groups"]):
        git("lfs", "fetch", "origin", storage["revision"], "--include=assets/**", "--exclude=", cwd=directory)
    else:
        git("lfs", "fetch", "origin", storage["revision"], "--include=assets/**",
            "--exclude=assets/synapse/products/**", cwd=directory)
        patterns = ["assets/synapse/products/" + g["name"].removeprefix("product-") + "/**"
                    for g in groups if g["name"] != "shared"]
        for offset in range(0, len(patterns), 100):
            git("lfs", "fetch", "origin", storage["revision"], "--include=" + ",".join(patterns[offset:offset + 100]),
                "--exclude=", cwd=directory)
    git("lfs", "checkout", cwd=directory)
    return directory


def restore(products=None, directory=None, replace=False):
    lock = load_lock()
    groups = selected_groups(lock, products)
    for row in lock["git_files"]:
        if not matches(row):
            raise ValueError("Use the matching checkout for this snapshot: " + row["path"])
    work = ROOT / ".work"
    work.mkdir(parents=True, exist_ok=True)
    restored, unchanged = 0, 0
    with analysis.index_lock(work / "resource-writer.lock"), tempfile.TemporaryDirectory(prefix="asset-restore-", dir=work) as temp:
        changes = []
        for row in rows_for(lock, groups):
            if matches(row):
                unchanged += 1
                continue
            target = source_path(row["path"])
            if target.exists() and not target.is_file():
                raise ValueError("Asset destination is not a regular file: " + row["path"])
            if target.exists() and not replace:
                raise ValueError("Local asset was edited; use --replace-existing to restore it: " + row["path"])
            changes.append(row)
        cache = Path(directory).resolve() if directory else checkout(lock, groups) if changes else None
        stage = Path(temp) / "new"
        if cache:
            if (cache / "release-index.json").read_bytes() != encoded(published_manifest(lock)):
                raise ValueError("Snapshot index differs from the checked-in lock")
            for row in changes:
                copy_verified(row, cache, stage)
        recovery_root = work / "asset-recovery"
        recovery_root.mkdir(parents=True, exist_ok=True)
        backups, applied, cleanup_backups = None, [], False
        try:
            if changes:
                backups = Path(tempfile.mkdtemp(prefix="restore-", dir=recovery_root))
            for row in changes:
                target = source_path(row["path"])
                if target.exists() and not target.is_file():
                    raise ValueError("Asset destination changed into a non-file: " + row["path"])
                target.parent.mkdir(parents=True, exist_ok=True)
                backup = source_path(row["path"], backups)
                had_old = target.exists()
                if had_old:
                    backup.parent.mkdir(parents=True, exist_ok=True)
                    os.replace(target, backup)
                applied.append((target, backup if had_old else None))
                os.replace(source_path(row["path"], stage), target)
                restored += 1
            cleanup_backups = True
        except BaseException as original:
            failures = []
            for target, backup in reversed(applied):
                try:
                    if backup:
                        os.replace(backup, target)
                    elif target.exists():
                        target.unlink()
                except BaseException as error:
                    failures.append(f"{target.relative_to(ROOT).as_posix()}: {type(error).__name__}")
            if failures:
                raise RuntimeError("Asset rollback needs recovery; backups preserved at "
                                   + backups.relative_to(ROOT).as_posix() + "; " + "; ".join(failures)) from original
            cleanup_backups = True
            raise
        finally:
            if backups and cleanup_backups:
                checked = backups.resolve()
                if checked == recovery_root.resolve() or not checked.is_relative_to(recovery_root.resolve()):
                    raise ValueError("Recovery cleanup escaped its dedicated directory")
                shutil.rmtree(checked)
    return {"tag": lock["tag"], "groups": sorted(g["name"] for g in groups),
            "restored_files": restored, "unchanged_files": unchanged,
            "scope": "source hydration; full workspace compilation and pack preparation require all products", "passed": True}


def publish():
    lock = load_lock()
    directory = ROOT / OUTPUT / lock["tag"]
    verify(directory, check_local=True)
    if lock["storage"].get("revision"):
        return verify_remote(lock)
    uploads = ROOT / OUTPUT / "uploads"
    uploads.mkdir(parents=True, exist_ok=True)
    upload = Path(tempfile.mkdtemp(prefix=lock["tag"] + "-", dir=uploads))
    url = "https://huggingface.co/datasets/" + lock["storage"]["repository"]
    git("clone", url, str(upload))
    git("lfs", "install", "--local", cwd=upload)
    git("config", "core.autocrlf", "false", cwd=upload)
    git("config", "core.longpaths", "true", cwd=upload)
    old_rows = []
    if (upload / "release-index.json").is_file():
        previous = json.loads((upload / "release-index.json").read_bytes())
        previous["storage"] = dict(lock["storage"], revision=None)
        old_rows = rows_for(validate_lock(previous))
    existing = set(git("ls-files", cwd=upload).splitlines())
    if not existing <= {r["path"] for r in old_rows} | META_FILES:
        raise ValueError("HF repository has unowned files; refusing to replace its content")
    expected = {r["path"] for r in rows_for(lock)}
    for row in old_rows:
        if row["path"] not in expected:
            source_path(row["path"], upload).unlink()
    for row in rows_for(lock):
        target = source_path(row["path"], upload)
        if target.exists():
            target.unlink()  # This is an isolated owned scratch clone, never the source checkout.
        copy_verified(row, directory, upload)
    for name in META_FILES:
        shutil.copyfile(directory / name, upload / name)
    git("add", "--force", "--all", "--", "assets", *sorted(META_FILES), cwd=upload)
    if set(git("ls-files", "-z", cwd=upload).strip("\0").split("\0")) != expected | META_FILES:
        raise ValueError("Staged HF file set differs from the verified snapshot")
    if git("status", "--porcelain", cwd=upload).strip():
        git("-c", "user.name=Razer UI asset maintenance", "-c", "user.email=assets@local.invalid",
            "commit", "-m", lock["tag"], cwd=upload)
    revision = git("rev-parse", "HEAD", cwd=upload).strip()
    print(json.dumps({"phase": "upload", "files": len(expected),
                      "bytes": sum(r["bytes"] for r in rows_for(lock))}), flush=True)
    git("push", "origin", "HEAD:refs/heads/main", cwd=upload)
    lock["storage"]["revision"] = revision
    print(json.dumps({"phase": "verify_remote", "revision": revision}), flush=True)
    report = verify_remote(lock)
    analysis.save(ROOT / LOCK, lock)
    return report


def add_commands(commands):
    p = commands.add_parser("prepare-asset-release", help="Snapshot raw resource files and SHA-256 identities")
    p.add_argument("--repository", help="Hugging Face owner/dataset; defaults to the existing lock")
    p = commands.add_parser("verify-asset-release", help="Verify every raw snapshot file against its lock")
    p.add_argument("--from-dir", help="Offline raw snapshot directory")
    p.add_argument("--check-local", action="store_true")
    p.add_argument("--remote", action="store_true", help="Verify pinned remote LFS identities and manifest")
    p = commands.add_parser("fetch-asset-release", help="Restore raw resources from pinned HF Git LFS or offline files")
    p.add_argument("--product", type=int, action="append", help="PID plus shared/dependency sources; omit for all")
    p.add_argument("--from-dir", help="Prepared raw snapshot directory; no network")
    p.add_argument("--replace-existing", action="store_true", help="Explicitly replace modified resource files")
    commands.add_parser("publish-asset-release", help="Push raw files with Git LFS, verify and pin the HF commit")
    p = commands.add_parser("pin-asset-release", help="Verify an existing HF upload and pin it without uploading or downloading payloads")
    p.add_argument("--revision", required=True)


def run(args):
    if args.command == "prepare-asset-release":
        return prepare(args.repository)
    if args.command == "verify-asset-release":
        return verify_remote() if args.remote else verify(args.from_dir, args.check_local)
    if args.command == "fetch-asset-release":
        return restore(args.product, args.from_dir, args.replace_existing)
    if args.command == "publish-asset-release":
        return publish()
    if args.command == "pin-asset-release":
        return pin_remote(args.revision)
    raise ValueError("Unknown asset snapshot command")
