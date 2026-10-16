"""Maintain static analysis inputs, captures and Rust verification."""
from __future__ import annotations

import argparse
import base64
import bisect
from collections import Counter
import concurrent.futures
from contextlib import contextmanager
from datetime import datetime, timezone
from functools import lru_cache
from html.parser import HTMLParser
import hashlib
import json
import os
from pathlib import Path, PureWindowsPath
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import urllib.request
import xml.etree.ElementTree as ET
import asset_release as resource_sources

ROOT = Path(__file__).resolve().parent.parent
GENERATED_ASSET_PACK = "target/asset-pack/synapse.pack"
INDEX = ROOT / ".analysis/inputs.json"
INPUT_AUDIT = ROOT / ".analysis/results/input-audit.json"
INPUT_RECOVERY = ROOT / ".analysis/results/input-index-recovery.json"
RESOURCE_AUDIT = ROOT / ".analysis/results/resource-audit.json"
IMAGE_RESOURCE_EXTENSIONS = {".svg", ".png", ".jpg", ".jpeg", ".webp", ".avif", ".gif", ".bmp", ".ico"}
INCLUDE = re.compile(r'include(?:_str|_bytes)?!\s*\(\s*"([^"]+)"')


def safe(path):
    path = path.resolve()
    if not path.is_relative_to(ROOT):
        raise ValueError(f"Outside project: {path}")
    return path


def relative(path):
    return safe(path).relative_to(ROOT).as_posix()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for raw in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(raw)
    return h.hexdigest()


def write_json(path, data):
    # Reuse the workflow's atomic writer: private temporary names keep parallel
    # receipts separate, and Windows readers may briefly deny share-delete.
    import analysis
    analysis.save(safe(path), data)


def source_layout_destination(path):
    """Physical capability folders; existing Rust module APIs remain stable."""
    parts = path.parts
    if len(parts) < 4 or parts[0] != "crates" or parts[2] != "src":
        return path
    crate, rest = parts[1], Path(*parts[3:])
    if crate in {"razer-device", "razer-storage"} and len(rest.parts) == 2 and rest.parts[0] == "data":
        rest = Path(rest.name)
    name = rest.parts[0]
    if name in ("lib.rs", "main.rs"):
        return path
    group = None
    if crate == "razer-device":
        groups = (
            ("audio", ("audio_", "blackshark_", "seiren_", "simple_audio", "thx_")),
            ("camera", ("camera",)), ("gamepad", ("controller_",)),
            ("keyboard", ("keyboard_",)), ("mouse", ("mouse_", "dpi_matcher")),
            ("receiver", ("receiver_",)), ("mapping", ("mapping_", "macro_engine")),
            ("identity", ("device_identity", "discovery_catalog")),
            ("transport", ("backend", "device_query", "device_reads", "device_writes", "feature_exchange", "protocol")),
            ("transport", ("device_read_capabilities", "device_write_capabilities")),
            ("power", ("battery_threshold",)), ("host", ("user_app_files",)),
            ("lighting", ("lighting_driver",)),
        )
        group = next((folder for folder, prefixes in groups if name.startswith(prefixes)), None)
    elif crate == "razer-storage" and name.startswith("receiver_"):
        group = "receiver"
    elif crate == "razer-service":
        if name == "runtime" and len(rest.parts) >= 3 and rest.parts[1] == "windows":
            inner = Path(*rest.parts[2:])
            groups = (("audio", ("audio_", "simple_audio", "seiren_", "thx_")),
                      ("mouse", ("mouse_",)), ("keyboard", ("keyboard_",)),
                      ("receiver", ("receiver",)), ("hid", ("hid", "usb")))
            selected = next((folder for folder, prefixes in groups if inner.parts[0].startswith(prefixes)), None)
            if selected and len(inner.parts) == 1:
                return Path("crates") / crate / "src/runtime/windows" / selected / inner
        groups = (("audio", ("audio_", "simple_audio", "thx")),
                  ("native", ("dll", "native_query", "native_read")),
                  ("mouse", ("dpi_matcher",)))
        group = next((folder for folder, prefixes in groups if name.startswith(prefixes)), None)
    elif crate == "razer-app-pages":
        groups = (("app_picker", ("app_picker",)), ("chroma", ("chroma_",)),
                  ("firmware", ("firmware_update",)),
                  ("profiles", ("profiles_page", "profile_migration")),
                  ("pairing", ("pairing_",)),
                  ("gamer_room", ("gamer_room", "iot_popup", "alexa_page")),
                  ("services", ("service_", "module_", "devices_modules_", "feedback_")),
                  ("armory", ("armory_page",)), ("introduction", ("introduction_tour",)),
                  ("release_notes", ("release_notes",)), ("macros", ("macro_page",)))
        group = next((folder for folder, prefixes in groups if name.startswith(prefixes)), None)
    elif crate == "razer-platform":
        groups = (("camera", ("camera_",)),
                  ("input", ("global_shortcuts", "keyboard_layout", "macro_geometry", "wheel_scroll")),
                  ("observation", ("device_changes", "foreground_monitor", "session_changes")))
        group = next((folder for folder, prefixes in groups if name.startswith(prefixes)), None)
    elif crate == "razer-settings":
        groups = (("window", ("settings_window",)),
                  ("preferences", ("settings_page", "settings_lighting", "settings_systray_action", "settings_tests")))
        group = next((folder for folder, prefixes in groups if name.startswith(prefixes)), None)
    elif crate == "razer-widgets" and len(rest.parts) == 1:
        for component in ("battery", "source_alert", "source_tooltip", "scroll", "surface", "tutorial"):
            if name == component + ".rs" or name.startswith(component + "_"):
                group = component
                break
    elif crate == "razer-shell" and name == "shell" and len(rest.parts) >= 2:
        inner = Path(*rest.parts[1:])
        groups = (("audio", ("audio_", "seiren_", "thx_")),
                  ("keyboard", ("keyboard_",)), ("mouse", ("mouse_",)),
                  ("receiver", ("receiver_", "local_pairing_")),
                  ("gamepad", ("controller_",)),
                  ("devices", ("device_discovery", "product_")),
                  ("host", ("host_",)),
                  ("navigation", ("main_pages",)),
                  ("windows", ("chroma_", "pairing_window", "settings_window", "display_window", "firmware_update")))
        selected = next((folder for folder, prefixes in groups if inner.parts[0].startswith(prefixes)), None)
        if selected and inner.parts[0] != selected:
            return Path("crates") / crate / "src/shell" / selected / inner
    elif crate == "razer-pages" and name == "features" and len(rest.parts) >= 2:
        inner = Path(*rest.parts[1:])
        first = inner.parts[0]
        if first in ("keyboard", "mouse", "audio") and len(inner.parts) == 2:
            leaf = inner.parts[1]
            if first == "keyboard":
                groups = (("mapping", ("keyboard_mapping_", "keyboard_all_mapping")),
                          ("actuation", ("keyboard_actuation", "keyboard_analog", "keyboard_snap_")),
                          ("calibration", ("keyboard_calibration",)),
                          ("lighting", ("keyboard_lighting", "keyboard_brightness", "keyboard_chroma", "keyboard_rgb")),
                          ("artwork", ("keyboard_artwork", "keyboard_keypad_artwork", "keyboard_keypad_device", "keyboard_preview", "keyboard_source_artwork")),
                          ("gaming", ("keyboard_gaming",)),
                          ("controller", ("keyboard_controller", "keyboard_floating")))
            elif first == "mouse":
                groups = (("mapping", ("mouse_mapping", "mouse_remap", "mouse_sidebar", "mouse_window_click")),
                          ("dpi", ("mouse_dpi", "mouse_current_dpi", "mouse_matcher", "sensitivity")),
                          ("calibration", ("mouse_calibration", "mouse_preset_calibration", "mouse_smart_calibration")),
                          ("customize", ("mouse_customize", "mouse_button_state", "mouse_svg_button")),
                          ("lighting", ("mouse_lighting",)),
                          ("power", ("mouse_current_power", "mouse_low_power")),
                          ("polling", ("mouse_polling",)),
                          ("dynamic_sensitivity", ("mouse_dynamic",)),
                          ("scrolling", ("mouse_scroll",)))
            else:
                groups = (("mixer", ("audio_mixer", "stream_mixer")),
                          ("oled", ("audio_oled",)), ("seiren", ("audio_seiren",)),
                          ("sound", ("audio_sound",)), ("nommo", ("audio_nommo",)),
                          ("control_pod", ("control_pod",)))
            selected = next((folder for folder, prefixes in groups if leaf.startswith(prefixes)), None)
            if selected:
                return Path("crates") / crate / "src/features" / first / selected / leaf
        groups = (("help", ("source_help", "help_page", "help_tests", "product_workspace_help")),
                  ("mapping", ("mapping_", "macro_input")),
                  ("profiles", ("profile", "linked_games", "onboard_memory", "source_profile_menu")),
                  ("shortcuts", ("shortcut",)),
                  ("lighting", ("lighting_", "product_workspace_lighting")),
                  ("customize", ("customize_",)),
                  ("macros", ("macro_library",)),
                  ("workspace", ("workspace", "product_workspace", "product_surface", "device_pages", "controls", "source_controls", "source_workspace", "adjustment_prompt", "source_owner", "display_mode_roots", "display-mode-roots")))
        selected = next((folder for folder, prefixes in groups if first.startswith(prefixes)), None)
        if selected and first != selected:
            return Path("crates") / crate / "src/features" / selected / inner
    if group:
        # Existing folders with the capability name are already organized.
        if name == group:
            return path
        return Path("crates") / crate / "src" / group / rest
    return path


def source_structure_inventory():
    """Inventory every workspace owner, dependency and embedded product input."""
    import tomllib
    root_manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    shared = root_manifest.get("workspace", {}).get("dependencies", {})
    manifests = [ROOT / "Cargo.toml", *sorted((ROOT / "crates").glob("*/Cargo.toml"))]
    packages = []
    for path in manifests:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        if "package" not in manifest:
            continue
        sections = [("normal", manifest.get("dependencies", {})), ("build", manifest.get("build-dependencies", {})),
                    ("test", manifest.get("dev-dependencies", {}))]
        for target, value in manifest.get("target", {}).items():
            sections.extend((target + ":" + kind, value.get(key, {})) for kind, key in
                            (("normal", "dependencies"), ("build", "build-dependencies"), ("test", "dev-dependencies")))
        dependencies = []
        for kind, items in sections:
            for alias, specification in items.items():
                specification = shared[alias] if isinstance(specification, dict) and specification.get("workspace") else specification
                if isinstance(specification, dict) and "path" in specification:
                    dependencies.append({"name": specification.get("package", alias), "kind": kind})
        source = ROOT / "app" if path.parent == ROOT else path.parent / "src"
        files = sorted(source.rglob("*.rs"))
        packages.append({"name": manifest["package"]["name"], "manifest": relative(path),
                         "internal_dependencies": dependencies,
                         "rust_files": len(files), "rust_bytes": sum(item.stat().st_size for item in files),
                         "largest_sources": [{"path": relative(item), "bytes": item.stat().st_size}
                                             for item in sorted(files, key=lambda item: item.stat().st_size, reverse=True)[:3]]})
    embedded = [row for row in rust_inputs() if row["path"].startswith("assets/")]
    sources = [path for package in (ROOT / "crates").iterdir() if package.is_dir() for path in (package / "src").rglob("*.rs")]
    obsolete = [relative(path) for path in sources if "/src/data/" in path.as_posix() or path.name == "data.rs"]
    stale_calls = [relative(path) for path in sources if re.search(r"crate::data::\w+(?:::\w+)?!\s*\(", path.read_text(encoding="utf-8"))]
    return {"packages": packages, "packages_scanned": len(packages), "product_asset_inputs": embedded,
            "compiled_asset_inputs": [row for row in embedded if row["path"] != resource_sources.RUNTIME_METADATA],
            "installed_catalog_input": next((row for row in embedded if row["path"] == resource_sources.RUNTIME_METADATA), None),
            "obsolete_data_files": obsolete, "resource_macro_callers": stale_calls,
            "scope": "All workspace manifests, Rust source files, module/include closure and embedded input identities; not a source-to-device acceptance claim"}


def source_layout_plan():
    """Resolve complete Rust module/include paths before moving any bytes."""
    from importlib.metadata import version
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    import tree_sitter
    import tree_sitter_rust
    parser = tree_sitter.Parser(tree_sitter.Language(tree_sitter_rust.language()))
    files = sorted(path for folder in ("app", "crates") for path in (ROOT / folder).rglob("*")
                   if path.is_file() and ("src" in path.relative_to(ROOT).parts
                                          or path.suffix == ".rs"))
    mapping = {}
    for path in files:
        old = path.relative_to(ROOT)
        new = source_layout_destination(old)
        if source_layout_destination(new) != new:
            raise ValueError("Source layout is not idempotent: " + old.as_posix())
        if old != new:
            mapping[old.as_posix()] = new.as_posix()
    destinations = list(mapping.values())
    affected_crates = {Path(name).parts[1] for name in mapping}
    if len(set(destinations)) != len(destinations):
        raise ValueError("Source layout has colliding destination paths")
    for old, new in mapping.items():
        safe(ROOT / old)
        target = safe(ROOT / new)
        if target.exists() and new not in mapping:
            raise ValueError("Source layout target already exists: " + new)
    def moved(path):
        key = relative(path)
        return safe(ROOT / mapping.get(key, key))
    def relpath(target, owner):
        return os.path.relpath(target, owner).replace("\\", "/")
    rewrites, edges, unresolved = {}, [], []
    for path in files:
        if path.suffix != ".rs":
            continue
        raw = path.read_bytes()
        tree = parser.parse(raw)
        edits = []
        def visit(node, inline=()):
            if node.type == "mod_item":
                name_node = node.child_by_field_name("name")
                if name_node is None:
                    return
                name = raw[name_node.start_byte:name_node.end_byte].decode("utf-8")
                body = node.child_by_field_name("body")
                if body is not None:
                    for child in body.named_children:
                        visit(child, (*inline, name))
                    return
                attributes = []
                sibling = node.prev_named_sibling
                while sibling is not None and sibling.type in ("attribute_item", "line_comment", "block_comment"):
                    if sibling.type == "attribute_item":
                        attributes.append(sibling)
                    sibling = sibling.prev_named_sibling
                attr = next((attr for attr in attributes if re.fullmatch(
                    rb'#\[\s*path\s*=\s*"[^"\r\n]+"\s*\]', raw[attr.start_byte:attr.end_byte])), None)
                old_base = path.parent
                new_path = moved(path)
                new_base = new_path.parent
                if inline:
                    if path.name not in ("lib.rs", "main.rs", "mod.rs"):
                        old_base /= path.stem
                    if new_path.name not in ("lib.rs", "main.rs", "mod.rs"):
                        new_base /= new_path.stem
                    old_base = old_base.joinpath(*inline)
                    new_base = new_base.joinpath(*inline)
                if attr is not None:
                    match = re.search(rb'"([^"]+)"', raw[attr.start_byte:attr.end_byte])
                    literal = match[1].decode("utf-8")
                    candidates = [safe(old_base / literal)]
                else:
                    base = old_base if inline or path.name in ("lib.rs", "main.rs", "mod.rs") else old_base / path.stem
                    candidates = [safe(base / (name + ".rs")), safe(base / name / "mod.rs")]
                found = [candidate for candidate in candidates if candidate.is_file()]
                if len(found) != 1:
                    unresolved.append({"owner": relative(path), "module": name,
                                       "candidates": [relative(candidate) for candidate in candidates]})
                    return
                target = moved(found[0])
                edges.append({"owner": relative(path), "target": relative(found[0]), "kind": "module"})
                if attr is not None:
                    replacement = relpath(target, new_base).encode("utf-8")
                    if replacement != match[1]:
                        start = attr.start_byte + match.start(1)
                        edits.append((start, start + len(match[1]), replacement))
                else:
                    base = new_base if inline or new_path.name in ("lib.rs", "main.rs", "mod.rs") else new_base / new_path.stem
                    # An explicit path on a relocated parent changes Rust's
                    # default child-directory ownership. Pin every implicit
                    # module in affected crates to the resolved existing file.
                    if (path.relative_to(ROOT).parts[1] in affected_crates
                            or target not in ((base / (name + ".rs")).resolve(), (base / name / "mod.rs").resolve())):
                        indent_start = raw.rfind(b"\n", 0, node.start_byte) + 1
                        indent = re.match(rb'[ \t]*', raw[indent_start:node.start_byte])[0]
                        newline = b"\r\n" if b"\r\n" in raw else b"\n"
                        attribute = ('#[path = "' + relpath(target, new_base) + '"]').encode("utf-8")
                        edits.append((node.start_byte, node.start_byte, attribute + newline + indent))
                return
            for child in node.named_children:
                visit(child, inline)
        visit(tree.root_node)
        for match in re.finditer(rb'\binclude(?:_str|_bytes)?!\s*\(\s*"([^"\r\n]+)"', raw):
            context = tree.root_node.descendant_for_byte_range(match.start(), match.start() + 1)
            ignored = False
            while context is not None:
                if context.type in ("line_comment", "block_comment", "string_literal", "raw_string_literal"):
                    ignored = True
                    break
                context = context.parent
            if ignored:
                continue
            literal = match[1].decode("utf-8")
            target = safe(path.parent / literal)
            if not target.is_file():
                unresolved.append({"owner": relative(path), "include": literal})
                continue
            edges.append({"owner": relative(path), "target": relative(target), "kind": "include"})
            replacement = relpath(moved(target), moved(path).parent).encode("utf-8")
            if replacement != match[1]:
                edits.append((match.start(1), match.end(1), replacement))
        if edits:
            edits.sort(reverse=True)
            last = len(raw) + 1
            for start, end, replacement in edits:
                if end > last:
                    raise ValueError("Overlapping Rust path edits: " + relative(path))
                raw = raw[:start] + replacement + raw[end:]
                last = start
            rewrites[relative(path)] = raw
    # Current workflow and docs refer to implementation files by project paths.
    # Original inputs and prior analysis evidence are immutable, not rewritten.
    references = [ROOT / "README.md", ROOT / "Cargo.toml", ROOT / "tools/README.md"]
    references += list((ROOT / "tools").glob("*.py"))
    references += [p for folder in ("docs", "crates") for p in (ROOT / folder).rglob("*")
                   if p.is_file() and p.suffix in (".py", ".md", ".toml", ".rs")]
    replacements = sorted(mapping.items(), key=lambda pair: len(pair[0]), reverse=True)
    crate_replacements = {}
    for old, new in replacements:
        crate_root = "/".join(Path(old).parts[:2]) + "/"
        crate_replacements.setdefault(crate_root, []).append(
            ('"' + old.removeprefix(crate_root) + '"',
             '"' + new.removeprefix(crate_root) + '"'))
    for path in references:
        name = relative(path)
        raw = rewrites.get(name, path.read_bytes())
        text = raw.decode("utf-8")
        for old, new in replacements:
            text = text.replace(old, new)
        for crate_root, local_replacements in crate_replacements.items():
            if name.startswith(crate_root):
                for old, new in local_replacements:
                    text = text.replace(old, new)
        if text.encode("utf-8") != raw:
            rewrites[name] = text.encode("utf-8")
    tracked = sorted(set(mapping) | set(rewrites))
    inputs = [{"path": name, "bytes": (ROOT / name).stat().st_size, "sha256": digest(ROOT / name)}
              for name in tracked]
    return {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
            "operation": "physical source organization with stable Rust module namespaces",
            "analyzers": {name: version(name) for name in ("tree-sitter", "tree-sitter-rust")},
            "moves": mapping, "inputs": inputs, "rewrite_files": sorted(rewrites),
            "module_and_include_edges": len(edges), "unresolved": unresolved,
            "move_counts_by_crate": dict(sorted(Counter(Path(name).parts[1] for name in mapping).items())),
            "target_execution": False, "git_mutation": False}, rewrites


def remove_empty_source_directories(moves):
    candidates = set()
    for name in moves:
        source = safe(ROOT / name)
        source_root = safe(ROOT / Path(*Path(name).parts[:3]))
        directory = source.parent
        while directory != source_root:
            if not directory.is_relative_to(source_root):
                raise ValueError("Source directory escaped its crate: " + name)
            candidates.add(directory)
            directory = directory.parent
    removed = []
    for directory in sorted(candidates, key=lambda p: (-len(p.parts), str(p))):
        directory = safe(directory)
        if directory.is_dir() and not any(directory.iterdir()):
            directory.rmdir()
            removed.append(relative(directory))
    return removed


def organize_sources(apply=False):
    plan, rewrites = source_layout_plan()
    receipt = safe(ROOT / (".analysis/results/source-organization.json" if apply
                           else ".analysis/results/source-organization-plan.json"))
    if apply:
        if plan["unresolved"]:
            raise ValueError("Unresolved Rust module/include paths; inspect the dry run before applying")
        for row in plan["inputs"]:
            path = safe(ROOT / row["path"])
            if path.stat().st_size != row["bytes"] or digest(path) != row["sha256"]:
                raise ValueError("Source changed during layout preparation: " + row["path"])
        for old, new in plan["moves"].items():
            if safe(ROOT / new).exists():
                raise ValueError("Source destination appeared during layout preparation: " + new)
        # Every absolute origin/destination was checked inside ROOT above.
        # Move individual files; never recursively delete or replace folders.
        for old, new in plan["moves"].items():
            source, target = safe(ROOT / old), safe(ROOT / new)
            target.parent.mkdir(parents=True, exist_ok=True)
            source.rename(target)
        for old, raw in rewrites.items():
            target = safe(ROOT / plan["moves"].get(old, old))
            target.write_bytes(raw)
        plan["removed_empty_directories"] = remove_empty_source_directories(plan["moves"])
        plan["applied"] = True
        plan["outputs"] = [{"path": plan["moves"].get(row["path"], row["path"]),
                            "sha256": digest(ROOT / plan["moves"].get(row["path"], row["path"]))}
                           for row in plan["inputs"]]
        plan["validation"] = "Pending locked workspace check and affected pure/simulated tests"
    else:
        plan["applied"] = False
    if (ROOT / "Cargo.toml").is_file():
        plan["workspace_inventory"] = source_structure_inventory()
    write_json(receipt, plan)
    print(json.dumps({"receipt": relative(receipt), "moves": len(plan["moves"]),
                      "rewrite_files": len(rewrites), "unresolved": plan["unresolved"],
                      "applied": plan["applied"]}), flush=True)


def audit_git_lfs():
    """Read index pointers, attributes and local bytes without staging or fsck writes."""
    def git(*args, data=None):
        return subprocess.run(["git", *args], input=data, cwd=ROOT,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True).stdout
    def attributes(names):
        output = git("check-attr", "-z", "--stdin", "filter",
                     data=b"\0".join(name.encode("utf-8") for name in names) + b"\0")
        fields = output.rstrip(b"\0").split(b"\0")
        if len(fields) != len(names) * 3:
            raise ValueError("Incomplete Git attribute output")
        return {fields[i].decode("utf-8"): fields[i + 2].decode("utf-8")
                for i in range(0, len(fields), 3)}
    pointer = re.compile(rb'version https://git-lfs.github.com/spec/v1\noid sha256:([0-9a-f]{64})\nsize ([0-9]+)\n')
    index_before = git("ls-files", "--stage", "-z")
    index, failures = {}, []
    for entry in index_before.rstrip(b"\0").split(b"\0"):
        metadata, name = entry.split(b"\t", 1)
        mode, oid, stage = metadata.split()
        name = name.decode("utf-8")
        if stage != b"0":
            failures.append({"path": name, "error": "unmerged_index_entry"})
        else:
            index[name] = oid.decode("ascii")
    current = sorted(set(git("ls-files", "--cached", "--others", "--exclude-standard", "-z")
                         .decode("utf-8").rstrip("\0").split("\0")))
    names = sorted(set(current) | set(index))
    filters = attributes(names)
    lfs_index = {name: oid for name, oid in index.items() if filters[name] == "lfs"}
    pointers, objects = {}, {}
    raw = git("cat-file", "--batch", data=("\n".join(lfs_index.values()) + "\n").encode("ascii"))
    position = 0
    for name, oid in lfs_index.items():
        end = raw.index(b"\n", position)
        header = raw[position:end].split()
        if len(header) != 3 or header[1] != b"blob":
            raise ValueError("Missing LFS index blob: " + name)
        length = int(header[2])
        body = raw[end + 1:end + 1 + length]
        position = end + 2 + length
        match = pointer.fullmatch(body)
        if match is None:
            failures.append({"path": name, "error": "lfs_index_blob_is_not_a_canonical_pointer"})
            continue
        sha, size = match[1].decode("ascii"), int(match[2])
        if sha in objects and objects[sha] != size:
            failures.append({"path": name, "error": "conflicting_lfs_object_sizes"})
        pointers[name], objects[sha] = sha, size
    environment = git("lfs", "env").decode("utf-8")
    media = re.search(r"(?m)^LocalMediaDir=(.+)$", environment)
    if media is None:
        raise ValueError("Git LFS did not report its local object directory")
    storage = Path(media[1].strip())
    def verify_object(item):
        sha, size = item
        path = storage / sha[:2] / sha[2:4] / sha
        if not path.is_file():
            return {"oid": sha, "error": "missing_local_lfs_object"}
        if path.stat().st_size != size or digest(path) != sha:
            return {"oid": sha, "error": "local_lfs_object_identity_mismatch"}
        return None
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        failures.extend(result for result in executor.map(verify_object, objects.items()) if result)
    working, pending, ordinary_large, deleted = [], [], [], 0
    for name in current:
        path = safe(ROOT / name)
        if not path.is_file():
            deleted += 1
            continue
        size = path.stat().st_size
        if filters[name] != "lfs":
            if size > 100 * 1024 * 1024:
                ordinary_large.append({"path": name, "bytes": size})
            continue
        if size < 1024 and pointer.fullmatch(path.read_bytes()):
            failures.append({"path": name, "error": "unhydrated_worktree_pointer"})
            continue
        sha = digest(path)
        working.append({"path": name, "bytes": size, "sha256": sha})
        if pointers.get(name) != sha:
            pending.append(name)
    failures.extend({**row, "error": "ordinary_file_exceeds_100_MiB"} for row in ordinary_large)
    unchanged = index_before == git("ls-files", "--stage", "-z") and filters == attributes(names)
    if not unchanged:
        failures.append({"error": "index_or_attributes_changed_during_audit"})
    report = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "head": git("rev-parse", "HEAD").decode("ascii").strip(),
              "git_lfs_version": git("lfs", "version").decode("utf-8").strip(),
              "index_sha256": hashlib.sha256(index_before).hexdigest(),
              "summary": {"index_pointers": len(pointers), "unique_local_objects": len(objects),
                          "working_lfs_files": len(working), "pending_add_or_update": len(pending),
                          "deleted_index_paths": deleted, "ordinary_large_files": len(ordinary_large),
                          "failures": len(failures)},
              "working_files": working, "pending_add_or_update": pending, "failures": failures,
              "index_and_attributes_unchanged": unchanged, "git_mutation": False,
              "remote_upload_verified": False, "passed": not failures}
    write_json(ROOT / ".analysis/results/git-lfs-audit.json", report)
    return report


def resource_configuration_plan():
    mapping, records = {}, []
    for folder, filename in (("keyboard-products", "keyboard.json"), ("mouse-products/configs", "mouse.json")):
        for path in sorted((ROOT / "assets/synapse" / folder).glob("*.json")):
            path = safe(path)
            raw = path.read_bytes()
            data = json.loads(raw)
            product = data.get("product_id")
            if type(product) is not int or product <= 0 or path.stem != str(product):
                raise ValueError("Configuration product identity disagrees with its declared owner: " + relative(path))
            target = f"assets/synapse/products/{product}/configs/{filename}"
            if (ROOT / target).exists():
                raise ValueError("Configuration migration destination already exists: " + target)
            declared = data.get("config", {}).get("DeviceInfo", {}).get("productId")
            mapping[relative(path)] = target
            records.append({"source": relative(path), "target": target, "product_id": product,
                            "device_info_product_id": declared, "sha256": hashlib.sha256(raw).hexdigest(),
                            "bytes": len(raw), "ownership_basis": "existing_configuration_product_id",
                            "identity_status": "consistent" if declared == product else "declared_device_identity_mismatch",
                            "current_source_verified": False})
    names = {"customize", "customize-layouts", "deviceconfig", "keys", "layouts"}
    names.update(f"customize-{layout}" for layout in (*range(2, 13), 15, 16, 17, 18))
    remaining = [safe(ROOT / f"assets/synapse/keyboard-653-{name}.json") for name in sorted(names)]
    remaining = [path for path in remaining if path.is_file()]
    bundle = ROOT / "assets/synapse/keyboard-653-deviceconfig.json"
    if not bundle.is_file():
        bundle = ROOT / "assets/synapse/products/653/configs/deviceconfig.json"
    if remaining:
        if not bundle.is_file():
            raise ValueError("Existing keyboard configuration bundle has no product identity anchor")
        if json.loads(bundle.read_bytes()).get("PID") != 653:
            raise ValueError("Existing keyboard configuration bundle no longer declares PID 653")
        for path in remaining:
            raw = path.read_bytes()
            target = "assets/synapse/products/653/configs/" + path.name.removeprefix("keyboard-653-")
            if (ROOT / target).exists():
                raise ValueError("Configuration migration destination already exists: " + target)
            mapping[relative(path)] = target
            records.append({"source": relative(path), "target": target, "product_id": 653,
                            "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw),
                            "ownership_basis": "existing_keyboard_653_bundle_with_deviceconfig_PID_653",
                            "current_source_verified": False})
    return mapping, records


def asset_support_archive_plan():
    import ast
    mapping, originals, writes, references = {}, {}, {}, []
    base = ROOT / "assets/synapse"
    obsolete = list(base.glob("*.rs")) + list((base / "mouse-products").glob("*.rs"))
    if obsolete:
        raise ValueError("Rust resource fragments require audited conversion into " + resource_sources.MAPPINGS)
    candidates = []
    indexes = {"asset-list.json", "resource-layout.json", "dashboard-image-map.json", "product-image-map.json", "keyboard-svg-ids.json"}
    candidates.extend((path, ("indexes/" if path.name in indexes else "manifests/") + path.name) for path in sorted(base.glob("*.json"))
                      if not path.name.startswith("keyboard-653-"))
    for product in (70, 162):
        source = base / f"mouse-calibration-{product}/manifest.json"
        if source.is_file():
            candidates.append((source, f"products/{product}/manifests/mouse-calibration.json"))
    lighting_manifest = base / "mouse-lighting/manifest.json"
    if lighting_manifest.is_file():
        candidates.append((lighting_manifest, "manifests/mouse-lighting.json"))
    for source, destination in candidates:
        source_name = relative(source)
        target = base / destination
        if source.resolve() != source or target.resolve() != target:
            raise ValueError("Archive paths must not redirect through links")
        if target.exists() or target.is_symlink():
            raise ValueError("Asset support archive destination already exists: " + relative(target))
        mapping[source_name] = relative(target)
        originals[source_name] = source.read_bytes()
    if len(set(mapping.values())) != len(mapping):
        raise ValueError("Asset support archive destinations are not unique")

    snapshots = {}

    def snapshot(path, raw):
        name = relative(path)
        snapshots[name] = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    for name, raw in originals.items():
        snapshot(ROOT / name, raw)
    files = set()
    for folder in ("app", "crates", "assets", "docs"):
        for path in (ROOT / folder).rglob("*"):
            if path.is_file() and path.suffix in {".rs", ".json", ".md"}:
                files.add(path)
    files.update(ROOT.glob("*.md"))
    physical = re.compile("(?:" + "|".join(re.escape(name) for name in sorted(mapping, key=len, reverse=True))
                          + r")(?![A-Za-z0-9_.\-/])") if mapping else None
    for path in sorted(files):
        name = relative(path)
        if path.resolve() != path:
            raise ValueError("Archive reference owner must not redirect: " + name)
        raw = path.read_bytes()
        snapshot(path, raw)
        text = raw.decode("utf-8")
        rewritten = text
        if path.suffix == ".rs":
            edits = []
            owner = ROOT / mapping.get(name, name)
            for kind, literal, begin, end in rust_embed_references(text, spans=True):
                if kind == "locale":
                    continue
                target = safe(path.parent / literal)
                target_name = relative(target)
                if name not in mapping and target_name not in mapping:
                    continue
                if not target.is_file():
                    raise ValueError("Archive include target is missing: " + target_name)
                replacement = os.path.relpath(ROOT / mapping.get(target_name, target_name), owner.parent).replace("\\", "/")
                if replacement != literal:
                    edits.append((begin, end, json.dumps(replacement)))
                    references.append({"owner": name, "kind": kind, "before": literal, "after": replacement})
            for begin, end, replacement in reversed(edits):
                rewritten = rewritten[:begin] + replacement + rewritten[end:]
            if physical:
                rewritten = physical.sub(lambda match: mapping[match[0]], rewritten)
        elif path.suffix == ".json" and b"assets/synapse/" in raw and mapping:
            data = json.loads(text)
            changed = False

            def visit(value, historical=False):
                nonlocal changed
                if isinstance(value, dict):
                    return {key: visit(child, historical or key == "original_paths") for key, child in value.items()}
                if isinstance(value, list):
                    return [visit(child, historical) for child in value]
                if not historical and isinstance(value, str) and value in mapping:
                    changed = True
                    references.append({"owner": name, "kind": "physical_json_path", "before": value, "after": mapping[value]})
                    return mapping[value]
                return value

            data = visit(data)
            if changed:
                rewritten = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
                if b"\r\n" in raw:
                    rewritten = rewritten.replace("\n", "\r\n")
        elif path.suffix == ".md" and physical:
            rewritten = physical.sub(lambda match: mapping[match[0]], text)
        updated = rewritten.encode("utf-8")
        if updated != raw:
            writes[name] = updated

    pending = []
    for name in ("tools/workspace.py", "tools/families.py", "tools/host.py"):
        path = ROOT / name
        if not path.is_file():
            continue
        raw = path.read_bytes()
        snapshot(path, raw)
        tree = ast.parse(raw.decode("utf-8-sig"))
        for statement in tree.body:
            if isinstance(statement, (ast.FunctionDef, ast.AsyncFunctionDef)) and statement.name in {
                    "asset_support_archive_plan", "apply_asset_support_archive", "resource_configuration_plan"}:
                continue
            for node in ast.walk(statement):
                if isinstance(node, ast.Constant) and isinstance(node.value, str):
                    for source in mapping:
                        if source in node.value:
                            pending.append({"owner": name, "line": node.lineno, "path": source})
    records = []
    for source, target in mapping.items():
        before, after = originals[source], writes.get(source, originals[source])
        records.append({"source": source, "target": target, "bytes": len(before),
                        "sha256": hashlib.sha256(before).hexdigest(), "output_bytes": len(after),
                        "output_sha256": hashlib.sha256(after).hexdigest()})
    return {"mapping": mapping, "records": records, "writes": writes,
            "snapshots": list(snapshots.values()), "references": references,
            "maintenance_paths_pending": pending,
            "summary": {"tables": sum(target.endswith(".rs") for target in mapping.values()),
                        "manifests": sum(target.endswith(".json") for target in mapping.values()),
                        "rewritten_files": len(writes), "include_references": sum(row["kind"].startswith("include") for row in references),
                        "maintenance_paths_pending": len(pending)}}


