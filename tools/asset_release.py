"""Raw Hugging Face resources and code-owned runtime metadata.

workspace.py owns the CLI. Cargo uses committed metadata without hydrating the
resource payload; the product fetches individual files through its asset service.
"""
from __future__ import annotations

import concurrent.futures
from contextlib import contextmanager
from datetime import datetime, timezone
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tempfile
import urllib.parse
import urllib.request

import analysis

ROOT = Path(__file__).resolve().parent.parent
LOCK = "tools/asset-release.json"
MAPPINGS = "tools/resource-mappings.json"
OUTPUT = ".work/asset-releases"
UPDATE_REPORT = ".analysis/results/asset-release-update.json"


def _checked_upload_tree(path, identity=None):
    """Constrain owned clone cleanup to a regular private uploads directory."""
    boundary = ROOT / OUTPUT / "uploads"
    path = Path(path)
    if path.parent != boundary or not re.fullmatch(r"assets-[a-f0-9]{20}-[A-Za-z0-9_-]+", path.name):
        raise ValueError("Upload cleanup target is outside the owned clone directory")
    for ancestor in (ROOT, ROOT / ".work", ROOT / OUTPUT, boundary, path):
        if (ancestor.is_symlink() or ancestor.is_junction()
                or os.name == "nt" and ancestor.lstat().st_file_attributes & 0x400):
            raise ValueError("Upload cleanup target contains a link or reparse point")
    if path.resolve().parent != boundary.resolve() or not path.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError("Upload cleanup target escaped its resolved boundary")
    current = path.stat()
    if identity is not None and (current.st_dev, current.st_ino) != identity:
        raise ValueError("Owned upload directory changed before cleanup")
    for folder, directories, files in os.walk(path, followlinks=False):
        for name in directories + files:
            child = Path(folder) / name
            if (child.is_symlink() or child.is_junction()
                    or os.name == "nt" and child.lstat().st_file_attributes & 0x400):
                raise ValueError("Owned upload clone contains a link or reparse point")
    return path


@contextmanager
def _owned_upload_clone(tag):
    """Release this invocation's clone unless the current receipt retains it."""
    if not re.fullmatch(r"assets-[a-f0-9]{20}", tag):
        raise ValueError("Upload clone requires a verified release tag")
    uploads = ROOT / OUTPUT / "uploads"
    uploads.mkdir(parents=True, exist_ok=True)
    upload = Path(tempfile.mkdtemp(prefix=tag + "-", dir=uploads))
    current = upload.stat()
    identity = (current.st_dev, current.st_ino)
    try:
        yield upload
    finally:
        checked = _checked_upload_tree(upload, identity)
        report_path = ROOT / UPDATE_REPORT
        report = json.loads(report_path.read_bytes()) if report_path.is_file() else {}
        name = upload.relative_to(ROOT).as_posix()
        released = report.get("verification_checkout_release", {}).get("checkout", {}).get("path") == name
        if report.get("verification_checkout") != name and not released:
            def remove_readonly(operation, path, error):
                child = Path(path)
                if (not isinstance(error, PermissionError) or not child.is_file()
                        or not child.resolve().is_relative_to(checked.resolve()) or child.is_symlink()
                        or child.is_junction() or os.name == "nt" and child.lstat().st_file_attributes & 0x400):
                    raise error
                child.chmod(child.stat().st_mode | stat.S_IWRITE)
                operation(path)
            shutil.rmtree(checked, onexc=remove_readonly)


NATIVE_ICONS = (
    "a64b8a64b721fb74-tray-native.rgba", "f979021796dfd342-tray-app.rgba",
    "81851a9004a803d3-tray-icon-24.rgba", "71b354f660adb5db-tray-icon.rgba",
    "f8fd980e3bec82fe-tray-icon-40.rgba", "5e5df98271550aca-tray-icon-48.rgba",
    "afb1350c23ce2416-tray-icon-64.rgba", "7747f51f284ded65-tray-gear-dark.rgba",
    "33892a0b56b91958-tray-gear-light.rgba",
)
GIT_ICONS = {"assets/synapse/shared/pending-source-audit/" + n for n in NATIVE_ICONS}
GIT_ICONS.update("assets/synapse/shared/window-controls/" + name + ".svg"
                 for name in ("minimize", "maximize", "restore", "close"))
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
SELECTOR_FIELDS = {
    "product_image_selector": ("product_id", "edition_id", "layout_id", "part", "key"),
    "dashboard_image_selector": ("product_id", "edition_id", "layout_id", "key"),
    "dashboard_plugin_image_selector": ("product_id", "folder_edition_id", "filename_edition_id", "layout_id", "wrist", "density", "key"),
    "pairing_image_selector": ("product_id", "edition_id", "view_id", "key"),
    "gamepad_calibration_svg_selector": ("product_id", "edition_id", "kind", "key"),
}


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
    return name in GIT_ICONS