def apply_asset_support_archive(plan):
    if plan["maintenance_paths_pending"]:
        raise ValueError("Update maintained archive producer paths before applying this plan")
    mapping, writes = plan["mapping"], plan["writes"]
    if len(set(mapping.values())) != len(mapping):
        raise ValueError("Asset support archive destinations are not unique")
    snapshots = {row["path"]: row for row in plan["snapshots"]}

    def checked(name):
        path = safe(ROOT / name)
        if relative(path) != name or name.startswith(".analysis/"):
            raise ValueError("Invalid asset support archive path: " + name)
        return path

    for row in snapshots.values():
        path = checked(row["path"])
        if path.stat().st_size != row["bytes"] or digest(path) != row["sha256"]:
            raise ValueError("Asset support archive input changed: " + row["path"])
    for source, target in mapping.items():
        if source not in snapshots or not target.startswith(("assets/synapse/manifests/",
                                                            "assets/synapse/indexes/", "assets/synapse/products/")):
            raise ValueError("Invalid asset support archive mapping")
        path = checked(target)
        if path.exists() or path.is_symlink():
            raise ValueError("Asset support archive destination already exists: " + target)
    for source in writes:
        if source not in snapshots:
            raise ValueError("Asset support archive write has no input snapshot: " + source)
    records = {row["source"]: row for row in plan["records"]}
    outputs = {}
    for source, target in mapping.items():
        raw = writes.get(source, checked(source).read_bytes())
        row = records[source]
        if row["target"] != target or len(raw) != row["output_bytes"] or hashlib.sha256(raw).hexdigest() != row["output_sha256"]:
            raise ValueError("Asset support archive output identity differs: " + target)
        outputs[target] = raw
    for target, raw in outputs.items():
        path = checked(target)
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    for source, raw in writes.items():
        if source in mapping:
            continue
        path = checked(source)
        if digest(path) != snapshots[source]["sha256"]:
            raise ValueError("Asset support archive reference changed: " + source)
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=path.name + ".", suffix=".tmp", delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        try:
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    for target, raw in outputs.items():
        if checked(target).read_bytes() != raw:
            raise ValueError("Asset support archive output changed: " + target)
    for source, target in mapping.items():
        path = checked(source)
        if digest(path) != snapshots[source]["sha256"]:
            raise ValueError("Asset support archive source changed before removal: " + source)
        path.unlink()
    base = checked("assets/synapse")
    for path in sorted(base.rglob("*"), key=lambda item: len(item.parts), reverse=True):
        if path.is_dir() and not any(path.iterdir()):
            checked(relative(path)).rmdir()
    return {"applied": True, "records": plan["records"], "summary": plan["summary"],
            "original_inputs_modified": False, "target_execution": False, "real_device_operations": False}


def organize_resources(*, apply=False):
    import analysis
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        plan = asset_support_archive_plan()
        if plan["maintenance_paths_pending"]:
            raise ValueError("Unmigrated maintenance paths: " + json.dumps(plan["maintenance_paths_pending"]))
        report = {key: value for key, value in plan.items() if key != "writes"}
        if apply:
            report.update(apply_asset_support_archive(plan))
            rust_inputs(include_asset_pack=False)
            report["catalog_reconciliation"] = reconcile_resource_layout()
        destination = ROOT / ".analysis/results/resource-deduplication.json"
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {}
        previous["support_archive"] = report
        write_json(destination, previous)
        return report


def reconcile_resource_layout():
    """Remove obsolete inventory rows only after resolving every live alias."""
    layout_path = ROOT / "assets/synapse/indexes/resource-layout.json"
    layout = json.loads(layout_path.read_bytes())
    active = {row["physical_path"]: row for row in layout["resources"]
              if safe(ROOT / row["physical_path"]).is_file()}
    missing = [row for row in layout["resources"] if row["physical_path"] not in active]
    if not missing:
        return {"obsolete_records": [], "rebound_aliases": [], "physical_files": len(active)}
    manifest = json.loads((ROOT / "assets/synapse/manifests/manifest.json").read_bytes())
    aliases = {alias: row for row in manifest["entries"] for alias in row.get("display_paths", [])}
    missing_aliases = {alias for row in missing for alias in row.get("runtime_keys", [])}
    referenced = set()
    pattern = re.compile("|".join(re.escape(alias) for alias in sorted(missing_aliases, key=len, reverse=True)))
    mapping_path = ROOT / resource_sources.MAPPINGS
    if mapping_path.is_file():
        referenced.update(pattern.findall(mapping_path.read_text(encoding="utf-8")))
    for folder in ("app", "crates"):
        for path in sorted((ROOT / folder).rglob("*")):
            if path.is_file() and path.suffix in {".rs", ".json", ".toml"}:
                referenced.update(pattern.findall(path.read_text(encoding="utf-8-sig")))
    rebound, obsolete = [], []
    for row in missing:
        live = set(row.get("runtime_keys", [])) & referenced
        if not live:
            obsolete.append(row)
            continue
        for alias in sorted(live):
            prepared = aliases.get(alias)
            if prepared is None or prepared["output"] not in active:
                raise ValueError("Current resource alias has no prepared output: " + alias)
            target = active[prepared["output"]]
            path = safe(ROOT / target["physical_path"])
            if digest(path) != prepared["sha256"] or digest(path) != target["sha256"]:
                raise ValueError("Prepared replacement identity mismatch: " + alias)
            target["runtime_keys"] = list(dict.fromkeys([*target.get("runtime_keys", []), alias]))
            target["original_paths"] = list(dict.fromkeys([*target.get("original_paths", []),
                                                           *row.get("original_paths", [])]))
            rebound.append({"alias": alias, "previous": row, "physical_path": target["physical_path"],
                            "sha256": target["sha256"]})
    usage = {}
    for row in active.values():
        for product in row.get("source_products", []):
            usage.setdefault(str(product), []).append({"physical_path": row["physical_path"],
                "runtime_keys": row.get("runtime_keys", []),
                "current_source_evidence": product in row.get("current_source_products", [])})
    layout["resources"] = sorted(active.values(), key=lambda row: row["physical_path"])
    layout["product_usage"] = usage
    layout["summary"]["physical_files"] = len(active)
    layout["summary"]["requires_current_source_audit"] = sum(
        row["ownership_status"] != "current_source_evidence" for row in active.values())
    write_json(layout_path, layout)
    for product, rows in usage.items():
        path = ROOT / f"assets/synapse/products/{product}/resources.json"
        data = json.loads(path.read_bytes())
        if data["resources"] != rows:
            data["resources"] = rows
            write_json(path, data)
    return {"obsolete_records": obsolete, "rebound_aliases": rebound, "physical_files": len(active),
            "source_search_roots": ["app", "crates", resource_sources.MAPPINGS],
            "physical_files_deleted": 0, "original_inputs_modified": False}


def indexed_path(name, group):
    if (not isinstance(name, str) or not name or PureWindowsPath(name).anchor
            or "\\" in name or any(part in {"", ".", ".."} for part in name.split("/"))
            or any(character in name for character in ':*?"<>|')):
        raise ValueError("Invalid project-relative evidence path")
    parts = Path(name).parts
    if group == "inputs":
        allowed = parts[:2] in {(".analysis", "inputs"), (".analysis", "references")} and len(parts) > 2
    else:
        allowed = group == "fixtures" and len(parts) > 3 and parts[0] == "crates" and parts[2] == "testdata"
    if not allowed:
        raise ValueError("Evidence path is outside its retained-input boundary")
    path = safe(ROOT / name)
    if path.relative_to(ROOT).as_posix() != name:
        raise ValueError("Evidence path redirects from its declared location")
    return path


def recover_inputs():
    """Recover missing index declarations from an audit, then recheck actual bytes."""
    import analysis
    with analysis.index_lock():
        if any(safe(path) != path for path in (INDEX, INPUT_AUDIT, INPUT_RECOVERY)):
            raise ValueError("Recovery paths redirect from their declared locations")
        if INDEX.exists() or INDEX.is_symlink():
            raise ValueError("Input index already exists; recovery never replaces it")
        raw = INPUT_AUDIT.read_bytes()
        previous = json.loads(raw)
        if (previous.get("schema_version") != 1 or previous.get("index_errors")
                or previous.get("index_unchanged") is not True
                or previous.get("index", {}).get("path") != relative(INDEX)
                or not isinstance(previous.get("entries"), list) or not previous["entries"]):
            raise ValueError("Recovery requires a complete, unchanged-index audit")
        recovered = {"schema_version": 1, "inputs": [], "fixtures": [],
                     "missing_current_roots": [], "current_sources": {}}
        seen = set()
        for entry in previous["entries"]:
            group, name = entry.get("group"), entry.get("path")
            indexed_path(name, group)
            expected = entry.get("expected", {})
            if (type(expected.get("bytes")) is not int or expected["bytes"] < 0
                    or not isinstance(expected.get("sha256"), str)
                    or not re.fullmatch(r"[0-9a-fA-F]{64}", expected["sha256"])):
                raise ValueError("Audit entry has no valid expected byte identity")
            if name in seen or entry.get("ordinal") != len(recovered[group]):
                raise ValueError("Audit entries are duplicated, missing or reordered")
            seen.add(name)
            recovered[group].append({"path": name, "bytes": expected["bytes"], "sha256": expected["sha256"]})
        summary = previous.get("summary", {})
        if (summary.get("total") != len(seen)
                or any(summary.get("groups", {}).get(group, {}).get("total") != len(recovered[group])
                       for group in ("inputs", "fixtures"))):
            raise ValueError("Audit entry count does not match its declared inventory")
        provenance = {"recorded_at_utc": datetime.now(timezone.utc).isoformat(),
                      "method": "restore_expected_identities_from_prior_audit",
                      "source_audit": {"path": relative(INPUT_AUDIT), "bytes": len(raw),
                                       "sha256": hashlib.sha256(raw).hexdigest(),
                                       "recorded_at_utc": previous.get("recorded_at_utc"),
                                       "index": previous["index"], "summary": summary},
                      "limitations": ["Original index byte identity is not reconstructed",
                                      "Per-file acquisition metadata is not present in the audit; consult original acquisition receipts",
                                      "Historical byte expectations and source metadata do not establish current release or dependency closure"],
                      "target_execution": False, "real_device_operations": False}
        ledger = safe(ROOT / ".analysis/results/gaps.json")
        if ledger.is_file():
            ledger_raw = ledger.read_bytes()
            owners = json.loads(ledger_raw).get("owners", {}).get(relative(INDEX), [])
            for entry in owners:
                if entry.get("kind") == "current_source_missing":
                    recovered["missing_current_roots"].append(entry["source_root"])
                elif entry.get("kind") == "current_source_set_closure_pending":
                    recovered["current_sources"][entry["source"]] = {
                        key: value for key, value in entry.items() if key not in {"kind", "source"}}
            provenance["historical_source_metadata"] = {
                "path": relative(ledger), "bytes": len(ledger_raw),
                "sha256": hashlib.sha256(ledger_raw).hexdigest(), "owner": relative(INDEX),
                "status": "partial_prior_gap_declarations_only"}
        else:
            provenance["limitations"].append("Current-source metadata was unavailable and must be reacquired")
        recovered["recovery"] = provenance
        report = {"schema_version": 1, **provenance,
                  "restored": {group: len(recovered[group]) for group in ("inputs", "fixtures")},
                  "audit_status": "pending", "passed": False}
        write_json(INPUT_RECOVERY, report)
        if INPUT_AUDIT.read_bytes() != raw:
            raise ValueError("Source audit changed during index recovery")
        write_json(INDEX, recovered)
        report["index"] = {"path": relative(INDEX), "bytes": INDEX.stat().st_size, "sha256": digest(INDEX)}
        audit = audit_inputs()
        report.update(audit_status="completed", passed=audit["passed"], audit_summary=audit["summary"],
                      audit={"path": relative(INPUT_AUDIT), "sha256": digest(INPUT_AUDIT)})
        write_json(INPUT_RECOVERY, report)
        return report


def prepare_static_python():
    """Restore pinned parsing libraries for this interpreter's ABI, locally."""
    import analysis
    import io
    import sysconfig
    import zipfile

    destination = safe(ROOT / "tools/analyzers/python-static")
    receipt_path = destination / "acquisition.json"
    previous = json.loads(receipt_path.read_bytes())
    platform = sysconfig.get_platform().replace("-", "_").replace(".", "_")
    abi = f"cp{sys.version_info.major}{sys.version_info.minor}"
    wheel_root = safe(ROOT / ".analysis/references/python-static-wheels")
    wheel_root.mkdir(parents=True, exist_ok=True)
    wheels, files, indexed = [], [], []
    pins = {row["name"]: row for row in previous["wheels"]}
    pins["tree-sitter-rust"] = {"name": "tree-sitter-rust", "version": "0.24.2"}
    for pinned in pins.values():
        name, version = pinned["name"], pinned["version"]
        metadata_url = f"https://pypi.org/pypi/{name}/{version}/json"
        with urllib.request.urlopen(metadata_url, timeout=40) as response:
            metadata_raw = response.read()
        metadata = json.loads(metadata_raw)
        if metadata["info"]["version"] != version:
            raise ValueError(f"Parser metadata version changed: {name}")
        candidates = []
        for row in metadata["urls"]:
            parts = row["filename"].removesuffix(".whl").split("-")
            if row["packagetype"] != "bdist_wheel" or len(parts) < 5 or parts[-1] != platform:
                continue
            tag, wheel_abi = parts[-3:-1]
            exact = tag == abi and wheel_abi == abi
            stable = wheel_abi == "abi3" and re.fullmatch(r"cp3\d+", tag) and int(tag[2:]) <= int(abi[2:])
            if exact or stable:
                candidates.append((int(exact), int(tag[2:]), row))
        if not candidates:
            raise ValueError(f"Pinned parser {name} {version} has no wheel for {abi}-{platform}")
        selected = max(candidates, key=lambda item: item[:2])[2]
        metadata_path = wheel_root / (name + "-" + version + "." + hashlib.sha256(metadata_raw).hexdigest() + ".pypi.json")
        metadata_path.write_bytes(metadata_raw)
        wheel_path = wheel_root / selected["filename"]
        if not wheel_path.is_file():
            with urllib.request.urlopen(selected["url"], timeout=40) as response:
                raw = response.read()
        else:
            raw = wheel_path.read_bytes()
        if len(raw) != selected["size"] or hashlib.sha256(raw).hexdigest() != selected["digests"]["sha256"]:
            raise ValueError(f"Parser wheel differs from publisher identity: {name}")
        wheel_path.write_bytes(raw)
        wheel = {"name": name, "version": version, "path": relative(wheel_path),
                 "url": selected["url"], "bytes": len(raw), "sha256": digest(wheel_path),
                 "metadata_path": relative(metadata_path), "metadata_url": metadata_url,
                 "metadata_sha256": digest(metadata_path)}
        wheels.append(wheel)
        indexed.extend([{**wheel, "kind": "static_analyzer_wheel"},
                        {"path": relative(metadata_path), "bytes": len(metadata_raw),
                         "sha256": digest(metadata_path), "url": metadata_url, "kind": "static_analyzer_metadata"}])
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            for member in archive.infolist():
                if member.is_dir():
                    continue
                target = safe(destination / member.filename)
                if not target.is_relative_to(destination) or ".." in Path(member.filename).parts:
                    raise ValueError("Parser wheel member escaped its installation directory")
                content = archive.read(member)
                target.parent.mkdir(parents=True, exist_ok=True)
                if not target.is_file() or target.read_bytes() != content:
                    target.write_bytes(content)
                files.append({"path": relative(target), "bytes": len(content), "sha256": digest(target),
                              "wheel": relative(wheel_path), "member": member.filename})
    receipt = {"schema_version": 1, "acquired_at_utc": analysis.stamp(), "python_abi": abi + "-" + platform,
               "target_execution": False, "scope": "Static JavaScript and Rust parsing analyzers; project-local installation",
               "wheels": wheels, "files": files}
    write_json(receipt_path, receipt)
    analysis.index_files(indexed)
    sys.path.insert(0, str(destination))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    tree = Parser(Language(tree_sitter_javascript.language())).parse(b"export const value = {enabled: true};")
    if tree.root_node.has_error:
        raise ValueError("Restored static parser failed its syntax smoke check")
    import tree_sitter_rust
    tree = Parser(Language(tree_sitter_rust.language())).parse(b"pub struct SourceEntry { name: &'static str }")
    if tree.root_node.has_error:
        raise ValueError("Restored Rust parser failed its syntax smoke check")
    return {"receipt": relative(receipt_path), "sha256": digest(receipt_path),
            "python_abi": receipt["python_abi"], "files_verified": len(files), "static_parse_passed": True}


def prepare_analysis():
    """Bootstrap a fresh checkout without inventing retained evidence."""
    import analysis
    import io
    import zipfile

    with analysis.index_lock():
        if not INDEX.exists():
            write_json(INDEX, {
                "schema_version": 1, "inputs": [], "fixtures": [],
                "missing_current_roots": ["host dependency closure", "all product UI/MW dependency closures"],
                "current_sources": {},
                "initialization": {"recorded_at_utc": analysis.stamp(),
                    "method": "empty_index_for_new_acquisitions",
                    "historical_claims_reused": False,
                    "scope": "Only subsequently acquired and hashed original inputs are indexed"},
            })
        else:
            index = json.loads(INDEX.read_bytes())
            if index.get("schema_version") != 1 or not isinstance(index.get("inputs"), list):
                raise ValueError("Existing evidence index is invalid; refusing to replace it")
    print(json.dumps({"index": relative(INDEX), "initialized": True}), flush=True)

    tool_dir = safe(ROOT / "tools/analyzers/wakaru-1.14.0")
    receipt = json.loads((tool_dir / "acquisition.json").read_bytes())
    executable = next(row for row in receipt["files"] if row["path"].endswith("/wakaru.exe"))
    tool = safe(ROOT / executable["path"])
    if tool.is_file():
        if tool.stat().st_size != executable["bytes"] or digest(tool) != executable["sha256"]:
            raise ValueError("Existing Wakaru differs from the pinned acquisition receipt")
    else:
        request = urllib.request.Request(receipt["download_url"],
                                         headers={"User-Agent": "razer-ui-static-evidence"})
        with urllib.request.urlopen(request, timeout=60) as response:
            raw = response.read()
        if len(raw) != receipt["download_bytes"] or hashlib.sha256(raw).hexdigest() != receipt["download_sha256"]:
            raise ValueError("Wakaru release archive differs from its pinned SHA-256/length")
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            members = [name for name in archive.namelist() if Path(name).name == "wakaru.exe"]
            if len(members) != 1:
                raise ValueError("Wakaru archive must contain exactly one executable")
            binary = archive.read(members[0])
        if len(binary) != executable["bytes"] or hashlib.sha256(binary).hexdigest() != executable["sha256"]:
            raise ValueError("Wakaru executable differs from its pinned SHA-256/length")
        temporary = tool.with_suffix(".exe.tmp")
        temporary.write_bytes(binary)
        temporary.replace(tool)
    # Version/help only executes the analyzer, never a vendor input.
    version = subprocess.run([str(tool), "--version"], capture_output=True, check=True)
    report = {"schema_version": 1, "recorded_at_utc": analysis.stamp(),
              "index": relative(INDEX), "tool": {**executable, "version_output": version.stdout.decode("utf-8").strip()},
              "pinned_receipt": {"path": relative(tool_dir / "acquisition.json"),
                                  "sha256": digest(tool_dir / "acquisition.json")},
              "historical_evidence_restored": False, "target_execution": False}
    report["python_static"] = prepare_static_python()
    write_json(ROOT / ".analysis/results/analysis-preparation.json", report)
    return report


def audit_inputs():
    """Audit every indexed original and fixture without stopping at missing files."""
    report = {
        "schema_version": 1,
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__)),
                 "python_version": sys.version.split()[0]},
        "index": {"path": relative(INDEX)},
        "index_errors": [],
        "entries": [],
        "target_execution": False,
        "real_device_operations": False,
    }
    try:
        raw = INDEX.read_bytes()
        report["index"].update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
        data = json.loads(raw)
        if not isinstance(data, dict):
            raise ValueError("index root must be an object")
    except (OSError, ValueError) as error:
        # Exception filenames can be workstation paths; keep diagnostics portable.
        report["index_errors"].append({"kind": type(error).__name__,
                                       "errno": getattr(error, "errno", None)})
        data = {}
    for group in ("inputs", "fixtures"):
        rows = data.get(group)
        if not isinstance(rows, list):
            report["index_errors"].append({"group": group, "kind": "invalid_or_missing_array"})
            continue
        for ordinal, row in enumerate(rows):
            entry = {"group": group, "ordinal": ordinal, "status": "error"}
            report["entries"].append(entry)
            if not isinstance(row, dict):
                entry["error"] = {"kind": "invalid_index_record"}
                continue
            name = row.get("path")
            if not isinstance(name, str) or not name or Path(name).is_absolute() or ".." in Path(name).parts:
                entry["error"] = {"kind": "invalid_project_relative_path"}
                continue
            entry["path"] = Path(name).as_posix()
            entry["expected"] = {"bytes": row.get("bytes"), "sha256": row.get("sha256")}
            if (type(row.get("bytes")) is not int or row["bytes"] < 0
                    or not isinstance(row.get("sha256"), str)
                    or not re.fullmatch(r"[0-9a-fA-F]{64}", row["sha256"])):
                entry["error"] = {"kind": "invalid_expected_byte_identity"}
                continue
            try:
                path = indexed_path(name, group)
                h = hashlib.sha256()
                size = 0
                with path.open("rb") as stream:
                    before = os.fstat(stream.fileno())
                    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                        size += len(chunk)
                        h.update(chunk)
                    after = os.fstat(stream.fileno())
                entry["actual"] = {"bytes": size, "sha256": h.hexdigest()}
                if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
                    entry["error"] = {"kind": "changed_while_reading"}
                    continue
                entry["mismatches"] = [
                    field for field in ("bytes", "sha256")
                    if entry["actual"][field] != (row[field].lower() if field == "sha256" else row[field])
                ]
                entry["status"] = "mismatch" if entry["mismatches"] else "verified"
            except FileNotFoundError:
                entry["status"] = "missing"
            except (OSError, ValueError) as error:
                entry["error"] = {"kind": type(error).__name__,
                                  "errno": getattr(error, "errno", None)}
    try:
        report["index_unchanged"] = hashlib.sha256(INDEX.read_bytes()).hexdigest() == report["index"].get("sha256")
    except OSError:
        report["index_unchanged"] = False
    if not report["index_unchanged"]:
        report["index_errors"].append({"kind": "index_unavailable_or_changed_during_audit"})
    statuses = ("verified", "missing", "mismatch", "error")
    counts = Counter(entry["status"] for entry in report["entries"])
    report["summary"] = {"total": len(report["entries"]), **{status: counts[status] for status in statuses},
                         "index_errors": len(report["index_errors"]),
                         "groups": {group: {"total": sum(entry["group"] == group for entry in report["entries"]),
                                            **{status: sum(entry["group"] == group and entry["status"] == status
                                                           for entry in report["entries"]) for status in statuses}}
                                    for group in ("inputs", "fixtures")}}
    report["passed"] = not report["index_errors"] and counts["verified"] == len(report["entries"])
    write_json(INPUT_AUDIT, report)
    return report


def image_resource_inputs():
    return [row for row in rust_inputs(include_asset_pack=False) if Path(row["path"]).suffix.lower() in IMAGE_RESOURCE_EXTENSIONS]


def verify_resources():
    """Decode the actual product image include closure without opening a window."""
    import io
    sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
    import PIL
    from PIL import Image

    baseline = image_resource_inputs()
    report = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__)),
                       "python_version": sys.version.split()[0], "pillow_version": PIL.__version__},
              "inputs": baseline, "entries": [], "target_execution": False,
              "real_device_operations": False, "application_runtime_acceptance": "not performed"}
    react_attributes = {"xmlnsXlink", "xlinkHref", "xmlSpace", "className", "strokeWidth",
                        "strokeMiterlimit", "strokeLinecap", "strokeLinejoin", "clipPath",
                        "fillRule", "clipRule", "fillOpacity", "strokeOpacity", "strokeDasharray",
                        "strokeDashoffset", "stopColor", "stopOpacity"}
    for original in baseline:
        entry = {"path": original["path"], "errors": []}
        report["entries"].append(entry)
        try:
            path = safe(ROOT / original["path"])
            raw = path.read_bytes()
            if len(raw) != original["bytes"] or hashlib.sha256(raw).hexdigest() != original["sha256"]:
                raise ValueError("Image changed after its input identity was captured")
            if path.suffix.lower() == ".svg":
                root = ET.fromstring(raw)
                if root.tag.split("}")[-1] != "svg":
                    raise ValueError("Image root is not SVG")
                unresolved = sorted({name for node in root.iter() for name in node.attrib
                                     if name in react_attributes})
                if unresolved:
                    entry["errors"].append({"kind": "unprojected_react_svg_attribute", "properties": unresolved})
                ids = {node.get("id") for node in root.iter() if node.get("id")}
                missing = sorted({href for node in root.iter() if node.tag.split("}")[-1] == "use"
                                  for name, href in node.attrib.items()
                                  if name in {"href", "{http://www.w3.org/1999/xlink}href"}
                                  and href.startswith("#") and href[1:] not in ids})
                if missing:
                    entry["errors"].append({"kind": "svg_use_target_missing", "references": missing})
                entry.update(format="SVG", xml_nodes=sum(1 for _ in root.iter()))
            else:
                with Image.open(io.BytesIO(raw)) as image:
                    image_format, image_size = image.format, image.size
                    image.verify()
                # Header recognition alone misses truncated pixel data. Decode
                # it through the maintained static codec, never GPUI or a DLL.
                with Image.open(io.BytesIO(raw)) as image:
                    image.load()
                if min(image_size) <= 0:
                    raise ValueError("Image dimensions are empty")
                entry.update(format=image_format, width=image_size[0], height=image_size[1])
        except (OSError, ValueError, ET.ParseError, Image.DecompressionBombError) as error:
            entry["errors"].append({"kind": type(error).__name__, "message": str(error)})
        entry["passed"] = not entry["errors"]
    report["inputs_unchanged"] = baseline == image_resource_inputs()
    counts = Counter(row.get("format", "unrecognized") for row in report["entries"])
    report["summary"] = {"total": len(baseline), "verified": sum(row["passed"] for row in report["entries"]),
                         "failed": sum(not row["passed"] for row in report["entries"]), "formats": dict(counts)}
    report["passed"] = report["inputs_unchanged"] and not report["summary"]["failed"]
    write_json(RESOURCE_AUDIT, report)
    return report


def clean():
    """Remove disposable scratch after project analyzer sessions have exited."""
    import ctypes
    from ctypes import wintypes
    for session in (ROOT / ".analysis/results/native").glob("*/*session.json"):
        row = json.loads(session.read_bytes())
        if os.name == "nt":
            api = ctypes.WinDLL("kernel32", use_last_error=True)
            api.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
            api.OpenProcess.restype = wintypes.HANDLE
            api.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
            api.CloseHandle.argtypes = [wintypes.HANDLE]
            handle = api.OpenProcess(0x00100000, False, row["pid"])
            if handle:
                running = api.WaitForSingleObject(handle, 0) == 258
                api.CloseHandle(handle)
                if running:
                    raise ValueError(f"Analyzer session {row['pid']} is active; scratch is in use")
    scratch = ROOT / ".work"
    if scratch.is_symlink() or scratch.is_junction():
        raise ValueError("Scratch root is a link")
    safe(scratch)
    for path in scratch.rglob("*"):
        if path.is_symlink() or path.is_junction():
            raise ValueError(f"Scratch contains a link: {path}")
        if not safe(path).is_relative_to(scratch.resolve()):
            raise ValueError("Scratch target escaped")
    entries = list(scratch.iterdir()) if scratch.exists() else []
    for path in entries:
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()
    scratch.mkdir(exist_ok=True)
    print(json.dumps({"scratch_removed_entries": len(entries)}), flush=True)


def inventory():
    rows = {}
    for name in ("docs", "tools", ".work"):
        files = []
        for path in sorted((ROOT / name).rglob("*")):
            if not path.is_file():
                continue
            if path.is_symlink():
                raise ValueError(f"Symlink must be reviewed: {path}")
            files.append({"path": relative(path), "bytes": path.stat().st_size, "sha256": digest(path)})
        rows[name] = files
        print(json.dumps({"scanned": name, "files": len(files),
                          "bytes": sum(row["bytes"] for row in files)}), flush=True)
    return rows
















def verify_results(data):
    """Check result bodies against their manifests, including recorded failures."""
    indexed = {row["path"]: row for row in data["inputs"]}
    verified = set()

    def record(row, base=ROOT):
        path = safe(base / row["path"])
        if not path.is_relative_to(base.resolve()):
            raise ValueError(f"Result escaped its manifest directory: {row['path']}")
        if not path.is_file() or ("bytes" in row and path.stat().st_size != row["bytes"]) or digest(path) != row["sha256"]:
            raise ValueError(f"Analysis artifact changed: {relative(path)}")
        verified.add(path)
        return path

    native_functions = 0
    for manifest in sorted((ROOT / ".analysis/results/native").glob("*/capture.json")):
        capture = json.loads(manifest.read_bytes())
        source = indexed.get(capture["input"])
        if source is None or source["sha256"] != capture["input_sha256"] or source["bytes"] != capture["input_bytes"]:
            raise ValueError(f"Native capture input differs: {relative(manifest)}")
        if len(capture["functions"]) != capture["functions_captured"]:
            raise ValueError(f"Native function count differs: {relative(manifest)}")
        if capture.get("capture_complete") and capture["functions_captured"] != capture["settled_functions_total"]:
            raise ValueError(f"Settled native function count differs: {relative(manifest)}")
        for row in capture.get("outputs", []):
            record(row, manifest.parent)
        bodies = set()
        addresses = set()
        for entry in capture["functions"]:
            path = record(entry, manifest.parent)
            row = json.loads(path.read_bytes())
            if row["address"] in addresses:
                raise ValueError(f"Duplicate native function: {relative(path)}")
            addresses.add(row["address"])
            bodies.add(path)
            if any(row[key] != entry[key] for key in ("address", "rva", "name")):
                raise ValueError(f"Native function identity differs: {relative(path)}")
            if int(row["address"], 16) - int(capture["image_base"], 16) != int(row["rva"], 16):
                raise ValueError(f"Native RVA differs: {relative(path)}")
            if (row["assembly"] is not None) != entry["assembly_captured"] or (row["pseudocode"] is not None) != entry["pseudocode_captured"]:
                raise ValueError(f"Native function capture status differs: {relative(path)}")
            for chunk in row["chunks"]:
                raw = bytes.fromhex(chunk["raw_hex"])
                if len(raw) != int(chunk["end"], 16) - int(chunk["start"], 16) or hashlib.sha256(raw).hexdigest() != chunk["sha256"]:
                    raise ValueError(f"Native chunk byte identity differs: {relative(path)}")
            native_functions += 1
        if bodies != {path.resolve() for path in (manifest.parent / "functions").glob("*.json")}:
            raise ValueError(f"Unlisted native function bodies: {relative(manifest)}")

    frontend_manifests = 0
    for manifest in sorted((ROOT / ".analysis/results/frontend").glob("*/analysis.json")):
        analysis = json.loads(manifest.read_bytes())
        for name in analysis["original_js_inputs"]:
            if name not in indexed:
                raise ValueError(f"Unindexed frontend analysis input: {name}")
        for row in analysis["outputs"]:
            path = record(row)
            if not path.is_relative_to(manifest.parent):
                raise ValueError(f"Frontend output escaped its result directory: {relative(path)}")
        frontend_manifests += 1

    validation_path = ROOT / ".analysis/validation.json"
    rust_validation_current = False
    validation_errors = []
    if validation_path.is_file():
        try:
            validation = json.loads(validation_path.read_bytes())
            if validation["inputs"] != rust_inputs():
                validation_errors.append("Rust validation inputs changed; run the applicable static/pure/mock checks")
            for command in validation["commands"]:
                try:
                    record({"path": command["log"], "sha256": command["log_sha256"]})
                except (OSError, ValueError, KeyError, TypeError) as error:
                    validation_errors.append(f"Rust validation log failed ({type(error).__name__}): {error}")
            rust_validation_current = validation["passed"] and not validation_errors
        except (OSError, ValueError, KeyError, TypeError) as error:
            validation_errors.append(f"Rust validation receipt failed ({type(error).__name__}): {error}")
    return {"analysis_artifacts_verified": len(verified), "native_function_bodies_verified": native_functions,
            "frontend_manifests_verified": frontend_manifests, "rust_validation_current": rust_validation_current,
            "analysis_verification_errors": validation_errors}


def verify():
    audit = audit_inputs()
    errors = []
    if not audit["passed"]:
        errors.append(f"Retained input verification failed: {relative(INPUT_AUDIT)}; "
                      "run python tools/workspace.py audit-inputs for the complete inventory")
    documents = sorted({ROOT / "README.md", ROOT / "AGENTS.md", ROOT / ".analysis/README.md",
                        ROOT / ".analysis/results/README.md",
                        ROOT / "tools/README.md", ROOT / "tools/ida/README.md",
                        *(ROOT / "docs").rglob("*.md"),
                        *(ROOT / "crates").rglob("*.md"),
                        *(ROOT / "app").rglob("*.md")})
    for path in documents:
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"Cannot read document: {relative(path)} ({type(error).__name__})")
            continue
        if "\ufffd" in text:
            errors.append(f"Invalid text: {relative(path)}")
        for target in re.findall(r'\[[^\]\n]*\]\(([^)\n]+)\)', text):
            target = target.strip().strip("<>").split("#", 1)[0]
            if not target or re.match(r'(?:[a-z]+:|//)', target, re.I):
                continue
            resolved = (path.parent / target).resolve()
            if resolved.is_relative_to(ROOT) and not resolved.exists():
                errors.append(f"{relative(path)}: missing link {target}")
    count = 0
    for folder in ("app", "crates"):
        for source in (ROOT / folder).rglob("*.rs"):
            try:
                for name in INCLUDE.findall(source.read_text(encoding="utf-8")):
                    if not safe(source.parent / name).is_file():
                        errors.append(f"Missing include: {relative(source)}: {name}")
                    count += 1
            except (OSError, UnicodeError, ValueError) as error:
                errors.append(f"Cannot verify includes: {relative(source)} ({type(error).__name__})")
    results = {}
    try:
        data = json.loads(INDEX.read_bytes())
        if digest(INDEX) != audit["index"].get("sha256"):
            raise ValueError("Input index changed after audit; rerun verification")
        results = verify_results(data)
        errors.extend(results.pop("analysis_verification_errors"))
    except (OSError, ValueError, KeyError, TypeError) as error:
        errors.append(f"Analysis result verification failed ({type(error).__name__}): {error}")
    print(json.dumps({"input_audit": relative(INPUT_AUDIT), "input_summary": audit["summary"],
                      "embedded_inputs_checked": count, "live_documents_checked": len(documents),
                      "errors": errors, "passed": not errors, **results}), flush=True)
    if errors:
        raise ValueError("\n".join(errors))


def rust_embed_references(text, *, spans=False):
    """Read literal macro arguments, ignoring Rust comments and string contents.

    Generated Rust can use raw strings or a non-.rs include filename. Conversely,
    include_str!/include_bytes! data is never parsed again as executable Rust.
    An expression-valued path needs an explicit audit rather than a silent omission.
    """
    token = re.compile(
        r'//[^\n]*|/\*|(?:br|r)(?P<hashes>\#*)"(?P<raw>[\s\S]*?)"(?P=hashes)'
        r'|"(?:\\[\s\S]|[^"\\])*"|\'(?:\\.|[^\'\\\n])\''
        r'|[A-Za-z_][A-Za-z_0-9]*|::|[^\s]')
    tokens = []
    skip_until = 0
    for match in token.finditer(text):
        if match.start() < skip_until:
            continue
        word = match.group(0)
        if word.startswith("//"):
            continue
        if word == "/*":
            depth = 1
            for boundary in re.finditer(r'/\*|\*/', text[match.end():]):
                depth += 1 if boundary.group(0) == "/*" else -1
                if depth == 0:
                    skip_until = match.end() + boundary.end()
                    break
            else:
                raise ValueError("Unterminated Rust block comment")
            continue
        literal = match.group("raw")
        if word.startswith('"'):
            literal = re.sub(r'\\\r?\n\s*', '', word[1:-1])

            def escape(item):
                if item[1] is not None:
                    return chr(int(item[1].replace('_', ''), 16))
                if item[2] is not None:
                    return chr(int(item[2], 16))
                return {'n': '\n', 'r': '\r', 't': '\t', '0': '\0',
                        '\\': '\\', '"': '"', "'": "'"}[item[3]]

            literal = re.sub(r'\\(?:u\{([0-9a-fA-F_]+)\}|x([0-9a-fA-F]{2})|(.))', escape, literal)
        tokens.append((word, literal, match.start(), match.end()))
    for index, (word, _, _, _) in enumerate(tokens):
        locale = word == "i18n" and [part[0] for part in tokens[max(0, index - 2):index]] == ["rust_i18n", "::"]
        if word not in ("include", "include_str", "include_bytes") and not locale:
            continue
        if index + 1 >= len(tokens) or tokens[index + 1][0] != "!":
            continue
        if (index + 3 >= len(tokens) or tokens[index + 2][0] not in ("(", "[", "{")
                or tokens[index + 3][1] is None):
            raise ValueError(f"Rust {word}! path is not a statically audited literal")
        argument = tokens[index + 3]
        result = ("locale" if locale else word, argument[1])
        yield (*result, argument[2], argument[3]) if spans else result


def product_input(path, source, *, locale=False):
    """Resolve every embedded edge, including locale files, inside product roots."""
    path = safe(path)
    parts = path.relative_to(ROOT).parts
    allowed = ("app", "crates", "assets", "locales")
    if not parts or parts[0] not in allowed or any(
            part in (".analysis", ".work", ".ref", "target") for part in parts):
        raise ValueError(f"Product include depends on analysis/tooling: {relative(source)} -> {relative(path)}")
    return path


def rust_inputs(*, include_asset_pack=True):
    """Follow the complete embedded resource closure, independent of analysis inputs.

    In particular, assets/*.rs embed images themselves. Hashing only the first
    include would miss a changed image and would not prove a portable checkout.
    Provenance JSON source fields are descriptions, never filesystem edges.
    """
    import resource_database
    files = {ROOT / "Cargo.toml", ROOT / "Cargo.lock"}
    if (ROOT / "crates/razer-resource-db/Cargo.toml").is_file():
        files.update({ROOT / resource_database.INSTALLATION, ROOT / resource_database.OUTPUT})
    for folder in ("app", "crates"):
        for path in (ROOT / folder).rglob("*"):
            if not path.is_file():
                continue
            if path.suffix == ".rs" or path.name == "Cargo.toml":
                files.add(product_input(path, ROOT / "Cargo.toml"))
    pending = [path for path in files if path.suffix == ".rs"]
    visited = set()
    while pending:
        path = pending.pop()
        if path in visited:
            continue
        visited.add(path)
        text = path.read_text(encoding="utf-8")
        if path == ROOT / "crates/razer-resource-db/src/lib.rs":
            # build.rs derives only small installation constants from the
            # policy and database already captured in this input closure.
            generated = 'include!(concat!(env!("OUT_DIR"), "/installation.rs"));'
            if text.count(generated) != 1:
                raise ValueError("Resource installation constants need a new build-output audit")
            text = text.replace(generated, "")
        for kind, literal in rust_embed_references(text):
            if re.match(r"(?:[A-Za-z]:|[/\\])", literal):
                raise ValueError(f"Nonportable include in {relative(path)}: {literal}")
            if kind == "locale":
                owner = path.parent
                while not (owner / "Cargo.toml").is_file():
                    owner = owner.parent
                    if owner == ROOT.parent:
                        raise ValueError(f"No crate owns locale include: {relative(path)}")
                locale_root = product_input(owner / literal, path, locale=True)
                if not locale_root.is_dir():
                    raise ValueError(f"Missing product locale directory: {relative(path)} -> {relative(locale_root)}")
                for child in locale_root.rglob("*"):
                    if child.is_file():
                        files.add(product_input(child, path, locale=True))
                continue
            included = product_input(path.parent / literal, path)
            if not included.is_file():
                raise ValueError(f"Missing product include: {relative(path)} -> {relative(included)}")
            files.add(included)
            if kind == "include":
                pending.append(included)
    return [{"path": relative(path), "bytes": path.stat().st_size, "sha256": digest(path)}
            for path in sorted(files)]