def validate_resource_mappings(value, *, root=None):
    """Validate complete maintenance records without loading media or vendor code."""
    def fields(record, expected, label):
        if type(record) is not dict or set(record) != set(expected):
            raise ValueError("Invalid resource mapping fields: " + label)

    def alias(key):
        if (type(key) is not str or not key.startswith("synapse/")
                or any(ord(c) < 32 for c in key) or "\\" in key
                or any(p in ("", ".", "..") for p in key.split("/"))):
            raise ValueError("Invalid runtime asset alias: " + repr(key))

    def entries(rows):
        if type(rows) is not list:
            raise ValueError("Resource mapping entries must be ordered lists")
        for row in rows:
            fields(row, ("key", "path"), "alias")
            alias(row["key"])
            source_path(row["path"], root=root)

    fields(value, ("schema_version", "tables", "audio_demo_controls", "audio_volume_sprite", "selectors"), "root")
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        raise ValueError("Unknown resource mapping schema version")
    if type(value["tables"]) is not list or not value["tables"]:
        raise ValueError("Missing ordered resource tables")
    names = set()
    for table in value["tables"]:
        fields(table, ("name", "entries"), "table")
        name = table["name"]
        if type(name) is not str or not re.fullmatch(r"[A-Z][A-Z_0-9]+", name) or name in names:
            raise ValueError("Invalid or repeated resource table identity")
        names.add(name)
        entries(table["entries"])
    entries(value["audio_demo_controls"])
    if {row["key"] for row in value["audio_demo_controls"]} != {
            "synapse/audio-demo-control-play.svg", "synapse/audio-demo-control-pause.svg",
            "synapse/audio-demo-control-fullscreen.svg", "synapse/audio-demo-control-fullscreen-exit.svg"} or len(value["audio_demo_controls"]) != 4:
        raise ValueError("Audio demo AssetSource aliases require a source audit")
    entries([value["audio_volume_sprite"]])
    fields(value["selectors"], SELECTOR_FIELDS, "selectors")
    aliases = {row["key"] for table in value["tables"] for row in table["entries"]}
    aliases.update(row["key"] for row in value["audio_demo_controls"])
    aliases.add(value["audio_volume_sprite"]["key"])
    for table, columns in SELECTOR_FIELDS.items():
        rows = value["selectors"][table]
        if type(rows) is not list or not rows:
            raise ValueError("Empty maintained selector table: " + table)
        for row in rows:
            fields(row, columns, table)
            for column, field in row.items():
                if column == "key":
                    alias(field)
                    if field not in aliases:
                        raise ValueError("Selector references an unregistered asset: " + field)
                elif column == "part":
                    if field not in ("Product", "MouseBottom", "KeyboardWrist", "KeyboardDial", "Streamer"):
                        raise ValueError("Unknown device image part")
                elif column == "kind":
                    if field not in ("prd", "ActuationDevice"):
                        raise ValueError("Unknown gamepad SVG kind")
                elif column == "wrist":
                    if type(field) is not bool:
                        raise ValueError("Invalid selector wrist flag")
                elif type(field) is not int or not 0 <= field < 2**32 or (column == "product_id" and field == 0):
                    raise ValueError("Invalid selector identity: " + column)
                elif column == "density" and field not in (1, 3):
                    raise ValueError("Invalid selector density")
    return value