def static_react_svg(original_text, anchor):
    """Convert literal createElement SVG trees; reject executable expressions."""
    import families

    def parts(text):
        result, begin, index = [], 0, 0
        while index < len(text):
            char = text[index]
            if char in "([{":
                index = families.javascript_delimited(text, index)
                continue
            if char in "\"'":
                quote = char
                index += 1
                while index < len(text):
                    if text[index] == "\\":
                        index += 2
                        continue
                    if text[index] == quote:
                        break
                    index += 1
                else:
                    raise ValueError("Incomplete SVG source string")
            if char == ",":
                result.append(text[begin:index].strip())
                begin = index + 1
            index += 1
        result.append(text[begin:].strip())
        return result

    begin = original_text.index(anchor)
    opening = begin + len(anchor) - 1
    if original_text[opening] != "{":
        raise ValueError("SVG producer anchor must include its opening brace")
    end = families.javascript_delimited(original_text, opening)
    body = original_text[begin:end]
    returned = list(re.finditer(r'return ([\w$]+\.createElement)\("svg",', body))
    if len(returned) != 1:
        raise ValueError("Expected one complete SVG producer return")
    call_name = returned[0][1]
    expression = body[returned[0].start() + len("return "):].removesuffix("}").rstrip(";")
    omitted = []

    def attribute_name(name):
        # React's DOM host maps these property names to XML namespaces. The
        # literal extractor must preserve that mapping too: an SVG <use> with
        # xlinkHref instead of xlink:href silently loses its referenced shape.
        return {"xmlnsXlink": "xmlns:xlink", "xlinkHref": "xlink:href",
                "xmlSpace": "xml:space", "className": "class", "strokeWidth": "stroke-width",
                "strokeMiterlimit": "stroke-miterlimit", "strokeLinecap": "stroke-linecap",
                "strokeLinejoin": "stroke-linejoin", "clipPath": "clip-path",
                "fillRule": "fill-rule", "clipRule": "clip-rule"}.get(name, name)

    def node(expression, root=False):
        cached = re.fullmatch(r'([\w$]+)\|\|\(\1=(.*)\)', expression, flags=re.S)
        if cached:
            expression = cached[2]
        title = re.fullmatch(r'[\w$]+\?' + re.escape(call_name)
                            + r'\("title",\{id:[\w$]+\},[\w$]+\):null', expression)
        if title:
            omitted.append("optional title absent when no title property is supplied")
            return None
        prefix = call_name + "("
        if not expression.startswith(prefix):
            raise ValueError("Nonliteral SVG node expression: " + expression[:100])
        opening = len(prefix) - 1
        if families.javascript_delimited(expression, opening) != len(expression):
            raise ValueError("SVG node contains an unresolved expression suffix")
        arguments = parts(expression[opening + 1:-1])
        tag = families.static_js_literal(arguments[0])
        if not isinstance(tag, str) or not re.fullmatch(r'[A-Za-z][A-Za-z0-9]*', tag):
            raise ValueError("SVG element name is not literal")
        attributes = arguments[1]
        if root and attributes != "null" and not attributes.startswith("{"):
            merge = re.fullmatch(r'[\w$]+\((.*)\)', attributes, flags=re.S)
            if not merge:
                raise ValueError("Unresolved SVG root properties")
            merged = parts(merge[1])
            if len(merged) != 2 or not re.fullmatch(r'[\w$]+', merged[1]):
                raise ValueError("SVG root does not have the observed empty default-property merge")
            fields = []
            for field in parts(merged[0][1:-1]):
                name, value = field.split(":", 1)
                name = name.strip('"\'')
                if name in {"ref", "aria-labelledby"} and re.fullmatch(r'[\w$]+', value):
                    omitted.append(name + " runtime reference")
                else:
                    fields.append(field)
            attributes = "{" + ",".join(fields) + "}"
        properties = {} if attributes == "null" else families.static_js_literal(attributes)
        if not isinstance(properties, dict):
            raise ValueError("SVG attributes are not a literal object")
        element = ET.Element(tag)
        for name, value in properties.items():
            if name == "style":
                if not isinstance(value, dict) or any(not isinstance(v, (str, int, float)) for v in value.values()):
                    raise ValueError("SVG style is not literal")
                value = ";".join(re.sub(r'[A-Z]', lambda m: "-" + m[0].lower(), k) + ":" + str(v)
                                 for k, v in value.items())
            if not isinstance(value, (str, int, float)) or isinstance(value, bool):
                raise ValueError("Unresolved SVG attribute value")
            element.set(attribute_name(name), str(value))
        for child in arguments[2:]:
            if child.startswith(('"', "'")):
                # Object-key rewriting must not enter quoted CSS text. JSON5
                # decodes the whole literal and rejects expression suffixes.
                sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
                import json5
                text = json5.loads(child)
                if not isinstance(text, str):
                    raise ValueError("SVG text child is not a literal string")
                # React preserves literal text between element children (the
                # current mouse SVG producers also use it for <style> bodies).
                if len(element):
                    previous = element[-1]
                    previous.tail = (previous.tail or "") + text
                else:
                    element.text = (element.text or "") + text
                continue
            child_node = node(child)
            if child_node is not None:
                element.append(child_node)
        return element

    tree = node(expression, root=True)
    if tree.tag != "svg":
        raise ValueError("Producer root is not an SVG")
    try:
        module = families.complete_raw_module(original_text, anchor)
        module["body_sha256"] = hashlib.sha256(module.pop("body").encode("utf-8")).hexdigest()
    except ValueError as error:
        # CRA's bundle entry can sit outside numbered webpack modules. Its
        # complete literal producer remains above; never invent a module ID.
        module = {"resolution_failure": str(error), "producer_body_retained": True}
    return ET.tostring(tree, encoding="utf-8", short_empty_elements=False), {
        "operation": "Statically convert literal React SVG nodes with default properties; no JavaScript evaluation",
        "tool": "Python restricted literal parser / xml.etree.ElementTree", "version": sys.version.split()[0],
        "source_regions": [{"character_start": begin, "character_end": end, "body": body,
                            "body_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest()}],
        "module": module, "omitted_nonvisual_properties": omitted,
    }


def app_picker_category_literals(original_text):
    """Read the mounted category selector's literal tables, never evaluate JS."""
    import families
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    import json5
    regions = []

    def region(anchor, opening):
        if original_text.count(anchor) != 1:
            raise ValueError("App menu producer anchor is absent or ambiguous: " + anchor)
        begin = original_text.index(anchor)
        start = begin + len(anchor) - 1
        if original_text[start] != opening:
            raise ValueError("App menu producer boundary differs")
        end = families.javascript_delimited(original_text, start)
        body = original_text[begin:end]
        regions.append({"character_start": begin, "character_end": end, "body": body,
                        "body_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest()})
        return original_text[start:end]

    shapes = json5.loads(region("const y=O,I=[", "["))
    # P is the active reduce table. T belongs to an unused map expression.
    begin = original_text.index(",P={KEYBOARD:") + 1
    opening = begin + 2
    end = families.javascript_delimited(original_text, opening)
    body = original_text[begin:end]
    regions.append({"character_start": begin, "character_end": end, "body": body,
                    "body_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest()})
    transforms = json5.loads(original_text[opening:end])
    renderer = region("function Y(e){", "{")
    selector = region("const U=e=>{", "{")
    expected_renderer = '{const{width:a,height:t,d:r,transformX:n,transformY:o,fill:s,transformG_X:C,transformG_Y:i,click:c}=e;return(0,l.jsx)("svg",{xmlns:"http://www.w3.org/2000/svg",xmlnsXlink:"http://www.w3.org/1999/xlink",width:a,height:t,fill:s||"#ccc",viewBox:`0 0 ${a/2} ${t/2}`,onClick:c,children:(0,l.jsx)("g",{transform:`translate(${C||0} ${i||0}) ${a<40?`scale(${a/40})`:""} `,children:(0,l.jsx)("path",{d:r,transform:`translate(${n} ${o})`})})})}'
    expected_selector = '{const a=Object.keys(d.Su);let t=b.find(t=>a.includes(e.category)&&e.subCategory?t.name===e.subCategory:t.name===e.category);return!t&&e.subCategory&&(t=b.find(a=>a.name===e.category)),t&&(0,l.jsx)(Y,{d:t.d,width:e.size||40,height:e.size||40,transformX:t.transformX,transformY:t.transformY})}'
    if renderer != expected_renderer or selector != expected_selector:
        raise ValueError("Current app menu SVG renderer/selector requires a new static audit")
    reduction = 'b=(I.map(e=>T[e.name]?{...e,...T[e.name]}:e),I.reduce((e,a)=>(P[a.name]&&e.push({...a,...P[a.name]}),e),[]))'
    if original_text.count(reduction) != 1:
        raise ValueError("Current app menu active category table differs")
    active = {}
    for shape in shapes:
        if set(shape) != {"name", "d"} or not isinstance(shape["d"], str):
            raise ValueError("Current app menu category path is not a literal")
        if shape["name"] in transforms:
            transform = transforms[shape["name"]]
            if set(transform) != {"transformX", "transformY"} or any(type(v) not in (int, float) for v in transform.values()):
                raise ValueError("Current app menu category translation is not literal")
            active[shape["name"]] = {**shape, **transform}
    if len(active) != len(transforms):
        raise ValueError("Current app menu category path/translation closure differs")
    return active, regions


def static_app_picker_category_svg(original_text, category):
    active, regions = app_picker_category_literals(original_text)
    shape = active[category]
    tree = ET.Element("svg", {"xmlns": "http://www.w3.org/2000/svg",
        "xmlns:xlink": "http://www.w3.org/1999/xlink", "width": "40", "height": "40",
        "fill": "#ccc", "viewBox": "0 0 20 20"})
    group = ET.SubElement(tree, "g", {"transform": "translate(0 0)  "})
    ET.SubElement(group, "path", {"d": shape["d"],
        "transform": f'translate({shape["transformX"]} {shape["transformY"]})'})
    return ET.tostring(tree, encoding="utf-8", short_empty_elements=False), {
        "kind": "app_picker_category_svg", "category": category,
        "operation": "Serialize the mounted app-menu category SVG at its caller default size 40, preserving its literal path, active P translation, viewBox and fill; no JavaScript evaluation",
        "tool": "Python json5 restricted literals / xml.etree.ElementTree", "version": sys.version.split()[0],
        "literal_shape": shape,
        "source_regions": [({k: v for k, v in region.items() if k != "body"} if index < 2 else region)
                           for index, region in enumerate(regions)],
        "complete_tables": ".analysis/results/frontend/rz-app-menu-current/device-artwork.json",
    }


def complete_resource_module(original_text, anchor):
    """Keep grammar-owned bodies when a minified regex defeats delimiter scans."""
    import families
    try:
        return families.complete_raw_module(original_text, anchor)
    except ValueError:
        offset = original_text.index(anchor)
        owners = [module for module in families.keyboard_source_modules(original_text)
                  if module["character_start"] <= offset < module["character_end"]]
        if not owners:
            raise ValueError("Resource anchor is outside every complete grammar-owned module")
        return min(owners, key=lambda module: module["character_end"] - module["character_start"])


@lru_cache(maxsize=4)
def validate_jsx_svg_original(original_text):
    """Validate the entire input, including bytes outside a selected owner."""
    import tree_sitter
    import tree_sitter_javascript
    parser = tree_sitter.Parser(tree_sitter.Language(tree_sitter_javascript.language()))
    if parser.parse(original_text.encode("utf-8")).root_node.has_error:
        raise ValueError("Current JSX SVG original contains an AST error")


def static_jsx_svg(original_text, specification):
    """Extract one literal React JSX-runtime SVG using complete AST ownership."""
    import families
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    import tree_sitter
    import tree_sitter_javascript
    import json5
    validate_jsx_svg_original(original_text)
    parser = tree_sitter.Parser(tree_sitter.Language(tree_sitter_javascript.language()))
    anchor = specification["anchor"]
    if original_text.count(anchor) != 1:
        raise ValueError("Current JSX SVG declaration anchor is ambiguous")
    module = complete_resource_module(original_text, anchor)
    if module["module_id"] != specification["module_id"]:
        raise ValueError("Current JSX SVG producer module differs")
    # Webpack's concatenated entry is a complete arrow expression rather than
    # a numbered property. Preserve that grammar when reparsing the owner.
    concatenated = str(module["module_id"]).startswith("concatenated-entry@")
    prefix = "const entry=" if concatenated else "const modules={"
    suffix = ";" if concatenated else "};"
    encoded = (prefix + module["body"] + suffix).encode("utf-8")
    tree = parser.parse(encoded)
    if tree.root_node.has_error:
        raise ValueError("Current complete JSX SVG module contains an AST error")
    declarations = []
    def visit(node):
        if node.type == "variable_declarator":
            text = encoded[node.start_byte:node.end_byte].decode("utf-8")
            if text.startswith(anchor):
                declarations.append((node, text))
        for child in node.named_children:
            visit(child)
    visit(tree.root_node)
    if len(declarations) != 1:
        raise ValueError("Current JSX SVG has no unique complete declaration")
    declaration_node, declaration = declarations[0]
    declaration_start = module["character_start"] + len(encoded[len(prefix):declaration_node.start_byte].decode("utf-8"))
    matches = list(re.finditer(r'\(0,[\w$]+\.jsx[s]?\)\("svg",\{', declaration))
    index = specification["svg_index"]
    if type(index) is not int or not 0 <= index < len(matches):
        raise ValueError("Current JSX SVG index is absent")
    match = matches[index]
    call_opening = declaration.index('("svg",', match.start())
    call_end = families.javascript_delimited(declaration, call_opening)
    expression = declaration[match.start():call_end]
    omitted = []
    def fields(text):
        result, begin, cursor = [], 0, 0
        while cursor < len(text):
            if text[cursor] in "([{":
                cursor = families.javascript_delimited(text, cursor)
                continue
            if text[cursor] in "\"'":
                quote = text[cursor]
                cursor += 1
                while cursor < len(text) and text[cursor] != quote:
                    cursor += 2 if text[cursor] == "\\" else 1
                if cursor >= len(text):
                    raise ValueError("Incomplete JSX SVG string")
            if text[cursor] == ",":
                result.append(text[begin:cursor].strip()); begin = cursor + 1
            cursor += 1
        result.append(text[begin:].strip())
        return result
    def node(expression, root=False):
        call = re.fullmatch(r'\(0,[\w$]+\.jsx[s]?\)\((.*)\)', expression, re.S)
        if not call:
            raise ValueError("Nonliteral JSX SVG node expression")
        arguments = fields(call[1])
        if len(arguments) != 2:
            raise ValueError("Unresolved JSX SVG arguments")
        tag = json5.loads(arguments[0])
        if not isinstance(tag, str) or not re.fullmatch(r"[A-Za-z][A-Za-z0-9]*", tag):
            raise ValueError("JSX SVG tag is not literal")
        props = arguments[1]
        if not props.startswith("{") or families.javascript_delimited(props, 0) != len(props):
            raise ValueError("JSX SVG properties are not a complete object")
        element = ET.Element(tag)
        for field in fields(props[1:-1]):
            if root and re.fullmatch(r"\.\.\.[\w$]+", field):
                omitted.append("forwarded runtime root properties; source caller supplies only event handler")
                continue
            if ":" not in field:
                raise ValueError("Nonliteral JSX SVG property")
            name, value = field.split(":", 1)
            name = name.strip().strip('"\'')
            if name == "children":
                children = fields(value[1:-1]) if value.startswith("[") else [value]
                for child in children:
                    element.append(node(child))
                continue
            literal = json5.loads(value)
            if not isinstance(literal, (str, int, float)) or isinstance(literal, bool):
                raise ValueError("JSX SVG attribute is not scalar literal")
            name = {"className":"class", "strokeWidth":"stroke-width", "strokeLinecap":"stroke-linecap",
                    "strokeLinejoin":"stroke-linejoin", "fillRule":"fill-rule", "clipRule":"clip-rule",
                    "clipPath":"clip-path", "xmlnsXlink":"xmlns:xlink", "xlinkHref":"xlink:href"}.get(name, name)
            if name.startswith("on") or name == "style":
                raise ValueError("JSX SVG has executable or unresolved attributes")
            element.set(name, str(literal))
        return element
    svg = node(expression, root=True)
    if svg.tag != "svg":
        raise ValueError("Current JSX SVG root differs")
    if "xmlns" not in svg.attrib:
        svg.set("xmlns", "http://www.w3.org/2000/svg")
    opacity_layers = []
    if "opacity_layer" in specification:
        parents = {child: parent for parent in svg.iter() for child in parent}
        animated = []
        for target in list(svg.iter()):
            tracks = [child for child in target if child.tag == "animate"]
            if not tracks:
                continue
            if not target.get("id") or len(tracks) != 1 or tracks[0].get("attributeName") != "opacity":
                raise ValueError("Inline SVG opacity layer needs one named, literal opacity track")
            opacity_layers.append({"id": target.get("id"), "initial_opacity": float(target.get("opacity", "1")),
                                   "animation": dict(tracks[0].attrib)})
            animated.append(target)
        layer = specification["opacity_layer"]
        if layer == "base":
            for target in animated:
                parents[target].remove(target)
        else:
            selected = [target for target in animated if target.get("id") == layer]
            if len(selected) != 1:
                raise ValueError("Inline SVG requested opacity layer is absent/ambiguous")
            target = selected[0]
            target.set("opacity", "1")
            for child in list(target):
                if child.tag == "animate":
                    target.remove(child)
            retained = set(target.iter())
            for definition in svg.iter("defs"):
                retained.update(definition.iter())
            ancestor = target
            while ancestor is not svg:
                ancestor = parents[ancestor]
                retained.add(ancestor)
            for parent in list(svg.iter()):
                for child in list(parent):
                    if child not in retained and child.tag != "defs":
                        parent.remove(child)
    return ET.tostring(svg, encoding="utf-8", short_empty_elements=False), {
        "operation":"Statically convert a literal JSX-runtime SVG from its complete AST declaration; no JavaScript evaluation",
        "tool":"tree-sitter-javascript / Python restricted JSON5 literals / xml.etree.ElementTree",
        "version":sys.version.split()[0], "analyzers":{name:__import__('importlib.metadata', fromlist=['version']).version(name)
            for name in ("tree-sitter", "tree-sitter-javascript", "json5")},
        "module":module, "producer":{"character_start":declaration_start,
            "character_end":declaration_start+len(declaration), "body":declaration,
            "body_sha256":hashlib.sha256(declaration.encode("utf-8")).hexdigest()},
        "source_regions":[{"character_start":declaration_start+match.start(),
            "character_end":declaration_start+call_end,"body":expression,
            "body_sha256":hashlib.sha256(expression.encode("utf-8")).hexdigest()}],
        "svg_index":index,"svg_count":len(matches),"omitted_nonvisual_properties":omitted,
        "opacity_layer": specification.get("opacity_layer"), "source_opacity_layers": opacity_layers,
        "target_execution":False,
    }


def static_svg_button_layer(content, button_index, state, css_text):
    """Split current data-btn geometry and materialize source CSS state rules."""
    selectors = {
        "normal": ".side-view-svg [data-btn]",
        "hover": ".side-view-svg [data-btn].is-hover",
        "active": ".side-view-svg [data-btn].is-active",
        "hyper_hover": ".side-view-svg.is-hyper [data-btn].is-hover",
        "hyper_active": ".side-view-svg.is-hyper [data-btn].is-active",
        "disabled": ".side-view-svg [data-btn].is-disabled",
    }
    if state not in {"base", "background", "foreground", "normal", "hover", "active", "hyper_hover", "hyper_active"}:
        raise ValueError("Unknown current source SVG button state")
    rules = {}
    for name, selector in selectors.items():
        matches = list(re.finditer(re.escape(selector) + r"\{([^{}]*)\}", css_text))
        if len(matches) != 1:
            raise ValueError("Current source SVG state selector is ambiguous: " + selector)
        match = matches[0]
        fields = {}
        for field in match[1].split(";"):
            if field:
                key, value = field.split(":", 1)
                fields[key.strip()] = value.removesuffix("!important").strip()
        rules[name] = {"character_start": match.start(), "character_end": match.end(),
                       "selector": selector, "body": match[0],
                       "body_sha256": hashlib.sha256(match[0].encode("utf-8")).hexdigest(),
                       "declarations": fields}
    expected_base = {"stroke": "#666", "fill": "#0000", "fill-opacity": "0", "opacity": "1"}
    if any(rules["normal"]["declarations"].get(key) != value for key, value in expected_base.items()):
        raise ValueError("Current source SVG normal appearance differs from its renderer")
    if rules["disabled"]["declarations"].get("opacity") != ".25":
        raise ValueError("Current source SVG disabled opacity differs from its renderer")
    root = ET.fromstring(content)
    parents = {child: parent for parent in root.iter() for child in parent}
    buttons = [node for node in root.iter() if node.get("data-btn") is not None]
    indices = [node.get("data-btn") for node in buttons]
    if (not indices or len(set(indices)) != len(indices)
            or any(not re.fullmatch(r"\d+", index) for index in indices)):
        raise ValueError("Current SVG button identifiers are absent or ambiguous")
    if any(node.tag.split("}")[-1] != "path" for node in buttons):
        raise ValueError("Current SVG button contains an unresolved non-path geometry")
    source_buttons = [{"index": int(node.get("data-btn")), "attributes": dict(node.attrib)} for node in buttons]

    def retain_geometry(selected):
        retained = set(selected)
        for node in selected:
            ancestor = parents[node]
            while ancestor is not root:
                retained.add(ancestor)
                ancestor = parents[ancestor]

        def prune(node):
            for child in list(node):
                tag = child.tag.split("}")[-1]
                if tag in {"defs", "style", "title", "desc", "metadata"}:
                    continue
                if child not in retained:
                    node.remove(child)
                else:
                    prune(child)

        prune(root)

    paint_order = None
    if state == "base":
        if button_index is not None:
            raise ValueError("Source SVG base must not select a button")
        for node in buttons:
            parents[node].remove(node)
        applied = {}
    elif state in {"background", "foreground"}:
        if button_index is not None:
            raise ValueError("Source SVG draw-order layer must not select a button")
        graphic_tags = {"path", "rect", "circle", "ellipse", "polygon", "polyline", "text", "line", "image", "use"}
        graphics = [node for node in root.iter() if node.tag.split("}")[-1] in graphic_tags]
        positions = [graphics.index(node) for node in buttons]
        first, last = min(positions), max(positions)
        if any(node not in buttons for node in graphics[first:last + 1]):
            raise ValueError("Current source SVG interleaves foreground geometry among button paths")
        paint_order = [{"tag": node.tag.split("}")[-1], "id": node.get("id"), "button_index": node.get("data-btn")}
                       for node in graphics]
        selected = graphics[:first] if state == "background" else graphics[last + 1:]
        retain_geometry(selected)
        applied = {}
    else:
        if type(button_index) is not int or str(button_index) not in indices:
            raise ValueError("Selected source SVG button identifier is absent")
        target = next(node for node in buttons if int(node.get("data-btn")) == button_index)
        retain_geometry([target])
        applied = {key: value for key, value in rules["normal"]["declarations"].items()
                   if key in {"stroke", "stroke-width", "fill", "fill-opacity", "opacity"}}
        cascade = ["hover" if state.endswith("hover") else "active"] if state != "normal" else []
        if state.startswith("hyper_"):
            cascade.append(state)
        for name in cascade:
            applied.update({key: value for key, value in rules[name]["declarations"].items()
                            if key in {"stroke", "stroke-width", "fill", "fill-opacity", "opacity"}})
        inline_style = {}
        for field in target.get("style", "").split(";"):
            if field.strip():
                key, value = field.split(":", 1)
                inline_style[key.strip()] = value.strip()
        inline_style.update(applied)
        target.set("style", ";".join(key + ":" + value for key, value in inline_style.items()))
    ET.register_namespace("", "http://www.w3.org/2000/svg")
    ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
    derivation = {
        "tool": "Python xml.etree.ElementTree / restricted literal CSS declarations",
        "version": sys.version.split()[0],
        "operation": "Split only current source data-btn paths into a preserved base and source CSS state layers",
        "state": state, "button_index": button_index, "root_attributes": dict(root.attrib),
        "source_buttons": source_buttons, "complete_css_rules": list(rules.values()),
        "applied_visual_declarations": applied,
        "disabled_opacity_owner": "Rust layer container applies the current source .is-disabled opacity .25",
        "target_execution": False,
    }
    if paint_order is not None:
        derivation["source_graphics_paint_order"] = paint_order
        derivation["native_button_paint_owner"] = "Rust source paths paint between preserved background and foreground SVG resources"
    return ET.tostring(root, encoding="utf-8"), derivation


def static_smil_svg(content, seconds):
    """Sample literal SMIL colors/translations and straight motion paths."""
    import math

    if not math.isfinite(seconds) or seconds < 0:
        raise ValueError("Invalid source animation time")
    root = ET.fromstring(content)
    identified = {node.get("id"): node for node in root.iter() if node.get("id")}
    tracks = []
    translations = {}

    def spline(progress, controls):
        x1, y1, x2, y2 = controls
        def cubic(t, first, second):
            return 3 * (1 - t) ** 2 * t * first + 3 * (1 - t) * t ** 2 * second + t ** 3
        lower, upper = 0., 1.
        for _ in range(40):
            middle = (lower + upper) / 2
            if cubic(middle, x1, x2) < progress:
                lower = middle
            else:
                upper = middle
        return cubic((lower + upper) / 2, y1, y2)

    def interpolate(attributes, values, phase):
        times = [float(value) for value in attributes.get("keyTimes", "0;1").split(";")]
        if len(times) != len(values) or times[0] != 0 or times[-1] != 1 or sorted(set(times)) != times:
            raise ValueError("Incomplete source SMIL key times")
        segment = min(len(times) - 2, max(0, bisect.bisect_right(times, phase) - 1))
        progress = (phase - times[segment]) / (times[segment + 1] - times[segment])
        mode = attributes.get("calcMode", "linear")
        if mode == "spline":
            splines = attributes["keySplines"].split(";")
            if len(splines) != len(times) - 1:
                raise ValueError("Incomplete source SMIL splines")
            controls = [float(value) for value in splines[segment].split()]
            if len(controls) != 4:
                raise ValueError("Invalid source SMIL spline")
            progress = spline(progress, controls)
        elif mode != "linear":
            raise ValueError("Unresolved source SMIL interpolation: " + mode)
        left, right = values[segment], values[segment + 1]
        return [a + (b - a) * progress for a, b in zip(left, right)]

    def motion(attributes, phase):
        path = attributes["path"]
        match = re.fullmatch(r'\s*M\s*([-\d.]+)\s+([-\d.]+)\s*((?:C[-\d.,\s]+)+)\s*', path)
        if not match:
            raise ValueError("Unresolved source SMIL motion path")
        previous = (float(match[1]), float(match[2]))
        segments, total = [], 0.
        for source in match[3].split("C")[1:]:
            numbers = [float(value) for value in re.split(r'[,\s]+', source.strip())]
            if len(numbers) != 6:
                raise ValueError("Incomplete source SMIL motion segment")
            first, second, target = tuple(numbers[:2]), tuple(numbers[2:4]), tuple(numbers[4:])
            delta = (target[0] - previous[0], target[1] - previous[1])
            length = math.hypot(*delta)
            if length:
                for control in (first, second):
                    if abs((control[0] - previous[0]) * delta[1] - (control[1] - previous[1]) * delta[0]) > 1e-7:
                        raise ValueError("Curved source SMIL motion needs a path-length analyzer")
                    projection = ((control[0] - previous[0]) * delta[0] + (control[1] - previous[1]) * delta[1]) / length ** 2
                    if not 0 <= projection <= 1:
                        raise ValueError("Nonmonotonic source SMIL motion is unresolved")
            elif first != previous or second != previous:
                raise ValueError("Source SMIL segment has an unresolved loop")
            segments.append((total, length, previous, target))
            total += length
            previous = target
        if total == 0:
            return list(previous)
        distance = interpolate(attributes, [[float(value)] for value in attributes["keyPoints"].split(";")], phase)[0] * total
        for start, length, left, right in segments:
            if length and distance <= start + length:
                progress = (distance - start) / length
                return [left[axis] + (right[axis] - left[axis]) * progress for axis in (0, 1)]
        return list(previous)

    for parent in list(root.iter()):
        for animation in list(parent):
            kind = animation.tag.split("}")[-1]
            if kind not in {"animate", "animateTransform", "animateMotion"}:
                continue
            attributes = dict(animation.attrib)
            target_id = attributes.get("{http://www.w3.org/1999/xlink}href", attributes.get("href", ""))
            if not target_id.startswith("#") or target_id[1:] not in identified:
                raise ValueError("Source SMIL target is unresolved")
            target = identified[target_id[1:]]
            if attributes.get("begin", "0s") != "0s" or not re.fullmatch(r'[\d.]+s', attributes["dur"]):
                raise ValueError("Source SMIL lifecycle is unresolved")
            duration = float(attributes["dur"][:-1])
            if duration <= 0:
                raise ValueError("Source SMIL duration must be positive")
            phase = seconds % duration / duration if attributes.get("repeatCount") == "indefinite" else min(1., seconds / duration)
            if kind == "animateMotion":
                if target.get("transform") or attributes.get("rotate"):
                    raise ValueError("Source SMIL motion transform composition is unresolved")
                translations.setdefault(target_id, []).append(motion(attributes, phase))
            elif kind == "animateTransform":
                if attributes.get("type") != "translate" or attributes.get("additive") != "sum" or target.get("transform"):
                    raise ValueError("Source SMIL transform composition is unresolved")
                values = [[float(value) for value in item.split()] for item in attributes["values"].split(";")]
                if any(len(value) != 2 for value in values):
                    raise ValueError("Source SMIL translate is unresolved")
                translations.setdefault(target_id, []).append(interpolate(attributes, values, phase))
            elif attributes["attributeName"] == "fill":
                colors = attributes["values"].split(";")
                if any(not re.fullmatch(r'#[0-9a-fA-F]{6}', color) for color in colors):
                    raise ValueError("Source SMIL color is unresolved")
                values = [[int(color[index:index + 2], 16) for index in (1, 3, 5)] for color in colors]
                channels = interpolate(attributes, values, phase)
                target.set("fill", "#" + "".join(f"{round(value):02x}" for value in channels))
            elif attributes["attributeName"] == "opacity":
                values = [[float(attributes["from"])], [float(attributes["to"])]]
                target.set("opacity", f"{interpolate(attributes, values, phase)[0]:.8g}")
            else:
                raise ValueError("Source SMIL attribute is unresolved: " + attributes["attributeName"])
            tracks.append({"kind": kind, "attributes": attributes})
            parent.remove(animation)
    if not tracks:
        raise ValueError("No source SMIL tracks found")
    for target_id, offsets in translations.items():
        x, y = [sum(value[axis] for value in offsets) for axis in (0, 1)]
        identified[target_id[1:]].set("transform", f"translate({x:.8g} {y:.8g})")
    ET.register_namespace("", "http://www.w3.org/2000/svg")
    ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
    return ET.tostring(root, encoding="utf-8"), {
        "operation": "Sample literal source SMIL tracks into a static SVG frame; original geometry preserved",
        "tool": "Python restricted SMIL sampler / xml.etree.ElementTree", "version": sys.version.split()[0],
        "seconds": seconds, "source_tracks": tracks,
    }


def resource_reference_files(excluded=()):
    excluded_directories = {".git", "target", "node_modules", "__pycache__", "analyzers", "ida"}
    extensions = {".rs", ".json", ".py", ".toml", ".md", ".txt", ".css", ".js", ".ps1", ".yml", ".yaml"}
    for folder in ("app", "crates", "assets", "locales", "tools", "docs"):
        for directory, children, names in os.walk(ROOT / folder):
            children[:] = sorted(name for name in children if name not in excluded_directories)
            for name in sorted(names):
                path = Path(directory) / name
                if path.suffix.lower() in extensions and path.name != "resource-deduplication.json" and path not in excluded:
                    yield safe(path)
    result = subprocess.run(["rg", "--files-with-matches", "--fixed-strings", "--hidden",
                             "--glob", "!**/native/**", "--glob", "!**/modules/**",
                             "--glob", "!**/chunk-references/**", "--glob", "!**/functions/**",
                             "--glob", "!**/asar/**", "assets/synapse/", ".analysis/results"],
                            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, encoding="utf-8")
    if result.returncode not in (0, 1):
        raise ValueError("Cannot audit generated resource references: " + result.stderr)
    for name in sorted(result.stdout.splitlines()):
        path = ROOT / name
        if path.suffix.lower() in extensions and path.name != "resource-deduplication.json" and path not in excluded:
            yield safe(path)
    for path in sorted(ROOT.iterdir()):
        if path.is_file() and path.suffix.lower() in extensions:
            yield safe(path)


def resource_reference_plan(mapping, *, excluded=()):
    changes, references, blockers, snapshots = {}, [], [], []
    source_label = ""
    known_hashes = {}
    basenames = {}
    for name in mapping:
        basenames.setdefault(Path(name).name, []).append(name)
    pattern = re.compile(r"[A-Za-z0-9_./\\-]+\.(?:svg|png|jpg|jpeg|webp|avif|gif|bmp|ico|json)\b", re.I)

    def affected(value, source):
        normalized = value.replace("\\", "/")
        if normalized in mapping:
            return {normalized}
        if source_label.startswith(".analysis/") and "assets/synapse/" not in normalized:
            return set()
        matched = set()
        for match in pattern.finditer(normalized):
            token = match[0]
            for candidate in basenames.get(Path(token).name, []):
                if token == candidate or token.endswith("/" + candidate):
                    matched.add(candidate)
                elif not source_label.startswith(".analysis/"):
                    for base in (source.parent, ROOT, ROOT / "assets/synapse"):
                        if (base / token).resolve() == ROOT / candidate:
                            matched.add(candidate)
        return matched

    def record(source, candidate, location, kind, replacement=None):
        row = {"owner": source_label, "location": location, "resource": candidate, "kind": kind}
        if replacement is not None:
            row["replacement"] = replacement
        references.append(row)
        if kind == "unresolved":
            blockers.append(row)

    for source in resource_reference_files(excluded):
        source_label = relative(source)
        raw = source.read_bytes()
        snapshots.append({"path": source_label, "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)})
        if len(snapshots) % 1000 == 0:
            print(json.dumps({"resource_reference_files": len(snapshots), "last": source_label}), flush=True)
        if source_label.startswith(".analysis/") and b"assets/synapse/" not in raw:
            continue
        text = raw.decode("utf-8-sig")
        if not any(Path(match[0].replace("\\", "/")).name in basenames for match in pattern.finditer(text)):
            continue
        rewritten = text
        if source.suffix == ".rs":
            edits = []
            covered = []
            for kind, literal, begin, end in rust_embed_references(text, spans=True):
                if kind == "locale":
                    continue
                target = safe(source.parent / literal)
                name = relative(target)
                if name in mapping:
                    replacement = os.path.relpath(ROOT / mapping[name], source.parent).replace("\\", "/")
                    edits.append((begin, end, json.dumps(replacement)))
                    covered.append((begin, end))
                    record(source, name, f"{kind}@{begin}", "rust_literal", replacement)
            for begin, end, replacement in reversed(edits):
                rewritten = rewritten[:begin] + replacement + rewritten[end:]
            remaining = text
            for begin, end in reversed(covered):
                remaining = remaining[:begin] + " " * (end - begin) + remaining[end:]
            for candidate in affected(remaining, source):
                record(source, candidate, "outside_literal_include", "unresolved")
        elif source.suffix == ".json":
            data = json.loads(text)
            edits = {}

            def visit(value, pointer="", field="", owner=None, historical=False):
                historical = historical or field == "original_paths"
                if isinstance(value, dict):
                    for key, child in value.items():
                        visit(child, pointer + "/" + key.replace("~", "~0").replace("/", "~1"), key, value, historical)
                elif isinstance(value, list):
                    for index, child in enumerate(value):
                        visit(child, pointer + "/" + str(index), field, owner, historical)
                elif isinstance(value, str):
                    candidates = affected(value, source)
                    for candidate in candidates:
                        if (source_label in {".analysis/results/resource-audit.json", ".analysis/results/asset-pack-manifest.json"}
                                and field == "path" and value == candidate):
                            record(source, candidate, pointer, "generated_audit_snapshot_requires_regeneration")
                            continue
                        if field == "path" and candidate not in known_hashes:
                            known_hashes[candidate] = digest(ROOT / candidate)
                        typed_hash = None
                        typed_reference = False
                        if isinstance(owner, dict):
                            if source.name == "keyboard-panel-dropdown-manifest.json" and field == "asset":
                                typed_reference = True
                                typed_hash = owner.get("sha256")
                            elif source.name == "keyboard-keypad-artwork-manifest.json" and field == "asset":
                                typed_reference = True
                                typed_hash = owner.get("resource_sha256")
                            elif source.name == "keyboard-product-manifest.json" and field == "customize_output":
                                typed_reference = True
                                typed_hash = owner.get("customize_output_sha256")
                        if typed_reference and candidate not in known_hashes:
                            known_hashes[candidate] = digest(ROOT / candidate)
                        if historical:
                            record(source, candidate, pointer, "historical_original_path")
                        elif value == candidate and (field in {"output", "physical_path"}
                                or typed_reference
                                or (field == "path" and isinstance(owner, dict)
                                    and owner.get("sha256") == known_hashes[candidate])):
                            edits[pointer] = mapping[candidate]
                            record(source, candidate, pointer, "typed_physical_path", mapping[candidate])
                            if typed_reference and typed_hash != known_hashes[candidate]:
                                references[-1]["provenance_sha256_discrepancy"] = {
                                    "recorded": typed_hash, "actual": known_hashes[candidate],
                                    "preserved_original_provenance": True}
                        else:
                            record(source, candidate, pointer, "unresolved")

            visit(data)
            if edits:
                for pointer, replacement in edits.items():
                    components = [part.replace("~1", "/").replace("~0", "~") for part in pointer.split("/")[1:]]
                    owner = data
                    for component in components[:-1]:
                        owner = owner[int(component)] if isinstance(owner, list) else owner[component]
                    key = int(components[-1]) if isinstance(owner, list) else components[-1]
                    owner[key] = replacement
                rewritten = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
        else:
            remaining = text
            if source_label == "tools/workspace.py":
                import ast
                for node in ast.parse(text).body:
                    if isinstance(node, ast.FunctionDef) and node.name == "resource_configuration_plan":
                        body = ast.get_source_segment(text, node)
                        for candidate in affected(body, source):
                            record(source, candidate, "resource_configuration_plan", "migration_source_locator")
                        remaining = remaining.replace(body, "", 1)
            for candidate in affected(remaining, source):
                record(source, candidate, "unclassified_text_reference", "unresolved")
        if rewritten != text:
            encoding = "utf-8-sig" if raw.startswith(b"\xef\xbb\xbf") else "utf-8"
            changes[source_label] = rewritten.encode(encoding)
    return changes, references, blockers, snapshots


def registered_resource_products():
    """Use the complete validated maintenance directory for endpoint IDs only."""
    import resource_database
    return {row["id"] for row in resource_database.product_registry()}


def current_product_favicon(product_id, acquisition, cache):
    """Parse a verified current HTML entry; no browser or vendor JS execution."""
    import families
    key = (product_id, acquisition)
    if key in cache:
        return cache[key]
    receipt_path = safe(ROOT / acquisition)
    if not receipt_path.is_relative_to(ROOT / ".analysis/inputs/frontend"):
        raise ValueError("Favicon acquisition must be a retained current frontend input")
    receipt = json.loads(receipt_path.read_bytes())
    if not receipt.get("source_current_during_acquisition"):
        raise ValueError("Favicon acquisition lacks a stable current deployment")
    index_url = f"https://apps.razer.com/synapse/products/{product_id}/ui/index.html"
    index = next((row for row in receipt["files"] if row.get("url") == index_url), None)
    if not index or index.get("http_status") != 200 or not families.valid(index):
        raise ValueError("Current product favicon HTML identity does not verify")

    class FaviconLinks(HTMLParser):
        def __init__(self):
            super().__init__()
            self.links = []

        def handle_starttag(self, tag, attributes):
            attributes = dict(attributes)
            if tag == "link" and "icon" in (attributes.get("rel") or "").lower().split():
                self.links.append(attributes)

    parser = FaviconLinks()
    parser.feed((ROOT / index["path"]).read_text(encoding="utf-8"))
    if not parser.links or not parser.links[0].get("href"):
        raise ValueError("Current product HTML has no first favicon href")
    first = parser.links[0]
    source_url = urllib.parse.urljoin(index_url, first["href"])
    original = next((row for row in receipt["files"] if row.get("url") == source_url), None)
    if not original or original.get("http_status") != 200 or not families.valid(original):
        raise ValueError("Current first favicon original identity does not verify")
    proof = {"product_id": product_id, "acquisition": families.identity(receipt_path),
             "index": {**families.identity(ROOT / index["path"]), "url": index_url},
             "first_favicon_element": first,
             "original": {**families.identity(ROOT / original["path"]), "url": source_url}}
    cache[key] = proof
    return proof


def resource_ownership(pool):
    import analysis
    manifest_path = ROOT / "assets/synapse/manifests/manifest.json"
    manifest = json.loads(manifest_path.read_bytes())
    registered = registered_resource_products()
    aliases = {}
    mappings = resource_sources.load_resource_mappings(root=ROOT)
    for row in mappings["selectors"]["dashboard_plugin_image_selector"]:
        aliases.setdefault(row["key"], set()).add(row["product_id"])
    previews_path = analysis.builtin_data_path("razer-pages", "KEYBOARD_PREVIEW_CURRENT_DATA")
    for row in json.loads(previews_path.read_bytes())["products"]:
        for image in row["images"]:
            aliases.setdefault(image["asset"], set()).add(row["product_id"])
    picker_path = analysis.builtin_data_path("razer-app-pages", "APP_PICKER_ICONS_DATA")
    if picker_path.is_file():
        for product, row in json.loads(picker_path.read_bytes())["products"].items():
            aliases.setdefault(row["asset"], set()).add(int(product))
    favicon_aliases = {}
    favicon_metadata = json.loads(analysis.builtin_data_path("razer-shell", "HOST_DEVICE_FAVICONS").read_bytes())
    for product, alias in favicon_metadata.items():
        if alias and int(product) in registered:
            favicon_aliases.setdefault(alias, set()).add(int(product))
            aliases.setdefault(alias, set()).add(int(product))
    favicon_cache = {}
    receipts, originals, owners = {}, {}, {}
    actual_outputs = {relative(path): key for key, paths in pool.items() for path in paths}
    for key, paths in pool.items():
        owners[key] = {"products": set(), "declared_products": set(), "evidence": [], "unverified": [], "conflicts": []}
    for row in json.loads((ROOT / "assets/synapse/indexes/resource-layout.json").read_bytes())["resources"]:
        key = (row["sha256"], Path(row["physical_path"]).suffix.lower())
        if key in owners and row["physical_path"] in {relative(path) for path in pool[key]}:
            owners[key]["declared_products"].update(pid for pid in row.get("source_products", []) if pid in registered)
    for ordinal, row in enumerate(manifest["entries"]):
        output = row.get("output", "")
        key = actual_outputs.get(output)
        if key is None:
            continue
        state = owners[key]
        source = row.get("source", "")
        reason = None if row.get("sha256") == key[0] else "Prepared output bytes differ from the provenance hash"
        if not source.startswith(".analysis/inputs/") or not row.get("acquisition", "").startswith(".analysis/inputs/"):
            reason = "No retained current acquisition for this provenance record"
        else:
            receipt_name = row["acquisition"]
            if receipt_name not in receipts:
                receipt_path = safe(ROOT / receipt_name)
                receipt = json.loads(receipt_path.read_bytes()) if receipt_path.is_file() else {}
                receipts[receipt_name] = (receipt, {item.get("path"): item for item in receipt.get("files", [])})
            receipt, indexed = receipts[receipt_name]
            original = indexed.get(source)
            if source not in originals:
                path = safe(ROOT / source)
                originals[source] = {"sha256": digest(path), "bytes": path.stat().st_size} if path.is_file() else {}
            if (not receipt.get("source_current_during_acquisition") or not original
                    or original.get("http_status") != 200 or original.get("url") != row.get("source_url")
                    or original.get("sha256") != row.get("source_sha256")
                    or original.get("bytes") != row.get("source_bytes")
                    or originals[source] != {"sha256": row.get("source_sha256"), "bytes": row.get("source_bytes")}):
                reason = "Current acquisition/source identity mismatch"
        products = {int(value) for value in re.findall(r'https://apps\.razer\.com/synapse/products/(\d+)/', row.get("source_url", ""))}
        metadata_products = set().union(*(aliases.get(alias, set()) for alias in row.get("display_paths", [])))
        expected_favicon_products = set().union(*(favicon_aliases.get(alias, set())
                                                   for alias in row.get("display_paths", [])))
        if expected_favicon_products:
            try:
                retained = row.get("host_favicon_sources", [])
                if {item["product_id"] for item in retained} != expected_favicon_products:
                    raise ValueError("First favicon provenance does not cover every alias owner")
                for item in retained:
                    current = current_product_favicon(item["product_id"], item["acquisition"]["path"], favicon_cache)
                    if current != item or (ROOT / item["original"]["path"]).read_bytes() != (ROOT / source).read_bytes():
                        raise ValueError("Current first favicon input or product ownership changed")
                source_bytes = (ROOT / source).read_bytes()
                prepared_bytes = (ROOT / output).read_bytes()
                if prepared_bytes != source_bytes:
                    if (source_bytes.count(b"\r") or source_bytes.replace(b"\n", b"\r\n") != prepared_bytes
                            or row.get("transformation", {}).get("kind") != "source_lf_to_prepared_crlf"):
                        raise ValueError("Favicon prepared bytes differ beyond the recorded newline transform")
            except (KeyError, OSError, ValueError) as error:
                reason = str(error)
        if products and metadata_products and products != metadata_products:
            reason = "Source endpoint and explicit alias product metadata disagree"
            state["conflicts"].append({"manifest_ordinal": ordinal, "source_products": sorted(products),
                                       "alias_products": sorted(metadata_products)})
        products |= metadata_products
        state["declared_products"].update(products & registered)
        if not products or not products <= registered:
            reason = reason or "No unambiguous registered product ownership"
        if reason:
            state["unverified"].append({"manifest_ordinal": ordinal, "output": output, "reason": reason})
        else:
            state["products"].update(products)
            state["evidence"].append({"manifest_ordinal": ordinal, "source": source,
                                      "source_url": row["source_url"], "source_sha256": row["source_sha256"],
                                      "acquisition": row["acquisition"], "products": sorted(products),
                                      "display_paths": row.get("display_paths", [])})
    for state in owners.values():
        state["products"] = sorted(state["products"])
        state["declared_products"] = sorted(state["declared_products"])
    return owners


def resource_layout_changes(pool, ownership, mapping, creations, changes, configurations):
    layout_name = "assets/synapse/indexes/resource-layout.json"
    previous = json.loads((ROOT / layout_name).read_bytes())
    actual_paths = {relative(path) for paths in pool.values() for path in paths}
    ownership_identity = hashlib.sha256(json.dumps([
        {"sha256": key[0], "extension": key[1], "products": state["products"],
         "declared_products": state["declared_products"], "evidence": state["evidence"],
         "unverified": [{name: value for name, value in row.items() if name != "output"} for row in state["unverified"]],
         "conflicts": state["conflicts"]} for key, state in sorted(ownership.items())
    ], sort_keys=True).encode("utf-8")).hexdigest()
    expected = {relative(path): (key[0], path.stat().st_size) for key, paths in pool.items() for path in paths}
    retained = {row["physical_path"]: (row["sha256"], row["bytes"]) for row in previous["resources"]}
    if (not mapping and not configurations and previous.get("ownership_audit_sha256") == ownership_identity
            and actual_paths <= retained.keys() and all(retained[name] == value for name, value in expected.items())):
        return
    historical = {}
    for row in previous["resources"]:
        target = mapping.get(row["physical_path"], row["physical_path"])
        historical.setdefault(target, []).append(row)
    manifest = json.loads((ROOT / "assets/synapse/manifests/manifest.json").read_bytes())
    aliases = {}
    for row in manifest["entries"]:
        target = mapping.get(row.get("output"), row.get("output"))
        aliases.setdefault(target, []).extend(row.get("display_paths", []))
    resources, usage = [], {}
    for key, paths in sorted(pool.items()):
        state = ownership[key]
        for target in sorted({mapping.get(relative(path), relative(path)) for path in paths}):
            old = historical.get(target, [])
            products = sorted(set(state["products"]) | set(state["declared_products"])
                              | {pid for row in old for pid in row.get("source_products", [])})
            current = sorted(state["products"])
            runtime_keys = list(dict.fromkeys([alias for row in old for alias in row.get("runtime_keys", [])] + aliases.get(target, [])))
            row = {"sha256": key[0], "bytes": paths[0].stat().st_size, "physical_path": target,
                   "original_paths": list(dict.fromkeys([name for row in old for name in row.get("original_paths", [])]
                                                        + [relative(path) for path in paths])),
                   "runtime_keys": runtime_keys, "source_products": products, "current_source_products": current,
                   "historical_current_source_products": sorted({pid for row in old for pid in
                       [*row.get("current_source_products", []), *row.get("historical_current_source_products", [])]}),
                   "ownership_status": "current_source_evidence" if state["products"] and not state["unverified"]
                       else "requires_current_source_audit",
                   "evidence_documents": list(dict.fromkeys([name for row in old for name in row.get("evidence_documents", [])]
                                                             + (["assets/synapse/manifests/manifest.json"] if state["evidence"] else []))),
                   "current_manifest_ordinals": [item["manifest_ordinal"] for item in state["evidence"]]}
            resources.append(row)
            for product in products:
                usage.setdefault(str(product), []).append({"physical_path": target, "runtime_keys": runtime_keys,
                                                          "current_source_evidence": product in current})
    included = {row["physical_path"] for row in resources}
    for row in previous["resources"]:
        if row["physical_path"] in included:
            continue
        if row["physical_path"] in mapping:
            continue
        resources.append(row)
        for product in row.get("source_products", []):
            usage.setdefault(str(product), []).append({"physical_path": row["physical_path"],
                "runtime_keys": row.get("runtime_keys", []),
                "current_source_evidence": product in row.get("current_source_products", [])})
    for item in configurations:
        target = mapping.get(item["source"], item["source"])
        if target in {row["physical_path"] for row in resources}:
            continue
        product = item["product_id"]
        prepared = changes.get(item["source"])
        resources.append({"sha256": hashlib.sha256(prepared).hexdigest() if prepared is not None else item["sha256"],
                          "bytes": len(prepared) if prepared is not None else item["bytes"], "physical_path": target,
                          "original_paths": [item["source"]], "runtime_keys": [], "source_products": [product],
                          "current_source_products": [], "ownership_status": "requires_current_source_audit",
                          "evidence_documents": [], "ownership_basis": item["ownership_basis"]})
        usage.setdefault(str(product), []).append({"physical_path": target, "runtime_keys": [], "current_source_evidence": False})
    resources.sort(key=lambda row: row["physical_path"])
    removed_bytes = sum((ROOT / name).stat().st_size for name in mapping) - sum((ROOT / name).stat().st_size for name in creations.values())
    layout = {**previous, "resources": resources, "product_usage": usage, "ownership_audit_sha256": ownership_identity,
              "summary": {"before_files": sum(len(paths) for paths in pool.values()), "physical_files": len(resources),
                          "duplicate_files_removed": len(mapping) - len(creations), "redundant_bytes_removed": removed_bytes,
                          "source_products": len(usage), "requires_current_source_audit": sum(
                              row["ownership_status"] != "current_source_evidence" for row in resources)},
              "cumulative_deduplication": {"files": previous.get("cumulative_deduplication", {}).get("files", 0) + len(mapping) - len(creations),
                                           "bytes": previous.get("cumulative_deduplication", {}).get("bytes", 0) + removed_bytes}}
    changes[layout_name] = (json.dumps(layout, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    for product, rows in usage.items():
        name = f"assets/synapse/products/{product}/resources.json"
        row = {"product_id": int(product), "generated_from": layout_name, "resources": rows,
               "limits": "Source ownership and current-source evidence are separate; this is not actual page-mount acceptance. Shared files are referenced, never copied."}
        changes[name] = (json.dumps(row, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    for name in list(changes):
        path = ROOT / name
        if path.is_file() and path.read_bytes() == changes[name]:
            del changes[name]


def resource_deduplication_plan(*, report_path=None):
    groups, proposed, creations, ownership_unknown = [], {}, {}, []
    pool = resource_payload_index()
    ownership = resource_ownership(pool)
    print(json.dumps({"resource_layout_phase": "ownership_classified", "payload_groups": len(pool)}), flush=True)
    for (identity, extension), paths in sorted(pool.items()):
        content = paths[0].read_bytes()
        if any(path.read_bytes() != content for path in paths[1:]):
            raise ValueError("Resource hash collision or concurrent modification")
        members = [relative(path) for path in paths]
        state = ownership[(identity, extension)]
        products = sorted(set(state["products"]) | set(state["declared_products"]))
        if products and not state["conflicts"]:
            folder = f"products/{products[0]}" if len(products) == 1 else "shared/products"
            suitable = [name for name in members if name.startswith("assets/synapse/" + folder + "/")]
            canonical = min(suitable) if suitable else f"assets/synapse/{folder}/{identity}{extension}"
        else:
            ownership_unknown.append({"sha256": identity, "members": members, **state})
            canonical = f"assets/synapse/shared/source-unverified/{identity}{extension}"
        if state["unverified"] and products and not state["conflicts"]:
            ownership_unknown.append({"sha256": identity, "members": members, **state})
        if canonical not in members:
            creations[canonical] = members[0]
        groups.append({"sha256": identity, "bytes": len(content), "extension": extension,
                       "canonical": canonical, "members": members, "ownership": state})
        proposed.update({member: canonical for member in members if member != canonical})
    configuration_mapping, configuration_records = resource_configuration_plan()
    proposed.update(configuration_mapping)
    creations.update({target: source for source, target in configuration_mapping.items()})
    excluded = {report_path} if report_path is not None else set()
    initial_changes, all_references, blockers, initial_snapshots = resource_reference_plan(proposed, excluded=excluded)
    print(json.dumps({"resource_layout_phase": "references_classified", "references": len(all_references),
                      "unknown_references": len(blockers)}), flush=True)
    blocked = {row["resource"] for row in blockers}
    blocked.update(member for group in groups if group["ownership"]["conflicts"]
                   for member in group["members"] if member in proposed)
    for group in groups:
        if any(member in blocked for member in group["members"]):
            blocked.update(member for member in group["members"] if member in proposed)
    ready = {name: canonical for name, canonical in proposed.items() if name not in blocked}
    creations = {target: source for target, source in creations.items() if target in ready.values()}
    if ready == proposed:
        changes, references, unexpected, snapshots = initial_changes, all_references, [], initial_snapshots
    else:
        changes, references, unexpected, snapshots = resource_reference_plan(ready, excluded=excluded)
    if unexpected:
        raise ValueError("Reference classification changed during planning")
    resource_layout_changes(pool, ownership, ready, creations, changes, configuration_records)
    logical = []
    for name, rewritten in changes.items():
        if not name.endswith(".rs"):
            continue
        path = ROOT / name
        before = list(rust_embed_references(path.read_text(encoding="utf-8-sig")))
        after = list(rust_embed_references(rewritten.decode("utf-8-sig")))
        if len(before) != len(after):
            raise ValueError("Resource include order/count changed")
        for ordinal, (old, new) in enumerate(zip(before, after)):
            if old[0] != new[0]:
                raise ValueError("Resource include kind changed")
            if old == new:
                continue
            old_path, new_path = safe(path.parent / old[1]), safe(path.parent / new[1])
            comparison = ROOT / creations.get(relative(new_path), relative(new_path))
            if old_path.read_bytes() != comparison.read_bytes():
                raise ValueError("Resource include payload changed")
            logical.append({"owner": name, "ordinal": ordinal, "kind": old[0], "sha256": digest(old_path)})
    report = {"schema_version": 1, "mode": "dry-run", "groups": groups,
              "mapping": ready, "creations": creations, "ownership_unknown": ownership_unknown,
              "configuration_records": configuration_records,
              "blocked_mapping": {name: proposed[name] for name in sorted(blocked)},
              "references": all_references, "blockers": blockers, "snapshot": snapshots,
              "provenance_discrepancies": [row for row in all_references if "provenance_sha256_discrepancy" in row],
              "rewrites": [{"path": name, "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}
                           for name, raw in sorted(changes.items())],
              "verified_ordered_include_payloads": logical,
              "regenerate_after_apply": ["audit-asset-pack --prepare", "prepare-asset-pack", "verify-asset-pack",
                                         "verify-resources", "affected family audits", "validate-rust"],
              "limits": ["Byte sharing does not establish shared product semantics or current source provenance",
                         "Historical original_paths and separate per-source provenance records are preserved",
                         "Unknown references block their corresponding deletion; original inputs and fixtures are not deduplicated",
                         "No Git index, history or LFS mutation; no vendor or application execution"],
              "summary": {"duplicate_groups": sum(len(paths) > 1 for paths in pool.values()),
                          "duplicate_files": sum(len(paths) - 1 for paths in pool.values()),
                          "redundant_bytes": sum(paths[0].stat().st_size * (len(paths) - 1) for paths in pool.values()),
                          "verified_ownership_groups": sum(bool(group["ownership"]["products"]) for group in groups),
                          "unknown_ownership_groups": len(ownership_unknown),
                          "ownership_conflict_groups": sum(bool(group["ownership"]["conflicts"]) for group in groups),
                          "new_canonical_files": len(creations),
                          "configuration_candidates": len(configuration_mapping),
                          "safe_deletions": len(ready), "blocked_deletions": len(blocked),
                          "safe_redundant_bytes": sum((ROOT / name).stat().st_size for name in ready)
                              - sum((ROOT / name).stat().st_size for name in creations.values()),
                          "rewritten_files": len(changes), "reference_records": len(all_references)}}
    return report, changes


def deduplicate_resources(*, apply=False, plan=".analysis/results/resource-deduplication.json", prepare_and_apply=False):
    import analysis
    report_path = safe(ROOT / plan)
    if not report_path.is_relative_to(ROOT / ".analysis/results"):
        raise ValueError("Deduplication report must be under .analysis/results")
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        report, changes = resource_deduplication_plan(report_path=report_path)
        if not apply and not prepare_and_apply:
            write_json(report_path, report)
            return report
        approved = report if prepare_and_apply else json.loads(report_path.read_bytes())
        if approved != report:
            raise ValueError("Dry-run snapshot changed; generate and review a new dry-run before apply")
        if prepare_and_apply:
            write_json(report_path, report)
        before = {name: (ROOT / name).read_bytes() if (ROOT / name).is_file() else None for name in changes}
        removed = {}
        created = []
        try:
            for target, source in report["creations"].items():
                path = safe(ROOT / target)
                if path.exists():
                    raise ValueError("Planned resource destination already exists")
                path.parent.mkdir(parents=True, exist_ok=True)
                with path.open("xb") as stream:
                    created.append(path)
                    stream.write((ROOT / source).read_bytes())
                    stream.flush()
                    os.fsync(stream.fileno())
            for name, raw in changes.items():
                destination = safe(ROOT / report["mapping"].get(name, name))
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(raw)
            for name, canonical in report["mapping"].items():
                path = safe(ROOT / name)
                original = path.read_bytes()
                expected = changes.get(name, original)
                if (not path.is_relative_to(ROOT / "assets/synapse")
                        or (name in before and original != before[name])
                        or expected != (ROOT / canonical).read_bytes()):
                    raise ValueError("Duplicate identity changed before removal: " + name)
                removed[name] = path.read_bytes()
                path.unlink()
            _, _, unresolved, _ = resource_reference_plan(report["mapping"], excluded={report_path})
            if unresolved:
                raise ValueError("Unresolved resource references after migration")
            rust_inputs(include_asset_pack=False)
        except BaseException:
            for name, raw in removed.items():
                (ROOT / name).write_bytes(raw)
            for name, raw in before.items():
                if raw is None:
                    (ROOT / name).unlink(missing_ok=True)
                else:
                    (ROOT / name).write_bytes(raw)
            for path in created:
                path.unlink()
            raise
        report["mode"] = "applied"
        report["applied_at_utc"] = datetime.now(timezone.utc).isoformat()
        write_json(report_path, report)
        return report


def resource_payload_index():
    groups = {}
    for folder in ("products", "shared"):
        for path in sorted((ROOT / "assets/synapse" / folder).rglob("*")):
            if path.is_file() and path.suffix.lower() in IMAGE_RESOURCE_EXTENSIONS:
                if safe(path) != path or path.is_symlink():
                    raise ValueError(f"Resource path redirects: {path}")
                groups.setdefault((digest(path), path.suffix.lower()), []).append(path)
    for paths in groups.values():
        paths.sort(key=lambda path: (not path.is_relative_to(ROOT / "assets/synapse/shared"), relative(path)))
    return groups


def shared_resource_output(content, extension, pool, *, product_id=None):
    if extension not in IMAGE_RESOURCE_EXTENSIONS:
        raise ValueError(f"Unsupported prepared resource extension: {extension}")
    identity = hashlib.sha256(content).hexdigest()
    key = (identity, extension)
    for candidate in pool.get(key, []):
        if candidate.read_bytes() != content:
            raise ValueError(f"Resource content pool changed: {relative(candidate)}")
        return candidate
    folder = f"products/{product_id}" if product_id is not None else "shared/source-unverified"
    output = safe(ROOT / "assets/synapse" / folder / (identity + extension))
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        if output.read_bytes() != content:
            raise ValueError(f"Resource output collision: {relative(output)}")
    else:
        descriptor, temporary = tempfile.mkstemp(prefix=".resource-", dir=output.parent)
        try:
            with os.fdopen(descriptor, "wb") as stream:
                stream.write(content)
            Path(temporary).replace(output)
        finally:
            Path(temporary).unlink(missing_ok=True)
    pool[key] = [output]
    return output


def prepare_locale_keys(acquisition, namespace, bindings, keys):
    """Prepare scoped UI strings from verified current literal JS dictionaries."""
    import evidence
    if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", namespace):
        raise ValueError("Invalid locale namespace")
    receipt_path = safe(ROOT / acquisition)
    if not receipt_path.is_relative_to(ROOT / ".analysis/inputs"):
        raise ValueError("Locale acquisition must be a retained current input")
    receipt = json.loads(receipt_path.read_bytes())
    if not receipt.get("source_current_during_acquisition"):
        raise ValueError("Locale acquisition has no deployment-stability proof")
    originals = {row.get("path"): row for row in receipt.get("files", [])}
    prepared = []
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        for locale, source_name in bindings.items():
            if not re.fullmatch(r"[A-Za-z]+(?:-[A-Za-z]+)*", locale):
                raise ValueError("Invalid locale name")
            source = safe(ROOT / source_name)
            if not source.is_relative_to(ROOT / ".analysis/inputs"):
                raise ValueError("Locale source is outside retained originals")
            original = originals.get(relative(source))
            raw = source.read_bytes()
            if (not original or original.get("http_status") != 200
                    or original.get("sha256") != hashlib.sha256(raw).hexdigest()
                    or original.get("bytes") != len(raw)):
                raise ValueError("Locale original identity does not verify: " + source_name)
            selected = evidence.locale_keys(raw, keys)
            destination = ROOT / "locales" / (locale + ".json")
            before = destination.read_bytes()
            dictionary = json.loads(before)
            dictionary[namespace] = selected["values"]
            output = json.dumps(dictionary, ensure_ascii=False, indent=2) + "\n"
            if b"\r\n" in before:
                output = output.replace("\n", "\r\n")
            prepared.append({"locale": locale, "namespace": namespace,
                "acquisition": acquisition, "source": original, "dictionary": selected,
                "destination": relative(destination), "before_sha256": hashlib.sha256(before).hexdigest(),
                "_before": before, "_output": output.encode("utf-8")})
        # Validate the whole batch before changing any product locale.
        for row in prepared:
            if (ROOT / row["destination"]).read_bytes() != row["_before"]:
                raise ValueError("Locale changed during preparation")
            if digest(ROOT / row["source"]["path"]) != row["source"]["sha256"]:
                raise ValueError("Locale original changed during preparation")
        for row in prepared:
            (ROOT / row["destination"]).write_bytes(row.pop("_output"))
            row.pop("_before")
            row["output"] = {"path": row["destination"], "sha256": digest(ROOT / row["destination"]),
                             "bytes": (ROOT / row["destination"]).stat().st_size}
        report_path = ROOT / ".analysis/results/locale-preparation.json"
        previous = json.loads(report_path.read_bytes()) if report_path.is_file() else {"entries": []}
        previous["entries"] = [row for row in previous["entries"] if row.get("namespace") != namespace] + prepared
        previous.update(recorded_at_utc=datetime.now(timezone.utc).isoformat(),
            tool={"path": "tools/workspace.py", "sha256": digest(Path(__file__))},
            static_only=True, vendor_javascript_executed=False)
        report_path.write_text(json.dumps(previous, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return {"report": relative(report_path), "namespace": namespace, "locales": len(prepared), "keys": keys}


def prepare_image_analyzer():
    """Acquire a hash-verified Pillow wheel matching the current Python ABI."""
    import csv
    import io
    import sysconfig
    import zipfile
    import analysis
    version = "12.3.0"
    abi = f"cp{sys.version_info.major}{sys.version_info.minor}"
    platform_tag = sysconfig.get_platform().replace("-", "_").replace(".", "_")
    filename = f"pillow-{version}-{abi}-{abi}-{platform_tag}.whl"
    metadata_url = f"https://pypi.org/pypi/pillow/{version}/json"
    reference = ROOT / ".analysis/references/python-static-wheels"
    reference.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(metadata_url, timeout=40) as response:
        metadata_bytes = response.read()
    metadata = json.loads(metadata_bytes)
    artifact = next((item for item in metadata["urls"] if item["filename"] == filename), None)
    if artifact is None:
        raise ValueError("Pillow has no verified wheel matching the current analyzer Python: " + filename)
    metadata_path = reference / f"pillow-{version}.{abi}.pypi.json"
    metadata_path.write_bytes(metadata_bytes)
    wheel_path = reference / filename
    if not wheel_path.is_file() or digest(wheel_path) != artifact["digests"]["sha256"]:
        with urllib.request.urlopen(artifact["url"], timeout=40) as response:
            wheel_bytes = response.read()
        if hashlib.sha256(wheel_bytes).hexdigest() != artifact["digests"]["sha256"]:
            raise ValueError("Pillow wheel differs from its published SHA-256")
        wheel_path.write_bytes(wheel_bytes)
    destination = ROOT / "tools/analyzers/pillow"
    records = []
    with zipfile.ZipFile(wheel_path) as archive:
        record_name = f"pillow-{version}.dist-info/RECORD"
        members = list(csv.reader(io.StringIO(archive.read(record_name).decode("utf-8"))))
        for name, encoded_digest, length in members:
            output = safe(destination / name)
            if not output.is_relative_to(destination.resolve()):
                raise ValueError("Pillow wheel member escaped its analyzer directory")
            content = archive.read(name)
            if encoded_digest:
                algorithm, expected = encoded_digest.split("=", 1)
                actual = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode("ascii")
                if algorithm != "sha256" or actual != expected or len(content) != int(length):
                    raise ValueError("Pillow wheel RECORD does not verify: " + name)
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(content)
            records.append({"path": relative(output), "bytes": len(content),
                            "sha256": digest(output), "member": name, "wheel": relative(wheel_path)})
    receipt = {"schema_version": 1, "acquired_at_utc": analysis.stamp(), "python_abi": abi + "-" + platform_tag,
        "python_version": sys.version.split()[0], "target_execution": False,
        "scope": "Project-local static image decoder only; no product runtime dependency",
        "wheels": [{"name": "pillow", "version": version, "path": relative(wheel_path), "url": artifact["url"],
                    "bytes": wheel_path.stat().st_size, "sha256": digest(wheel_path),
                    "metadata_path": relative(metadata_path), "metadata_url": metadata_url,
                    "metadata_sha256": digest(metadata_path)}], "files": records,
        "record_verification": {"members": len(records), "all_record_members_verified": True,
                                "all_extracted_bytes_match": True}}
    write_json(destination / "acquisition.json", receipt)
    print(json.dumps({"receipt": relative(destination / "acquisition.json"), "python_abi": receipt["python_abi"],
                      "files": len(records)}), flush=True)


def verified_wheel_members(archive, record_name):
    """Verify every wheel member before publishing any analyzer files."""
    import csv
    import io
    names = [item.filename for item in archive.infolist() if not item.is_dir()]
    if len(names) != len(set(names)):
        raise ValueError("Analyzer wheel has duplicate members")
    records = list(csv.reader(io.StringIO(archive.read(record_name).decode("utf-8"))))
    listed = []
    for row in records:
        if len(row) != 3:
            raise ValueError("Analyzer wheel RECORD has an invalid row")
        name, encoded_digest, length = row
        if (not name or "\\" in name or PureWindowsPath(name).drive
                or name.startswith("/") or ".." in Path(name).parts):
            raise ValueError("Analyzer wheel member escaped its installation directory")
        listed.append(name)
        content = archive.read(name)
        if name == record_name:
            if encoded_digest or length:
                raise ValueError("Analyzer wheel RECORD must be unsigned")
            continue
        if not encoded_digest or not length:
            raise ValueError("Analyzer wheel member lacks a RECORD identity: " + name)
        algorithm, expected = encoded_digest.split("=", 1)
        actual = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode("ascii")
        if algorithm != "sha256" or actual != expected or len(content) != int(length):
            raise ValueError("Analyzer wheel RECORD does not verify: " + name)
        if archive.getinfo(name).external_attr >> 16 & 0o170000 == 0o120000:
            raise ValueError("Analyzer wheel contains a symbolic link")
    if len(listed) != len(set(listed)) or set(listed) != set(names):
        raise ValueError("Analyzer wheel RECORD does not cover every unique member")
    return listed


def prepare_media_analyzer():
    """Restore the pinned project-local PyAV decoder for this Python ABI."""
    import analysis
    import sysconfig
    import zipfile
    version = "16.0.1"
    abi = f"cp{sys.version_info.major}{sys.version_info.minor}"
    wheel_abi = abi + ("t" if sysconfig.get_config_var("Py_GIL_DISABLED") else "")
    platform_tag = sysconfig.get_platform().replace("-", "_").replace(".", "_")
    filename = f"av-{version}-{abi}-{wheel_abi}-{platform_tag}.whl"
    metadata_url = f"https://pypi.org/pypi/av/{version}/json"
    reference = safe(ROOT / ".analysis/references/python-static-wheels")
    destination = safe(ROOT / "tools/analyzers/media-python")
    reference.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(metadata_url, timeout=40) as response:
        metadata_bytes = response.read()
    metadata = json.loads(metadata_bytes)
    if metadata["info"]["version"] != version or metadata["info"]["name"].lower() != "av":
        raise ValueError("PyAV metadata does not match the pinned analyzer")
    artifact = next((item for item in metadata["urls"]
                     if item["filename"] == filename and item["packagetype"] == "bdist_wheel"), None)
    if artifact is None:
        raise ValueError("Pinned PyAV has no wheel for the active Python ABI: " + filename)
    metadata_path = reference / f"av-{version}.{hashlib.sha256(metadata_bytes).hexdigest()}.pypi.json"
    metadata_path.write_bytes(metadata_bytes)
    wheel_path = reference / filename
    if not wheel_path.is_file() or digest(wheel_path) != artifact["digests"]["sha256"]:
        with urllib.request.urlopen(artifact["url"], timeout=90) as response:
            wheel_bytes = response.read()
        if len(wheel_bytes) != artifact["size"] or hashlib.sha256(wheel_bytes).hexdigest() != artifact["digests"]["sha256"]:
            raise ValueError("PyAV wheel differs from its published byte identity")
        wheel_path.write_bytes(wheel_bytes)
    if wheel_path.stat().st_size != artifact["size"]:
        raise ValueError("PyAV wheel length differs from its publisher metadata")
    records = []
    with zipfile.ZipFile(wheel_path) as archive:
        members = verified_wheel_members(archive, f"av-{version}.dist-info/RECORD")
        for name in members:
            output = safe(destination / name)
            if not output.is_relative_to(destination):
                raise ValueError("PyAV wheel member escaped its analyzer directory")
            content = archive.read(name)
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(content)
            records.append({"path": relative(output), "bytes": len(content), "sha256": digest(output),
                            "member": name, "wheel": relative(wheel_path)})
    sys.path.insert(0, str(destination))
    import av
    if av.__version__ != version or not Path(av.__file__).resolve().is_relative_to(destination):
        raise ValueError("Restored PyAV does not load from the verified analyzer directory")
    receipt = {"schema_version": 1, "acquired_at_utc": analysis.stamp(),
        "python_abi": wheel_abi + "-" + platform_tag, "python_version": sys.version.split()[0],
        "target_execution": False,
        "scope": "Project-local static media decoder and encoder; no vendor target execution or product runtime dependency",
        "tool": {"path": "tools/workspace.py", "sha256": digest(Path(__file__))},
        "wheels": [{"name": "av", "version": version, "path": relative(wheel_path), "url": artifact["url"],
                    "bytes": wheel_path.stat().st_size, "sha256": digest(wheel_path),
                    "metadata_path": relative(metadata_path), "metadata_url": metadata_url,
                    "metadata_sha256": digest(metadata_path)}], "files": records,
        "record_verification": {"members": len(records), "all_record_members_verified": True,
                                "all_extracted_bytes_match": True},
        "decoder_probe": {"pyav": av.__version__, "libraries": {name: list(value) for name, value in av.library_versions.items()}}}
    receipt_path = destination / "acquisition.json"
    write_json(receipt_path, receipt)
    analysis.index_files([
        {"path": relative(wheel_path), "bytes": wheel_path.stat().st_size, "sha256": digest(wheel_path),
         "url": artifact["url"], "kind": "static_analyzer_wheel"},
        {"path": relative(metadata_path), "bytes": len(metadata_bytes), "sha256": digest(metadata_path),
         "url": metadata_url, "kind": "static_analyzer_metadata"}])
    return {"receipt": relative(receipt_path), "python_abi": receipt["python_abi"], "files": len(records),
            "pyav": av.__version__}


def prepare_native_cursors(sources, group_id=None):
    """Extract byte-verified PE cursor groups into analysis-owned CUR evidence."""
    import analysis
    import host
    if not isinstance(sources, list) or not sources:
        raise ValueError("Native cursor sources must be a nonempty ordered list")
    input_root = (ROOT / ".analysis/inputs").resolve()
    destination = input_root / "native-resources"
    files, derivations, inputs, snapshots = {}, [], [], []
    for source in sources:
        if type(source) is not dict or set(source) != {"path", "acquisition"}:
            raise ValueError("Native cursor source requires a path and acquisition receipt")
        paths = [safe(ROOT / source[key]) for key in ("path", "acquisition")]
        if any(not path.is_relative_to(input_root) or path.is_symlink() for path in paths):
            raise ValueError("Native cursor evidence must belong to analysis inputs")
        original_path, receipt_path = paths
        original_raw, receipt_raw = original_path.read_bytes(), receipt_path.read_bytes()
        original = {"path": relative(original_path), "bytes": len(original_raw),
                    "sha256": hashlib.sha256(original_raw).hexdigest()}
        acquisition = {"path": relative(receipt_path), "bytes": len(receipt_raw),
                       "sha256": hashlib.sha256(receipt_raw).hexdigest()}
        receipt = json.loads(receipt_raw)
        declared = receipt.get("files", []) + receipt.get("resources", [])
        if not any(all(row.get(key) == original[key] for key in original) for row in declared):
            raise ValueError("Native cursor original differs from its acquisition receipt")
        snapshots.extend(((original_path, original_raw), (receipt_path, receipt_raw)))
        inputs.append({"original": original, "acquisition": acquisition})
        groups = host.pe_cursor_groups(original_raw, group_id)
        if not groups:
            raise ValueError("Native cursor source contains no supported cursor group")
        for group in groups:
            raw = group.pop("data")
            digest_value = hashlib.sha256(raw).hexdigest()
            path = destination / (digest_value + ".cur")
            path.parent.mkdir(parents=True, exist_ok=True)
            if path.exists():
                if path.read_bytes() != raw:
                    raise ValueError("Prepared native cursor has conflicting bytes")
            else:
                temporary = path.with_suffix(".cur.partial")
                temporary.write_bytes(raw)
                os.replace(temporary, path)
            output = {"path": relative(path), "bytes": path.stat().st_size, "sha256": digest(path)}
            if output["sha256"] != digest_value or output["bytes"] != len(raw):
                raise ValueError("Prepared native cursor changed during publication")
            files[output["path"]] = output
            derivations.append({"original": original, "acquisition": acquisition, **group, "output": output})
    if any(path.read_bytes() != raw for path, raw in snapshots):
        raise ValueError("Native cursor original or acquisition changed during extraction")
    receipt_path = destination / "acquisition.json"
    receipt = {"schema_version": 1, "recorded_at_utc": analysis.stamp(),
               "tool": {"path": "tools/workspace.py", "sha256": digest(Path(__file__))},
               "pe_tool": {"path": "tools/host.py", "sha256": digest(ROOT / "tools/host.py")},
               "python_version": sys.version.split()[0], "inputs": inputs,
               "files": list(files.values()), "derivations": derivations,
               "input_bytes_unchanged": True, "target_execution": False,
               "conversion": "RT_GROUP_CURSOR order and original RT_CURSOR hotspots; standard CUR directory, original DIB and AND-mask bytes unchanged",
               "limitations": "Uncompressed BITMAPINFOHEADER frames only; absent/ambiguous language, invalid lengths, palette, bounds or hotspots fail explicitly"}
    write_json(receipt_path, receipt)
    analysis.index_files([{**row, "kind": "prepared_native_cursor", "acquisition_receipt": relative(receipt_path)}
                          for row in files.values()] + [{"path": relative(receipt_path),
                          "bytes": receipt_path.stat().st_size, "sha256": digest(receipt_path), "kind": "acquisition_receipt"}])
    return {"receipt": relative(receipt_path), "originals": len(inputs), "groups": len(derivations),
            "unique_files": len(files), "files": list(files.values()), "target_execution": False}


def prepare_resources(acquisition, resources=None):
    import analysis
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        return prepare_resources_locked(acquisition, resources)


def prepare_resources_locked(acquisition, resources=None):
    """Prepare verified original media and maintain explicit display mappings.

    Resource mappings are original project-relative paths to display aliases.
    Exact shared bytes may reuse a product output after its actual hash matches.
    AVIF is decoded statically to RGBA PNG; both byte hashes and codec versions
    remain in the provenance manifest, separately from the original acquisition.
    """
    import analysis
    groups = acquisition if isinstance(acquisition, list) else [{"acquisition": acquisition, "resources": resources}]
    contexts, requests = {}, []
    for group in groups:
        acquired = group["acquisition"]
        if acquired not in contexts:
            receipt_path = safe(ROOT / acquired)
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            contexts[acquired] = (receipt_path, receipt, {row.get("path"): row for row in receipt.get("files", [])})
        for source_name, values in group["resources"].items():
            for request in (values if isinstance(values, list) else [values]):
                request = request if isinstance(request, dict) else {"display": request}
                requests.append((source_name, {**request, "_acquisition": acquired}))
    manifest_path = ROOT / "assets/synapse/manifests/manifest.json"
    mappings = resource_sources.load_resource_mappings(root=ROOT)
    aliases = resource_sources.resource_table("ASSETS", mappings=mappings)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    content_pool = resource_payload_index()
    registered_products = registered_resource_products()
    favicon_cache = {}
    favicon_metadata = json.loads(analysis.builtin_data_path("razer-shell", "HOST_DEVICE_FAVICONS").read_bytes())
    prepared = []
    expanded = []
    muted_animation_cache = {}
    verified_svg_producers = {}
    for source_name, request in requests:
        if isinstance(request, dict) and "svg_button_states" in request:
            expanded.append((source_name, {**request, "svg_button_state": "base", "svg_button_index": None}))
            for layer in request["svg_button_states"]:
                expanded.append((source_name, {**request, "display": layer["display"],
                                              "svg_button_state": layer["state"],
                                              "svg_button_index": layer["button_index"]}))
        elif isinstance(request, dict) and "svg_smil_frames" in request:
            count = request["svg_smil_frames"]
            duration = request["svg_smil_duration_seconds"]
            if type(count) is not int or not 2 <= count <= 600 or not 0 < duration <= 60:
                raise ValueError("Invalid source SMIL frame sequence")
            for frame in range(count):
                expanded.append((source_name, {**request, "display": request["display"] + f"-{frame:03}.svg",
                                               "svg_smil_seconds": frame * duration / count}))
        else:
            expanded.append((source_name, request))
    for source_name, request in expanded:
        receipt_path, receipt, original_rows = contexts[request["_acquisition"]]
        request = request if isinstance(request, dict) else {"display": request}
        display = request["display"]
        source = safe(ROOT / source_name)
        if not source.is_relative_to(ROOT / ".analysis/inputs"):
            raise ValueError("Resource must come from retained current original inputs")
        row = original_rows.get(relative(source))
        source_hash = digest(source)
        if (not row or row.get("http_status") != 200
                or row.get("sha256") != source_hash
                or row.get("bytes") != source.stat().st_size):
            raise ValueError(f"Resource acquisition bytes do not verify: {relative(source)}")
        if not re.fullmatch(r"synapse/[\w./-]+", display):
            raise ValueError("Invalid product display alias")
        content = source.read_bytes()
        conversion = None
        transformation = None
        favicon_sources = []
        if "host_favicon_sources" in request:
            if source.suffix.lower() != ".svg" or not receipt.get("source_current_during_acquisition"):
                raise ValueError("Host favicon preparation requires current acquired SVG originals")
            for locator in request["host_favicon_sources"]:
                proof = current_product_favicon(locator["product_id"], locator["acquisition"], favicon_cache)
                if (ROOT / proof["original"]["path"]).read_bytes() != content:
                    raise ValueError("Products sharing a favicon alias have different current SVG bytes")
                favicon_sources.append(proof)
            expected = {int(pid) for pid, alias in favicon_metadata.items()
                        if alias == display and int(pid) in registered_products}
            observed = [item["product_id"] for item in favicon_sources]
            if not expected or set(observed) != expected or len(observed) != len(expected):
                raise ValueError("Current first favicon sources do not cover the exact registered alias owners")
            retained = asset_pack_identity(request["retain_prepared_favicon"])
            if not any(retained.is_relative_to(ROOT / "assets/synapse" / folder) for folder in ("shared", "products")):
                raise ValueError("Retained favicon output is outside product resources")
            prepared_content = retained.read_bytes()
            if content != prepared_content:
                if content.count(b"\r") or content.replace(b"\n", b"\r\n") != prepared_content:
                    raise ValueError("Existing favicon differs beyond exact source LF to prepared CRLF conversion")
                transformation = {
                    "kind": "source_lf_to_prepared_crlf", "tool": "Python bytes.replace", "version": sys.version.split()[0],
                    "operation": "Replace source byte 0A with 0D 0A; every other byte retained; original contains no 0D bytes",
                    "source_lf_count": content.count(b"\n"), "prepared_crlf_count": prepared_content.count(b"\r\n"),
                    "source_bytes": len(content), "source_sha256": source_hash,
                    "prepared_bytes": len(prepared_content), "prepared_sha256": hashlib.sha256(prepared_content).hexdigest(),
                    "inverse_operation": "Replace prepared byte sequence 0D 0A with 0A",
                    "inverse_restores_original_bytes": prepared_content.replace(b"\r\n", b"\n") == content,
                    "original_bytes_equal_prepared": False,
                }
                content = prepared_content
        if "svg_button_state" in request:
            import families
            if source.suffix.lower() != ".svg" or not receipt.get("source_current_during_acquisition"):
                raise ValueError("SVG button layers require stable current acquired originals")
            evidence = {}
            for role in ("producer", "css"):
                input_spec = request["svg_button_" + role]
                evidence_path = safe(ROOT / input_spec["path"])
                original = original_rows.get(relative(evidence_path))
                if (not original or original.get("http_status") != 200
                        or original.get("sha256") != input_spec["sha256"]
                        or digest(evidence_path) != input_spec["sha256"]
                        or original.get("bytes") != evidence_path.stat().st_size):
                    raise ValueError("Current SVG button " + role + " acquisition does not verify")
                evidence[role] = {"input": original, "text": evidence_path.read_text(encoding="utf-8")}
            retained_regions = []
            for region in request["svg_button_producer"]["complete_regions"]:
                text = evidence["producer"]["text"]
                begin, end = region["character_start"], region["character_end"]
                if not 0 <= begin < end <= len(text):
                    raise ValueError("Current SVG producer boundary is invalid")
                body = text[begin:end]
                if hashlib.sha256(body.encode("utf-8")).hexdigest() != region["body_sha256"]:
                    raise ValueError("Current complete SVG producer differs from its source")
                if region["role"] == "mounted_svg_button_component":
                    declaration = re.match(r"class [\w$]+ extends [\w.$]+\{", body)
                    if not declaration or families.javascript_delimited(body, declaration.end() - 1) != len(body):
                        raise ValueError("Current SVG producer component body is incomplete")
                    if not all(token in body for token in ("querySelectorAll(", '"is-active"', '"is-hover"',
                                                          '"is-disabled"', "dangerouslySetInnerHTML", "hyperShiftOn")):
                        raise ValueError("Current SVG button state propagation differs from the renderer")
                retained_regions.append({**region, "body": body})
            if {region["role"] for region in retained_regions} != {
                    "mounted_svg_button_component", "svg_button_helper_declarations"}:
                raise ValueError("Current SVG button producer/helper closure is incomplete")
            content, transformation = static_svg_button_layer(content, request["svg_button_index"],
                request["svg_button_state"], evidence["css"]["text"])
            transformation["producer"] = {"input": evidence["producer"]["input"], "complete_regions": retained_regions}
            transformation["source_css"] = evidence["css"]["input"]
        if "svg_smil_seconds" in request:
            if source.suffix.lower() != ".svg" or not receipt.get("source_current_during_acquisition"):
                raise ValueError("SMIL resource preparation requires a stable acquired SVG original")
            content, transformation = static_smil_svg(content, request["svg_smil_seconds"])
        if "inline_react_svg" in request:
            if source.suffix.lower() != ".js" or not receipt.get("source_current_during_acquisition"):
                raise ValueError("Inline SVG extraction requires a stable acquired JS original")
            content, transformation = static_react_svg(content.decode("utf-8"), request["inline_react_svg"])
        if "inline_jsx_svg" in request:
            if source.suffix.lower() != ".js" or not receipt.get("source_current_during_acquisition"):
                raise ValueError("Inline JSX SVG extraction requires a stable acquired JS original")
            content, transformation = static_jsx_svg(content.decode("utf-8"), request["inline_jsx_svg"])
        if "app_picker_category" in request:
            if source.suffix.lower() != ".js" or not receipt.get("source_current_during_acquisition"):
                raise ValueError("App menu category SVG preparation requires stable acquired JS originals")
            content, transformation = static_app_picker_category_svg(content.decode("utf-8"), request["app_picker_category"])
        if "inline_data_sha256" in request:
            import families
            if source.suffix.lower() != ".js":
                raise ValueError("Inline resource extraction requires an acquired JS original")
            original_text = content.decode("utf-8")
            matches = []
            for match in re.finditer(r'"data:image/png;base64,([A-Za-z0-9+/=]+)"', original_text):
                decoded = base64.b64decode(match[1], validate=True)
                if hashlib.sha256(decoded).hexdigest() != request["inline_data_sha256"]:
                    continue
                body = complete_resource_module(original_text, match[0])
                matches.append({"character_start": match.start(), "character_end": match.end(),
                                "complete_module": body})
                content = decoded
            if not matches:
                raise ValueError("Verified inline PNG literal is absent from the acquired source")
            if not content.startswith(b"\x89PNG\r\n\x1a\n"):
                raise ValueError("Inline image does not contain actual PNG bytes")
            transformation = {"tool": "Python base64", "version": sys.version.split()[0],
                              "operation": "Decode a complete literal source PNG data URL; no JavaScript evaluation",
                              "source_regions": matches}
        if "inline_css_png" in request:
            if source.suffix.lower() != ".css":
                raise ValueError("CSS bitmap extraction requires an acquired CSS original")
            bitmap = request["inline_css_png"]
            candidates = []
            for rule in re.finditer(r"([^{}]+)\{[^{}]*\}", content.decode("utf-8")):
                if rule[1].strip() != bitmap["selector"]:
                    continue
                for literal in re.finditer(r"data:image/png;base64,([A-Za-z0-9+/=]+)", rule[0]):
                    decoded = base64.b64decode(literal[1], validate=True)
                    if hashlib.sha256(decoded).hexdigest() == bitmap["sha256"]:
                        candidates.append((rule, decoded))
            if len(candidates) != 1 or not candidates[0][1].startswith(b"\x89PNG\r\n\x1a\n"):
                raise ValueError("Current CSS PNG source rule is absent or ambiguous")
            rule, content = candidates[0]
            transformation = {
                "tool": "Python base64", "version": sys.version.split()[0],
                "operation": "Decode the PNG data URL from the verified complete source CSS rule; no CSS execution",
                "complete_css_rules": [{"character_start": rule.start(), "character_end": rule.end(), "body": rule[0]}],
            }
        if "svg_remove_root_ids" in request:
            import families
            if source.suffix.lower() != ".svg":
                raise ValueError("Source SVG root filtering requires an SVG original")
            producer = request["producer"]
            producer_path = safe(ROOT / producer["path"])
            producer_row = original_rows.get(relative(producer_path))
            if (not producer_row or producer_row.get("http_status") != 200
                    or producer_row.get("sha256") != producer["sha256"]
                    or digest(producer_path) != producer["sha256"]
                    or producer_row.get("bytes") != producer_path.stat().st_size):
                raise ValueError("Source SVG producer acquisition does not verify")
            producer_text = producer_path.read_text(encoding="utf-8")
            producer_key = (relative(producer_path), producer["sha256"], producer["anchor"])
            body = verified_svg_producers.get(producer_key)
            if body is None:
                if producer.get("boundary_parser") == "tree-sitter-javascript" or not isinstance(producer["module_id"], int):
                    anchor_offset = producer_text.index(producer["anchor"])
                    owners = [module for module in families.keyboard_source_modules(producer_text)
                              if module["character_start"] <= anchor_offset < module["character_end"]]
                    if not owners:
                        raise ValueError("Current source SVG producer is outside a complete grammar function")
                    body = min(owners, key=lambda owner: owner["character_end"]-owner["character_start"])
                else:
                    body = families.complete_raw_module(producer_text, producer["anchor"])
                verified_svg_producers[producer_key] = body
            if body["module_id"] != producer["module_id"]:
                raise ValueError("Source SVG producer module does not match")
            root_ids = request["svg_remove_root_ids"]
            if (not root_ids or len(set(root_ids)) != len(root_ids)
                    or any(not isinstance(root_id, str) for root_id in root_ids)):
                raise ValueError("Source SVG root identifiers must be distinct strings")
            root = ET.fromstring(content)
            removed = []
            for child in list(root):
                if child.get("id") in root_ids:
                    removed.append(child.get("id"))
                    root.remove(child)
            if set(removed) != set(root_ids):
                raise ValueError("Source SVG producer filter contains absent root nodes")
            ET.register_namespace("", "http://www.w3.org/2000/svg")
            ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
            content = ET.tostring(root, encoding="utf-8")
            transformation = {
                "tool": "Python xml.etree.ElementTree", "version": sys.version.split()[0],
                "operation": "Remove only named source SVG root children; serialize preserved tree",
                "removed_root_ids": removed, "root_attributes": dict(root.attrib),
                "producer": {"input": producer_row, "complete_module": body},
            }
        if source.suffix.lower() == ".avif":
            import io
            sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
            import PIL
            from PIL import Image, features
            with Image.open(source) as image:
                rgba = image.convert("RGBA")
                buffer = io.BytesIO()
                rgba.save(buffer, format="PNG")
                content = buffer.getvalue()
                conversion = {"tool": "Pillow", "version": PIL.__version__,
                              "codec": "libavif", "codec_version": features.version("avif"),
                              "mode": "RGBA", "width": rgba.width, "height": rgba.height}
        if request.get("muted_video_animation") and source_hash in muted_animation_cache:
            content, conversion = muted_animation_cache[source_hash]
        elif request.get("muted_video_animation"):
            import io
            sys.path.insert(0, str(ROOT / "tools/analyzers/media-python"))
            sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
            import av
            import PIL
            from PIL import Image, features
            if source.suffix.lower() != ".mp4" or not display.endswith(".webp"):
                raise ValueError("Muted source video preparation requires MP4 and animated WebP")
            with av.open(str(source)) as container:
                if len(container.streams.video) != 1:
                    raise ValueError("Muted source tutorial must have exactly one video stream")
                stream = container.streams.video[0]
                frames, timestamps = [], []
                for frame in container.decode(stream):
                    if frame.pts is None or frame.time_base is None:
                        raise ValueError("Muted source frame has no presentation timestamp")
                    timestamps.append(float(frame.pts * frame.time_base))
                    frames.append(frame.to_image().convert("RGBA"))
                if not frames or any(b <= a for a, b in zip(timestamps, timestamps[1:])):
                    raise ValueError("Muted source frame presentation order is invalid")
                end = float(stream.duration * stream.time_base) if stream.duration is not None else timestamps[-1] + float(1 / stream.average_rate)
                boundaries = [round(value * 1000) for value in timestamps + [end]]
                durations = [right - left for left, right in zip(boundaries, boundaries[1:])]
                if any(value <= 0 for value in durations):
                    raise ValueError("Muted source timing cannot be represented in millisecond animation frames")
                buffer = io.BytesIO()
                frames[0].save(buffer, format="WEBP", save_all=True, append_images=frames[1:],
                               duration=durations, loop=0, lossless=True, quality=100, method=0)
                content = buffer.getvalue()
                conversion = {"tool": "PyAV / Pillow", "version": {"av": av.__version__, "pillow": PIL.__version__},
                    "codec": "libwebp lossless RGBA", "codec_version": features.version("webp"),
                    "width": frames[0].width, "height": frames[0].height, "decoded_frames": len(frames),
                    "presentation_timestamps_seconds": timestamps, "frame_durations_ms": durations,
                    "muted": True, "loop": True, "target_execution": False}
                muted_animation_cache[source_hash] = content, conversion
        output_hash = hashlib.sha256(content).hexdigest()
        product_match = re.fullmatch(r"https://apps\.razer\.com/synapse/products/(\d+)/.*", row["url"])
        product_id = int(product_match[1]) if product_match else None
        output = shared_resource_output(content, Path(display).suffix.lower(), content_pool,
                                        product_id=product_id if product_id in registered_products else None)
        entry = {"source": relative(source), "source_url": row["url"],
                 "source_sha256": source_hash, "source_bytes": len(source.read_bytes()),
                 "output": relative(output), "sha256": output_hash, "display_paths": [display],
                 "acquisition": relative(receipt_path),
                 "preparation": ("Static muted video-to-lossless-RGBA-animation conversion; original frames and timing retained."
                    if request.get("muted_video_animation") else "Static AVIF-to-RGBA-PNG conversion; original bytes retained.")
                    if conversion else "Exact current acquired bytes; no geometry or color edits."}
        if conversion:
            entry["conversion"] = conversion
        if transformation:
            entry["transformation"] = transformation
            entry["preparation"] = transformation["operation"] + "; original and display bytes retained separately."
        if favicon_sources:
            entry["host_favicon_sources"] = favicon_sources
        manifest["entries"] = [known for known in manifest["entries"]
                               if display not in known.get("display_paths", [])]
        manifest["entries"].append(entry)
        aliases[:] = [row for row in aliases if row["key"] != display]
        aliases.append({"key": display, "path": relative(output)})
        prepared.append({"display": display, "source_sha256": source_hash,
                         "output_sha256": output_hash, "output": relative(output),
                         "conversion": conversion,
                          "transformation": {key: value for key, value in transformation.items()
                                             if key not in {"producer", "source_regions", "source_buttons", "complete_css_rules", "source_css"}}
                            if transformation else None})
    write_json(manifest_path, manifest)
    resource_sources.save_resource_mappings(mappings, root=ROOT)
    return prepared


def prepare_host_favicons():
    """Attach all registered first-favicon sources to the existing resource flow."""
    import analysis
    import families
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        registered = registered_resource_products()
        metadata_path = analysis.builtin_data_path("razer-shell", "HOST_DEVICE_FAVICONS")
        metadata = json.loads(metadata_path.read_bytes())
        layout = json.loads((ROOT / "assets/synapse/indexes/resource-layout.json").read_bytes())
        resources = {key: row for row in layout["resources"] for key in row["runtime_keys"]}
        cache, by_alias = {}, {}
        for product in sorted(registered):
            alias = metadata.get(str(product))
            if not alias or alias not in resources:
                raise ValueError(f"Registered product {product} lacks a prepared favicon alias")
            receipts = [ROOT / f".analysis/inputs/frontend/{family}-{product}-current/acquisition.json"
                        for family in families.FAMILY_CANDIDATES]
            receipts = [path for path in receipts if path.is_file()]
            if len(receipts) != 1:
                raise ValueError(f"Product {product} does not have one unambiguous current UI acquisition")
            proof = current_product_favicon(product, relative(receipts[0]), cache)
            by_alias.setdefault(alias, []).append(proof)
        groups = []
        for alias, sources in sorted(by_alias.items()):
            first = sources[0]
            prepared = safe(ROOT / resources[alias]["physical_path"])
            original = (ROOT / first["original"]["path"]).read_bytes()
            current = prepared.read_bytes()
            if original != current and (original.count(b"\r") or original.replace(b"\n", b"\r\n") != current):
                raise ValueError("Prepared favicon has no exact current-source byte transformation: " + alias)
            groups.append({"acquisition": first["acquisition"]["path"], "resources": {
                first["original"]["path"]: {"display": alias,
                    "host_favicon_sources": [{"product_id": row["product_id"], "acquisition": row["acquisition"]["path"]}
                                             for row in sources],
                    "retain_prepared_favicon": families.identity(prepared)}}})
        prepared = prepare_resources_locked(groups)
        return {"registered_products": len(registered), "favicon_aliases": len(prepared),
                "newline_transformed_aliases": sum(bool(row["transformation"]) for row in prepared),
                "prepared": prepared, "target_execution": False}


def prepare_app_picker():
    """Capture and prepare the current mounted menu's category art and defaults."""
    import analysis
    import families
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    import json5
    receipt_path = ROOT / ".analysis/inputs/frontend/rz-app-menu-current/acquisition.json"
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        receipt = json.loads(receipt_path.read_bytes())
        if not receipt.get("source_current_during_acquisition"):
            raise ValueError("App menu deployment has not been verified stable")
        originals = []

        def original(suffix):
            matches = [row for row in receipt["files"] if row.get("http_status") == 200
                       and row.get("url", "").endswith(suffix)]
            if len(matches) != 1:
                raise ValueError("App menu original input is ambiguous: " + suffix)
            row = matches[0]
            path = safe(ROOT / row["path"])
            if digest(path) != row["sha256"] or path.stat().st_size != row["bytes"]:
                raise ValueError("App menu original bytes differ: " + row["path"])
            originals.append(row)
            return row, path.read_text(encoding="utf-8")

        root_row, root_text = original("/rz-app-menu/static/js/Root.fddb4c8a.chunk.js")
        main_row, main_text = original("/rz-app-menu/static/js/main.83ced465.js")
        css_row, css_text = original("/rz-app-menu/static/css/main.3f2a3e21.css")
        shapes, regions = app_picker_category_literals(root_text)
        root_module = complete_resource_module(root_text, "const y=O,I=[")
        category_module = complete_resource_module(main_text, 'const o="currentLanguage"')
        if root_module["module_id"] != 569 or category_module["module_id"] != 141:
            raise ValueError("Current app menu mounted module ownership differs")
        category_text = category_module["body"]
        if "Su:()=>n" not in category_text:
            raise ValueError("Current app menu category export differs")
        start = category_text.index("n={KEYBOARD:") + 2
        end = families.javascript_delimited(category_text, start)
        categories = json5.loads(category_text[start:end])
        if any(key != value for key, value in categories.items()):
            raise ValueError("Current app menu category identity table differs")

        def selected(category, subcategory):
            key = subcategory if category in categories and subcategory else category
            return key if key in shapes else category if subcategory and category in shapes else None

        aliases = {name: "synapse/app-picker-category/" + name.lower().replace("_", "-") + ".svg"
                   for name in shapes}
        registered = registered_resource_products()
        manifest = json.loads((ROOT / "assets/synapse/manifests/manifest.json").read_bytes())
        host_alias_set = set(json.loads(analysis.builtin_data_path("razer-shell", "HOST_DEVICE_FAVICONS").read_bytes()).values())
        products, proofs, cache = {}, [], {}
        for entry in manifest["entries"]:
            sources = entry.get("host_favicon_sources", [])
            if not sources:
                continue
            host_aliases = [alias for alias in entry["display_paths"] if alias in host_alias_set]
            if len(host_aliases) != 1:
                raise ValueError("Current first favicon alias is ambiguous")
            for retained in sources:
                proof = current_product_favicon(retained["product_id"], retained["acquisition"]["path"], cache)
                if proof != retained:
                    raise ValueError("Current first favicon provenance changed")
                pid = proof["product_id"]
                category, _, subcategory = Path(proof["first_favicon_element"]["href"]).stem.partition("_")
                chosen = selected(category, subcategory)
                # This is native catalog presentation when category observations
                # are absent. It is never published as service/device state.
                asset = aliases[chosen] if chosen else host_aliases[0]
                if str(pid) in products:
                    raise ValueError("Duplicate app-menu product fallback")
                products[str(pid)] = {"category": category, "sub_category": subcategory or None,
                                      "asset": asset}
                proofs.append({"product_id": pid, "first_favicon": proof,
                    "selected_category": chosen, "runtime_alias": asset,
                    "local_fallback": "current first favicon classification" if chosen else "current first favicon SVG; no category vector exists in U"})
        if set(map(int, products)) != registered:
            raise ValueError("App menu local category fallbacks do not cover the exact registry")
        css_rules = [match[0] for match in re.finditer(r"[^{}]+\{[^{}]*\}", css_text)
                     if any(value in match[0].split("{", 1)[0] for value in (
                         ".app-explorer-popup", ".app-module-wrapper", ".item-wrapper", ".discover-app"))]
        report = {"recorded_at_utc": datetime.now(timezone.utc).isoformat(), "target_execution": False,
            "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__)),
                     "python": sys.version.split()[0], "json5": json5.__version__},
            "acquisition": families.identity(receipt_path), "original_inputs": originals,
            "source_current_during_acquisition": True, "stability": receipt["stability"],
            "acquisition_failures": [row for row in receipt["files"] if row.get("http_status") != 200],
            "unresolved_acquisition_chains": receipt["unresolved"],
            "complete_modules": [{"input": root_row["path"], **root_module}, {"input": main_row["path"], **category_module}],
            "complete_regions": regions, "complete_css_rules": css_rules,
            "category_domain": categories, "active_paths": shapes, "product_fallbacks": proofs,
            "source_semantics": {"mounted_chain": "Root ne -> connected te validDevices map -> U -> b (I reduced by P) -> Y; H -> c uses inline SVG for device, image URL for modules/apps",
                "icon_size": [40, 40], "icon_viewbox": [0, 0, 20, 20], "item_height": 96,
                "unused_device_item_height": "100px rule is not mounted by c; c uses item and optional device-item-elipsis",
                "popup_width": 410, "group_horizontal_padding": 15, "title_width": 110,
                "title_font": "normal 14px Roboto", "title_line_height": 1.2,
                "title_clamp": {"measured_threshold": 110, "lines": 2, "height": 33,
                    "overflow": "hidden", "text_overflow": "ellipsis",
                    "hover": "display:block; height:auto; overflow:initial; natural wrapping within 110px above next row"},
                "excluded_online_branch": "discover-app image, external software-catalog link and its 10/15/20 padding removed together",
                "device_category_policy": "Observed category/subCategory uses U exactly, including absent result. Native previews and snapshots without source_category use the separately audited current first-favicon presentation fallback without writing it to device state."},
            "runtime_acceptance": "unverified; no application/window/vendor execution"}
        report_path = ROOT / ".analysis/results/frontend/rz-app-menu-current/device-artwork.json"
        write_json(report_path, report)
        output = analysis.builtin_data_path("razer-app-pages", "APP_PICKER_ICONS_DATA")
        write_json(output, {"subcategory_categories": list(categories), "icons": aliases, "products": products})
        prepared = prepare_resources_locked(relative(receipt_path), {root_row["path"]: [
            {"display": alias, "app_picker_category": category} for category, alias in aliases.items()]})
        report["output"] = families.identity(output)
        report["prepared"] = prepared
        write_json(report_path, report)
        return {"report": relative(report_path), "registered_products": len(products),
                "category_icons": len(aliases), "target_execution": False}


@contextmanager
def verification_environment(name):
    """Give each check owned temporary storage without changing product paths."""
    directory = safe(ROOT / "target/verification-temp")
    directory.mkdir(parents=True, exist_ok=True)
    # Keep generated test state under build output, not analysis inputs or
    # fixture/scratch paths. TemporaryDirectory owns only its unique child;
    # parallel validation commands cannot remove each other's working files.
    with tempfile.TemporaryDirectory(prefix=name + "-", dir=directory) as temporary:
        path = safe(Path(temporary))
        environment = os.environ.copy()
        for variable in ("TEMP", "TMP", "TMPDIR"):
            environment[variable] = str(path)
        yield environment, relative(path)


def prepare_keyboard_identity():
    import analysis
    audit_path = ROOT / ".analysis/results/frontend/keyboard-mw-audit.json"
    audit = json.loads(audit_path.read_bytes())["keyboard_metadata"]
    products, sources = [], []
    verified = {}
    for row in audit["products"]:
        cache = row.get("identity_cache")
        if row.get("failures") or not cache:
            continue
        if cache["policy"] not in {"replace_product", "replace_serial"}:
            raise ValueError("Unknown current identity cache policy")
        inputs = row["original_inputs"] + [row["acquisition"]]
        for original in inputs:
            path = safe(ROOT / original["path"])
            if original["path"] not in verified:
                verified[original["path"]] = {"bytes": path.stat().st_size, "sha256": digest(path)}
            if any(verified[original["path"]][key] != original[key] for key in ("bytes", "sha256")):
                raise ValueError("Keyboard identity original bytes changed: " + original["path"])
        receipt = json.loads((ROOT / row["acquisition"]["path"]).read_bytes())
        if not receipt.get("source_current_during_acquisition"):
            raise ValueError("Keyboard identity acquisition is not stable")
        owners = [owner for owner in row["complete_bodies"] if owner["role"] in {"identity-cache-publication", "identity-runtime-defaults"}]
        if len(owners) != 2:
            raise ValueError("Keyboard identity cache owner is incomplete")
        for owner in owners:
            text = (ROOT / owner["input"]).read_text(encoding="utf-8")
            body = text[owner["character_start"]:owner["character_end"]]
            if (body != audit["complete_bodies"][owner["body_sha256"]]
                    or hashlib.sha256(body.encode("utf-8")).hexdigest() != owner["body_sha256"]):
                raise ValueError("Keyboard identity cache function body differs")
        for key in ("available_layout_ids", "available_edition_ids"):
            if not cache.get(key) or any(type(value) is not int or not 0 <= value <= 255 for value in cache[key]):
                raise ValueError("Keyboard identity source domain is invalid")
        products.append({"product_id": row["product_id"], **{key: cache[key] for key in (
            "policy", "category", "available_layout_ids", "available_edition_ids", "runtime_default_layout_id", "is_chroma_device")}})
        sources.append({"product_id": row["product_id"], "original_inputs": inputs,
                        "complete_owners": owners, "dependency_closure": row["dependency_closure"]})
    if not products:
        raise ValueError("No current keyboard identity cache sources verified")
    output = analysis.builtin_data_path("razer-model", "SOURCE_IDENTITY_DATA")
    write_json(output, products)
    report = {"recorded_at_utc": datetime.now(timezone.utc).isoformat(), "products": sources,
              "audit": {"path": relative(audit_path), "sha256": digest(audit_path)},
              "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__))},
              "output": {"path": relative(output), "sha256": digest(output), "bytes": output.stat().st_size},
              "target_execution": False, "real_device_operations": False}
    destination = ROOT / ".analysis/results/keyboard-identity-current.json"
    write_json(destination, report)
    return {"products": len(products), "report": relative(destination), "output": report["output"]}


def prepare_product_names():
    import analysis
    import families
    manifests = []
    for receipt_path in sorted((ROOT / ".analysis/inputs/frontend").glob("*current/acquisition.json")):
        receipt = json.loads(receipt_path.read_bytes())
        match = re.fullmatch(r"https://apps\.razer\.com/synapse/products/(\d+)/mw/index\.html", receipt.get("entry_url", ""))
        if match:
            manifests.append(receipt_path.parent / f"raw/apps.razer.com/synapse/products/{match[1]}/mw/manifest.json")
    output = analysis.builtin_data_path("razer-model", "CATALOG")
    report_path = ROOT / ".analysis/results/product-names-current.json"
    catalog, sources, failures, indexed, excluded_endpoints = {}, [], [], [], []
    index = {row["path"]: row for row in json.loads(INDEX.read_bytes())["inputs"]}
    for manifest in manifests:
        receipt_path = manifest.parents[6] / "acquisition.json"
        try:
            receipt = json.loads(receipt_path.read_bytes())
            rows = {row.get("path"): row for row in receipt["files"]}
            entry = manifest.with_name("index.html")
            verified = []
            for path in (manifest, entry):
                name = relative(path)
                actual = {"path": name, "bytes": path.stat().st_size, "sha256": digest(path)}
                row = rows.get(name, {})
                if (row.get("http_status") != 200
                        or any(row.get(key) != actual[key] for key in ("bytes", "sha256"))
                        or not row.get("url", "").startswith("https://apps.razer.com/synapse/products/")):
                    raise ValueError("Acquisition identity does not verify: " + name)
                if name in index and any(index[name].get(key) != actual[key] for key in ("bytes", "sha256")):
                    raise ValueError("Input index identity differs: " + name)
                stable = next((value for value in receipt.get("stability", []) if value["url"] == row["url"]), {})
                if (not receipt.get("source_current_during_acquisition") or not stable.get("unchanged")
                        or stable.get("before_sha256") != actual["sha256"]
                        or stable.get("after_sha256") != actual["sha256"]):
                    raise ValueError("Current deployment stability does not verify: " + name)
                verified.append({**actual, "url": row["url"]})
            main_scripts = re.findall(r'<script\b[^>]*\bsrc=[\"\'](\./main\.[\w]+\.js)[\"\']', entry.read_text(encoding="utf-8"))
            if len(main_scripts) != 1:
                raise ValueError("Current middleware entry does not mount one main module")
            main_path = safe(manifest.parent / main_scripts[0])
            main_row = rows.get(relative(main_path), {})
            if (main_row.get("sha256") != digest(main_path)
                    or main_row.get("bytes") != main_path.stat().st_size
                    or main_row.get("url") != rows[relative(entry)]["url"].rsplit("/", 1)[0] + "/" + main_path.name):
                raise ValueError("Current mounted middleware main module does not verify")
            main_provenance = {"kind": "official_http_response", "http_status": main_row.get("http_status")}
            if main_row.get("http_status") != 200:
                if main_row.get("kind") != "installed_cached_official_resource" or not main_row.get("cache_crc_verified"):
                    raise ValueError("Mounted middleware main acquisition provenance does not verify")
                container = safe(ROOT / main_row["container"])
                if (not container.is_relative_to(ROOT / ".analysis/inputs")
                        or digest(container) != main_row.get("container_sha256")):
                    raise ValueError("Mounted middleware cache container does not verify")
                offset, size = main_row.get("body_offset"), main_row.get("body_bytes")
                if (type(offset) is not int or type(size) is not int or offset < 0
                        or size != main_path.stat().st_size or offset + size > container.stat().st_size):
                    raise ValueError("Mounted middleware cache body bounds do not verify")
                with container.open("rb") as stream:
                    stream.seek(offset)
                    if stream.read(size) != main_path.read_bytes():
                        raise ValueError("Mounted middleware cache body differs from the original container")
                main_provenance = {"kind": "installed_cache_body_mounted_by_current_entry",
                                   "container": relative(container), "container_sha256": digest(container),
                                   "body_offset": offset, "body_bytes": size,
                                   "remote_body_revalidated": False}
            verified.append({"path": relative(main_path), "bytes": main_path.stat().st_size,
                             "sha256": digest(main_path), "url": main_row["url"],
                             "provenance": main_provenance})
            product_id = manifest.parts[-3]
            data = json.loads(manifest.read_bytes())
            inline_provider = None
            if not data.get("productTranslations"):
                inline_provider = families.inline_product_names(receipt_path, main_row)
                if inline_provider["declared_product_id"] != int(product_id):
                    excluded_endpoints.append({"endpoint_product_id": int(product_id),
                                               "declared_product_id": inline_provider["declared_product_id"],
                                               "reason": "Current endpoint runtime declares a different product; do not relabel or overwrite its actual product endpoint",
                                               "inputs": verified, "provider": inline_provider["evidence"]})
                    continue
                data["productTranslations"] = inline_provider["product_translations"]
            editions = {}
            for variant, translations in data.get("productTranslations", {}).items():
                match = re.fullmatch(r"(\d+)_(\d+)", variant)
                if not match or not isinstance(translations, dict):
                    raise ValueError("Invalid product translation identity")
                if match[1] != product_id:
                    continue
                values = {}
                for locale, strings in translations.items():
                    if not isinstance(strings, dict):
                        raise ValueError("Invalid product translation object")
                    value = {target: strings.get(source, "") for source, target in (
                        ("PRODUCT", "product"), ("DASHBOARD_NAME", "dashboard"), ("DASHBOARD_EDITION", "edition"))}
                    if not all(isinstance(text, str) for text in value.values()):
                        raise ValueError("Invalid product translation string")
                    values[locale.lower().replace("_", "-")] = value
                editions[match[2]] = values
            if not any(editions.values()):
                failures.append({"path": relative(manifest), "error": "No productTranslations in current manifest"})
                continue
            if product_id in catalog and catalog[product_id] != editions:
                raise ValueError("Conflicting current product translations")
            catalog[product_id] = editions
            indexed.extend(verified)
            sources.append({"product_id": int(product_id), "inputs": verified,
                            "acquisition": {"path": relative(receipt_path), "sha256": digest(receipt_path)},
                            "source_current_during_acquisition": True, "source_role": "middleware_device_name_provider",
                            "unresolved_resource_count": len(receipt.get("unresolved", [])),
                            "inline_provider": inline_provider["evidence"] if inline_provider else None})
        except (OSError, ValueError, KeyError) as error:
            failures.append({"path": relative(manifest),
                             "error": type(error).__name__ if isinstance(error, OSError) else str(error),
                             "errno": getattr(error, "errno", None)})
    import resource_database
    registry = ROOT / resource_database.PRODUCT_REGISTRY
    registered = registered_resource_products()
    report = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__)),
                       "python_version": sys.version.split()[0]},
              "sources": sources, "failures": failures, "excluded_endpoints": excluded_endpoints, "product_count": len(catalog),
              "missing_registered_products": sorted(registered - {int(product) for product in catalog}),
              "registry_locator": {"path": relative(registry), "sha256": digest(registry)},
              "scope": "Current middleware manifest productTranslations or verified mounted inline providers; static display strings only, no device edition/layout or service observations",
              "missing_source_policy": "No legacy dictionary merge; use untranslated registered manufacturer name",
              "target_execution": False, "real_device_operations": False}
    if not catalog:
        write_json(report_path, report)
        raise ValueError("No current product translation manifests verified")
    with analysis.index_lock():
        current = json.loads(INDEX.read_bytes())
        current_rows = {row["path"]: row for row in current["inputs"]}
        for row in indexed:
            previous = current_rows.get(row["path"])
            if previous and any(previous.get(key) != row[key] for key in ("bytes", "sha256")):
                raise ValueError("Input index changed during product-name preparation")
            current_rows.setdefault(row["path"], {**row, "kind": "official_frontend_artifact"})
        current["inputs"] = sorted(current_rows.values(), key=lambda row: row["path"])
        write_json(INDEX, current)
    write_json(output, catalog)
    report["output"] = {"path": relative(output), "bytes": output.stat().st_size, "sha256": digest(output)}
    write_json(report_path, report)
    return {"report": relative(report_path), "product_count": len(catalog), "failures": len(failures),
            "output": report["output"]}


def prepare_source_preview_domains(parameter_locators, *, prepare=False):
    import analysis
    import families
    products = {}
    evidence = []
    failures = []
    mappings = {}
    images = {}
    retry_producers = {}
    integration_previews = []
    source_cache = {}
    family_atlas = json.loads(analysis.builtin_data_path("razer-pages", "KEYBOARD_ARTWORK_CURRENT_DATA").read_bytes())
    family_pids = {product["product_id"] for product in family_atlas["products"]}
    module_literal = families.JAVASCRIPT_INTEGER_LITERAL
    export_pattern = re.compile(r'\b(' + module_literal + r'):(?:\([\w$,]*\)=>|[\w$]+=>|function\([\w$,]*\))\{([^{}]{0,1000})\}')
    for locator in parameter_locators:
        source = locator.get("input", {})
        source_url = source.get("url", "")
        if not source_url.endswith(".js") or not locator.get("index"):
            continue
        if not families.valid(source) or not families.valid(locator["index"]):
            raise ValueError("Current preview resource context identity differs")
        requests = []
        source_text = (ROOT / source["path"]).read_text(encoding="utf-8")
        for route in locator.get("routes", []):
            match = re.fullmatch(r'"\./(\d+)_(\d+)/(?:img_prods|svg_prods|img_svgs)/(\d+|prd)(?:-[13]x\.(?:png|svg)|\.svg)"', route["literal"])
            if match is None:
                continue
            pid, edition = map(int, match.groups()[:2])
            layout = 0 if match[3] == "prd" else int(match[3])
            if f"/products/{pid}/ui/" not in source_url:
                continue
            variants = products.setdefault(pid, set())
            if match[3] != "prd":
                variants.add((edition, layout))
            requests.append(route)
            if pid in family_pids:
                continue
            try:
                receipt_path = ROOT / locator["acquisition"]
                if pid not in source_cache:
                    receipt = json.loads(receipt_path.read_bytes())
                    originals = {item["url"]: item for item in receipt["files"] if item.get("path")}
                    manifest = locator.get("asset_manifest")
                    if manifest is None or not families.valid(manifest):
                        raise ValueError("Current preview asset manifest is missing or changed")
                    declared = json.loads((ROOT / manifest["path"]).read_bytes())["files"].values()
                    script_urls = {urllib.parse.urljoin(manifest["url"], value) for value in declared
                                   if urllib.parse.urlsplit(value).path.endswith(".js")}
                    exports = {}
                    inline_images = {}
                    for url in script_urls:
                        original = originals.get(url)
                        if original is None or not families.valid(original):
                            for attempt in range(3):
                                original = analysis.download(url, receipt_path.parent)
                                if original.get("http_status") == 200 and families.valid(original):
                                    break
                            if not families.valid(original):
                                failures.append({"product_id": pid, "kind": "missing-current-image-export-script",
                                                 "original": original})
                                continue
                            receipt["files"] = [item for item in receipt["files"] if item["url"] != url] + [original]
                            originals[url] = original
                            write_json(receipt_path, receipt)
                            families.index_rows([original], receipt_path)
                        body = (ROOT / original["path"]).read_text(encoding="utf-8")
                        if not any(token in body for token in (
                                "static/media/", "data:image/png;base64,", "img_prods", "svg_prods", "img_svgs")):
                            continue
                        async_functions = list(re.finditer(r'(?:return )?async function\([\w$,]*\)\{', body))
                        retry_anchors = list(re.finditer(r'0!==[\w$]+\)try\{', body))
                        candidate_functions = {}
                        for anchor in retry_anchors:
                            for function in reversed(async_functions):
                                if function.start() >= anchor.start():
                                    continue
                                end = families.javascript_delimited(body, function.end() - 1)
                                if end > anchor.start():
                                    candidate_functions[function.start()] = (function, end)
                                    break
                        for function, end in candidate_functions.values():
                            producer = body[function.start():end]
                            if not re.search(r'0!==[\w$]+\)try\{', producer):
                                continue
                            template = re.search(r'`\./\$\{([\w$]+)\}_\$\{([\w$]+)\}/', producer)
                            template_retry = template is not None and (
                                f'`./${{{template[1]}}}_0/' in producer
                                and f'0!=={template[2]})try{{' in producer)
                            concat_retry = ('"[ProductImage] loadImg"' in producer
                                and re.search(r'\.concat\([\w$]+,"_",0,"/"\)', producer))
                            if (template_retry or concat_retry) and (
                                    'img_prods' in producer or 'svg_prods' in producer):
                                retry_producers[pid] = {"original": original,
                                    "character_start": function.start(), "character_end": end,
                                    "complete_function": producer}
                                break
                        hook_retry = re.search(
                            r'(?P<loader>[\w$]+)\((?P<edition>[\w$]+)\)\.then\((?P<setter>[\w$]+)\)'
                            r'\.catch\((?P<error>[\w$]+)=>\{if\(console\.warn\((?P=error)\),'
                            r'0!==(?:\s*)(?P=edition)\)return (?P=loader)\(0\)\.then\((?P=setter)\)\}\)', body)
                        if hook_retry and pid not in retry_producers:
                            arrows = list(re.finditer(r'\b([\w$]+)=(?:(?:\([\w$,]+\)|[\w$]+)=>|function\([\w$,]+\))\{', body))
                            for hook in reversed(arrows):
                                if hook.start() >= hook_retry.start():
                                    continue
                                hook_end = families.javascript_delimited(body, hook.end() - 1)
                                if hook_end <= hook_retry.end():
                                    continue
                                for caller in arrows:
                                    if caller.start() < hook_end:
                                        continue
                                    caller_end = families.javascript_delimited(body, caller.end() - 1)
                                    complete_caller = body[caller.start():caller_end]
                                    if ('img_prods/prd-1x' in complete_caller
                                            and re.search(r'\b' + re.escape(hook[1]) + r'\([^)]{0,100}["`]single-', complete_caller)):
                                        retry_producers[pid] = {"original": original,
                                            "character_start": hook.start(), "character_end": hook_end,
                                            "complete_function": body[hook.start():hook_end],
                                            "caller_start": caller.start(), "caller_end": caller_end,
                                            "complete_caller": complete_caller}
                                        break
                                break
                        for exported in export_pattern.finditer(body):
                            filename = re.search(r'\.p\s*\+\s*["\'](static/media/[^"\']+)["\']', exported[2])
                            if filename:
                                exports[families.javascript_integer_literal(exported[1])] = (original, exported[0], filename[1])
                        for exported in re.finditer(r'\b(' + module_literal + r'):(?:\([\w$,]*\)=>|[\w$]+=>|function\([\w$,]*\))\{(?:"use strict";)?[\w$]+\.exports="data:image/png;base64,([A-Za-z0-9+/=]+)"\}', body):
                            module_id = families.javascript_integer_literal(exported[1])
                            exports[module_id] = (original, exported[0], None)
                            inline_images[module_id] = hashlib.sha256(base64.b64decode(exported[2], validate=True)).hexdigest()
                    source_cache[pid] = receipt, originals, exports, inline_images
                receipt, originals, exports, inline_images = source_cache[pid]
                binding = re.match(r'\s*:\s*(?:\[(' + module_literal + r')(?:,[^\]]*)?\]|(' + module_literal + r'))(?![\w$.])', source_text[route["end_character"]:])
                if binding is None:
                    raise ValueError("Current preview resource context binding is unresolved")
                module_id = families.javascript_integer_literal(binding[1] or binding[2])
                original, export_body, filename = exports[module_id]
                url = urllib.parse.urljoin(receipt["entry_url"], filename) if filename else original["url"]
                resource = originals.get(url)
                if resource is None or not families.valid(resource):
                    resource = analysis.download(url, receipt_path.parent)
                    if resource.get("http_status") != 200 or not families.valid(resource):
                        raise ValueError("Current preview resource acquisition failed: " + json.dumps(resource))
                    receipt["files"] = [item for item in receipt["files"] if item["url"] != url] + [resource]
                    originals[url] = resource
                    write_json(receipt_path, receipt)
                    families.index_rows([resource], receipt_path)
                svg = route["literal"].endswith('.svg"')
                density = 1 if svg else int(re.search(r'-([13])x\.png', route["literal"])[1])
                alias = f"synapse/preview-source-{pid}-{edition}-{layout}-{density}x" + (".svg" if svg else ".png")
                item = {"edition_id": edition, "layout_id": layout, "resolution": density, "svg": svg, "asset": alias}
                if item not in images.setdefault(pid, []):
                    images[pid].append(item)
                requests_for_resource = mappings.setdefault(locator["acquisition"], {}).setdefault(resource["path"], [])
                preparation = {"display": alias}
                if module_id in inline_images:
                    preparation["inline_data_sha256"] = inline_images[module_id]
                if preparation not in requests_for_resource:
                    requests_for_resource.append(preparation)
                route["resolved_image"] = {"runtime": item, "resource": resource,
                                           "export_original": original, "complete_export": export_body}
            except (OSError, ValueError, KeyError, AttributeError, TypeError) as error:
                failures.append({"product_id": pid, "edition_id": edition, "layout_id": layout,
                                 "request": route, "reason": str(error)})
        if requests:
            evidence.append({"input": source, "index": locator["index"],
                             "asset_manifest": locator.get("asset_manifest"), "requests": requests})
    hue_receipt_path = ROOT / ".analysis/inputs/frontend/accessories-769-current/acquisition.json"
    try:
        hue_receipt = json.loads(hue_receipt_path.read_bytes())
        if not hue_receipt.get("source_current_during_acquisition"):
            raise ValueError("Hue current deployment identity is unverified")
        originals = {item["url"]: item for item in hue_receipt["files"] if item.get("path")}
        index = originals[hue_receipt["entry_url"]]
        manifest = originals[urllib.parse.urljoin(hue_receipt["entry_url"], "asset-manifest.json")]
        if not families.valid(index) or not families.valid(manifest):
            raise ValueError("Hue current root identity differs")
        declared = json.loads((ROOT / manifest["path"]).read_bytes())["files"].values()
        mounted = [originals[urllib.parse.urljoin(manifest["url"], value)] for value in declared
                   if urllib.parse.urlsplit(value).path.endswith((".js", ".css"))]
        style = None
        component = None
        for original in mounted:
            if not families.valid(original):
                raise ValueError("Hue current component identity differs")
            body = (ROOT / original["path"]).read_text(encoding="utf-8")
            if original["url"].endswith(".css"):
                match = re.search(r'\.BridgeLogo_logo__[\w-]+\{[^{}]*background-image:url\(([^)]+hue_scan_logo[^)]*)\)[^{}]*\}', body)
                if match:
                    style = {"original": original, "complete_rule": match[0]}
                    resource_url = urllib.parse.urljoin(original["url"], match[1])
            else:
                for function in re.finditer(r'function [\w$]+\([\w$]+\)\{', body):
                    prefix = body[function.start():function.start() + 500]
                    bindings = body[max(0, function.start() - 300):function.start()]
                    if ('BridgeLogo_logo__' not in bindings
                            or '.isLoading' not in prefix or '.isPair' not in prefix):
                        continue
                    end = families.javascript_delimited(body, function.end() - 1)
                    component = {"original": original, "character_start": function.start(),
                                 "character_end": end, "complete_function": body[function.start():end]}
                    break
        if style is None or component is None:
            raise ValueError("Current Hue BridgeLogo source is unresolved")
        resource = originals[resource_url]
        if not families.valid(resource):
            raise ValueError("Current Hue BridgeLogo resource identity differs")
        alias = "synapse/preview-source-769-0-0-1x.svg"
        products.setdefault(769, set())
        images[769] = [{"edition_id": 0, "layout_id": 0, "resolution": 1, "svg": True, "asset": alias}]
        mappings.setdefault(relative(hue_receipt_path), {})[resource["path"]] = [{"display": alias}]
        integration_previews.append({"product_id": 769, "kind": "integration-bridge-logo",
                                     "not_hardware_photo": True, "index": index, "manifest": manifest,
                                     "component": component, "style": style, "resource": resource,
                                     "asset": alias})
    except (OSError, ValueError, KeyError, AttributeError, TypeError) as error:
        failures.append({"product_id": 769, "reason": str(error), "kind": "integration-preview"})
    destination = analysis.builtin_data_path("razer-pages", "KEYBOARD_PREVIEW_CURRENT_DATA")
    runtime = {"products": [{"product_id": pid,
                              "images": images.get(pid, []),
                              "edition_zero_retry": pid in retry_producers,
                              "source_variants": [{"edition_id": edition, "layout_id": layout}
                                                  for edition, layout in sorted(variants)]}
                             for pid, variants in sorted(products.items())]}
    if prepare:
        prepare_resources([{"acquisition": receipt, "resources": resources}
                           for receipt, resources in mappings.items()])
    else:
        previous = {item["product_id"]: item for item in json.loads(destination.read_bytes())["products"]} if destination.exists() else {}
        for product in runtime["products"]:
            product["images"] = previous.get(product["product_id"], {}).get("images", [])
    write_json(destination, runtime)
    write_json(ROOT / ".analysis/results/frontend/keyboard-preview-domain-audit.json",
               {"target_execution": False, "inputs": evidence, "output": families.identity(destination),
                "failures": failures, "images_prepared": prepare,
                "edition_zero_retry_producers": retry_producers,
                "integration_previews": integration_previews,
                "recorded_at_utc": analysis.stamp(), "tool": families.identity(Path(__file__)),
                "rule": "Exact current numeric resource-context requests, independent of successful image preparation"})
    unavailable = []
    dashboard_receipt_path = ROOT / ".analysis/inputs/frontend/dashboard-product-images-current/acquisition.json"
    if dashboard_receipt_path.is_file():
        dashboard_receipt = json.loads(dashboard_receipt_path.read_bytes())
        import resource_database
        registry = {row["id"]: row for row in resource_database.product_registry()}
        for pid in (103, 147, 196):
            cached = source_cache.get(pid)
            if cached is None:
                continue
            receipt, originals, _, _ = cached
            producer = None
            for original in originals.values():
                if not original["url"].endswith(".js") or f"/products/{pid}/" not in original["url"]:
                    continue
                if not families.valid(original):
                    continue
                body = (ROOT / original["path"]).read_text(encoding="utf-8")
                anchor = body.find("this.getImage=()=>{")
                if anchor < 0:
                    continue
                end = families.javascript_delimited(body, anchor + len("this.getImage=()=>"))
                complete = body[anchor:end]
                if 'this.props.editionId' not in complete or 'img_prods/prd' not in complete:
                    continue
                if 'catch' in complete or 'onError' in complete:
                    continue
                producer = {"input": original, "character_start": anchor, "character_end": end,
                            "complete_function": complete}
                break
            if producer is None or pid in retry_producers:
                continue
            product = registry.get(pid)
            if product is None:
                continue
            contexts = [{"input": locator["input"], "requests": [route["literal"] for route in locator.get("routes", [])
                         if route.get("product_id") == pid]}
                        for locator in parameter_locators
                        if f"/products/{pid}/ui/" in locator.get("input", {}).get("url", "")]
            source_editions = {int(re.match(r'"\./\d+_(\d+)/', request)[1])
                               for context in contexts for request in context["requests"]}
            for edition in product["edition_ids"]:
                if edition in source_editions:
                    continue
                requests = [item for item in dashboard_receipt["requests"]
                            if item["product_id"] == pid and item["edition_id"] == edition
                            and item["filename_edition_id"] == edition and item["layout_id"] == 0
                            and item["variant"] == "product"]
                if ({item["density"] for item in requests} != {1, 3}
                        or any('HTTP Error 404' not in item["original"].get("error", "") for item in requests)):
                    continue
                unavailable.append({"product_id": pid, "edition_id": edition, "layout_id": 0,
                                    "classification": "official-edition-image-unavailable",
                                    "reason": "Both exact Dashboard requests return 404; the current direct image caller preserves the requested edition and supplies no retry, and that edition has no resource export.",
                                    "dashboard_requests": requests, "producer": producer,
                                    "resource_contexts": contexts,
                                    "available_resource_editions": sorted(source_editions)})
    write_json(ROOT / "crates/razer-settings/testdata/product-image-unavailability.json",
               {"target_execution": False, "variants": unavailable})
    return runtime


def prepare_dashboard_images(workers=8, *, refresh_ledger=True, products=None, source_preview_only=False):
    """Acquire source-selected Dashboard artwork for the entire product locator.

    The current App.tt producer and BoxItemDevice.onBoxImageError own the URL
    grammar. Registry/previous atlas keys supply request parameters only; neither
    supplies artwork, behavior or a successful deployment claim. Missing images
    remain failed acquisition rows and never become fabricated placeholders.
    """
    import analysis
    import families
    acquisition_path = ROOT / ".analysis/inputs/frontend/synapse-dashboard-current/acquisition.json"
    dashboard = json.loads(acquisition_path.read_bytes())
    if not dashboard.get("source_current_during_acquisition"):
        raise ValueError("Dashboard artwork caller lacks verified deployment provenance")
    producers = []
    for prefix, anchor, terminator in (
        ("App.", "tt=e=>{var t,a,s,n,i;", None),
        ("7861.", "this.onBoxImageError=()=>this.setState(", ",this.renderBattValue="),
    ):
        row = next(row for row in dashboard["files"]
                   if Path(row.get("path", "")).name.startswith(prefix)
                   and row.get("path", "").endswith(".js"))
        if not families.valid(row):
            raise ValueError("Dashboard artwork caller original byte identity differs")
        source = (ROOT / row["path"]).read_text(encoding="utf-8")
        start = source.index(anchor)
        end = (families.javascript_delimited(source, source.index("{", start))
               if terminator is None else source.index(terminator, start))
        body = source[start:end]
        if prefix == "App.":
            required = ("_dashboard1x.avif", "_dashboard3x.avif", "e.sidepadLayout",
                        '"boolean"===typeof e.sidepadLayout', "e.wristrestConnected",
                        "e.isMatPlugged&&!e.isMouseConnected", "return c}")
        else:
            required = ("_0/PluginImages/", "_0_0_dashboard1x.avif",)
        if not all(literal in body for literal in required):
            raise ValueError("Current Dashboard artwork producer grammar changed")
        producer = {"input": row, "start_character": start, "end_character": end,
                    "complete_function": body,
                    "complete_function_sha256": hashlib.sha256(body.encode()).hexdigest()}
        try:
            producer["complete_module"] = families.complete_raw_module(source, anchor)
        except ValueError as error:
            # Retain the complete verified original and complete caller function.
            # This lexical scanner failure does not become a made-up module ID.
            producer["complete_module_failure"] = str(error)
        producers.append(producer)

    scope = set()
    keyboard_ids = set()
    locators = []
    for family in families.FAMILY_CANDIDATES:
        ids, locator = families.family_candidate_scope(family)
        scope.update(ids)
        if family == "keyboard":
            keyboard_ids.update(ids)
        locators.append({"family": family, "ids": ids, **locator})
    # Folder and filename editions are independent: App.tt's mat-only branch
    # retains the observed edition folder but explicitly uses filename edition 0.
    keys = {(pid, 0, 0, 0, "product") for pid in scope}
    # Common layout one is a request parameter, not an edition/layout fallback.
    keys.update((pid, 0, 0, 1, "product") for pid in keyboard_ids)
    parameter_locators = []
    parameter_failures = []
    import resource_database
    registry_path = ROOT / resource_database.PRODUCT_REGISTRY
    for product in resource_database.product_registry():
        pid = product["id"]
        if pid not in scope:
            continue
        for edition in product["edition_ids"]:
            keys.add((pid, edition, edition, 0, "product"))
            if pid in keyboard_ids:
                keys.add((pid, edition, edition, 1, "product"))
    parameter_locators.append({"input": families.identity(registry_path),
                               "usage": "Edition request candidates only; no historical semantics or artwork used"})
    # Only the verified current index's entry scripts supply additional resource
    # request parameters. Receipts may retain superseded main bundles, so do not
    # treat every file beginning with main as a mounted current entry.
    for locator in locators:
        for pid in locator["ids"]:
            source_receipt = ROOT / f'.analysis/inputs/frontend/{locator["family"]}-{pid}-current/acquisition.json'
            if not source_receipt.is_file():
                parameter_failures.append({"product_id": pid, "kind": "missing-current-ui-receipt"})
                continue
            source_data = json.loads(source_receipt.read_bytes())
            if not source_data.get("source_current_during_acquisition"):
                parameter_failures.append({"product_id": pid, "kind": "unverified-current-ui-deployment",
                                           "receipt": relative(source_receipt)})
                continue
            index_url = f"https://apps.razer.com/synapse/products/{pid}/ui/index.html"
            index = next((row for row in source_data["files"] if row.get("url") == index_url), None)
            if index is None or not families.valid(index):
                parameter_failures.append({"product_id": pid, "kind": "missing-or-changed-current-ui-index"})
                continue
            html = (ROOT / index["path"]).read_text(encoding="utf-8")
            script_urls = {urllib.parse.urljoin(index_url, match[1])
                           for match in re.finditer(r'<script\b[^>]*\bsrc=[\"\']([^\"\']+)[\"\']', html)}
            manifest_url = urllib.parse.urljoin(index_url, "asset-manifest.json")
            manifest_source = next((item for item in source_data["files"]
                                    if item.get("url") == manifest_url and item.get("path")), None)
            if manifest_source is not None:
                if not families.valid(manifest_source):
                    parameter_failures.append({"product_id": pid, "kind": "changed-current-ui-asset-manifest",
                                               "input": manifest_source})
                else:
                    manifest = json.loads((ROOT / manifest_source["path"]).read_bytes())
                    script_urls.update(urllib.parse.urljoin(manifest_url, value)
                                       for value in manifest["files"].values()
                                       if urllib.parse.urlsplit(value).path.endswith(".js"))
            for source_row in source_data["files"]:
                if source_row.get("url") not in script_urls:
                    continue
                if not families.valid(source_row):
                    parameter_failures.append({"product_id": pid, "kind": "changed-current-ui-entry-script",
                                               "input": source_row})
                    continue
                source = (ROOT / source_row["path"]).read_text(encoding="utf-8")
                routes = []
                for match in re.finditer(r'"\./(\d+)_(\d+)/(img_prods|svg_prods|img_svgs)/([^"\\]+)"', source):
                    request_pid, edition = int(match[1]), int(match[2])
                    if request_pid not in scope:
                        continue
                    filename = match[4]
                    layouts = [0]
                    layout_image = re.fullmatch(r"(\d+)-(?:1|3)x\.png", filename)
                    layout_image = layout_image or re.fullmatch(r"(\d+)\.svg", filename)
                    side_image = re.fullmatch(r"prd-sideview-(\d+)-(?:1|3)x\.png", filename)
                    if layout_image or side_image:
                        layouts.append(int((layout_image or side_image)[1]))
                    for layout in layouts:
                        keys.add((request_pid, edition, edition, layout, "product"))
                    routes.append({"literal": match[0], "start_character": match.start(),
                                   "end_character": match.end(), "product_id": request_pid,
                                   "resource_family": match[3],
                                   "edition_id": edition, "candidate_layout_ids": layouts})
                    if filename == "Mat Dark-S3.svg":
                        keys.add((request_pid, edition, 0, 1, "product"))
                if routes:
                    parameter_locators.append({"input": source_row, "index": index,
                                               "asset_manifest": manifest_source,
                                               "acquisition": relative(source_receipt), "routes": routes,
                                               "usage": "Exact current resource-context literals locate request parameters only"})
    mappings = resource_sources.load_resource_mappings(root=ROOT)
    for row in mappings["selectors"]["dashboard_image_selector"]:
        pid, edition, layout = row["product_id"], row["edition_id"], row["layout_id"]
        if pid in scope:
            keys.add((pid, edition, edition, layout, "product"))
    input_index_path = ROOT / ".analysis/inputs.json"
    input_index = json.loads(input_index_path.read_bytes())
    retained_routes = []
    retained_pattern = re.compile(r"https://apps\.razer\.com/synapse/products/(\d+)/ui/(\d+)_(\d+)/PluginImages/(\d+)_(\d+)_(\d+)(_wrist)?_dashboard[13]x\.avif$")
    resource_manifest_path = ROOT / "assets/synapse/manifests/manifest.json"
    resource_manifest = json.loads(resource_manifest_path.read_bytes())
    retained_sources = input_index["inputs"] + [{"url": item.get("source_url", "")}
                                               for item in resource_manifest["entries"]]
    for original in retained_sources:
        route = retained_pattern.fullmatch(original.get("url", ""))
        if route is None:
            continue
        pid, folder_pid, edition, image_pid, image_edition, layout = map(int, route.groups()[:6])
        if pid not in scope or pid != folder_pid or pid != image_pid:
            continue
        keys.add((pid, edition, image_edition, layout, "wrist" if route[7] else "product"))
        retained_routes.append({"url": original["url"], "product_id": pid,
                                "edition_id": edition, "filename_edition_id": image_edition,
                                "layout_id": layout})
    parameter_locators.append({"input": families.identity(input_index_path),
                               "resource_manifest": families.identity(resource_manifest_path), "routes": retained_routes,
                               "usage": "Retained official URL parameters only; every selected image is reverified against its official endpoint"})
    preview = prepare_source_preview_domains(parameter_locators, prepare=True)
    if source_preview_only:
        if refresh_ledger:
            analysis.refresh_gaps()
        print(json.dumps({"source_preview_only": True,
                          "report": ".analysis/results/frontend/keyboard-preview-domain-audit.json",
                          "products": len(preview["products"])}), flush=True)
        return
    # The same current producer has a wrist image branch. Acquire both densities
    # for observed keyboard layouts, keeping its distinct suffix and failures.
    keys.update((pid, edition, image_edition, layout, "wrist")
                for pid, edition, image_edition, layout, _ in tuple(keys)
                if pid in keyboard_ids)
    output = ROOT / ".analysis/inputs/frontend/dashboard-product-images-current"
    output.mkdir(parents=True, exist_ok=True)
    receipt_path = output / "acquisition.json"
    previous = json.loads(receipt_path.read_bytes()) if receipt_path.is_file() else {}
    previous_rows = {row["url"]: row for row in previous.get("files", [])}
    files = []
    failures = []
    stability = []
    for producer in producers:
        current = analysis.download(producer["input"]["url"], output)
        files.append(current)
        unchanged = (current.get("sha256") == producer["input"]["sha256"]
                     and current.get("http_status") == 200)
        stability.append({"url": current["url"], "kind": "current-artwork-caller",
                          "before_sha256": producer["input"]["sha256"],
                          "after_sha256": current.get("sha256"), "unchanged": unchanged})
        if not unchanged:
            raise ValueError("Official Dashboard artwork caller changed; reacquire its complete source set")

    requests = []
    for pid, edition, image_edition, layout, variant in sorted(keys):
        for density in (1, 3):
            suffix = "_wrist" if variant == "wrist" else ""
            url = (f"https://apps.razer.com/synapse/products/{pid}/ui/{pid}_{edition}/PluginImages/"
                   f"{pid}_{image_edition}_{layout}{suffix}_dashboard{density}x.avif")
            requests.append({"product_id": pid, "edition_id": edition, "layout_id": layout,
                             "filename_edition_id": image_edition,
                             "variant": variant, "density": density, "url": url})
    selected = set(products or scope)
    if selected - scope:
        raise ValueError("Dashboard image refresh product is outside the current product scope")
    retained_requests = [item for item in previous.get("requests", []) if item["product_id"] not in selected]
    requests = [item for item in requests if item["product_id"] in selected]

    def acquire(request):
        old = previous_rows.get(request["url"])
        if old and families.valid(old):
            return dict(old)
        attempts = []
        for _ in range(3):
            current = analysis.download(request["url"], output)
            if current.get("http_status") == 200 or "HTTP Error 404" in current.get("error", ""):
                break
            attempts.append(current)
        if attempts:
            current["previous_attempts"] = attempts
        return current

    print(json.dumps({"dashboard_artwork_requests": len(requests), "product_ids": len(scope)}), flush=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        for request, row in zip(requests, pool.map(acquire, requests)):
            request["original"] = row
            files.append(row)
            if row.get("http_status") != 200:
                failures.append({"kind": "source-artwork-request-unavailable", **request})

    def recheck(row):
        failures = []
        for _ in range(3):
            try:
                request = urllib.request.Request(row["url"], headers={
                    "User-Agent": "razer-ui-static-evidence", "Accept-Encoding": "identity"})
                with urllib.request.urlopen(request, timeout=40) as response:
                    after = hashlib.sha256(response.read()).hexdigest()
                return {"url": row["url"], "before_sha256": row["sha256"],
                        "after_sha256": after, "unchanged": row["sha256"] == after,
                        "previous_attempt_failures": failures}
            except Exception as error:
                failures.append(str(error))
        return {"url": row["url"], "before_sha256": row["sha256"],
                "unchanged": False, "error": failures[-1], "previous_attempt_failures": failures[:-1]}

    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        stability.extend(pool.map(recheck, [row for row in files if row.get("http_status") == 200]))
    retained_urls = {item["url"] for item in retained_requests}
    stability.extend({**item, "retained_from_acquisition_utc": previous.get("recorded_at_utc")}
                     for item in previous.get("stability", []) if item["url"] in retained_urls)
    for request in retained_requests:
        files.append(request["original"])
        if request["original"].get("http_status") != 200:
            failures.append({"kind": "source-artwork-request-unavailable", **request})
    requests = sorted([*retained_requests, *requests], key=lambda item: item["url"])
    stable_urls = {row["url"] for row in stability if row["unchanged"]}
    caller_stable = all(producer["input"]["url"] in stable_urls
                        and all(row["unchanged"] for row in stability if row["url"] == producer["input"]["url"])
                        for producer in producers)
    data = {"schema_version": 1, "entry_url": dashboard["entry_url"],
            "recorded_at_utc": analysis.stamp(), "files": files, "stability": stability,
            "source_current_during_acquisition": caller_stable and all(row["unchanged"] for row in stability),
            "declared_references_fetched": not failures and not parameter_failures,
            "source_acquisition": relative(acquisition_path), "producers": producers,
            "product_locators": locators, "parameter_locators": parameter_locators,
            "parameter_failures": parameter_failures,
            "requests": requests, "unresolved": failures + parameter_failures,
            "refreshed_product_ids": sorted(selected),
            "retained_product_ids": sorted({item["product_id"] for item in retained_requests}),
            "target_execution": False}
    write_json(receipt_path, data)
    families.index_rows(files, receipt_path)
    if refresh_ledger:
        analysis.refresh_gaps()
    if not caller_stable:
        raise ValueError("Dashboard artwork caller changed during acquisition")
    available = [request for request in requests if request["original"].get("http_status") == 200
                 and request["url"] in stable_urls]
    mappings = {request["original"]["path"]:
                f"synapse/current-dashboard-{request['product_id']}-{request['edition_id']}-"
                + (f"filename-{request['filename_edition_id']}-"
                   if request['filename_edition_id'] != request['edition_id'] else "")
                +
                f"{request['layout_id']}-{request['variant']}-{request['density']}x.png"
                for request in available}
    prepared = prepare_resources(relative(receipt_path), mappings)
    aliases = {entry["display"] for entry in prepared}
    rows = []
    for request in available:
        display = mappings[request["original"]["path"]]
        if display not in aliases:
            raise ValueError("Current Dashboard artwork preparation is incomplete")
        rows.append({"product_id": request["product_id"], "folder_edition_id": request["edition_id"],
                     "filename_edition_id": request["filename_edition_id"], "layout_id": request["layout_id"],
                     "wrist": request["variant"] == "wrist", "density": request["density"], "key": display})
    mappings = resource_sources.load_resource_mappings(root=ROOT)
    mappings["selectors"]["dashboard_plugin_image_selector"] = rows
    resource_sources.save_resource_mappings(mappings, root=ROOT)
    print(json.dumps({"dashboard_artwork_prepared": len(rows), "failed_requests": len(failures),
                      "receipt": relative(receipt_path)}), flush=True)
    favicon_result = prepare_host_favicons()
    print(json.dumps({key: value for key, value in favicon_result.items() if key != "prepared"}), flush=True)


def asset_pack_key(key):
    if (not isinstance(key, str) or not key or len(key.encode("utf-8")) > 4096
            or any(ord(character) < 32 or 127 <= ord(character) <= 159 or character in "\\:" for character in key)
            or any(part in {"", ".", ".."} for part in key.split("/"))):
        raise ValueError(f"Noncanonical pack key: {key!r}")
    return key.encode("utf-8")


def asset_pack_identity(row):
    name = row.get("path")
    if not isinstance(name, str) or not name or PureWindowsPath(name).anchor or "\\" in name:
        raise ValueError("Pack input must be project-relative")
    path = safe(ROOT / name)
    if relative(path) != name or not path.is_file():
        raise ValueError(f"Pack input path is redirected or absent: {name}")
    expected_bytes, expected_hash = row.get("bytes"), row.get("sha256")
    if (type(expected_bytes) is not int or expected_bytes < 0
            or not isinstance(expected_hash, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_hash)):
        raise ValueError(f"Pack input has no byte identity: {name}")
    if path.stat().st_size != expected_bytes or digest(path) != expected_hash:
        raise ValueError(f"Pack input changed: {name}")
    return path


def asset_pack_manifest_inputs(manifest_path):
    raw = manifest_path.read_bytes()
    manifest = json.loads(raw)
    if (manifest.get("schema_version") != 1 or manifest.get("inputs_unchanged") is not True
            or manifest.get("static_pack_ready") is not True or manifest.get("conflicting_aliases")):
        raise ValueError("Pack manifest is incomplete, changed, or has conflicting aliases")
    entries = manifest.get("ordered_load_entries")
    source_inputs = manifest.get("source_inputs")
    if not isinstance(entries, list) or not entries or not isinstance(source_inputs, list) or not source_inputs:
        raise ValueError("Pack manifest lacks static entries or source snapshots")
    if len(entries) > 1_000_000:
        raise ValueError("Pack entry count exceeds limit")
    keys, originals = set(), {}
    for entry in entries:
        asset_pack_key(entry.get("key"))
        if entry["key"] in keys:
            raise ValueError("Duplicate pack manifest key: " + entry["key"])
        keys.add(entry["key"])
        path = asset_pack_identity(entry)
        name = relative(path)
        if not (name.startswith("assets/") or name.startswith(".analysis/results/asset-pack-staging/")):
            raise ValueError("Non-runtime or original analysis input cannot become a pack entry: " + name)
        if entry["bytes"] > 256 * 1024 * 1024:
            raise ValueError("Pack entry exceeds byte limit: " + entry["key"])
        originals[name] = entry
    for item in source_inputs:
        asset_pack_identity(item)
        previous = originals.get(item["path"])
        if previous and (previous["bytes"], previous["sha256"]) != (item["bytes"], item["sha256"]):
            raise ValueError("Conflicting input identities in pack manifest")
        originals[item["path"]] = item
    return manifest, entries, list(originals.values()), hashlib.sha256(raw).hexdigest()


def asset_pack_wire_index(entries):
    ordered = sorted(entries, key=lambda entry: asset_pack_key(entry["key"]))
    if len({entry["key"] for entry in ordered}) != len(ordered):
        raise ValueError("Duplicate pack key")
    unique = {}
    for entry in ordered:
        previous = unique.setdefault(entry["sha256"], entry)
        if previous["bytes"] != entry["bytes"]:
            raise ValueError("Pack content hash collision")
    blobs = [unique[sha] for sha in sorted(unique)]
    identifiers = {entry["sha256"]: index for index, entry in enumerate(blobs)}
    index = bytearray()
    for entry in ordered:
        key = asset_pack_key(entry["key"])
        index.extend(struct.pack("<I", len(key)))
        index.extend(key)
        index.extend(struct.pack("<I", identifiers[entry["sha256"]]))
    index_length = len(index) + len(blobs) * 64
    if index_length > 64 * 1024 * 1024:
        raise ValueError("Pack index exceeds byte limit")
    offset = 96 + index_length
    for entry in blobs:
        index.extend(struct.pack("<IIQQQ32s", 0, 0, offset, entry["bytes"], entry["bytes"], bytes.fromhex(entry["sha256"])))
        offset += entry["bytes"]
    if offset > 16 * 1024 * 1024 * 1024:
        raise ValueError("Pack file exceeds byte limit")
    header = struct.pack("<8sIIQQQQQ32sQ", b"RZASPACK", 1, 0, len(ordered), len(blobs), index_length,
                         96 + index_length, offset, hashlib.sha256(index).digest(), 0)
    return header, index, blobs


def asset_pack_read_index(stream):
    stream.seek(0, os.SEEK_END)
    actual_length = stream.tell()
    stream.seek(0)
    header = stream.read(96)
    if len(header) != 96 or actual_length > 16 * 1024 * 1024 * 1024:
        raise ValueError("Invalid pack header or file length")
    magic, version, flags, key_count, blob_count, index_length, payload_start, file_length, index_hash, reserved = struct.unpack("<8sIIQQQQQ32sQ", header)
    if magic != b"RZASPACK" or version != 1 or flags != 0 or reserved != 0:
        raise ValueError("Unknown pack magic, version or header flags")
    if (file_length != actual_length or key_count > 1_000_000 or blob_count > key_count
            or (key_count == 0) != (blob_count == 0) or index_length > 64 * 1024 * 1024
            or payload_start != 96 + index_length or payload_start > actual_length
            or index_length < key_count * 9 + blob_count * 64):
        raise ValueError("Invalid pack counts, index or file bounds")
    index = stream.read(index_length)
    if len(index) != index_length or hashlib.sha256(index).digest() != index_hash:
        raise ValueError("Pack index is truncated or has a SHA-256 mismatch")
    position, keys, used = 0, {}, set()
    previous = None
    for _ in range(key_count):
        if position + 4 > len(index):
            raise ValueError("Truncated pack key length")
        key_length = struct.unpack_from("<I", index, position)[0]
        position += 4
        if not 1 <= key_length <= 4096 or position + key_length + 4 > len(index):
            raise ValueError("Invalid pack key length")
        raw_key = index[position:position + key_length]
        key = raw_key.decode("utf-8")
        asset_pack_key(key)
        position += key_length
        blob_index = struct.unpack_from("<I", index, position)[0]
        position += 4
        if previous is not None and previous >= raw_key:
            raise ValueError("Duplicate or unsorted pack key")
        if blob_index >= blob_count:
            raise ValueError("Pack key refers to an absent blob")
        keys[key], previous = blob_index, raw_key
        used.add(blob_index)
    blobs, expected_offset, previous_hash = [], payload_start, None
    for _ in range(blob_count):
        if position + 64 > len(index):
            raise ValueError("Truncated pack blob record")
        codec, flags, offset, stored_length, decoded_length, sha = struct.unpack_from("<IIQQQ32s", index, position)
        position += 64
        if codec != 0 or flags != 0:
            raise ValueError(f"Unknown pack codec or blob flags: {codec}/{flags}")
        if (stored_length > 256 * 1024 * 1024 or stored_length != decoded_length
                or offset != expected_offset or offset + stored_length > file_length
                or previous_hash is not None and previous_hash >= sha):
            raise ValueError("Invalid pack blob ordering, overlap, length or hash duplication")
        blobs.append({"offset": offset, "bytes": stored_length, "sha256": sha.hex()})
        expected_offset, previous_hash = offset + stored_length, sha
    if position != len(index) or expected_offset != file_length or len(used) != blob_count:
        raise ValueError("Pack has trailing index/data bytes or unreferenced blobs")
    return keys, blobs, {"entry_count": key_count, "blob_count": blob_count, "index_bytes": index_length,
                         "payload_bytes": file_length - payload_start, "file_bytes": file_length}


def verify_asset_pack_file(path, expected=None):
    with path.open("rb") as stream:
        keys, blobs, summary = asset_pack_read_index(stream)
        for blob in blobs:
            stream.seek(blob["offset"])
            remaining, actual = blob["bytes"], hashlib.sha256()
            while remaining:
                chunk = stream.read(min(1024 * 1024, remaining))
                if not chunk:
                    raise ValueError("Pack payload is truncated")
                actual.update(chunk)
                remaining -= len(chunk)
            if actual.hexdigest() != blob["sha256"]:
                raise ValueError("Pack payload SHA-256 mismatch")
    if expected is not None:
        if set(keys) != {entry["key"] for entry in expected}:
            raise ValueError("Pack key set differs from publication manifest")
        for entry in expected:
            blob = blobs[keys[entry["key"]]]
            if (blob["bytes"], blob["sha256"]) != (entry["bytes"], entry["sha256"]):
                raise ValueError("Pack alias differs from publication manifest: " + entry["key"])
    return summary


def prepare_asset_pack(manifest_name, output_name):
    verify_asset_pack_wire_fixture()
    manifest_path = safe(ROOT / manifest_name)
    manifest, entries, originals, manifest_hash = asset_pack_manifest_inputs(manifest_path)
    output = safe(ROOT / output_name)
    if relative(output) != GENERATED_ASSET_PACK and not relative(output).startswith(".analysis/results/asset-pack/"):
        raise ValueError("Pack output must be " + GENERATED_ASSET_PACK + " or its dedicated analysis staging directory")
    if output.suffix != ".pack" or relative(output) in {entry["path"] for entry in originals}:
        raise ValueError("Pack output cannot overwrite any source input")
    header, index, blobs = asset_pack_wire_index(entries)
    representatives = {entry["sha256"]: entry for entry in blobs}
    compared = set()
    for entry in entries:
        reference = representatives[entry["sha256"]]
        pair = (entry["path"], reference["path"])
        if pair[0] == pair[1] or pair in compared:
            continue
        compared.add(pair)
        with safe(ROOT / pair[0]).open("rb") as left, safe(ROOT / pair[1]).open("rb") as right:
            while True:
                left_chunk, right_chunk = left.read(1024 * 1024), right.read(1024 * 1024)
                if left_chunk != right_chunk:
                    raise ValueError("Pack content hash collision or concurrent source change")
                if not left_chunk:
                    break
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="wb", dir=output.parent, prefix=output.name + ".", suffix=".tmp", delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(header)
            stream.write(index)
            for entry in blobs:
                actual, count = hashlib.sha256(), 0
                with safe(ROOT / entry["path"]).open("rb") as source:
                    for chunk in iter(lambda: source.read(1024 * 1024), b""):
                        actual.update(chunk)
                        count += len(chunk)
                        if count > entry["bytes"]:
                            raise ValueError("Pack source grew while copying: " + entry["path"])
                        stream.write(chunk)
                if count != entry["bytes"] or actual.hexdigest() != entry["sha256"]:
                    raise ValueError("Pack source changed while copying: " + entry["path"])
            stream.flush()
            os.fsync(stream.fileno())
        summary = verify_asset_pack_file(temporary, entries)
        for item in originals:
            asset_pack_identity(item)
        if digest(manifest_path) != manifest_hash:
            raise ValueError("Pack publication manifest changed during preparation")
        package_hash = digest(temporary)
        os.replace(temporary, output)
        temporary = None
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()
    report = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "manifest": {"path": relative(manifest_path), "sha256": manifest_hash},
              "pack": {"path": relative(output), "sha256": package_hash, "bytes": summary["file_bytes"]},
              "summary": summary, "static_entries_verified": len(entries), "inputs_unchanged": True,
              "codec": "store", "atomic_publication": True,
              "production_loader_changed": manifest.get("production_loader_changed", False),
              "excluded_contracts": manifest.get("special_providers", []),
              "direct_runtime_input_count": len(manifest.get("direct_runtime_inputs", [])),
              "runtime_acceptance": "not performed", "passed": True}
    write_json(ROOT / ".analysis/results/asset-pack/preparation.json", report)
    return report


def verify_asset_pack(pack_name, manifest_name):
    verify_asset_pack_wire_fixture()
    path, manifest_path = safe(ROOT / pack_name), safe(ROOT / manifest_name)
    _, entries, originals, manifest_hash = asset_pack_manifest_inputs(manifest_path)
    before_hash = digest(path)
    summary = verify_asset_pack_file(path, entries)
    for item in originals:
        asset_pack_identity(item)
    if digest(manifest_path) != manifest_hash or digest(path) != before_hash:
        raise ValueError("Pack or publication manifest changed during verification")
    return {"pack": relative(path), "sha256": before_hash, "summary": summary,
            "static_entries_verified": len(entries), "inputs_unchanged": True, "passed": True}


def verify_asset_pack_wire_fixture():
    import io
    rows = [{"key": key, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            for key, data in (("a", b"abc"), ("alias", b"abc"), ("zero", b""))]
    header, index, blobs = asset_pack_wire_index(rows)
    content = {hashlib.sha256(data).hexdigest(): data for data in (b"abc", b"")}
    actual = header + index + b"".join(content[blob["sha256"]] for blob in blobs)
    fixture = ROOT / "crates/razer-asset-pack/testdata/store-v1.hex"
    if actual != bytes.fromhex(fixture.read_text(encoding="utf-8")):
        raise ValueError("Python pack preparation differs from the Rust wire fixture")
    keys, decoded, summary = asset_pack_read_index(io.BytesIO(actual))
    for entry in rows:
        blob = decoded[keys[entry["key"]]]
        if actual[blob["offset"]:blob["offset"] + blob["bytes"]] != content[entry["sha256"]]:
            raise ValueError("Pack wire fixture alias did not decode correctly")
    return summary


def validate_rust(selected_suites=None, cargo_config=None):
    """Compile all targets and run only pure Rust / simulated library suites."""
    baseline = rust_inputs()
    destination = ROOT / ".analysis/verification"
    destination.mkdir(parents=True, exist_ok=True)
    commands = [
        ("ipc-client", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "client_tests"]),
        ("controller-trigger-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "controller_trigger"]),
        ("controller-trigger-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_products::trigger_runtime::tests"]),
        ("controller-profile", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_products::profile::tests"]),
        ("controller-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "controller_"]),
        ("audio-demo-media", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "media::tests"]),
        ("screen-color-platform", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "screen_color"]),
        ("screen-color-ui", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "screen_color"]),
        ("audio-demo-floating", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "demo::tests"]),
        ("source-feature-transport", ["cargo", "test", "--locked", "--offline", "-p", "razer-hid", "--lib"]),
        ("audio-mixer-feature-transport", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "source_feature_tests"]),
        ("app-picker-current", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "current_app_picker_"]),
        ("source-catalog", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_catalog::tests"]),
        ("controller-input", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "controller_input"]),
        ("controller-trigger-visualization", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "trigger_visualization"]),
        ("controller-navigation", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_products::navigation::tests"]),
        ("source-dpi-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_dpi_owner_drop"]),
        ("shell-dpi-stage-persistence", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "stage_save"]),
        ("shell-persisted-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "persisted_"]),
        ("camera-preview", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "camera_preview"]),
        ("camera-media-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_media::tests"]),
        ("camera-source-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_source_state::tests"]),
        ("camera-source-runtime", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_runtime::tests"]),
        ("camera-mic-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_mic::tests"]),
        ("camera-mic-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "camera_mic"]),
        ("startup-platform", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "startup::tests"]),
        ("startup-owner", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "startup::tests"]),
        ("restart-notification-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-dashboard", "--lib", "restart_notification::tests"]),
        ("restart-notification-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "restart_notification::tests"]),
        ("mouse-dpi-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "dpi_70::tests"]),
        ("storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib"]),
        ("shell-store-migration", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "store::tests"]),
        ("catalog-registry", ["cargo", "test", "--locked", "--offline", "-p", "razer-catalog", "--lib", "registry::tests"]),
        ("emoji-search", ["cargo", "test", "--locked", "--offline", "-p", "razer-catalog", "--lib", "emoji::tests"]),
        ("resource-database", ["cargo", "test", "--locked", "--offline", "-p", "razer-resource-db", "--lib"]),
        ("resource-database-index", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "remote::database_tests"]),
        ("packaged-catalogs", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "-p", "razer-app-pages", "-p", "razer-assets", "-p", "razer-model", "-p", "razer-catalog", "-p", "razer-shell", "-p", "razer-tray", "-p", "razer-settings", "--lib", "packaged_catalog_"]),
        ("model", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib"]),
        ("i18n", ["cargo", "test", "--locked", "--offline", "-p", "razer-i18n", "--lib"]),
        ("discovery", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib"]),
        ("mixer-shortcut-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "audio_shortcut_lifecycle_tests"]),
        ("macro-shortcut-resume", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "shortcut_resume_tests"]),
        ("startup-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "-p", "razer-dashboard", "-p", "razer-device", "-p", "razer-discovery", "-p", "razer-shell", "--lib", "startup_state"]),
        ("hid-inventory", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "hid::inventory_tests"]),
        ("device", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib"]),
        ("mouse-idle-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_polling_profile::idle_tests"]),
        ("mouse-receiver-polling", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_receiver_polling::tests"]),
        ("mouse-scroll-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_scroll::tests"]),
        ("mouse-scroll-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mouse_scroll_tests"]),
        ("mouse-scroll-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "mouse_scroll_tests"]),
        ("mouse-scroll-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "scroll_task::tests"]),
        ("mouse-scroll-persistence", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_scroll::tests"]),
        ("mouse-profile-source", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_profile_source::tests"]),
        ("keyboard-metadata", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_metadata::tests"]),
        ("hid-interrupt", ["cargo", "test", "--locked", "--offline", "-p", "razer-hid", "--lib", "native::tests"]),
        ("receiver-events-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "receiver_events::tests"]),
        ("receiver-reconnect-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "receiver_reconnect::tests"]),
        ("mouse-runtime-artwork-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mouse_runtime_artwork_controller"]),
        ("mouse-runtime-artwork-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_runtime_artwork"]),
        ("mouse-runtime-artwork-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "runtime_artwork_tests"]),
        ("mouse-current-power-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_power::tests"]),
        ("mouse-current-power-migration", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_current_power_migration"]),
        ("mouse-connection-navigation", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_connection_navigation"]),
        ("seiren-eq-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "seiren_eq"]),
        ("seiren-audio-capture-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "seiren_audio_capture"]),
        ("seiren-audio-capture-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "seiren_audio_capture::tests"]),
        ("settings-products", ["cargo", "test", "--locked", "--offline", "-p", "razer-settings", "--lib", "products::tests"]),
        ("settings-runtime-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-settings", "--lib", "runtime_page::tests::", "--", "--skip", "runtime_page::tests::partial_enumeration_is_visible_without_starting_a_service", "--skip", "runtime_page::tests::connection_page_does_not_start_services_on_render", "--test-threads=1"]),
        ("shell-save-queue", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "save_queue_tests::", "--", "--test-threads=1"]),
        ("display-window-contract", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "display_window::tests"]),
        ("keyboard-wristrest-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "wristrest_controller::tests"]),
        ("keyboard-wristrest-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "wristrest::tests"]),
        ("mapping-turbo-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "turbo_persistence_accepts_existing_numeric_values"]),
        ("receiver-shell-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "receiver_pairing"]),
        ("dashboard-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-dashboard", "--lib", "artwork_tests"]),
        ("dashboard-preview-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-dashboard", "--lib", "preview_artwork_does_not_replace_source_dashboard_images_or_live_device_failures"]),
        ("shell-preview-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "every_catalog_preview_variant_keeps_its_dashboard_artwork", "--", "--nocapture"]),
        ("shell-product-favicons", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "every_registered_product_tab_has_a_loadable_source_favicon"]),
        ("audio-mixer-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "audio_mixer"]),
        ("audio-sound-mapping", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "sound_mapping::tests"]),
        ("keyboard-dynamic-metadata", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "all_current_dynamic_mapping_metadata_loads"]),
        ("keyboard-source-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_artwork::tests"]),
        ("gamepad-source-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_products::customize_artwork::tests"]),
        ("gamepad-trigger-assets", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "asset_decode_tests::current_gamepad_trigger_resources_have_source_identities"]),
        ("gamepad-trigger-tooltip", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_tooltip::tests::gamepad_rapid_trigger_help_uses_source_measured_vertical_fallback"]),
        ("hyperflux-pairing-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "hyperflux_207::artwork::tests"]),
        ("keyboard-analog-controls", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "keyboard_products::analog_controls::tests"]),
        ("keyboard-gaming-combinations", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "keyboard_products::gaming_combined::tests"]),
        ("receiver-pairing-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "receiver::pairing_state::tests"]),
        ("receiver-discovery-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "receiver_pairing::tests"]),
        ("receiver-discovery-projection", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "receiver_projection::tests"]),
        ("receiver-service-metadata", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "receiver_pairing::tests"]),
        ("standalone-pairing-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "pairing_page::tests", "--",
                                     "--skip", "initial_pairing_loading_has_no_unmounted_toolbar_or_status_messages",
                                     "--skip", "confirmation_escape_closes_without_invented_focus_restore_and_enter_cannot_fake_binding",
                                     "--skip", "inline_confirmation_uses_content_height_and_a_centered_compact_button"]),
        ("thx-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "thx"]),
        ("thx-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "thx"]),
        ("camera-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "camera"]),
        ("user-app-files-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "simulated_native_write_retains_bytes_and_failed_callback_without_poisoning"]),
        ("camera-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "camera"]),
        ("camera-image-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_image_state::tests"]),
        ("camera-exposure-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_exposure_state::tests"]),
        ("source-slider", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_slider::tests"]),
        ("source-hypershift-tooltip", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "hypershift_geometry_tests"]),
        ("source-text-layout", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_check_text_preserves_max_content_width"]),
        ("source-locale-choices", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_locale_choices"]),
        ("retained-locale-choices", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "retained_source_choices_translate"]),
        ("mouse-inter-device-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mapping_inter_70::locale_tests"]),
        ("keyboard-inter-device-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mapping_drawer::inter_device::locale_tests"]),
        ("gamepad-number-layout", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "trigger_range_tip_preserves_intrinsic_single_line_width_inside_zero_width_anchor"]),
        ("audio-number-layout", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "audio_mixer_number::tests"]),
        ("mouse-svg-button-motion", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "svg_button_motion::tests"]),
        ("mouse-calibration-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "panel_transition_tests"]),
        ("mouse-canvas-projection", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "pid70_canvas_projection"]),
        ("assets", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib"]),
        ("asset-pack-core", ["cargo", "test", "--locked", "--offline", "-p", "razer-asset-pack", "--lib"]),
        ("assets-resource-identity", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "asset_decode_tests"]),
        ("assets-resource-runtime", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "remote::"]),
        ("assets-fixed-icons", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "icons::tests"]),
        ("workspace", ["cargo", "check", "--locked", "--offline", "--workspace", "--all-targets"]),
    ]
    available = {name for name, _ in commands}
    if selected_suites:
        unknown = set(selected_suites) - available
        if unknown:
            raise ValueError(f"Unknown Rust verification suites: {sorted(unknown)}; available: {sorted(available)}")
        commands = [(name, command) for name, command in commands if name in selected_suites]
    # Catch all-target compilation failures before repeatedly linking the
    # selected pure suites. Keep the relative order of the test suites.
    commands.sort(key=lambda item: item[0] != "workspace")
    if cargo_config:
        # Cargo options belong before any test-harness `--` separator. Record
        # exact overrides with argv; never modify the project's build config.
        options = [part for value in cargo_config for part in ("--config", value)]
        commands = [(name, command[:2] + options + command[2:]) for name, command in commands]
    results = []
    for name, command in commands:
        print(json.dumps({"checking": name, "argv": command}), flush=True)
        log = destination / (name + ".txt")
        started = datetime.now(timezone.utc).isoformat()
        with verification_environment(name) as (environment, temporary_directory):
            with log.open("wb") as output:
                completed = subprocess.run(command, cwd=ROOT, env=environment, stdout=output,
                                           stderr=subprocess.STDOUT, check=False)
        raw = log.read_text(encoding="utf-8", errors="replace")
        warning_lines = re.findall(r"^warning(?:\[[^\]]+\])?:.*$", raw, re.MULTILINE)
        totals = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored", raw)
        executed_tests = sum(int(passed) + int(failed) for _, passed, failed, _ in totals)
        errors = []
        if completed.returncode != 0:
            errors.append("command_failed")
        if command[1] == "test" and executed_tests == 0:
            errors.append("no_tests_executed")
        row = {"name": name, "argv": command, "started_at_utc": started,
               "temporary_directory": temporary_directory,
               "temporary_directory_variables": ["TEMP", "TMP", "TMPDIR"],
               "finished_at_utc": datetime.now(timezone.utc).isoformat(),
               "exit_code": completed.returncode, "log": relative(log), "log_sha256": digest(log),
               "executed_tests": executed_tests, "errors": errors, "passed": not errors,
               "warning_count": len(warning_lines), "warning_lines": warning_lines,
               "test_summaries": [{"result": state, "passed": int(passed), "failed": int(failed),
                                   "ignored": int(ignored)} for state, passed, failed, ignored in totals]}
        results.append(row)
        print(json.dumps(row), flush=True)
    unchanged = baseline == rust_inputs()
    receipt = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
               "inputs": baseline, "inputs_unchanged": unchanged, "commands": results,
               "suite_selection": [name for name, _ in commands],
               "all_configured_suites": selected_suites is None,
               "warning_count": sum(row["warning_count"] for row in results),
               "target_execution": False, "real_device_operations": False,
               "application_runtime_acceptance": "not performed",
               "passed": unchanged and all(row["passed"] for row in results)}
    write_json(ROOT / ".analysis/validation.json", receipt)
    if not receipt["passed"]:
        raise ValueError("Validation failed or its inputs changed; inspect .analysis/validation.json and logs.")


def asset_pack_manifest(*, prepare=False):
    import families
    owner = ROOT / "crates/razer-assets/src/lib.rs"
    tables_owner = ROOT / resource_sources.MAPPINGS
    mappings = resource_sources.load_resource_mappings(root=ROOT)
    source_inputs = {}
    payloads = {}

    def snapshot(path):
        path = safe(path)
        name = relative(path)
        record = {"path": name, "bytes": path.stat().st_size, "sha256": digest(path)}
        if name in source_inputs and source_inputs[name] != record:
            raise ValueError("Asset input changed during enumeration: " + name)
        source_inputs[name] = record
        return record

    def payload(key, path, provider):
        path = product_input(path, owner)
        record = snapshot(path)
        payloads[record["path"]] = record
        return {"key": key, **record, "provider": provider}

    snapshot(owner)
    snapshot(tables_owner)
    snapshot(Path(__file__))
    snapshot(Path(families.__file__))
    names = [table["name"] for table in mappings["tables"]]
    table_entries = [payload(row["key"], ROOT / row["path"], table["name"])
                     for table in mappings["tables"] for row in table["entries"]]
    demo_entries = [payload(row["key"], ROOT / row["path"], "audio_demo_controls")
                    for row in mappings["audio_demo_controls"]]
    sprite_path = tables_owner
    row = mappings["audio_volume_sprite"]
    sprite = payload(row["key"], ROOT / row["path"], "audio_volume_sprite_base")
    all_entries = [sprite, *demo_entries, *table_entries]
    first = {}
    duplicates, conflicts = [], []
    for entry in all_entries:
        if entry["key"] in first:
            previous = first[entry["key"]]
            duplicate = {"key": entry["key"], "selected": previous, "shadowed": entry,
                         "same_bytes": previous["sha256"] == entry["sha256"] and previous["bytes"] == entry["bytes"]}
            duplicates.append(duplicate)
            if not duplicate["same_bytes"]:
                conflicts.append(duplicate)
        else:
            first[entry["key"]] = entry
    ordered = list(first.values())
    table_list = [entry["key"] for entry in table_entries]
    table_list_path = ROOT / "assets/synapse/indexes/asset-list.json"
    if prepare:
        write_json(table_list_path, table_list)
    table_list_matches = table_list_path.exists() and json.loads(table_list_path.read_bytes()) == table_list
    if table_list_path.exists():
        snapshot(table_list_path)
    fonts = [snapshot(product_input(ROOT / name, owner)) for name in resource_sources.REMOTE_FONT_PATHS]
    runtime_inputs, fixture_inputs, mixed_inputs = [], [], []
    rust_snapshot = rust_inputs(include_asset_pack=False)
    for entry in rust_snapshot:
        path = Path(entry["path"])
        if path.suffix in {".rs", ".toml"} or entry["path"] in payloads:
            continue
        if "testdata" in path.parts or "tests" in path.parts:
            fixture_inputs.append(entry)
        elif path.parts[0] == "locales":
            runtime_inputs.append({**entry, "role": "compiled_locale_dictionary"})
        else:
            mixed_inputs.append({**entry, "role": "direct_compile_input_requires_cfg_and_owner_audit_before_pack_migration"})
    referenced_assets = {entry["path"] for entry in source_inputs.values() if entry["path"].startswith("assets/")}
    referenced_assets.update(entry["path"] for entry in rust_snapshot if entry["path"].startswith("assets/"))
    unreferenced = [{"path": relative(path), "bytes": path.stat().st_size,
                     "role": "not_reachable_from_current_rust_literal_include_graph_not_authorized_for_deletion"}
                    for path in sorted((ROOT / "assets").rglob("*"))
                    if path.is_file() and relative(path) not in referenced_assets and relative(path) != "assets/synapse.pack"]
    for row in rust_snapshot:
        source_inputs.setdefault(row["path"], row)
    unchanged = all((ROOT / row["path"]).is_file() and (ROOT / row["path"]).stat().st_size == row["bytes"]
                    and digest(ROOT / row["path"]) == row["sha256"] for row in source_inputs.values())
    content = {entry["sha256"]: entry["bytes"] for entry in ordered}
    manifest = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
                "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__))},
                "source_inputs": sorted(source_inputs.values(), key=lambda row: row["path"]),
                "ordered_load_entries": ordered,
                "list_entries": [entry["key"] for entry in demo_entries] + [sprite["key"], sprite["key"] + "#link", sprite["key"] + "#hover"] + [entry["key"] for entry in table_entries],
                "list_semantics": {"prefix": "Dependency gpui_kit::assets::AllAssets.list(path) remains first and is not repackaged",
                                   "audio_demo_controls": "Only empty, synapse, or synapse/ prefix returns these four aliases",
                                   "sprite_and_tables": "starts_with(path); retains duplicate aliases in original order"},
                "load_semantics": ["audio_volume_sprite", "audio_demo_controls", "ASSET_TABLES ordered first match", "gpui_kit::assets::AllAssets fallback"],
                "special_providers": [{"provider": "audio_volume_sprite", "source": relative(sprite_path),
                                       "base_key": sprite["key"], "dynamic_keys": [sprite["key"] + "#link", sprite["key"] + "#hover"],
                                       "pack_policy": "Only exact original SVG bytes; retain runtime viewBox insertion for fragment aliases"}],
                "font_inputs": fonts, "direct_runtime_inputs": runtime_inputs,
                "table_list": {"path": relative(table_list_path), "matches": table_list_matches,
                               "keys": len(table_list), "duplicates_preserved": True},
                "direct_inputs_pending_owner_classification": mixed_inputs, "test_fixture_inputs": fixture_inputs,
                "unreferenced_assets": unreferenced,
                "production_pack": {"path": GENERATED_ASSET_PACK, "installed_path": "assets/synapse.pack",
                                    "role": "generated_runtime_package",
                                    "verification": "verify-asset-pack; excluded from its own generator inputs"},
                "original_evidence_policy": ".analysis/inputs and .analysis/results are analysis-only; no original evidence or reports enter the production payload",
                "duplicate_aliases": duplicates, "conflicting_aliases": conflicts,
                "summary": {"tables": len(names), "table_aliases": len(table_entries), "unique_static_aliases": len(ordered),
                            "duplicate_aliases": len(duplicates), "conflicting_aliases": len(conflicts),
                            "unique_blobs": len(content), "alias_bytes": sum(entry["bytes"] for entry in ordered),
                            "deduplicated_bytes": sum(content.values()), "fonts": len(fonts),
                            "test_fixtures": len(fixture_inputs), "unreferenced_assets": len(unreferenced)},
                "inputs_unchanged": unchanged, "static_pack_ready": unchanged and not conflicts and table_list_matches,
                "production_loader_changed": tables_owner != owner, "runtime_acceptance": "unverified"}
    write_json(ROOT / ".analysis/results/asset-pack-manifest.json", manifest)
    return manifest