def load_resource_mappings(*, root=None, input_rows=None):
    root = (root or ROOT).resolve()
    path = root / MAPPINGS
    if path.is_symlink() or not path.resolve().is_relative_to(root):
        raise ValueError("Resource maintenance input escaped the workspace")
    raw = path.read_bytes()
    value = validate_resource_mappings(json.loads(raw), root=root)
    if input_rows is not None:
        input_rows.append({"path": MAPPINGS, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
    return value


def save_resource_mappings(value, *, root=None):
    root = (root or ROOT).resolve()
    validate_resource_mappings(value, root=root)
    analysis.save(root / MAPPINGS, value)


def resource_table(name, *, root=None, mappings=None):
    value = load_resource_mappings(root=root) if mappings is None else mappings
    found = [table["entries"] for table in value["tables"] if table["name"] == name]
    if len(found) != 1:
        raise ValueError("Resource table owner is missing or repeated: " + name)
    return found[0]


def verified_alias_reuse(original, alias, *, normalize_source_lf=False, root=None, mappings=None):
    """Verify source bytes against a pinned file without materializing media."""
    if type(original) is not bytes or type(normalize_source_lf) is not bool:
        raise ValueError("Alias reuse requires original bytes and an explicit normalization flag")
    root = (root or ROOT).resolve()
    mappings = load_resource_mappings(root=root) if mappings is None else mappings
    paths = {row["path"] for table in mappings["tables"] for row in table["entries"] if row["key"] == alias}
    if len(paths) != 1:
        raise ValueError("Existing resource alias is absent or ambiguous")
    path = paths.pop()
    lock = validate_lock(json.loads((root / LOCK).read_bytes()))
    files = [row for row in lock["git_files"] if row["path"] == path]
    files.extend(row for group in lock["groups"] for row in group["files"] if row["path"] == path)
    if len(files) != 1:
        raise ValueError("Existing alias has no unique pinned resource")
    content = original
    if normalize_source_lf:
        if b"\r" in original:
            raise ValueError("Source LF normalization requires no existing CR bytes")
        content = original.replace(b"\n", b"\r\n")
    locked = files[0]
    if len(content) != locked["bytes"] or hashlib.sha256(content).hexdigest() != locked["sha256"]:
        raise ValueError("Current original bytes differ from the existing pinned resource")
    return {"existing_alias": alias, "locked_file": locked, "resource_revision": lock["storage"]["revision"],
            "source_sha256": hashlib.sha256(original).hexdigest(), "source_bytes": len(original),
            "transformation": "Replace each 0A with 0D 0A; all other bytes unchanged" if normalize_source_lf else "identity",
            "source_lf_count": original.count(b"\n") if normalize_source_lf else None,
            "inverse_restores_original_bytes": content.replace(b"\r\n", b"\n") == original if normalize_source_lf else True}


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


def builtin_data_path(crate, symbol, *, root=None, macro_name=None):
    """Read the owning crate's data.rs instead of duplicating data paths."""
    if (not re.fullmatch(r"razer-[a-z][a-z0-9-]*", crate)
            or not re.fullmatch(r"[A-Z][A-Z0-9_]*", symbol)):
        raise ValueError("Invalid compiled-data owner or symbol")
    root = (root or ROOT).resolve()
    source = root / "crates" / crate / "src"
    registry = source / "data.rs"
    text = registry.read_text(encoding="utf-8") if registry.is_file() else ""
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
        paths = {item["source"] for item in recipes if item["crate"] == crate and item["symbol"] == symbol
                 and (macro_name is None or item["macro"] == macro_name)}
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
        if any(item.get("storage") in {"sqlite", "sqlite-profile"} for item in matches):
            import resource_database
            resource_database.prepare()
        for item in matches:
            if item.get("storage") in {"sqlite", "sqlite-profile"}:
                continue
            rust_data.build_table(item["crate"], item["symbol"], item["module"], item["root_type"],
                                 source=item["source"], macro_name=item["macro"],
                                 allow_value=item.get("allow_value", False),
                                 product_factory=item.get("product_factory", False))


def runtime_metadata(lock=None, *, check_local=False, input_rows=None):
    """Resolve ordered mappings against pinned identities; media is optional."""
    lock = load_lock() if lock is None else validate_lock(lock)
    mappings = load_resource_mappings(input_rows=input_rows)
    files = {row["path"]: dict(row) for row in [*rows_for(lock), *lock["git_files"]]}

    def row_for(name):
        if name not in files:
            raise ValueError("Runtime asset is absent from the resource lock: " + name)
        return dict(files[name])

    def entry(row):
        return {"key": row["key"], **row_for(row["path"])}

    table_entries = [entry(row) for table in mappings["tables"] for row in table["entries"]]
    demo_entries = [entry(row) for row in mappings["audio_demo_controls"]]
    sprite = entry(mappings["audio_volume_sprite"])
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
    token = env.get("HF_TOKEN")
    encoded_token = None
    if token:
        if not re.fullmatch(r"hf_[A-Za-z0-9]+", token):
            raise ValueError("HF_TOKEN is not a valid single-line access token")
        encoded_token = base64.b64encode(("hf_user:" + token).encode("ascii")).decode("ascii")
        # Process-only configuration reaches Git LFS without persisting the
        # credential in the checkout, command line, or credential manager.
        count = int(env.get("GIT_CONFIG_COUNT", "0"))
        if count < 0:
            raise ValueError("Git configuration count cannot be negative")
        for key, value in (
            ("http.https://huggingface.co/.extraHeader", ""),
            ("http.https://huggingface.co/.extraHeader", "Authorization: Basic " + encoded_token),
            ("credential.https://huggingface.co.helper", ""),
        ):
            env[f"GIT_CONFIG_KEY_{count}"] = key
            env[f"GIT_CONFIG_VALUE_{count}"] = value
            count += 1
        env["GIT_CONFIG_COUNT"] = str(count)
        for key in list(env):
            if key.startswith("GIT_TRACE") or key == "GIT_CURL_VERBOSE":
                env.pop(key)
    result = subprocess.run(["git", *args], cwd=cwd or ROOT, env=env, capture_output=True,
                            text=True, encoding="utf-8", errors="replace")
    if result.returncode:
        message = result.stderr.strip() or result.stdout.strip()
        for secret in (token, encoded_token):
            if secret:
                message = message.replace(secret, "[redacted]")
        message = re.sub(r"hf_[A-Za-z0-9]+", "[redacted]", message)
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


def verify_remote_files(lock, tree, evidence_roots=()):
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
            roots = (ROOT, ROOT / OUTPUT / lock["tag"], *evidence_roots)
            evidence = next((root for root in roots if matches(row, root)), None)
            if evidence is None:
                raise ValueError("Local SHA-256 evidence is required to verify a remote Git blob: " + name)
            blob = source_path(name, evidence).read_bytes()
            oid = hashlib.sha1(b"blob " + str(len(blob)).encode("ascii") + b"\0" + blob).hexdigest()
            if actual.get("oid") != oid:
                raise ValueError("Remote Git blob differs from SHA-256-verified source: " + name)


def verify_remote(lock=None, *, evidence_roots=()):
    lock = lock or load_lock()
    storage = lock["storage"]
    if not storage.get("revision"):
        raise ValueError("Resource snapshot has no verified immutable HF commit")
    tree = remote_tree(storage["repository"], storage["revision"])
    verify_remote_files(lock, tree, evidence_roots)
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


def release_additions(manifest_name):
    """Plan verified additions without hydrating or replacing existing assets."""
    lock_path, mapping_path = ROOT / LOCK, ROOT / MAPPINGS
    lock_raw, mapping_raw = lock_path.read_bytes(), mapping_path.read_bytes()
    original = validate_lock(json.loads(lock_raw))
    if not original["storage"].get("revision"):
        raise ValueError("Asset additions require a pinned existing release")
    manifest_path = (ROOT / manifest_name).resolve()
    input_root = (ROOT / ".analysis/inputs").resolve()
    if not manifest_path.is_relative_to(input_root) or manifest_path.is_symlink():
        raise ValueError("Asset addition manifest must belong to analysis inputs")
    manifest_raw = manifest_path.read_bytes()
    manifest = json.loads(manifest_raw)
    if set(manifest) != {"schema_version", "files"} or manifest["schema_version"] != 1 or not manifest["files"]:
        raise ValueError("Invalid asset addition manifest")
    lock = json.loads(lock_raw)
    mappings = validate_resource_mappings(json.loads(mapping_raw))
    existing = {row["path"]: row for row in rows_for(lock)}
    aliases = {row["key"]: row["path"] for table in mappings["tables"] for row in table["entries"]}
    tables = {table["name"]: table["entries"] for table in mappings["tables"]}
    groups = {group["name"]: group for group in lock["groups"]}
    additions, used, snapshots = [], set(), [(lock_path, lock_raw), (mapping_path, mapping_raw), (manifest_path, manifest_raw)]
    for item in manifest["files"]:
        if set(item) != {"path", "source", "acquisition", "alias", "table"}:
            raise ValueError("Invalid asset addition fields")
        target = canonical(item["path"])
        if in_git(target) or target == RUNTIME_METADATA or target.casefold() in used:
            raise ValueError("Asset addition repeats or replaces an installation resource")
        used.add(target.casefold())
        source, receipt_row = item["source"], item["acquisition"]
        for row in (source, receipt_row):
            if set(row) != {"path", "bytes", "sha256"}:
                raise ValueError("Asset addition lacks a complete source identity")
            path = ROOT / row["path"]
            if (not path.resolve().is_relative_to(input_root) or path.is_symlink()
                    or path.stat().st_size != row["bytes"] or digest(path) != row["sha256"]):
                raise ValueError("Asset addition source or receipt identity differs")
        receipt_path = ROOT / receipt_row["path"]
        receipt_raw = receipt_path.read_bytes()
        receipt = json.loads(receipt_raw)
        if not any(all(row.get(key) == source[key] for key in ("path", "bytes", "sha256"))
                   for row in receipt.get("files", []) + receipt.get("resources", [])):
            raise ValueError("Asset addition source is not declared by its acquisition receipt")
        snapshots.append((receipt_path, receipt_raw))
        row = {"path": target, "bytes": source["bytes"], "sha256": source["sha256"]}
        if target in existing and existing[target] != row:
            raise ValueError("Asset additions cannot replace existing resource bytes")
        if target not in existing:
            name = group_name(target)
            if name not in groups:
                raise ValueError("Asset addition requires an existing product resource group")
            groups[name]["files"].append(row)
            groups[name]["files"].sort(key=lambda entry: entry["path"])
            existing[target] = row
        if item["table"] not in tables:
            raise ValueError("Asset addition has no maintained alias table")
        if item["alias"] in aliases and aliases[item["alias"]] != target:
            raise ValueError("Asset addition cannot replace an existing alias")
        if item["alias"] not in aliases:
            tables[item["table"]].append({"key": item["alias"], "path": target})
            aliases[item["alias"]] = target
        additions.append({"asset": row, "source": source, "acquisition": receipt_row,
                          "alias": item["alias"], "table": item["table"]})
    lock["source_sha256"] = content_hash({"groups": lock["groups"], "git_files": lock["git_files"]})
    lock["tag"] = "assets-" + lock["source_sha256"][:20]
    validate_lock(lock)
    validate_resource_mappings(mappings)
    return original, lock, mappings, additions, snapshots


def publish_additions(manifest_name, *, dry_run=False, prune_maintenance_tables=False):
    """Publish only verified new files through an isolated LFS pointer clone."""
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        old, lock, mappings, additions, snapshots = release_additions(manifest_name)
        report = {"schema_version": 1, "recorded_at_utc": analysis.stamp(),
            "tool": {"path": "tools/asset_release.py", "sha256": digest(Path(__file__))},
            "repository": lock["storage"]["repository"], "base_revision": old["storage"]["revision"],
            "previous_tag": old["tag"], "tag": lock["tag"], "additions": additions,
            "target_execution": False, "payload_hydration": False,
            "status": "prepared", "remote_publication": False}
        report_path = ROOT / ".analysis/results/asset-release-update.json"
        if report_path.is_file():
            previous = json.loads(report_path.read_bytes())
            if previous.get("repository") == report["repository"]:
                for field in ("publication_attempts", "read_only_remote_preflight"):
                    if field in previous:
                        report[field] = previous[field]
                cleanup = previous.get("preceding_maintenance_cleanup")
                if previous.get("status") == "published_and_verified" and previous.get("removed_obsolete_maintenance_files"):
                    cleanup = {"base_revision": previous["base_revision"], "revision": previous["revision"],
                               "verified_removed_files": previous["removed_obsolete_maintenance_files"],
                               "verification": "Pinned Git/LFS byte identities verified before the publication",
                               "verification_checkout": previous.get("verification_checkout")}
                if cleanup:
                    report["preceding_maintenance_cleanup"] = cleanup
        analysis.save(report_path, report)
        if dry_run:
            return {"report": report_path.relative_to(ROOT).as_posix(), "files": len(additions),
                    "bytes": sum(row["asset"]["bytes"] for row in additions), "status": "prepared"}
        repository = lock["storage"]["repository"]
        with _owned_upload_clone(lock["tag"]) as upload:
            report["verification_checkout"] = upload.relative_to(ROOT).as_posix()
            git("-c", "core.autocrlf=false", "clone", "--depth", "1",
                "https://huggingface.co/datasets/" + repository, str(upload))
            revision = git("rev-parse", "HEAD", cwd=upload).strip()
            if revision != old["storage"]["revision"]:
                if (upload / "release-index.json").read_bytes() != encoded(published_manifest(lock)):
                    raise ValueError("HF main advanced beyond the pinned release; refusing to overwrite it")
                lock["storage"]["revision"] = revision
                verification = verify_remote(lock, evidence_roots=(upload,))
            else:
                remote_manifest = upload / "release-index.json"
                if remote_manifest.is_file():
                    remote_lock = json.loads(remote_manifest.read_bytes())
                    remote_lock["storage"] = dict(old["storage"])
                    validate_lock(remote_lock)
                    verify_remote(remote_lock, evidence_roots=(upload,))
                    remote_tree_rows = None
                else:
                    remote_lock = old
                    remote_tree_rows = remote_tree(repository, revision)
                    allowed = {row["path"] for row in rows_for(old) + old["git_files"]} | META_FILES
                    extra = remote_tree_rows.keys() - allowed
                    if (extra and not prune_maintenance_tables) or any(
                            not re.fullmatch(r"assets/synapse/tables/[A-Za-z0-9_-]+\.rs", name) for name in extra):
                        raise ValueError("Pinned HF snapshot has unowned files outside the explicit maintenance-table cleanup")
                    verify_remote_files(old, {name: value for name, value in remote_tree_rows.items() if name not in extra}, (upload,))
                git("lfs", "install", "--local", cwd=upload)
                git("config", "core.autocrlf", "false", cwd=upload)
                git("config", "core.longpaths", "true", cwd=upload)
                remote_rows = {row["path"]: row for row in rows_for(remote_lock)}
                expected = set(remote_rows)
                if remote_tree_rows is not None:
                    expected.update(name for name in remote_tree_rows if name.startswith("assets/synapse/tables/"))
                retained = {row["path"] for row in remote_lock["git_files"]}
                tracked = set(git("ls-files", "-z", cwd=upload).strip("\0").split("\0"))
                if not expected <= tracked or tracked - expected - retained - META_FILES:
                    raise ValueError("HF checkout has files outside the pinned release")
                target_rows = {row["path"]: row for row in rows_for(lock)}
                for name in target_rows.keys() & remote_rows.keys():
                    if target_rows[name] != remote_rows[name]:
                        raise ValueError("Targeted publication cannot replace existing remote resource bytes")
                obsolete = sorted(expected - target_rows.keys())
                if (obsolete and not prune_maintenance_tables) or any(
                        not re.fullmatch(r"assets/synapse/tables/[A-Za-z0-9_-]+\.rs", name) for name in obsolete):
                    raise ValueError("Targeted publication cannot remove unrelated remote resources")
                removal_records = []
                for name in obsolete:
                    path = source_path(name, upload)
                    if remote_tree_rows is not None:
                        entry = remote_tree_rows[name]
                        raw = path.read_bytes()
                        lfs = entry.get("lfs")
                        if lfs:
                            expected_pointer = ("version https://git-lfs.github.com/spec/v1\n"
                                                f"oid sha256:{lfs['oid']}\nsize {lfs['size']}\n").encode()
                            if raw != expected_pointer or entry["size"] != lfs["size"]:
                                raise ValueError("Obsolete maintenance-table LFS pointer differs from the pinned remote identity")
                            removal_records.append({"path": name, "bytes": lfs["size"], "sha256": lfs["oid"],
                                                    "payload_downloaded": False})
                        else:
                            oid = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
                            if len(raw) != entry["size"] or oid != entry["oid"]:
                                raise ValueError("Obsolete maintenance-table Git blob differs from the pinned remote identity")
                            removal_records.append({"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                                                    "payload_downloaded": False})
                    else:
                        removal_records.append(remote_rows[name])
                if obsolete:
                    git("rm", "--", *obsolete, cwd=upload)
                report["removed_obsolete_maintenance_files"] = removal_records
                report["previous_remote_manifest_present"] = remote_tree_rows is None
                for item in additions:
                    row, source = item["asset"], ROOT / item["source"]["path"]
                    if row["path"] in expected:
                        continue
                    target = source_path(row["path"], upload)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with source.open("rb") as source_stream, target.open("xb") as output:
                        shutil.copyfileobj(source_stream, output, 1024 * 1024)
                    if target.stat().st_size != row["bytes"] or digest(target) != row["sha256"]:
                        raise ValueError("Asset addition changed during publication preparation")
                    git("lfs", "track", "--filename", row["path"], cwd=upload)
                (upload / "release-index.json").write_bytes(encoded(published_manifest(lock)))
                if (upload / "README.md").is_file():
                    text = (upload / "README.md").read_bytes().replace(remote_lock["tag"].encode(), lock["tag"].encode())
                    (upload / "README.md").write_bytes(text)
                else:
                    (upload / "README.md").write_text(
                        "---\npretty_name: Razer UI local product resources\ntags:\n- desktop-ui\n---\n\n"
                        f"Raw resources for `{lock['tag']}`. Applications request only files used by the current interface.\n",
                        encoding="utf-8")
                git("add", "--", ".gitattributes", "release-index.json", "README.md", *(row["asset"]["path"] for row in additions), cwd=upload)
                for path, raw in snapshots:
                    if path.read_bytes() != raw:
                        raise ValueError("Asset addition maintenance inputs changed before publication")
                for row in additions:
                    source = ROOT / row["source"]["path"]
                    if source.stat().st_size != row["source"]["bytes"] or digest(source) != row["source"]["sha256"]:
                        raise ValueError("Asset addition source changed before publication")
                if git("status", "--porcelain", cwd=upload).strip():
                    git("-c", "user.name=Razer UI asset maintenance", "-c", "user.email=assets@local.invalid",
                        "commit", "-m", lock["tag"], cwd=upload)
                    git("push", "origin", "HEAD:refs/heads/main", cwd=upload)
                revision = git("rev-parse", "HEAD", cwd=upload).strip()
                lock["storage"]["revision"] = revision
                report.update(status="uploaded_pending_verification", revision=revision, remote_publication=True)
                analysis.save(report_path, report)
                verification = verify_remote(lock, evidence_roots=(upload,))
            for path, raw in snapshots:
                if path.read_bytes() != raw:
                    raise ValueError("Asset addition maintenance inputs changed during publication")
            analysis.save(ROOT / LOCK, lock)
            save_resource_mappings(mappings)
            report.update(status="published_and_verified", revision=revision, remote_publication=True, verification=verification)
            analysis.save(report_path, report)
            index_addition_sources(manifest_name, reason="Publish verified targeted resource delivery inputs after remote byte verification")
            _release_verification_checkout(verification)
            return {"report": report_path.relative_to(ROOT).as_posix(), "files": len(additions),
                    "revision": revision, "status": report["status"], "source_files_verified": verification["source_files_verified"]}


def index_addition_sources(manifest_name, *, reason):
    """Publish source identities only after the maintained addition planner verifies them."""
    original, planned, mappings, additions, snapshots = release_additions(manifest_name)
    if original["tag"] != planned["tag"]:
        raise ValueError("Resource source-index reconciliation requires additions in the pinned resource release")
    current_aliases = {row["key"]: row["path"] for table in
                       validate_resource_mappings(json.loads((ROOT / MAPPINGS).read_bytes()))["tables"] for row in table["entries"]}
    if any(current_aliases.get(item["alias"]) != item["asset"]["path"] for item in additions):
        raise ValueError("Resource source-index reconciliation requires published maintained aliases")
    indexed = {}
    for item in additions:
        source, receipt = item["source"], item["acquisition"]
        acquisition = json.loads((ROOT / receipt["path"]).read_bytes())
        declaration = next(row for row in acquisition.get("files", []) + acquisition.get("resources", [])
                           if all(row.get(key) == source[key] for key in ("path", "bytes", "sha256")))
        if source["path"].startswith(".analysis/inputs/frontend/"):
            metadata = {"kind": "official_frontend_artifact"}
        else:
            kind = {".analysis/inputs/native-resources/acquisition.json": "prepared_native_cursor",
                    ".analysis/inputs/prepared-resources/acquisition.json": "prepared_resource"}.get(receipt["path"], "retained_resource_publication_source")
            metadata = {"kind": kind, "acquisition_receipt": receipt["path"]}
        indexed[source["path"]] = {**{key: declaration[key] for key in
            ("path", "bytes", "sha256", "url", "acquired_at_utc") if key in declaration}, **metadata}
        indexed[receipt["path"]] = {**receipt, "kind": "acquisition_receipt"}
    manifest_path = ROOT / manifest_name
    indexed[manifest_name] = {"path": manifest_name, "bytes": manifest_path.stat().st_size,
                            "sha256": digest(manifest_path), "kind": "resource_publication_manifest"}
    for path, raw in snapshots:
        if path.read_bytes() != raw:
            raise ValueError("Resource publication inputs changed during source-index reconciliation")
    analysis.index_files(indexed.values(), publication={"owner": "tools/asset_release.py", "receipt": manifest_name,
        "reason": reason, "verification": {"source_receipt_membership_verified": True,
            "pinned_resource_and_alias_identities_verified": True, "tag": original["tag"],
            "revision": original["storage"]["revision"], "files": len(additions),
            "network_requests": False, "remote_publication_repeated": False}})
    return additions


def _checkout_release_state(report, lock, upload):
    """Bind a completed publication to its retained clone and indexed sources."""
    import workspace

    revision, repository = lock["storage"].get("revision"), lock["storage"]["repository"]
    completion = report.get("verification", {})
    if (report.get("schema_version") != 1 or report.get("target_execution") is not False
            or report.get("status") != "published_and_verified" or report.get("remote_publication") is not True
            or report.get("repository") != repository or report.get("revision") != revision
            or report.get("tag") != lock["tag"] or not re.fullmatch(r"[a-f0-9]{40}", revision or "")
            or completion.get("passed") is not True or completion.get("repository") != repository
            or completion.get("revision") != revision
            or completion.get("source_files_verified") != len(rows_for(lock))
            or completion.get("retained_files_present") != len(lock["git_files"])
            or completion.get("remote_manifest_present") is not True):
        raise ValueError("Verification checkout requires the complete pinned publication receipt")
    recorded = datetime.fromisoformat(report["recorded_at_utc"])
    if recorded.tzinfo is None or recorded > datetime.now(timezone.utc):
        raise ValueError("Verification checkout publication has no valid completion time")
    publication_tool = report.get("tool", {})
    if (publication_tool.get("path") != "tools/asset_release.py"
            or not re.fullmatch(r"[a-f0-9]{64}", publication_tool.get("sha256", ""))):
        raise ValueError("Verification checkout publication has no retained tool identity")
    upload = _checked_upload_tree(upload)
    name = upload.relative_to(ROOT).as_posix()
    current = upload.stat()
    manifest = upload / "release-index.json"
    if manifest.read_bytes() != encoded(published_manifest(lock)):
        raise ValueError("Verification checkout manifest differs from the pinned resource release")
    if workspace._clean_upload_git(upload, "rev-parse", "--verify", "HEAD").decode("ascii").strip() != revision:
        raise ValueError("Verification checkout HEAD differs from the pinned publication")
    index_path = ROOT / ".analysis/inputs.json"
    index_raw = index_path.read_bytes()
    indexed = {row["path"]: row for row in json.loads(index_raw)["inputs"]}
    additions = report.get("additions")
    if not isinstance(additions, list) or not additions:
        raise ValueError("Verification checkout has no published addition identities")
    expected_manifest = {"schema_version": 1, "files": [
        {"path": item["asset"]["path"], **{key: item[key] for key in ("source", "acquisition", "alias", "table")}}
        for item in additions]}
    manifests = []
    for row in indexed.values():
        if row.get("kind") == "resource_publication_manifest":
            path = analysis.resolve(row["path"], ".analysis/inputs")
            if json.loads(path.read_bytes()) == expected_manifest:
                manifests.append(row)
    if len(manifests) != 1:
        raise ValueError("Verification checkout requires its unique indexed publication manifest")
    original, planned, _, planned_additions, _ = release_additions(manifests[0]["path"])
    if original["tag"] != planned["tag"] or planned_additions != additions:
        raise ValueError("Verification checkout additions or aliases are not fully published")
    source_rows = [manifests[0], *(item[key] for item in additions for key in ("source", "acquisition"))]
    for row in source_rows:
        if any(indexed.get(row["path"], {}).get(key) != row[key] for key in ("path", "bytes", "sha256")):
            raise ValueError("Verification checkout source identity is absent from the formal index")
    analysis.verify_source_rows(source_rows)
    def identity(path):
        return {"path": path.relative_to(ROOT).as_posix(), "bytes": path.stat().st_size, "sha256": digest(path)}
    return {"checkout": {"path": name, "device": current.st_dev, "file_id": current.st_ino},
            "repository": repository, "revision": revision, "manifest": identity(manifest),
            "lock": identity(ROOT / LOCK), "mappings": identity(ROOT / MAPPINGS),
            "source_index": {"path": ".analysis/inputs.json", "bytes": len(index_raw),
                             "sha256": hashlib.sha256(index_raw).hexdigest()},
            "source_manifest": identity(ROOT / manifests[0]["path"]),
            "publication_sha256": content_hash({key: value for key, value in report.items()
                if key not in {"verification_checkout", "verification_checkout_release", "verification_checkout_release_history"}}),
            "publication_tool": publication_tool}


def _verify_checkout_release_proof(release, report, state, *, current_owner):
    tool = release.get("tool", {})
    if (release.get("schema_version") != 1 or release.get("target_execution") is not False
            or release.get("deleted") is not False
            or set(tool) != {"path", "sha256"} or tool.get("path") != "tools/asset_release.py"
            or not re.fullmatch(r"[a-f0-9]{64}", tool.get("sha256", ""))
            or current_owner and tool["sha256"] != digest(ROOT / "tools/asset_release.py")):
        raise ValueError("Verification checkout release does not match the current verification owner")
    recorded = datetime.fromisoformat(release["recorded_at_utc"])
    if recorded.tzinfo is None or recorded > datetime.now(timezone.utc):
        raise ValueError("Verification checkout release has no valid completion time")
    if set(release) != {"schema_version", "recorded_at_utc", "tool", "verification", "target_execution", "deleted", *state}:
        raise ValueError("Verification checkout release has unrecognized dependency fields")
    if any(release.get(key) != value for key, value in state.items()):
        raise ValueError("Verification checkout release evidence changed")
    if release.get("verification") != report["verification"]:
        raise ValueError("Verification checkout release lost its remote verification result")
    return release


def verify_checkout_release(report, lock, upload):
    """Recheck a current release proof and its retained history without deleting."""
    release = report.get("verification_checkout_release")
    if not isinstance(release, dict) or report.get("verification_checkout") is not None:
        raise ValueError("Verification checkout has no completed release proof")
    state = _checkout_release_state(report, lock, upload)
    history = report.get("verification_checkout_release_history", [])
    if not isinstance(history, list):
        raise ValueError("Verification checkout release history is not a retained proof list")
    for previous in history:
        if not isinstance(previous, dict):
            raise ValueError("Verification checkout release history has an invalid proof")
        _verify_checkout_release_proof(previous, report, state, current_owner=False)
    return _verify_checkout_release_proof(release, report, state, current_owner=True)


def _release_verification_checkout(verification):
    """Release a verified borrow while leaving deletion to the cleanup plan."""
    report_path = ROOT / UPDATE_REPORT
    raw = report_path.read_bytes()
    report = json.loads(raw)
    lock = load_lock()
    existing = report.get("verification_checkout_release")
    name = report.get("verification_checkout") or (existing or {}).get("checkout", {}).get("path")
    if not isinstance(name, str) or (ROOT / name).relative_to(ROOT).as_posix() != name:
        raise ValueError("Verification checkout requires its canonical retained path")
    upload = ROOT / name
    with analysis.index_lock():
        state = _checkout_release_state(report, lock, upload)
        if verification != report["verification"]:
            raise ValueError("Verification checkout remote result differs from the completed publication")
        if existing is not None:
            _verify_checkout_release_proof(existing, report, state, current_owner=False)
            history = report.get("verification_checkout_release_history", [])
            if not isinstance(history, list):
                raise ValueError("Verification checkout release history is not a retained proof list")
            for previous in history:
                _verify_checkout_release_proof(previous, report, state, current_owner=False)
            if existing["tool"]["sha256"] == digest(ROOT / "tools/asset_release.py"):
                return verify_checkout_release(report, lock, upload)
            report["verification_checkout_release_history"] = [*history, existing]
        release = {"schema_version": 1, "recorded_at_utc": analysis.stamp(),
                   "tool": {"path": "tools/asset_release.py", "sha256": digest(ROOT / "tools/asset_release.py")},
                   **state, "verification": verification, "target_execution": False, "deleted": False}
        if report_path.read_bytes() != raw:
            raise ValueError("Verification checkout completion receipt changed during release")
        report.pop("verification_checkout", None)
        report["verification_checkout_release"] = release
        verify_checkout_release(report, lock, upload)
        analysis.save(report_path, report)
    return release


def release_verification_checkout(*, evidence_roots=()):
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        lock = load_lock()
        report = json.loads((ROOT / UPDATE_REPORT).read_bytes())
        release = report.get("verification_checkout_release", {})
        name = report.get("verification_checkout") or release.get("checkout", {}).get("path")
        if not isinstance(name, str):
            raise ValueError("No retained verification checkout is available for release")
        upload = _checked_upload_tree(ROOT / name)
        verification = verify_remote(lock, evidence_roots=(*evidence_roots, upload))
        return _release_verification_checkout(verification)


def publish():
    lock = load_lock()
    directory = ROOT / OUTPUT / lock["tag"]
    verify(directory, check_local=True)
    if lock["storage"].get("revision"):
        return verify_remote(lock)
    with _owned_upload_clone(lock["tag"]) as upload:
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
    p.add_argument("--release-verification-checkout", action="store_true",
                   help="With --remote, release the completed checkout borrow without deleting its files")
    p = commands.add_parser("fetch-asset-release", help="Restore raw resources from pinned HF Git LFS or offline files")
    p.add_argument("--product", type=int, action="append", help="PID plus shared/dependency sources; omit for all")
    p.add_argument("--from-dir", help="Prepared raw snapshot directory; no network")
    p.add_argument("--replace-existing", action="store_true", help="Explicitly replace modified resource files")
    p = commands.add_parser("publish-asset-release", help="Push raw files with Git LFS, verify and pin the HF commit")
    p.add_argument("--add-files", help="Verified analysis-input manifest of additions; preserve existing remote payloads")
    p.add_argument("--dry-run", action="store_true", help="Prepare the exact addition plan without network or publication")
    p.add_argument("--prune-maintenance-tables", action="store_true", help="Remove only obsolete assets/synapse/tables/*.rs after verifying their pinned remote identities")
    p = commands.add_parser("pin-asset-release", help="Verify an existing HF upload and pin it without uploading or downloading payloads")
    p.add_argument("--revision", required=True)


def run(args):
    if args.command == "prepare-asset-release":
        return prepare(args.repository)
    if args.command == "verify-asset-release":
        roots = (Path(args.from_dir).resolve(),) if args.from_dir else ()
        if getattr(args, "release_verification_checkout", False):
            if not args.remote:
                raise ValueError("Verification checkout release requires --remote")
            return release_verification_checkout(evidence_roots=roots)
        return verify_remote(evidence_roots=roots) if args.remote else verify(args.from_dir, args.check_local)
    if args.command == "fetch-asset-release":
        return restore(args.product, args.from_dir, args.replace_existing)
    if args.command == "publish-asset-release":
        if args.add_files:
            return publish_additions(args.add_files, dry_run=args.dry_run, prune_maintenance_tables=args.prune_maintenance_tables)
        if args.dry_run or args.prune_maintenance_tables:
            raise ValueError("Targeted publication options require --add-files")
        return publish()
    if args.command == "pin-asset-release":
        return pin_remote(args.revision)
    raise ValueError("Unknown asset snapshot command")