def main():
    import analysis
    import asset_release
    import evidence
    import families
    import windows_installer
    import typed_tables
    import rust_data
    import resource_database
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("inventory", "clean", "verify", "audit-inputs", "recover-inputs", "prepare-analysis", "prepare-static-python", "verify-resources", "prepare-product-names", "prepare-keyboard-identity"):
        commands.add_parser(name)
    pack_audit = commands.add_parser("audit-asset-pack")
    pack_audit.add_argument("--prepare", action="store_true", help="Generate the exact ordered production asset-list.json")
    validation = commands.add_parser("validate-rust")
    validation.add_argument("--suite", action="append", help="Run an applicable pure/mock suite; repeat to select multiple suites")
    validation.add_argument("--cargo-config", action="append", help="Per-command Cargo configuration override, retained in verification argv")
    pack_prepare = commands.add_parser("prepare-asset-pack")
    pack_prepare.add_argument("--manifest", default=".analysis/results/asset-pack-manifest.json")
    pack_prepare.add_argument("--output", default=GENERATED_ASSET_PACK)
    pack_verify = commands.add_parser("verify-asset-pack")
    pack_verify.add_argument("--manifest", default=".analysis/results/asset-pack-manifest.json")
    pack_verify.add_argument("--pack", default=GENERATED_ASSET_PACK)
    commands.add_parser("prepare-image-analyzer", help="Verify a project-local Pillow wheel for the active Python ABI")
    commands.add_parser("prepare-media-analyzer", help="Verify a project-local PyAV wheel for the active Python ABI")
    native_cursors = commands.add_parser("prepare-native-cursors", help="Extract verified PE cursor groups without loading DLLs")
    native_cursors.add_argument("--sources", required=True, help="JSON list of path/acquisition records; '-' reads stdin")
    native_cursors.add_argument("--group", type=int, help="Extract only this numeric RT_GROUP_CURSOR identifier")
    resource = commands.add_parser("prepare-resources")
    resource.add_argument("--acquisition", required=True)
    resource.add_argument("--resources", required=True, help="JSON map of original paths to product display aliases; '-' reads stdin")
    locale_keys = commands.add_parser("prepare-locale-keys")
    locale_keys.add_argument("--acquisition", required=True)
    locale_keys.add_argument("--namespace", required=True)
    locale_keys.add_argument("--bindings", required=True, help="JSON locale-to-original-path map; '-' reads stdin")
    locale_keys.add_argument("--key", action="append", required=True)
    commands.add_parser("prepare-host-favicons", help="Verify all current product HTML first favicons and attach existing bytes to resource provenance")
    commands.add_parser("prepare-app-picker", help="Prepare current mounted app-menu category vectors and native catalog presentation fallbacks")
    deduplicate = commands.add_parser("deduplicate-resources")
    deduplicate.add_argument("--apply", action="store_true", help="Apply only an unchanged, previously reviewed dry-run")
    deduplicate.add_argument("--plan", default=".analysis/results/resource-deduplication.json")
    organize = commands.add_parser("organize-resources")
    organize.add_argument("--apply", action="store_true", help="Archive resource tables/manifests/indexes and update all physical references")
    source_layout = commands.add_parser("organize-sources", help="Group crate source files by capability while preserving Rust module APIs")
    source_layout.add_argument("--apply", action="store_true", help="Move files after resolving and checking all module/include paths")
    commands.add_parser("audit-lfs", help="Verify index pointers and local LFS objects without staging or modifying Git")
    dashboard_images = commands.add_parser("prepare-dashboard-images")
    dashboard_images.add_argument("--workers", type=int, default=8)
    dashboard_images.add_argument("--no-refresh-gaps", action="store_true", help="Leave global ledger refresh to the owning batch")
    dashboard_images.add_argument("--source-preview-only", action="store_true", help="Rebuild current product resource contexts without repeating Dashboard image acquisition")
    dashboard_images.add_argument("--product", type=int, action="append", help="Refresh selected products while retaining other verified requests and explicit failures")
    analysis.add_commands(commands)
    asset_release.add_commands(commands)
    windows_installer.add_commands(commands)
    typed_tables.add_commands(commands)
    rust_data.add_commands(commands)
    resource_database.add_commands(commands)
    for name in ("prepare-runtime-assets", "verify-runtime-assets"):
        runtime_assets = commands.add_parser(name)
        runtime_assets.add_argument("--check-local", action="store_true", help="Verify existing local bytes without fetching")
    evidence.add_commands(commands)
    families.add_commands(commands)
    args = parser.parse_args()
    if args.command == "inventory":
        rows = inventory()
        print(json.dumps({name: {"files": len(files), "bytes": sum(r["bytes"] for r in files)} for name, files in rows.items()}))
    elif args.command == "clean":
        clean()
    elif args.command == "verify":
        verify()
    elif args.command == "audit-inputs":
        report = audit_inputs()
        print(json.dumps({"input_audit": relative(INPUT_AUDIT), "passed": report["passed"],
                          "index": report["index"], **report["summary"]}), flush=True)
        return 0 if report["passed"] else 1
    elif args.command == "recover-inputs":
        report = recover_inputs()
        print(json.dumps({"input_recovery": relative(INPUT_RECOVERY), "passed": report["passed"],
                          "restored": report["restored"], "audit": report["audit"],
                          **report["audit_summary"]}), flush=True)
        return 0 if report["passed"] else 1
    elif args.command == "prepare-analysis":
        print(json.dumps(prepare_analysis(), ensure_ascii=False), flush=True)
    elif args.command == "connect-analysis":
        evidence.connect_analysis()
    elif args.command == "audit-ui-tree":
        evidence.audit_ui_tree()
    elif args.command == "audit-host-local":
        import host
        host.audit_host_local(args)
    elif args.command == "audit-mouse-receiver-runtime":
        families.audit_mouse_receiver_runtime(args)
    elif args.command == "audit-keyboard-linked-profiles":
        families.audit_keyboard_linked_profiles(args)
    elif args.command == "verify-analysis-tools":
        import test_evidence
        return test_evidence.run()
    elif args.command == "verify-resources":
        report = verify_resources()
        print(json.dumps({"resource_audit": relative(RESOURCE_AUDIT), "passed": report["passed"],
                          "inputs_unchanged": report["inputs_unchanged"], **report["summary"]}), flush=True)
        return 0 if report["passed"] else 1
    elif args.command == "audit-asset-pack":
        report = asset_pack_manifest(prepare=args.prepare)
        print(json.dumps({"report": ".analysis/results/asset-pack-manifest.json", "static_pack_ready": report["static_pack_ready"], **report["summary"]}), flush=True)
        return 0 if report["static_pack_ready"] else 1
    elif args.command == "validate-rust":
        validate_rust(args.suite, args.cargo_config)
    elif args.command == "prepare-asset-pack":
        print(json.dumps(prepare_asset_pack(args.manifest, args.output), ensure_ascii=False))
    elif args.command == "verify-asset-pack":
        print(json.dumps(verify_asset_pack(args.pack, args.manifest), ensure_ascii=False))
    elif args.command in {"prepare-asset-release", "verify-asset-release", "fetch-asset-release", "publish-asset-release", "pin-asset-release"}:
        print(json.dumps(asset_release.run(args), ensure_ascii=False), flush=True)
    elif args.command in {"prepare-device-write-table", "prepare-storage-tables", "prepare-local-protocol-tables"}:
        print(json.dumps(typed_tables.run(args), ensure_ascii=False), flush=True)
    elif args.command in {"prepare-typed-data", "prepare-emoji-search"}:
        print(json.dumps(rust_data.run(args), ensure_ascii=False), flush=True)
    elif args.command == "prepare-resource-database":
        print(json.dumps(resource_database.run(args), ensure_ascii=False), flush=True)
    elif args.command == "prepare-static-python":
        print(json.dumps(prepare_static_python(), ensure_ascii=False), flush=True)
    elif args.command == "prepare-runtime-assets":
        print(json.dumps(asset_release.prepare_runtime_metadata(check_local=args.check_local), ensure_ascii=False), flush=True)
    elif args.command == "verify-runtime-assets":
        print(json.dumps(asset_release.verify_runtime_metadata(check_local=args.check_local), ensure_ascii=False), flush=True)
    elif args.command in {"prepare-windows-installer", "verify-windows-installer", "compile-windows-installer"}:
        print(json.dumps(windows_installer.run(args), ensure_ascii=False), flush=True)
    elif args.command == "prepare-image-analyzer":
        prepare_image_analyzer()
    elif args.command == "prepare-media-analyzer":
        print(json.dumps(prepare_media_analyzer(), ensure_ascii=False), flush=True)
    elif args.command == "prepare-native-cursors":
        sources = json.loads(sys.stdin.read() if args.sources == "-" else args.sources)
        print(json.dumps(prepare_native_cursors(sources, args.group), ensure_ascii=False), flush=True)
    elif args.command == "prepare-resources":
        mappings = json.loads(sys.stdin.read() if args.resources == "-" else args.resources)
        print(json.dumps(prepare_resources(args.acquisition, mappings), ensure_ascii=False))
    elif args.command == "prepare-locale-keys":
        bindings = json.loads(sys.stdin.read() if args.bindings == "-" else args.bindings)
        print(json.dumps(prepare_locale_keys(args.acquisition, args.namespace, bindings, args.key), ensure_ascii=False))
    elif args.command == "prepare-host-favicons":
        print(json.dumps(prepare_host_favicons(), ensure_ascii=False))
    elif args.command == "prepare-app-picker":
        print(json.dumps(prepare_app_picker(), ensure_ascii=False))
    elif args.command == "deduplicate-resources":
        report = deduplicate_resources(apply=args.apply, plan=args.plan)
        print(json.dumps({"report": args.plan, "mode": report["mode"], **report["summary"]}, ensure_ascii=False))
    elif args.command == "organize-sources":
        organize_sources(apply=args.apply)
    elif args.command == "audit-lfs":
        report = audit_git_lfs()
        print(json.dumps({"report": ".analysis/results/git-lfs-audit.json", "passed": report["passed"], **report["summary"]}))
        return 0 if report["passed"] else 1
    elif args.command == "organize-resources":
        report = organize_resources(apply=args.apply)
        print(json.dumps({"applied": report.get("applied", False), **report["summary"]}, ensure_ascii=False))
    elif args.command == "prepare-product-names":
        print(json.dumps(prepare_product_names(), ensure_ascii=False))
    elif args.command == "prepare-keyboard-identity":
        print(json.dumps(prepare_keyboard_identity(), ensure_ascii=False))
    elif args.command == "prepare-dashboard-images":
        if not 1 <= args.workers <= 12:
            raise ValueError("Dashboard resource workers must be between one and twelve")
        prepare_dashboard_images(args.workers, refresh_ledger=not args.no_refresh_gaps,
                                 products=args.product, source_preview_only=args.source_preview_only)
    elif args.command == "audit-family-function":
        families.audit_family_function(args)
    elif args.command == "audit-runtime-setup":
        families.audit_runtime_setup(args)
    elif args.command == "audit-registry-identity":
        families.audit_registry_identity(args.prepare, args.product)
    elif args.command == "acquire-family":
        families.run(args, refresh=not args.no_refresh_gaps)
    else:
        analysis.run(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
