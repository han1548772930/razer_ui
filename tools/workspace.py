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
import threading
import tomllib
import urllib.error
import urllib.parse
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
            ("transport", ("backend", "device_query", "device_reads", "device_writes", "feature_exchange")),
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
                  ("native", ("dll",)),
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
                  ("windows", ("chroma_", "settings_window", "display_window", "firmware_update")))
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
        dependencies, external = [], []
        for kind, items in sections:
            for alias, specification in items.items():
                specification = shared[alias] if isinstance(specification, dict) and specification.get("workspace") else specification
                if isinstance(specification, dict) and "path" in specification:
                    dependencies.append({"name": specification.get("package", alias), "kind": kind})
                else:
                    external.append({"name": specification.get("package", alias) if isinstance(specification, dict) else alias, "kind": kind})
        source = ROOT / "app" if path.parent == ROOT else path.parent
        files = sorted(source.rglob("*.rs"))
        fixtures = sorted(item for item in (path.parent / "testdata").rglob("*") if item.is_file())
        packages.append({"name": manifest["package"]["name"], "manifest": relative(path),
                         "internal_dependencies": dependencies,
                         "external_dependencies": external,
                         "rust_files": len(files), "rust_bytes": sum(item.stat().st_size for item in files),
                         "rust_paths": [relative(item) for item in files],
                         "declared_targets": {key: manifest[key] for key in ("lib", "bin", "example", "test", "bench") if key in manifest},
                         "build_script": manifest["package"].get("build", "build.rs" if (path.parent / "build.rs").is_file() else None),
                         "test_fixture_inputs": [{"path": relative(item), "bytes": item.stat().st_size, "sha256": digest(item)} for item in fixtures],
                         "largest_sources": [{"path": relative(item), "bytes": item.stat().st_size}
                                             for item in sorted(files, key=lambda item: item.stat().st_size, reverse=True)[:3]]})
    identities = rust_inputs()
    embedded = [row for row in identities if row["path"].startswith("assets/")]
    sources = [path for package in (ROOT / "crates").iterdir() if package.is_dir() for path in (package / "src").rglob("*.rs")]
    obsolete = [relative(path) for path in sources if "/src/data/" in path.as_posix() or path.name == "data.rs"]
    stale_calls = [relative(path) for path in sources if re.search(r"crate::data::\w+(?:::\w+)?!\s*\(", path.read_text(encoding="utf-8"))]
    return {"packages": packages, "packages_scanned": len(packages), "input_identities": identities,
            "product_asset_inputs": embedded,
            "compiled_asset_inputs": [row for row in embedded if row["path"] != resource_sources.RUNTIME_METADATA],
            "installed_catalog_input": next((row for row in embedded if row["path"] == resource_sources.RUNTIME_METADATA), None),
            "obsolete_data_files": obsolete, "resource_macro_callers": stale_calls,
            "scope": "All workspace manifests, source/test/build Rust, crate fixtures, module/include closure and embedded input identities; not a source-to-device acceptance claim"}


def dependency_boundaries(packages):
    """Check normal, test, build and platform branches, including internal transitive edges."""
    presentation = {"razer_ui", "razer-app", "razer-shell", "razer-pages", "razer-app-pages", "razer-state",
                    "razer-widgets", "razer-assets", "razer-dashboard", "razer-settings", "razer-tray"}
    # The locked reqwest fork is an HTTP client and has no GPUI dependency.
    non_gui_gpui_packages = {"gpui-pre-reqwest"}
    domain = {"razer-device", "razer-model", "razer-catalog", "razer-i18n", "razer-resource-db"}
    foundations = {"razer-assets", "razer-widgets", "razer-state"}
    features = presentation - foundations - {"razer-app", "razer-shell", "razer_ui"}
    by_name = {row["name"]: row for row in packages}
    violations = []
    for package in packages:
        name = package["name"]
        pending, seen = [(name, [name])], set()
        while pending:
            current, chain = pending.pop()
            if current in seen or current not in by_name:
                continue
            seen.add(current)
            row = by_name[current]
            for edge in row.get("internal_dependencies", []) + row.get("external_dependencies", []):
                target = edge["name"]
                reason = None
                if name not in presentation and (target in presentation or (target not in non_gui_gpui_packages and (target == "gpui" or target.startswith(("gpui-", "gpui_"))))):
                    reason = "non_gui_owner_depends_on_presentation"
                elif name in domain and target.startswith("razer-") and target not in domain:
                    reason = "domain_depends_on_runtime_or_storage"
                elif name in foundations and target in features | {"razer-app", "razer-shell", "razer_ui"}:
                    reason = "presentation_foundation_depends_on_feature"
                elif target == "razer-shell" and name not in {"razer-app", "razer-shell", "razer_ui"}:
                    reason = "feature_depends_on_shell"
                elif target in {"razer-app", "razer_ui"} and name not in {"razer-app", "razer_ui"}:
                    reason = "feature_depends_on_application"
                if reason:
                    violations.append({"owner": name, "kind": reason, "chain": chain + [target], "dependency_kind": edge["kind"]})
                elif target in by_name:
                    pending.append((target, chain + [target]))
    return violations


def inspect_architecture_source(raw, *, owner, rust_parser, expression_fragment=False):
    """Inspect actual Rust items and calls, with byte locations for actionable diagnostics."""
    tree = rust_parser.parse(raw)
    violations = []
    gui_io_violations = []
    offset = 0
    if tree.root_node.has_error and expression_fragment:
        prefix = b"const ARCHITECTURE_INCLUDED_EXPRESSION: () = "
        wrapped = rust_parser.parse(prefix + raw + b";")
        if not wrapped.root_node.has_error:
            raw, tree, offset = prefix + raw + b";", wrapped, len(prefix)
    if tree.root_node.has_error:
        return [{"owner": owner, "kind": "architecture_rust_parse_error"}]
    factory_owner = "crates/razer-shell/src/shell/persistence.rs"
    publication_owner = "crates/razer-resource-db/src/publication.rs"
    resource_bridge = "crates/razer-assets/src/resource_runtime.rs"
    image_cache_owner = "crates/razer-assets/src/image_cache.rs"
    local_image_owner = "crates/razer-assets/src/local_image.rs"
    camera_page = (owner.startswith("crates/razer-pages/src/features/workspace/source_controls/")
                   and Path(owner).name.startswith("camera"))
    camera_workers = "crates/razer-service/src/camera/lifecycle.rs"
    brightness_runtime = "crates/razer-shell/src/shell/keyboard/keyboard_brightness_runtime.rs"
    brightness_worker = "crates/razer-shell/src/shell/keyboard/keyboard_brightness_worker.rs"
    brightness_callers = {
        "crates/razer-shell/src/shell/keyboard/keyboard_brightness_read.rs": "read_keyboard_brightness",
        "crates/razer-shell/src/shell/keyboard/keyboard_brightness_write.rs": "write_keyboard_brightness",
    }
    brightness_gui = owner in {*brightness_callers, brightness_runtime,
        "crates/razer-pages/src/features/keyboard/lighting/keyboard_brightness.rs"}
    brightness_state_types = {("keyboard_brightness_runtime", "State"),
        ("self", "keyboard_brightness_runtime", "State"), ("crate", "shell", "keyboard_brightness_runtime", "State")}
    dpi_caller = "crates/razer-shell/src/shell/mouse/mouse_current_dpi.rs"
    dpi_runtime = "crates/razer-shell/src/shell/mouse/mouse_current_dpi_runtime.rs"
    dpi_worker = "crates/razer-shell/src/shell/mouse/mouse_current_dpi_worker.rs"
    dpi_page = "crates/razer-pages/src/features/mouse/dpi/mouse_current_dpi.rs"
    dpi_gui_owners = {dpi_caller, dpi_runtime, dpi_page,
                      "crates/razer-pages/src/features/mouse/dpi/mouse_current_dpi_observation.rs"}
    dpi_gui = owner in dpi_gui_owners
    dpi_gui_io_targets = {("mouse_current_dpi", "request"), ("mouse_current_dpi", "request_with_context"),
                          ("ble226", "read")}
    polling_caller = "crates/razer-shell/src/shell/mouse/mouse_polling_write.rs"
    polling_runtime = "crates/razer-shell/src/shell/mouse/mouse_polling_runtime.rs"
    polling_worker = "crates/razer-shell/src/shell/mouse/mouse_polling_worker.rs"
    polling_gui = owner in {polling_caller, polling_runtime,
        "crates/razer-pages/src/features/mouse/polling/mouse_polling.rs",
        "crates/razer-pages/src/features/mouse/polling/mouse_polling_profile.rs",
        "crates/razer-pages/src/features/mouse/polling/mouse_polling_view.rs"}
    power_caller = "crates/razer-shell/src/shell/mouse/mouse_current_power.rs"
    power_runtime = "crates/razer-shell/src/shell/mouse/mouse_current_power_runtime.rs"
    power_worker = "crates/razer-shell/src/shell/mouse/mouse_current_power_worker.rs"
    power_gui = owner in {power_caller, power_runtime,
        "crates/razer-pages/src/features/mouse/power/mouse_current_power.rs"}
    discovery_owner = "crates/razer-shell/src/shell/devices/device_discovery.rs"
    shutdown_owner = "crates/razer-shell/src/shell/shutdown.rs"
    interdevice_owner = "crates/razer-model/src/mapping/interdevice.rs"
    mapping_adapter = "crates/razer-shell/src/shell/devices/product_mapping.rs"
    shortcut_owner = "crates/razer-model/src/shortcuts/native.rs"
    input_owner = "crates/razer-device/src/mapping/mapping_engine.rs"
    hash_owner = "crates/razer-device/src/mapping/mapping_engine_hash.rs"
    mapping_product_owner = "crates/razer-device/src/mapping/mapping_document.rs"
    mapping_compiler_owner = "crates/razer-model/src/mapping.rs"
    mapping_workers = "crates/razer-service/src/mapping/"
    seiren_workers = "crates/razer-service/src/audio/seiren/"
    seiren_gui = owner.startswith("crates/razer-pages/src/features/audio/seiren/") or owner in {
        "crates/razer-shell/src/shell/audio/seiren_audio_capture.rs",
        "crates/razer-shell/src/shell/audio/seiren_audio_playback.rs"}
    macro_page = "crates/razer-app-pages/src/macros/macro_page"
    macro_transfer = macro_page + "/transfer.rs"
    macro_xml = "crates/razer-model/src/macro_library/xml.rs"
    macro_io = "crates/razer-storage/src/macro_transfer.rs"
    macro_recording = "crates/razer-model/src/macro_library/recording.rs"
    macro_recorder = mapping_workers + "recorder.rs"
    macro_recorder_owner = mapping_workers + "recorder_owner.rs"
    help_report_owner = "crates/razer-service/src/native/help_system_info.rs"
    independent_help_report = owner == help_report_owner or owner.startswith(
        "crates/razer-service/src/native/help_system_info/")
    macro_native = "crates/razer-service/src/runtime/windows/mapping_runtime/capture/recorder.rs"
    macro_receipts = "crates/razer-ipc/src/macro_recording.rs"
    mapping_receipts = "crates/razer-ipc/src/mapping_control.rs"
    mapping_control = mapping_workers + "control.rs"
    mapping_native_control = "crates/razer-service/src/runtime/windows/mapping_runtime/backend.rs"
    keyboard_restore = "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore.rs"
    keyboard_publisher = "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_publisher.rs"
    keyboard_worker = "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_worker.rs"
    legacy_keyboard_mapping = "crates/razer-pages/src/features/keyboard/mapping/keyboard_mapping_runtime_515.rs"
    keyboard_actuation = "crates/razer-shell/src/shell/keyboard/keyboard_actuation_write.rs"
    keyboard_actuation_worker = "crates/razer-shell/src/shell/keyboard/keyboard_actuation_worker.rs"
    session_monitor = "crates/razer-platform/src/platform/windows/session_changes.rs"
    runtime_panel = "crates/razer-settings/src/runtime_page.rs"
    device_monitor = "crates/razer-platform/src/observation/device_changes.rs"
    thx_lifecycle = "crates/razer-shell/src/shell/audio/thx_sound.rs"
    ipc_client = "crates/razer-ipc/src/lib.rs"
    ipc_pipe_workers = {"input_worker", "output_worker", "diagnostic_worker"}
    ipc_bootstrap = "crates/razer-ipc/src/hid_broker/bootstrap.rs"
    ipc_broker_client = "crates/razer-ipc/src/hid_broker/client.rs"
    broker_service = "crates/razer-service/src/runtime/mod.rs"
    broker_server = "crates/razer-service/src/runtime/hid_broker/server.rs"
    hid_clock = "crates/razer-service/src/runtime/hid_broker/clock.rs"
    hid_collection = "crates/razer-service/src/runtime/hid_broker/collection.rs"
    legacy_ipc_functions = {"stop_simulated", "finish_simulated_process", "reap_child", "join_pipe", "forward_requests"}
    keyboard_lifecycle = "crates/razer-shell/src/shell/keyboard/keyboard_lifecycle.rs"
    retired_shortcuts = "crates/razer-storage/src/workspace/retired_shortcuts.rs"
    preference_state = "crates/razer-state/src/ui_preferences.rs"
    preference_storage = "crates/razer-storage/src/ui_preferences.rs"
    preference_pages = ("crates/razer-pages/src/features/keyboard/", "crates/razer-pages/src/features/accessories/hue/",
                        "crates/razer-app-pages/src/armory/")
    preference_page = owner.startswith(preference_pages) or owner == "crates/razer-pages/src/features/accessories/hue.rs"
    profile_codec = "crates/razer-device/src/profile_transfer.rs"
    matcher_shell = "crates/razer-shell/src/shell/mouse/mouse_matcher.rs"
    matcher_page = "crates/razer-pages/src/features/mouse/dpi/mouse_matcher_owner.rs"
    matcher_worker = "crates/razer-service/src/mouse/dpi_matcher_worker.rs"
    matcher_gui_owners = {matcher_shell, matcher_page,
        "crates/razer-pages/src/features/mouse/dpi/mouse_dpi_matcher.rs",
        "crates/razer-pages/src/features/mouse/dpi/mouse_dpi_matcher_state.rs"}
    shared_gui_scope = {
        "crates/razer-pages/src/features/mouse/mod.rs": (
            "mouse_matcher", matcher_shell, matcher_worker,
            {"performance_matcher_and_properties", "performance", "refresh_locale"}),
        "crates/razer-pages/src/features/workspace/source_workspace.rs": (
            "mouse_dpi", dpi_runtime, dpi_worker,
            {"initialize_current_mouse_profile", "accept_current_dpi_ble_catalog"}),
        "crates/razer-pages/src/features/workspace/product_workspace.rs": (
            "mouse_dpi", dpi_runtime, dpi_worker, {"accept_current_dpi_ble_catalog"}),
    }.get(owner)
    independent_dpi_owner = owner in dpi_gui_owners | matcher_gui_owners | {
        dpi_worker, matcher_worker,
        "crates/razer-device/src/mouse/mouse_current_dpi.rs",
        "crates/razer-device/src/mouse/mouse_current_dpi_ble.rs",
        "crates/razer-device/src/mouse/mouse_current_dpi_ble_initialization.rs",
        "crates/razer-device/src/mouse/mouse_onboard.rs",
        "crates/razer-device/src/mouse/dpi_matcher.rs",
        "crates/razer-device/src/mouse/dpi_matcher_storage.rs",
        "crates/razer-discovery/src/direct/mouse_current_dpi.rs",
        "crates/razer-ipc/src/device_tasks.rs",
        "crates/razer-ipc/src/mouse_current_dpi_ble.rs",
        "crates/razer-ipc/src/request_cancellation.rs",
        "crates/razer-storage/src/host_local_profile/ble226.rs",
        "crates/razer-service/src/mouse/dpi_matcher.rs",
        "crates/razer-service/src/mouse/dpi_matcher_input.rs",
        "crates/razer-service/src/runtime/windows/mouse/mouse_current_dpi_ble.rs"}
    profile_name_owner = "crates/razer-device/src/source_string.rs"
    profile_name_consumers = {
        "crates/razer-device/src/mouse/dpi_matcher_storage.rs": "confirmation_name",
        "crates/razer-device/src/mouse/mouse_philips/configuration.rs": "prepare",
        "crates/razer-storage/src/host_local_profile/initialization.rs": "prepare",
        "crates/razer-storage/src/receiver/receiver_reset.rs": "unique_name",
        "crates/razer-storage/src/receiver/receiver_indicator.rs": "ensure_cache",
    }
    receiver_save = "crates/razer-shell/src/shell/receiver/receiver_profile_save.rs"
    chroma_shell = "crates/razer-shell/src/shell.rs"
    chroma_service = "crates/razer-service/src/lighting/chroma_connect.rs"
    chroma_observation = "crates/razer-shell/src/shell/devices/product_chroma.rs"
    configuration_paths_owner = "crates/razer-storage/src/paths.rs"
    configuration_path_accessors = {"configuration_directory", "platform_configuration_directory",
                                    "store_path", "runtime_log_path", "panic_log_path"}
    chroma_session = ("razer_service", "chroma_connect", "Session")
    gui_source = owner.startswith(tuple("crates/" + name + "/src/" for name in (
        "razer-pages", "razer-app-pages", "razer-settings", "razer-shell", "razer-dashboard", "razer-widgets", "razer-tray")))
    keyboard_navigation_owner = "crates/razer-widgets/src/keyboard_navigation.rs"
    keyboard_button_owner = "crates/razer-widgets/src/keyboard_navigation/button.rs"
    source_lifecycle = "crates/razer-pages/src/features/workspace/source_workspace/lifecycle.rs"
    mapping_pages = ("crates/razer-pages/src/features/mapping/", "crates/razer-pages/src/features/mouse/mapping/",
                     "crates/razer-pages/src/features/keyboard/mapping/")
    retired_sources = {"crates/razer-pages/src/features/shortcuts/shortcut_engine.rs": shortcut_owner,
                       "crates/razer-pages/src/features/shortcuts/shortcut_engine_keys.rs": input_owner,
                       "crates/razer-pages/src/features/mapping/mapping_device_runtime.rs": mapping_workers + "device.rs",
                       "crates/razer-pages/src/features/mapping/mapping_key_capture_runtime.rs": mapping_workers + "key_capture.rs",
                       macro_page + "/transfer/xml.rs": macro_xml,
                       macro_page + "/recording_decode.rs": macro_recording,
                       macro_page + "/recording_actor.rs": macro_recorder,
                       "crates/razer-service/src/runtime/windows/global_shortcuts.rs": mapping_native_control,
                       "crates/razer-service/src/runtime/windows/mapping_execution.rs": mapping_native_control,
                       "crates/razer-service/src/runtime/windows/global_shortcut_capture.rs": "crates/razer-service/src/runtime/windows/mapping_runtime/capture.rs",
                       "crates/razer-service/src/runtime/windows/mapping_native_output.rs": "crates/razer-service/src/runtime/windows/mapping_runtime/driver.rs",
                       "crates/razer-service/src/native/native_read.rs": "Explicit tools/workspace.py comparisons"}
    shortcut_scope = owner.startswith(("crates/razer-model/src/shortcuts/", "crates/razer-pages/src/features/shortcuts/"))
    publication_consumers = {"crates/razer-resource-db/src/installation.rs": ("InstallationPaths", "publish_catalog"),
                             "crates/razer-resources/src/cache.rs": ("Cache", "publish"),
                             "crates/razer-resource-db/build.rs": (None, "stage")}
    contracts = {"Shortcut": "crates/razer-model/src/shortcuts.rs",
                 "MacroLibraryFile": "crates/razer-model/src/macro_library.rs",
                 "AppPreferences": "crates/razer-model/src/preferences.rs",
                 "ModuleServiceSnapshot": "crates/razer-model/src/module_service.rs",
                 "WorkspaceFile": "crates/razer-storage/src/workspace.rs",
                 "RetiredShortcuts": retired_shortcuts}
    required_fields = {"with_shortcuts", "with_preferences", "with_custom_colors", "with_host_tab_order",
                       "with_dashboard", "with_module_services", "with_macros", "with_retired_shortcuts"}
    aliases = {name: name for name in ("WorkspaceFile", "PreparedSave", "StagedResource",
                                      "HeldInterdeviceTarget", "TargetObservation", "TargetTransition",
                                      "MappingEngine", "canonical_json", "source_hash", "ServiceClient",
                                      "WorkerGuard", "ACTIVE_WORKERS", "thread", "Command", "Admission")}
    def text(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8") if node is not None else ""
    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)
    def ancestors(node):
        parent = node.parent
        while parent is not None:
            yield parent
            parent = parent.parent
    def fail(node, kind, expected_owner):
        violations.append({"owner": owner, "kind": kind, "expected_owner": expected_owner,
                           "byte_start": max(0, node.start_byte - offset), "byte_end": min(len(raw) - offset, node.end_byte - offset)})
    def in_test(node):
        path = Path(owner)
        if path.stem.endswith(("_tests", "_test")) or path.stem in ("tests", "test") or {"testdata", "tests"} & set(path.parts):
            return True
        while node is not None:
            sibling = node.prev_named_sibling
            while sibling is not None and sibling.type in ("line_comment", "block_comment", "attribute_item"):
                if sibling.type == "attribute_item":
                    attribute = "".join(text(sibling).split())
                    if "cfg(test)" in attribute or "prelude::v1::test" in attribute or attribute.startswith(("#[test]", "#[gpui_kit::test]")):
                        return True
                sibling = sibling.prev_named_sibling
            node = node.parent
        return False
    def legacy_ipc_function(node):
        function = node if node.type == "function_item" else next(
            (parent for parent in ancestors(node) if parent.type == "function_item"), None)
        return owner == ipc_client and function is not None and text(function.child_by_field_name("name")) in legacy_ipc_functions
    def test_only_field(node):
        if in_test(node):
            return True
        sibling = node.prev_named_sibling
        while sibling is not None and sibling.type in {"attribute_item", "line_comment", "block_comment"}:
            if sibling.type == "attribute_item" and "".join(text(sibling).split()) in {
                    "#[cfg(all(test,windows))]", "#[cfg(all(windows,test))]"}:
                return True
            sibling = sibling.prev_named_sibling
        return False
    def called_methods(node):
        methods = {text(call.child_by_field_name("function").child_by_field_name("field")) for call in walk(node)
                   if call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"}
        # Rust macro token trees do not expose their boolean expression as AST.
        # Parse ensure!'s arguments as an ordinary call without evaluating it.
        for child in walk(node):
            if child.type != "macro_invocation" or text(child.child_by_field_name("macro")).split("::")[-1] != "ensure":
                continue
            tokens = next((item for item in child.named_children if item.type == "token_tree"), None)
            if tokens is None:
                continue
            source = b"fn __architecture_assertion() { assertion(" + tokens.text[1:-1] + b"); }"
            parsed = rust_parser.parse(source)
            if parsed.root_node.has_error:
                continue
            methods.update(call.child_by_field_name("function").child_by_field_name("field").text.decode()
                for call in walk(parsed.root_node) if call.type == "call_expression"
                and call.child_by_field_name("function").type == "field_expression")
        return methods
    def calls(node):
        return [(text(call.child_by_field_name("function")).split("::"), call) for call in walk(node)
                if call.type == "call_expression" and call.child_by_field_name("function").type != "field_expression"]
    if owner in retired_sources:
        fail(tree.root_node, "retired_architecture_source", retired_sources[owner])
    declarations = list(walk(tree.root_node))
    thread_builders = set()
    imports = {}
    def imported_paths(node, prefix=()):
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
        elif node.type == "self":
            yield prefix[-1], prefix
        elif node.type in {"identifier", "scoped_identifier"}:
            path = prefix + tuple(text(node).split("::"))
            yield path[-1], path
    for declaration in declarations:
        if declaration.type == "use_declaration":
            for alias, path in imported_paths(declaration):
                imports[alias] = path
                if path == ("std", "thread", "Builder"):
                    thread_builders.add(alias)
    def resolved_call(parts):
        return imports.get(parts[0], (parts[0],)) + tuple(parts[1:])
    native_button_types = {("gpui_kit", "base", "Button"), ("gpui_kit", "base", "button", "Button")}
    button_aliases = set()
    changed = True
    while changed:
        changed = False
        for declaration in declarations:
            if declaration.type == "type_item":
                target = resolved_call(text(declaration.child_by_field_name("type")).split("::"))
                alias = text(declaration.child_by_field_name("name"))
                if (target in native_button_types or len(target) == 1 and target[0] in button_aliases) and alias not in button_aliases:
                    button_aliases.add(alias)
                    changed = True
    def inspect_gui_operation(node, prefix, runtime, worker, targets, native_methods=(), revision_owner=None):
        if node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type == "field_expression":
                method = text(function.child_by_field_name("field"))
                if method == "detach" or (method == "join" and not node.child_by_field_name("arguments").named_children):
                    fail(node, prefix + "_gui_detaches_or_waits_for_native_work", runtime)
                if method in native_methods:
                    fail(node, prefix + "_gui_owns_device_io", worker)
            else:
                target = resolved_call(text(function).split("::"))
                if (target[-2:] in targets | {("ServiceClient", "spawn")}
                        or target[-1] in native_methods
                        or target in {("std", "thread", "spawn"), ("std", "thread", "Builder", "new"),
                                      ("thread", "spawn"), ("thread", "Builder", "new")}):
                    fail(node, prefix + "_gui_owns_device_io", worker)
        if ((node.type == "field_expression" and text(node.child_by_field_name("field")) == "discovery_revision")
                or node.type == "identifier" and text(node) == "discovery_revision"):
            fail(node, prefix + "_uses_global_discovery_revision", revision_owner or runtime)
    def owned_type(node):
        if node is None:
            return ((), ())
        if node.type == "generic_type":
            name = resolved_call(text(node.child_by_field_name("type")).split("::"))
            arguments = node.child_by_field_name("type_arguments")
            return (name, tuple(owned_type(child) for child in arguments.named_children))
        return (resolved_call(text(node).split("::")), ())
    def contains_owned_type(shape, target):
        return shape[0] == target or any(contains_owned_type(child, target) for child in shape[1])
    def path_type(node):
        if node is None:
            return False
        if node.type in {"reference_type", "mutable_specifier"}:
            return any(path_type(child) for child in node.named_children)
        return resolved_call(text(node).split("::")) in {
            ("std", "path", "Path"), ("std", "path", "PathBuf")}
    def path_expression(node, seen=frozenset()):
        if node is None or node.id in seen:
            return False
        seen = seen | {node.id}
        if node.type in {"try_expression", "reference_expression", "parenthesized_expression"}:
            return any(path_expression(child, seen) for child in node.named_children)
        if node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type == "field_expression":
                method = text(function.child_by_field_name("field"))
                return method in {"join", "with_file_name", "with_extension", "to_path_buf", "canonicalize", "parent", "unwrap", "expect"} \
                    and path_expression(function.child_by_field_name("value"), seen)
            return resolved_call(text(function).split("::")) in {
                ("std", "path", "Path", "new"), ("std", "path", "PathBuf", "from"),
                ("std", "env", "current_exe"), ("std", "env", "current_dir"), ("std", "env", "temp_dir")}
        if node.type == "identifier":
            function = node.parent
            while function is not None and function.type not in {"function_item", "closure_expression"}:
                function = function.parent
            if function is None:
                return False
            for declaration in walk(function):
                if declaration.type not in {"parameter", "let_declaration"} \
                        or text(declaration.child_by_field_name("pattern")) != text(node):
                    continue
                if path_type(declaration.child_by_field_name("type")):
                    return True
                if declaration.type == "let_declaration" and declaration.end_byte < node.start_byte \
                        and path_expression(declaration.child_by_field_name("value"), seen):
                    return True
        return False
    def locks_capture_lifecycle(node):
        return any(call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
            and text(call.child_by_field_name("function").child_by_field_name("field")) in {"lock", "try_lock"}
            and resolved_call(text(call.child_by_field_name("function").child_by_field_name("value")).split("::")) == ("razer_ipc", "KEY_CAPTURE_LIFECYCLE")
            for call in walk(node)) or any(resolved_call(parts) == ("super", "lock_lifecycle") for parts, _ in calls(node))
    def self_field(node, field):
        return (node is not None and node.type == "field_expression"
                and text(node.child_by_field_name("value")) == "self"
                and text(node.child_by_field_name("field")) == field)
    def local_variant(node, enum, variant):
        if node.type == "call_expression":
            node = node.child_by_field_name("function")
        if node.type != "scoped_identifier":
            return False
        parts = node.text.decode().split("::")
        return len(parts) == 2 and aliases.get(parts[0], parts[0]) == enum and parts[1] == variant
    def tuple_pattern(node, constructor):
        return any(child.type == "tuple_struct_pattern" and resolved_call(text(child.child_by_field_name("type")).split("::")) == (constructor,)
            for child in walk(node))
    def macro_arguments(node, name):
        if node.type != "macro_invocation" or text(node.child_by_field_name("macro")).split("::")[-1] != name:
            return []
        tokens = next((child for child in node.named_children if child.type == "token_tree"), None)
        if tokens is None:
            return []
        source = b"fn __architecture_arguments() { arguments(" + tokens.text[1:-1] + b"); }"
        parsed = rust_parser.parse(source)
        if parsed.root_node.has_error:
            return []
        call = next(child for child in walk(parsed.root_node) if child.type == "call_expression")
        return call.child_by_field_name("arguments").named_children
    def select_arm_nodes(node):
        # Parse Rust bodies inside select!'s token tree without evaluating futures.
        # Preserve original byte offsets so shared AST readers and diagnostics apply.
        result = []
        prefix = b"fn __architecture_select() "
        for invocation in walk(node):
            if invocation.type != "macro_invocation" or text(invocation.child_by_field_name("macro")).split("::")[-1] != "select":
                continue
            tokens = next((child for child in invocation.named_children if child.type == "token_tree"), None)
            if tokens is None:
                continue
            for body in tokens.named_children:
                if body.type != "token_tree" or not body.text.startswith(b"{") or body.start_byte < len(prefix):
                    continue
                parsed = rust_parser.parse(b" " * (body.start_byte - len(prefix)) + prefix + body.text)
                if not parsed.root_node.has_error:
                    function = next((child for child in parsed.root_node.named_children if child.type == "function_item"), None)
                    if function is not None:
                        result.extend(walk(function.child_by_field_name("body")))
        return result
    def conjunction_terms(node):
        if node.type == "parenthesized_expression":
            return conjunction_terms(node.named_children[0])
        if node.type == "binary_expression" and text(node.child_by_field_name("operator")) == "&&":
            return conjunction_terms(node.child_by_field_name("left")) + conjunction_terms(node.child_by_field_name("right"))
        return [node]
    def condition_atoms(node):
        return {"".join(text(term).split()) for term in conjunction_terms(node)}
    def expression_shape(node):
        children = [child for child in node.children if child.type not in {",", "line_comment", "block_comment"}]
        return (node.type, tuple(expression_shape(child) for child in children)) if children else (node.type, node.text)
    def parsed_expression_shape(source):
        parsed = rust_parser.parse(b"fn __condition() { " + source.encode() + b" }")
        if parsed.root_node.has_error:
            raise ValueError("Invalid architecture condition expression")
        return expression_shape(parsed.root_node.named_children[0].child_by_field_name("body").named_children[-1])
    def disjunction_terms(node):
        if node.type == "parenthesized_expression":
            return disjunction_terms(node.named_children[0])
        if node.type == "binary_expression" and text(node.child_by_field_name("operator")) == "||":
            return disjunction_terms(node.child_by_field_name("left")) + disjunction_terms(node.child_by_field_name("right"))
        return [node]
    def enclosing_conditions(node):
        conditions = set()
        child, parent = node, node.parent
        while parent is not None:
            if parent.type == "if_expression":
                body = parent.child_by_field_name("consequence")
                if body.start_byte <= child.start_byte and child.end_byte <= body.end_byte:
                    conditions.update(condition_atoms(parent.child_by_field_name("condition")))
            child, parent = parent, parent.parent
        return conditions
    def returning_branch(node):
        body = node.child_by_field_name("consequence")
        statements = [child for child in body.named_children if child.type not in {"line_comment", "block_comment"}]
        return (len(statements) == 1 and (statements[0].type == "return_expression"
                or statements[0].type == "expression_statement" and statements[0].named_children
                and statements[0].named_children[0].type == "return_expression"))
    def background_wait_branch(node):
        if not any(parts[-1] == "background_exit_pending" for parts, _ in calls(node.child_by_field_name("condition"))):
            return False
        body = node.child_by_field_name("consequence")
        statements = [child for child in body.named_children if child.type not in {"line_comment", "block_comment"}]
        last = statements[-1] if statements else None
        return (last is not None and "wait_for_background_exit" in called_methods(body)
                and (last.type == "return_expression" or last.type == "expression_statement"
                    and last.named_children and last.named_children[0].type == "return_expression"))
    def preceding_return(branch, call):
        parent = branch.parent
        if parent is not None and parent.type == "expression_statement":
            parent = parent.parent
        ancestor = call.parent
        while ancestor is not None and ancestor != parent:
            ancestor = ancestor.parent
        return ancestor is not None and branch.end_byte < call.start_byte and (returning_branch(branch) or background_wait_branch(branch))
    def method_on_field(node, method, field):
        return (node.type == "call_expression" and node.child_by_field_name("function").type == "field_expression"
                and text(node.child_by_field_name("function").child_by_field_name("field")) == method
                and self_field(node.child_by_field_name("function").child_by_field_name("value"), field))
    def straight_statement(node, body):
        transparent = {"call_expression", "field_expression", "try_expression", "parenthesized_expression",
                       "expression_statement", "return_expression", "let_declaration"}
        while node is not None and node.parent != body:
            node = node.parent
            if node is None or node.type not in transparent:
                return None
        return node
    def observer_worker_joins(function):
        body = function.child_by_field_name("body")
        def retained_take(expression):
            while expression is not None:
                if method_on_field(expression, "take", "worker"):
                    return expression
                if expression.type in {"try_expression", "parenthesized_expression"}:
                    expression = expression.named_children[0]
                elif expression.type == "call_expression":
                    method = expression.child_by_field_name("function")
                    if method.type != "field_expression" or text(method.child_by_field_name("field")) not in {"unwrap", "expect"}:
                        return None
                    expression = method.child_by_field_name("value")
                else:
                    return None
            return None
        takes = [call for call in walk(body) if method_on_field(call, "take", "worker")]
        if any(child.type in {"return_expression", "try_expression"}
               and straight_statement(child, body) is not None and child.end_byte < take.start_byte
               for take in takes for child in walk(body)):
            return []
        joins = [call for call in walk(body) if call.type == "call_expression"
                 and call.child_by_field_name("function").type == "field_expression"
                 and text(call.child_by_field_name("function").child_by_field_name("field")) == "join"]
        joined = set()
        for join in joins:
            receiver = join.child_by_field_name("function").child_by_field_name("value")
            take = retained_take(receiver)
            if take is not None and straight_statement(join, body) is not None:
                joined.add(take.id)
                continue
            if receiver.type != "identifier":
                return []
            declaration = next((item for item in body.named_children if item.type == "let_declaration"
                and text(item.child_by_field_name("pattern")) == text(receiver)
                and item.end_byte < join.start_byte), None)
            take = retained_take(declaration.child_by_field_name("value")) if declaration is not None else None
            if take is not None and straight_statement(join, body) is not None:
                joined.add(take.id)
                continue
            arm = next((parent for parent in ancestors(join) if parent.type == "match_arm"), None)
            match = arm.parent.parent if arm is not None else None
            if match is None or match.type != "match_expression" or straight_statement(match, body) is None:
                return []
            take = retained_take(match.child_by_field_name("value"))
            pattern = arm.child_by_field_name("pattern")
            bindings = [item for item in walk(pattern) if item.type == "tuple_struct_pattern"
                and resolved_call(text(item.child_by_field_name("type")).split("::")) == ("Some",)
                and [text(child) for child in item.named_children[1:]] == [text(receiver)]]
            if take is None or not bindings or straight_statement(join, arm) != arm.child_by_field_name("value"):
                return []
            joined.add(take.id)
        return takes if joins and takes and joined == {take.id for take in takes} else []
    def legacy_mapping_field(node, field):
        return (node is not None and node.type == "field_expression"
                and self_field(node.child_by_field_name("value"), "legacy_mapping")
                and text(node.child_by_field_name("field")) == field)
    def unwrapped_expression(node):
        while node is not None:
            children = [child for child in node.named_children if child.type not in {"line_comment", "block_comment", "mutable_specifier"}]
            if node.type == "block" and children:
                node = children[-1]
            elif node.type in {"parenthesized_expression", "reference_expression", "expression_statement", "return_expression"} and len(children) == 1:
                node = children[0]
            else:
                break
        return node
    def visible_binding(value, name):
        function = next((parent for parent in ancestors(value) if parent.type == "function_item"), None)
        if function is None:
            return None
        scopes = {parent.id for parent in ancestors(value)}
        candidates = [child for child in walk(function) if child.type == "let_declaration"
            and child.end_byte <= value.start_byte and child.parent.id in scopes
            and text(child.child_by_field_name("pattern")) == name]
        return max(candidates, key=lambda child: child.start_byte, default=None)
    def unchanged_binding(value, declaration):
        if declaration is None or any(child.type == "mutable_specifier" for child in declaration.children):
            return False
        function = next(parent for parent in ancestors(value) if parent.type == "function_item")
        return not any(child.type == "assignment_expression" and text(child.child_by_field_name("left")) == text(value)
            and declaration.end_byte <= child.start_byte < value.start_byte for child in walk(function))
    def native_number_spinner(node):
        number_owner = {
            "crates/razer-pages/src/features/audio/seiren/audio_seiren_eq_number.rs": "Number",
            "crates/razer-pages/src/features/chroma/chroma_studio_numeric.rs": "StudioNumeric",
            "crates/razer-widgets/src/stepper.rs": "Stepper",
        }.get(owner)
        if number_owner is None or node.parent.type != "call_expression" \
                or node.parent.child_by_field_name("function") != node:
            return False
        function = next((parent for parent in ancestors(node) if parent.type == "function_item"), None)
        implementation = next((parent for parent in ancestors(node) if parent.type == "impl_item"), None)
        if function is None or implementation is None or text(function.child_by_field_name("name")) != "render" \
                or text(implementation.child_by_field_name("type")) != number_owner \
                or resolved_call(text(implementation.child_by_field_name("trait")).split("::")) not in {
                    ("Render",), ("gpui_kit", "Render")}:
            return False
        constructors = node.parent.parent
        decorator = constructors.parent
        if constructors.type != "arguments" or decorator.type != "call_expression" \
                or text(decorator.child_by_field_name("function")) != "self.spinner" \
                or constructors.named_children[0] != node.parent:
            return False
        spinner_methods = [item for item in declarations if item.type == "function_item"
            and text(item.child_by_field_name("name")) == "spinner"
            and item.parent.parent.type == "impl_item"
            and text(item.parent.parent.child_by_field_name("type")) == number_owner]
        def native_button_type(value):
            path = resolved_call(text(value).split("::"))
            return path in native_button_types or len(path) == 1 and path[0] in button_aliases
        if len(spinner_methods) != 1 or not native_button_type(spinner_methods[0].child_by_field_name("return_type")):
            return False
        parameters = [item for item in spinner_methods[0].child_by_field_name("parameters").named_children
            if item.type == "parameter"]
        if not parameters or not native_button_type(parameters[0].child_by_field_name("type")):
            return False
        binding = decorator.parent
        if binding.type != "let_declaration" or binding.child_by_field_name("value") != decorator \
                or binding.child_by_field_name("pattern").type != "identifier" \
                or any(child.type == "mutable_specifier" for child in binding.children):
            return False
        name = text(binding.child_by_field_name("pattern"))
        usages = [item for item in walk(function.child_by_field_name("body")) if item.type == "identifier"
            and text(item) == name and item.start_byte >= binding.end_byte]
        has_number_callback = False
        for usage in usages:
            closure = usage.parent
            if closure.type == "closure_expression" and closure.child_by_field_name("body") == usage \
                    and closure.parent.type == "arguments":
                callback = closure.parent.parent
                target = callback.child_by_field_name("function")
                if callback.type != "call_expression" or target.type != "field_expression" \
                        or text(target.child_by_field_name("field")) not in {"increment_button", "decrement_button"}:
                    return False
                receiver = target.child_by_field_name("value")
                while receiver.type == "call_expression" \
                        and receiver.child_by_field_name("function").type == "field_expression":
                    receiver = receiver.child_by_field_name("function").child_by_field_name("value")
                if receiver.type != "call_expression" or resolved_call(text(receiver.child_by_field_name("function")).split("::")) not in {
                        ("gpui_kit", "base", "NumberInput", "new"),
                        ("gpui_kit", "base", "number_input", "NumberInput", "new")}:
                    return False
                has_number_callback = True
                continue
            # The custom Stepper renders the same native spinners without tab stops.
            current, unfocusable = usage, False
            while current.parent is not None and current.parent.type == "field_expression" \
                    and current.parent.child_by_field_name("value") == current \
                    and current.parent.parent.type == "call_expression":
                target = current.parent
                current = target.parent
                if text(target.child_by_field_name("field")) == "focusable" \
                        and [text(arg) for arg in current.child_by_field_name("arguments").named_children] == ["false"]:
                    unfocusable = True
            parent_call = current.parent.parent if current.parent is not None and current.parent.type == "arguments" else None
            if number_owner != "Stepper" or not unfocusable or parent_call is None \
                    or parent_call.type != "call_expression" \
                    or parent_call.child_by_field_name("function").type != "field_expression" \
                    or text(parent_call.child_by_field_name("function").child_by_field_name("field")) != "child":
                return False
        return has_number_callback
    def source_clock_value(value, seen=frozenset()):
        value = unwrapped_expression(value)
        if value is None or value.id in seen:
            return False
        seen = seen | {value.id}
        if value.type == "unary_expression" and text(value).lstrip().startswith("*"):
            return source_clock_value(value.named_children[-1], seen)
        if value.type == "self":
            implementation = next((parent for parent in ancestors(value) if parent.type == "impl_item"), None)
            return implementation is not None and resolved_call(text(implementation.child_by_field_name("type")).split("::"))[-1] == "SourceClock"
        if value.type == "field_expression" and text(value.child_by_field_name("value")) == "self":
            implementation = next((parent for parent in ancestors(value) if parent.type == "impl_item"), None)
            owner_type = text(implementation.child_by_field_name("type")) if implementation is not None else ""
            return any(text(item.child_by_field_name("name")) == owner_type
                and any(text(field.child_by_field_name("name")) == text(value.child_by_field_name("field"))
                    and any(resolved_call(text(child).split("::"))[-1] == "SourceClock"
                        for child in walk(field.child_by_field_name("type")) if child.type in {"type_identifier", "scoped_type_identifier"})
                    for field in item.child_by_field_name("body").named_children if field.type == "field_declaration")
                for item in declarations if item.type == "struct_item")
        if value.type == "identifier":
            function = next((parent for parent in ancestors(value) if parent.type == "function_item"), None)
            if function is None:
                return False
            binding = visible_binding(value, text(value))
            if binding is not None:
                declared = binding.child_by_field_name("type")
                if declared is not None and any(resolved_call(text(child).split("::"))[-1] == "SourceClock"
                        for child in walk(declared) if child.type in {"type_identifier", "scoped_type_identifier"}):
                    return True
                initializer = unwrapped_expression(binding.child_by_field_name("value"))
                if initializer is not None and initializer.type == "call_expression":
                    target = resolved_call(text(initializer.child_by_field_name("function")).split("::"))
                    if target[-2:] == ("SourceClock", "default"):
                        return True
                return source_clock_value(initializer, seen)
            return any(text(parameter.child_by_field_name("pattern")) == text(value)
                and any(resolved_call(text(child).split("::"))[-1] == "SourceClock"
                        for child in walk(parameter.child_by_field_name("type"))
                        if child.type in {"type_identifier", "scoped_type_identifier"})
                for parameter in function.child_by_field_name("parameters").named_children
                if parameter.type == "parameter")
        return False
    def native_clock_call(call, method):
        function = call.child_by_field_name("function")
        arguments = call.child_by_field_name("arguments").named_children
        if function.type == "field_expression":
            return (arguments if text(function.child_by_field_name("field")) == method
                    and source_clock_value(function.child_by_field_name("value")) else None)
        target = resolved_call(text(function).split("::"))
        return arguments[1:] if target[-2:] == ("SourceClock", method) else None
    def classification_match(node, result_name, batch):
        if node is not None and node.type == "block" and any(child.type == "let_declaration"
                and text(child.child_by_field_name("pattern")) == result_name for child in node.named_children):
            return False
        node = unwrapped_expression(node)
        if node is None or node.type != "match_expression" \
                or text(unwrapped_expression(node.child_by_field_name("value"))) != result_name:
            return False
        arms = [arm for arm in node.child_by_field_name("body").named_children if arm.type == "match_arm"]
        if len(arms) != 2:
            return False
        matched = set()
        for arm in arms:
            pattern = arm.child_by_field_name("pattern")
            patterns = [child for child in walk(pattern) if child.type == "tuple_struct_pattern"]
            if len(patterns) != 1 or pattern.child_by_field_name("condition") is not None \
                    or any(child.type == "or_pattern" for child in walk(pattern)):
                return False
            pattern = patterns[0]
            target = resolved_call(text(pattern.child_by_field_name("type")).split("::"))
            name = text(pattern.named_children[-1])
            if target not in {("Ok",), ("Err",)} or pattern.named_children[-1].type != "identifier" or target in matched:
                return False
            matched.add(target)
            if any(child.type == "let_declaration" and text(child.child_by_field_name("pattern")) == name
                    for child in walk(arm.child_by_field_name("value"))):
                return False
            value = unwrapped_expression(arm.child_by_field_name("value"))
            if value.type != "call_expression":
                return False
            function = resolved_call(text(value.child_by_field_name("function")).split("::"))
            arguments = value.child_by_field_name("arguments").named_children
            if target == ("Err",):
                if function not in {("native_io_outcome",), ("razer_device", "backend", "native_io_outcome")} \
                        or len(arguments) != 1 or text(unwrapped_expression(arguments[0])) != name:
                    return False
                continue
            if function != ("Some",) or len(arguments) != 1 or arguments[0].type != "struct_expression" \
                    or resolved_call(text(arguments[0].child_by_field_name("name")).split("::"))[-2:] != ("NativeIoOutcome", "Returned"):
                return False
            fields = [child for child in arguments[0].child_by_field_name("body").named_children
                if child.type not in {"line_comment", "block_comment"}]
            expected = (name + ".byte_count", name + ".byte_count()") if batch else ("*" + name,)
            if len(fields) != 1 or fields[0].type != "field_initializer" \
                    or text(fields[0].child_by_field_name("field")) != "byte_count" \
                    or not any(expression_shape(fields[0].child_by_field_name("value")) == parsed_expression_shape(value)
                               for value in expected):
                return False
        return matched == {("Ok",), ("Err",)}
    def native_result_binding(value, kind):
        value = unwrapped_expression(value)
        if value is None or value.type != "identifier":
            return False
        declaration = visible_binding(value, text(value))
        if not unchanged_binding(value, declaration):
            return False
        function = next(parent for parent in ancestors(value) if parent.type == "function_item")
        methods = {"Send": {"send_feature_counted", "send_feature_at_collection_max_counted"},
                   "Get": {"get_feature", "get_feature_at_collection_max"}, "Batch": {"send_feature_batch"}}[kind]
        def dispatched(expression):
            expression = unwrapped_expression(expression)
            if expression is None:
                return False
            if expression.type == "if_expression":
                return dispatched(expression.child_by_field_name("consequence")) and dispatched(expression.child_by_field_name("alternative"))
            if expression.type == "else_clause":
                return dispatched(expression.named_children[-1])
            if expression.type != "call_expression":
                return False
            target = expression.child_by_field_name("function")
            if target.type != "field_expression" or text(target.child_by_field_name("field")) not in methods:
                return False
            receiver = unwrapped_expression(target.child_by_field_name("value"))
            return any(text(parameter.child_by_field_name("pattern")) == text(receiver)
                and any(resolved_call(text(child).split("::"))[-1] == "FeatureTransport"
                        for child in walk(parameter.child_by_field_name("type")) if child.type in {"type_identifier", "scoped_type_identifier"})
                for parameter in function.child_by_field_name("parameters").named_children if parameter.type == "parameter")
        return dispatched(declaration.child_by_field_name("value"))
    def native_clock_outcome(value, kind, seen=frozenset()):
        value = unwrapped_expression(value)
        if value is None or value.id in seen:
            return False
        seen = seen | {value.id}
        if value.type == "identifier":
            binding = visible_binding(value, text(value))
            return unchanged_binding(value, binding) and native_clock_outcome(binding.child_by_field_name("value"), kind, seen)
        if value.type == "match_expression":
            result = unwrapped_expression(value.child_by_field_name("value"))
            return native_result_binding(result, kind) and classification_match(value, text(result), kind == "Batch")
        if value.type != "call_expression" or kind == "Batch" \
                or resolved_call(text(value.child_by_field_name("function")).split("::")) not in {
                    ("native_result_outcome",), ("self", "native_result_outcome")}:
            return False
        arguments = value.child_by_field_name("arguments").named_children
        helpers = [child for child in declarations if child.type == "function_item"
            and text(child.child_by_field_name("name")) == "native_result_outcome"]
        return len(arguments) == 1 and native_result_binding(arguments[0], kind) and len(helpers) == 1 \
            and classification_match(helpers[0].child_by_field_name("body"),
                text(helpers[0].child_by_field_name("parameters").named_children[0].child_by_field_name("pattern")), False)
    changed = True
    while changed:
        changed = False
        for node in declarations:
            if node.type == "use_as_clause":
                original = text(node.child_by_field_name("path")).split("::")[-1]
                alias = text(node.child_by_field_name("alias"))
            elif node.type == "type_item":
                original = text(node.child_by_field_name("type")).split("::")[-1]
                alias = text(node.child_by_field_name("name"))
            else:
                continue
            if original in aliases and alias not in aliases:
                aliases[alias], changed = aliases[original], True
    for node in walk(tree.root_node):
        scoped_gui = shared_gui_scope is not None and any(
            parent.type == "function_item" and text(parent.child_by_field_name("name")) in shared_gui_scope[3]
            for parent in ancestors(node))
        if node.type == "macro_invocation" and text(node.child_by_field_name("macro")) == "include":
            token_tree = next(child for child in node.named_children if child.type == "token_tree")
            values = token_tree.named_children
            if len(values) == 1 and values[0].type in {"string_literal", "raw_string_literal"}:
                literal = text(values[0])
                if values[0].type == "raw_string_literal":
                    literal = re.sub(r'^r#*"|"#*$', '', literal)
                else:
                    literal = literal[1:-1]
                if "\\" in literal:
                    fail(node, "include_path_requires_resolution", "Public Rust types and modules")
                else:
                    target = (ROOT / owner).parent.joinpath(literal).resolve()
                    parts = Path(owner).parts
                    crate_root = ROOT.joinpath(*parts[:2]) if parts[:1] == ("crates",) else ROOT
                    if not target.is_relative_to(crate_root) or (crate_root == ROOT and target.is_relative_to(ROOT / "crates")):
                        fail(node, "cross_crate_private_include", "Public Rust types and modules")
            elif (owner, "".join(text(node).split())) not in {
                    ("crates/razer-resource-db/src/lib.rs", 'include!(concat!(env!("OUT_DIR"),"/installation.rs"))'),
                    ("crates/razer-platform/src/observation/hardware_driver.rs", 'include!(concat!(env!("OUT_DIR"),"/hardware_driver_installation.rs"))')}:
                fail(node, "include_path_requires_resolution", "Explicit local include path or the installation constants generator")
        if (independent_help_report or independent_dpi_owner or scoped_gui) and node.type in {
                "use_declaration", "scoped_identifier", "scoped_type_identifier"}:
            paths = [path for _, path in imported_paths(node)] if node.type == "use_declaration" else [
                resolved_call(text(node).split("::"))]
            for path in paths:
                vendor_loader = path[:1] == ("libloading",) or path[:2] in {
                        ("crate", "dll"), ("super", "dll"),
                        ("crate", "native_read"), ("super", "native_read"),
                        ("razer_catalog", "native_library")}
                vendor_locator = (path[:2] == ("razer_platform", "native_paths")
                                  and (independent_help_report or path[2:3] in {("EnginePaths",), ("PathRule",)}))
                vendor_type = ((independent_dpi_owner or scoped_gui)
                               and ("EngineLibrary" in path or path[-2:] == ("PathRule", "CommonDll")))
                if vendor_loader or vendor_locator or vendor_type:
                    fail(node, "independent_help_report_uses_vendor_loader" if independent_help_report
                         else "independent_dpi_uses_vendor_loader",
                         help_report_owner if independent_help_report else "Explicit tools/workspace.py comparisons")
                    break
        if in_test(node) and not legacy_ipc_function(node):
            continue
        if gui_source and owner != keyboard_button_owner and node.type == "scoped_identifier":
            path = resolved_call(text(node).split("::"))
            if path[-1:] == ("new",) and (path[:-1] in native_button_types
                    or len(path) == 2 and path[0] in button_aliases) and not native_number_spinner(node):
                fail(node, "source_button_bypasses_window_navigation", keyboard_button_owner + "::Button")
        if owner == keyboard_navigation_owner and node.type == "function_item" \
                and text(node.child_by_field_name("name")) == "new":
            implementation = next((parent for parent in ancestors(node) if parent.type == "impl_item"), None)
            if implementation is not None and text(implementation.child_by_field_name("type")) == "KeyboardNavigation":
                retention = [call for call in walk(node) if call.type == "call_expression"
                             and call.child_by_field_name("function").type == "field_expression"
                             and text(call.child_by_field_name("function").child_by_field_name("value")) == "window"
                             and text(call.child_by_field_name("function").child_by_field_name("field")) == "set_view_retention"]
                if len(retention) != 1 or [text(argument) for argument in retention[0].child_by_field_name("arguments").named_children] != ["false"]:
                    fail(node, "keyboard_navigation_replays_stale_focus_geometry", keyboard_navigation_owner + "::KeyboardNavigation::new")
        if owner != hid_clock and node.type == "call_expression":
            if native_clock_call(node, "native_return") is not None:
                fail(node, "hid_native_clock_bypasses_outcome_owner", hid_clock + "::SourceClock::native_outcome")
            clock_arguments = native_clock_call(node, "native_outcome")
            if clock_arguments is not None:
                function = next((parent for parent in ancestors(node) if parent.type == "function_item"), None)
                implementation = next((parent for parent in ancestors(node) if parent.type == "impl_item"), None)
                dispatch = (owner == hid_collection and function is not None and implementation is not None
                    and text(function.child_by_field_name("name")) == "execute"
                    and text(implementation.child_by_field_name("type")) == "Packet")
                if not dispatch:
                    fail(node, "hid_native_clock_updates_outside_packet_dispatch", hid_collection + "::Packet::execute")
                else:
                    kind = resolved_call(text(clock_arguments[1]).split("::")) if len(clock_arguments) == 3 else ()
                    if kind[-2:] not in {("NativeReturn", "Send"), ("NativeReturn", "Get"), ("NativeReturn", "Batch")} \
                            or not native_clock_outcome(clock_arguments[2], kind[-1]):
                        fail(node, "hid_native_clock_uses_non_native_result", hid_collection + "::native_result_outcome")
        if owner != hid_clock and node.type == "scoped_identifier" \
                and resolved_call(text(node).split("::"))[-2:] in {("SourceClock", "native_return"), ("SourceClock", "native_outcome")} \
                and not (node.parent.type == "call_expression" and node.parent.child_by_field_name("function") == node):
            direct_return = resolved_call(text(node).split("::"))[-1] == "native_return"
            fail(node, "hid_native_clock_bypasses_outcome_owner" if direct_return else "hid_native_clock_uses_non_native_result",
                 hid_clock + "::SourceClock::native_outcome" if direct_return else hid_collection + "::Packet::execute")
        if owner == hid_collection and node.type == "function_item" \
                and text(node.child_by_field_name("name")) == "native_result_outcome":
            parameters = node.child_by_field_name("parameters").named_children
            if len(parameters) != 1 or not classification_match(node.child_by_field_name("body"),
                    text(parameters[0].child_by_field_name("pattern")), False):
                fail(node, "hid_native_result_classifier_discards_outcome", hid_collection + "::native_result_outcome")
        if node.type == "function_item" and owner.startswith("crates/") \
                and text(node.child_by_field_name("name")) in configuration_path_accessors \
                and owner != configuration_paths_owner:
            fail(node, "configuration_path_has_a_second_owner", configuration_paths_owner)
        if owner == configuration_paths_owner and node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type == "field_expression" and text(function.child_by_field_name("field")) in {
                    "unwrap", "expect", "unwrap_or", "unwrap_or_default", "unwrap_or_else", "or", "or_else"}:
                fail(node, "configuration_path_replaces_a_resolution_failure", configuration_paths_owner)
            elif function.type != "field_expression" and resolved_call(text(function).split("::")) == \
                    ("std", "env", "current_dir"):
                fail(node, "configuration_path_uses_working_directory", configuration_paths_owner)
        profile_workspace = owner in {
            "crates/razer-pages/src/features/workspace/workspace.rs",
            "crates/razer-pages/src/features/workspace/source_workspace.rs",
            "crates/razer-model/src/product_preview.rs",
        }
        enclosing_function = next((parent for parent in ancestors(node)
                                   if parent.type == "function_item"), None) if profile_workspace else None
        function_name = text(enclosing_function.child_by_field_name("name")) if enclosing_function else ""
        constructing_workspace = function_name in {"new", "new_loaded", "registered_preview", "mouse_mat_preview"}
        restoring_workspace = function_name in {"restore_active", "restore_active_with_dpi_observation", "restore_from_device"}
        if constructing_workspace and node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type == "field_expression" \
                    and text(function.child_by_field_name("field")) in {"push", "insert", "extend", "get_or_insert_with"}:
                receiver = function.child_by_field_name("value")
                if receiver.type == "field_expression" and text(receiver.child_by_field_name("field")) == "profiles":
                    fail(node, "workspace_constructor_creates_profile_collection",
                         "Verified source initialization or explicit profile mutation")
        if node.type == "assignment_expression" and (constructing_workspace or restoring_workspace):
            fields = {text(part.child_by_field_name("field")) for part in walk(node.child_by_field_name("left"))
                      if part.type == "field_expression"}
            if fields & {"profiles", "source_settings", "source_device_settings", "saved"}:
                fail(node, "workspace_restore_or_constructor_persists_reducer_state",
                     "Observed input, retained reducer state, and explicit persistence owner")
            if constructing_workspace and "active_profile" in fields:
                fail(node, "workspace_constructor_selects_unobserved_profile",
                     "Source serial metadata or explicit profile selection")
        if node.type == "enum_variant" and text(node.child_by_field_name("name")) == "NativeLibrarySnapshot" \
                and owner == ipc_client:
            fail(node, "product_protocol_exposes_vendor_analysis", "Explicit tools/workspace.py comparisons")
        if node.type in {"scoped_identifier", "scoped_type_identifier"} and \
                resolved_call(text(node).split("::")) in {
                    ("ServiceRequest", "NativeLibrarySnapshot"),
                    ("razer_ipc", "ServiceRequest", "NativeLibrarySnapshot")}:
            fail(node, "product_protocol_exposes_vendor_analysis", "Explicit tools/workspace.py comparisons")
        if gui_source and owner != chroma_observation \
                and node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type != "field_expression" and resolved_call(text(function).split("::")) == \
                    ("razer_platform", "chroma_installation", "inspect"):
                fail(node, "chroma_installation_has_a_second_observation_owner", chroma_observation)
        if gui_source and node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function is not None and function.type == "field_expression" \
                    and text(function.child_by_field_name("field")) in {
                        "hover", "group_hover", "active", "group_active", "focus", "focus_visible", "focus_within"}:
                arguments = node.child_by_field_name("arguments")
                for callback in arguments.named_children:
                    if callback.type != "closure_expression":
                        continue
                    display_methods = {"hidden", "block", "flex", "inline", "inline_block", "inline_flex", "grid", "display"}
                    assigns_display = any(part.type == "assignment_expression" and any(
                        field.type == "field_expression" and text(field.child_by_field_name("field")) == "display"
                        for field in walk(part.child_by_field_name("left"))) for part in walk(callback))
                    if called_methods(callback) & display_methods or assigns_display:
                        fail(node, "interaction_style_changes_display_between_draw_phases",
                             "Retained page interaction state selected before layout")
        if dpi_gui:
            inspect_gui_operation(node, "mouse_dpi", dpi_runtime, dpi_worker, dpi_gui_io_targets)
        if owner in matcher_gui_owners:
            inspect_gui_operation(node, "mouse_matcher", matcher_shell, matcher_worker,
                                  dpi_gui_io_targets)
        if scoped_gui:
            inspect_gui_operation(node, *shared_gui_scope[:3], dpi_gui_io_targets)
        if owner == dpi_page and node.type == "function_item" \
                and text(node.child_by_field_name("name")) == "initialize_connection" \
                and any(parent.type == "impl_item" and text(parent.child_by_field_name("type")) == "State" for parent in ancestors(node)):
            self_calls = [call for call in walk(node) if call.type == "call_expression"
                          and call.child_by_field_name("function").type == "field_expression"
                          and text(call.child_by_field_name("function").child_by_field_name("value")) == "self"]
            if not self_calls or text(self_calls[0].child_by_field_name("function").child_by_field_name("field")) != "install_connection":
                fail(node, "mouse_dpi_initializer_precedes_connection_installation", dpi_page + "::State")
        if owner == "crates/razer-pages/src/features/workspace/source_workspace.rs" and node.type == "call_expression" \
                and node.child_by_field_name("function").type == "field_expression" \
                and text(node.child_by_field_name("function").child_by_field_name("field")) == "initialize_mouse182_profile":
            scope = next((parent for parent in ancestors(node) if parent.type in {"closure_expression", "function_item"}), None)
            if scope is not None and "observe_current_dpi_connection" in called_methods(scope):
                fail(node, "mouse182_initializer_has_a_second_connection_owner", dpi_page + "::State::initialize_connection")
        if node.type == "function_item" and owner == dpi_caller \
                and text(node.child_by_field_name("name")) == "submit_current_dpi_device":
            def uses_operation(function):
                return any(call.type == "call_expression"
                        and text(call.child_by_field_name("function")) == "self.start_current_dpi_operation"
                        for call in walk(function))
            implementation = next((parent for parent in ancestors(node)
                                   if parent.type == "impl_item"), None)
            delegates = any(call.type == "call_expression"
                            and text(call.child_by_field_name("function")) == "self.submit_current_dpi_at"
                            for call in walk(node))
            retained_delegate = delegates and implementation is not None and any(
                item.type == "function_item" and not in_test(item)
                and text(item.child_by_field_name("name")) == "submit_current_dpi_at"
                and uses_operation(item) for item in walk(implementation))
            if not uses_operation(node) and not retained_delegate:
                fail(node, "mouse_dpi_bypasses_retained_operation_owner", dpi_runtime)
        if node.type == "function_item" and owner == dpi_runtime \
                and text(node.child_by_field_name("name")) == "current_route":
            if not any(resolved_call(parts) in {("super", "device_discovery", "retained_connection"),
                    ("crate", "shell", "device_discovery", "retained_connection")} for parts, _ in calls(node)):
                fail(node, "mouse_dpi_does_not_retain_the_physical_connection", dpi_runtime)
        if polling_gui:
            inspect_gui_operation(node, "mouse_polling", polling_runtime, polling_worker,
                                  {("mouse_current_polling", "request"), ("mouse_receiver_polling", "write")})
        if node.type == "function_item" and owner == polling_caller \
                and text(node.child_by_field_name("name")) == "write_mouse_polling_device":
            if not any(call.type == "call_expression"
                    and text(call.child_by_field_name("function")) == "self.start_mouse_polling_operation"
                    for call in walk(node)):
                fail(node, "mouse_polling_bypasses_retained_operation_owner", polling_runtime)
        if node.type == "function_item" and owner == polling_runtime \
                and text(node.child_by_field_name("name")) == "current_route":
            if not any(resolved_call(parts) in {("super", "device_discovery", "retained_connection"),
                    ("crate", "shell", "device_discovery", "retained_connection")} for parts, _ in calls(node)):
                fail(node, "mouse_polling_does_not_retain_the_physical_connection", polling_runtime)
        if brightness_gui:
            inspect_gui_operation(node, "keyboard_brightness", brightness_runtime, brightness_worker, set(),
                                  ("request_keyboard_brightness",), brightness_runtime + "::Scope")
        power_idle_scope = owner == discovery_owner and any(parent.type == "function_item"
            and text(parent.child_by_field_name("name")) == "write_mouse_idle" for parent in ancestors(node))
        if power_gui or power_idle_scope:
            inspect_gui_operation(node, "mouse_power", power_runtime, power_worker,
                                  {("mouse_current_power", "request"), ("mouse_receiver_polling", "write_idle")})
        if node.type == "function_item" and ((owner == power_caller
                and text(node.child_by_field_name("name")) == "submit_current_power_device")
                or (owner == discovery_owner and text(node.child_by_field_name("name")) == "write_mouse_idle")):
            if not any(call.type == "call_expression" and text(call.child_by_field_name("function")) == "self.start_mouse_power_operation"
                       for call in walk(node)):
                fail(node, "mouse_power_bypasses_retained_operation_owner", power_runtime)
        if node.type == "function_item" and owner == power_runtime and text(node.child_by_field_name("name")) == "current_route":
            if not any(resolved_call(parts) in {("super", "device_discovery", "retained_connection"),
                    ("crate", "shell", "device_discovery", "retained_connection")} for parts, _ in calls(node)):
                fail(node, "mouse_power_does_not_retain_the_physical_connection", power_runtime)
        if node.type == "function_item" and owner in brightness_callers \
                and text(node.child_by_field_name("name")) == brightness_callers[owner]:
            dispatches = [call for call in walk(node) if call.type == "call_expression"
                and text(call.child_by_field_name("function")) == "self.start_keyboard_brightness_operation"]
            scopes = [call for parts, call in calls(node)
                if resolved_call(parts)[-3:] == ("keyboard_brightness_runtime", "Scope", "new")]
            if not dispatches or not scopes:
                fail(node, "keyboard_brightness_bypasses_retained_operation_owner", brightness_runtime)
        if node.type == "function_item" and owner == brightness_runtime \
                and text(node.child_by_field_name("name")) == "current_route":
            if not any(resolved_call(parts) in {("super", "device_discovery", "retained_connection"),
                    ("crate", "shell", "device_discovery", "retained_connection")} for parts, _ in calls(node)):
                fail(node, "keyboard_brightness_does_not_retain_the_physical_connection", brightness_runtime + "::Scope")
        if camera_page and node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type == "field_expression":
                method = text(function.child_by_field_name("field"))
                if method == "detach" or (method == "join" and not node.child_by_field_name("arguments").named_children):
                    fail(node, "camera_page_detaches_or_waits_for_native_work", camera_workers)
            else:
                target = resolved_call(text(function).split("::"))
                if (target[:2] == ("std", "thread")
                        or target[-2:] in {("ServiceClient", "spawn"), ("SessionMonitor", "start")}):
                    fail(node, "camera_page_starts_unowned_native_work", camera_workers)
        if (node.type == "function_item" and owner == source_lifecycle
                and text(node.child_by_field_name("name")) == "prepare_exit"
                and "prepare_camera_exit" not in called_methods(node)):
            fail(node, "camera_exit_does_not_close_native_task_admission", camera_workers)
        if (node.type == "function_item"
                and owner == "crates/razer-pages/src/features/workspace/source_workspace.rs"
                and text(node.child_by_field_name("name")) == "sync_camera_activity"
                and not any(child.type == "unary_expression" and "".join(text(child).split()) == "!self.exiting"
                            for child in walk(node))):
            fail(node, "camera_exit_reopens_the_physical_scope", camera_workers)
        if seiren_gui and node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type != "field_expression":
                path = resolved_call(text(function).split("::"))
                if path in {("ServiceClient", "spawn"), ("razer_ipc", "ServiceClient", "spawn"),
                            ("std", "thread", "Builder", "new"), ("thread", "Builder", "new"),
                            ("std", "thread", "spawn"), ("thread", "spawn"),
                            ("razer_storage", "audio_recording", "RecordingFile", "write")}:
                    fail(node, "gui_owns_seiren_audio_io", seiren_workers)
        if seiren_gui and node.type in {"struct_item", "static_item"} \
                and text(node.child_by_field_name("name")) in {"WorkerGuard", "ACTIVE_WORKERS"}:
            fail(node, "duplicate_seiren_audio_worker_owner", seiren_workers + "mod.rs")
        if seiren_gui and node.type in {"scoped_identifier", "scoped_type_identifier"}:
            path = resolved_call(text(node).split("::"))
            if len(path) == 3 and path[:2] == ("razer_ipc", "ServiceRequest") and path[-1] in {
                    "SeirenAudioCaptureStart", "SeirenAudioCapturePoll", "SeirenAudioCaptureStop",
                    "SeirenAudioPlaybackStart", "SeirenAudioPlaybackPoll", "SeirenAudioPlaybackStop"}:
                fail(node, "gui_owns_seiren_audio_protocol", seiren_workers)
        if gui_source and node.type == "call_expression":
            function = node.child_by_field_name("function")
            if function.type != "field_expression":
                path = resolved_call(text(function).split("::"))
                if path[:1] == ("razer_device",) and path[-1:] in {
                        ("prepare_document",), ("prepare_document_with_device_settings",)} \
                        and len(path) >= 3 and path[-2] in {
                            "mouse_legacy_mapping", "mouse_receiver_mapping", "keyboard_legacy_mapping",
                            "keyboard_current_mapping", "keyboard_magnetic_mapping", "keyboard_latest_mapping"}:
                    fail(node, "gui_duplicates_mapping_document_compiler", mapping_compiler_owner)
            if function.type == "field_expression":
                method = text(function.child_by_field_name("field"))
                if method in {"metadata", "symlink_metadata", "canonicalize", "read_dir", "read_link", "try_exists", "exists", "is_file", "is_dir"} \
                        and path_expression(function.child_by_field_name("value")):
                    fail(node, "gui_owns_path_observation", "crates/razer-platform/src/")
                    gui_io_violations.append(violations.pop())
            else:
                path = resolved_call(text(function).split("::"))
                if path[:2] in {("std", "fs"), ("tokio", "fs")} or path[:1] == ("async_fs",):
                    fail(node, "gui_owns_filesystem_io", "crates/razer-storage/src/file_transfer.rs")
                    gui_io_violations.append(violations.pop())
        if gui_source and node.type in {"scoped_identifier", "scoped_type_identifier"}:
            path = resolved_call(text(node).split("::"))
            if len(path) == 3 and path[:2] == ("razer_ipc", "ServiceRequest") and path[-1] in {
                    "MouseInputMonitoringStart", "MouseInputMonitoringPoll", "MouseInputMonitoringStop"}:
                fail(node, "gui_owns_mouse_input_monitoring_protocol", matcher_worker)
        if owner != mapping_product_owner and node.type == "or_pattern":
            numbers = [text(child) for child in walk(node) if child.type == "integer_literal"]
            if len(numbers) == 6 and set(numbers) == {"70", "515", "679", "740", "746", "756"}:
                fail(node, "duplicate_mapping_product_gate", mapping_product_owner)
        if node.type == "scoped_identifier" and resolved_call(text(node).split("::")) == ("razer_ipc", "ServiceRequest", "MacroRecordingMode"):
            if owner not in {macro_recorder_owner, "crates/razer-service/src/runtime/windows/native.rs"}:
                fail(node, "macro_mode_queries_an_unrelated_worker", macro_recorder_owner)
        if owner != mapping_product_owner and node.type == "string_literal" and text(node) in {
                '"_mouse70_mapping_document"', '"_keyboard515_mapping_document"'}:
            fail(node, "duplicate_mapping_document_storage_identity", mapping_product_owner)
        if node.type in {"mod_item", "use_declaration"}:
            identifiers = {text(child) for child in walk(node) if child.type in {"identifier", "type_identifier"}}
            if identifiers & {"mapping_device_runtime", "mapping_key_capture_runtime"}:
                fail(node, "retired_mapping_worker_module", mapping_workers)
            if node.type == "mod_item" and owner.startswith(macro_page) and text(node.child_by_field_name("name")) in {"recording_decode", "recording_actor"}:
                fail(node, "retired_macro_recording_module", macro_recording + " and " + macro_recorder)
        if node.type == "struct_item":
            name = text(node.child_by_field_name("name"))
            if name == "MappingProduct" and owner != mapping_product_owner:
                fail(node, "duplicate_mapping_product_owner", mapping_product_owner)
            body = node.child_by_field_name("body")
            fields = {text(item.child_by_field_name("name")) for item in body.named_children if item.type == "field_declaration"} if body is not None else set()
            if name in contracts and owner != contracts[name]:
                native_registration = (name == "Shortcut" and owner == "crates/razer-platform/src/input/global_shortcuts.rs"
                                       and fields == {"virtual_key", "modifiers", "argument"})
                if not native_registration:
                    fail(node, "duplicate_persisted_contract", contracts[name])
            if name == "PreparedSave" and (owner != factory_owner or fields != {
                    "window", "file", "owners", "product", "device_draft"}):
                fail(node, "multiple_product_save_owners", factory_owner)
            if name == "SavedRevisionOwner" and (owner != factory_owner or fields != {
                    "workspace", "connection_revision"}):
                fail(node, "duplicate_save_revision_owner", factory_owner)
            if name == "StagedResource" and owner != publication_owner:
                fail(node, "duplicate_resource_publication_owner", publication_owner)
            if name in {"HeldInterdeviceTarget", "TargetObservation", "TargetTransition"} and owner != interdevice_owner:
                fail(node, "duplicate_interdevice_lifecycle_owner", interdevice_owner)
            if name == "WorkerGuard" and owner.startswith((*mapping_pages, mapping_workers)) and owner != mapping_workers + "mod.rs":
                fail(node, "duplicate_mapping_worker_owner", mapping_workers + "mod.rs")
            if name == "MacroPage" and owner == macro_page + ".rs" and "transfer_task" not in fields:
                fail(node, "macro_transfer_has_no_page_task_owner", macro_transfer)
            if name == "PendingTransfer" and owner == macro_transfer and not {"request", "_task"} <= fields:
                fail(node, "macro_transfer_has_no_cancellation_owner", macro_transfer)
            if name == "Decoder" and owner.startswith(macro_page):
                fail(node, "page_owns_macro_recording_decoder", macro_recording)
            if name in {"SuspensionReceipt", "RestoreReceipt", "ActionReply"} and (owner.startswith(macro_page) or owner in {macro_recorder, macro_native}):
                fail(node, "duplicate_macro_recording_receipt", macro_receipts)
            if name == "RecordingUi" and owner == macro_page + "/recording.rs" and not {"command", "countdown_task", "updates_task", "generation", "cancelled"} <= fields:
                fail(node, "macro_recording_has_no_page_session_owner", macro_page + "/recording.rs")
            if name == "UiPreferences" and owner.startswith("crates/") and owner != preference_state:
                fail(node, "duplicate_ui_preference_state_owner", preference_state)
            if name == "UiPreferences" and owner == preference_state and not {"core", "worker"} <= fields:
                fail(node, "ui_preferences_has_no_retained_worker", preference_state)
            if owner == runtime_panel and name == "RuntimeBridge":
                types = {resolved_call(text(child).split("::"))[-1] for child in walk(body) if child.type == "type_identifier"}
                if not {"Arc", "ConnectionOwner"} <= types:
                    fail(node, "discovery_connection_has_no_shared_worker_owner", runtime_panel + "::ConnectionOwner")
            if owner == runtime_panel and name == "ConnectionOwner":
                worker = next((child.child_by_field_name("type") for child in body.named_children
                    if child.type == "field_declaration" and text(child.child_by_field_name("name")) == "worker"), None)
                types = {resolved_call(text(child).split("::"))[-1] for child in walk(worker) if child.type == "type_identifier"} if worker else set()
                if not {"commands", "worker"} <= fields or not {"Mutex", "Option", "JoinHandle"} <= types:
                    fail(node, "discovery_connection_has_no_retained_worker", runtime_panel + "::ConnectionOwner")
            if owner == runtime_panel and name == "RuntimePanel" and not {"bridge", "retired_bridges", "monitor", "retired_monitors", "monitor_task", "disconnect_task"} <= fields:
                fail(node, "discovery_panel_discards_observation_cleanup_owner", runtime_panel + "::RuntimePanel")
            if owner == device_monitor and name == "DeviceChangeMonitor":
                worker = next((child.child_by_field_name("type") for child in body.named_children
                    if child.type == "field_declaration" and text(child.child_by_field_name("name")) == "worker"), None)
                types = {resolved_call(text(child).split("::"))[-1] for child in walk(worker) if child.type == "type_identifier"} if worker else set()
                stop = next((child.child_by_field_name("type") for child in body.named_children
                    if child.type == "field_declaration" and text(child.child_by_field_name("name")) == "stop"), None)
                stop_types = {text(child).split("::")[-1] for child in walk(stop) if child.type == "type_identifier"} if stop else set()
                if not {"stop", "worker"} <= fields or not {"Option", "JoinHandle"} <= types or not {"Option", "SyncSender"} <= stop_types:
                    fail(node, "device_observer_has_no_retained_worker", device_monitor + "::DeviceChangeMonitor")
            if owner == "crates/razer-shell/src/shell.rs" and name == "AppShell" and "thx_retired_owners" not in fields:
                fail(node, "thx_spawn_failure_has_no_retained_owner", thx_lifecycle + "::RetiredOwner")
            if owner == chroma_shell and name == "AppShell":
                brightness = {text(child.child_by_field_name("name")): owned_type(child.child_by_field_name("type"))
                    for child in body.named_children if child.type == "field_declaration"
                    and any(contains_owned_type(owned_type(child.child_by_field_name("type")), target)
                            for target in brightness_state_types)}
                if (set(brightness) != {"keyboard_brightness"}
                        or brightness["keyboard_brightness"] not in {(target, ()) for target in brightness_state_types}):
                    fail(node, "keyboard_brightness_has_no_single_retained_shell_owner", brightness_runtime + "::State")
                sessions = {text(child.child_by_field_name("name")): owned_type(child.child_by_field_name("type"))
                    for child in body.named_children if child.type == "field_declaration"
                    and contains_owned_type(owned_type(child.child_by_field_name("type")), chroma_session)}
                shared_session = (("std", "sync", "Arc"), ((("std", "sync", "Mutex"), ((chroma_session, ()),)),))
                if sessions != {"receiver_chroma_connect": shared_session}:
                    fail(node, "chroma_connect_has_no_single_shared_shell_owner", chroma_shell + "::AppShell")
            if owner == thx_lifecycle and name == "RetiredOwner" and fields != {"owner", "source_close"}:
                fail(node, "thx_retirement_discards_owner_or_source_close", thx_lifecycle + "::RetiredOwner")
            if owner == ipc_client and name == "ServiceClient":
                members = {text(child.child_by_field_name("name")): child for child in body.named_children
                           if child.type == "field_declaration"}
                broker = members.get("broker")
                broker_types = {text(child).split("::")[-1] for child in walk(broker.child_by_field_name("type"))
                                if child.type == "type_identifier"} if broker is not None else set()
                if not {"Option", "BrokerClient"} <= broker_types:
                    fail(node, "ipc_client_has_no_shared_broker_session", ipc_broker_client + "::BrokerClient")
                if any(field in members and not test_only_field(members[field])
                       for field in {"child", "job", "requests", "responses"} | ipc_pipe_workers):
                    fail(node, "ipc_client_owns_production_process_or_pipe_workers", ipc_bootstrap + "::BrokerBootstrap")
                retained = {}
                for child in body.named_children:
                    if child.type == "field_declaration" and text(child.child_by_field_name("name")) in ipc_pipe_workers:
                        retained[text(child.child_by_field_name("name"))] = {resolved_call(text(item).split("::"))[-1]
                            for item in walk(child.child_by_field_name("type")) if item.type == "type_identifier"}
                if (not {"child", "requests"} | ipc_pipe_workers <= fields
                        or set(retained) != ipc_pipe_workers
                        or any(not {"Option", "JoinHandle", "Result"} <= types for types in retained.values())):
                    fail(node, "ipc_client_discards_owned_process_or_pipe_workers", ipc_client + "::ServiceClient")
            if name == "MappingTargetOwner" and owner == mapping_adapter and fields != {"workspace", "identity", "connection_revision"}:
                fail(node, "unscoped_interdevice_target_owner", interdevice_owner)
            if {"revision", "scopes", "owners"} <= fields and owner != mapping_receipts:
                fail(node, "duplicate_mapping_suppression_registry", mapping_receipts)
            if name == "InterdeviceHold" and owner == mapping_adapter:
                target = next((item.child_by_field_name("type") for item in body.named_children
                    if item.type == "field_declaration" and text(item.child_by_field_name("name")) == "target"), None)
                identifiers = {text(child) for child in walk(target) if child.type == "type_identifier"} if target else set()
                if not {"origin", "product_id", "raw", "target"} <= fields or not any(aliases.get(name) == "HeldInterdeviceTarget" for name in identifiers):
                    fail(node, "interdevice_hold_bypasses_lifecycle_owner", interdevice_owner)
        if node.type == "impl_item" and aliases.get(text(node.child_by_field_name("type")).split("::")[-1]) == "StagedResource" and owner != publication_owner:
            fail(node, "duplicate_resource_publication_owner", publication_owner)
        if node.type == "impl_item" and aliases.get(text(node.child_by_field_name("type")).split("::")[-1]) == "WorkerGuard" and owner.startswith((*mapping_pages, mapping_workers)) and owner != mapping_workers + "mod.rs":
            fail(node, "duplicate_mapping_worker_owner", mapping_workers + "mod.rs")
        if node.type == "impl_item" and owner != interdevice_owner:
            item_type = node.child_by_field_name("type")
            base = item_type.child_by_field_name("type") if item_type is not None and item_type.type == "generic_type" else item_type
            if aliases.get(text(base).split("::")[-1]) in {"HeldInterdeviceTarget", "TargetObservation", "TargetTransition"}:
                fail(node, "duplicate_interdevice_lifecycle_owner", interdevice_owner)
        if node.type == "struct_expression" and aliases.get(text(node.child_by_field_name("name")).split("::")[-1]) == "PreparedSave" and owner != factory_owner:
            fail(node, "product_recomposes_save_owners", factory_owner)
        if node.type == "static_item" and owner.startswith("crates/razer-resources/src/"):
            identifiers = {text(child) for child in walk(node.child_by_field_name("type")) if child.type == "type_identifier"}
            if identifiers & {"Service", "ResourceClient", "ResourceRuntime", "State"}:
                fail(node, "process_global_resource_session", "razer_resources::ResourceRuntime")
        if node.type == "static_item" and text(node.child_by_field_name("name")) == "ACTIVE_WORKERS" and owner.startswith((*mapping_pages, mapping_workers)) and owner != mapping_workers + "mod.rs":
            fail(node, "duplicate_mapping_worker_owner", mapping_workers + "mod.rs")
        if node.type == "static_item" and ((text(node.child_by_field_name("name")) == "KEY_CAPTURE_LIFECYCLE" and owner != "crates/razer-ipc/src/lib.rs")
                or text(node.child_by_field_name("name")) == "LIFECYCLE" and owner in {macro_recorder, mapping_workers + "key_capture.rs"}):
            fail(node, "duplicate_native_capture_lifecycle_owner", "crates/razer-ipc/src/lib.rs::KEY_CAPTURE_LIFECYCLE")
        if node.type in {"const_item", "static_item"} and shortcut_scope:
            for child in walk(node):
                if child.type == "tuple_expression" and len(child.named_children) >= 2:
                    first, *remaining = child.named_children
                    if first.type == "string_literal" and text(first).startswith('"KEY_') and any(item.type == "integer_literal" for item in remaining):
                        fail(node, "duplicate_shortcut_key_table", input_owner)
                        break
        if node.type == "function_item":
            name = text(node.child_by_field_name("name"))
            if not in_test(node):
                if name == "unique_profile_name" and owner != profile_name_owner:
                    fail(node, "duplicate_source_profile_name_owner", profile_name_owner)
                if profile_name_consumers.get(owner) == name:
                    delegated = any(resolved_call(parts) in {
                        ("crate", "source_string", "unique_profile_name"),
                        ("razer_device", "source_string", "unique_profile_name"),
                    } for parts, _ in calls(node))
                    local_loop = any(child.type == "for_expression" and any(
                        nested.type == "range_expression" and any(
                            literal.type == "integer_literal" and text(literal) == "100"
                            for literal in walk(nested)) for nested in walk(child)) for child in walk(node))
                    if not delegated or local_loop:
                        fail(node, "source_profile_name_bypasses_shared_owner", profile_name_owner)
            parent = node.parent.parent if node.parent is not None and node.parent.parent is not None else None
            free_function = parent is None or parent.type != "impl_item"
            implementation = text(parent.child_by_field_name("type")) if not free_function else ""
            if owner == matcher_worker and implementation == "NativeBackend<'_>" and name == "release_transport":
                body = node.child_by_field_name("body")
                finish = [call for call in walk(body) if call.type == "call_expression"
                    and text(call.child_by_field_name("function")) == "client.finish_termination"
                    and call.parent.type == "try_expression"]
                releases = [call for call in walk(body) if call.type == "call_expression"
                    and text(call.child_by_field_name("function")) in {
                        "self.client.take", "self.device_task.take", "self.lifecycle.take"}]
                if (len(finish) != 1 or {text(call.child_by_field_name("function")) for call in releases} != {
                        "self.client.take", "self.device_task.take", "self.lifecycle.take"}
                        or any(call.start_byte < finish[0].end_byte for call in releases)):
                    fail(node, "matcher_releases_leases_before_transport_termination", matcher_worker + "::NativeBackend::release_transport")
            if owner == matcher_worker and implementation == "Handle" and name == "finish":
                body = node.child_by_field_name("body")
                joins = [call for call in walk(body) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "join"]
                publications = [call for call in walk(body) if call.type == "call_expression"
                    and text(call.child_by_field_name("function")) == "self.completion.publish"
                    and any(text(ancestor.child_by_field_name("pattern")) == "Ok(exit)"
                        for ancestor in ancestors(call) if ancestor.type == "match_arm")]
                if (len(joins) != 1 or len(publications) != 1
                        or publications[0].start_byte < joins[0].end_byte
                        or ["".join(text(argument).split()) for argument in publications[0].child_by_field_name("arguments").named_children] != ["&exit.cleanup", "&exit.released"]):
                    fail(node, "matcher_completion_does_not_publish_joined_release", matcher_worker + "::Handle::finish")
            if owner == matcher_shell and name == "mouse_matcher_save_current":
                body = node.child_by_field_name("body")
                terms = conjunction_terms(body.named_children[-1]) if body.named_children else []
                predicates = [term for term in terms if term.type == "call_expression"
                    and term.child_by_field_name("function").type == "field_expression"
                    and text(term.child_by_field_name("function").child_by_field_name("field")) == "is_some_and"]
                correlated = False
                for predicate in predicates:
                    function = predicate.child_by_field_name("function")
                    receiver = function.child_by_field_name("value")
                    arguments = predicate.child_by_field_name("arguments").named_children
                    if ("".join(text(receiver).split()) != "self.mouse_matcher.sessions.get(&save.owner.key)"
                            or len(arguments) != 1 or arguments[0].type != "closure_expression"):
                        continue
                    closure = arguments[0].child_by_field_name("body")
                    if closure.type == "block" and closure.named_children:
                        closure = closure.named_children[-1]
                    correlated = {"session.owner.id==save.owner.id",
                        "session.owner.scope==save.owner.scope",
                        "session.owner.workspace.upgrade().as_ref()==Some(&save.workspace)",
                        "save.owner.current(self,session.closing,cx)"} <= condition_atoms(closure)
                if not correlated:
                    fail(node, "matcher_save_accepts_stale_session_or_connection", matcher_shell + "::mouse_matcher_save_current")
            if owner == matcher_shell and implementation == "Owner" and name == "current":
                body = node.child_by_field_name("body")
                terms = condition_atoms(body.named_children[-1]) if body.named_children else set()
                required = {"shell.devices.contains(&workspace)",
                    "workspace_state.identity(cx)==self.identity",
                    "workspace_state.connection_revision()==self.connection_revision",
                    "workspace_state.mouse_polling_scope(cx)==Some(self.domain_scope)",
                    "device.active_profile==self.profile_id",
                    "device.profiles.iter().any(|profile|profile.id==self.profile_id&&profile.guid==self.profile_guid)",
                    "workspace_state.retains_mouse_matcher_scope(&self.scope,cx)",
                    "draining||workspace_state.accepts_mouse_matcher_scope(&self.scope,cx)"}
                route = ("match&self.route{Some(route)=>super::device_discovery::retained_connection("
                         "std::slice::from_ref(route),&shell.device_observations,device,),"
                         "None=>self.scope.is_normalization(),}")
                if not required <= terms or route not in terms:
                    fail(node, "matcher_drain_bypasses_profile_or_connection_owner", matcher_shell + "::Owner::current")
            if owner == matcher_page and implementation == "MouseProductWorkspace" and name == "accepts_mouse_matcher_scope":
                body = node.child_by_field_name("body")
                terms = condition_atoms(body.named_children[-1]) if body.named_children else set()
                required = {"scope.is_valid()", "self.retains_mouse_matcher_scope(scope,cx)",
                    "scope.is_normalization()||self.mouse_matcher_scope(cx).as_ref()==Some(scope)"}
                if not required <= terms:
                    fail(node, "matcher_request_accepts_expired_or_unmounted_ui_scope", matcher_page + "::accepts_mouse_matcher_scope")
            if owner == matcher_page and implementation == "MouseProductWorkspace" and name == "retains_mouse_matcher_scope":
                body = node.child_by_field_name("body")
                terms = {expression_shape(term) for term in conjunction_terms(body.named_children[-1])} if body.named_children else set()
                required = {"self.mouse_polling_scope(cx)==Some(scope.domain)",
                    "self.matcher.as_ref().is_some_and(|matcher| match scope.kind {"
                    "ScopeKind::Ui=>scope.retained_by(matcher.entity_id(),self.matcher_epoch,&self.matcher_valid),"
                    "ScopeKind::Normalization=>scope.retained_by(matcher.entity_id(),self.matcher_normalization.epoch,&self.matcher_normalization.valid),})"}
                if not {parsed_expression_shape(source) for source in required} <= terms:
                    fail(node, "matcher_retention_loses_domain_or_owner_epoch", matcher_page + "::retains_mouse_matcher_scope")
            if owner == matcher_shell and name == "dispatch_mouse_matcher":
                for call in walk(node):
                    if call.type != "call_expression" or call.child_by_field_name("function").type != "field_expression":
                        continue
                    function = call.child_by_field_name("function")
                    if (text(function.child_by_field_name("field")) != "retire_mouse_matcher"
                            or text(function.child_by_field_name("value")) != "shell"):
                        continue
                    args = call.child_by_field_name("arguments").named_children
                    if (not args or "".join(text(args[0]).split()) != "&delivery_owner.key"
                            or not any(condition.endswith(".is_some_and(|session|session.owner.id==delivery_owner.id)")
                                and ".sessions.get(&delivery_owner.key)" in condition
                                for condition in enclosing_conditions(call))):
                        fail(call, "matcher_retired_worker_removes_successor", matcher_shell + "::dispatch_mouse_matcher")
            if owner in {matcher_shell, receiver_save} and name in {
                    "rebase_mouse_matcher_save", "rebase_receiver_profile_save"}:
                method = "mouse_matcher_save_current" if owner == matcher_shell else "receiver_profile_current"
                checks = [child for child in walk(node) if child.type == "macro_invocation"
                    and (arguments := macro_arguments(child, "ensure"))
                    and any(call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                        and call.child_by_field_name("function").child_by_field_name("field").text.decode() == method
                        for call in walk(arguments[0]))]
                mutations = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) in {"apply", "project"}]
                if not checks or not mutations or any(not any(check.end_byte < call.start_byte for check in checks) for call in mutations):
                    fail(node, "product_profile_rebase_skips_current_owner", owner + "::" + method)
            if free_function and name == "encode_shortcuts" and owner != shortcut_owner and owner.startswith(("crates/razer-model/", "crates/razer-pages/", "crates/razer-shell/")):
                fail(node, "duplicate_native_shortcut_encoder", shortcut_owner)
            if free_function and name == "input_modifier_bit" and owner != input_owner:
                fail(node, "duplicate_mapping_modifier_encoder", input_owner)
            if free_function and shortcut_scope and name in {"stable_json", "canonical_json", "source_hash"}:
                fail(node, "duplicate_shortcut_hash_encoder", hash_owner)
            if free_function and owner == profile_codec and name in {"stable_json", "stable_number", "canonical_json", "source_hash"}:
                fail(node, "duplicate_profile_hash_encoder", hash_owner)
            if free_function and owner.startswith(macro_page) and name == "parse_delay":
                fail(node, "page_owns_macro_delay_semantics", "crates/razer-model/src/macro_library.rs")
            if free_function and owner == profile_codec and name == "entry" and "source_hash" not in {aliases.get(parts[-1]) for parts, _ in calls(node)}:
                fail(node, "profile_hash_bypasses_shared_owner", hash_owner)
            if owner == macro_transfer:
                methods = called_methods(node)
                if name == "drop" and parent is not None and text(parent.child_by_field_name("type")) == "PendingTransfer" and "cancel" not in methods:
                    fail(node, "macro_transfer_drop_skips_cancellation", macro_io)
                if name in {"import_xml", "export_xml"}:
                    fields = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                    if "detach" in methods or "transfer_task" not in fields or "is_same" not in methods:
                        fail(node, "macro_transfer_accepts_unowned_completion", macro_transfer)
            if owner == preference_state and name == "start_worker" and parent is not None and text(parent.child_by_field_name("type")) == "UiPreferences":
                fields = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                if "worker" not in fields or "detach" in called_methods(node):
                    fail(node, "ui_preferences_has_no_retained_worker", preference_state)
            if owner == macro_page + "/recording.rs" and name == "start_recording":
                fields = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                methods = called_methods(node)
                if (not {"command", "updates_task", "generation"} <= fields
                        or "receive_recording_update" not in methods
                        or {"detach", "accept_recording_update", "commit_recorded_actions"} & methods):
                    fail(node, "macro_recording_accepts_unowned_completion", macro_page + "/recording.rs")
            if owner == macro_page + "/recording.rs" and name == "receive_recording_update":
                deliveries = [call for call in walk(node) if call.type == "call_expression"
                              and call.child_by_field_name("function").type == "field_expression"
                              and text(call.child_by_field_name("function").child_by_field_name("value")) == "self"
                              and text(call.child_by_field_name("function").child_by_field_name("field")) == "accept_recording_update"]
                guards = [branch for branch in walk(node) if branch.type == "if_expression"
                          and condition_atoms(branch.child_by_field_name("condition")) == {"self.recording.generation!=generation"}]
                if not deliveries or any(not any(preceding_return(guard, call) for guard in guards) for call in deliveries):
                    fail(node, "macro_recording_accepts_stale_session", macro_page + "/recording.rs::receive_recording_update")
            if owner == macro_page + "/recording.rs" and name == "commit_recorded_actions":
                fields = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                commits = [call for call in walk(node) if call.type == "call_expression"
                           and call.child_by_field_name("function").type == "field_expression"
                           and text(call.child_by_field_name("function").child_by_field_name("field")) == "append_recorded_actions"]
                guards = [branch for branch in walk(node) if branch.type == "if_expression"
                          and condition_atoms(branch.child_by_field_name("condition")) == {"self.recording.cancelled"}]
                if (not {"current", "actions", "library_snapshot"} <= fields or not commits
                        or "recording_commit_options" not in called_methods(node)
                        or any(not any(preceding_return(guard, call) for guard in guards) for call in commits)):
                    fail(node, "macro_recording_commit_ignores_current_profile", macro_page + "/recording.rs::commit_recorded_actions")
            if owner == macro_recorder and free_function and name == "spawn_with_registry":
                statements = node.child_by_field_name("body").named_children
                tail = statements[-1] if statements else None
                if (tail is None or tail.type != "call_expression" or resolved_call(text(tail.child_by_field_name("function")).split("::")) != ("spawn_with_geometry",)
                        or [text(argument) for argument in tail.child_by_field_name("arguments").named_children][:3] != ["options", "factory", "registry"]):
                    fail(node, "macro_recorder_bypasses_capture_worker", macro_recorder + "::spawn_with_geometry")
            if owner == macro_recorder and free_function and name == "spawn_with_geometry":
                direct = calls(node)
                if not any(len(parts) >= 2 and aliases.get(parts[-2]) == "WorkerGuard" and parts[-1] == "start" for parts, _ in direct):
                    fail(node, "mapping_worker_bypasses_cleanup_accounting", mapping_workers + "mod.rs")
                if not {"shutdown", "drop"} <= {parts[-1] for parts, _ in direct}:
                    fail(node, "macro_recorder_skips_transport_cleanup", mapping_workers + "mod.rs")
                if not locks_capture_lifecycle(node):
                    fail(node, "macro_recorder_bypasses_shared_capture_lifecycle", "razer_ipc::KEY_CAPTURE_LIFECYCLE")
            if owner == macro_recorder and free_function and name in {"spawn", "mode"}:
                paths = {resolved_call(text(child).split("::")) for child in walk(node) if child.type == "scoped_identifier"}
                if ("super", "recorder_owner", name) not in paths:
                    fail(node, "macro_recorder_bypasses_shared_transport_owner", macro_recorder_owner)
            if owner == keyboard_publisher and name == "macro_mode":
                paths = {resolved_call(text(child).split("::")) for child in walk(node) if child.type == "scoped_identifier"}
                if ("razer_service", "mapping", "recorder", "mode") not in paths:
                    fail(node, "mapping_publisher_bypasses_recorder_owner", macro_recorder_owner)
            if owner == mapping_workers + "mod.rs" and free_function and name == "lock_lifecycle":
                if not locks_capture_lifecycle(node) or "load" not in called_methods(node):
                    fail(node, "capture_lifecycle_bypasses_cancelable_shared_lock", "razer_ipc::KEY_CAPTURE_LIFECYCLE")
            if owner == mapping_workers + "mod.rs" and free_function and name == "apply_execution_control":
                if not any(tuple(parts) == ("control", "apply_with") for parts, _ in calls(node)):
                    fail(node, "mapping_control_helper_bypasses_receipt_owner", mapping_control)
            if owner == keyboard_restore and name == "stop_if_drained" and parent is not None and text(parent.child_by_field_name("type")) == "Owner":
                guarded = []
                for branch in walk(node):
                    if branch.type != "if_expression":
                        continue
                    terms = conjunction_terms(branch.child_by_field_name("condition"))
                    idle = any(method_on_field(term, "is_idle", "coordinator") for term in terms)
                    restored = any(term.type == "binary_expression" and text(term.child_by_field_name("operator")) == "=="
                        and ((method_on_field(term.child_by_field_name("left"), "load", "recoveries") and text(term.child_by_field_name("right")) == "0")
                            or (method_on_field(term.child_by_field_name("right"), "load", "recoveries") and text(term.child_by_field_name("left")) == "0")) for term in terms)
                    if idle and restored:
                        guarded.append(branch.child_by_field_name("consequence"))
                stops = [child for child in walk(node) if method_on_field(child, "store", "valid")
                    or child.type == "assignment_expression" and self_field(child.child_by_field_name("left"), "stopping")]
                if not stops or any(not any(body.start_byte <= stop.start_byte and stop.end_byte <= body.end_byte for body in guarded) for stop in stops):
                    fail(node, "keyboard_exit_stops_before_queue_and_recovery_drain", keyboard_restore + "::Owner")
            if owner == keyboard_publisher and name == "start_with" and "register" not in called_methods(node):
                fail(node, "keyboard_publisher_bypasses_mapping_control_registry", mapping_receipts)
            if owner == keyboard_publisher and name == "apply_control":
                if not any(resolved_call(parts) == ("razer_service", "mapping", "apply_execution_control") for parts, _ in calls(node)):
                    fail(node, "keyboard_publisher_bypasses_receipt_owner", mapping_control)
            if owner == keyboard_publisher and name == "cleanup":
                direct = calls(node)
                shuts = [call for parts, call in direct if resolved_call(parts) == ("shutdown",)
                    and any("".join(text(argument).split()) == "&mutclient" for argument in call.child_by_field_name("arguments").named_children)]
                drops = [call for parts, call in direct if resolved_call(parts) in {("drop",), ("std", "mem", "drop")}
                    and any(text(argument) == "client" for argument in call.child_by_field_name("arguments").named_children)]
                finishes = [call for call in walk(node) if method_on_field(call, "finish", "control")]
                if not finishes or any(not any(shut.end_byte < drop.start_byte < finish.start_byte for shut in shuts for drop in drops) for finish in finishes):
                    fail(node, "keyboard_publisher_releases_registry_before_native_cleanup", keyboard_publisher + "::Publisher")
            if owner == keyboard_worker and name == "run":
                resets = [call for parts, call in calls(node) if parts[-1] == "reset_navigation_feedback"]
                skipped = [branch for branch in walk(node) if branch.type == "if_expression"
                    and branch.child_by_field_name("condition").type == "unary_expression"
                    and branch.child_by_field_name("condition").children[0].type == "!"
                    and any(call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                        and text(call.child_by_field_name("function")) == "automatic.load"
                        for call in walk(branch.child_by_field_name("condition")))
                    and any(child.type == "continue_expression" for child in walk(branch.child_by_field_name("consequence")))]
                if (not resets or not skipped or any(reset.start_byte > branch.start_byte for reset in resets for branch in skipped)
                        or not any(tuple(parts) == ("PendingRecovery", "start") for parts, _ in calls(node))):
                    fail(node, "keyboard_exit_skips_accepted_navigation_recovery", keyboard_worker + "::run")
            if owner == legacy_keyboard_mapping and name == "start_legacy_mapping_owner":
                body = node.child_by_field_name("body")
                retained = {field: [child for child in walk(body) if child.type == "assignment_expression"
                    and legacy_mapping_field(child.child_by_field_name("left"), field)] for field in ("owner", "poll")}
                submissions = [call for call in walk(body) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "submit_legacy_mapping"
                    and call.parent.type == "expression_statement" and call.parent.parent == body]
                deferred = [branch for branch in walk(body) if branch.type == "match_arm"
                    and text(branch.child_by_field_name("pattern")) == "Reply::Ready"
                    and "submit_legacy_mapping" in called_methods(branch.child_by_field_name("value"))]
                if (not submissions or deferred or any(not retained[field]
                        or any(assignment.end_byte > call.start_byte for assignment in retained[field] for call in submissions)
                        for field in retained)):
                    fail(node, "keyboard_initial_publication_waits_for_presentation_ready", legacy_keyboard_mapping + "::start_legacy_mapping_owner")
            if owner == legacy_keyboard_mapping and name == "publish_legacy_mapping":
                connections = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "set_legacy_mapping_connection"]
                for call in connections:
                    block = call.parent
                    while block is not None and block.type != "block":
                        block = block.parent
                    starts = [child for child in walk(block) if child.type == "call_expression"
                        and child.child_by_field_name("function").type == "field_expression"
                        and text(child.child_by_field_name("function").child_by_field_name("field")) == "start_legacy_mapping_owner"
                        and child.start_byte > call.end_byte] if block is not None else []
                    if block is None or not starts or not any(child.type == "assignment_expression"
                            and legacy_mapping_field(child.child_by_field_name("left"), "document")
                            and child.end_byte < call.start_byte for child in walk(block)):
                        fail(call, "keyboard_replacement_registers_before_current_document", legacy_keyboard_mapping + "::publish_legacy_mapping")
            if owner == legacy_keyboard_mapping and name == "set_legacy_mapping_connection" and "start_legacy_mapping_owner" in called_methods(node):
                fail(node, "keyboard_connection_starts_with_retained_stale_document", legacy_keyboard_mapping + "::publish_legacy_mapping")
            if owner == mapping_control and free_function and name in {"apply", "apply_with"}:
                observed = set()
                for child in walk(node):
                    if child.type != "let_declaration":
                        continue
                    declared_type, value = child.child_by_field_name("type"), child.child_by_field_name("value")
                    if declared_type is not None and value is not None and any(
                            resolved_call(parts) == ("serde_json", "from_value") for parts, _ in calls(value)):
                        observed.add(resolved_call(text(declared_type).split("::")))
                required = {"drain", "owner_guid", "revision", "target", "confirms", "acknowledge"}
                methods = called_methods(node)
                direct = {resolved_call(parts) for parts, _ in calls(node)}
                delegated = name == "apply" and ("apply_with",) in direct
                if (not delegated and (("razer_ipc", "mapping_control", "Receipt") not in observed
                        or not required <= methods or not ("request" in methods or ("request",) in direct))):
                    fail(node, "mapping_control_bypasses_correlated_receipt", mapping_receipts)
            if owner == mapping_native_control and name == "request":
                for branch in walk(node):
                    if branch.type != "match_arm":
                        continue
                    pattern, value = branch.child_by_field_name("pattern"), branch.child_by_field_name("value")
                    if pattern is None or value is None or not any(
                            item.type == "struct_pattern" and text(item.child_by_field_name("type")).split("::")[-1] == "MappingExecution"
                            for item in walk(pattern)):
                        continue
                    direct = {resolved_call(parts) for parts, _ in calls(value)}
                    required = {"registration", "ensure_active", "is_some_and", "is_none_or", "mapping", "shortcuts", "result", "reason"}
                    identities = set()
                    for item in walk(value):
                        if item.type != "binary_expression" or text(item.child_by_field_name("operator")) != "==":
                            continue
                        left, right = item.child_by_field_name("left"), item.child_by_field_name("right")
                        if left is not None and left.type == "field_expression":
                            identities.add((text(left.child_by_field_name("field")), text(right)))
                    revisions = [item for item in walk(value) if item.type == "macro_invocation"
                                 and text(item.child_by_field_name("macro")).split("::")[-1] == "ensure"
                                 and "is_none_or" in called_methods(item)]
                    registrations = [item for item in walk(value) if item.type == "call_expression"
                                     and item.child_by_field_name("function").type == "field_expression"
                                     and text(item.child_by_field_name("function").child_by_field_name("field")) == "registration"]
                    scoped_registration = any([text(arg) for arg in call.child_by_field_name("arguments").named_children] == ["scope", "&owner_guid"]
                                              for call in registrations)
                    if (("razer_ipc", "mapping_control", "Receipt", "with_observations") not in direct
                            or not required <= called_methods(value) or len(revisions) != 2
                            or not {("scope", "scope"), ("owner", "owner_guid")} <= identities or not scoped_registration):
                        fail(branch, "native_mapping_control_bypasses_owner_revision_receipt", mapping_receipts)
            if owner == mapping_workers + "key_capture.rs" and name == "start_with_registry":
                for branch in walk(node):
                    if branch.type != "match_arm":
                        continue
                    pattern, value = branch.child_by_field_name("pattern"), branch.child_by_field_name("value")
                    if pattern is None or text(pattern) != "Kind::GlobalShortcut" or value is None or value.type != "block":
                        continue
                    identifiers = {resolved_call(text(item).split("::")) for item in walk(value) if item.type == "scoped_identifier"}
                    if (("razer_ipc", "ServiceRequest", "GlobalShortcutCaptureEvents") not in identifiers
                            or identifiers & {("razer_ipc", "ServiceRequest", "MappingKeyCaptureEvents"),
                                              ("razer_ipc", "ServiceRequest", "DeviceModeEvents")}
                            or "into_events" not in called_methods(value)):
                        fail(branch, "global_capture_splits_input_and_mode_order", "razer_ipc::global_shortcut_capture::Batch")
            if ((owner == mapping_workers + "device.rs" and name == "start_with_registry")
                    or (owner == mapping_workers + "shortcuts.rs" and name == "run")):
                if (not {"register", "finish"} <= called_methods(node)
                        or not any(tuple(parts) == ("super", "control", "apply") for parts, _ in calls(node))):
                    fail(node, "mapping_actor_bypasses_registered_control_owner", mapping_control)
                if owner == mapping_workers + "shortcuts.rs":
                    receivers = {text(parameter.child_by_field_name("pattern"))
                        for parameter in node.child_by_field_name("parameters").named_children
                        if parameter.type == "parameter" and any(kind.type == "generic_type"
                            and text(kind.child_by_field_name("type")).split("::")[-1] == "Receiver"
                            and any(aliases.get(text(argument), text(argument)) == "Command"
                                for argument in kind.child_by_field_name("type_arguments").named_children)
                            for kind in walk(parameter.child_by_field_name("type")))}
                    if any(call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                            and text(call.child_by_field_name("function").child_by_field_name("field")) == "try_recv"
                            and text(call.child_by_field_name("function").child_by_field_name("value")) in receivers
                            for call in walk(node)):
                        fail(node, "shortcut_worker_coalesces_compiler_publications", mapping_workers + "shortcuts.rs::run")
            if (owner in {mapping_workers + "device.rs", mapping_workers + "shortcuts.rs"}
                    and (name == "request_stop_after_pending" or name == ("publish" if owner.endswith("/shortcuts.rs") else "publish_with_restore"))):
                locked = {}
                for declaration in walk(node):
                    if declaration.type != "let_declaration":
                        continue
                    value = declaration.child_by_field_name("value")
                    if value is not None and any(call.type == "call_expression"
                            and call.child_by_field_name("function").type == "field_expression"
                            and text(call.child_by_field_name("function").child_by_field_name("field")) == "lock"
                            and self_field(call.child_by_field_name("function").child_by_field_name("value"), "admission")
                            for call in walk(value)):
                        locked[text(declaration.child_by_field_name("pattern"))] = declaration
                variant = "Finish" if name == "request_stop_after_pending" else "Publish"
                sends = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "send"
                    and self_field(call.child_by_field_name("function").child_by_field_name("value"), "commands")
                    and any(local_variant(argument, "Command", variant)
                            for argument in call.child_by_field_name("arguments").named_children)]
                if variant == "Publish":
                    rejections = []
                    for branch in walk(node):
                        if branch.type != "if_expression":
                            continue
                        for condition in disjunction_terms(branch.child_by_field_name("condition")):
                            if condition.type != "unary_expression" or condition.children[0].type != "!":
                                continue
                            arguments = macro_arguments(condition.named_children[0], "matches")
                            if (len(arguments) == 2 and arguments[0].type == "unary_expression"
                                    and arguments[0].children[0].type == "*"
                                    and arguments[0].named_children[0].text.decode() in locked
                                    and local_variant(arguments[1], "Admission", "Accepting")
                                    and any(child.type == "return_expression" for child in walk(branch.child_by_field_name("consequence")))):
                                rejections.append(branch)
                    if (not sends or not rejections or not locked
                            or any(not any(lock.end_byte < branch.start_byte < send.start_byte
                                for lock in locked.values() for branch in rejections) for send in sends)):
                        fail(node, "mapping_publish_bypasses_closed_admission", owner + "::Session")
                else:
                    closing = [child for child in walk(node) if child.type == "assignment_expression"
                        and child.child_by_field_name("left").type == "unary_expression"
                        and child.child_by_field_name("left").children[0].type == "*"
                        and text(child.child_by_field_name("left").named_children[0]) in locked
                        and local_variant(child.child_by_field_name("right"), "Admission", "Draining")]
                    cancels = [call for call in walk(node) if call.type == "call_expression"
                        and call.child_by_field_name("function").type == "field_expression"
                        and any(self_field(call.child_by_field_name("function").child_by_field_name("value"), field) for field in ("canceled", "stop"))]
                    if (not sends or not closing or cancels or "request_stop" in called_methods(node)
                            or any(not any(lock.end_byte < close.start_byte < send.start_byte
                                for lock in locked.values() for close in closing) for send in sends)):
                        fail(node, "mapping_drain_bypasses_fifo_admission", owner + "::Session")
            if owner == macro_recorder and free_function and name == "record":
                observed = set()
                for child in walk(node):
                    if child.type != "let_declaration":
                        continue
                    declared_type, value = child.child_by_field_name("type"), child.child_by_field_name("value")
                    if declared_type is None or value is None:
                        continue
                    contract = resolved_call(text(declared_type).split("::"))
                    if any(resolved_call(parts) == ("serde_json", "from_value") for parts, _ in calls(value)):
                        observed.add(contract)
                expected = {("razer_ipc", "macro_recording", "SuspensionReceipt"), ("razer_ipc", "macro_recording", "RestoreReceipt")}
                if not expected <= observed or not {"suspended", "global_shortcuts", "result"} <= called_methods(node):
                    fail(node, "macro_recorder_bypasses_typed_receipts", macro_receipts)
            if owner == macro_native and name == "recorder_request":
                for expected in ("SuspensionReceipt", "RestoreReceipt"):
                    if not any(resolved_call(parts) == ("razer_ipc", "macro_recording", expected, "new") for parts, _ in calls(node)):
                        fail(node, "native_macro_receipt_bypasses_shared_contract", macro_receipts)
            if owner == macro_native and name == "release_recorder":
                methods = called_methods(node)
                confirmations = [parts for parts, _ in calls(node) if parts[-1:] == ["confirmed"]]
                if "restore" not in methods or not confirmations:
                    fail(node, "native_macro_cleanup_accepts_unconfirmed_restore", macro_native + "::release_recorder")
            if owner == shortcut_owner:
                methods = called_methods(node)
                direct = calls(node)
                if name == "encode_input" and (not {"encode_shortcut_input", "into_parts"} <= methods
                        or not any(len(parts) >= 2 and aliases.get(parts[-2]) == "MappingEngine" and parts[-1] == "global_shortcuts" for parts, _ in direct)):
                    fail(node, "shortcut_input_bypasses_shared_engine", input_owner)
                targets = {aliases.get(parts[-1], parts[-1]) for parts, _ in direct}
                if (name in {"encode_shortcuts", "encode_with_context"}
                        and not {"canonical_json", "source_hash"} <= targets
                        and not (name == "encode_shortcuts" and "encode_with_context" in targets)):
                    fail(node, "shortcut_hash_bypasses_shared_owner", hash_owner)
            if owner in {mapping_workers + "device.rs", mapping_workers + "key_capture.rs", mapping_workers + "shortcuts.rs"} and name in {"start_with", "start_with_registry"} and parent is not None and text(parent.child_by_field_name("type")) == "Session":
                if not any(len(parts) >= 2 and aliases.get(parts[-2]) == "WorkerGuard" and parts[-1] == "start" for parts, _ in calls(node)):
                    fail(node, "mapping_worker_bypasses_cleanup_accounting", mapping_workers + "mod.rs")
                if owner == mapping_workers + "key_capture.rs" and not locks_capture_lifecycle(node):
                    fail(node, "key_capture_bypasses_shared_lifecycle", "razer_ipc::KEY_CAPTURE_LIFECYCLE")
        if node.type == "function_item" and owner == factory_owner:
            name = text(node.child_by_field_name("name"))
            parent_impl = node.parent.parent if node.parent is not None else None
            if (name == "current" and parent_impl is not None and parent_impl.type == "impl_item"
                    and text(parent_impl.child_by_field_name("type")) == "SavedRevisionOwner"):
                bindings = [child for child in walk(node) if child.type == "let_declaration"
                    and text(child.child_by_field_name("pattern")) == "owner"
                    and "".join(text(child.child_by_field_name("value")).split()) == "self.workspace.upgrade()?"]
                scoped_returns = [call for call in walk(node) if call.type == "call_expression"
                    and call == node.child_by_field_name("body").named_children[-1]
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "then_some"
                    and condition_atoms(call.child_by_field_name("function").child_by_field_name("value")) == {
                        "devices.contains(&owner)", "owner.read(cx).identity(cx)==identity",
                        "owner.read(cx).connection_revision()==self.connection_revision"}
                    and [text(argument) for argument in call.child_by_field_name("arguments").named_children] == ["owner"]]
                if not bindings or not scoped_returns:
                    fail(node, "save_acknowledgement_ignores_connection_revision", factory_owner + "::SavedRevisionOwner")
            if name == "cancel_pending_exit":
                body = node.child_by_field_name("body")
                resets = [call for parts, call in calls(node) if resolved_call(parts) == ("std", "mem", "take")
                    and any("".join(text(argument).split()) == "&mutself.close_requested"
                        for argument in call.child_by_field_name("arguments").named_children)]
                restores = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) in {"cancel_keyboard_latest_exit", "cancel_keyboard_game_exit", "cancel_exit"}
                    and text(call.child_by_field_name("function").child_by_field_name("value")) != "runtime"]
                runtime_restores = [call for call in walk(body) if method_on_field(call, "update", "runtime")
                    and call.parent.type == "expression_statement" and call.parent.parent == body
                    and any(child.type == "call_expression" and child.child_by_field_name("function").type == "field_expression"
                        and text(child.child_by_field_name("function").child_by_field_name("field")) == "cancel_exit"
                        and text(child.child_by_field_name("function").child_by_field_name("value")) == "runtime"
                        for child in walk(call))]
                branches = [child for child in walk(node) if child.type == "if_expression"]
                methods = called_methods(node)
                if (not resets or not {"resume_global_shortcuts", "cancel_keyboard_latest_exit", "cancel_keyboard_game_exit", "cancel_exit", "defer", "notify"} <= methods
                        or not any(text(call.child_by_field_name("function").child_by_field_name("field")) == "cancel_exit" for call in restores)
                        or not runtime_restores or any(not any(reset.end_byte < call.start_byte for reset in resets)
                            for call in runtime_restores)
                        or not any(resolved_call(parts) == ("razer_state", "ui_preferences", "cancel_exit") for parts, _ in calls(node))
                        or any(not any(condition_atoms(branch.child_by_field_name("condition")) == {"shell.close_requested"}
                            and preceding_return(branch, call) for branch in branches) for call in restores)
                        or not any(child.type == "assignment_expression" and self_field(child.child_by_field_name("left"), "background_exit_task")
                            and text(child.child_by_field_name("right")) == "None" for child in walk(node))):
                    fail(node, "failed_exit_skips_shared_admission_recovery", factory_owner + "::cancel_pending_exit")
            if name == "workspace_file":
                members = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                if not required_fields <= members:
                    fail(node, "incomplete_shared_workspace_snapshot", factory_owner)
            if name == "saved_revision_owner":
                members = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                if not {"current", "workspace_identity"} <= members:
                    fail(node, "unscoped_save_acknowledgement", factory_owner)
            if name == "capture_save_owners":
                captures = [child for child in walk(node) if child.type == "struct_expression"
                    and text(child.child_by_field_name("name")) == "SavedRevisionOwner"]
                for capture in captures:
                    body = capture.child_by_field_name("body")
                    fields = {text(child.child_by_field_name("field")): "".join(text(child.child_by_field_name("value")).split())
                        for child in body.named_children if child.type == "field_initializer"}
                    if fields != {"workspace": "owner.downgrade()",
                                  "connection_revision": "owner.read(cx).connection_revision()"}:
                        fail(capture, "save_owner_epoch_is_not_captured_with_entity", factory_owner)
                if not captures:
                    fail(node, "save_owner_epoch_is_not_captured_with_entity", factory_owner)
            if name == "prepare_save":
                members = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                if "capture_save_owners" not in members:
                    fail(node, "uncaptured_save_owners", factory_owner)
        if node.type == "function_item" and owner.startswith("crates/razer-shell/src/") and text(node.child_by_field_name("name")) == "start_pending_save":
            methods = [child for child in walk(node) if child.type == "call_expression" and child.child_by_field_name("function").type == "field_expression"]
            names = [(text(child.child_by_field_name("function").child_by_field_name("field")), child) for child in methods]
            if any(name == "capture_save_owners" for name, _ in names):
                fail(node, "save_owners_recaptured_after_queue", factory_owner)
            validation = next((call for name, call in names if name == "prepared_save_owners_current"), None)
            rebase = next((call for name, call in names if name.startswith("rebase_")), None)
            if validation is None or (rebase is not None and validation.start_byte > rebase.start_byte):
                fail(node, "queued_save_not_validated_before_rebase", factory_owner)
        consumer = publication_consumers.get(owner)
        parent_impl = node.parent.parent if node.parent is not None and node.parent.parent is not None else None
        if (node.type == "function_item" and consumer is not None
                and text(node.child_by_field_name("name")) == consumer[1]
                and (consumer[0] is None or (parent_impl is not None and parent_impl.type == "impl_item"
                     and text(parent_impl.child_by_field_name("type")) == consumer[0]))):
            staged_bindings = set()
            for child in walk(node):
                if child.type != "let_declaration":
                    continue
                value = child.child_by_field_name("value")
                if value is None:
                    continue
                for call in walk(value):
                    if call.type == "call_expression":
                        parts = text(call.child_by_field_name("function")).split("::")
                        if len(parts) >= 2 and parts[-1] == "write" and aliases.get(parts[-2]) == "StagedResource":
                            staged_bindings.add(text(child.child_by_field_name("pattern")))
            publishes = [child for child in walk(node) if child.type == "call_expression"
                         and child.child_by_field_name("function").type == "field_expression"
                         and text(child.child_by_field_name("function").child_by_field_name("field")) == "publish"
                         and text(child.child_by_field_name("function").child_by_field_name("value")) in staged_bindings]
            if not staged_bindings or not publishes:
                fail(node, "resource_publication_bypasses_shared_owner", publication_owner)
        if node.type == "function_item" and text(node.child_by_field_name("name")) == "atomic_replace" and owner.startswith(("crates/razer-resource-db/", "crates/razer-resources/")) and owner != publication_owner:
            fail(node, "duplicate_resource_publication_owner", publication_owner)
        if node.type == "function_item" and owner == mapping_adapter:
            name = text(node.child_by_field_name("name"))
            methods = called_methods(node)
            if name == "sync_local_mapping_targets" and ("sync_interdevice_holds" not in methods or "consume_interdevice_mapping" in methods):
                fail(node, "ui_refresh_replays_held_interdevice_mapping", interdevice_owner)
            if name == "sync_interdevice_holds":
                if not {"observe", "released", "replayed", "first_mapping_target", "matches"} <= methods:
                    fail(node, "interdevice_sync_bypasses_lifecycle_owner", interdevice_owner)
                for call in walk(node):
                    if call.type != "call_expression" or text(call.child_by_field_name("function")) != "dispatch_to_target":
                        continue
                    ancestor = call.parent
                    while ancestor is not None and ancestor != node:
                        if ancestor.type == "if_expression" and "replayed" in called_methods(ancestor.child_by_field_name("condition")):
                            break
                        ancestor = ancestor.parent
                    if ancestor is None or ancestor == node:
                        fail(call, "held_interdevice_replay_without_transition", interdevice_owner)
            if name == "dispatch_to_target" and ("matches" not in methods or "upgrade" not in methods):
                fail(node, "interdevice_dispatch_accepts_stale_owner", interdevice_owner)
            if name == "route_mapping_callback":
                fields = [child for child in walk(node) if child.type == "field_expression" and text(child.child_by_field_name("field")) == "close_requested"]
                first_dispatch = next((call for _, call in calls(node) if text(call.child_by_field_name("function")) == "dispatch_to_target"), None)
                if not fields or first_dispatch is not None and fields[0].start_byte > first_dispatch.start_byte:
                    fail(node, "interdevice_callback_accepted_during_exit", interdevice_owner)
            if name == "release_interdevice_holds" and ("release_hold" not in {parts[-1] for parts, _ in calls(node)} or "into_values" not in methods):
                fail(node, "interdevice_exit_skips_held_release", interdevice_owner)
            if name == "retain_interdevice_connections" and not {"matches", "owner", "observe", "released", "insert"} <= methods:
                fail(node, "interdevice_disconnect_skips_held_release", interdevice_owner)
            parent = node.parent.parent if node.parent is not None and node.parent.parent is not None else None
            if parent is not None and parent.type == "impl_item" and text(parent.child_by_field_name("type")) == "MappingTargetOwner":
                if name in {"capture", "matches"}:
                    fields = {text(child.child_by_field_name("field")) for child in walk(node) if child.type == "field_expression"}
                    if not {"identity", "connection_revision"} <= fields:
                        fail(node, "interdevice_owner_ignores_connection_revision", interdevice_owner)
        if node.type == "function_item" and owner == "crates/razer-shell/src/shell.rs" and text(node.child_by_field_name("name")) == "request_exit":
            release = next((call for call in walk(node) if call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                            and text(call.child_by_field_name("function").child_by_field_name("field")) == "release_interdevice_holds"), None)
            closing = [child for child in walk(node) if child.type == "assignment_expression" and text(child.child_by_field_name("left")) == "self.close_requested" and text(child.child_by_field_name("right")) == "true"]
            stops = [call for call in walk(node) if call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                     and text(call.child_by_field_name("function").child_by_field_name("field")) in {"prepare_exit", "prepare_legacy_dpi_exit"}]
            if release is None or not closing or any(call.start_byte < release.start_byte for call in stops):
                fail(node, "interdevice_release_after_worker_stop", interdevice_owner)
            starts = [call for parts, call in calls(node) if resolved_call(parts) == ("razer_state", "ui_preferences", "begin_exit")]
            cancellations = [call for call in walk(node) if call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                             and text(call.child_by_field_name("function").child_by_field_name("field")) == "cancel_transfer"]
            if (not starts or not cancellations or any(call.start_byte < starts[0].start_byte for call in stops)
                    or cancellations[0].start_byte > starts[0].start_byte):
                fail(node, "shared_local_io_admitted_during_exit", preference_state + " and " + macro_io)
            if "dismiss_for_navigation" not in called_methods(node) or not any(
                    call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "prepare_exit"
                    and text(call.child_by_field_name("function").child_by_field_name("value")) == "workspace"
                    for call in walk(node)):
                fail(node, "exit_keeps_retained_capture_or_product_owners", source_lifecycle)
        if node.type == "function_item" and owner == source_lifecycle:
            name = text(node.child_by_field_name("name"))
            if name == "prepare_exit":
                assignments = [child for child in walk(node) if child.type == "assignment_expression"
                    and text(child.child_by_field_name("left")) == "self.exiting" and text(child.child_by_field_name("right")) == "true"]
                stops = [call for call in walk(node) if call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) in {"set_active", "dismiss_editors", "finish_mapping_service", "prepare_exit"}]
                if (not assignments or not {"set_active", "dismiss_editors", "prepare_exit"} <= called_methods(node)
                        or "finish_mapping_service" in called_methods(node)
                        or any(call.start_byte < assignments[0].start_byte for call in stops)):
                    fail(node, "product_exit_keeps_session_admission_or_workers", source_lifecycle)
            if name == "finish_exit" and "finish_mapping_service" not in called_methods(node):
                fail(node, "product_exit_skips_pending_mapping_drain", source_lifecycle)
            if name == "cancel_exit":
                reset = any(child.type == "assignment_expression" and text(child.child_by_field_name("left")) == "self.exiting"
                    and text(child.child_by_field_name("right")) == "false" for child in walk(node))
                if not reset or not {"cancel_mapping_service_exit", "set_active"} <= called_methods(node):
                    fail(node, "failed_exit_does_not_restore_product_admission", source_lifecycle)
        if node.type == "function_item" and owner == runtime_panel:
            implementation = node.parent.parent if node.parent is not None and node.parent.parent is not None else None
            if implementation is not None and implementation.type == "impl_item" and text(implementation.child_by_field_name("type")) == "RuntimeBridge":
                name = text(node.child_by_field_name("name"))
                body = node.child_by_field_name("body")
                if name == "start_worker":
                    workers = {text(child.child_by_field_name("pattern")) for child in body.named_children
                        if child.type == "let_declaration" and child.child_by_field_name("value") is not None
                        and "spawn" in called_methods(child.child_by_field_name("value"))}
                    retained = [field for child in walk(body) if child.type == "struct_expression"
                        and text(child.child_by_field_name("name")) == "ConnectionOwner"
                        for field in child.child_by_field_name("body").named_children if field.type == "field_initializer"
                        and text(field.child_by_field_name("field")) == "worker"
                        and any(resolved_call(parts) == ("Some",)
                            and any(text(argument) in workers for argument in call.child_by_field_name("arguments").named_children)
                            for parts, call in calls(field.child_by_field_name("value")))]
                    if not workers or not retained or "detach" in called_methods(node):
                        fail(node, "discovery_connection_discards_worker_handle", runtime_panel + "::ConnectionOwner")
                if name == "disconnect":
                    worker_takes = [child for child in body.named_children if child.type == "let_declaration"
                        and child.child_by_field_name("value") is not None
                        and {"lock", "take"} <= called_methods(child.child_by_field_name("value"))
                        and any(part.type == "field_expression" and text(part.child_by_field_name("field")) == "worker"
                            for part in walk(child.child_by_field_name("value")))]
                    completed = [child for child in body.named_children if child.type == "let_declaration"
                        and child.child_by_field_name("value") is not None and "join" in called_methods(child.child_by_field_name("value"))]
                    early_exits = []
                    for child in walk(body):
                        if child.type not in {"return_expression", "try_expression"}:
                            continue
                        ancestor = child.parent
                        while ancestor is not None and ancestor != body and ancestor.type != "closure_expression":
                            ancestor = ancestor.parent
                        if ancestor == body:
                            early_exits.append(child)
                    merged = [child for child in body.named_children if child.type == "expression_statement"
                        and child.named_children and child.named_children[0].type == "match_expression"]
                    merged.extend(child for child in body.named_children if child.type == "match_expression")
                    joined_names = {text(child.child_by_field_name("pattern")) for child in completed}
                    if (not worker_takes or not completed or early_exits
                            or any(not any(take.end_byte < join.start_byte for take in worker_takes) for join in completed)
                            or not any(child.child_by_field_name("value").type == "tuple_expression"
                                and any(text(value) in joined_names for value in child.child_by_field_name("value").named_children)
                                and len(child.child_by_field_name("value").named_children) == 2
                                for item in merged for child in [item.named_children[0] if item.type == "expression_statement" else item])):
                        fail(node, "discovery_connection_shutdown_does_not_join_after_ack_failure", runtime_panel + "::RuntimeBridge")
            if implementation is not None and implementation.type == "impl_item" and text(implementation.child_by_field_name("type")) == "RuntimePanel":
                name = text(node.child_by_field_name("name"))
                if name == "prepare_exit":
                    prepared = [child for child in walk(node) if child.type == "assignment_expression"
                        and self_field(child.child_by_field_name("left"), "exit_prepared")
                        and text(child.child_by_field_name("right")) == "true"]
                    if (not prepared or "cancel_read" not in called_methods(node) or {"disconnect", "begin_disconnect"} & called_methods(node)
                            or any(resolved_call(parts)[-1] == "DiscoveryObserved" for parts, _ in calls(node))):
                        fail(node, "exit_preparation_invalidates_accepted_device_observations", runtime_panel + "::RuntimePanel")
                if name == "refresh_queries":
                    statements = [child for child in node.child_by_field_name("body").named_children
                        if child.type not in {"line_comment", "block_comment"}]
                    first = statements[0] if statements else None
                    if first is not None and first.type == "expression_statement" and first.named_children:
                        first = first.named_children[0]
                    if (first is None or first.type != "if_expression" or not returning_branch(first)
                            or not any(self_field(term, "exit_prepared") for term in disjunction_terms(first.child_by_field_name("condition")))):
                        fail(node, "discovery_queries_admitted_during_exit", runtime_panel + "::RuntimePanel")
                    retired = [call for call in walk(node) if call.type == "call_expression"
                        and call.child_by_field_name("function").type == "field_expression"
                        and text(call.child_by_field_name("function").child_by_field_name("value")) == "this.retired_bridges"]
                    if (not any(text(call.child_by_field_name("function").child_by_field_name("field")) == "push"
                            and [text(argument) for argument in call.child_by_field_name("arguments").named_children] == ["bridge"]
                            for call in retired)
                            or not any(text(call.child_by_field_name("function").child_by_field_name("field")) == "extend"
                                and any(child.type == "call_expression" and text(child.child_by_field_name("function")) == "this.bridge.take"
                                    for child in walk(call)) for call in retired)):
                        fail(node, "discovery_refresh_failure_discards_connection_join_owner", runtime_panel + "::RuntimePanel")
                if name == "monitor_device_changes":
                    retired = [call for call in walk(node) if call.type == "call_expression"
                        and call.child_by_field_name("function").type == "field_expression"
                        and text(call.child_by_field_name("function").child_by_field_name("field")) == "extend"
                        and text(call.child_by_field_name("function").child_by_field_name("value")) == "this.retired_monitors"
                        and any(child.type == "call_expression" and text(child.child_by_field_name("function")) == "this.monitor.take"
                            for child in walk(call))]
                    if not retired or any(child.type == "assignment_expression" and text(child.child_by_field_name("left")) == "this.monitor"
                            and text(child.child_by_field_name("right")) == "None" for child in walk(node)):
                        fail(node, "failed_discovery_observer_discards_its_join_owner", runtime_panel + "::RuntimePanel")
                if name == "disconnect":
                    invokes = [call for call in walk(node) if call.type == "call_expression"
                        and text(call.child_by_field_name("function")) == "self.begin_disconnect"
                        and call.child_by_field_name("arguments").named_children
                        and local_variant(call.child_by_field_name("arguments").named_children[0], "DisconnectIntent", "User")]
                    if not invokes:
                        fail(node, "manual_discovery_disconnect_loses_user_intent", runtime_panel + "::RuntimePanel")
                if name == "shutdown_for_app_quit":
                    prepared = [child for child in walk(node) if child.type == "assignment_expression"
                        and self_field(child.child_by_field_name("left"), "exit_prepared")
                        and text(child.child_by_field_name("right")) == "true"]
                    invokes = [call for call in walk(node) if call.type == "call_expression"
                        and text(call.child_by_field_name("function")) == "self.begin_disconnect"
                        and call.child_by_field_name("arguments").named_children
                        and local_variant(call.child_by_field_name("arguments").named_children[0], "DisconnectIntent", "ApplicationExit")]
                    retained = [call for call in walk(node) if method_on_field(call, "take", "disconnect_task")]
                    if (not prepared or not invokes or not retained
                            or not any(prepare.end_byte < invoke.start_byte and invoke.end_byte < task.start_byte
                                for prepare in prepared for invoke in invokes for task in retained)):
                        fail(node, "application_discovery_shutdown_loses_completion_or_intent", runtime_panel + "::RuntimePanel")
                if name == "begin_disconnect":
                    loops = [child for child in walk(node) if child.type == "for_expression"
                        and text(child.child_by_field_name("value")) == "monitors"
                        and "shutdown" in called_methods(child.child_by_field_name("body"))
                        and not any(part.type in {"try_expression", "return_expression", "break_expression"}
                            for part in walk(child.child_by_field_name("body")))]
                    connections = [call for call in walk(node) if call.type == "call_expression"
                        and text(call.child_by_field_name("function")) == "bridge.disconnect"]
                    background = [call for call in walk(node) if call.type == "call_expression"
                        and call.child_by_field_name("function").type == "field_expression"
                        and text(call.child_by_field_name("function").child_by_field_name("field")) == "background_spawn"
                        and text(call.child_by_field_name("function").child_by_field_name("value")) == "cx"]
                    retained = [child for child in walk(node) if child.type == "assignment_expression"
                        and self_field(child.child_by_field_name("left"), "disconnect_task") and "spawn" in called_methods(child)]
                    active = [call for call in walk(node) if method_on_field(call, "take", "monitor")]
                    retired = [call for parts, call in calls(node) if resolved_call(parts) == ("std", "mem", "take")
                        and any("".join(text(argument).split()) == "&mutself.retired_monitors"
                            for argument in call.child_by_field_name("arguments").named_children)]
                    retired_connections = [call for parts, call in calls(node) if resolved_call(parts) == ("std", "mem", "take")
                        and any("".join(text(argument).split()) == "&mutself.retired_bridges"
                            for argument in call.child_by_field_name("arguments").named_children)]
                    active_connections = [call for call in walk(node) if method_on_field(call, "take", "bridge")]
                    connection_loops = [child for child in walk(node) if child.type == "for_expression"
                        and text(child.child_by_field_name("value")) == "bridges"
                        and "disconnect" in called_methods(child.child_by_field_name("body"))
                        and not any(part.type in {"try_expression", "return_expression", "break_expression"}
                            for part in walk(child.child_by_field_name("body")))]
                    gates = [branch for branch in walk(node) if branch.type == "if_expression" and returning_branch(branch)
                        and condition_atoms(branch.child_by_field_name("condition")) == {
                            "self.bridge.is_none()", "self.retired_bridges.is_empty()", "self.monitor.is_none()", "self.retired_monitors.is_empty()"}]
                    if (not retained or not active or not retired or not gates or not loops or not connections
                            or not active_connections or not retired_connections or not connection_loops
                            or any(not any(loop.end_byte < connection.start_byte and parent.start_byte < loop.start_byte
                                and connection.end_byte < parent.end_byte for loop in loops for parent in background)
                                for connection in connections)):
                        fail(node, "discovery_shutdown_skips_or_blocks_observation_join", runtime_panel + "::RuntimePanel")
                    emitted = [call for parts, call in calls(node) if resolved_call(parts)[-1] == "DiscoveryObserved"]
                    def only_user_branch(call):
                        ancestor = call.parent
                        while ancestor is not None and ancestor != node:
                            if ancestor.type == "if_expression":
                                body = ancestor.child_by_field_name("consequence")
                                args = macro_arguments(ancestor.child_by_field_name("condition"), "matches")
                                if (body.start_byte < call.start_byte and call.end_byte < body.end_byte
                                        and [part.text for part in args] == [b"intent", b"DisconnectIntent::User"]):
                                    return True
                            ancestor = ancestor.parent
                        return False
                    if not emitted or any(not only_user_branch(call) for call in emitted):
                        fail(node, "application_exit_republishes_artificial_device_disconnect", runtime_panel + "::RuntimePanel")
        if node.type == "function_item" and owner == "crates/razer-shell/src/shell.rs" and text(node.child_by_field_name("name")) == "background_exit_pending":
            actual = {resolved_call(parts) for parts, _ in calls(node)}
            required = {("razer_service", "mapping", "active_workers"), ("razer_storage", "file_transfer", "active_io"),
                        ("razer_service", "seiren_audio", "active_workers"),
                        ("razer_state", "ui_preferences", "active_io"), ("razer_state", "chroma_settings", "active_io")}
            if not required <= actual:
                fail(node, "shared_local_io_not_drained_at_exit", "Shared service, file transfer, Chroma and UI preference owners")
        if node.type == "function_item" and owner == "crates/razer-shell/src/shell.rs" \
                and text(node.child_by_field_name("name")) == "new":
            implementation = node.parent.parent if node.parent is not None and node.parent.parent is not None else None
            if implementation is not None and implementation.type == "impl_item" \
                    and text(implementation.child_by_field_name("type")) == "AppShell" \
                    and not any(call.type == "call_expression" and text(call.child_by_field_name("function")) in {
                        "this.install_keyboard_brightness_cleanup", "self.install_keyboard_brightness_cleanup"} for call in walk(node)):
                fail(node, "keyboard_brightness_cleanup_is_not_installed", brightness_runtime)
        if node.type == "function_item" and owner == "crates/razer-shell/src/shell.rs" and text(node.child_by_field_name("name")) == "continue_save_queue":
            methods = [call for call in walk(node) if call.type == "call_expression"
                and call.child_by_field_name("function").type == "field_expression"]
            failures = [call for call in methods
                if text(call.child_by_field_name("function").child_by_field_name("field")) == "cancel_pending_exit"
                and "!succeeded" in enclosing_conditions(call)]
            finishes = [call for call in methods
                if text(call.child_by_field_name("function").child_by_field_name("field")) in {"finish_exit", "stop_global_shortcuts"}]
            names = {text(call.child_by_field_name("function").child_by_field_name("field")) for call in finishes}
            branches = [branch for branch in walk(node) if branch.type == "if_expression"]
            continuations = [call for parts, call in calls(node)
                if resolved_call(parts) == ("save_continuation",)]
            cleanup_waits = [branch for branch in branches if returning_branch(branch)
                and method_on_field(branch.child_by_field_name("condition"), "defer_save_continuation", "service_exit")
                and [text(argument) for argument in branch.child_by_field_name("condition").child_by_field_name("arguments").named_children] == ["succeeded"]]
            if continuations and any(not any(preceding_return(branch, call) for branch in cleanup_waits)
                    for call in continuations + failures):
                fail(node, "cleanup_completion_consumes_exit_before_owners_resume", shutdown_owner + "::State")
            save_waits = [branch for branch in branches
                if condition_atoms(branch.child_by_field_name("condition")) == {"self.save_task.is_some()"}
                and returning_branch(branch)]
            if not failures or not save_waits or any(not any(failure.end_byte < branch.start_byte for failure in failures) for branch in save_waits):
                fail(node, "failed_exit_keeps_preferences_closed", factory_owner + "::cancel_pending_exit")
            gates = {"succeeded", "!queued", "!auxiliary_pending", "self.close_requested"}
            waiting = [branch for branch in branches if background_wait_branch(branch)]
            quits = [call for call in methods if text(call.child_by_field_name("function").child_by_field_name("field")) == "quit"]
            if (not {"finish_exit", "stop_global_shortcuts"} <= names or not quits or not waiting
                    or any(not gates <= enclosing_conditions(call)
                        or not any(condition_atoms(branch.child_by_field_name("condition")) == {"self.save_task.is_some()"}
                            and preceding_return(branch, call) for branch in branches)
                        or not any(condition_atoms(branch.child_by_field_name("condition")) == {"operations_pending"}
                            and preceding_return(branch, call) for branch in branches) for call in finishes)
                    or any(not any(gates <= enclosing_conditions(branch)
                            and all(call.end_byte < branch.start_byte for call in finishes)
                            and branch.end_byte < quit.start_byte for branch in waiting) for quit in quits)):
                fail(node, "exit_stops_mapping_before_requested_saves_drain", source_lifecycle)
            resource_waits = [branch for branch in branches if returning_branch(branch)
                and "finish_resource_exit" in called_methods(branch.child_by_field_name("condition"))]
            resource_finishes = [call for call in methods
                if text(call.child_by_field_name("function").child_by_field_name("field")) == "finish_resource_exit"]
            if (not resource_waits or not resource_finishes
                    or any(not gates <= enclosing_conditions(call)
                        or not any(condition_atoms(branch.child_by_field_name("condition")) == {"operations_pending"}
                            and preceding_return(branch, call) for branch in branches)
                        or not any(preceding_return(branch, call) for branch in waiting) for call in resource_finishes)
                    or any(not any(branch.end_byte < quit.start_byte for branch in resource_waits) for quit in quits)):
                fail(node, "exit_quits_before_resource_owner_is_joined", resource_bridge)
            service_waits = [branch for branch in branches if returning_branch(branch)
                and "finish_service_exit" in called_methods(branch.child_by_field_name("condition"))]
            service_finishes = [call for call in methods
                if text(call.child_by_field_name("function").child_by_field_name("field")) == "finish_service_exit"]
            def service_before_resource(branch, call):
                if preceding_return(branch, call):
                    return True
                terms = disjunction_terms(branch.child_by_field_name("condition"))
                service_terms = [index for index, term in enumerate(terms) if "finish_service_exit" in called_methods(term)]
                resource_terms = [index for index, term in enumerate(terms)
                    if term.start_byte <= call.start_byte and call.end_byte <= term.end_byte]
                return any(service < resource for service in service_terms for resource in resource_terms)
            if (not service_waits or not service_finishes
                    or any(not gates <= enclosing_conditions(call)
                        or not any(condition_atoms(branch.child_by_field_name("condition")) == {"operations_pending"}
                            and preceding_return(branch, call) for branch in branches)
                        or not any(preceding_return(branch, call) for branch in waiting) for call in service_finishes)
                    or any(not any(service_before_resource(branch, call) for branch in service_waits)
                        for call in resource_finishes)
                    or any(not any(branch.end_byte < quit.start_byte for branch in service_waits) for quit in quits)):
                fail(node, "exit_quits_before_service_cleanups_complete", shutdown_owner)
            actuation_waits = [branch for branch in branches if returning_branch(branch)
                and any("finish_keyboard_actuation_exit" in called_methods(term)
                    for term in disjunction_terms(branch.child_by_field_name("condition")))
                and any("".join(text(term).split()) == "!self.close_requested"
                    for term in disjunction_terms(branch.child_by_field_name("condition")))]
            actuation_finishes = [call for call in methods
                if text(call.child_by_field_name("function").child_by_field_name("field")) == "finish_keyboard_actuation_exit"]
            if (not actuation_waits or not actuation_finishes
                    or any(not gates <= enclosing_conditions(call)
                        or not any(condition_atoms(branch.child_by_field_name("condition")) == {"operations_pending"}
                            and preceding_return(branch, call) for branch in branches) for call in actuation_finishes)
                    or any(not any(preceding_return(branch, call) for branch in actuation_waits)
                        for call in finishes + service_finishes + resource_finishes)):
                fail(node, "exit_cancels_accepted_actuation_or_skips_failure_gate", keyboard_actuation)
            pending = [child.child_by_field_name("value") for child in node.child_by_field_name("body").named_children
                if child.type == "let_declaration" and text(child.child_by_field_name("pattern")) == "operations_pending"]
            if (len(pending) != 1 or pending[0] is None
                    or not any(method_on_field(term, "pending", "keyboard_brightness") for term in disjunction_terms(pending[0]))):
                fail(node, "keyboard_brightness_work_is_not_drained_before_exit", brightness_runtime + "::State")
        if node.type == "function_item" and owner == keyboard_actuation:
            name = text(node.child_by_field_name("name"))
            if name in {"finish_keyboard_actuation_save", "submit_keyboard_actuation_device"}:
                def nested_callback(child):
                    parent = child.parent
                    while parent is not None and parent != node:
                        if parent.type == "closure_expression":
                            return True
                        parent = parent.parent
                    return False
                if any(child.type == "field_expression" and text(child.child_by_field_name("field")) == "close_requested"
                        and not nested_callback(child)
                        for child in walk(node)):
                    fail(node, "accepted_actuation_is_rejected_by_application_close", keyboard_actuation)
            if name == "finish_keyboard_actuation_exit":
                methods = called_methods(node)
                waited_fields = {field for field in ("keyboard_actuation_operations", "keyboard_actuation_workers", "keyboard_actuation_retired")
                    if any(method_on_field(call, "is_empty", field) for call in walk(node))}
                if (not {"stop_after_pending", "reap_keyboard_actuation_workers"} <= methods
                        or len(waited_fields) != 3
                        or {"store", "clear"} & methods):
                    fail(node, "actuation_exit_cancels_accepted_native_fifo", keyboard_actuation_worker)
        if node.type == "function_item" and owner == keyboard_actuation_worker:
            name = text(node.child_by_field_name("name"))
            if name == "stop_after_pending" and (not any(method_on_field(call, "take", "commands") for call in walk(node))
                    or {"store", "clear"} & called_methods(node)):
                fail(node, "actuation_exit_cancels_accepted_native_fifo", keyboard_actuation_worker)
            if name == "run_with":
                stops = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "shutdown"]
                drops = [call for parts, call in calls(node) if resolved_call(parts) == ("drop",)
                    and any(text(argument) == "transport" for argument in call.child_by_field_name("arguments").named_children)]
                receipts = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "send"
                    and any(text(argument) == "Reply::Stopped" for argument in call.child_by_field_name("arguments").named_children)]
                if (not stops or not drops or not receipts
                        or any(not any(stop.end_byte < drop.start_byte and drop.end_byte < receipt.start_byte
                            for stop in stops for drop in drops) for receipt in receipts)):
                    fail(node, "actuation_stops_before_transport_cleanup", keyboard_actuation_worker)
        if node.type == "function_item" and owner == session_monitor:
            name = text(node.child_by_field_name("name"))
            methods = called_methods(node)
            if (name == "shutdown" and "shutdown_with" not in methods and not {"signal_stop", "take", "join"} <= methods
                    or name == "shutdown_with" and not {"signal_stop_with", "take", "join"} <= methods):
                fail(node, "session_observer_shutdown_does_not_join_its_owner", session_monitor)
            if (name == "signal_stop" and "signal_stop_with" not in methods and ("try_lock" not in methods or "lock" in methods)
                    or name == "signal_stop_with" and ("try_lock" not in methods or "lock" in methods)):
                fail(node, "session_observer_stop_waits_for_native_teardown_on_ui", session_monitor)
            if name == "drop" and "join" in methods:
                fail(node, "session_observer_stop_waits_for_native_teardown_on_ui", session_monitor)
            if name == "pump_until_stopped":
                loops = [child for child in walk(node) if child.type == "while_expression"
                    and "load" in called_methods(child.child_by_field_name("condition"))]
                bounded = [argument for child in walk(node) for argument in macro_arguments(child, "matches")
                    if argument.type == "try_expression" and argument.named_children
                    and argument.named_children[0].type == "call_expression"
                    and argument.named_children[0].child_by_field_name("function").text == b"wait_and_dispatch"
                    and [item.text for item in argument.named_children[0].child_by_field_name("arguments").named_children] == [b"STOP_RECHECK_MS"]]
                if not loops or not bounded:
                    fail(node, "session_observer_wait_has_no_bounded_stop_recheck", session_monitor)
        if node.type == "function_item" and owner == device_monitor:
            implementation = node.parent.parent if node.parent is not None and node.parent.parent is not None else None
            implementation_type = text(implementation.child_by_field_name("type")) if implementation is not None and implementation.type == "impl_item" else None
            name = text(node.child_by_field_name("name"))
            methods = called_methods(node)
            if implementation_type == "DeviceChangeMonitor":
                owner_functions = {text(item.child_by_field_name("name")): item for declaration in declarations
                    if declaration.type == "impl_item" and text(declaration.child_by_field_name("type")) == "DeviceChangeMonitor"
                    for item in declaration.child_by_field_name("body").named_children
                    if item.type == "function_item" and not in_test(item)}
                def blocks_observer(function, seen=frozenset()):
                    if function.id in seen:
                        return False
                    seen = seen | {function.id}
                    return bool({"join", "lock", "recv"} & called_methods(function)) or any(
                        call.type == "call_expression" and self_field(call.child_by_field_name("function"), method)
                        and method in owner_functions and blocks_observer(owner_functions[method], seen)
                        for method in called_methods(function) for call in walk(function))
                if name == "start_worker":
                    retained = [field for child in walk(node) if child.type == "struct_expression"
                        and text(child.child_by_field_name("name")) == "Self"
                        for field in child.child_by_field_name("body").named_children if field.type == "field_initializer"
                        and text(field.child_by_field_name("field")) == "worker"
                        and any(resolved_call(parts) == ("Some",) for parts, _ in calls(field))]
                    stops = [call for parts, call in calls(node) if resolved_call(parts)[-2:] == ("mpsc", "sync_channel")
                        and [text(argument) for argument in call.child_by_field_name("arguments").named_children] == ["1"]]
                    if not retained or not stops or "spawn" not in methods or "detach" in methods:
                        fail(node, "device_observer_discards_worker_handle", device_monitor + "::DeviceChangeMonitor")
                if name in {"shutdown", "try_shutdown"}:
                    body = node.child_by_field_name("body")
                    stops = [call for call in walk(node) if call.type == "call_expression"
                        and self_field(call.child_by_field_name("function"), "signal_stop")
                        and straight_statement(call, body) is not None]
                    retained = observer_worker_joins(node)
                    delegates = [call for call in walk(node) if call.type == "call_expression"
                        and call.child_by_field_name("function").type == "field_expression"
                        and self_field(call.child_by_field_name("function"), text(call.child_by_field_name("function").child_by_field_name("field")))
                        and straight_statement(call, body) is not None
                        and (helper := owner_functions.get(text(call.child_by_field_name("function").child_by_field_name("field")))) is not None
                        and helper != node and observer_worker_joins(helper)]
                    operations = retained or delegates
                    unjoined = [call for call in walk(node) if method_on_field(call, "take", "worker") and call not in retained]
                    if (not stops or not operations or unjoined
                            or any(not any(stop.end_byte < operation.start_byte for stop in stops) for operation in operations)
                            or any(child.type in {"return_expression", "try_expression"}
                                and straight_statement(child, body) is not None and child.end_byte < operation.start_byte
                                for operation in operations for child in walk(body))):
                        fail(node, "device_observer_shutdown_does_not_stop_then_join", device_monitor + "::DeviceChangeMonitor")
                    if name == "try_shutdown":
                        guards = [branch for branch in body.named_children if branch.type == "expression_statement"
                            for branch in branch.named_children if branch.type == "if_expression"
                            and expression_shape(branch.child_by_field_name("condition")) == parsed_expression_shape("!self.is_finished()")
                            and returning_branch(branch)
                            and any(resolved_call(parts) == ("Ok",)
                                and [text(argument) for argument in call.child_by_field_name("arguments").named_children] == ["false"]
                                for parts, call in calls(branch.child_by_field_name("consequence")))]
                        finished = owner_functions.get("is_finished")
                        read_only = finished is not None and any(method_on_field(call, "as_ref", "worker") for call in walk(finished)) \
                            and any(resolved_call(text(child).split("::"))[-2:] == ("JoinHandle", "is_finished")
                                    for child in walk(finished) if child.type == "scoped_identifier") \
                            and not any(method_on_field(call, "take", "worker") for call in walk(finished))
                        if (not guards or not read_only or any(not any(
                                stop.end_byte < guard.start_byte and guard.end_byte < operation.start_byte
                                for stop in stops for guard in guards) for operation in operations)):
                            fail(node, "device_observer_try_shutdown_discards_pending_worker", device_monitor + "::DeviceChangeMonitor")
                if name == "signal_stop":
                    takes = [call for call in walk(node) if method_on_field(call, "take", "stop")]
                    sends = [call for call in walk(node) if call.type == "call_expression"
                        and call.child_by_field_name("function").type == "field_expression"
                        and text(call.child_by_field_name("function").child_by_field_name("field")) == "try_send"
                        and text(call.child_by_field_name("function").child_by_field_name("value")) == "stop"]
                    if not takes or not sends or {"send", "send_blocking", "recv", "join", "force_send"} & methods:
                        fail(node, "device_observer_stop_uses_delivery_queue_or_blocks", device_monitor + "::DeviceChangeMonitor::signal_stop")
                if name == "drop":
                    if not any(call.type == "call_expression" and self_field(call.child_by_field_name("function"), "signal_stop")
                               and straight_statement(call, node.child_by_field_name("body")) is not None for call in walk(node)):
                        fail(node, "device_observer_drop_does_not_signal_stop", device_monitor + "::DeviceChangeMonitor")
                    if blocks_observer(node):
                        fail(node, "device_observer_drop_waits_for_native_teardown_on_ui", device_monitor + "::DeviceChangeMonitor")
            if implementation_type == "EventDelivery" and name == "drop" and "close" not in methods:
                fail(node, "device_observer_delivery_outlives_its_worker", device_monitor + "::EventDelivery")
            if name == "wait_for_stop":
                ready = [call for call in walk(node) if call.type == "call_expression"
                    and text(call.child_by_field_name("function")) == "events.ready"]
                waits = [call for call in walk(node) if call.type == "call_expression"
                    and text(call.child_by_field_name("function")) == "stopped.recv"]
                if (not ready or not waits or {"send", "send_blocking", "force_send"} & methods
                        or any(read.end_byte > wait.start_byte for read in ready for wait in waits)):
                    fail(node, "device_observer_stop_waits_for_ui_to_drain_queue", device_monitor + "::wait_for_stop")
        if node.type == "function_item" and owner == thx_lifecycle:
            name = text(node.child_by_field_name("name"))
            if name == "retire_thx_owner":
                clones = {text(child.child_by_field_name("pattern")) for child in walk(node) if child.type == "let_declaration"
                    and child.child_by_field_name("value") is not None
                    and child.child_by_field_name("value").type == "call_expression"
                    and text(child.child_by_field_name("value").child_by_field_name("function")) == "owner.clone"}
                jobs = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "spawn"
                    and any(parts[-1] == "release_owner" and call.child_by_field_name("arguments").named_children
                        and text(call.child_by_field_name("arguments").named_children[0]) in clones
                        for parts, call in calls(call))]
                errors = [arm.child_by_field_name("value") for arm in walk(node) if arm.type == "match_arm"
                    and tuple_pattern(arm.child_by_field_name("pattern"), "Err")]
                retained = [call for branch in errors for call in walk(branch) if call.type == "call_expression"
                    and text(call.child_by_field_name("function")) == "self.thx_retired_owners.push"
                    and any(child.type == "struct_expression" and text(child.child_by_field_name("name")) == "RetiredOwner"
                        and {text(field.child_by_field_name("field")): text(field.child_by_field_name("value"))
                            if field.type == "field_initializer" else text(field)
                            for field in child.child_by_field_name("body").named_children
                            if field.type == "field_initializer"} | {text(field): text(field)
                            for field in child.child_by_field_name("body").named_children
                            if field.type == "shorthand_field_initializer"} == {"owner": "owner", "source_close": "source_close"}
                        for child in walk(call))]
                if not clones or not jobs or not errors or not retained or not any(method_on_field(call, "push", "thx_cleanup") for call in walk(node)):
                    fail(node, "thx_release_spawn_failure_discards_its_owner", thx_lifecycle + "::RetiredOwner")
            if name == "install_thx_cleanup":
                takes = [call for parts, call in calls(node) if resolved_call(parts) == ("std", "mem", "take")
                    and any("".join(text(argument).split()) == "&mutthis.thx_retired_owners"
                        for argument in call.child_by_field_name("arguments").named_children)]
                release = [call for parts, call in calls(node) if parts[-1] == "release_owner"
                    and [text(argument) for argument in call.child_by_field_name("arguments").named_children][:2] == ["retired.owner", "retired.source_close"]]
                background = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "spawn"
                    and "background_executor" in called_methods(call.child_by_field_name("function"))]
                if (not takes or not release or "register_exit_cleanup" not in called_methods(node)
                        or any(not any(parent.start_byte < call.start_byte and call.end_byte < parent.end_byte for parent in background) for call in release)):
                    fail(node, "thx_retired_owners_not_released_by_background_exit", thx_lifecycle + "::install_thx_cleanup")
            if name == "submit_thx_job":
                errors = [arm.child_by_field_name("value") for arm in walk(node) if arm.type == "match_arm"
                    and tuple_pattern(arm.child_by_field_name("pattern"), "Err")]
                restored = [call for branch in errors for call in walk(branch) if method_on_field(call, "extend", "thx_retired_owners")
                    and any(resolved_call(parts) == ("std", "mem", "take") for parts, _ in calls(call))]
                releases = [call for parts, call in calls(node) if parts[-1] == "release_owner"
                    and [text(argument) for argument in call.child_by_field_name("arguments").named_children][:2] == ["retired.owner", "retired.source_close"]]
                if not restored or not releases:
                    fail(node, "thx_successor_discards_failed_release_backlog", thx_lifecycle + "::submit_thx_job")
        if node.type == "function_item" and owner == ipc_client:
            implementation = node.parent.parent if node.parent is not None and node.parent.parent is not None else None
            implementation_type = text(implementation.child_by_field_name("type")) if implementation is not None and implementation.type == "impl_item" else None
            name = text(node.child_by_field_name("name"))
            if name in legacy_ipc_functions and not in_test(node):
                fail(node, "ipc_legacy_process_path_is_available_in_production", ipc_bootstrap + "::BrokerBootstrap")
            if implementation_type == "ServiceClient" and name == "spawn_with_program":
                connections = [call for parts, call in calls(node)
                               if resolved_call(parts)[-2:] == ("BrokerClient", "connect")]
                factories = [call for parts, call in calls(node) if resolved_call(parts) == ("Self", "from_broker")]
                forbidden = {"spawn", "assign", "kill", "terminate", "connect_or_start", "start"} & called_methods(node)
                static_forbidden = any(resolved_call(parts)[-2:] in {
                    ("BrokerBootstrap", "start"), ("WorkerJob", "new"), ("Command", "new")}
                    or resolved_call(parts)[-1] == "connect_or_start" for parts, _ in calls(node))
                if not connections or not factories or forbidden or static_forbidden:
                    fail(node, "ipc_client_bootstraps_or_bypasses_shared_broker", ipc_bootstrap + "::BrokerBootstrap")
            if implementation_type == "ServiceClient" and name in {"stop", "finish_process"}:
                delegates = [branch for branch in walk(node) if branch.type == "if_expression"
                    and any(method_on_field(call, "is_some", "broker") for call in walk(branch.child_by_field_name("condition")))
                    and any(child.type == "return_expression" and "close_broker_session" in called_methods(child)
                            for child in walk(branch.child_by_field_name("consequence")))]
                if not delegates or {"kill", "terminate", "assign"} & called_methods(node):
                    fail(node, "ipc_stop_skips_shared_session_cleanup", ipc_client + "::ServiceClient::close_broker_session")
            if implementation_type == "ServiceClient" and name == "close_broker_session":
                methods = called_methods(node)
                closed = [child for child in walk(node) if child.type == "assignment_expression"
                    and self_field(child.child_by_field_name("left"), "stopped") and text(child.child_by_field_name("right")) == "true"]
                receipts = [call for parts, call in calls(node) if resolved_call(parts) == ("worker_shutdown", "confirm_session_close")
                            and call.parent.type == "try_expression"]
                closes = [call for call in walk(node) if call.type == "call_expression"
                          and call.child_by_field_name("function").type == "field_expression"
                          and text(call.child_by_field_name("function").child_by_field_name("field")) == "close_session"]
                releases = [call for call in walk(node) if method_on_field(call, "take", "broker")]
                confirmations = [child for child in walk(node) if child.type == "assignment_expression"
                    and self_field(child.child_by_field_name("left"), "shutdown_confirmation")
                    and any(resolved_call(parts) == ("Some",) for parts, _ in calls(child.child_by_field_name("right")))]
                successful = [arm for arm in walk(node) if arm.type == "match_arm" and tuple_pattern(arm.child_by_field_name("pattern"), "Ok")]
                body = node.child_by_field_name("body")
                declarations = [child for child in body.named_children if child.type == "let_declaration"]
                cancellation_names = {text(child.child_by_field_name("pattern")) for child in declarations
                    if child.child_by_field_name("value") is not None
                    and expression_shape(child.child_by_field_name("value")) == parsed_expression_shape("self.cancel_request()")}
                def preserves_close_failure(match):
                    arms = [arm for arm in match.child_by_field_name("body").named_children if arm.type == "match_arm"]
                    errors = [arm for arm in arms if tuple_pattern(arm.child_by_field_name("pattern"), "Err")]
                    return bool(errors) and all(any(expression_shape(arm.child_by_field_name("value")) == parsed_expression_shape(
                        "Err(with_cleanup_error(" + text(pattern.named_children[-1]) + "," + cancellation + "))")
                        for cancellation in cancellation_names for pattern in walk(arm.child_by_field_name("pattern"))
                        if pattern.type == "tuple_struct_pattern" and text(pattern.child_by_field_name("type")) == "Err"
                        and pattern.named_children[-1].type == "identifier") for arm in errors)
                def confirms_close_success(match):
                    return any(any(expression_shape(call.child_by_field_name("arguments").named_children[0])
                            == parsed_expression_shape("&" + text(pattern.named_children[-1]))
                            for call in receipts if arm.start_byte < call.start_byte < call.end_byte < arm.end_byte
                            and call.child_by_field_name("arguments").named_children)
                        for arm in match.child_by_field_name("body").named_children if arm.type == "match_arm"
                        for pattern in walk(arm.child_by_field_name("pattern")) if pattern.type == "tuple_struct_pattern"
                        and text(pattern.child_by_field_name("type")) == "Ok" and pattern.named_children[-1].type == "identifier")
                broker_bindings = []
                for declaration in declarations:
                    value = declaration.child_by_field_name("value")
                    if (value is not None and value.type == "try_expression"
                            and declaration.child_by_field_name("pattern").type == "identifier"):
                        value = value.named_children[0]
                        if (value.type == "call_expression" and value.child_by_field_name("function").type == "field_expression"
                                and text(value.child_by_field_name("function").child_by_field_name("field")) == "context"):
                            value = value.child_by_field_name("function").child_by_field_name("value")
                        if method_on_field(value, "as_mut", "broker"):
                            broker_bindings.append(declaration)
                terminal_releases = set()
                for branch in walk(node):
                    if branch.type != "if_expression" or straight_statement(branch, body) is None:
                        continue
                    condition = branch.child_by_field_name("condition")
                    if condition.type != "let_condition":
                        continue
                    pattern = condition.child_by_field_name("pattern")
                    if (pattern.type != "tuple_struct_pattern" or text(pattern.child_by_field_name("type")) != "Some"
                            or pattern.named_children[-1].type != "identifier"):
                        continue
                    receipt_name = text(pattern.named_children[-1])
                    for broker in broker_bindings:
                        broker_name = text(broker.child_by_field_name("pattern"))
                        if expression_shape(condition.child_by_field_name("value")) != parsed_expression_shape(broker_name + ".close_receipt().cloned()"):
                            continue
                        close_bindings = [child for child in declarations if broker.end_byte < child.start_byte < branch.start_byte
                            and child.child_by_field_name("value") is not None
                            and expression_shape(child.child_by_field_name("value")) == parsed_expression_shape(broker_name + ".close_session()")]
                        matches = [child for child in walk(node) if child.type == "match_expression" and branch.end_byte < child.start_byte
                            and straight_statement(child, body) is not None
                            and any(text(child.child_by_field_name("value")) == text(close.child_by_field_name("pattern")) for close in close_bindings)]
                        cached = [child for child in walk(branch.child_by_field_name("consequence")) if child.type == "assignment_expression"
                            and self_field(child.child_by_field_name("left"), "broker_close_receipt")
                            and expression_shape(child.child_by_field_name("right")) == parsed_expression_shape("Some(" + receipt_name + ")")
                            and straight_statement(child, branch.child_by_field_name("consequence")) is not None]
                        if (matches and all(preserves_close_failure(match) and confirms_close_success(match) for match in matches)
                                and len([child for child in branch.child_by_field_name("consequence").named_children
                                    if child.type not in {"line_comment", "block_comment"}]) == 2):
                            terminal_releases.update(release.id for release in releases
                                if straight_statement(release, branch.child_by_field_name("consequence")) is not None
                                and any(cache.end_byte < release.start_byte for cache in cached))
                cached_failure_preserved = any(child.type == "match_expression" and preserves_close_failure(child)
                    and any(expression_shape(child.child_by_field_name("value")) == parsed_expression_shape(receipt_name + ".validate()")
                        for branch in ancestors(child) if branch.type == "if_expression"
                        and straight_statement(branch, body) is not None
                        and all(branch.end_byte < broker.start_byte for broker in broker_bindings)
                        for condition in [branch.child_by_field_name("condition")] if condition.type == "let_condition"
                        and expression_shape(condition.child_by_field_name("value")) == parsed_expression_shape("&self.broker_close_receipt")
                        for pattern in [condition.child_by_field_name("pattern")]
                        if pattern.type == "tuple_struct_pattern" and text(pattern.child_by_field_name("type")) == "Some"
                        for receipt_name in [text(pattern.named_children[-1])]
                        if any(parent.type == "return_expression" and straight_statement(parent, branch.child_by_field_name("consequence")) is not None
                            for parent in ancestors(child))) for child in walk(node))
                if (not closed or not receipts or not closes or not releases or not confirmations
                        or not {"cancel_request"} <= methods or {"kill", "terminate", "assign"} & methods
                        or any(close.end_byte > operation.start_byte for close in closed for operation in closes)
                        or any(release.id not in terminal_releases and not any(arm.start_byte <= release.start_byte < release.end_byte <= arm.end_byte
                               and any(arm.start_byte <= receipt.start_byte and receipt.end_byte < release.start_byte for receipt in receipts)
                               for arm in successful) for release in releases)
                        or (terminal_releases and not cached_failure_preserved)
                        or any(not any(receipt.end_byte < confirmation.start_byte for receipt in receipts) for confirmation in confirmations)):
                    fail(node, "ipc_stop_releases_session_without_confirmed_cleanup", ipc_client + "::ServiceClient::close_broker_session")
            if implementation_type == "ServiceClient" and name == "stop_simulated":
                drains = [call for parts, call in calls(node) if resolved_call(parts) == ("worker_shutdown", "drain")]
                releases = [call for call in walk(node) if call.type == "call_expression"
                    and self_field(call.child_by_field_name("function"), "finish_process")]
                closed = [child for child in walk(node) if child.type == "assignment_expression"
                    and self_field(child.child_by_field_name("left"), "stopped") and text(child.child_by_field_name("right")) == "true"]
                merged = [call for parts, call in calls(node) if resolved_call(parts) == ("combine_cleanup",)]
                if (not drains or not releases or not closed or not merged
                        or any(method_on_field(call, "take", "job") for call in walk(node))
                        or any(close.end_byte > drain.start_byte for close in closed for drain in drains)
                        or any(drain.end_byte > release.start_byte for drain in drains for release in releases)
                        or any(release.end_byte > merge.start_byte for release in releases for merge in merged)):
                    fail(node, "ipc_stop_skips_graceful_owner_cleanup", ipc_client + "::ServiceClient::stop")
            if implementation_type == "ServiceClient" and name == "finish_simulated_process":
                admission = [call for call in walk(node) if method_on_field(call, "take", "requests")]
                stopped = [child for child in walk(node) if child.type == "assignment_expression"
                    and self_field(child.child_by_field_name("left"), "stopped") and text(child.child_by_field_name("right")) == "true"]
                def propagates_reap_failure(call):
                    if call.parent.type == "try_expression":
                        return True
                    condition = call.parent
                    branch = condition.parent if condition.type == "let_condition" else None
                    if (branch is None or branch.type != "if_expression"
                            or branch.child_by_field_name("condition") != condition
                            or branch.child_by_field_name("alternative") is not None):
                        return False
                    container = branch.parent
                    if container.type == "expression_statement":
                        container = container.parent
                    if container != node.child_by_field_name("body"):
                        return False
                    pattern = condition.child_by_field_name("pattern")
                    if (pattern.type != "tuple_struct_pattern" or text(pattern.child_by_field_name("type")) != "Err"
                            or len(pattern.named_children) != 2 or pattern.named_children[1].type != "identifier"):
                        return False
                    error = text(pattern.named_children[1])
                    statements = [child for child in branch.child_by_field_name("consequence").named_children
                                  if child.type not in {"line_comment", "block_comment"}]
                    if len(statements) != 1:
                        return False
                    returned = statements[0]
                    if returned.type == "expression_statement" and len(returned.named_children) == 1:
                        returned = returned.named_children[0]
                    if returned.type != "return_expression" or len(returned.named_children) != 1:
                        return False
                    result = returned.named_children[0]
                    if result.type != "call_expression" or resolved_call(text(result.child_by_field_name("function")).split("::")) != ("Err",):
                        return False
                    arguments = result.child_by_field_name("arguments").named_children
                    if len(arguments) != 1 or arguments[0].type != "call_expression":
                        return False
                    merge = arguments[0]
                    merged_arguments = merge.child_by_field_name("arguments").named_children
                    if (resolved_call(text(merge.child_by_field_name("function")).split("::")) != ("with_cleanup_error",)
                            or len(merged_arguments) != 2 or text(merged_arguments[0]) != error
                            or merged_arguments[1].type != "identifier"):
                        return False
                    cancellation = text(merged_arguments[1])
                    retained = any(child.type == "let_declaration" and child.end_byte < call.start_byte
                        and text(child.child_by_field_name("pattern")) == cancellation
                        and child.child_by_field_name("value") is not None
                        and child.child_by_field_name("value").type == "call_expression"
                        and self_field(child.child_by_field_name("value").child_by_field_name("function"), "cancel_request")
                        for child in node.child_by_field_name("body").named_children)
                    return retained and any(cancellation in {text(child) for child in walk(merge.child_by_field_name("arguments"))
                        if child.type == "identifier"} for parts, merge in calls(node) if resolved_call(parts) == ("combine_cleanup",))
                reaped = [call for parts, call in calls(node) if resolved_call(parts) == ("reap_child",)
                    and propagates_reap_failure(call)
                    and any("".join(text(argument).split()) == "&mutself.child"
                        for argument in call.child_by_field_name("arguments").named_children)]
                joins = {field: [call for parts, call in calls(node) if resolved_call(parts) == ("join_pipe",)
                    and call.parent.type != "try_expression"
                    and any(method_on_field(child, "take", field) for child in walk(call))]
                    for field in ipc_pipe_workers}
                merged = [call for parts, call in calls(node) if resolved_call(parts) == ("combine_cleanup",)]
                joined_results = [join.parent.child_by_field_name("pattern")
                    if join.parent.type == "let_declaration" else None
                    for values in joins.values() for join in values]
                if (not admission or not stopped or not reaped or any(not value for value in joins.values()) or not merged
                        or any(method_on_field(call, "take", "child") for call in walk(node))
                        or any(result is None or result.type != "identifier"
                            or not any(text(result) in {text(child) for child in walk(merge.child_by_field_name("arguments"))
                                if child.type == "identifier"} for merge in merged) for result in joined_results)
                        or any(start.end_byte > reap.start_byte for start in admission + stopped for reap in reaped)
                        or any(not any(reap.end_byte < join.start_byte for reap in reaped)
                            for values in joins.values() for join in values)
                        or any(not any(join.end_byte < merge.start_byte for merge in merged) for values in joins.values() for join in values)):
                    fail(node, "ipc_cleanup_does_not_preserve_owner_until_reap_and_pipe_joins", ipc_client + "::ServiceClient")
            if implementation_type == "ServiceClient" and name == "drop":
                branches = [child for child in walk(node) if child.type == "if_expression"
                    and any(self_field(part, "stopped") for part in walk(child.child_by_field_name("condition")))]
                if "stop" not in called_methods(node) or branches:
                    fail(node, "ipc_drop_skips_unfinished_cleanup_after_admission_closed", ipc_client + "::ServiceClient")
            if name == "reap_child":
                waits = [call for parts, call in calls(node) if resolved_call(parts) == ("terminate_child",)
                    and call.parent.type == "try_expression"]
                takes = [call for call in walk(node) if call.type == "call_expression"
                    and text(call.child_by_field_name("function")) == "child.take"]
                if not waits or not takes or any(not any(wait.end_byte < take.start_byte for wait in waits) for take in takes):
                    fail(node, "ipc_reap_discards_unconfirmed_process_owner", ipc_client + "::reap_child")
            if name == "join_pipe" and "join" not in called_methods(node):
                fail(node, "ipc_pipe_completion_does_not_join_its_retained_worker", ipc_client + "::join_pipe")
        if node.type == "function_item" and owner == broker_service and text(node.child_by_field_name("name")) == "run_worker":
            methods = called_methods(node)
            connections = any(resolved_call(parts)[-2:] == ("BrokerClient", "connect") for parts, _ in calls(node))
            native = any(resolved_call(parts)[-2:] in {("NativeBackend", "new"), ("NativeRuntime", "new"), ("SessionRuntime", "new")}
                         or "EngineLibrary" in resolved_call(parts) for parts, _ in calls(node))
            if not connections or not {"request", "close_session", "validate"} <= methods or native:
                fail(node, "legacy_stdio_worker_bypasses_shared_broker", ipc_broker_client + "::BrokerClient")
        if node.type == "function_item" and owner == ipc_broker_client:
            name = text(node.child_by_field_name("name"))
            if name in {"connect", "request", "close_session", "drop"}:
                forbidden = {"spawn", "kill", "terminate", "assign"} & called_methods(node)
                if forbidden or any(resolved_call(parts)[-2:] in {("Command", "new"), ("WorkerJob", "new"), ("BrokerBootstrap", "start")}
                                    for parts, _ in calls(node)):
                    fail(node, "shared_broker_session_controls_process_lifetime", ipc_bootstrap + "::BrokerBootstrap")
            if name == "close_session" and not {"validate", "read"} <= called_methods(node):
                fail(node, "shared_broker_close_has_no_real_cleanup_receipt", ipc_broker_client + "::BrokerClient::close_session")
        if node.type == "function_item" and owner == ipc_bootstrap:
            name = text(node.child_by_field_name("name"))
            if name in {"start", "finish", "drop"} and ({"kill", "terminate", "assign"} & called_methods(node)
                    or any(resolved_call(parts)[-2:] == ("WorkerJob", "new") for parts, _ in calls(node))):
                fail(node, "shared_broker_process_is_owned_by_a_client_kill_job", ipc_bootstrap + "::BrokerBootstrap")
        if node.type == "function_item" and owner == broker_server:
            name = text(node.child_by_field_name("name"))
            if name == "run_listener":
                prepares = [call for parts, call in calls(node) if resolved_call(parts) == ("prepare_listener_shutdown",)]
                releases = [call for parts, call in calls(node) if resolved_call(parts) == ("drop",)
                    and [text(argument) for argument in call.child_by_field_name("arguments").named_children] == ["listener"]]
                admissions = [branch for branch in list(walk(node)) + select_arm_nodes(node) if branch.type == "if_expression"
                    and branch.child_by_field_name("condition").type == "unary_expression"
                    and text(branch.child_by_field_name("condition")).lstrip().startswith("!")
                    and any(resolved_call(parts) == ("session_available",) for parts, _ in calls(branch.child_by_field_name("condition")))
                    and any(child.type == "continue_expression" for child in walk(branch.child_by_field_name("consequence")))]
                if not prepares or not releases or any(not any(prepare.end_byte < release.start_byte for prepare in prepares) for release in releases):
                    fail(node, "shared_broker_listener_released_before_native_owners_drained", broker_server + "::prepare_listener_shutdown")
                if not admissions:
                    fail(node, "shared_broker_session_admission_is_unbounded", broker_server + "::session_available")
            if name == "prepare_listener_shutdown" and not any(resolved_call(parts)[-1] == "prepare_shutdown" for parts, _ in calls(node)):
                fail(node, "shared_broker_native_shutdown_is_not_drained", broker_service + "::hid_broker::prepare_shutdown")
        if node.type == "function_item" and owner == keyboard_lifecycle:
            name = text(node.child_by_field_name("name"))
            if name == "retire" and not any(resolved_call(parts) == ("drop",)
                    and any(text(argument) == "pump" for argument in call.child_by_field_name("arguments").named_children)
                    for parts, call in calls(node)):
                fail(node, "session_observer_retirement_keeps_its_event_pump", keyboard_lifecycle)
            if name == "install_keyboard_lifecycle":
                cleanups = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "register_exit_cleanup"]
                if (not cleanups or not any({"spawn", "shutdown"} <= called_methods(cleanup)
                        and any(child.type == "await_expression" for child in walk(cleanup)) for cleanup in cleanups)):
                    fail(node, "session_observer_is_not_drained_by_application_exit", shutdown_owner)
        if node.type == "function_item" and owner == shutdown_owner:
            name = text(node.child_by_field_name("name"))
            if name == "register_exit_cleanup":
                pushes = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "push"
                    and "".join(text(call.child_by_field_name("function").child_by_field_name("value")).split()) == "self.service_exit.hooks"]
                invokes = [call for parts, call in calls(node) if parts == ["cleanup"]
                    and [text(argument) for argument in call.child_by_field_name("arguments").named_children] == ["shell", "cx"]]
                if not pushes or not invokes or not any(push.start_byte < call.start_byte and call.end_byte < push.end_byte
                        for push in pushes for call in invokes):
                    fail(node, "exit_cleanup_registration_bypasses_shared_owner", shutdown_owner)
            if name == "take_exit_cleanups":
                takes = [call for parts, call in calls(node) if resolved_call(parts) == ("std", "mem", "take")
                    and any("".join(text(argument).split()) == "&mutself.service_exit.hooks"
                        for argument in call.child_by_field_name("arguments").named_children)]
                invocations = [call for parts, call in calls(node) if parts == ["cleanup"]
                    and [text(argument) for argument in call.child_by_field_name("arguments").named_children] == ["self", "cx"]]
                bindings = {text(declaration.child_by_field_name("pattern")).removeprefix("mut ")
                    for declaration in walk(node) if declaration.type == "let_declaration"
                    and any(declaration.start_byte <= take.start_byte and take.end_byte <= declaration.end_byte
                        for take in takes)}
                restores = [child for child in walk(node) if child.type == "assignment_expression"
                    and "".join(text(child.child_by_field_name("left")).split()) == "self.service_exit.hooks"
                    and text(child.child_by_field_name("right")) in bindings]
                iterators = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "iter_mut"
                    and text(call.child_by_field_name("function").child_by_field_name("value")) in bindings]
                if (not takes or not invocations or "collect" not in called_methods(node)
                        or not iterators or not restores
                        or any(not any(call.end_byte < restore.start_byte for call in invocations) for restore in restores)
                        or {"filter", "filter_map", "take", "skip"} & called_methods(node)):
                    fail(node, "exit_cleanup_hooks_have_no_single_retained_owner", shutdown_owner)
            if name == "finish_service_exit":
                branches = [branch for branch in walk(node) if branch.type == "if_expression"]
                task_waits = [branch for branch in branches if returning_branch(branch)
                    and "".join(text(branch.child_by_field_name("condition")).split()) == "self.service_exit.task.is_some()"]
                takes = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "take_exit_cleanups"]
                retained = [child for child in walk(node) if child.type == "assignment_expression"
                    and "".join(text(child.child_by_field_name("left")).split()) == "self.service_exit.task"
                    and "spawn" in called_methods(child.child_by_field_name("right"))]
                completions = {text(declaration.child_by_field_name("pattern")) for take in takes for declaration in walk(node)
                    if declaration.type == "let_declaration"
                    and declaration.start_byte <= take.start_byte and take.end_byte <= declaration.end_byte}
                loops = [child for child in walk(node) if child.type == "for_expression"
                    and text(child.child_by_field_name("value")) in completions
                    and not any(statement.type in {"break_expression", "return_expression"}
                        for statement in walk(child.child_by_field_name("body")))]
                waits = [child for child in walk(node) if child.type == "await_expression"
                    and any(assignment.start_byte < child.start_byte and child.end_byte < assignment.end_byte
                        for assignment in retained)
                    and any(loop.start_byte < child.start_byte and child.end_byte < loop.end_byte
                        and len(child.named_children) == 1
                        and text(child.named_children[0]) == text(loop.child_by_field_name("pattern")) for loop in loops)]
                resets = [child for child in walk(node) if child.type == "assignment_expression"
                    and "".join(text(child.child_by_field_name("left")).split()) == "shell.service_exit.task"
                    and text(child.child_by_field_name("right")) == "None"]
                resumes = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "continue_save_queue"]
                if (not task_waits or not takes or not retained or not waits or not resets or not resumes
                        or any(not any(preceding_return(branch, call) for branch in task_waits) for call in takes)
                        or any(not any(wait.end_byte < reset.start_byte for wait in waits) for reset in resets)
                        or any(not any(reset.end_byte < resume.start_byte for reset in resets)
                            or "shell.close_requested" not in enclosing_conditions(resume) for resume in resumes)
                        or {"detach", "quit"} & called_methods(node)):
                    fail(node, "service_cleanups_have_no_retained_completion_barrier", shutdown_owner)
        if node.type == "function_item" and owner == "crates/razer-shell/src/shell.rs" and text(node.child_by_field_name("name")) == "finish_resource_exit":
            take_calls = [call for parts, call in calls(node)
                if resolved_call(parts) == ("razer_assets", "take_resource_runtime_for_shutdown")]
            branches = [branch for branch in walk(node) if branch.type == "if_expression"]
            task_waits = [branch for branch in branches if returning_branch(branch)
                and method_on_field(branch.child_by_field_name("condition"), "is_some", "resource_exit_task")]
            retained = [child for child in walk(node) if child.type == "assignment_expression"
                and self_field(child.child_by_field_name("left"), "resource_exit_task")
                and "spawn" in called_methods(child.child_by_field_name("right"))]
            bindings = {text(child) for take in take_calls for declaration in walk(node)
                if declaration.type == "let_declaration"
                and declaration.start_byte <= take.start_byte and take.end_byte <= declaration.end_byte
                for child in walk(declaration.child_by_field_name("pattern")) if child.type == "identifier"}
            waits = [child for child in walk(node) if child.type == "await_expression"
                and "background_spawn" in called_methods(child)
                and any(call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"
                    and text(call.child_by_field_name("function").child_by_field_name("field")) == "shutdown"
                    and text(call.child_by_field_name("function").child_by_field_name("value")) in bindings
                    for call in walk(child))]
            resets = [child for child in walk(node) if child.type == "assignment_expression"
                and child.child_by_field_name("left").type == "field_expression"
                and text(child.child_by_field_name("left").child_by_field_name("field")) == "resource_exit_task"
                and text(child.child_by_field_name("right")) == "None"]
            resumes = [call for call in walk(node) if call.type == "call_expression"
                and call.child_by_field_name("function").type == "field_expression"
                and text(call.child_by_field_name("function").child_by_field_name("field")) == "continue_save_queue"]
            if (not take_calls or not task_waits or not retained or not waits or not resets or not resumes
                    or any(not any(preceding_return(branch, take) for branch in task_waits) for take in take_calls)
                    or any(not any(assignment.start_byte < wait.start_byte and wait.end_byte < assignment.end_byte
                        for assignment in retained) for wait in waits)
                    or any(not any(wait.end_byte < reset.start_byte for wait in waits) for reset in resets)
                    or any(not any(reset.end_byte < resume.start_byte for reset in resets)
                        or "shell.close_requested" not in enclosing_conditions(resume) for resume in resumes)
                    or {"detach", "quit"} & called_methods(node)):
                fail(node, "resource_shutdown_has_no_retained_completion_barrier", resource_bridge)
        if node.type == "function_item" and owner == image_cache_owner:
            name = text(node.child_by_field_name("name"))
            implementation = next((parent for parent in ancestors(node) if parent.type == "impl_item"), None)
            cache_method = implementation is not None and text(implementation.child_by_field_name("type")) == "ResourceImageCache"
            if cache_method and name in {"load", "load_filtered"}:
                gates = [branch for branch in walk(node) if branch.type == "if_expression" and returning_branch(branch)
                    and any(resolved_call(parts) in {("ensure_decoders_running",), ("self", "ensure_decoders_running")}
                        for parts, _ in calls(branch.child_by_field_name("condition")))]
                entries = [child for child in walk(node) if child.type == "field_expression"
                    and text(child.child_by_field_name("value")) == "self"
                    and text(child.child_by_field_name("field")) in {"images", "filtered", "filter_clock"}]
                if not gates or not entries or any(not any(preceding_return(gate, entry) for gate in gates) for entry in entries):
                    fail(node, "image_decode_bypasses_application_stop", image_cache_owner + "::ensure_decoders_running")
            if cache_method and name == "clear":
                normal = [call for parts, call in calls(node) if resolved_call(parts)[-1] == "discard_image_entries"
                    and any("".join(text(argument).split()) == "&mutself.images"
                        for argument in call.child_by_field_name("arguments").named_children)]
                if not normal or not {"clear_filtered", "drop_image"} <= called_methods(node):
                    fail(node, "image_cache_cleanup_leaves_normal_or_filtered_loads", image_cache_owner + "::ResourceImageCache::clear")
            if name == "stop_resource_images":
                methods = [call for call in walk(node) if call.type == "call_expression"
                    and call.child_by_field_name("function").type == "field_expression"]
                stops = [call for call in methods if text(call.child_by_field_name("function").child_by_field_name("field")) == "stop"
                    and text(call.child_by_field_name("function").child_by_field_name("value")).endswith(".lifecycle")]
                clears = [call for call in methods if text(call.child_by_field_name("function").child_by_field_name("field")) == "clear"]
                installs = [call for parts, call in calls(node) if parts == ["WindowResourceCaches", "install"]]
                if not installs or not stops or not clears or any(not any(stop.end_byte < clear.start_byte for stop in stops) for clear in clears):
                    fail(node, "image_cache_stop_does_not_close_both_load_paths", image_cache_owner + "::stop_resource_images")
        if node.type == "function_item" and owner == local_image_owner and text(node.child_by_field_name("name")) == "preview_source":
            methods = [call for call in walk(node) if call.type == "call_expression"
                and call.child_by_field_name("function").type in {"field_expression", "generic_function"}]
            replacements = [call for call in methods if call.child_by_field_name("function").type == "field_expression"
                and text(call.child_by_field_name("function").child_by_field_name("field")) == "set_source"
                and text(call.child_by_field_name("function").child_by_field_name("value")) == "lease"]
            uses = []
            for call in methods:
                function = call.child_by_field_name("function")
                if function.type != "generic_function":
                    continue
                field = function.child_by_field_name("function")
                arguments = function.child_by_field_name("type_arguments")
                if (field.type == "field_expression" and text(field.child_by_field_name("field")) == "use_asset"
                        and text(field.child_by_field_name("value")) == "window"
                        and arguments is not None and len(arguments.named_children) == 1
                        and resolved_call(text(arguments.named_children[0]).split("::")) == ("ImportedImageAsset",)):
                    uses.append(call)
            if not replacements or not uses or any(not any(replacement.end_byte < use.start_byte for replacement in replacements) for use in uses):
                fail(node, "displayed_image_slot_keeps_the_previous_content_lease", local_image_owner + "::ImportedImageLease::set_source")
        if node.type == "function_item" and owner == resource_bridge and text(node.child_by_field_name("name")) == "take_resource_runtime_for_shutdown":
            methods = [call for call in walk(node) if call.type == "call_expression"
                and call.child_by_field_name("function").type == "field_expression"]
            takes = [call for call in methods
                if text(call.child_by_field_name("function").child_by_field_name("field")) == "take"
                and call.child_by_field_name("function").child_by_field_name("value").type == "field_expression"]
            owners = [call for call in takes
                if text(call.child_by_field_name("function").child_by_field_name("value").child_by_field_name("field")) == "owner"]
            results = [call for call in takes
                if text(call.child_by_field_name("function").child_by_field_name("value").child_by_field_name("field")) == "results"]
            stops = [call for call in methods
                if text(call.child_by_field_name("function").child_by_field_name("field")) == "begin_shutdown"]
            decoder_stops = [call for parts, call in calls(node)
                if resolved_call(parts) == ("crate", "image_cache", "stop_resource_images")]
            if (not owners or not results or not stops
                    or not decoder_stops or any(not any(stop.end_byte < take.start_byte for stop in decoder_stops) for take in owners)
                    or not any(owner_take.end_byte < stop.start_byte and stop.end_byte < result.start_byte
                        for owner_take in owners for stop in stops for result in results)
                    or {"shutdown", "join"} & called_methods(node)):
                fail(node, "resource_owner_transfer_keeps_admission_or_blocks_ui", resource_bridge)
        if node.type == "function_item" and (owner, text(node.child_by_field_name("name"))) in {
                ("crates/razer-shell/src/shell.rs", "save_auxiliary_preferences"),
                ("crates/razer-shell/src/shell/mouse/mouse_dpi_70.rs", "persist_legacy_profile")}:
            failures = [branch for branch in walk(node) if branch.type == "if_expression"
                and any(child.type == "field_expression" and text(child.child_by_field_name("field")) == "storage_error"
                    for child in walk(branch.child_by_field_name("condition")))]
            if (not failures or any("cancel_pending_exit" not in called_methods(branch.child_by_field_name("consequence"))
                    or not any(child.type == "return_expression" for child in walk(branch.child_by_field_name("consequence")))
                    for branch in failures)):
                kind = ("auxiliary_save_failure_keeps_exit_admission_closed" if text(node.child_by_field_name("name")) == "save_auxiliary_preferences"
                    else "legacy_profile_save_failure_keeps_exit_admission_closed")
                fail(node, kind, factory_owner + "::cancel_pending_exit")
        if node.type == "function_item" and owner == "crates/razer-shell/src/shell/devices/device_discovery.rs" and text(node.child_by_field_name("name")) in {"observe_discovery", "publish_receiver_query"}:
            method_calls = [call for call in walk(node) if call.type == "call_expression" and call.child_by_field_name("function").type == "field_expression"]
            release = next((call for call in method_calls if text(call.child_by_field_name("function").child_by_field_name("field")) == "retain_interdevice_connections"), None)
            resets = [call for call in method_calls if text(call.child_by_field_name("function").child_by_field_name("field")) == "observe_connection"]
            if release is None or any(call.start_byte < release.start_byte for call in resets):
                fail(node, "interdevice_connection_reset_before_release", interdevice_owner)
        if node.type != "call_expression":
            continue
        function = node.child_by_field_name("function")
        segments = text(function).split("::")
        canonical = resolved_call(segments)
        if canonical in {chroma_session + ("new",), chroma_session + ("default",),
                ("crate", "chroma_connect", "Session", "new"), ("crate", "chroma_connect", "Session", "default")} \
                and owner not in {chroma_shell, chroma_service}:
            fail(node, "duplicate_chroma_connect_session_owner", chroma_shell + "::AppShell")
        if owner == ipc_client:
            native_spawn = (canonical == ("std", "thread", "spawn") or function.type == "field_expression"
                and text(function.child_by_field_name("field")) == "spawn"
                and any(resolved_call(parts) == ("std", "thread", "Builder", "new")
                    for parts, _ in calls(function.child_by_field_name("value"))))
            if native_spawn:
                ancestor = node.parent
                while ancestor is not None and ancestor.type != "function_item":
                    if (ancestor.type == "assignment_expression" and ancestor.child_by_field_name("left").type == "field_expression"
                            and text(ancestor.child_by_field_name("left").child_by_field_name("field")) in ipc_pipe_workers):
                        break
                    ancestor = ancestor.parent
                if ancestor is None or ancestor.type == "function_item":
                    fail(node, "ipc_thread_has_no_retained_pipe_owner", ipc_client + "::ServiceClient")
        if owner == session_monitor and canonical[-1] == "GetMessageW":
            fail(node, "session_observer_wait_has_no_bounded_stop_recheck", session_monitor)
        if (owner == keyboard_actuation and function.type == "field_expression"
                and text(function.child_by_field_name("field")) == "detach"):
            fail(node, "actuation_delivery_escapes_retained_session_lifetime", keyboard_actuation)
        if (owner.startswith("crates/razer-shell/src/") and owner != shutdown_owner
                and function.type == "field_expression" and text(function.child_by_field_name("field")) == "on_app_quit"
                and (any(child.type == "await_expression" for child in walk(node))
                    or {"spawn", "background_spawn", "join", "shutdown", "shutdown_for_app_quit"} & called_methods(node))):
            fail(node, "background_cleanup_relies_on_bounded_framework_quit", shutdown_owner)
        if owner.startswith(macro_page) and ((canonical[:2] in {("std", "thread"), ("thread", "Builder")})
                or canonical[-2:] == ("ServiceClient", "spawn")):
            fail(node, "page_owns_macro_recording_worker", macro_recorder)
        if canonical[:2] == ("razer_storage", "ui_preferences") and canonical[-1] in {"load", "save"} and owner not in {preference_storage, preference_state}:
            fail(node, "ui_preference_io_bypasses_shared_state", preference_state)
        if preference_page and ((canonical[:2] in {("std", "fs"), ("tokio", "fs")})
                or canonical[:2] in {("razer_storage", "source_local"), ("crate", "source_local")}
                or (canonical[:2] == ("std", "env") and canonical[-1] in {"var", "var_os"}
                    and any(text(child) == '"APPDATA"' for child in node.child_by_field_name("arguments").named_children))):
            fail(node, "preference_page_owns_local_file_io", preference_state)
        if owner == macro_transfer and len(segments) >= 2 and (("fs" in segments) or segments[-2] == "Document"):
            fail(node, "page_owns_macro_xml_io_or_codec", macro_xml if segments[-2] == "Document" else macro_io)
        if owner == profile_codec and segments[0] == "md5":
            fail(node, "profile_hash_bypasses_shared_owner", hash_owner)
        if owner.startswith(mapping_pages) and len(segments) >= 2 and ((aliases.get(segments[-2]) == "ServiceClient" and segments[-1] == "spawn")
                or any(aliases.get(part) == "thread" for part in segments) or segments[-2] in thread_builders):
            fail(node, "page_owns_mapping_native_worker", mapping_workers)
        if owner.startswith("crates/razer-shell/src/") and segments[-1] == "active_workers" and any("mapping" in part for part in segments[:-1]) and segments[:-1] != ["razer_service", "mapping"]:
            fail(node, "shell_counts_mapping_workers_outside_service", mapping_workers + "mod.rs")
        if owner.startswith("crates/razer-shell/src/") and len(segments) >= 2 and segments[-1] == "new" and aliases.get(segments[-2]) == "WorkspaceFile" and owner != factory_owner:
            fail(node, "product_recomposes_workspace", factory_owner)
        if segments[-1] == "inspect_cached":
            fail(node, "blocking_cache_inspection_in_product", "razer_resources::ResourceClient")
        if function is not None and function.type == "field_expression" and text(function.child_by_field_name("field")) == "zip" and owner.startswith("crates/razer-shell/src/"):
            receiver = function.child_by_field_name("value")
            if any(child.type == "field_expression" and text(child.child_by_field_name("field")) == "devices" for child in walk(receiver)):
                fail(node, "positional_device_acknowledgement", factory_owner)
    return violations + gui_io_violations


def architecture_audit(inventory):
    import resource_scope
    rust_parser = resource_scope.parser(ROOT)
    violations = dependency_boundaries(inventory["packages"])
    sources = sorted({*ROOT.glob("app/**/*.rs"), *ROOT.glob("crates/**/*.rs")})
    included = set()
    for path in sources:
        raw = path.read_bytes()
        tree = rust_parser.parse(raw)
        pending = [tree.root_node]
        while pending:
            node = pending.pop()
            pending.extend(node.named_children)
            if node.type == "macro_invocation" and raw[node.child_by_field_name("macro").start_byte:node.child_by_field_name("macro").end_byte] == b"include":
                values = next(child for child in node.named_children if child.type == "token_tree").named_children
                if len(values) == 1 and values[0].type == "string_literal":
                    literal = raw[values[0].start_byte + 1:values[0].end_byte - 1].decode("utf-8")
                    if "\\" not in literal:
                        included.add((path.parent / literal).resolve())
    for path in sources:
        violations.extend(inspect_architecture_source(path.read_bytes(), owner=relative(path), rust_parser=rust_parser,
                                                      expression_fragment=path.resolve() in included))
    return {"packages_scanned": len(inventory["packages"]), "rust_files_scanned": len(sources),
            "violations": violations, "passed": not violations,
            "scope": "All workspace dependency branches and Rust item/call AST; compilation and runtime acceptance are separate",
            "limits": ["No Rust type resolution or macro expansion; dynamic include paths require an explicit owner exception",
                       "Literal include expression fragments are parsed inside a const expression with original byte offsets",
                       "Explicit owners do not prove page/device runtime behavior"]}


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
            "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__)), "python_version": sys.version.split()[0]},
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
        plan["architecture"] = architecture_audit(plan["workspace_inventory"])
        import resource_scope
        plan["resource_scope"] = resource_scope.audit_sources(ROOT)
    write_json(receipt, plan)
    print(json.dumps({"receipt": relative(receipt), "moves": len(plan["moves"]),
                      "rewrite_files": len(rewrites), "unresolved": plan["unresolved"],
                      "applied": plan["applied"],
                      "architecture_passed": plan.get("architecture", {}).get("passed"),
                      "resource_scope_passed": plan.get("resource_scope", {}).get("passed")}), flush=True)
    return plan


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
        report["source_publications"] = data.get("source_publications", [])
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


def rust_resource_literals(raw, *, owner="", test_only=False):
    """Read actual Rust literals and their complete owners, including macro tokens."""
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    import tree_sitter
    import tree_sitter_rust
    parser = tree_sitter.Parser(tree_sitter.Language(tree_sitter_rust.language()))
    tree = parser.parse(raw)
    offset = 0
    if tree.root_node.has_error:
        # Some include! owners are Rust expressions rather than source files.
        # Parse their complete bytes in a grammar wrapper and preserve offsets.
        prefix = b"const RESOURCE_AUDIT_EXPRESSION: () = "
        wrapped = parser.parse(prefix + raw + b";")
        if wrapped.root_node.has_error:
            raise ValueError("Rust resource owner has an AST error: " + owner)
        raw = prefix + raw + b";"
        tree, offset = wrapped, len(prefix)
    rows = []
    def attributes(node):
        result = []
        sibling = node.prev_named_sibling
        while sibling is not None and sibling.type in ("attribute_item", "line_comment", "block_comment"):
            if sibling.type == "attribute_item":
                result.append(raw[sibling.start_byte:sibling.end_byte].decode("utf-8"))
            sibling = sibling.prev_named_sibling
        return result
    def visit(node, context=(), fixture=test_only):
        if node.type in ("function_item", "mod_item", "impl_item"):
            fixture = fixture or any(re.search(r"\b(?:test|test_context)\b", a) for a in attributes(node))
            name = node.child_by_field_name("name")
            body = raw[node.start_byte:node.end_byte]
            context = (*context, {"kind": node.type,
                "name": raw[name.start_byte:name.end_byte].decode("utf-8") if name else "impl",
                "byte_start": node.start_byte - offset, "byte_end": node.end_byte - offset,
                "body_sha256": hashlib.sha256(body).hexdigest()})
        if node.type in ("string_literal", "raw_string_literal"):
            literal = raw[node.start_byte:node.end_byte].decode("utf-8")
            if node.type == "raw_string_literal":
                value = re.sub(r'^r#*"|"#*$', '', literal)
            else:
                # Resource identifiers are ASCII paths; reject escapes instead
                # of applying Python's different string escape semantics.
                value = literal[1:-1]
                if "\\" in value:
                    if value.startswith(("synapse/", "assets/")):
                        raise ValueError("Escaped runtime resource path requires explicit decoding: " + owner)
                    return
            media = re.search(r"\.(?:svg|png|jpe?g|webp|avif|gif|bmp|ico|mp4|woff2?|ttf)(?:#.*)?$", value)
            if value.startswith(("synapse/", "assets/")) and (media or "{" in value):
                rows.append({"owner": owner, "line": node.start_point.row + 1,
                    "byte_start": node.start_byte - offset, "byte_end": node.end_byte - offset,
                    "literal": value, "test_only": fixture, "context": list(context)})
            return
        for child in node.named_children:
            visit(child, context, fixture)
    visit(tree.root_node)
    return rows


def repair_current_resource_aliases():
    """Verify missing ARGB interaction layers against complete current originals."""
    import analysis
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    import json5
    import tree_sitter
    import tree_sitter_javascript
    parser = tree_sitter.Parser(tree_sitter.Language(tree_sitter_javascript.language()))
    lock = resource_sources.load_lock()
    files = resource_sources.rows_for(lock)
    headers = None
    repaired, inputs = [], []
    mappings = resource_sources.load_resource_mappings()
    table = resource_sources.resource_table("ASSETS", mappings=mappings)
    layer_names = ("refresh-hover", "warning-hover", "remove-hover", "power-hover", "power_off-hover",
                   "power-base", "power_off-base", "auto_on-ring", "auto_on-glyph",
                   "auto_off-glyph", "auto_off-green", "auto_off-gray")
    markers = {"refresh": 'd:"M15.649', "warning": 'd:"M9.156',
               "remove": 'className:"icon-close-glitter"', "power": 'd:"M11 7a1',
               "power_off": 'd:"M11 7a1', "auto_on": 'className:"icon-detection--active"',
               "auto_off": 'className:"icon-detection ".concat'}
    for pid in (3884, 3886):
        receipt_path = ROOT / f".analysis/inputs/frontend/accessories-{pid}-current/acquisition.json"
        receipt = json.loads(receipt_path.read_bytes())
        if receipt.get("source_current_during_acquisition") is not True:
            raise ValueError("ARGB layer repair requires stable current originals")
        originals = [row for row in receipt["files"] if row.get("http_status") == 200]
        selected = {}
        for extension in ("js", "css"):
            matches = [r for r in originals if re.search(r"/static/" + extension + r"/main\.[^/]+\." + extension + "$", r["url"])]
            if len(matches) != 1:
                raise ValueError("ARGB layer source main bundle is absent or ambiguous")
            row = matches[0]
            path = asset_pack_identity(row)
            selected[extension] = (row, path.read_bytes())
        js_row, raw = selected["js"]
        css_row, css_raw = selected["css"]
        text, css = raw.decode("utf-8"), css_raw.decode("utf-8")
        tree = parser.parse(raw)
        if tree.root_node.has_error:
            raise ValueError("ARGB current original has a JS AST error")
        css_rules = [(m[1], m[2], m[0]) for m in re.finditer(r"([^{}]+)\{([^{}]*)\}", css)]
        owners = {}
        for role, marker in markers.items():
            if text.count(marker) != 1:
                raise ValueError("ARGB current layer owner is ambiguous: " + role)
            begin = len(text[:text.index(marker)].encode("utf-8"))
            node = tree.root_node.descendant_for_byte_range(begin, begin + len(marker))
            while node is not None and node.type not in ("class_declaration", "variable_declarator"):
                node = node.parent
            if node is None:
                raise ValueError("ARGB current layer has no complete owner: " + role)
            body = raw[node.start_byte:node.end_byte].decode("utf-8")
            owners[role] = {"byte_start": node.start_byte, "byte_end": node.end_byte,
                "body": body, "body_sha256": hashlib.sha256(body.encode()).hexdigest()}
        input_proof = {"product_id": pid, "acquisition": {"path": relative(receipt_path),
            "sha256": digest(receipt_path)}, "js": js_row, "css": css_row,
            "complete_owners": owners,
            "complete_original_input": js_row,
            "css_rules": [body for selector, _, body in css_rules if any(name in selector
                for name in ("icon-detection", "icon-refreshing", "icon-power", "icon-warning", "icon-close-glitter"))]}
        inputs.append(input_proof)
        for name in layer_names:
            source_pid = pid if name.endswith("-base") else 3884
            suffix = f"-wireless-argb-{source_pid}-{name}.svg"
            known = [row for row in files if row["path"].endswith(suffix)]
            if len(known) != 1:
                raise ValueError("Pinned ARGB interaction layer is absent or ambiguous: " + name)
            locked = known[0]
            cache = safe(ROOT / ".work/resource-closure/payloads" / Path(locked["path"]).name)
            if not cache.is_file() or cache.stat().st_size != locked["bytes"] or digest(cache) != locked["sha256"]:
                headers = headers or resource_sources.hf_headers()
                url = ("https://huggingface.co/datasets/" + lock["storage"]["repository"] + "/resolve/"
                       + lock["storage"]["revision"] + "/" + locked["path"])
                with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=60) as response:
                    content = response.read(locked["bytes"] + 1)
                if len(content) != locked["bytes"] or hashlib.sha256(content).hexdigest() != locked["sha256"]:
                    raise ValueError("Pinned ARGB layer payload byte identity differs")
                cache.parent.mkdir(parents=True, exist_ok=True)
                cache.write_bytes(content)
            content = cache.read_bytes()
            svg = ET.fromstring(content)
            role = name.rsplit("-", 1)[0]
            owner = owners[role]["body"]
            shapes = []
            for match in re.finditer(r'\("(path|rect)",(\{[^{}]*\})\)', owner):
                shapes.append((match[1], json5.loads(match[2])))
            evidence = []
            for element in svg:
                tag = element.tag.split("}")[-1]
                if tag == "style":
                    # Prepared bases omit animation declarations. Check every
                    # retained selector/declaration against the actual CSS.
                    for rule in re.finditer(r"([^{}]+)\{([^{}]*)\}", element.text or ""):
                        selectors = rule[1].split(",")
                        expected = dict(part.split(":", 1) for part in rule[2].split(";") if part)
                        for selector in selectors:
                            candidates = [(s, b, full) for s, b, full in css_rules
                                if any(part.endswith(selector) for part in s.split(","))]
                            found = [full for _, body, full in candidates
                                if {k: v for k, v in (p.split(":", 1) for p in body.split(";") if p)
                                    if k not in ("animation", "-webkit-animation")} == expected]
                            if not found:
                                raise ValueError("Prepared ARGB base CSS differs from the current source: " + selector)
                            evidence.extend(found)
                    continue
                if tag not in ("path", "rect"):
                    raise ValueError("Unresolved ARGB SVG geometry: " + tag)
                geometry = {key: value for key, value in element.attrib.items()
                            if key not in ("fill", "stroke", "stroke-width")}
                found = [props for kind, props in shapes if kind == tag and all(str(props.get(k)) == v
                    for k, v in geometry.items())]
                if not found:
                    raise ValueError("Prepared ARGB geometry differs from current complete source: " + name)
                evidence.append({"tag": tag, "attributes": geometry, "current_source_properties": found})
                if element.get("fill") == "#111" and not any(".detect-b" in selector and "fill:#111" in body
                    for selector, body, _ in css_rules):
                    raise ValueError("ARGB active glyph paint is unverified")
                if element.get("fill") in ("#44d62c", "#666") and element.get("fill") not in css:
                    raise ValueError("ARGB animation paint is unverified")
                if element.get("stroke-width") and not any(".detect-a" in selector and "stroke-width:10px" in body
                    for selector, body, _ in css_rules):
                    raise ValueError("ARGB ring stroke is unverified")
            alias = f"synapse/wireless-argb-{pid}-{name}.svg"
            previous = [r for r in table if r["key"] == alias]
            if previous and previous != [{"key": alias, "path": locked["path"]}]:
                raise ValueError("ARGB alias repair conflicts with an existing mapping")
            if not previous:
                table.append({"key": alias, "path": locked["path"]})
            repaired.append({"display_paths": [alias], "output": locked["path"],
                "sha256": locked["sha256"], "output_bytes": locked["bytes"],
                "resource_revision": lock["storage"]["revision"], "product_id": pid,
                "source": js_row, "source_css": css_row, "source_role": role,
                "verified_geometry_and_css": evidence,
                "paint_projection": "Current original geometry retained; the Rust owner supplies the source idle/hover/pressed colors through currentColor",
                "media_materialized_in_source_assets": False})
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        resource_sources.save_resource_mappings(mappings)
        result_path = ROOT / ".analysis/results/typed-data.json"
        result = json.loads(result_path.read_bytes()) if result_path.is_file() else {"schema_version": 1}
        result["current_inline_layer_aliases"] = {"inputs": inputs, "aliases": repaired,
            "target_execution": False, "runtime_acceptance": "unverified"}
        write_json(result_path, result)
    return {"aliases": len(repaired), "files": len({row["output"] for row in repaired})}


def rust_named_item_strings(path, name, *, decode_strings=True):
    """Capture one complete Rust item and its literal values for branch audits."""
    import resource_scope
    source = safe(ROOT / path)
    raw = source.read_bytes()
    tree = resource_scope.parser(ROOT).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Rust branch owner has an AST error: " + path)
    matches = []
    def visit(node):
        identity = node.child_by_field_name("name")
        if node.type in ("function_item", "const_item", "static_item") and identity is not None:
            if raw[identity.start_byte:identity.end_byte].decode("utf-8") == name:
                matches.append(node)
        for child in node.named_children:
            visit(child)
    visit(tree.root_node)
    if len(matches) != 1:
        raise ValueError(f"Expected one Rust branch owner {path}/{name}, found {len(matches)}")
    node, strings = matches[0], []
    def literals(node):
        if node.type in ("string_literal", "raw_string_literal"):
            literal = raw[node.start_byte:node.end_byte].decode("utf-8")
            if node.type == "raw_string_literal":
                strings.append(re.sub(r'^r#*"|"#*$', '', literal))
            elif "\\" not in literal[1:-1]:
                strings.append(literal[1:-1])
            else:
                raise ValueError(f"Escaped branch literal requires Rust decoding: {path}/{name}")
            return
        for child in node.named_children:
            literals(child)
    if decode_strings:
        literals(node)
    body = raw[node.start_byte:node.end_byte]
    return strings, {"path": path, "name": name, "byte_start": node.start_byte,
        "byte_end": node.end_byte, "body": body.decode("utf-8"),
        "body_sha256": hashlib.sha256(body).hexdigest()}


def rust_call_argument_strings(path, function, index):
    """Capture literal branches from one caller argument and retain full owners."""
    import resource_scope
    source = safe(ROOT / path)
    raw = source.read_bytes()
    tree = resource_scope.parser(ROOT).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Rust resource caller has an AST error: " + path)
    result, owners = [], []
    def text(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8")
    def literals(node):
        if node.type == "string_literal":
            literal = text(node)[1:-1]
            if "\\" in literal:
                raise ValueError("Escaped resource caller argument requires Rust decoding")
            result.append(literal)
            return
        for child in node.named_children:
            literals(child)
    def visit(node, context=None):
        if node.type == "function_item":
            context = node
        if node.type == "call_expression":
            callable = node.child_by_field_name("function")
            name = text(callable.child_by_field_name("field")) if callable.type == "field_expression" else text(callable)
            if name == function:
                arguments = node.child_by_field_name("arguments").named_children
                if len(arguments) <= index:
                    raise ValueError("Resource caller argument position changed: " + path + "/" + function)
                argument = arguments[index]
                literals(argument)
                body = raw[context.start_byte:context.end_byte] if context else raw
                early_returns = []
                if context is not None:
                    for statement in context.child_by_field_name("body").named_children:
                        if statement.end_byte > node.start_byte:
                            break
                        candidate = statement.named_children[0] if statement.type == "expression_statement" else statement
                        if candidate.type != "if_expression" or candidate.child_by_field_name("alternative"):
                            continue
                        consequence = candidate.child_by_field_name("consequence").named_children
                        if len(consequence) != 1:
                            continue
                        returned = consequence[0]
                        if returned.type == "expression_statement":
                            returned = returned.named_children[0]
                        if returned.type == "return_expression" and len(returned.named_children) == 1:
                            early_returns.append({"condition": text(candidate.child_by_field_name("condition")),
                                "value": text(returned.named_children[0]), "body": text(candidate),
                                "byte_start": candidate.start_byte, "byte_end": candidate.end_byte})
                guards, ancestor = [], node.parent
                while ancestor is not None and ancestor != context:
                    if ancestor.type == "if_expression":
                        condition = ancestor.child_by_field_name("condition")
                        for field, truth in (("consequence", True), ("alternative", False)):
                            branch = ancestor.child_by_field_name(field)
                            if branch and branch.start_byte <= node.start_byte and node.end_byte <= branch.end_byte:
                                guards.append({"kind": "if", "condition": text(condition), "selected": truth,
                                    "byte_start": ancestor.start_byte, "byte_end": ancestor.end_byte})
                    elif ancestor.type == "call_expression":
                        function_node = ancestor.child_by_field_name("function")
                        if function_node.type == "field_expression" and text(function_node.child_by_field_name("field")) == "when":
                            args = ancestor.child_by_field_name("arguments").named_children
                            if len(args) == 2 and args[1].start_byte <= node.start_byte and node.end_byte <= args[1].end_byte:
                                guards.append({"kind": "when", "condition": text(args[0]), "selected": True,
                                    "byte_start": ancestor.start_byte, "byte_end": ancestor.end_byte})
                    ancestor = ancestor.parent
                owners.append({"path": path, "function": function, "argument": index,
                    "owner_name": text(context.child_by_field_name("name")) if context else None,
                    "branch_guards": guards,
                    "early_returns": early_returns,
                    "argument_body": text(argument), "call_byte_start": node.start_byte, "call_byte_end": node.end_byte,
                    "owner_byte_start": context.start_byte if context else 0,
                    "owner_byte_end": context.end_byte if context else len(raw),
                    "owner_body": body.decode("utf-8"), "owner_body_sha256": hashlib.sha256(body).hexdigest()})
        for child in node.named_children:
            visit(child, context)
    visit(tree.root_node)
    return result, owners


def rust_finite_string_values(expression, bindings=None):
    """Project a finite Rust string expression without executing target code."""
    import itertools
    import resource_scope
    bindings = {re.sub(r"\s+", "", key): value for key, value in (bindings or {}).items()}
    parser = resource_scope.parser(ROOT)
    def parse(source):
        raw = ("fn __resource_domain() { " + source + " }").encode()
        tree = parser.parse(raw)
        if tree.root_node.has_error:
            raise ValueError("Resource expression has a Rust AST error")
        body = tree.root_node.named_children[0].child_by_field_name("body")
        if len(body.named_children) != 1:
            raise ValueError("Expected one resource expression")
        node = body.named_children[0]
        if node.type == "expression_statement":
            node = node.named_children[0]
        return raw, node
    def condition(node, raw):
        key = re.sub(r"\s+", "", raw[node.start_byte:node.end_byte].decode())
        if key in bindings:
            return {bool(bindings[key])}
        if node.type == "boolean_literal":
            return {key == "true"}
        if node.type == "unary_expression" and key.startswith("!"):
            return {not value for value in condition(node.named_children[-1], raw)}
        if node.type == "binary_expression":
            operator = node.child_by_field_name("operator")
            op = raw[operator.start_byte:operator.end_byte].decode() if operator else ""
            if op in ("||", "&&"):
                left = condition(node.child_by_field_name("left"), raw)
                right = condition(node.child_by_field_name("right"), raw)
                return {(a or b) if op == "||" else (a and b) for a in left for b in right}
        # An unknown condition can select either finite arm. Unknown string
        # values are rejected below; a wildcard alias list never supplies them.
        return {False, True}
    def strings(node, raw):
        if node.type == "string_literal":
            return {json.loads(raw[node.start_byte:node.end_byte].decode())}
        if node.type in ("block", "else_clause", "parenthesized_expression", "reference_expression"):
            children = node.named_children
            if len(children) != 1:
                raise ValueError("Resource branch contains unresolved statements")
            return strings(children[0], raw)
        if node.type == "if_expression":
            result = set()
            for truth in condition(node.child_by_field_name("condition"), raw):
                branch = node.child_by_field_name("consequence" if truth else "alternative")
                if branch is None:
                    raise ValueError("Resource branch has no string result")
                result.update(strings(branch, raw))
            return result
        if node.type == "macro_invocation":
            name = node.child_by_field_name("macro")
            if name is None or raw[name.start_byte:name.end_byte] != b"format":
                raise ValueError("Unresolved resource string macro")
            tokens = next(child for child in node.named_children if child.type == "token_tree")
            arguments_raw, call = parse("__format" + raw[tokens.start_byte:tokens.end_byte].decode())
            arguments = call.child_by_field_name("arguments").named_children
            formats = strings(arguments[0], arguments_raw)
            values = [strings(argument, arguments_raw) for argument in arguments[1:]]
            return {fmt.format(*combination) for fmt in formats for combination in itertools.product(*values)}
        raise ValueError("Unresolved resource string expression: " + node.type)
    raw, node = parse(expression)
    return sorted(strings(node, raw))


def dock_resource_domain():
    """Capture the three dock product branches and every dynamic SVG caller."""
    base = "crates/razer-pages/src/features/accessories/dock_pairing"
    data_path = ".analysis/inputs/typed-tables/razer-pages/dock_pairing_data.json"
    products = json.loads((ROOT / data_path).read_bytes())
    config = {product["product_id"]: product["config"] for product in products}
    if set(config) != {164, 241, 756} or any(row.get("productId") != pid for pid, row in config.items()):
        raise ValueError("Dock product identity branches changed")
    dual = {pid for pid, row in config.items() if row.get("canPairTwoDevices") is True}
    legacy = {pid for pid, row in config.items() if row.get("legacyDualLink") is True}
    if dual != {241} or legacy != {756}:
        raise ValueError("Dock dual/legacy capability branches changed")
    owners, calls, domains = [], [], {pid: set() for pid in config}
    # These are complete owner identities, not matches against the alias index.
    inventory = {base + ".rs": ["page", "warning_icon", "spinner"],
        base + "/widget.rs": ["render"],
        base + "/dialog.rs": ["channel", "body", "body", "body"],
        base + "/dialog/legacy.rs": ["legacy_body", "legacy_body", "legacy_body"]}
    for path, names in inventory.items():
        _, captured = rust_call_argument_strings(path, "asset", 0)
        if [row["owner_name"] for row in captured] != names:
            raise ValueError("Dock dynamic resource caller inventory changed: " + path)
        for caller in captured:
            owner = caller["owner_name"]
            pids = set(config)
            if path.endswith("/widget.rs") or owner == "legacy_body":
                pids = legacy
            elif owner in ("page", "body"):
                pids -= legacy
            elif owner == "warning_icon":
                pids = dual
            argument = caller["argument_body"]
            if owner == "body":
                if not any(re.sub(r"\s+", "", ret["condition"]) == "self.spec.legacy()"
                    and re.sub(r"\s+", "", ret["value"]) == "self.legacy_body(cx)"
                    for ret in caller["early_returns"]):
                    raise ValueError("Dock legacy body return changed")
                for guard in caller["branch_guards"]:
                    if re.sub(r"\s+", "", guard["condition"]) == "dual":
                        pids = pids & dual if guard["selected"] else pids - dual
            selected = {}
            for pid in sorted(pids):
                bindings = {"dual": pid in dual, "self.multi_pairing": False}
                possibilities = [bindings]
                if owner == "page" and config[pid].get("isMultiPairingSupported") is True:
                    possibilities.append({**bindings, "self.multi_pairing": True})
                values = {value for branch in possibilities for value in rust_finite_string_values(argument, branch)}
                if owner == "channel" and pid == 164:
                    values = set(rust_finite_string_values(argument, {"lane == Lane::Mouse": True}))
                domains[pid].update(values)
                selected[str(pid)] = sorted(values)
            calls.append({**caller, "selected_names_by_product": selected})
    for path, names in {
        base + ".rs": ("spec", "dual", "legacy", "new", "page", "warning", "dialog_warning"),
        base + "/dialog.rs": ("body",),
        base + "/preview.rs": ("select",),
    }.items():
        for name in names:
            _, owner = rust_named_item_strings(path, name, decode_strings=False)
            owners.append(owner)
    # Preview writes the optional original-dongle warning only for the dual
    # product; production initializes it false and observes that case via dual.
    preview = next(row["body"] for row in owners if row["name"] == "select")
    if "view.original_dongle=dual&&id==\"original_dongle\"" not in re.sub(r"\s+", "", preview):
        raise ValueError("Dock preview warning capability guard changed")
    return {"verified": True, "expected_aliases": sorted(f"synapse/dock-{pid}-{name}.svg"
        for pid, values in domains.items() for name in values), "owners": owners, "callers": calls,
        "product_branches": [{"product_id": pid, "dual": pid in dual, "legacy": pid in legacy,
            "names": sorted(domains[pid])} for pid in sorted(config)],
        "inputs": [{"path": data_path, "bytes": (ROOT / data_path).stat().st_size,
            "sha256": digest(ROOT / data_path)}],
        "basis": "Finite expression projection of every captured dynamic caller, separated by current single/dual/legacy product branches; no alias-index inference or target execution",
        "runtime_acceptance": "unverified"}


def finite_resource_domain(row):
    """Enumerate audited caller branches independently of the resource directory."""
    value = row["literal"]
    text = (ROOT / row["owner"]).read_text(encoding="utf-8")
    expected, constraints, owners = None, [], []
    def choices(prefix, names, suffix=".svg"):
        return [prefix + str(name) + suffix for name in names]
    if value == "synapse/dock-{}-{name}.svg":
        return dock_resource_domain()
    elif value == "synapse/audio-footstep-{frame:03}.svg":
        constraints = ["min(329)", "phase * 330."]
        expected = [f"synapse/audio-footstep-{i:03}.svg" for i in range(330)]
    elif value in ("synapse/keyboard-chroma-sync-{:02}.png", "synapse/keyboard-chroma-sync-{frame:02}.png"):
        constraints = ["phase * 108.", "% 36"]
        expected = [f"synapse/keyboard-chroma-sync-{i:02}.png" for i in range(36)]
    elif value == "synapse/wired-argb-3871-detecting-ring-{frame:03}.svg":
        constraints = ["min(119)", "phase * 120."]
        expected = [f"synapse/wired-argb-3871-detecting-ring-{i:03}.svg" for i in range(120)]
    elif value == "synapse/wired-argb-3871-detecting-led-{side}{number}.svg":
        constraints = ['["left", "right"]', "1..=9"]
        expected = [f"synapse/wired-argb-3871-detecting-led-{side}{i}.svg" for side in ("left", "right") for i in range(1, 10)]
    elif value in ("synapse/tray-widget-battery-{}.svg", "synapse/tray-widget-battery-paused-{}.svg"):
        constraints = ["(level / 10) * 10", "100"]
        prefix = "synapse/tray-widget-battery-" + ("paused-" if "paused" in value else "")
        expected = choices(prefix, range(0, 101, 10))
    elif value == "synapse/tray-widget-category-{}.svg":
        names, owner = rust_named_item_strings(row["owner"], "NAMES")
        owners.append(owner)
        constraints = ["NAMES.contains(&name)"]
        expected = choices("synapse/tray-widget-category-", [name.lower() for name in names])
    elif value == "synapse/service-category-{}.svg":
        data = json.loads((ROOT / ".analysis/inputs/typed-tables/razer-app-pages/module_service_source.json").read_bytes())
        constraints = ["source.categories.contains(first)", "source.categories.contains(&category)", "key.to_ascii_lowercase()"]
        expected = choices("synapse/service-category-", [name.lower() for name in data["categories"]])
    elif value == "synapse/aether-{name}.svg":
        names = ["tooltip_questionmark", "close-hovered-btn", "offline-badge"]
        for path, function, argument in (
            (row["owner"], "icon_button", 1),
            ("crates/razer-pages/src/features/accessories/aether_strip/device.rs", "SourceTipItem::new", 1)):
            strings, captured = rust_call_argument_strings(path, function, argument)
            names.extend(strings)
            owners.extend(captured)
        for name in ("distribute", "restored"):
            _, captured = rust_named_item_strings("crates/razer-pages/src/features/accessories/aether_strip/state.rs", name)
            owners.append(captured)
        if all(token in owners[-2]["body"] for token in ("1..=4", "(0..sides)")) and "array.len() > 4" in owners[-1]["body"]:
            constraints = ["(1..=4u32)", 'format!("side-{}", id + 1)']
            expected = [*choices("synapse/aether-", names),
                        *choices("synapse/aether-side-", range(1, 5)), *choices("synapse/aether-shape-", range(1, 5))]
    elif value == "synapse/{file}":
        constraints = ["(0..=100).contains(&bucket)", '"paused-"', '"battery-error.svg"']
        expected = ["synapse/battery-charging.svg", "synapse/battery-error.svg", "synapse/battery-charging-100.svg",
            *choices("synapse/battery-", range(0, 101, 10)), *choices("synapse/battery-paused-", range(0, 101, 10))]
    elif value.startswith("synapse/wireless-argb-{}-"):
        constraints = ["3884", "3886"]
        if value.endswith("-{name}-base.svg"):
            names = ["power-base", "power_off-base"]
        elif value.endswith("-{name}-hover.svg"):
            names = ["refresh-hover", "warning-hover", "remove-hover", "power-hover", "power_off-hover"]
        elif value.endswith("-detecting.svg"):
            names = ["detecting"]
        elif value.endswith("-{name}.png"):
            names = ["fan", "strip-1", "strip-2", "strip-3", "strip-4", "prd-3x"]
        elif any(owner["name"] == "plain_icon" for owner in row["context"]):
            names = ["rename"]
        else:
            names = ["auto_on-ring", "auto_on-glyph", "auto_off-glyph", "auto_off-green", "auto_off-gray"]
        extension = ".png" if value.endswith(".png") else ".svg"
        expected = [f"synapse/wireless-argb-{pid}-{name}{extension}" for pid in (3884, 3886) for name in names]
    elif value == "synapse/hyperflux-207/product-{edition}-3x.png":
        constraints = ["128", "edition"]
        expected = [f"synapse/hyperflux-207/product-{edition}-3x.png" for edition in (0, 128)]
    elif value == "synapse/automation-icon_direction_{name}{}.svg":
        constraints = ['[(3, "up"), (4, "down")]', '"_999"']
        expected = [f"synapse/automation-icon_direction_{name}{suffix}.svg" for name in ("up", "down") for suffix in ("", "_999")]
    elif value == "synapse/automation-{kind}.svg":
        names, owner = rust_named_item_strings(row["owner"], "action_name")
        owners.append(owner)
        if len(names) == 6:
            return {"verified": False, "expected_aliases": choices("synapse/automation-", range(6)), "owners": owners,
                "unresolved_constraints": ["Generated action IDs are 0..6, but restored rule IDs are not bounded before icon() and action_name()"]}
    elif value in ("synapse/automation-quick-macro-{kind}.svg", "synapse/automation-quick-macro-{}.svg"):
        names = ("keyboard", "launch", "runCommand", "text")
        constraints = ['value: "' + name + '"' for name in names]
        expected = choices("synapse/automation-quick-macro-", names)
    elif value == "synapse/audio-mixer-{icon}.svg":
        names = ("playback-mix", "stream-mix", "line-out", "voice-chat")
        constraints = ['"' + name + '"' for name in names] + ["BUSES[1..]", "BUSES[0]"]
        expected = choices("synapse/audio-mixer-", names)
    elif value == "synapse/stepper-{direction}.svg":
        constraints = ['[(1, "up"), (-1, "down")]']
        expected = choices("synapse/stepper-", ("up", "down"))
    elif value == "synapse/controller_icon_{name}.svg":
        constraints = ['"wasd"', '"qe"', "groups.map"]
        expected = choices("synapse/controller_icon_", ("wasd", "qe"))
    elif value == "synapse/xbox-trigger-{name}.svg":
        constraints = ['trigger("left"', 'trigger("right"']
        expected = choices("synapse/xbox-trigger-", ("left", "right"))
    elif value == "synapse/chroma-dashboard_{icon}.svg":
        names = ("spectrumcycling", "static_green", "breathing_green", "wave", "fire", "starlight", "wheel")
        constraints = ['"' + name + '"' for name in names]
        expected = choices("synapse/chroma-dashboard_", names)
    elif value == "synapse/chroma-settings-{asset}.svg":
        constraints = ['"chroma-settings-back"', '"chroma-settings-forward"', '"chroma-settings-refresh"']
        expected = choices("synapse/chroma-settings-", ("back", "forward", "refresh"))
    elif value == "synapse/audio-mixer-lighting-direction-{}{}.svg":
        names = ("cw", "ccw", "left", "right")
        constraints = ['"' + name + '"' for name in names] + ['"-active"']
        expected = [f"synapse/audio-mixer-lighting-direction-{name}{suffix}.svg" for name in names for suffix in ("", "-active")]
    elif value == "synapse/audio-oled-artwork-{kind}.svg":
        constraints = ['artwork_icon("replace"', 'artwork_icon("reset"']
        expected = choices("synapse/audio-oled-artwork-", ("replace", "reset"))
    elif value in ("synapse/seiren-eq-tutorial-{kind}.svg", "synapse/seiren-eq-tutorial-{}.png"):
        names = ("peaking", "highpass", "lowpass", "highshelf", "lowshelf")
        constraints = ['"' + name + '"' for name in names]
        expected = choices("synapse/seiren-eq-tutorial-", range(1, 6), ".png") if value.endswith(".png") else choices("synapse/seiren-eq-tutorial-", names)
    elif value == "synapse/audio-surround-{}-tutorial-{}.webp":
        constraints = ["1392 | 1442 | 3942", "step + 1", "0..6"]
        expected = [f"synapse/audio-surround-{pid}-tutorial-{step}.webp" for pid in (1392, 1442, 3942) for step in range(1, 7)]
    elif value == "synapse/seiren-eq-{name}{}.svg":
        strings, owner = rust_named_item_strings("crates/razer-pages/src/features/audio/seiren/audio_seiren_eq.rs", "MONITORS")
        owners.append(owner)
        names = strings[::2]
        constraints = ["let (name, _) = MONITORS[index]", 'if muted { "-off" } else { "" }']
        expected = [f"synapse/seiren-eq-{name}{suffix}.svg" for name in names for suffix in ("", "-off")]
    elif value == "synapse/keyboard679-current/{icon}":
        strings, owner = rust_named_item_strings(row["owner"], "special_icons")
        owners.append(owner)
        constraints = ["special_icons(default)", "let Some((normal_icon, pressed_icon))", "return None"]
        expected = ["synapse/keyboard679-current/" + name for name in strings if name.endswith(".svg")]
    elif value == "synapse/keyboard-mapping-dynamic/{}":
        data = json.loads((ROOT / ".analysis/inputs/typed-tables/razer-pages/keyboard_mapping_dynamic_data.json").read_bytes())
        constraints = ["for (index, h) in m.headers.iter().enumerate()", "h.icon"]
        expected = ["synapse/keyboard-mapping-dynamic/" + header["icon"] for product in data for header in product["headers"]]
    elif value == "synapse/keyboard-mapping-notices/{icon}":
        names = []
        for item in ("mapping_chroma_install_element", "mapping_prerequisite_element"):
            strings, owner = rust_named_item_strings(row["owner"], item)
            owners.append(owner)
            names.extend(name for name in strings if name.endswith((".svg", ".png")))
        constraints = ["prerequisite_notice(", "for (show, icon, label, tooltip) in"]
        expected = ["synapse/keyboard-mapping-notices/" + name for name in names]
    elif value == "synapse/mouse-calibration-70/{name}":
        names, captured = rust_call_argument_strings(row["owner"], "calibration_image", 0)
        owners.extend(captured)
        strings, owner = rust_named_item_strings(row["owner"], "calibration_steps")
        owners.append(owner)
        names.extend(name for name in strings if name.endswith(".svg"))
        constraints = [".images.get(name)", "calibration_artwork(self.spec.product_id)"]
        expected = ["synapse/mouse-calibration-70/" + name for name in names]
    elif value == "synapse/keyboard-keymap-slot-{slot}.png":
        return {"verified": False, "expected_aliases": choices("synapse/keyboard-keymap-slot-", range(1, 9), ".png"),
            "unresolved_constraints": ["New keymaps allocate slots 1..=8; restored row slot remains an unvalidated external string"]}
    elif value in ("synapse/direction-{asset}{}.svg", "synapse/direction-{name}{}.svg"):
        names = [name for name in ("out", "in", "cw", "ccw", "left", "right") if '"' + name + '"' in text]
        constraints = ['"-active"']
        expected = [f"synapse/direction-{name}{suffix}.svg" for name in names for suffix in ("", "-active")]
    elif value == "synapse/{}direction-{name}{}.svg":
        names = [name for name in ("out", "in", "cw", "ccw", "left", "right", "up", "down") if '"' + name + '"' in text]
        constraints = ['"mouse-lighting/"', '"-active"']
        expected = ["synapse/" + ("mouse-lighting/" if name in ("up", "down") else "")
            + f"direction-{name}{suffix}.svg" for name in names for suffix in ("", "-active")]
    elif value == "synapse/mapping-{stem}{}.{}":
        names = ("profile", "sensitivity", "lighting", "macro", "text", "launch", "multimedia", "windows")
        constraints = ['"' + name + '"' for name in names]
        expected = [f"synapse/mapping-{name}{suffix}." + ("png" if name == "lighting" else "svg") for name in names for suffix in ("", "-active")]
    elif value == "synapse/mapping-{}{}.{}":
        names = ("default", "keyboard", "mouse", "sensitivity", "macro", "interdevice", "profile", "lighting",
                 "brightness", "hypershift", "launch", "multimedia", "windows", "text", "disable")
        constraints = ['=> "' + name + '"' for name in names]
        expected = [f"synapse/mapping-{name}{suffix}." + ("png" if name == "lighting" else "svg") for name in names for suffix in ("", "-active")]
    elif value == "synapse/macro/{kind}.svg":
        names = ("delay", "keyboard", "mouse", "macro", "launch", "command", "text", "loop")
        constraints = ['("' + name + '",' for name in names] + ["PALETTE.into_iter()"]
        expected = choices("synapse/macro/", names)
    elif value == "synapse/macro/{icon}{}.svg":
        names = ("record", "delete") if row["owner"].endswith("phased.rs") else ("duplicate", "delete")
        constraints = ['"' + name + '"' for name in names] + ['"-hover"']
        expected = [*choices("synapse/macro/", names), "synapse/macro/delete-hover.svg"]
    elif value == "synapse/macro/{kind}{suffix}.svg":
        callers = (ROOT / "crates/razer-app-pages/src/macros/macro_page/body.rs").read_text(encoding="utf-8")
        if all('history_icon("' + name + '"' in callers for name in ("undo", "redo")):
            constraints = ['"-enable"', '"-hover"']
            expected = [f"synapse/macro/{name}{suffix}.svg" for name in ("undo", "redo") for suffix in ("", "-enable", "-hover")]
    elif value in ("synapse/macro/{}.svg", "synapse/macro/drag-{icon}.svg"):
        owner = ROOT / "crates/razer-model/src/macro_library.rs"
        source = owner.read_text(encoding="utf-8")
        match = re.search(r"pub fn icon\(self\).*?match self\s*\{(.*?)\n\s*\}", source, re.S)
        if match:
            names = re.findall(r'=> "([\w-]+)"', match[1])
            if value == "synapse/macro/{}.svg":
                constraints = ['"mouse-movement"', "kind.icon()"]
                expected = choices("synapse/macro/", [*names, "mouse-movement"])
            else:
                constraints = ['"layers"', "self.drag.kind.icon()"]
                expected = choices("synapse/macro/drag-", [*names, "layers"])
    elif value == "synapse/wired-argb-{}-dots.svg":
        data = json.loads((ROOT / ".analysis/inputs/typed-tables/razer-pages/wired_argb_data.json").read_bytes())
        constraints = ["self.spec.product_id"]
        expected = [f"synapse/wired-argb-{row['product_id']}-dots.svg" for row in data]
    elif value.startswith("synapse/gamepad-"):
        products = json.loads((ROOT / ".analysis/inputs/typed-tables/razer-pages/gamepad_products_data.json").read_bytes())
        pages = lambda page: [p["product_id"] for p in products if page in p["pages"]]
        modern = (2676, 2684)
        sticks = pages("THUMBSTICKS")
        legacy = [pid for pid in sticks if pid not in modern]
        if value == "synapse/gamepad-thumbstick-device/{}/{edition}.svg":
            constraints = ["matches!(self.spec.product_id, 2676 | 2684)", "load_edition_resource"]
            expected = [f"synapse/gamepad-thumbstick-device/{pid}/0.svg" for pid in modern]
        elif value == "synapse/gamepad-thumbsticks/{pid}/highlight.svg":
            constraints = ["self.modern_thumbsticks()", "thumbstick_legacy"]
            expected = [f"synapse/gamepad-thumbsticks/{pid}/highlight.svg" for pid in legacy]
        elif value in ("synapse/gamepad-thumbsticks/{pid}/{mode}-min.svg", "synapse/gamepad-thumbsticks/{pid}/{mode}-max.svg"):
            constraints = ['let mode = if sensitivity { "sensitivity" } else { "deadzone" }']
            bound = "min" if "-min.svg" in value else "max"
            expected = [f"synapse/gamepad-thumbsticks/{pid}/{mode}-{bound}.svg" for pid in legacy for mode in ("sensitivity", "deadzone")]
        elif value in ("synapse/gamepad-thumbsticks/{}/info.svg", "synapse/gamepad-thumbsticks/{}/warning.svg"):
            constraints = ["self.spec.product_id"]
            name = "info" if "info.svg" in value else "warning"
            expected = [f"synapse/gamepad-thumbsticks/{pid}/{name}.svg" for pid in sticks]
        elif value == "synapse/gamepad-trigger-reset/{}.svg":
            constraints = ["self.spec.product_id"]
            expected = [f"synapse/gamepad-trigger-reset/{pid}.svg" for pid in pages("TRIGGERS")]
        elif value == "synapse/gamepad-trigger-device/{pid}/{edition}.{extension}":
            constraints = ["matches!(pid, 2676 | 2684 | 4144)", "load_edition_resource"]
            registry = json.loads((ROOT / ".analysis/inputs/typed-tables/razer-catalog/product_registry.json").read_bytes())["products"]
            expected = [f"synapse/gamepad-trigger-device/{p['id']}/{edition}." + ("svg" if p["id"] in (*modern, 4144) else "png")
                for p in registry if p["id"] in pages("TRIGGERS") for edition in p["edition_ids"]]
        elif value == "synapse/gamepad-{pid}-calibration-edition-{edition}.png":
            constraints = ["(2629 | 2636 | 2647 | 4133, 128) => 128", "(2636 | 4133, 129) => 129", "let is_svg = pid == 4144"]
            expected = [f"synapse/gamepad-{pid}-calibration-edition-{edition}.png" for pid in legacy if pid != 4144
                for edition in ([0, 128, 129] if pid in (2636, 4133) else [0, 128] if pid in (2629, 2647) else [0])]
        elif value == "synapse/gamepad-{pid}-calibration-close.svg":
            constraints = ["calibration-close.svg"]
            expected = [f"synapse/gamepad-{pid}-calibration-close.svg" for pid in modern]
        elif value == "synapse/gamepad-{pid}-calibration-connectors-{}.svg":
            constraints = ["hovered_part.unwrap_or(0)"]
            expected = [f"synapse/gamepad-{pid}-calibration-connectors-{part}.svg" for pid in modern for part in range(5)]
        elif value == "synapse/gamepad-{pid}-trigger-steps-{}.svg":
            constraints = ["if !error && view.step >= 11 { 2 } else { 1 }"]
            expected = [f"synapse/gamepad-{pid}-trigger-steps-{step}.svg" for pid in modern for step in (1, 2)]
        elif value == "synapse/gamepad-{pid}-trigger-warning.svg":
            constraints = ["let error = view.step == -1"]
            expected = [f"synapse/gamepad-{pid}-trigger-warning.svg" for pid in modern]
        elif value in ("synapse/gamepad-2636-calibration-steps-{}.svg", "synapse/gamepad-2636-calibration-direction-{}-{}.svg"):
            state = (ROOT / "crates/razer-pages/src/features/gamepad/gamepad_calibration_state.rs").read_text(encoding="utf-8")
            if "!(-1..=6).contains(&step)" in state:
                constraints = ["state.step"]
                if "-steps-" in value:
                    expected = [f"synapse/gamepad-2636-calibration-steps-{step}.svg" for step in range(1, 7)]
                else:
                    expected = [f"synapse/gamepad-2636-calibration-direction-{step}-{edge}.svg" for step in range(1, 6) for edge in ("edge", "rest")]
    elif value == "synapse/hue-light-{name}.svg":
        path = ROOT / ".analysis/inputs/typed-tables/razer-pages/hue_data.json"
        data = json.loads(path.read_bytes())
        names = [row["name"] for row in data["archetypes"]] + ["default"]
        constraints = ["spec().archetypes.iter().any", '"default"']
        expected = choices("synapse/hue-light-", names)
    if expected is None:
        return None
    compact = re.sub(r"\s+", "", text)
    absent = [constraint for constraint in constraints if re.sub(r"\s+", "", constraint) not in compact]
    if absent:
        return {"verified": False, "unresolved_constraints": absent, "expected_aliases": expected}
    return {"verified": True, "constraints": constraints, "owners": owners,
        "expected_aliases": sorted(set(expected)),
        "basis": "Enumerated caller branches and bounded animation/state ranges from the captured Rust owner; independent of registered alias candidates"}


def module_service_declared_artwork(declared):
    """Resolve the one declared module-service chunk pair and application owner."""
    files = declared.get("files")
    if not isinstance(files, dict):
        raise ValueError("Module service asset manifest has no file declarations")
    selected = {}
    for kind in ("js", "css"):
        candidates = [(key, value) for key, value in files.items()
                      if re.fullmatch(r"static/" + kind + r"/6505\.[0-9a-f]+\.chunk\." + kind, key)]
        if len(candidates) != 1:
            raise ValueError("Module service declared chunk is missing or ambiguous: " + kind)
        key, value = candidates[0]
        if not isinstance(value, str) or value.removeprefix("./") != key:
            raise ValueError("Module service declared chunk points outside its canonical identity")
        selected[kind] = key
    app = files.get("App.js")
    if not isinstance(app, str) or not re.fullmatch(r"(?:\./)?static/js/App\.[0-9a-f]+\.chunk\.js", app):
        raise ValueError("Module service application chunk is not declared")
    selected["app"] = app.removeprefix("./")
    return selected


def module_service_deployment_projection(declared, main_raw, app_raw, script_raw):
    """Bind literal runtime paths and actual lazy consumers to factory 44442."""
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    parser = Parser(Language(tree_sitter_javascript.language()))
    selected = module_service_declared_artwork(declared)
    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)
    def one(nodes, role):
        nodes = list(nodes)
        if len(nodes) != 1:
            raise ValueError("Module service deployment owner is missing or ambiguous: " + role)
        return nodes[0]
    def text(node):
        return node.text.decode("utf-8") if node is not None else ""
    def capture(node):
        return {"byte_start": node.start_byte, "byte_end": node.end_byte,
                "body": text(node), "body_sha256": hashlib.sha256(node.text).hexdigest()}
    def function_owner(node):
        parent = node.parent
        while parent is not None and parent.type not in {"arrow_function", "function_expression", "function_declaration"}:
            parent = parent.parent
        return parent
    def parse(raw):
        tree = parser.parse(raw)
        if tree.root_node.has_error:
            raise ValueError("Module service deployment has a JavaScript AST error")
        return tree.root_node
    def property(node, name):
        return node is not None and node.type == "member_expression" and text(node.child_by_field_name("property")) == name
    def unwrap(node):
        while node.type == "parenthesized_expression" and len(node.named_children) == 1:
            node = node.named_children[0]
        return node
    def literal(node, parameter, identifier):
        node = unwrap(node)
        if node.type == "string":
            return json.loads(text(node))
        if node.type == "identifier" and text(node) == parameter:
            return identifier
        if node.type == "subscript_expression":
            table = unwrap(node.child_by_field_name("object"))
            index = node.child_by_field_name("index")
            if table.type != "object" or index.type != "identifier" or text(index) != parameter:
                raise ValueError("Module service chunk lookup is not a literal runtime table")
            values = {}
            for pair in table.named_children:
                if pair.type != "pair" or pair.child_by_field_name("key").type != "number" \
                        or pair.child_by_field_name("value").type != "string":
                    raise ValueError("Module service chunk table contains an indirect entry")
                key = text(pair.child_by_field_name("key"))
                if not key.isdecimal() or key in values:
                    raise ValueError("Module service chunk table identity is invalid or repeated")
                values[key] = json.loads(text(pair.child_by_field_name("value")))
            return values.get(str(identifier))
        if node.type == "binary_expression":
            left = literal(node.child_by_field_name("left"), parameter, identifier)
            right = literal(node.child_by_field_name("right"), parameter, identifier)
            operator = text(node.child_by_field_name("operator"))
            if operator == "||":
                return left or right
            if operator == "+" and left is not None and right is not None and (isinstance(left, str) or isinstance(right, str)):
                return str(left) + str(right)
        raise ValueError("Module service runtime filename expression is unsupported")
    main, app, script = parse(main_raw), parse(app_raw), parse(script_raw)
    assignments = {}
    for kind, field in (("js", "u"), ("css", "miniCssF")):
        assignment = one((node for node in walk(main) if node.type == "assignment_expression"
                          and property(node.child_by_field_name("left"), field)), field)
        function = assignment.child_by_field_name("right")
        parameter = function.child_by_field_name("parameter")
        if function.type != "arrow_function" or parameter is None or parameter.type != "identifier":
            raise ValueError("Module service runtime filename producer is not a literal arrow")
        for identifier, expected in ((6505, selected[kind]), *(((8180, selected["app"]),) if kind == "js" else ())):
            if literal(function.child_by_field_name("body"), text(parameter), identifier) != expected:
                raise ValueError("Module service runtime filename differs from the current declaration")
        assignments[kind] = assignment
    runtime = text(assignments["js"].child_by_field_name("left").child_by_field_name("object"))
    if text(assignments["css"].child_by_field_name("left").child_by_field_name("object")) != runtime:
        raise ValueError("Module service runtime owners differ")
    public = one((node for node in walk(main) if node.type == "assignment_expression"
        and property(node.child_by_field_name("left"), "p")
        and function_owner(node) == function_owner(assignments["js"])
        and text(node.child_by_field_name("left").child_by_field_name("object")) == runtime), "public path")
    if public.child_by_field_name("right").type != "string" or json.loads(text(public.child_by_field_name("right"))) not in {"./", "/synapse/dashboard/"}:
        raise ValueError("Module service runtime public path differs")
    def chunk_factory(root, chunk, module):
        candidates = []
        for call in walk(root):
            if call.type != "call_expression" or not property(call.child_by_field_name("function"), "push"):
                continue
            args = call.child_by_field_name("arguments").named_children
            if len(args) != 1 or args[0].type != "array":
                continue
            parts = args[0].named_children
            if len(parts) < 2 or parts[0].type != "array" or [text(n) for n in parts[0].named_children] != [str(chunk)] or parts[1].type != "object":
                continue
            candidates.extend(node for node in parts[1].named_children if node.type == "pair"
                              and text(node.child_by_field_name("key")) == str(module))
        return one(candidates, str(chunk) + "/" + str(module))
    def load_edge(root, chunk, module, loader):
        candidates = []
        for call in walk(root):
            if call.type != "call_expression" or not property(call.child_by_field_name("function"), "then"):
                continue
            args = call.child_by_field_name("arguments").named_children
            if len(args) != 1 or args[0].type != "call_expression" or not property(args[0].child_by_field_name("function"), "bind"):
                continue
            bind = args[0]
            if text(bind.child_by_field_name("function").child_by_field_name("object")) != loader \
                    or [text(n) for n in bind.child_by_field_name("arguments").named_children] != [loader, str(module)]:
                continue
            pending = call.child_by_field_name("function").child_by_field_name("object")
            loads = [pending]
            if pending.type == "call_expression" and text(pending.child_by_field_name("function")) == "Promise.all":
                arguments = pending.child_by_field_name("arguments").named_children
                loads = arguments[0].named_children if len(arguments) == 1 and arguments[0].type == "array" else []
            if not loads or any(load.type != "call_expression" or not property(load.child_by_field_name("function"), "e")
                    or text(load.child_by_field_name("function").child_by_field_name("object")) != loader
                    or len(load.child_by_field_name("arguments").named_children) != 1
                    or load.child_by_field_name("arguments").named_children[0].type != "number" for load in loads):
                continue
            if sum([text(n) for n in load.child_by_field_name("arguments").named_children] == [str(chunk)] for load in loads) == 1:
                candidates.append(call)
        return one(candidates, "lazy " + str(chunk) + "/" + str(module))
    application = chunk_factory(app, 8180, 35378)
    function = application.child_by_field_name("value")
    parameters = function.child_by_field_name("parameters")
    if function.type != "arrow_function" or parameters is None or len(parameters.named_children) != 3:
        raise ValueError("Module service application factory signature differs")
    app_loader = text(parameters.named_children[2])
    return {"declared": selected, "main_javascript_loader": capture(assignments["js"]),
            "main_stylesheet_loader": capture(assignments["css"]), "public_path": capture(public),
            "mounted_application": capture(load_edge(main, 8180, 35378, runtime)),
            "complete_application_factory": capture(application),
            "mounted_module_service": capture(load_edge(application, 6505, 44442, app_loader)),
            "complete_module_service_factory": capture(chunk_factory(script, 6505, 44442)),
            "target_execution": False}


def module_service_artwork_projection(raw):
    """Read the complete current More Information image owner without running JS."""
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Module service original has a JavaScript AST error")
    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)
    def text(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8")
    def one(nodes, role):
        nodes = list(nodes)
        if len(nodes) != 1:
            raise ValueError("Module service image owner is unresolved: " + role)
        return nodes[0]
    def capture(node):
        body = raw[node.start_byte:node.end_byte]
        return {"byte_start": node.start_byte, "byte_end": node.end_byte,
                "body": body.decode("utf-8"), "body_sha256": hashlib.sha256(body).hexdigest()}
    factory_pair = one((node for node in walk(tree.root_node) if node.type == "pair"
        and text(node.child_by_field_name("key")) == "44442"), "44442 factory")
    factory = factory_pair.child_by_field_name("value")
    if factory.type != "arrow_function":
        raise ValueError("Module service factory is not the retained arrow producer")
    declarations = [node for statement in factory.child_by_field_name("body").named_children
                    if statement.type in {"lexical_declaration", "variable_declaration"}
                    for node in statement.named_children if node.type == "variable_declarator"]
    def declaration(name, within=declarations):
        return one((node for node in within if text(node.child_by_field_name("name")) == name), name)
    row = declaration("w")
    image = one((node for node in walk(row) if node.type == "object" and any(
        pair.type == "pair" and text(pair.child_by_field_name("key")) == "className"
        and text(pair.child_by_field_name("value")) == '"item-description-image"'
        for pair in node.named_children)), "description image")
    props = {text(node.child_by_field_name("key")): node.child_by_field_name("value")
             for node in image.named_children if node.type == "pair"}
    source = props.get("src")
    if (source is None or source.type != "ternary_expression"
            or text(source.child_by_field_name("alternative")) != "n.detail.srcImage"):
        raise ValueError("Module service description must consume detail.srcImage")
    failure_name = text(source.child_by_field_name("condition"))
    fallback_name = text(source.child_by_field_name("consequence"))
    if source.child_by_field_name("condition").type != "identifier" or source.child_by_field_name("consequence").type != "identifier":
        raise ValueError("Module service description image selector is indirect")
    fallback = declaration(fallback_name)
    value = fallback.child_by_field_name("value")
    if (value.type != "binary_expression" or text(value.child_by_field_name("left")) != "n.p"
            or value.child_by_field_name("right").type != "string"):
        raise ValueError("Module service fallback is not the current public-path asset")
    media = json.loads(text(value.child_by_field_name("right")))
    if not media.startswith("static/media/") or Path(media).name != "installation_cover_placeholder.c5d354c7.avif":
        raise ValueError("Module service current fallback identity changed")
    row_declarations = [node for node in walk(row) if node.type == "variable_declarator"]
    state = one((node for node in row_declarations
        if node.child_by_field_name("name").type == "array_pattern"
        and node.child_by_field_name("name").named_children
        and text(node.child_by_field_name("name").named_children[0]) == failure_name), "image failure state")
    names = state.child_by_field_name("name").named_children
    initializer = state.child_by_field_name("value")
    if (len(names) != 2 or initializer.type != "call_expression"
            or text(initializer.child_by_field_name("function")) != "(0,i.useState)"
            or text(initializer.child_by_field_name("arguments")) != "(!1)"):
        raise ValueError("Module service image failure state initialization changed")
    setter = text(names[1])
    error_name = text(props.get("onError")) if props.get("onError") is not None else ""
    error = declaration(error_name, row_declarations)
    error_value = error.child_by_field_name("value")
    if error_value.type != "arrow_function" or text(error_value.child_by_field_name("body")) != setter + "(!0)":
        raise ValueError("Module service error callback no longer retains its failure")
    setter_calls = [node for node in walk(row) if node.type == "call_expression"
                    and text(node.child_by_field_name("function")) == setter]
    if len(setter_calls) != 1:
        raise ValueError("Module service image state has an unverified reset branch")
    mounted = image.parent
    while mounted is not None and mounted != row and mounted.type != "ternary_expression":
        mounted = mounted.parent
    if (mounted is None or mounted == row or text(mounted.child_by_field_name("alternative")) != "null"):
        raise ValueError("Module service description mount is unresolved")
    expanded_name = text(mounted.child_by_field_name("condition"))
    expanded = one((node for node in row_declarations
        if node.child_by_field_name("name").type == "array_pattern"
        and node.child_by_field_name("name").named_children
        and text(node.child_by_field_name("name").named_children[0]) == expanded_name), "expanded state")
    if text(expanded.child_by_field_name("value")) != "(0,i.useState)(!1)":
        raise ValueError("Module service description mount initialization changed")
    group = declaration("H")
    parameters = factory.child_by_field_name("parameters")
    if parameters is None or len(parameters.named_children) != 3:
        raise ValueError("Module service factory signature differs")
    exported, loader = (text(parameters.named_children[index]) for index in (1, 2))
    exports = []
    for node in walk(factory.child_by_field_name("body")):
        if node.type != "call_expression" or text(node.child_by_field_name("function")) != loader + ".d":
            continue
        arguments = node.child_by_field_name("arguments").named_children
        if len(arguments) != 2 or text(arguments[0]) != exported or arguments[1].type != "object":
            continue
        exports.extend(pair.child_by_field_name("value") for pair in arguments[1].named_children
                       if pair.type == "pair" and text(pair.child_by_field_name("key")) == "default")
    default = one(exports, "default export")
    if default.type != "arrow_function" or default.child_by_field_name("body").type != "identifier":
        raise ValueError("Module service default export is indirect")
    observer = declaration(text(default.child_by_field_name("body")))
    def device_projection(node):
        if node.type != "variable_declarator" or node.child_by_field_name("value") is None \
                or node.child_by_field_name("value").type != "arrow_function":
            return False
        assignments = [child for child in walk(node) if child.type == "assignment_expression"
            and child.child_by_field_name("left").type == "member_expression"
            and text(child.child_by_field_name("left").child_by_field_name("property")) == "srcImage"]
        if len(assignments) != 1:
            return False
        return re.fullmatch(r'`/synapse/products/\$\{([A-Za-z_$][\w$]*)\.productId\}/ui/\$\{\1\.productId\}_\$\{\1\.editionId\}/PluginImages/\$\{\1\.productId\}_\$\{\1\.editionId\}_\$\{\1\.layoutId\}_dashboard1x\.png`',
                            text(assignments[0].child_by_field_name("right"))) is not None
    projection = one((node for node in walk(observer) if node.type == "variable_declarator"
        and device_projection(node)), "observed device projection")
    projection_name = text(projection.child_by_field_name("name"))
    if not any(node.type == "call_expression" and text(node.child_by_field_name("function")) == projection_name
               for node in walk(observer)):
        raise ValueError("Module service device projection has no observer consumer")
    return {"fallback_source_path": "/synapse/dashboard/" + media,
        "detail_source": text(source.child_by_field_name("alternative")),
        "failure_lifetime": "Mounted w row; initialized false, set true only by onError, not reset by collapse or prop changes",
        "complete_factory": capture(factory_pair), "complete_row": capture(row),
        "fallback_declaration": capture(fallback), "failure_state": capture(state),
        "error_callback": capture(error), "expanded_state": capture(expanded),
        "mounted_description": capture(mounted), "complete_group": capture(group),
        "complete_observer": capture(observer), "complete_device_projection": capture(projection),
        "target_execution": False}


def prepare_module_service_artwork(*, check=False, manifest=None):
    """Maintain source-derived fixed artwork metadata for local observed rows."""
    import analysis
    import families
    import urllib.parse
    from importlib.metadata import version
    acquired_path = ROOT / ".analysis/inputs/frontend/synapse-dashboard-current/acquisition.json"
    acquired = json.loads(acquired_path.read_bytes())
    originals = {row["url"]: row for row in acquired["files"]}
    base = "https://apps.razer.com/synapse/dashboard/"
    inputs = [families.identity(acquired_path), families.identity(Path(__file__))]
    if not acquired.get("source_current_during_acquisition"):
        raise ValueError("Module service requires a stable current source acquisition")
    for stable in acquired.get("stability", []):
        original = originals.get(stable["url"])
        if (not original or not families.valid(original) or not stable.get("unchanged")
                or stable["before_sha256"] != original["sha256"] or stable["after_sha256"] != original["sha256"]):
            raise ValueError("Module service deployment stability does not verify")
        inputs.append({key: original[key] for key in ("path", "bytes", "sha256")})
    if len(acquired.get("stability", [])) < 2:
        raise ValueError("Module service deployment stability is incomplete")
    def original(url):
        row = originals.get(url)
        if not row or row.get("http_status") != 200 or not families.valid(row):
            raise ValueError("Module service original bytes do not verify: " + url)
        inputs.append({key: row[key] for key in ("path", "bytes", "sha256")})
        return row, (ROOT / row["path"]).read_bytes()
    html, html_raw = original(acquired["entry_url"])
    asset_manifest, asset_manifest_raw = original(base + "asset-manifest.json")
    declared = json.loads(asset_manifest_raw)
    references = analysis.References()
    references.feed(html_raw.decode("utf-8"))
    html_base = urllib.parse.urljoin(acquired["entry_url"], references.base or "")
    mounted = {analysis.artifact_url(html_base, literal) for literal, kind in references.refs
               if kind == "html:script:src"}
    declared_main = declared["files"]["main.js"]
    main_url = analysis.artifact_url(asset_manifest["url"], declared_main)
    if (main_url not in mounted or declared_main.removeprefix("./") not in declared["entrypoints"]):
        raise ValueError("Module service main is not mounted by the stable current HTML")
    main, main_raw = original(main_url)
    chunks = module_service_declared_artwork(declared)
    application, app_raw = original(base + chunks["app"])
    script, raw = original(base + chunks["js"])
    mounted_chain = module_service_deployment_projection(declared, main_raw, app_raw, raw)
    projection = module_service_artwork_projection(raw)
    fallback, _ = original("https://apps.razer.com" + projection["fallback_source_path"])
    css, css_raw = original(base + chunks["css"])
    selector = b".items .item-moreInfo.item-description .item-description-image"
    start = css_raw.find(selector + b"{")
    if start < 0 or css_raw.find(selector + b"{", start + 1) >= 0:
        raise ValueError("Module service image CSS selector is missing or ambiguous")
    end = css_raw.index(b"}", start) + 1
    rule = css_raw[start:end]
    fields = dict(part.split(b":", 1) for part in rule[len(selector) + 1:-1].split(b";") if part)
    if any(fields.get(key) != value for key, value in {
            b"width": b"288px", b"height": b"162px", b"margin-right": b"20px", b"object-fit": b"contain"}.items()):
        raise ValueError("Module service description geometry changed")
    expected = {"description_images": {projection["fallback_source_path"]: "synapse/installation-cover-placeholder.png"},
                "description_fallback": "synapse/installation-cover-placeholder.png"}
    registry_path = ROOT / "tools/typed-data.json"
    with analysis.index_lock(ROOT / ".work/typed-data-registry.lock"):
        registry = json.loads(registry_path.read_bytes())
        recipe = next(row for row in registry["tables"] if row["symbol"] == "MODULE_SERVICE_SOURCE")
        data_path = ROOT / recipe["source"]
        data = json.loads(data_path.read_bytes())
        if check:
            if any(data.get(key) != value for key, value in expected.items()) or digest(data_path) != recipe["source_sha256"]:
                raise ValueError("Module service artwork metadata differs from the complete current source")
        else:
            data.update(expected)
            # The database is published separately with its pinned resources.
            # analysis.save triggers the general catalog regenerator, whose
            # registry lock cannot be entered again inside this transaction.
            content = (json.dumps(data, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
            with tempfile.NamedTemporaryFile(dir=data_path.parent, suffix=".partial", delete=False) as staged:
                temporary = Path(staged.name)
                staged.write(content)
            try:
                os.replace(temporary, data_path)
            finally:
                temporary.unlink(missing_ok=True)
            recipe["source_sha256"] = digest(data_path)
            write_json(registry_path, registry)
    inputs.append(families.identity(data_path))
    if not all(families.valid(row) for row in inputs):
        raise ValueError("Module service source changed during artwork verification")
    report = {"original": script,
        "deployment": {"html": html, "asset_manifest": asset_manifest, "mounted_main": main,
                       "mounted_application": application, "mounted_chain": mounted_chain,
                       "declared_description_chunks_verified": True},
        "css": {"original": css, "byte_start": start, "byte_end": end,
        "body": rule.decode("utf-8"), "body_sha256": hashlib.sha256(rule).hexdigest()},
        "projection": projection, "fallback_original": fallback, "metadata": expected,
        "tool": {"python": sys.version.split()[0], "tree_sitter": version("tree-sitter"),
                 "tree_sitter_javascript": version("tree-sitter-javascript")},
        "inputs": inputs, "inputs_unchanged": True, "target_execution": False,
        "unresolved": ["Observed/cached local detail.srcImage remains a string boundary; no complete upstream producer domain is claimed"],
        "application_runtime_acceptance": "not performed"}
    if manifest is not None:
        import catalog_data
        import sqlite3
        from resource_database import OUTPUT
        document = catalog_data.Codec(ROOT / "crates" / recipe["crate"]).document(recipe, data)
        database = ROOT / OUTPUT
        with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as connection:
            stored = connection.execute("SELECT codec,sha256,payload FROM typed_catalog WHERE id=?", (document["id"],)).fetchone()
            if stored != (document["codec"], hashlib.sha256(document["raw"]).hexdigest(), document["raw"]):
                raise ValueError("Module service artwork fields differ from the installed catalog")
            positions = connection.execute("SELECT a.position,f.path,f.bytes,f.sha256 FROM resource_alias a JOIN resource_file f ON f.id=a.file_id WHERE a.key=? ORDER BY a.position", (expected["description_fallback"],)).fetchall()
        entries = [row for row in manifest["entries"] if row["key"] == expected["description_fallback"]]
        if not entries or len({row["path"] for row in entries}) != 1:
            raise ValueError("Module service fallback alias is missing or ambiguous")
        fixed = next(row for row in manifest["files"] if row["path"] == entries[0]["path"])
        if not positions or any(row[1:] != (fixed["path"], fixed["bytes"], fixed["sha256"]) for row in positions):
            raise ValueError("Module service fallback differs from its installed alias")
        report["installed_catalog"] = {"id": document["id"], "codec": document["codec"],
            "payload_sha256": hashlib.sha256(document["raw"]).hexdigest(), "complete_fields_verified": True,
            "fallback": fixed, "alias_positions": [row[0] for row in positions]}
        inputs.append(families.identity(database))
    if not all(families.valid(row) for row in inputs):
        raise ValueError("Module service source or catalog changed during artwork verification")
    if not check:
        result = ROOT / ".analysis/results/typed-data.json"
        previous = json.loads(result.read_bytes())
        previous["current_module_service_artwork"] = report
        write_json(result, previous)
        analysis.index_files([{**families.identity(data_path), "kind": "typed_table_input"}])
    return report


def audit_current_cycle_artwork(manifest):
    """Verify the current cycle JSX callers and exact prepared SVG identities."""
    import catalog_data
    import families
    import sqlite3
    import urllib.parse
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    parser = Parser(Language(tree_sitter_javascript.language()))
    recipes = json.loads((ROOT / "tools/typed-data.json").read_bytes())
    recipe = next(row for row in recipes["tables"] if row["macro"] == "mouse_customize_current_data")
    source = ROOT / recipe["source"]
    selectors = json.loads(source.read_bytes())
    document = catalog_data.Codec(ROOT / "crates" / recipe["crate"]).document(recipe, selectors)
    from resource_database import OUTPUT
    database = ROOT / OUTPUT
    inputs = [families.identity(source), families.identity(database),
        families.identity(ROOT / "tools/catalog_data.py"), families.identity(ROOT / "tools/resource_database.py"),
        families.identity(ROOT / recipe["module"])]
    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as connection:
        stored = connection.execute("SELECT codec,sha256,payload FROM typed_catalog WHERE id=?", (document["id"],)).fetchone()
        if stored != (document["codec"], hashlib.sha256(document["raw"]).hexdigest(), document["raw"]):
            raise ValueError("Current cycle selector fields differ from the installed SQLite typed catalog")
        aliases = connection.execute("SELECT a.position,a.key,f.path,f.bytes,f.sha256 FROM resource_alias a "
            "JOIN resource_file f ON f.id=a.file_id ORDER BY a.position").fetchall()
    _, selector_owner = rust_named_item_strings(recipe["module"], "cycle_artwork")
    _, render_owner = rust_named_item_strings(recipe["module"], "customize_current_surface")
    products = []
    for pid in (229, 239):
        selected = next(row for row in selectors if row["product_id"] == pid)
        if selected["cycle"]["fixed_edition"] != 0 or len(selected["cycle_images"]) != 1:
            raise ValueError("Current inline cycle selector must use its product's edition-independent producer")
        image = selected["cycle_images"][0]
        if image["edition"] != 0:
            raise ValueError("Current inline cycle selector edition differs")
        alias = image["asset"]
        entries = [row for row in manifest["entries"] if row["key"] == alias]
        if not entries or len({row["path"] for row in entries}) != 1:
            raise ValueError("Current inline cycle alias is missing or ambiguous")
        fixed = next(row for row in manifest["files"] if row["path"] == entries[0]["path"])
        installed_aliases = [row for row in aliases if row[1] == alias]
        if not installed_aliases or any(row[2:] != (fixed["path"], fixed["bytes"], fixed["sha256"]) for row in installed_aliases):
            raise ValueError("Current inline cycle installed alias differs from the pinned resource")
        receipt = ROOT / f".analysis/inputs/frontend/mouse-{pid}-current/acquisition.json"
        acquired = json.loads(receipt.read_bytes())
        files = {row["url"]: row for row in acquired["files"]}
        html = files[acquired["entry_url"]]
        config = files[urllib.parse.urljoin(html["url"], "manifest.json")]
        for original in (html, config):
            stable = next((row for row in acquired.get("stability", []) if row["url"] == original["url"]), {})
            if (not acquired.get("source_current_during_acquisition") or not families.valid(original)
                    or not stable.get("unchanged") or stable.get("before_sha256") != original["sha256"]
                    or stable.get("after_sha256") != original["sha256"]):
                raise ValueError(f"Current mouse {pid} cycle entry/manifest provenance does not verify")
        mounted = re.findall(r'<script\b[^>]*\bsrc=["\']([^"\']*\bmain\.[\w]+\.js)["\']', (ROOT / html["path"]).read_text(encoding="utf-8"))
        if len(mounted) != 1:
            raise ValueError("Current mouse cycle mounted main is ambiguous")
        main = files[urllib.parse.urljoin(html["url"], mounted[0])]
        if not families.valid(main):
            raise ValueError("Current mouse cycle main byte identity differs")
        inputs.extend([families.identity(receipt), *({key: row[key] for key in ("path", "bytes", "sha256")} for row in (html, config, main))])
        raw = (ROOT / main["path"]).read_bytes()
        tree = parser.parse(raw)
        if tree.root_node.has_error:
            raise ValueError("Current cycle original has a JavaScript AST error")
        def text(node):
            return raw[node.start_byte:node.end_byte].decode("utf-8")
        def walk(node):
            yield node
            for child in node.named_children:
                yield from walk(child)
        assignments, declarations = [], []
        for node in walk(tree.root_node):
            if node.type == "assignment_expression" and text(node.child_by_field_name("left")) == "this.getCycleImage":
                assignments.append(node)
            if node.type == "variable_declarator":
                declarations.append(node)
        if len(assignments) != 1:
            raise ValueError("Current cycle caller must have one complete assignment owner")
        caller = assignments[0]
        components = []
        for node in walk(caller):
            if node.type == "call_expression":
                arguments = node.child_by_field_name("arguments").named_children
                if len(arguments) == 2 and arguments[0].type == "identifier" and arguments[1].type == "object" and not arguments[1].named_children:
                    components.append(text(arguments[0]))
        if len(components) != 1:
            raise ValueError("Current cycle JSX component reference is ambiguous")
        wrappers = [node for node in declarations if text(node.child_by_field_name("name")) == components[0]
                    and node.child_by_field_name("value") is not None
                    and node.child_by_field_name("value").type == "call_expression"
                    and ".forwardRef" in text(node.child_by_field_name("value").child_by_field_name("function"))]
        if len(wrappers) != 1:
            raise ValueError("Current cycle forwardRef producer reference is unresolved")
        arguments = wrappers[0].child_by_field_name("value").child_by_field_name("arguments").named_children
        if len(arguments) != 1 or arguments[0].type != "identifier":
            raise ValueError("Current cycle forwardRef producer is indirect")
        name = text(arguments[0])
        producers = [node for node in declarations if text(node.child_by_field_name("name")) == name
                     and node.child_by_field_name("value") is not None
                     and node.child_by_field_name("value").type == "arrow_function"]
        if len(producers) != 1:
            raise ValueError("Current cycle complete SVG producer is ambiguous")
        producer = producers[0]
        anchor = raw[producer.start_byte:producer.child_by_field_name("value").child_by_field_name("body").start_byte + 1].decode("utf-8")
        svg, conversion = static_react_svg(raw.decode("utf-8"), anchor)
        if len(svg) != fixed["bytes"] or hashlib.sha256(svg).hexdigest() != fixed["sha256"]:
            raise ValueError("Current cycle SVG differs from the fixed resource payload")
        component = caller.parent
        while component is not None and component.type not in ("class_declaration", "class"):
            component = component.parent
        if component is None or "this.state.displayCycleButton?this.getCycleImage():null" not in text(component):
            raise ValueError("Current cycle conditional render chain is unresolved")
        def capture(node):
            body = raw[node.start_byte:node.end_byte]
            return {"byte_start": node.start_byte, "byte_end": node.end_byte,
                "body": body.decode("utf-8"), "body_sha256": hashlib.sha256(body).hexdigest()}
        products.append({"product_id": pid, "original": main, "alias": alias,
            "complete_component": capture(component), "get_cycle_image": capture(caller),
            "forward_ref": capture(wrappers[0]), "svg_producer": capture(producer),
            "transformation": conversion, "prepared_resource": fixed,
            "selector": {"cycle": selected["cycle"], "image": image,
                "catalog_id": document["id"], "codec": document["codec"],
                "payload_sha256": hashlib.sha256(document["raw"]).hexdigest(),
                "complete_catalog_fields_and_bytes_verified": True,
                "installed_alias_positions": [row[0] for row in installed_aliases],
                "rust_selector": selector_owner, "rust_render": render_owner},
            "current_source_selection_and_prepared_bytes_verified": True,
            "application_runtime_acceptance": "not performed"})
    return {"products": products, "inputs": inputs,
        "inputs_unchanged": all(families.valid(row) for row in inputs),
        "scope": "Current conditional render -> JSX reference -> forwardRef -> complete literal SVG producer -> pinned resource byte identity",
        "target_execution": False}


def audit_runtime_resources():
    """Audit the installed alias closure without requiring hydrated media."""
    import analysis
    import resource_database
    import resource_scope
    from importlib.metadata import version
    metadata_inputs = []
    manifest = resource_sources.runtime_metadata(input_rows=metadata_inputs)
    keys = {row["key"] for row in manifest["entries"]}
    paths = {row["path"] for row in manifest["files"]}
    registered = keys | paths
    files = sorted({*ROOT.glob("*.rs"), *(ROOT / "app").rglob("*.rs"),
                    *(ROOT / "crates").rglob("*.rs")})
    snapshots, literals, fixtures, patterns, missing = [], [], [], [], []
    for path in files:
        raw = path.read_bytes()
        snapshots.append({"path": relative(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        fixture = ("tests" in path.relative_to(ROOT).parts or "testdata" in path.relative_to(ROOT).parts
                   or path.stem.endswith(("_test", "_tests")) or path.stem == "tests")
        for row in rust_resource_literals(raw, owner=relative(path), test_only=fixture):
            if row["test_only"]:
                fixtures.append(row)
                continue
            value = row["literal"]
            candidates = keys if value.startswith("synapse/") else paths
            if "{" in value:
                expression = re.sub(r"\\\{[^}]*\\\}", ".*", re.escape(value))
                matches = sorted(key for key in candidates if re.fullmatch(expression, key))
                domain = finite_resource_domain(row)
                verified = domain is not None and domain["verified"]
                expected = domain["expected_aliases"] if domain is not None else []
                absent = [key for key in expected if key not in candidates]
                if absent:
                    missing.append({**row, "missing_aliases": absent})
                patterns.append({**row, "registered_candidates": matches, "finite_domain": domain,
                    "expected_missing": absent,
                    "finite_domain_verified": verified,
                    "remaining": None if verified else "Trace the caller and the complete typed product/state domain; wildcard matches do not establish coverage"})
            else:
                literals.append(row)
                if value not in candidates:
                    missing.append(row)
    recipes = json.loads((ROOT / "tools/typed-data.json").read_bytes())
    catalog_inputs = {row["source"] for row in recipes["tables"] if row.get("storage") in ("sqlite", "sqlite-profile")}
    catalog_inputs.update(resource_database.INPUTS.values())
    catalog_inputs.update((resource_database.PRODUCT_REGISTRY, resource_database.PRODUCT_HELP))
    catalog_inputs.update(row["path"] for row in metadata_inputs)
    documents, data_references = [], []
    def inspect(value, owner, pointer=""):
        if isinstance(value, str):
            if (value.startswith(("synapse/", "assets/"))
                    and re.search(r"\.(?:svg|png|jpe?g|webp|avif|gif|bmp|ico|mp4|woff2?|ttf)(?:#.*)?$", value)):
                row = {"owner": owner, "json_pointer": pointer, "literal": value}
                data_references.append(row)
                if value not in registered:
                    missing.append(row)
        elif isinstance(value, list):
            for index, child in enumerate(value):
                inspect(child, owner, pointer + "/" + str(index))
        elif isinstance(value, dict):
            for key, child in value.items():
                inspect(child, owner, pointer + "/" + key.replace("~", "~0").replace("/", "~1"))
    for name in sorted(catalog_inputs):
        path = safe(ROOT / name)
        raw = path.read_bytes()
        documents.append({"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        inspect(json.loads(raw), name)
    inputs = [*snapshots, *documents,
        *({"path": name, "bytes": (ROOT / name).stat().st_size, "sha256": digest(ROOT / name)}
          for name in (resource_sources.LOCK, resource_sources.MAPPINGS, "tools/typed-data.json",
                       "tools/resource_scope.py", "tools/workspace.py"))]
    report = {"schema_version": 1, "recorded_at_utc": analysis.stamp(),
        "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__)),
            "python": sys.version.split()[0], "tree_sitter": version("tree-sitter"),
            "tree_sitter_rust": version("tree-sitter-rust")},
        "resource_revision": manifest["storage"]["revision"], "inputs": inputs,
        "literal_references": literals, "catalog_references": data_references,
        "test_references": fixtures, "dynamic_references": patterns,
        "missing_references": missing,
        "summary": {"rust_files": len(snapshots), "catalog_inputs": len(documents),
            "production_literals": len(literals), "catalog_references": len(data_references),
            "dynamic_sites": len(patterns), "dynamic_patterns": len({r["literal"] for r in patterns}),
            "finite_dynamic_sites_verified": sum(row["finite_domain_verified"] for row in patterns),
            "test_references": len(fixtures), "missing_references": len(missing)},
        "literal_references_passed": not missing, "dynamic_domain_acceptance": "unverified",
        "target_execution": False, "real_device_operations": False,
        "application_runtime_acceptance": "not performed"}
    report["current_cycle_artwork"] = audit_current_cycle_artwork(manifest)
    inputs.extend(report["current_cycle_artwork"]["inputs"])
    report["current_module_service_artwork"] = prepare_module_service_artwork(check=True, manifest=manifest)
    inputs.extend(report["current_module_service_artwork"]["inputs"])
    report["resource_scope"] = resource_scope.audit_sources(ROOT, files)
    report["inputs_unchanged"] = all((ROOT / row["path"]).stat().st_size == row["bytes"]
        and digest(ROOT / row["path"]) == row["sha256"] for row in inputs)
    report["summary"]["overlay_edges"] = report["resource_scope"]["summary"]["edges"]
    report["summary"]["overlay_violations"] = report["resource_scope"]["summary"]["violations"]
    report["passed"] = (report["literal_references_passed"] and report["inputs_unchanged"]
                        and report["resource_scope"]["passed"])
    previous = json.loads(RESOURCE_AUDIT.read_bytes()) if RESOURCE_AUDIT.is_file() else {"schema_version": 1}
    previous["runtime_reference_closure"] = report
    write_json(RESOURCE_AUDIT, previous)
    return report


def verify_resources(*, scope="embedded", product_ids=None):
    """Decode the actual product image include closure without opening a window."""
    if scope == "keyboard-artwork":
        return verify_keyboard_artwork_resources(product_ids=product_ids)
    if scope != "embedded" or product_ids:
        raise ValueError("Product filters require the keyboard-artwork resource scope")
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
    if RESOURCE_AUDIT.is_file():
        previous = json.loads(RESOURCE_AUDIT.read_bytes())
        for section in ("runtime_reference_closure", "keyboard_artwork"):
            if section in previous:
                report[section] = previous[section]
    write_json(RESOURCE_AUDIT, report)
    return report


def keyboard_artwork_atlas(document, *, product_ids=None, additional_catalogs=None):
    """Enumerate every retained base and Customize image, without selecting a fallback."""
    products = document["products"]
    if not isinstance(products, list) or not products:
        raise ValueError("Keyboard artwork products are absent")
    known = [row["product_id"] for row in products]
    if any(type(pid) is not int or pid <= 0 for pid in known) or len(set(known)) != len(known):
        raise ValueError("Keyboard artwork products have invalid or repeated identities")
    additional = []
    additional_catalogs = additional_catalogs or {}
    def retain(image, pid, collection, **identity):
        asset = image["asset"]
        if (type(pid) is not int or pid <= 0 or type(asset) is not str or not asset.startswith("synapse/")
                or "\\" in asset or any(part in {"", ".", ".."} for part in asset.split("/"))
                or Path(asset).suffix not in {".svg", ".png"}):
            raise ValueError("Keyboard auxiliary artwork has an invalid typed identity")
        additional.append({"product_id": pid, "collection": collection, **identity,
                           "asset": asset, "svg": asset.endswith(".svg")})
    special = additional_catalogs.get("KEYBOARD_SOURCE_ARTWORK_DATA")
    if special is not None:
        for image in special["images"]:
            if image["role"] not in {"base", "wrist", "media", "dial"}:
                raise ValueError("Keyboard source artwork role is unknown")
            retain(image, special["product_id"], "source_artwork", **{key: value for key, value in image.items() if key != "asset"})
        retain(special["wrist"], special["product_id"], "source_artwork_wrist")
    for product in additional_catalogs.get("KEYBOARD_KEYPAD_ARTWORK_DATA", []):
        for collection in ("images", "icons"):
            for image in product[collection]:
                retain(image, product["product_id"], "keypad_" + collection,
                       **{key: value for key, value in image.items() if key != "asset"})
    for image in additional_catalogs.get("KEYBOARD_KEYPAD_DEVICE_IMAGE_DATA", []):
        retain(image, image["product_id"], "keypad_device_image", edition_id=image["edition_id"], density=image["density"])
    for product in additional_catalogs.get("KEYBOARD_MAPPING_BLADE_ARTWORK_DATA", []):
        for command, states in product["assets"].items():
            if "normal" not in states or not set(states) <= {"normal", "active"}:
                raise ValueError("Keyboard mapping artwork states are incomplete")
            for state, asset in states.items():
                retain({"asset": asset}, product["product_id"], "mapping_blade", command=command, state=state)
    all_known = set(known) | {row["product_id"] for row in additional}
    selected = set(all_known if product_ids is None else product_ids)
    if not selected or not selected <= all_known:
        raise ValueError("Keyboard artwork filter contains no typed product or an unknown product")
    aliases = {}
    for product in products:
        pid = product["product_id"]
        if pid not in selected:
            continue
        for kind in ("images", "customize_images"):
            images, identities = product[kind], set()
            if not isinstance(images, list):
                raise ValueError("Keyboard image collection is not a list")
            for image in images:
                required = {"edition_id", "layout_id", "resolution", "svg", "asset"}
                if (not required <= set(image) or not set(image) <= required | {"customize_asset", "source_layout_id"}
                        or type(image["edition_id"]) is not int or image["edition_id"] < 0
                        or type(image["layout_id"]) is not int or image["layout_id"] < 0
                        or type(image["resolution"]) is not int or image["resolution"] not in {1, 3}
                        or type(image["svg"]) is not bool or type(image["asset"]) is not str
                        or not image["asset"].startswith(f"synapse/keyboard-current/{pid}/")
                        or Path(image["asset"]).name != image["asset"].rsplit("/", 1)[-1]
                        or ".." in Path(image["asset"]).parts
                        or not image["asset"].endswith(".svg" if image["svg"] else ".png")
                        or "customize_asset" in image and (type(image["customize_asset"]) is not str
                            or not image["customize_asset"].startswith(f"synapse/keyboard-current/{pid}/"))
                        or "source_layout_id" in image and (type(image["source_layout_id"]) is not int
                            or image["source_layout_id"] < 0)):
                    raise ValueError("Keyboard image has an invalid typed identity")
                identity = tuple(image[key] for key in ("edition_id", "layout_id", "resolution", "svg"))
                if identity in identities:
                    raise ValueError("Keyboard image identity is repeated")
                identities.add(identity)
                aliases.setdefault(image["asset"], []).append({"product_id": pid, "collection": kind, **image})
    for use in additional:
        if use["product_id"] in selected:
            aliases.setdefault(use["asset"], []).append(use)
    if not aliases:
        raise ValueError("Selected keyboard products have no artwork images")
    return aliases


def keyboard_artwork_alias_identity(alias, mappings, locked_files):
    """Resolve one atlas alias to its unique delivered byte identity."""
    found = [row["path"] for table in mappings["tables"] for row in table["entries"] if row["key"] == alias]
    if not found:
        raise FileNotFoundError("Keyboard artwork alias has no delivery mapping: " + alias)
    if len(set(found)) != 1:
        raise ValueError("Keyboard artwork alias has conflicting delivery mappings: " + alias)
    found = [row for row in locked_files if row["path"] == found[0]]
    if not found:
        raise FileNotFoundError("Keyboard artwork alias has no pinned delivery identity: " + alias)
    if len(found) != 1:
        raise ValueError("Keyboard artwork delivery identity is repeated: " + alias)
    return found[0]


def image_analyzer_verification_inputs(*, svg=False):
    """Verify the isolated decoder dependency closure before importing native codecs."""
    destination = safe(ROOT / "tools/analyzers/pillow")
    receipt_path = destination / "acquisition.json"
    receipt_raw = receipt_path.read_bytes()
    receipt = json.loads(receipt_raw)
    needed = {"pillow": "12.3.0", **({"resvg-py": "0.5.0"} if svg else {})}
    wheels = {row["name"]: row for row in receipt["wheels"]}
    if len(wheels) != len(receipt["wheels"]):
        raise ValueError("Image analyzer acquisition repeats a package identity")
    if any(name not in wheels or wheels[name]["version"] != version for name, version in needed.items()):
        raise FileNotFoundError("Pinned image decoder is absent; run prepare-image-analyzer --svg")
    identities = [{"path": relative(receipt_path), "bytes": len(receipt_raw),
                   "sha256": hashlib.sha256(receipt_raw).hexdigest()}]
    for name in needed:
        wheel = wheels[name]
        for path, expected_size, expected_hash in (
                (wheel["path"], wheel["bytes"], wheel["sha256"]),
                (wheel["metadata_path"], None, wheel["metadata_sha256"])):
            current = safe(ROOT / path)
            raw = current.read_bytes()
            if hashlib.sha256(raw).hexdigest() != expected_hash or expected_size is not None and len(raw) != expected_size:
                raise ValueError("Pinned image analyzer acquisition changed: " + path)
            identities.append({"path": path, "bytes": len(raw), "sha256": expected_hash})
        metadata = json.loads(raw)
        if (metadata["info"]["name"].lower().replace("_", "-") != name
                or metadata["info"]["version"] != needed[name]):
            raise ValueError("Image analyzer metadata differs from the pinned package")
        artifacts = [row for row in metadata["urls"] if row["filename"] == Path(wheel["path"]).name
                     and row["packagetype"] == "bdist_wheel"]
        if (len(artifacts) != 1 or artifacts[0]["size"] != wheel["bytes"]
                or artifacts[0]["digests"]["sha256"] != wheel["sha256"] or artifacts[0]["url"] != wheel["url"]):
            raise ValueError("Image analyzer wheel differs from its published metadata")
        members = [row for row in receipt["files"] if row["wheel"] == wheel["path"]]
        if not members or len({row["path"] for row in members}) != len(members):
            raise ValueError("Image analyzer installation receipt is absent or repeated")
        import zipfile
        with zipfile.ZipFile(safe(ROOT / wheel["path"])) as archive:
            record = name.replace("-", "_") + "-" + wheel["version"] + ".dist-info/RECORD"
            expected_members = verified_wheel_members(archive, record)
            if {row["member"] for row in members} != set(expected_members):
                raise ValueError("Image analyzer receipt omits a wheel dependency")
            for member in members:
                path = safe(ROOT / member["path"])
                if (not path.is_relative_to(destination) or path.is_symlink()
                        or path != safe(destination / member["member"])):
                    raise ValueError("Image analyzer member escaped its isolated directory")
                raw = path.read_bytes()
                if (len(raw) != member["bytes"] or hashlib.sha256(raw).hexdigest() != member["sha256"]
                        or raw != archive.read(member["member"])):
                    raise ValueError("Image analyzer installed member differs from its verified wheel")
                identities.append({key: member[key] for key in ("path", "bytes", "sha256")})
    sys.path.insert(0, str(destination))
    import PIL
    if not Path(PIL.__file__).resolve().is_relative_to(destination) or PIL.__version__ != needed["pillow"]:
        raise ValueError("Raster decoder did not load from the pinned image analyzer")
    if svg:
        import resvg_py
        if not Path(resvg_py.__file__).resolve().is_relative_to(destination) or resvg_py.__version__ != needed["resvg-py"]:
            raise ValueError("SVG decoder did not load from the pinned image analyzer")
    return identities


def decoded_svg_rgba(content):
    """Rasterize SVG bytes with no external media, file fonts or system fonts."""
    root = ET.fromstring(content)
    if root.tag.split("}")[-1] != "svg":
        raise ValueError("Image root is not SVG")
    text = content.decode("utf-8")
    if re.search(r"<!DOCTYPE|<!ENTITY|<\?xml-stylesheet|@import|@font-face", text, re.IGNORECASE):
        raise ValueError("SVG external document/style/font dependencies are unverified")
    ids = {node.get("id") for node in root.iter() if node.get("id")}
    for node in root.iter():
        kind = node.tag.split("}")[-1]
        if kind in {"text", "tspan", "textPath", "foreignObject", "script"}:
            raise ValueError("SVG text, foreign content or executable dependencies are unverified")
        for name, value in node.attrib.items():
            if name.split("}")[-1] in {"href", "src"}:
                if value.startswith("#"):
                    if value[1:] not in ids:
                        raise ValueError("SVG fragment target is missing")
                elif not (kind == "image" and re.match(r"data:image/(?:png|jpeg|webp|gif|avif);base64,", value)):
                    raise ValueError("SVG external image/reference dependency is unverified")
    for reference in re.findall(r"url\(\s*['\"]?([^)'\"\s]+)", text, re.IGNORECASE):
        if not reference.startswith("#"):
            raise ValueError("SVG external CSS image/reference dependency is unverified")
        if reference[1:] not in ids:
            raise ValueError("SVG CSS fragment target is missing")
    sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
    import resvg_py
    encoded = resvg_py.svg_to_bytes(svg_string=text, skip_system_fonts=True, resources_dir=None,
                                   font_files=[], font_dirs=[], dpi=96.0)
    decoded, pixels = decoded_resource_rgba(encoded, "PNG")
    if decoded["width"] <= 0 or decoded["height"] <= 0:
        raise ValueError("Rasterized SVG has empty dimensions")
    visible = sum(alpha != 0 for alpha in pixels[3::4])
    if not visible:
        raise ValueError("Rasterized keyboard SVG has no visible pixels")
    return {**decoded, "source_format": "SVG", "renderer": "resvg-py", "version": resvg_py.__version__,
            "resvg_version": resvg_py.__resvg_version__, "visible_pixels": visible,
            "system_fonts": False, "external_resources": False}, pixels


def keyboard_artwork_acquisition(product_id, read_input):
    """Read the stable official acquisition graph without fetching missing inputs."""
    import urllib.parse
    path = safe(ROOT / f".analysis/inputs/frontend/keyboard-{product_id}-current/acquisition.json")
    document = json.loads(read_input(path))
    if document.get("source_current_during_acquisition") is not True or not document.get("stability") or any(
            row.get("unchanged") is not True for row in document["stability"]):
        raise ValueError("Keyboard current-source acquisition is unstable")
    expected_prefix = f"https://apps.razer.com/synapse/products/{product_id}/ui/"
    if document.get("entry_url") != expected_prefix + "index.html":
        raise ValueError("Keyboard acquisition does not have the current official product origin")
    files = {}
    for row in document["files"]:
        if row.get("path") and (row.get("http_status") == 200 or row.get("kind") == "installed_cached_official_resource"):
            if row["url"] in files and files[row["url"]] != row:
                raise ValueError("Keyboard acquisition has conflicting URL identities")
            files[row["url"]] = row
    roots = [document["entry_url"], expected_prefix + "asset-manifest.json"]
    if any(url not in files for url in roots):
        raise FileNotFoundError("Keyboard current entry or asset manifest was not acquired")
    entry = read_input(safe(ROOT / files[roots[0]]["path"]), files[roots[0]])
    manifest = json.loads(read_input(safe(ROOT / files[roots[1]]["path"]), files[roots[1]]))
    import analysis
    parser = analysis.References()
    parser.feed(entry.decode("utf-8"))
    graph = {}
    for edge in document.get("edges", []):
        if edge["from"] not in roots:
            graph.setdefault(edge["from"], set()).add(edge["to"])
    graph[roots[0]] = {urllib.parse.urljoin(roots[0], literal) for literal, _ in parser.refs}
    graph[roots[1]] = {urllib.parse.urljoin(roots[1], literal) for literal in manifest["files"].values()}
    pending, reachable = list(roots), set(roots)
    while pending:
        for url in graph.get(pending.pop(), ()):
            if url not in reachable:
                reachable.add(url)
                pending.append(url)
    return [row for url, row in files.items() if url in reachable]


def verified_official_http_revalidation(original, raw, read_input):
    """Verify both retained HTTP responses without changing the cache identity."""
    expected = {"bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
    attempts = original.get("current_http_revalidation", {}).get("attempts", [])
    if (len(attempts) < 2 or not official_resource_url(original.get("url")) or any(
            row.get("http_status") != 200 or row.get("status") != 200
            or row.get("url") != original.get("url")
            or row.get("final_url") != original.get("final_url", original.get("url"))
            or not official_resource_url(row.get("final_url"))
            or row.get("matches_retained") is not True
            or any(row.get(key) != value for key, value in expected.items()) for row in attempts)):
        raise ValueError("Cached original lacks matching official HTTP revalidation")
    identities = []
    for row in attempts:
        path = safe(ROOT / row["path"])
        if (not path.is_relative_to((ROOT / ".analysis/inputs").resolve()) or path.is_symlink()
                or relative(path) != row["path"] or read_input(path, row) != raw):
            raise ValueError("Official HTTP revalidation bytes differ from retained original")
        identities.append({key: row[key] for key in ("path", "bytes", "sha256")})
    return identities


def official_resource_url(value):
    """Recognize the official HTTPS source boundary shared by resource workflows."""
    parsed = urllib.parse.urlsplit(value or "")
    host = parsed.hostname or ""
    return (parsed.scheme == "https" and not parsed.username and not parsed.password
            and (host == "razer.com" or host.endswith(".razer.com")
                 or host == "razerzone.com" or host.endswith(".razerzone.com")))


def _frontend_revalidation_fetch(url):
    """Fetch one official deployment URL without writing a capture or cache."""
    if not official_resource_url(url):
        return {"url": url, "status": "rejected", "error": "non-official HTTPS URL"}
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "razer-ui-current-deployment-revalidation", "Accept-Encoding": "identity",
                 "Cache-Control": "no-cache"},
    )
    try:
        with urllib.request.urlopen(request, timeout=40) as response:
            raw = response.read()
            final_url = response.geturl()
            row = {
                "url": url,
                "status": "ok" if response.status == 200 else "http_error",
                "http_status": response.status,
                "final_url": final_url,
                "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest(),
                "acquired_at_utc": datetime.now(timezone.utc).isoformat(),
                "headers": {key: response.headers[key] for key in (
                    "Content-Type", "Content-Length", "ETag", "Last-Modified", "Date", "Age")
                    if key in response.headers},
            }
            length = response.headers.get("Content-Length")
            if length is not None and (not length.isdecimal() or int(length) != len(raw)):
                row.update(status="failed", error="HTTP Content-Length differs from response bytes")
            if not official_resource_url(final_url):
                row.update(status="failed", error="redirected outside official HTTPS origin")
            row["_raw"] = raw
            return row
    except urllib.error.HTTPError as error:
        row = {
            "url": url,
            "status": "missing" if error.code == 404 else "http_error",
            "http_status": error.code,
            "final_url": error.geturl(),
            "error": str(error),
            "acquired_at_utc": datetime.now(timezone.utc).isoformat(),
        }
        error.close()
        return row
    except Exception as error:
        return {"url": url, "status": "failed", "error": str(error),
                "acquired_at_utc": datetime.now(timezone.utc).isoformat()}


def _frontend_revalidation_originals(acquisition, deployment, owner_names):
    """Return bounded deployment URLs and retained identities from one receipt."""
    entry = acquisition.get("entry_url")
    if not official_resource_url(entry):
        raise ValueError("Frontend acquisition entry is outside the official HTTPS origin")
    urls = {entry: {"required": True, "roles": ["html_entry"]}}
    base = deployment.get("entry_response_url") or entry
    for name in ("asset-manifest.json", "manifest.json", "webpackManifest.json"):
        urls.setdefault(urllib.parse.urljoin(base, name), {"required": False, "roles": []})
    for source in deployment.get("sources", []):
        url = source.get("url")
        if (source.get("deployment_role") in {"html_include", "inline_equivalent_original"}
                and Path(urllib.parse.urlsplit(url or "").path).suffix.lower() in {".js", ".mjs"}):
            item = urls.setdefault(url, {"required": True, "roles": []})
            item["roles"].append(source.get("deployment_role"))
    referenced = {source.get("original", {}).get("path"): source for source in deployment.get("sources", [])
                  if source.get("identity_verified") and source.get("original")}
    for name in owner_names:
        if name in referenced:
            source = referenced[name]
            if Path(urllib.parse.urlsplit(source["url"]).path).suffix.lower() not in {".js", ".mjs"}:
                raise ValueError("Explicit deployed owner is not JavaScript: " + name)
            item = urls.setdefault(source["url"], {"required": True, "roles": []})
            item["roles"].append("explicit_deployed_owner")
    retained = {}
    for row in acquisition.get("files", []):
        url, path = row.get("url"), row.get("path")
        if not url or not path:
            continue
        if url not in urls:
            continue
        original = safe(ROOT / path)
        if (not original.is_relative_to((ROOT / ".analysis/inputs").resolve())
                or relative(original) != path or original.stat().st_size != row.get("bytes")
                or digest(original) != row.get("sha256")):
            raise ValueError("Retained deployment original byte identity differs: " + path)
        retained[url] = {key: row.get(key) for key in ("path", "bytes", "sha256", "final_url", "http_status")}
        urls[url]["required"] = True
    return urls, retained


def _publish_immutable_evidence(raw, *, directory, suffix="", created_paths=None):
    """Publish immutable bytes without replacing a racing writer's identity."""
    sha256 = hashlib.sha256(raw).hexdigest()
    candidate = ROOT / directory / (sha256 + suffix)
    if candidate.is_symlink():
        raise ValueError("Content-addressed evidence output is a symbolic link")
    destination = safe(candidate)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        if digest(destination) != sha256 or destination.read_bytes() != raw:
            raise ValueError("Content-addressed evidence output identity differs")
        return {"path": relative(destination), "bytes": len(raw), "sha256": sha256}
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=sha256 + ".", suffix=".tmp", delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        if digest(temporary) != sha256:
            raise ValueError("Evidence blob digest changed before publication")
        try:
            os.link(temporary, destination)
        except FileExistsError:
            if destination.is_symlink() or destination.read_bytes() != raw:
                raise ValueError("Content-addressed evidence publication raced different bytes")
        return {"path": relative(destination), "bytes": len(raw), "sha256": sha256}
    finally:
        if temporary is not None:
            try:
                if (created_paths is not None and temporary.exists() and destination.exists()
                        and os.path.samefile(temporary, destination)):
                    stat = destination.stat()
                    created_paths[relative(destination)] = {
                        "identity": {"path": relative(destination), "bytes": len(raw), "sha256": sha256},
                        "file_identity": (stat.st_dev, stat.st_ino),
                    }
            finally:
                temporary.unlink(missing_ok=True)


def _frontend_revalidation_publish(raw, *, receipt=False, input_receipt=False, created_paths=None):
    if receipt and input_receipt:
        raise ValueError("A revalidation publication has one receipt owner")
    directory = (".analysis/results/frontend-revalidation" if receipt else
                 ".analysis/inputs/frontend-revalidation/receipts" if input_receipt else
                 ".analysis/inputs/frontend-revalidation/blobs")
    return _publish_immutable_evidence(raw, directory=directory,
        suffix=".json" if receipt or input_receipt else "", created_paths=created_paths)


def _frontend_revalidation_cleanup_owned(created_paths):
    """Release only this observer's unpublished files after an index failure."""
    result = {"removed": [], "retained": [], "failures": [], "removed_logical_bytes": 0}
    if not created_paths:
        return result
    try:
        indexed = {row["path"]: row for row in json.loads((ROOT / ".analysis/inputs.json").read_bytes())["inputs"]}
    except (OSError, ValueError, KeyError) as error:
        result["failures"].append({"reason": "index_unavailable", "error": str(error)})
        result["retained"] = [{"path": name, "reason": "index_unavailable"} for name in created_paths]
        return result
    for name, owned in created_paths.items():
        identity = owned["identity"]
        if name in indexed:
            result["retained"].append({"path": name, "reason": "indexed"})
            continue
        candidate = ROOT / name
        try:
            if (not name.startswith((".analysis/inputs/frontend-revalidation/blobs/",
                                     ".analysis/inputs/frontend-revalidation/receipts/"))
                    or candidate.is_symlink() or relative(safe(candidate)) != name):
                raise ValueError("Owned revalidation cleanup boundary differs")
            if not candidate.exists():
                continue
            stat = candidate.stat()
            if ((stat.st_dev, stat.st_ino) != owned["file_identity"] or stat.st_size != identity["bytes"]
                    or digest(candidate) != identity["sha256"]):
                raise ValueError("Owned revalidation cleanup file identity differs")
            candidate.unlink()
            if candidate.exists():
                raise OSError("Owned revalidation cleanup did not remove the file")
            result["removed"].append(identity)
            result["removed_logical_bytes"] += identity["bytes"]
        except (OSError, ValueError) as error:
            result["failures"].append({"path": name, "error": str(error)})
    return result


def _frontend_revalidation_index(report, created_paths):
    """Append new HTTP originals through the verified source publication API."""
    import analysis
    original_rows, urls = {}, {}
    for observation in report["observations"]:
        original = observation.get("observed_original")
        if original:
            original_rows[original["path"]] = original
            urls.setdefault(original["path"], set()).add(observation["url"])
    if not original_rows:
        return {"status": "not_needed", "index_changed": False, "new_input_rows": 0}
    index_path = safe(ROOT / ".analysis/inputs.json")
    before_raw = index_path.read_bytes()
    before = {row["path"]: row for row in json.loads(before_raw)["inputs"]}
    before_identity = {"path": relative(index_path), "bytes": len(before_raw),
                       "sha256": hashlib.sha256(before_raw).hexdigest()}
    rows = []
    for name, identity in sorted(original_rows.items()):
        if name in before:
            if any(before[name].get(key) != identity[key] for key in ("bytes", "sha256")):
                raise ValueError("Indexed revalidation original identity differs: " + name)
            continue
        rows.append({**identity, "kind": "official_frontend_http_revalidation_observation", "urls": sorted(urls[name])})
    if not rows:
        return {"status": "already_indexed", "index_changed": False, "new_input_rows": 0,
                "all_observed_originals_indexed": True}
    publication = {
        "schema_version": 1, "kind": "immutable_frontend_revalidation_source_publication",
        "observation": report, "index_before": before_identity, "new_originals": rows,
        "publication_policy": "append content-addressed inputs; preserve existing entries and acquisition receipts",
        "source_semantics_complete": False, "target_execution": False, "runtime_acceptance": "unverified",
    }
    encoded = (json.dumps(publication, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")
    receipt = _frontend_revalidation_publish(encoded, input_receipt=True, created_paths=created_paths)
    rows.append({**receipt, "kind": "official_frontend_revalidation_publication_receipt"})
    analysis.index_files(rows, publication={
        "owner": "tools/workspace.py", "receipt": receipt["path"],
        "reason": "Append immutable official HTTP observations without replacing retained acquisition evidence",
        "verification": {"actual_original_bytes_verified": True, "originals": len(original_rows),
                         "new_input_rows": len(rows), "strict_deployment_match": report["passed"],
                         "target_execution": False},
    })
    after_raw = index_path.read_bytes()
    after = {row["path"]: row for row in json.loads(after_raw)["inputs"]}
    preserved = all(after.get(name) == row for name, row in before.items())
    published = all(after.get(row["path"]) == row for row in rows)
    if not preserved or not published:
        raise ValueError("Revalidation index publication changed old entries or omitted new identities")
    return {"status": "published", "index_changed": True, "index_before": before_identity,
            "index_after": {"path": relative(index_path), "bytes": len(after_raw),
                            "sha256": hashlib.sha256(after_raw).hexdigest()},
            "publication_receipt": receipt, "new_input_rows": len(rows),
            "previous_entries_unchanged": preserved, "all_observed_originals_indexed": published}


def _current_frontend_selection():
    """Resolve every retained official route without reading derived captures."""
    import frontend
    import host

    directory = ROOT / ".analysis/inputs/frontend"
    paths = sorted(directory.glob("*/acquisition.json"))
    routes, artifacts, excluded, failures, selected = {}, {}, [], [], []
    for path in paths:
        name = relative(path)
        raw = safe(path).read_bytes()
        artifacts[name] = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        try:
            acquired = json.loads(raw)
            entry = acquired.get("entry_url")
            if entry is None:
                excluded.append({"acquisition": artifacts[name], "reason": "no_official_deployment_route"})
                continue
            route = frontend.canonical_frontend_entry(entry)
        except (ValueError, TypeError, AttributeError) as error:
            failures.append({"kind": "current_source_selection_failed", "acquisition": name,
                             "error": str(error)})
            continue
        routes.setdefault(route, []).append(path)
    for route, candidates in sorted(routes.items()):
        audit = host.HostLocalAudit()
        try:
            result = frontend.current_source_selection(route, audit, candidates=candidates, verify_capture=False)
            selected.append({key: result[key] for key in (
                "name", "canonical_entry_url", "acquisition", "candidate_decisions")})
        except (ValueError, OSError, KeyError, TypeError) as error:
            failures.append({"kind": "current_source_selection_failed", "route": route,
                             "candidates": [relative(path) for path in candidates], "error": str(error)})
    return {"kind": "all_retained_official_deployment_routes", "candidate_count": len(paths),
            "route_count": len(routes), "selected": selected, "excluded": excluded, "failures": failures,
            "candidate_artifacts": list(artifacts.values()), "capture_verification": "not_requested",
            "source_semantics_complete": False}


def revalidate_current_frontends(acquisition_names=(), *, owner_names=(), workers=6, all_current=False):
    import analysis
    # The lease includes rollback, so one observer cannot delete a blob reserved
    # by another revalidation between its byte publication and index append.
    with analysis.index_lock(ROOT / ".analysis/frontend-revalidation.lock"):
        if all_current and acquisition_names:
            raise ValueError("Select all current routes or explicit acquisitions, not both")
        selection = _current_frontend_selection() if all_current else None
        if selection is not None:
            acquisition_names = [row["acquisition"]["path"] for row in selection["selected"]]
        return _revalidate_current_frontends(acquisition_names, owner_names=owner_names, workers=workers,
                                            selection=selection)


def _revalidate_current_frontends(acquisition_names, *, owner_names, workers, selection=None):
    """Revalidate retained current deployments without mutating acquisition inputs."""
    if not acquisition_names:
        raise ValueError("At least one retained frontend acquisition is required")
    if type(workers) is not int or not 1 <= workers <= 12:
        raise ValueError("Revalidation workers must be between one and twelve")
    import evidence
    import frontend

    jobs, sources, protected = [], [], {}

    def protect(path, expected=None):
        name = relative(path)
        identity = {"path": name, "bytes": path.stat().st_size, "sha256": digest(path)} if path.is_file() else {
            "path": name, "absent": True}
        protected[name] = identity
        if expected is not None and any(identity.get(key) != expected[key] for key in ("bytes", "sha256")):
            raise ValueError("Protected frontend input changed during verification: " + name)
        return identity

    protect(safe(ROOT / ".analysis/inputs.json"))
    if selection is not None:
        for artifact in selection["candidate_artifacts"]:
            protect(safe(ROOT / artifact["path"]), artifact)
    requested_owners = {relative(safe(ROOT / name)) for name in owner_names}
    found_owners = set()
    for name in dict.fromkeys(acquisition_names):
        acquisition_path = safe(ROOT / name)
        if (acquisition_path.name != "acquisition.json"
                or acquisition_path.parent.parent != (ROOT / ".analysis/inputs/frontend").resolve()
                or relative(acquisition_path) != name):
            raise ValueError("Revalidation requires a project-relative retained frontend acquisition")
        raw = acquisition_path.read_bytes()
        acquisition = json.loads(raw)
        route = frontend.canonical_frontend_entry(acquisition.get("entry_url"))
        if acquisition.get("target_execution") is not False:
            raise ValueError("Frontend acquisition does not establish static-only evidence")
        deployment = evidence.frontend_deployment_sources(acquisition)
        if deployment.get("failures"):
            raise ValueError(f"Current deployment {name} failed cached verification: "
                             + json.dumps(deployment["failures"], sort_keys=True))
        urls, retained = _frontend_revalidation_originals(acquisition, deployment, requested_owners)
        source_name = acquisition_path.parent.name
        sources.append({"name": source_name, "route": route,
                        "acquisition": protect(acquisition_path, {
                            "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}),
                        "baseline_recorded_at_utc": acquisition.get("recorded_at_utc")})
        for url, metadata in urls.items():
            original = retained.get(url)
            if original:
                protect(safe(ROOT / original["path"]), original)
                if "explicit_deployed_owner" in metadata["roles"]:
                    found_owners.add(original["path"])
            jobs.append({"source_set": source_name, "url": url, **metadata, "retained": original})
    if found_owners != requested_owners:
        raise ValueError("Explicit owners lack verified current deployment references: "
                         + json.dumps(sorted(requested_owners - found_owners)))

    def fetch_round(round_number):
        results = {}
        pool = concurrent.futures.ThreadPoolExecutor(max_workers=workers)
        try:
            pending = {pool.submit(_frontend_revalidation_fetch, url): url for url in sorted({job["url"] for job in jobs})}
            for future in concurrent.futures.as_completed(pending):
                url = pending[future]
                results[url] = {**future.result(), "round": round_number}
        finally:
            pool.shutdown(wait=True, cancel_futures=True)
        return results

    first, second, interrupted = {}, {}, None
    try:
        first = fetch_round(1)
        second = fetch_round(2)
    except BaseException as error:
        interrupted = error
    observations, failures, created_paths = [], list(selection["failures"]) if selection else [], {}
    if interrupted is not None:
        failures.append({"kind": "cancelled" if isinstance(interrupted, KeyboardInterrupt) else "fetch_failed",
                         "error_type": type(interrupted).__name__})
    for job in jobs:
        left, right = first.get(job["url"], {}), second.get(job["url"], {})
        source = {
            "source_set": job["source_set"],
            "url": job["url"],
            "required": job["required"],
            "roles": sorted(set(job["roles"])),
            "retained_original": job["retained"],
            "rounds": [{key: value for key, value in row.items() if key != "_raw"} for row in (left, right)],
        }
        stable = (left.get("status") == right.get("status") == "ok"
                  and left.get("http_status") == right.get("http_status") == 200
                  and left.get("final_url") == right.get("final_url")
                  and left.get("bytes") == right.get("bytes")
                  and left.get("sha256") == right.get("sha256"))
        missing = (left.get("status") == right.get("status") == "missing"
                   and left.get("final_url") == right.get("final_url")
                   and official_resource_url(left.get("final_url")))
        if missing and not job["required"]:
            source["classification"] = "optional_missing"
        elif stable:
            raw = right.get("_raw", b"")
            retained = job["retained"]
            if (retained and retained.get("bytes") == right.get("bytes")
                    and retained.get("sha256") == right.get("sha256")):
                source["classification"] = (
                    "unchanged_reused" if (retained.get("final_url") or job["url"]) == right.get("final_url")
                    else "changed_deployment_route")
                source["reused_path"] = retained.get("path")
            else:
                source["classification"] = "changed_or_new_deployment"
                if interrupted is None:
                    try:
                        source["observed_original"] = _frontend_revalidation_publish(raw, created_paths=created_paths)
                    except BaseException as error:
                        interrupted = error
                        source["classification"] = "publication_failed"
                else:
                    source["classification"] = "publication_not_completed"
        elif missing:
            source["classification"] = "required_missing" if job["required"] else "optional_missing"
        else:
            source["classification"] = "unstable_or_failed"
        if source["classification"] not in {"unchanged_reused", "optional_missing"}:
            failures.append({"source_set": job["source_set"], "url": job["url"],
                             "classification": source["classification"]})
        observations.append(source)

    input_changes = []
    for name, identity in protected.items():
        path = safe(ROOT / name)
        if identity.get("absent"):
            changed = path.exists()
        else:
            changed = not path.is_file() or path.stat().st_size != identity["bytes"] or digest(path) != identity["sha256"]
        if changed:
            input_changes.append(name)
    failures.extend({"kind": "protected_input_changed", "path": name} for name in input_changes)
    if selection is not None:
        candidates = sorted(relative(path) for path in (ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json"))
        if candidates != sorted(row["path"] for row in selection["candidate_artifacts"]):
            failures.append({"kind": "current_source_candidate_set_changed"})

    report = {
        "schema_version": 1,
        "kind": "immutable_current_frontend_deployment_revalidation",
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_sets": sources,
        "observations": observations,
        "protected_inputs": list(protected.values()),
        "failures": failures,
        "passed": not failures,
        "target_execution": False,
        "inputs_unchanged": not input_changes,
        "protected_originals_unchanged": not any(name != ".analysis/inputs.json" for name in input_changes),
        "source_semantics_complete": False,
        "runtime_acceptance": "unverified",
        "scope": "Two-round HTTP identity of retained deployment roots, direct JavaScript owners and explicit deployed owners; changed deployments are not promoted",
        "cleanup": {"http_temporary_files": 0, "atomic_publication_temporaries": "removed in finally"},
    }
    if selection is not None:
        report["current_source_selection"] = selection
    try:
        report["index_publication"] = _frontend_revalidation_index(report, created_paths)
    except BaseException as error:
        interrupted = interrupted or error
        report["index_publication"] = {"status": "failed", "error_type": type(error).__name__, "error": str(error)}
        report["failures"].append({"kind": "index_publication_failed", "error_type": type(error).__name__})
    finally:
        cleanup = _frontend_revalidation_cleanup_owned(created_paths)
        report["cleanup"]["owned_publication_files"] = cleanup
        report["failures"].extend({"kind": "publication_cleanup_failed", **failure} for failure in cleanup["failures"])
    removed = {row["path"] for row in cleanup["removed"]}
    for observation in observations:
        if observation.get("observed_original", {}).get("path") in removed:
            observation["retention"] = "removed_after_index_publication_failure"
    index_identity = protected[".analysis/inputs.json"]
    index_path = safe(ROOT / ".analysis/inputs.json")
    report["index_changed"] = index_path.exists() if index_identity.get("absent") else (
        not index_path.is_file() or digest(index_path) != index_identity["sha256"])
    report["inputs_unchanged"] = report["inputs_unchanged"] and not report["index_changed"]
    report["passed"] = not report["failures"]
    encoded = (json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")
    report["receipt"] = _frontend_revalidation_publish(encoded, receipt=True)
    if interrupted is not None:
        print(json.dumps({"interrupted_revalidation_receipt": report["receipt"]}), flush=True)
        raise interrupted
    return report


class FrontendRefreshResources:
    """Own bounded HTTP, static parsing and source publication for one batch."""

    def __init__(self, observation, identity, *, network_workers, parse_workers):
        import analysis
        self.identity = identity
        self.cancelled = threading.Event()
        self.network = threading.BoundedSemaphore(network_workers)
        self.parsers = threading.BoundedSemaphore(parse_workers)
        self.lock = threading.Lock()
        self.downloads, self.enumerations, self.pending_index = {}, {}, {}
        self.observed = {}
        for row in observation["observations"]:
            rounds = row.get("rounds", [])
            original = row.get("observed_original")
            if original is None and row.get("classification") == "unchanged_reused":
                original = row.get("retained_original")
            if (not isinstance(original, dict) or len(rounds) != 2
                    or [item.get("round") for item in rounds] != [1, 2]
                    or any(item.get("status") != "ok" or item.get("http_status") != 200
                           or item.get("url") != row["url"] or not official_resource_url(item.get("final_url"))
                           or any(item.get(key) != original.get(key) for key in ("bytes", "sha256"))
                           for item in rounds)
                    or rounds[0].get("final_url") != rounds[1].get("final_url")):
                continue
            last = rounds[-1]
            source = {**original, "url": row["url"], "final_url": last["final_url"], "http_status": 200,
                      "content_type": last.get("headers", {}).get("Content-Type", ""),
                      "headers": last.get("headers", {}), "acquired_at_utc": last["acquired_at_utc"],
                      "reused_from_observation": identity}
            prior = self.observed.setdefault(row["url"], source)
            if any(prior.get(key) != source.get(key) for key in ("bytes", "sha256", "final_url")):
                raise ValueError("Batch observation conflicts for one official URL")

    def checkpoint(self):
        if self.cancelled.is_set():
            raise InterruptedError("Frontend refresh was cancelled; retained partial evidence is resumable")

    def _once(self, collection, key, operation):
        self.checkpoint()
        with self.lock:
            future = collection.get(key)
            first = future is None
            if first:
                future = concurrent.futures.Future()
                collection[key] = future
        if first:
            try:
                future.set_result(operation())
            except BaseException as error:
                future.set_exception(error)
        return future.result()

    def download(self, url, output):
        import analysis
        def acquire():
            self.checkpoint()
            observed = self.observed.get(url)
            if observed is not None:
                analysis.verify_source_rows([observed])
                return observed
            attempts = []
            with self.network:
                for _ in range(3):
                    self.checkpoint()
                    result = analysis.download(url, output)
                    attempts.append({key: value for key, value in result.items() if key != "path"})
                    if ("path" in result or result.get("http_status") == 404
                            or result.get("error", "").startswith("HTTP Error 404:")):
                        break
            if len(attempts) > 1:
                result = {**result, "retrieval_attempts": attempts}
            return result
        return dict(self._once(self.downloads, url, acquire))

    def revalidate(self, url):
        self.checkpoint()
        with self.network:
            self.checkpoint()
            result = _frontend_revalidation_fetch(url)
        result.pop("_raw", None)
        return result

    def enumerate(self, path, tool, report_dir, identifier, tool_identity):
        import analysis
        key = (relative(path), digest(path), tool_identity["sha256"])
        def parse():
            directories = []
            boundary = ROOT / ".analysis/inputs/frontend"
            if path.is_relative_to(boundary):
                owner = path.relative_to(boundary).parts[0]
                directories.append(ROOT / ".analysis/results/frontend" / owner / "chunk-references")
            with self.parsers:
                self.checkpoint()
                return analysis.enumerate_frontend_chunks(path, tool, report_dir, identifier, tool_identity,
                                                          reuse_directories=directories)
        return self._once(self.enumerations, key, parse)

    def index(self, rows):
        with self.lock:
            for row in rows:
                previous = self.pending_index.setdefault(row["path"], row)
                if any(previous.get(key) != row.get(key) for key in ("bytes", "sha256")):
                    raise ValueError("Batch input publication has conflicting identities")

    def flush(self):
        import analysis
        with self.lock:
            rows = list(self.pending_index.values())
        if not rows:
            return None
        analysis.verify_source_rows(rows)
        publication = {"schema_version": 1, "kind": "frontend_refresh_source_publication",
                       "recorded_at_utc": analysis.stamp(), "observation": self.identity,
                       "inputs": rows, "target_execution": False, "source_semantics_complete": False}
        encoded = (json.dumps(publication, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")
        proof = _publish_immutable_evidence(encoded, directory=".analysis/inputs/frontend-refresh/receipts", suffix=".json")
        analysis.index_files([*rows, {**proof, "kind": "frontend_refresh_publication_receipt"}],
            preserve_existing=True, publication={"owner": "tools/workspace.py", "receipt": proof["path"],
                "reason": "Publish batch-acquired official originals while preserving prior source metadata",
                "verification": {"actual_bytes_verified": True, "inputs": len(rows),
                                 "target_execution": False, "source_semantics_complete": False}})
        with self.lock:
            for row in rows:
                if self.pending_index.get(row["path"]) == row:
                    del self.pending_index[row["path"]]
        return proof


def frontend_refresh_plan(observation_path, *, priority_sources=(), source_names=()):
    """Derive the complete refresh set from a retained immutable HTTP audit."""
    import analysis
    path = analysis.resolve(str(observation_path), ".analysis/results/frontend-revalidation")
    raw = path.read_bytes()
    identity = {"path": relative(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
    if path.stem != identity["sha256"] or path.is_symlink():
        raise ValueError("Frontend refresh requires an unchanged content-addressed observation receipt")
    observed = json.loads(raw)
    if (observed.get("schema_version") != 1
            or observed.get("kind") != "immutable_current_frontend_deployment_revalidation"
            or observed.get("target_execution") is not False
            or observed.get("protected_originals_unchanged") is not True):
        raise ValueError("Frontend refresh observation has an unverified scope or source identity")
    changed = {}
    for row in observed["observations"]:
        if row.get("classification") not in {"unchanged_reused", "optional_missing"}:
            changed.setdefault(row["source_set"], []).append({"url": row["url"], "classification": row["classification"]})
    jobs = []
    for source in observed["source_sets"]:
        if source["name"] in changed:
            jobs.append({**source, "reasons": changed[source["name"]], "status": "pending"})
    for failure in observed.get("current_source_selection", {}).get("failures", []):
        candidates = failure.get("candidates", [])
        if not failure.get("route") or not candidates:
            raise ValueError("Frontend refresh cannot resolve an unselected route without its retained acquisition")
        path = analysis.resolve(candidates[-1], ".analysis/inputs/frontend")
        jobs.append({"name": path.parent.name, "route": failure["route"],
                     "acquisition": {"path": relative(path), "bytes": path.stat().st_size, "sha256": digest(path)},
                     "reasons": [failure], "status": "pending"})
    selected_names = {job["name"] for job in jobs}
    if len(selected_names) != len(jobs) or set(source_names) - selected_names or set(priority_sources) - selected_names:
        raise ValueError("Frontend refresh source selection is repeated or absent from the observation")
    priority = {name: index for index, name in enumerate(dict.fromkeys(priority_sources))}
    jobs = [job for job in jobs if not source_names or job["name"] in source_names]
    jobs.sort(key=lambda job: (priority.get(job["name"], len(priority)), job["route"]))
    return {"observation": identity, "observed": observed, "jobs": jobs}


def frontend_refresh_manifest_roots(acquired):
    """Retain caller-declared static roots and host-supplied product manifests."""
    import analysis
    entry = acquired["entry_url"]
    roots = {edge["to"] for edge in acquired.get("edges", [])
             if edge.get("kind") == "explicit-retained-manifest"}
    if re.fullmatch(r"/synapse/products/[0-9]+/mw/(?:index\.html)?", urllib.parse.urlsplit(entry).path):
        roots.add(urllib.parse.urljoin(entry, "manifest.json"))
    for url in roots:
        if (analysis.artifact_url(entry, url) != url
                or urllib.parse.urlsplit(url).netloc != urllib.parse.urlsplit(entry).netloc):
            raise ValueError("Frontend refresh manifest root differs from its official source origin")
    return sorted(roots)


def _frontend_refresh_result(path, resource, required_manifests=()):
    import analysis
    import evidence
    import families
    acquired = json.loads(path.read_bytes())
    if acquired.get("target_execution") is not False:
        raise ValueError("Frontend refresh snapshot does not establish static-only acquisition")
    originals = [row for row in acquired["files"] if row.get("path")]
    analysis.verify_source_rows(originals)
    deployment = evidence.frontend_deployment_sources(acquired)
    gaps = families.frontend_reference_gaps(acquired)
    failures = list(deployment["failures"])
    files = {row["url"]: row for row in originals}
    stability = {row["url"]: row for row in acquired.get("stability", [])}
    for url in required_manifests:
        original, checked = files.get(url), stability.get(url)
        if original is None:
            failures.append({"kind": "required_manifest_root_missing", "url": url})
        elif (checked is None or checked.get("unchanged") is not True
                or checked.get("before_sha256") != original["sha256"]
                or checked.get("after_sha256") != original["sha256"]):
            failures.append({"kind": "required_manifest_stability_unverified", "url": url})
    for kind in ("declared_non_map_missing", "unattributed_non_map_missing"):
        failures.extend({"kind": kind, "reference": row} for row in gaps.get(kind, []))
    if acquired.get("source_current_during_acquisition") is not True:
        failures.append({"kind": "deployment_not_stable", "stability": acquired.get("stability", [])})
    identity = {"path": relative(path), "bytes": path.stat().st_size, "sha256": digest(path)}
    indexed = [{**{key: row[key] for key in ("path", "bytes", "sha256", "url", "acquired_at_utc")},
                "kind": "official_frontend_artifact"} for row in originals]
    resource.index([*indexed, {**identity, "kind": "acquisition_receipt"}])
    return {"status": "acquired" if not failures else "failed", "refreshed_acquisition": identity,
            "files": len(originals), "logical_bytes": sum(row["bytes"] for row in originals),
            "reference_gaps": gaps, "unresolved": len(acquired.get("unresolved", [])),
            "failures": failures, "source_semantics_complete": False, "runtime_acceptance": "unverified"}


def refresh_current_frontends(observation_path, *, workers=4, network_workers=12, parse_workers=4,
                              priority_sources=(), source_names=(), reuse_acquisitions=()):
    """Acquire changed routes in batches, retaining stable snapshots and retry history."""
    import analysis
    import frontend
    from types import SimpleNamespace
    for name, value, maximum in (("route", workers, 8), ("network", network_workers, 12), ("parse", parse_workers, 8)):
        if type(value) is not int or not 1 <= value <= maximum:
            raise ValueError(f"Frontend refresh {name} workers must be between one and {maximum}")
    plan = frontend_refresh_plan(observation_path, priority_sources=priority_sources, source_names=source_names)
    identity, observed, jobs = plan["observation"], plan["observed"], plan["jobs"]
    selection = ("-" + hashlib.sha256(json.dumps(sorted(source_names)).encode()).hexdigest()[:12]
                 if source_names else "")
    report_path = ROOT / ".analysis/results/frontend-refresh" / (identity["sha256"] + selection + ".json")
    with analysis.index_lock(ROOT / ".analysis/frontend-refresh.lock"):
        resource = FrontendRefreshResources(observed, identity, network_workers=network_workers, parse_workers=parse_workers)
        report = {"schema_version": 1, "kind": "current_frontend_batch_refresh", "observation": identity,
                  "started_at_utc": analysis.stamp(), "jobs": jobs, "target_execution": False,
                  "source_semantics_complete": False, "runtime_acceptance": "unverified", "publications": [],
                  "concurrency": {"routes": workers, "network": network_workers, "static_parsers": parse_workers},
                  "cleanup": {"atomic_publication_temporaries": "removed in finally",
                              "partial_acquisitions": "retained for verified resumption",
                              "worker_threads": "owned until executor join"}}
        newer = {}
        for path in sorted((ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")):
            acquired = json.loads(path.read_bytes())
            if acquired.get("source_current_during_acquisition") is not True:
                continue
            if datetime.fromisoformat(acquired["recorded_at_utc"]) <= datetime.fromisoformat(observed["created_at_utc"]):
                continue
            route = frontend.canonical_frontend_entry(acquired["entry_url"])
            newer.setdefault(route, []).append((datetime.fromisoformat(acquired["recorded_at_utc"]), path))
        prior = json.loads(report_path.read_bytes()) if report_path.is_file() else None
        if prior is not None:
            if prior.get("observation") != identity:
                raise ValueError("Refresh progress belongs to different observation bytes")
            previous = {job["name"]: job for job in prior["jobs"]}
            report["publications"] = prior.get("publications", [])
            for job in jobs:
                job["prior_attempts"] = previous.get(job["name"], {}).get("attempts", [])
        write_json(report_path, report)

        def refresh(job):
            resource.checkpoint()
            analysis.verify_source_rows([job["acquisition"]])
            baseline = json.loads((ROOT / job["acquisition"]["path"]).read_bytes())
            required_manifests = frontend_refresh_manifest_roots(baseline)
            reuse = [*reuse_acquisitions]
            candidates = newer.get(job["route"], [])
            if candidates:
                candidate = max(candidates)[1]
                result = _frontend_refresh_result(candidate, resource, required_manifests)
                if result["status"] == "acquired":
                    return {**result, "reused_newer_acquisition": True}
                reuse.append(relative(candidate))
            if baseline.get("source_current_during_acquisition") is True:
                reuse.append(job["acquisition"]["path"])
            attempts = []
            stem = job["name"] + "-refresh-" + identity["sha256"][:12]
            number = 1
            while True:
                output = ROOT / ".analysis/inputs/frontend" / (stem if number == 1 else stem + "-" + str(number))
                receipt = output / "acquisition.json"
                if not receipt.exists():
                    break
                previous = json.loads(receipt.read_bytes())
                if previous.get("refresh_observation") != identity:
                    raise ValueError("Refresh snapshot belongs to a different observation")
                if "source_current_during_acquisition" not in previous:
                    break
                result = _frontend_refresh_result(receipt, resource, required_manifests)
                attempts.append(result)
                if result["status"] == "acquired":
                    return {**result, "attempts": attempts, "resumed_verified_snapshot": True}
                if previous.get("source_current_during_acquisition") is True:
                    reuse.append(relative(receipt))
                number += 1
            analysis.fetch_frontend(SimpleNamespace(url=baseline["entry_url"], output=relative(output),
                reuse_acquisition=list(dict.fromkeys(reuse)), seed_urls=required_manifests,
                no_refresh_gaps=True, refresh_observation=identity,
                download_artifact=resource.download, revalidate_artifact=resource.revalidate,
                enumerate_artifact=resource.enumerate, index_artifacts=resource.index,
                checkpoint=resource.checkpoint, progress=lambda _: None), refresh_ledger=False)
            result = _frontend_refresh_result(receipt, resource, required_manifests)
            attempts.append(result)
            return {**result, "attempts": attempts}

        completed = 0
        interrupted = None
        pool = concurrent.futures.ThreadPoolExecutor(max_workers=workers)
        futures = {pool.submit(refresh, job): job for job in jobs}
        try:
            for future in concurrent.futures.as_completed(futures):
                job = futures[future]
                try:
                    job.update(future.result())
                except Exception as error:
                    job.update(status="failed", failures=[{"kind": type(error).__name__, "error": str(error)}])
                completed += 1
                if completed <= len(priority_sources) or completed % 8 == 0:
                    publication = resource.flush()
                    if publication is not None:
                        report["publications"].append(publication)
                report["updated_at_utc"] = analysis.stamp()
                write_json(report_path, report)
                print(json.dumps({"refresh": job["name"], "status": job["status"], "completed": completed,
                                  "total": len(jobs), "acquisition": job.get("refreshed_acquisition", {}).get("path"),
                                  "failures": len(job.get("failures", []))}), flush=True)
        except BaseException as error:
            interrupted = error
            resource.cancelled.set()
            for future in futures:
                future.cancel()
        finally:
            pool.shutdown(wait=True, cancel_futures=True)
            for future, job in futures.items():
                if job["status"] != "pending":
                    continue
                if future.cancelled():
                    job["status"] = "cancelled_before_start"
                elif future.done():
                    try:
                        job.update(future.result())
                    except BaseException as error:
                        job.update(status="failed", failures=[{"kind": type(error).__name__, "error": str(error)}])
            try:
                publication = resource.flush()
                if publication is not None:
                    report["publications"].append(publication)
            except BaseException as error:
                interrupted = interrupted or error
                report["publication_failure"] = {"kind": type(error).__name__, "error": str(error)}
            report.update(finished_at_utc=analysis.stamp(), interrupted=interrupted is not None,
                          passed=interrupted is None and all(job["status"] == "acquired" for job in jobs))
            report["cleanup"]["all_worker_threads_joined"] = True
            write_json(report_path, report)
        if interrupted is not None:
            raise interrupted
        return {**report, "report": relative(report_path)}


def keyboard_svg_import_module(producer_body, alias):
    """Resolve the factory's import without collecting nested lexical bindings."""
    import families
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    raw = ("const source = ({" + producer_body + "});").encode("utf-8")
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Current SVG import owner does not form a complete grammar module")
    declaration = tree.root_node.named_children[0].named_children[0]
    expression = declaration.child_by_field_name("value")
    if expression is None or expression.type != "parenthesized_expression":
        raise ValueError("Current SVG import factory wrapper is unresolved")
    members = expression.named_children[0].named_children
    if len(members) != 1 or members[0].type not in {"pair", "method_definition"}:
        raise ValueError("Current SVG import requires one complete factory")
    factory = members[0].child_by_field_name("value") if members[0].type == "pair" else members[0]
    parameters, body = factory.child_by_field_name("parameters"), factory.child_by_field_name("body")
    if (parameters is None or len(parameters.named_children) != 3
            or parameters.named_children[2].type != "identifier" or body is None):
        raise ValueError("Current SVG import factory require parameter is unresolved")
    require = parameters.named_children[2].text.decode("utf-8")
    pending, bindings = list(reversed(body.named_children)), []
    while pending:
        node = pending.pop()
        if node.type in {"arrow_function", "function_expression", "function_declaration",
                         "method_definition", "class_declaration", "class"}:
            if node.type in {"function_declaration", "class_declaration"}:
                name = node.child_by_field_name("name")
                if name is not None and name.text.decode("utf-8") == alias and node.parent == body:
                    bindings.append(None)
            continue
        if node.type == "variable_declarator":
            name = node.child_by_field_name("name")
            if name is not None and name.type == "identifier" and name.text.decode("utf-8") == alias:
                scope = node.parent.parent
                while scope != body and scope.type != "statement_block":
                    scope = scope.parent
                if node.parent.type == "variable_declaration" or scope == body:
                    bindings.append(node.child_by_field_name("value"))
        pending.extend(reversed(node.named_children))
    if len(bindings) != 1 or bindings[0] is None:
        raise ValueError("Current SVG lettering enum import is absent or ambiguous")
    value = bindings[0]
    while value.type == "parenthesized_expression" and len(value.named_children) == 1:
        value = value.named_children[0]
    calls = list(value.named_children) if value.type == "sequence_expression" else [value]
    imported = []
    for call in calls:
        function, arguments = call.child_by_field_name("function"), call.child_by_field_name("arguments")
        if (call.type != "call_expression" or function is None or function.type != "identifier"
                or function.text.decode("utf-8") != require
                or arguments is None or len(arguments.named_children) != 1
                or arguments.named_children[0].type != "number"):
            raise ValueError("Current SVG lettering enum import expression is unresolved")
        imported.append(families.javascript_integer_literal(arguments.named_children[0].text.decode("ascii")))
    if not imported:
        raise ValueError("Current SVG lettering enum import expression is empty")
    return imported[-1]


def keyboard_artwork_inline_png(sources, read_input, expected_hash, *, css_selector=None, verify_producer=None):
    """Resolve exact delivered PNG bytes to every current JS/CSS literal owner."""
    if not re.fullmatch(r"[a-f0-9]{64}", expected_hash):
        raise ValueError("Inline keyboard PNG requires its complete pinned SHA-256")
    found = []
    for original in sources:
        suffix = Path(original["path"]).suffix.lower()
        if suffix not in {".js", ".css"} or suffix == ".css" and css_selector is None:
            continue
        raw = read_input(original)
        matches = [match for match in re.finditer(rb"data:image/png;base64,([A-Za-z0-9+/=]+)", raw)
                   if hashlib.sha256(base64.b64decode(match[1], validate=True)).hexdigest() == expected_hash]
        if not matches:
            continue
        if suffix == ".css":
            text = raw.decode("utf-8")
            rules = [rule for rule in re.finditer(r"([^{}]+)\{[^{}]*\}", text)
                     if rule[1].strip() == css_selector and any(
                         rule.start() <= len(raw[:match.start()].decode("utf-8")) < rule.end() for match in matches)]
            if not rules:
                continue
            if verify_producer is not None and verify_producer(original) != raw:
                raise ValueError("Inline keyboard PNG producer changed during verification")
            pixels, extraction = extract_inline_css_png(raw, {"selector": css_selector, "sha256": expected_hash})
        else:
            if verify_producer is not None and verify_producer(original) != raw:
                raise ValueError("Inline keyboard PNG producer changed during verification")
            pixels, extraction = extract_inline_resource_png(raw, expected_hash)
        found.append((original, pixels, extraction))
    if not found or any(pixels != found[0][1] for _, pixels, _ in found):
        raise FileNotFoundError("Current keyboard PNG literal owner is absent or ambiguous")
    return found


def keyboard_artwork_svg_producer(sources, read_input, *, lettering=False):
    """Retain the complete current SVG producer and its literal lettering enum."""
    import families
    modules, producer_candidates = {}, []
    for row in sources:
        if not row["path"].endswith(".js"):
            continue
        text = read_input(safe(ROOT / row["path"]), row).decode("utf-8")
        anchor = '"Error fetching/parsing SVG:"'
        if anchor in text:
            offset = text.index(anchor)
            found = families.keyboard_source_modules(text)
            modules[row["path"]] = found
            owners = [module for module in found if module["character_start"] <= offset < module["character_end"]]
            if not owners:
                raise ValueError("Current SVG producer is outside a complete grammar function")
            body = min(owners, key=lambda module: module["character_end"] - module["character_start"])
            producer_candidates.append((row, body))
    if len(producer_candidates) != 1:
        raise ValueError("Current SVG root-filter producer is absent or ambiguous")
    original, module = producer_candidates[0]
    body = module["body"]
    if not all('"' + token + '"' in body for token in ("_LED", "_Selection")):
        raise ValueError("Current SVG producer root-filter semantics are unresolved")
    producer = {"path": original["path"], "sha256": original["sha256"],
                "anchor": '"Error fetching/parsing SVG:"', "module_id": module["module_id"],
                "boundary_parser": "tree-sitter-javascript"}
    proof = {"input": {key: original[key] for key in ("path", "bytes", "sha256")}, "complete_module": module}
    if not lettering:
        return producer, {}, proof
    namespaces = {(alias, export) for alias, export in
                  re.findall(r'\[([\w$]+)\.([\w$]+)\.[\w$]+\]:"[A-Z]+"', body)}
    if len(namespaces) != 1 or not all('"' + token + '"' in body for token in ("_Lettering", "_Characters")):
        raise ValueError("Current SVG lettering namespace/filter semantics are unresolved")
    alias, export = namespaces.pop()
    module_id = keyboard_svg_import_module(body, alias)
    enum_modules = []
    for row in sources:
        if not row["path"].endswith(".js"):
            continue
        if row["path"] not in modules:
            modules[row["path"]] = families.keyboard_source_modules(read_input(safe(ROOT / row["path"]), row).decode("utf-8"))
        enum_modules.extend((row, module) for module in modules[row["path"]] if module["module_id"] == module_id)
    if len(enum_modules) != 1:
        raise ValueError("Current SVG lettering enum module is absent or ambiguous")
    enum_original, enum_module = enum_modules[0]
    enum_body = enum_module["body"]
    symbol = re.search(re.escape(export) + r':(?:\(\)=>|function\(\)\{return )([\w$]+)', enum_body)
    if symbol is None:
        raise ValueError("Current SVG lettering enum export is unresolved")
    binding = re.search(r'(?<![\w$])' + re.escape(symbol[1]) + r'=\{', enum_body)
    if binding is None:
        raise ValueError("Current SVG lettering enum literal is unresolved")
    start = binding.end() - 1
    enums = families.keyboard_static_literal(enum_body[start:families.javascript_delimited(enum_body, start)])
    proof["lettering_enum"] = {"input": {key: enum_original[key] for key in ("path", "bytes", "sha256")},
                               "complete_module": enum_module}
    return producer, keyboard_svg_lettering(body, enums), proof


def verify_keyboard_artwork_resources(*, product_ids=None):
    """Verify the complete typed atlas against actual originals and pinned display bytes offline."""
    snapshots = {}
    def read_input(path, expected=None):
        path = safe(path)
        if path.is_symlink():
            raise ValueError("Keyboard artwork verification input is a symbolic link")
        raw = path.read_bytes()
        actual = {"path": relative(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        if expected is not None and any(actual[key] != expected[key] for key in ("path", "bytes", "sha256")):
            raise ValueError("Keyboard artwork original differs from its acquisition: " + relative(path))
        previous = snapshots.setdefault(actual["path"], actual)
        if previous != actual:
            raise ValueError("Keyboard artwork input changed during verification: " + actual["path"])
        return raw

    catalog = resource_sources.builtin_data_path("razer-pages", "KEYBOARD_ARTWORK_CURRENT_DATA", root=ROOT)
    document = json.loads(read_input(catalog))
    additional_catalogs = {}
    for symbol in ("KEYBOARD_SOURCE_ARTWORK_DATA", "KEYBOARD_KEYPAD_ARTWORK_DATA",
                   "KEYBOARD_KEYPAD_DEVICE_IMAGE_DATA", "KEYBOARD_MAPPING_BLADE_ARTWORK_DATA"):
        path = resource_sources.builtin_data_path("razer-pages", symbol, root=ROOT)
        additional_catalogs[symbol] = json.loads(read_input(path))
    aliases = keyboard_artwork_atlas(document, product_ids=product_ids, additional_catalogs=additional_catalogs)
    mapping_inputs = []
    mappings = resource_sources.load_resource_mappings(root=ROOT, input_rows=mapping_inputs)
    if len(mapping_inputs) != 1:
        raise ValueError("Resource mapping owner did not retain its actual input identity")
    read_input(ROOT / resource_sources.MAPPINGS, mapping_inputs[0])
    lock = resource_sources.validate_lock(json.loads(read_input(ROOT / resource_sources.LOCK)))
    locked_files = lock["git_files"] + [row for group in lock["groups"] for row in group["files"]]
    products = {row["product_id"]: row for row in document["products"]}
    report = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "scope": "keyboard-artwork", "selected_products": sorted({use["product_id"] for rows in aliases.values() for use in rows}),
              "resource_revision": lock["storage"]["revision"], "network_access": False,
              "target_execution": False, "real_device_operations": False,
              "application_runtime_acceptance": "not performed", "installation_cache_acceptance": "unverified",
              "output_verification": "Reconstruct pinned display bytes in memory; no source media or installation cache is populated",
              "entries": [], "errors": []}
    tool_path = Path(__file__).resolve()
    if tool_path.is_relative_to(ROOT):
        read_input(tool_path)
        report["tool"] = snapshots[relative(tool_path)]
        for name in ("families.py", "asset_release.py", "analysis.py", "typed-data.json"):
            read_input(ROOT / "tools" / name)
    try:
        for row in image_analyzer_verification_inputs(svg=True):
            snapshots[row["path"]] = row
        decoder_error = None
    except (OSError, ValueError, ImportError) as error:
        decoder_error = {"kind": type(error).__name__, "message": str(error)}
        report["errors"].append(decoder_error)
    acquired, producer_cache, decode_cache, conversion_cache, inline_sources = {}, {}, {}, {}, {}
    revalidated_originals = {}
    def sources_for(pid):
        if pid not in acquired:
            acquired[pid] = keyboard_artwork_acquisition(pid, read_input)
        return acquired[pid]
    def read_original(source):
        raw = read_input(safe(ROOT / source["path"]), source)
        if source.get("http_status") != 200 and source.get("deployment_current_verified") is not True:
            revalidated_originals[source["path"]] = verified_official_http_revalidation(source, raw, read_input)
        return raw
    for alias, uses in sorted(aliases.items()):
        entry = {"alias": alias, "uses": uses, "errors": [], "source_verified": False,
                 "delivery_identity_verified": False, "pixel_decode_performed": False, "conversion_verified": False}
        report["entries"].append(entry)
        try:
            locked = keyboard_artwork_alias_identity(alias, mappings, locked_files)
            entry.update(output=locked, delivery_identity_verified=True)
        except (OSError, ValueError) as error:
            locked = None
            entry["errors"].append({"kind": type(error).__name__, "message": str(error), "stage": "delivery_identity"})
        try:
            plans = []
            for use in uses:
                pid = use["product_id"]
                product = products.get(pid)
                base = use
                request = {"display": alias}
                if use["collection"] == "customize_images":
                    if product is None:
                        raise ValueError("Customize image has no typed product owner")
                    normalization = product.get("customize_layout_normalization")
                    source_layout = (keyboard_normalized_svg_layout(use["layout_id"], normalization)
                                     if normalization else use["layout_id"])
                    bases = [image for image in product["images"] if image["svg"]
                             and image["edition_id"] == use["edition_id"] and image["layout_id"] == source_layout]
                    if len(bases) != 1:
                        raise ValueError("Customize image has no unique typed source variant")
                    base = bases[0]
                filename = Path(base["asset"]).name
                prefix = re.fullmatch(r"synapse/keyboard-keypad-(?:artwork|device-image)/\d+-([a-f0-9]{16})\.(?:png|svg)", base["asset"])
                candidates = [row for row in sources_for(pid) if
                              (row["sha256"].startswith(prefix[1]) if prefix else Path(row["path"]).stem == Path(filename).stem)
                              and Path(row["path"]).suffix.lower() in ({".svg"} if base["svg"] else {".avif", ".png"})]
                embedded = use["collection"] == "mapping_blade" and filename.startswith("embedded-")
                keypad_inline = use["collection"] == "keypad_images" and prefix and not base["svg"] and not candidates
                if embedded or keypad_inline:
                    prefix_hash = re.fullmatch(r"embedded-([a-f0-9]{16})\.png", filename) if embedded else prefix
                    if prefix_hash is None or locked is None or not locked["sha256"].startswith(prefix_hash[1]):
                        raise ValueError("Keyboard mapping embedded PNG identity is malformed")
                    css_selector = (".action-wrapper." + use["command"]
                                    + (".open" if use["state"] == "active" else "") + " .head:before") if embedded else None
                    inline_key = (pid, locked["sha256"], css_selector)
                    if inline_key not in inline_sources:
                        inline_sources[inline_key] = keyboard_artwork_inline_png(sources_for(pid),
                            lambda original: read_input(safe(ROOT / original["path"]), original),
                            locked["sha256"], css_selector=css_selector, verify_producer=read_original)
                    for source, raw, extraction in inline_sources[inline_key]:
                        proof = {"input": {key: source[key] for key in ("path", "bytes", "sha256")}, "extraction": extraction}
                        if proof not in entry.setdefault("inline_png_producers", []):
                            entry["inline_png_producers"].append(proof)
                    source, raw, _ = inline_sources[inline_key][0]
                    plans.append((source, raw, request))
                    continue
                if len(candidates) != 1:
                    raise FileNotFoundError("Current reachable artwork original is absent or ambiguous: " + base["asset"])
                source = candidates[0]
                entry.setdefault("original_candidates", []).append({key: source[key] for key in ("path", "bytes", "sha256")})
                raw = read_original(source)
                if use["collection"] == "customize_images":
                    if pid not in producer_cache:
                        producer_cache[pid] = keyboard_artwork_svg_producer(sources_for(pid), read_input,
                            lettering=bool(product.get("customize_layout_normalization")))
                    producer, lettering, proof = producer_cache[pid]
                    request = keyboard_customize_svg_request(raw, base["asset"], producer,
                        normalized=bool(product.get("customize_layout_normalization")),
                        lettering=lettering.get(use["layout_id"], "US"), layout_id=use["layout_id"])
                    if request["display"] != alias:
                        raise ValueError("Customize alias differs from the current source producer")
                if use["collection"] in {"source_artwork", "source_artwork_wrist"}:
                    request["png_optimize"] = True
                plans.append((source, raw, request))
            if len({(hashlib.sha256(raw).hexdigest(), tuple(request.get("svg_remove_root_ids", [])),
                     request.get("png_optimize", False)) for _, raw, request in plans}) != 1:
                raise ValueError("Shared keyboard alias resolves to conflicting original/transformation identities")
            source, raw, request = plans[0]
            entry.update(source={key: source[key] for key in ("path", "bytes", "sha256")}, source_verified=True)
            suffix = Path(source["path"]).suffix.lower()
            entry["format"] = "SVG" if suffix == ".svg" else "PNG"
            if decoder_error:
                raise ImportError("Pinned static pixel decoder was not verified")
            if "svg_remove_root_ids" in request:
                output, transformation = filter_svg_root_children(raw, request["svg_remove_root_ids"])
                entry["transformation"] = transformation
            elif suffix == ".avif":
                conversion_key = (source["sha256"], request.get("png_optimize", False))
                if conversion_key not in conversion_cache:
                    output, conversion = avif_display_png(safe(ROOT / source["path"]),
                        png_optimize=request.get("png_optimize", False))
                    comparison = compare_resource_pixels(raw, output, "AVIF")
                    conversion_cache[conversion_key] = (output, conversion, comparison)
                output, conversion, comparison = conversion_cache[conversion_key]
                entry.update(conversion=conversion, pixel_comparison=comparison)
            else:
                output = raw
            entry["actual_output"] = {"bytes": len(output), "sha256": hashlib.sha256(output).hexdigest()}
            if locked is not None and (len(output) != locked["bytes"] or hashlib.sha256(output).hexdigest() != locked["sha256"]):
                if suffix == ".svg" and b"\r" not in output:
                    reuse = resource_sources.verified_alias_reuse(output, alias, normalize_source_lf=True, root=ROOT, mappings=mappings)
                    output = output.replace(b"\n", b"\r\n")
                    entry["delivery_projection"] = reuse
                    entry["actual_output"] = {"bytes": len(output), "sha256": hashlib.sha256(output).hexdigest()}
                else:
                    raise ValueError("Reconstructed display bytes differ from their pinned delivery identity")
            output_hash = hashlib.sha256(output).hexdigest()
            decode_key = (output_hash, entry["format"])
            if decode_key not in decode_cache:
                decoded, _ = (decoded_svg_rgba(output) if suffix == ".svg" else decoded_resource_rgba(output, "PNG"))
                decode_cache[decode_key] = decoded
            entry.update(actual_output={"bytes": len(output), "sha256": output_hash},
                         decode=decode_cache[decode_key], pixel_decode_performed=True,
                         conversion_verified=locked is not None)
        except (OSError, ValueError, KeyError, TypeError, ImportError, RuntimeError, ET.ParseError) as error:
            entry["errors"].append({"kind": type(error).__name__, "message": str(error), "stage": "source_conversion_decode"})
        entry["passed"] = not entry["errors"] and entry["pixel_decode_performed"] and entry["conversion_verified"]
    report["svg_producers"] = {str(pid): proof for pid, (_, _, proof) in sorted(producer_cache.items())}
    report["official_http_revalidations"] = revalidated_originals
    changed = []
    for row in snapshots.values():
        try:
            path = safe(ROOT / row["path"])
            if path.stat().st_size != row["bytes"] or digest(path) != row["sha256"]:
                changed.append(row["path"])
        except OSError:
            changed.append(row["path"])
    report.update(inputs=sorted(snapshots.values(), key=lambda row: row["path"]),
                  inputs_unchanged=not changed, changed_inputs=changed)
    counts = Counter(row.get("format", "unrecognized") for row in report["entries"])
    report["summary"] = {"products": len(report["selected_products"]), "atlas_rows": sum(len(uses) for uses in aliases.values()),
                         "total": len(aliases), "verified": sum(row["passed"] for row in report["entries"]),
                         "failed": sum(not row["passed"] for row in report["entries"]),
                         "pixel_decoded": sum(row["pixel_decode_performed"] for row in report["entries"]),
                         "missing_delivery_identities": sum(not row["delivery_identity_verified"] for row in report["entries"]),
                         "formats": dict(counts)}
    report["passed"] = not report["errors"] and report["inputs_unchanged"] and not report["summary"]["failed"]
    previous = json.loads(RESOURCE_AUDIT.read_bytes()) if RESOURCE_AUDIT.is_file() else {"schema_version": 1}
    previous["keyboard_artwork"] = report
    write_json(RESOURCE_AUDIT, previous)
    return report


def _scratch_link(path):
    if path.is_symlink():
        return True
    return os.name == "nt" and (path.is_junction() or bool(path.lstat().st_file_attributes & 0x400))


def _scratch_processes():
    """Read process metadata; file deletion separately requires exclusive access."""
    if os.name != "nt":
        return [], ["Process path inspection is implemented only for Windows"]
    command = ("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); "
               "Get-CimInstance Win32_Process -ErrorAction Stop | "
               "Select-Object ProcessId,Name,CommandLine | ConvertTo-Json -Compress")
    try:
        result = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", command],
                                capture_output=True, check=True, timeout=30,
                                creationflags=subprocess.CREATE_NO_WINDOW)
        rows = json.loads(result.stdout.decode("utf-8-sig"))
        if not isinstance(rows, list) or not rows:
            raise ValueError("Process inventory is empty")
        return [{"pid": row["ProcessId"], "executable": row.get("Name") or "",
                 "command_line": row.get("CommandLine") or ""}
                for row in rows], []
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        return [], ["Process inventory unavailable: " + type(error).__name__]


def _scratch_references(names):
    """Search maintained owners and receipts; never descend into original raw trees."""
    names = {name.lower(): name for name in names}
    if not names:
        return {}, []
    executable = shutil.which("rg")
    if executable is None:
        return {}, ["Fixed-string reference scanner rg is unavailable"]
    paths = [name for name in ("docs", "tools", ".agents", "crates", ".analysis/results", ".analysis/inputs", ".work")
             if (ROOT / name).exists()]
    paths.extend(path.name for path in ROOT.iterdir() if path.is_file()
                 and path.suffix.lower() in {".md", ".json", ".toml", ".py"})
    if (ROOT / ".analysis/inputs.json").is_file():
        paths.append(".analysis/inputs.json")
    if any(_scratch_link(ROOT / name) for name in paths):
        return {}, ["Reference root is linked"]
    arguments = [executable, "--no-config", "--hidden", "--no-ignore", "--threads", "16", "--fixed-strings", "--ignore-case",
                 "--only-matching", "--no-line-number", "--with-filename", "--no-heading", "--color", "never",
                 "--path-separator", "/"]
    for extension in ("json", "md", "py", "rs", "toml", "yml", "yaml", "ps1", "cmd"):
        arguments.extend(["--glob", "*." + extension])
    for directory in (".ref", "raw", "raw-by-url", "functions", "modules", "factories", "ui-trees",
                      "__pycache__", ".git", "node_modules", "target"):
        arguments.extend(["--glob", "!**/" + directory + "/**"])
    for directory in ("source", "sources", "decompiled"):
        arguments.extend(["--glob", "!.analysis/**/" + directory + "/**"])
    arguments.extend(["--glob", "!.work/*/**"])
    references = {}
    try:
        with tempfile.TemporaryDirectory(prefix="razer-scratch-reference-") as temporary:
            patterns = Path(temporary) / "names.txt"
            patterns.write_text("\n".join(names) + "\n", encoding="utf-8")
            arguments.extend(["--file", str(patterns), "--", *paths])
            options = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}
            result = subprocess.run(arguments, cwd=ROOT, capture_output=True, check=False, timeout=180, **options)
        if result.returncode not in (0, 1):
            return {}, ["Reference scanner failed: " + result.stderr.decode("utf-8", errors="replace")[:2000]]
        if result.returncode == 1 and result.stdout.strip():
            return {}, ["Reference scanner returned inconsistent no-match output"]
        if result.returncode == 0 and not result.stdout.strip():
            return {}, ["Reference scanner returned no matches with success status"]
        for line in result.stdout.decode("utf-8").splitlines():
            filename, match = line.rsplit(":", 1)
            name = names[match.lower()]
            references.setdefault(name, set()).add(relative(ROOT / filename))
        return {name: sorted(owners) for name, owners in references.items()}, []
    except (OSError, ValueError, KeyError, UnicodeError, subprocess.SubprocessError) as error:
        return {}, ["Reference scan unavailable: " + type(error).__name__]


def _scratch_unlink(path, snapshot):
    """Delete the selected regular file only while holding exclusive Windows access."""
    scratch = ROOT / ".work"
    if (_scratch_link(scratch) or _scratch_link(path) or path.parent not in {scratch, scratch / "__pycache__"}
            or (path.parent.name == "__pycache__" and _scratch_link(path.parent))
            or not safe(path).is_relative_to(scratch.resolve())):
        raise ValueError("Scratch deletion target is linked or escaped")
    def unchanged():
        current = path.stat()
        if (current.st_size, current.st_mtime_ns, current.st_ino) != snapshot:
            raise ValueError("candidate_changed")
    unchanged()
    if os.name != "nt":
        raise ValueError("Exclusive deletion is implemented only for Windows")
    import ctypes
    from ctypes import wintypes
    api = ctypes.WinDLL("kernel32", use_last_error=True)
    api.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, wintypes.LPVOID,
                               wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    api.CreateFileW.restype = wintypes.HANDLE
    api.ReadFile.argtypes = [wintypes.HANDLE, wintypes.LPVOID, wintypes.DWORD,
                            ctypes.POINTER(wintypes.DWORD), wintypes.LPVOID]
    api.ReadFile.restype = wintypes.BOOL
    api.SetFileInformationByHandle.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD]
    api.SetFileInformationByHandle.restype = wintypes.BOOL
    api.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = api.CreateFileW(str(path), 0x80010000, 0, None, 3, 0x00200000, None)
    if handle == ctypes.c_void_p(-1).value:
        raise OSError(ctypes.get_last_error(), "exclusive_access_unavailable")
    try:
        unchanged()
        if _scratch_link(path):
            raise ValueError("candidate_changed_to_link")
        checksum, length = hashlib.sha256(), 0
        buffer, count = ctypes.create_string_buffer(1024 * 1024), wintypes.DWORD()
        while True:
            if not api.ReadFile(handle, buffer, len(buffer), ctypes.byref(count), None):
                raise ctypes.WinError(ctypes.get_last_error())
            if not count.value:
                break
            checksum.update(buffer.raw[:count.value])
            length += count.value
        unchanged()
        if length != snapshot[0]:
            raise ValueError("candidate_changed")
        disposition = ctypes.c_ubyte(1)
        if not api.SetFileInformationByHandle(handle, 4, ctypes.byref(disposition), ctypes.sizeof(disposition)):
            raise ctypes.WinError(ctypes.get_last_error())
    finally:
        api.CloseHandle(handle)
    if path.exists():
        raise ValueError("deleted_path_reappeared")
    return {"bytes": length, "sha256": checksum.hexdigest()}


def _clean_stale(before=None):
    scratch = ROOT / ".work"
    if scratch.is_symlink() or (scratch.exists() and (_scratch_link(scratch) or not scratch.resolve().is_relative_to(ROOT.resolve()))):
        raise ValueError("Scratch root is a link or escaped")
    cutoff = min(datetime.now(timezone.utc).timestamp(), before) if before is not None else datetime.now(timezone.utc).timestamp()
    queue_path = ROOT / ".analysis/results/reverse-pending.json"
    if queue_path.is_file():
        queue = json.loads(queue_path.read_bytes())
        if queue.get("started_at_utc"):
            cutoff = min(cutoff, datetime.fromisoformat(queue["started_at_utc"]).timestamp())
    report = {"schema_version": 1, "mode": "stale", "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "before_utc": datetime.fromtimestamp(cutoff, timezone.utc).isoformat(),
              "scope": "old root log/txt/out and root __pycache__ bytecode only",
              "reference_scope": "maintenance sources, documents, input index and retained JSON receipts; indexed functions/modules/AST bodies excluded",
              "removed": [], "retained": [], "failures": [], "target_execution": False,
              "receipt_merge": "owning coordinator merges scratch_cleanup after publishing its queue receipt"}
    candidates = []
    for path in sorted(scratch.iterdir()) if scratch.exists() else []:
        name = ".work/" + path.name
        if _scratch_link(path):
            report["retained"].append({"path": name, "reason": "link"})
        elif path.is_file() and path.suffix.lower() in {".log", ".txt", ".out"}:
            candidates.append(path)
        elif path.is_dir() and path.name == "__pycache__":
            for cache in sorted(path.iterdir()):
                if not _scratch_link(cache) and cache.is_file() and cache.suffix == ".pyc":
                    candidates.append(cache)
                else:
                    report["retained"].append({"path": name + "/" + cache.name, "reason": "unknown_cache_or_link"})
        else:
            report["retained"].append({"path": name, "reason": "protected_or_outside_stale_scope"})
    snapshots = {path: (path.stat().st_size, path.stat().st_mtime_ns, path.stat().st_ino) for path in candidates}
    references, reference_errors = _scratch_references([path.name for path in candidates])
    processes, process_errors = _scratch_processes()
    report["failures"].extend(reference_errors + process_errors)
    report["processes_inspected"] = len(processes)
    for path in candidates:
        row = {"path": relative(path), "bytes": snapshots[path][0]}
        commands = [p["pid"] for p in processes if path.name.casefold() in p["command_line"].casefold()]
        if report["failures"]:
            row["reason"] = "guard_inventory_incomplete"
        elif path.name in references:
            row.update(reason="referenced", referenced_by=references[path.name])
        elif commands:
            row.update(reason="active_process_path", pids=commands)
        elif snapshots[path][1] >= int(cutoff * 1_000_000_000):
            row["reason"] = "newer_than_run"
        else:
            try:
                row.update(_scratch_unlink(path, snapshots[path]))
                report["removed"].append(row)
                continue
            except (OSError, ValueError) as error:
                row["reason"] = str(error)
        report["retained"].append(row)
    report["summary"] = {"candidate_files": len(candidates), "candidate_bytes": sum(r[0] for r in snapshots.values()),
                         "removed_files": len(report["removed"]), "removed_bytes": sum(r["bytes"] for r in report["removed"]),
                         "retained_entries": len(report["retained"]), "guard_failures": len(report["failures"]),
                         "retained_by_reason": dict(Counter(r["reason"] for r in report["retained"]))}
    print(json.dumps(report, ensure_ascii=False), flush=True)
    return report


def _clean_path(name):
    """Validate lexical and resolved containment before following any scratch path."""
    if not isinstance(name, str) or "\\" in name:
        raise ValueError("Cleanup path must be a project-relative POSIX path")
    path = Path(name)
    if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != ".work":
        raise ValueError("Cleanup path is outside scratch")
    target = ROOT / path
    for ancestor in [ROOT, *reversed(list(target.parents)[:len(path.parts)]) , target]:
        if ancestor.exists() and _scratch_link(ancestor):
            raise ValueError("Cleanup path contains a link or reparse point")
    if not target.resolve().is_relative_to((ROOT / ".work").resolve()) or target == ROOT / ".work":
        raise ValueError("Cleanup target escaped scratch")
    return target


def _clean_file_identity(path):
    stat = path.stat()
    return {"path": relative(path), "bytes": stat.st_size, "mtime_ns": stat.st_mtime_ns,
            "inode": stat.st_ino, "device": stat.st_dev, "sha256": digest(path)}


def _clean_tree(name):
    root = _clean_path(name)
    files, directories = [], []
    for folder, children, leaves in os.walk(root, followlinks=False):
        parent = Path(folder)
        for item in [parent, *(parent / child for child in children + leaves)]:
            _clean_path(relative(item))
            if _scratch_link(item):
                raise ValueError("Cleanup tree contains a link or reparse point")
        directories.append(relative(parent))
        files.extend(_clean_file_identity(parent / leaf) for leaf in sorted(leaves))
    return {"files": sorted(files, key=lambda row: row["path"]),
            "directories": sorted(directories), "bytes": sum(row["bytes"] for row in files)}


def _clean_capture_batch_ended(record, cutoff):
    """Release blanket reservations only after every recorded process has exited."""
    try:
        from reverse_pending import coordinator_instance_absence
        started = datetime.fromisoformat(record["started_at_utc"])
        finished = datetime.fromisoformat(record["finished_at_utc"])
        jobs = record["jobs"]
        if (record.get("target_execution") is not False or started.tzinfo is None
                or finished.tzinfo is None or not started.timestamp() <= finished.timestamp() < cutoff
                or not isinstance(jobs, list) or not jobs):
            return False
        processes = [{"pid": record["coordinator_pid"], "started_at_utc": record["started_at_utc"]}]
        for job in jobs:
            ended = datetime.fromisoformat(job["finished_at_utc"])
            if (job.get("status") not in {"passed", "finished"} or job.get("exit_code") != 0
                    or ended.tzinfo is None or not started <= ended <= finished):
                return False
            pid = job.get("cli_pid", job.get("maintenance_pid"))
            if type(pid) is not int or pid <= 0:
                return False
            processes.append({"pid": pid, "started_at_utc": job.get("started_at_utc", record["started_at_utc"])})
        return all(type(session["pid"]) is int and session["pid"] > 0
                   and coordinator_instance_absence(session).get("confirmed_absent") is True for session in processes)
    except (KeyError, TypeError, ValueError):
        return False


def _clean_supplement_attempt(directory):
    """Require source-bound ownership before admitting a version-1 attempt name."""
    import analysis

    if not analysis.native_supplement_attempt_name(directory.name):
        return False
    plan_path = directory / "plan.json"
    if not plan_path.is_file():
        return "-" not in directory.name
    plan = json.loads(plan_path.read_bytes())
    session_path = directory / "session.json"
    if not session_path.is_file() or _scratch_link(plan_path) or _scratch_link(session_path):
        return "-" not in directory.name and "analysis_request" not in plan
    session = json.loads(session_path.read_bytes())
    if "analysis_request" not in plan:
        return "-" not in directory.name and not any(key in plan or key in session for key in
            ("request_sha256", "attempt_id", "attempt_nonce", "tool_sources"))
    if plan.get("output") != relative(directory):
        return False
    try:
        analysis.verify_native_supplement_request(plan, session, root=ROOT)
    except (OSError, KeyError, TypeError, ValueError):
        return False
    return True


def _clean_upload_git(target, *arguments):
    """Inspect local Git objects without filters, optional writes or lazy fetches."""
    environment = {key: value for key, value in os.environ.items() if not key.upper().startswith("GIT_")}
    environment.update(GIT_TERMINAL_PROMPT="0", GIT_NO_LAZY_FETCH="1", GIT_OPTIONAL_LOCKS="0",
                       GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
    options = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}
    result = subprocess.run(["git", "--no-optional-locks", "--no-replace-objects",
                             "--git-dir=" + str(target / ".git"), "--work-tree=" + str(target),
                             "-c", "core.fsmonitor=false", "-c", "protocol.allow=never", *arguments],
                            cwd=target, env=environment, capture_output=True, timeout=120, **options)
    if result.returncode:
        raise ValueError("Upload clone Git identity or object verification failed")
    return result.stdout


def _clean_upload_evidence(target):
    """Prove that every disposable clone payload belongs to a retained pinned release."""
    import asset_release
    import configparser

    name = relative(_clean_path(relative(target)))
    git_directory = target / ".git"
    if not git_directory.is_dir():
        raise ValueError("Upload clone has no standalone Git directory")
    files = []
    for directory, children, leaves in os.walk(target, followlinks=False):
        for child in [Path(directory), *(Path(directory) / item for item in children + leaves)]:
            _clean_path(relative(child))
            if _scratch_link(child):
                raise ValueError("Upload clone contains a link or reparse point")
        files.extend(Path(directory) / item for item in leaves)
    forbidden = ["commondir", "gitdir", "objects/info/alternates", "objects/info/http-alternates", "info/grafts"]
    if any((git_directory / item).exists() for item in forbidden) or any(
            path.name.endswith((".lock", ".promisor")) for path in files if path.is_relative_to(git_directory)):
        raise ValueError("Upload clone has shared, active or incomplete Git objects")

    basis = []
    def bound(path):
        identity, raw = _clean_history_file(_clean_file_identity(path), contents=True)
        basis.append(identity)
        return raw

    lock_path = ROOT / asset_release.LOCK
    lock = asset_release.validate_lock(json.loads(bound(lock_path)))
    revision, repository = lock["storage"].get("revision"), lock["storage"]["repository"]
    if not isinstance(revision, str) or not re.fullmatch(r"[a-f0-9]{40}", revision):
        raise ValueError("Upload release has no immutable storage revision")
    tool = ROOT / "tools/asset_release.py"
    bound(tool)
    receipt_path = ROOT / ".analysis/results/asset-release-update.json"
    receipt = json.loads(bound(receipt_path))
    release = receipt.get("verification_checkout_release", {}) if isinstance(receipt, dict) else {}
    released = release.get("checkout", {}).get("path") == name
    if released:
        asset_release.verify_checkout_release(receipt, lock, target)
    if (not isinstance(receipt, dict) or receipt.get("schema_version") != 1
            or receipt.get("target_execution") is not False or receipt.get("repository") != repository
            or not released and receipt.get("tool") != {"path": relative(tool), "sha256": digest(tool)}):
        raise ValueError("Upload completion receipt does not belong to the current release owner")
    if receipt.get("verification_checkout") == name:
        raise ValueError("Upload clone remains the current verification checkout")
    manifest = bound(target / "release-index.json")
    if manifest != asset_release.encoded(asset_release.published_manifest(lock)):
        raise ValueError("Upload manifest differs from the retained resource lock")
    manifest_sha = hashlib.sha256(manifest).hexdigest()
    if released:
        for key in ("lock", "mappings", "source_index", "source_manifest"):
            bound(ROOT / release[key]["path"])
    rows = asset_release.rows_for(lock) + lock["git_files"]
    expected = {row["path"]: row for row in rows}
    expected_names = set(expected) | asset_release.META_FILES
    completion = receipt.get("verification", {})
    preflight = receipt.get("read_only_remote_preflight", {})
    if (receipt.get("status") == "published_and_verified" and receipt.get("remote_publication") is True
            and receipt.get("revision") == revision and isinstance(completion, dict)
            and completion.get("passed") is True and completion.get("repository") == repository
            and completion.get("revision") == revision
            and completion.get("source_files_verified") == len(asset_release.rows_for(lock))
            and completion.get("retained_files_present") == len(lock["git_files"])
            and completion.get("remote_manifest_present") is True):
        proof_kind, observed = "published_and_verified", receipt
    elif (isinstance(preflight, dict) and preflight.get("repository") == repository
            and receipt.get("base_revision") == revision
            and preflight.get("pinned_base_revision") == revision
            and preflight.get("observed_main_revision") == revision
            and preflight.get("main_equals_pinned_base") is True
            and preflight.get("pinned_tree_files") == len(expected_names)
            and preflight.get("main_tree_files") == len(expected_names)
            and preflight.get("remote_manifest_sha256") == manifest_sha
            and preflight.get("only_git_refs_tree_metadata_and_manifest_read") is True
            and preflight.get("media_downloaded") is False and preflight.get("remote_write") is False
            and preflight.get("target_execution") is False):
        proof_kind, observed = "pinned_readonly_remote_preflight", preflight
    else:
        raise ValueError("Upload clone has no matching completed or reproducible publication evidence")
    recorded = datetime.fromisoformat(observed["recorded_at_utc"])
    if recorded.tzinfo is None or recorded > datetime.now(timezone.utc):
        raise ValueError("Upload publication evidence has no valid completion time")

    config = configparser.ConfigParser(interpolation=None)
    config.read_string(bound(git_directory / "config").decode("utf-8"))
    if (config.get('remote "origin"', "url", fallback=None) != "https://huggingface.co/datasets/" + repository
            or config.has_option("core", "worktree") or config.getboolean("core", "bare", fallback=False)
            or config.has_option("extensions", "partialclone")
            or config.has_section("extensions")
            or any(config.get(section, "promisor", fallback="false").casefold() != "false"
                   for section in config.sections() if section.startswith("remote "))):
        raise ValueError("Upload origin or object ownership differs from the pinned resource repository")
    head = _clean_upload_git(target, "rev-parse", "--verify", "HEAD").decode("ascii").strip()
    refs = {}
    for line in _clean_upload_git(target, "for-each-ref", "--format=%(refname) %(objectname)").decode("ascii").splitlines():
        ref, commit = line.split()
        refs[ref] = commit
    allowed_refs = {"refs/heads/main", "refs/remotes/origin/main", "refs/remotes/origin/HEAD"}
    if (head != revision or not {"refs/heads/main", "refs/remotes/origin/main"} <= refs.keys()
            or not refs.keys() <= allowed_refs or any(commit != revision for commit in refs.values())):
        raise ValueError("Upload clone contains unpublished, additional or replaced refs")
    tree, index = {}, {}
    for raw in _clean_upload_git(target, "ls-tree", "-rz", "--full-tree", "HEAD").split(b"\0"):
        if not raw:
            continue
        metadata, raw_name = raw.split(b"\t", 1)
        mode, kind, oid = metadata.decode("ascii").split()
        path = raw_name.decode("utf-8")
        if mode not in {"100644", "100755"} or kind != "blob" or path in tree:
            raise ValueError("Upload tree contains unsupported or duplicate entries")
        tree[path] = (mode, oid)
    for raw in _clean_upload_git(target, "ls-files", "--stage", "-z").split(b"\0"):
        if not raw:
            continue
        metadata, raw_name = raw.split(b"\t", 1)
        mode, oid, stage = metadata.decode("ascii").split()
        path = raw_name.decode("utf-8")
        if stage != "0" or path in index:
            raise ValueError("Upload index contains an unresolved conflict")
        index[path] = (mode, oid)
    worktree = {path.relative_to(target).as_posix() for path in files if not path.is_relative_to(git_directory)}
    if set(tree) != expected_names or index != tree or worktree != expected_names:
        raise ValueError("Upload clone contains staged, untracked or unowned resource files")
    for path, (_, oid) in tree.items():
        if path in expected:
            asset_release.canonical(path)
        raw = (target / path).read_bytes()
        blob_oid = hashlib.sha1(b"blob " + str(len(raw)).encode("ascii") + b"\0" + raw).hexdigest()
        row = expected.get(path)
        if row:
            pointer = ("version https://git-lfs.github.com/spec/v1\n"
                       f"oid sha256:{row['sha256']}\nsize {row['bytes']}\n").encode("ascii")
            pointer_oid = hashlib.sha1(b"blob " + str(len(pointer)).encode("ascii") + b"\0" + pointer).hexdigest()
            payload = len(raw) == row["bytes"] and hashlib.sha256(raw).hexdigest() == row["sha256"]
            if not (raw == pointer and blob_oid == oid or payload and oid in {blob_oid, pointer_oid}):
                raise ValueError("Upload resource has unpublished or corrupt content")
        elif blob_oid != oid:
            raise ValueError("Upload metadata has uncommitted content")
    if _clean_upload_git(target, "fsck", "--full", "--no-reflogs", "--unreachable", "--no-progress").strip():
        raise ValueError("Upload clone contains unreachable unpublished Git objects")
    payloads, locked_hashes = [], {row["sha256"]: row["bytes"] for row in rows}
    hook_names = {"applypatch-msg", "pre-applypatch", "post-applypatch", "pre-commit", "pre-merge-commit",
                  "prepare-commit-msg", "commit-msg", "post-commit", "pre-rebase", "post-checkout",
                  "post-merge", "pre-push", "pre-receive", "update", "post-receive", "post-update",
                  "push-to-checkout", "pre-auto-gc", "post-rewrite", "sendemail-validate", "fsmonitor-watchman"}
    for path in files:
        rel = path.relative_to(git_directory).as_posix() if path.is_relative_to(git_directory) else ""
        if rel.startswith("lfs/objects/"):
            parts = rel.split("/")
            sha = parts[-1]
            if (len(parts) != 5 or not re.fullmatch(r"[a-f0-9]{64}", sha)
                    or parts[2:4] != [sha[:2], sha[2:4]] or sha not in locked_hashes
                    or path.stat().st_size != locked_hashes[sha] or digest(path) != sha):
                raise ValueError("Upload LFS cache contains an undeclared or corrupt payload")
            payloads.append({"path": relative(path), "bytes": locked_hashes[sha], "sha256": sha})
        elif rel.startswith("lfs/tmp/") and path.stat().st_size:
            raise ValueError("Upload LFS transfer has an unpublished partial payload")
        elif rel:
            hook = rel.removeprefix("hooks/").removesuffix(".sample")
            ordinary = (rel in {"HEAD", "config", "description", "index", "packed-refs", "shallow",
                               "COMMIT_EDITMSG", "FETCH_HEAD", "ORIG_HEAD", "info/exclude"}
                        or rel.startswith("hooks/") and hook in hook_names
                        or rel in {"logs/HEAD", *("logs/" + ref for ref in refs), *refs}
                        or re.fullmatch(r"objects/(?:[a-f0-9]{2}/[a-f0-9]{38}|pack/pack-[a-f0-9]{40}\.(?:pack|idx|rev|bitmap)|info/(?:packs|commit-graph))", rel)
                        or rel.startswith("lfs/tmp/") and path.stat().st_size == 0)
            if not ordinary:
                raise ValueError("Upload Git metadata contains an unowned file")
    for identity in basis:
        if _clean_file_identity(ROOT / identity["path"]) != identity:
            raise ValueError("Upload rebuild evidence changed during inspection")
    return {"path": name, "kind": "old_asset_upload_clone",
            "owner_receipts": [relative(lock_path), relative(receipt_path)], "basis": basis,
            "rebuild_evidence": {"proof_kind": proof_kind, "repository": repository, "revision": revision,
                                 "manifest_sha256": manifest_sha, "refs": refs, "tracked_files": len(tree),
                                 "local_lfs_payloads": sorted(payloads, key=lambda item: item["path"]),
                                 "worktree_clean": True, "unpublished_content": False, "network_access": False}}


def _clean_candidates(before=None):
    import configparser
    cutoff = datetime.now(timezone.utc).timestamp() if before is None else before
    queue = ROOT / ".analysis/results/reverse-pending.json"
    if queue.is_file():
        # An unfinished or still-owned queue protects every current-run copy.
        current = json.loads(queue.read_bytes())
        started = current.get("started_at_utc")
        if not isinstance(started, str):
            raise ValueError("Current queue start is unavailable; scratch cleanup cannot establish an age boundary")
        started_at = datetime.fromisoformat(started).timestamp()
        ended = False
        finished, session = current.get("finished_at_utc"), current.get("queue_session")
        if isinstance(finished, str) and isinstance(session, dict):
            from reverse_pending import coordinator_instance_absence
            finished_at = datetime.fromisoformat(finished).timestamp()
            if started_at <= finished_at <= cutoff:
                ended = coordinator_instance_absence(session).get("confirmed_absent") is True
        if not ended:
            cutoff = min(cutoff, started_at)
    candidates, retained = [], []
    reserved = {}
    reservations = ROOT / ".work/reverse-pending"
    for manifest in sorted(reservations.glob("*-capture-batch.json")) if reservations.is_dir() else []:
        if _scratch_link(manifest):
            raise ValueError("Analyzer reservation is a reparse point")
        record = json.loads(manifest.read_bytes())
        if _clean_capture_batch_ended(record, cutoff):
            continue
        for job in record.get("jobs", []):
            identity = job.get("sha256")
            if isinstance(identity, str) and re.fullmatch(r"[a-f0-9]{64}", identity):
                reserved.setdefault(identity, []).append(relative(manifest))
    native = ROOT / ".work/native"
    if native.exists():
        _clean_path(".work/native")
    for sha_root in sorted(native.iterdir()) if native.is_dir() else []:
        if not re.fullmatch(r"[a-f0-9]{64}", sha_root.name) or _scratch_link(sha_root):
            continue
        supplements = sha_root / "supplements"
        if supplements.exists() and _scratch_link(supplements):
            raise ValueError("Supplement scratch root is a reparse point")
        for target in sorted(supplements.iterdir()) if supplements.is_dir() else []:
            name = relative(target)
            if sha_root.name in reserved:
                retained.append({"path": name, "reason": "reserved_analysis_family", "owners": reserved[sha_root.name]})
                continue
            result = ROOT / ".analysis/results/native" / sha_root.name / "supplements" / target.name
            if not target.is_dir() or _scratch_link(target) or not _clean_supplement_attempt(result):
                retained.append({"path": name, "reason": "unknown_or_linked_supplement"})
                continue
            basis = [result / item for item in ("session.json", "capture.json", "plan.json")]
            if not all(path.is_file() and not _scratch_link(path) for path in basis):
                retained.append({"path": name, "reason": "missing_retained_completion_evidence"})
                continue
            session, capture = [json.loads(path.read_bytes()) for path in basis[:2]]
            if (not session.get("finished_at_utc") or session.get("passed") is not True
                    or session.get("exit_code") != 0 or capture.get("capture_complete") is not True
                    or capture.get("all_pseudocode_captured") is False
                    or capture.get("target_execution") is not False):
                retained.append({"path": name, "reason": "unfinished_failed_or_unreviewed_supplement"})
                continue
            if datetime.fromisoformat(session["finished_at_utc"]).timestamp() >= cutoff:
                retained.append({"path": name, "reason": "current_run_supplement"})
                continue
            candidates.append({"path": name, "kind": "ended_supplement_private_copy",
                               "owner_receipts": [relative(path) for path in basis],
                               "basis": [_clean_file_identity(path) for path in basis]})
    uploads = ROOT / ".work/asset-releases/uploads"
    if uploads.exists():
        _clean_path(".work/asset-releases/uploads")
    if uploads.exists() and _scratch_link(uploads):
        raise ValueError("Asset uploads scratch root is a reparse point")
    for target in sorted(uploads.iterdir()) if uploads.is_dir() else []:
        name = relative(target)
        if (not target.is_dir() or _scratch_link(target)
                or not re.fullmatch(r"assets-[a-f0-9]{20}-[A-Za-z0-9_-]+", target.name)):
            retained.append({"path": name, "reason": "unknown_or_linked_upload"})
            continue
        config = target / ".git/config"
        if (not config.is_file() or _scratch_link(config) or not re.search(
                r"(?m)^\s*url\s*=\s*https://huggingface\.co/datasets/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\s*$",
                config.read_text(encoding="utf-8"))):
            retained.append({"path": name, "reason": "not_a_verified_private_upload_clone"})
            continue
        if target.stat().st_mtime >= cutoff:
            retained.append({"path": name, "reason": "current_run_upload"})
            continue
        try:
            candidates.append(_clean_upload_evidence(target))
        except (OSError, ValueError, KeyError, TypeError, UnicodeError, configparser.Error,
                subprocess.SubprocessError) as error:
            retained.append({"path": name, "reason": "upload_completion_or_rebuild_unverified",
                             "detail": str(error)})
    return candidates, retained


def _clean_historical_native_stdout(value):
    """Recognize exact ended launch/completion records, never arbitrary log text."""
    if not isinstance(value, str):
        return False
    try:
        lines = [json.loads(line) for line in value.splitlines() if line.strip()]
        if (len(lines) != 2 or set(lines[0]) != {"pid", "result", "log"}
                or set(lines[1]) != {"session", "passed"} or lines[1]["passed"] is not True):
            return False
        launch, completion = lines
        result = launch["result"]
        if not isinstance(result, str) or not re.fullmatch(
                r"\.analysis/results/native/[a-f0-9]{64}/supplements/[a-f0-9]{16}(?:-[a-f0-9]{16})?", result):
            return False
        if not _clean_supplement_attempt(safe(ROOT / result)):
            return False
        scratch = result.replace(".analysis/results/native/", ".work/native/", 1)
        if launch["log"] != scratch + "/ida.log" or completion["session"] != result + "/session.json":
            return False
        session_path = safe(ROOT / completion["session"])
        capture_path = safe(ROOT / result / "capture.json")
        if _scratch_link(session_path) or _scratch_link(capture_path):
            return False
        session = json.loads(session_path.read_bytes())
        capture = json.loads(capture_path.read_bytes())
        ended = datetime.fromisoformat(session["finished_at_utc"])
        if (session.get("pid") != launch["pid"] or session.get("passed") is not True
                or session.get("exit_code") != 0 or capture.get("capture_complete") is not True
                or capture.get("target_execution") is not False or ended.tzinfo is None
                or ended.timestamp() >= datetime.now(timezone.utc).timestamp()):
            return False
        from reverse_pending import coordinator_instance_absence
        return coordinator_instance_absence(session).get("confirmed_absent") is True
    except (KeyError, TypeError, ValueError, OSError):
        return False


def _clean_history_file(reference, *, contents=False):
    """Bind historical-field evidence to exact retained, non-linked file bytes."""
    name = reference["path"]
    if (not isinstance(name, str) or "\\" in name or Path(name).is_absolute()
            or ".." in Path(name).parts or not re.fullmatch(r"[a-f0-9]{64}", reference["sha256"])):
        raise ValueError("Historical capture evidence has an invalid identity")
    path = safe(ROOT / name)
    for part in (path, *path.parents):
        if _scratch_link(part):
            raise ValueError("Historical capture evidence contains a link")
        if part == ROOT:
            break
    actual = _clean_file_identity(path)
    if actual["sha256"] != reference["sha256"] or (
            "bytes" in reference and actual["bytes"] != reference["bytes"]):
        raise ValueError("Historical capture evidence bytes changed")
    raw = path.read_bytes() if contents else None
    after = path.stat()
    if ((after.st_size, after.st_mtime_ns, after.st_ino, after.st_dev) != tuple(
            actual[key] for key in ("bytes", "mtime_ns", "inode", "device"))
            or contents and (len(raw) != actual["bytes"] or hashlib.sha256(raw).hexdigest() != actual["sha256"])):
        raise ValueError("Historical capture evidence changed during verification")
    return actual, raw


def _clean_formal_capture_history(document, owner, raw, candidates, *, bind_history):
    """Release only exact exited CLI log inventories in frozen capture receipts."""
    from reverse_pending import coordinator_instance_absence

    result = {}
    if (not isinstance(document, dict) or document.get("schema_version") != 1
            or document.get("review_kind") != "frozen_unresolved_formal_static_capture_execution"
            or document.get("target_execution") is not False
            or document.get("source_guard_unchanged") is not True
            or document.get("all_capture_clis_succeeded") is not True
            or not isinstance(document.get("jobs"), list) or not document["jobs"]
            or owner.parent != ROOT / ".analysis/results/reverse-pending/review-sources"
            or owner.name != hashlib.sha256(raw).hexdigest() + ".json"):
        return result
    try:
        common = [_clean_history_file({"path": relative(owner), "bytes": len(raw),
                                      "sha256": hashlib.sha256(raw).hexdigest()})[0]]
        for field in ("preflight_batch", "coordinator_source"):
            reference = document[field]
            if not reference["path"].startswith(".analysis/results/reverse-pending/review-sources/"):
                return result
            common.append(_clean_history_file(reference)[0])
    except (KeyError, TypeError, ValueError, OSError):
        return result
    for job in document["jobs"]:
        try:
            lines = job["stdout_json_lines"]
            if (type(job["exit_code"]) is not int or job["exit_code"] != 0
                    or job.get("capture_cli_succeeded") is not True or job.get("stderr") is not None
                    or not isinstance(lines, list) or len(lines) != 2
                    or set(lines[0]) != {"pid", "result", "log"}
                    or set(lines[1]) != {"session", "passed"} or lines[1]["passed"] is not True
                    or type(lines[0]["pid"]) is not int or lines[0]["pid"] <= 0):
                continue
            output = lines[0]["result"]
            match = re.fullmatch(r"\.analysis/results/native/([a-f0-9]{64})/supplements/"
                                 r"([a-f0-9]{16}(?:-[a-f0-9]{16})?)", output)
            if match is None or job["input_sha256"] != match[1]:
                continue
            name = output.replace(".analysis/results/native/", ".work/native/", 1)
            row = candidates.get(name)
            if (row is None or lines[0]["log"] != name + "/ida.log"
                    or lines[1]["session"] != output + "/session.json"):
                continue
            stdout = job["stdout"]
            if (not isinstance(stdout, dict) or not stdout["path"].startswith(
                    ".analysis/results/reverse-pending/review-sources/")):
                continue
            stdout_identity, stdout_raw = _clean_history_file(stdout, contents=True)
            if [json.loads(line) for line in stdout_raw.decode("utf-8").splitlines() if line.strip()] != lines:
                continue
            basis = common + [stdout_identity]
            owned = {item["path"]: item for item in row["basis"]}
            evidence = {}
            for filename in ("plan.json", "session.json", "capture.json"):
                expected = owned[output + "/" + filename]
                actual, body = _clean_history_file(expected, contents=True)
                if actual != expected:
                    raise ValueError("Completion basis changed during reference scan")
                evidence[filename] = json.loads(body)
            plan, session, capture = (evidence[key] for key in ("plan.json", "session.json", "capture.json"))
            argv = job["argv"]
            if (not isinstance(argv, list) or not all(isinstance(item, str) for item in argv)
                    or argv.count("tools/workspace.py") != 1 or argv.count("supplement-native") != 1
                    or argv.index("supplement-native") != argv.index("tools/workspace.py") + 1
                    or argv.count("--input") != 1 or argv[argv.index("--input") + 1] != plan["input"]
                    or plan["output"] != output or plan["sha256"] != match[1]
                    or capture["input_sha256"] != match[1] or capture["input"] != plan["input"]
                    or session.get("state") != "finished" or session.get("passed") is not True
                    or type(session.get("exit_code")) is not int or session["exit_code"] != 0
                    or session.get("target_execution") is not False or plan.get("target_execution") is not False
                    or session.get("process_released") is not True or session.get("source_identity_rechecked") is not True
                    or capture.get("capture_complete") is not True or capture.get("all_pseudocode_captured") is not True
                    or capture.get("target_execution") is not False or capture.get("base_capture_unchanged") is not True
                    or capture.get("base_database_unchanged") is not True or session["pid"] != lines[0]["pid"]
                    or type(session.get("coordinator_pid")) is not int or session["coordinator_pid"] <= 0):
                continue
            times = [datetime.fromisoformat(value) for value in (session["started_at_utc"],
                     capture["started_at_utc"], capture["finished_at_utc"], session["finished_at_utc"])]
            if (any(value.tzinfo is None for value in times) or times != sorted(times)
                    or times[-1] >= datetime.now(timezone.utc)):
                continue
            processes = [session, {"pid": session["coordinator_pid"], "started_at_utc": session["started_at_utc"]}]
            if any(coordinator_instance_absence(process).get("confirmed_absent") is not True for process in processes):
                continue
            if not plan["input"].startswith(".analysis/inputs/"):
                continue
            basis.append(_clean_history_file({"path": plan["input"], "bytes": plan["input_bytes"],
                                              "sha256": plan["sha256"]})[0])
            for field in ("base_capture", "source_database"):
                reference = plan[field]
                prefix = ".analysis/results/native/" + match[1] + "/" if field == "base_capture" else (
                         ".work/native/" + match[1] + "/batch/")
                if capture[field] != reference or not reference["path"].startswith(prefix):
                    raise ValueError("Historical capture source dependency is not independently retained")
                basis.append(_clean_history_file(reference)[0])
            if not _clean_supplement_attempt(ROOT / output):
                continue
            # Only this job's structured stdout is historical; source/recovery fields remain visible.
            previous = {item["path"]: item for item in row["basis"]}
            if any(item["path"] in previous and previous[item["path"]] != item for item in basis):
                continue
            if bind_history:
                previous.update((item["path"], item) for item in basis)
                row["basis"] = sorted(previous.values(), key=lambda item: item["path"])
            elif any(previous.get(item["path"]) != item for item in basis):
                continue
            result[id(job)] = name
        except (KeyError, IndexError, TypeError, ValueError, OSError):
            continue
    return result


def _clean_json_dependencies(value, *, own_receipt=False, field=None, ended_batch=False, historical_job=False,
                             formal_stdout_jobs=(), verification_owners=(), released_checkouts=()):
    """Keep dependency fields while excluding explicitly historical inventory facts."""
    if isinstance(value, dict):
        if value.get("mode") in {"scratch_cleanup_plan", "scratch_cleanup_apply"}:
            return
        for key, child in value.items():
            if key == "directory" and id(child) in verification_owners:
                continue
            if key == "scratch_cleanup" or own_receipt and key in {"database", "argv", "retained_private_database"}:
                continue
            if (key == "verification_checkout_release" and own_receipt and isinstance(child, dict)
                    and child.get("checkout", {}).get("path") in released_checkouts):
                continue
            if (key == "verification_checkout_release_history" and own_receipt and isinstance(child, list)
                    and all(isinstance(item, dict) and item.get("checkout", {}).get("path") in released_checkouts for item in child)):
                continue
            if historical_job and key in {"scratch", "argv", "command", "log"}:
                continue
            if key == "stdout" and _clean_historical_native_stdout(child):
                continue
            if key == "stdout_json_lines" and id(value) in formal_stdout_jobs:
                continue
            yield from _clean_json_dependencies(child, own_receipt=own_receipt and key == "previous_attempts",
                                                field=key, historical_job=ended_batch and key == "jobs",
                                                formal_stdout_jobs=formal_stdout_jobs,
                                                verification_owners=verification_owners, released_checkouts=released_checkouts)
    elif isinstance(value, list):
        for child in value:
            yield from _clean_json_dependencies(child, field=field, own_receipt=own_receipt,
                                                historical_job=historical_job, formal_stdout_jobs=formal_stdout_jobs,
                                                verification_owners=verification_owners, released_checkouts=released_checkouts)
    elif isinstance(value, str):
        yield value


def _clean_stream_json_dependencies(path):
    """Read large pretty or compact receipts without retaining their object graphs."""
    stack, root_done = [], False
    scalar = re.compile(r'(?:-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?|true|false|null)')
    whitespace = re.compile(r'\s*')
    with path.open("r", encoding="utf-8") as stream:
        for line in stream:
            offset = 0
            while offset < len(line):
                offset = whitespace.match(line, offset).end()
                if offset == len(line):
                    break
                char = line[offset]
                if char == '"':
                    token, following = json.decoder.scanstring(line, offset + 1)
                    kind = "string"
                elif char in "{}[]:,":
                    token, following, kind = char, offset + 1, char
                else:
                    match = scalar.match(line, offset)
                    if match is None:
                        raise ValueError("Malformed retained JSON token")
                    token, following, kind = None, match.end(), "scalar"
                offset = following
                if stack:
                    frame = stack[-1]
                    state = frame["state"]
                    if state in {"key", "key_or_end"}:
                        if kind == "}" and state == "key_or_end":
                            stack.pop()
                            continue
                        if kind != "string":
                            raise ValueError("Malformed retained JSON key")
                        frame.update(key=token, state="colon")
                        continue
                    if state == "colon":
                        if kind != ":":
                            raise ValueError("Malformed retained JSON colon")
                        frame["state"] = "value"
                        continue
                    if state == "comma":
                        closing = "}" if frame["object"] else "]"
                        if kind == closing:
                            stack.pop()
                        elif kind == ",":
                            frame["state"] = "key" if frame["object"] else "value"
                        else:
                            raise ValueError("Malformed retained JSON separator")
                        continue
                    if state == "value_or_end" and kind == "]":
                        stack.pop()
                        continue
                    ignored = frame["ignored"] or frame.get("key") == "scratch_cleanup"
                    frame["state"] = "comma"
                else:
                    if root_done:
                        raise ValueError("Multiple retained JSON roots")
                    root_done, ignored = True, False
                if kind in {"{", "["}:
                    stack.append({"object": kind == "{", "state": "key_or_end" if kind == "{" else "value_or_end",
                                  "ignored": ignored, "key": None})
                elif kind == "string":
                    if not ignored and re.search(r"\.work|target[/\\]verification-temp", token, re.IGNORECASE):
                        yield token
                elif kind != "scalar":
                    raise ValueError("Malformed retained JSON value")
    if stack or not root_done:
        raise ValueError("Truncated retained JSON receipt")


def _clean_dependency_mentions(name, value):
    normalized = value.replace("\\", "/").casefold()
    name = name.casefold()
    if name in normalized:
        return True
    if name.startswith(".work/native/"):
        for match in re.finditer(r'\.work/[^\s"\'<>?*#;,:()\[\]{}]+', normalized):
            parent = match[0].rstrip("/.")
            if len(parent.split("/")) >= 3 and name.startswith(parent + "/"):
                return True
    return False


def _clean_references(candidates, *, exclude=(), bind_history=False):
    """Search every maintained text owner and receipt, including frozen reviews."""
    if not candidates:
        return {}, []
    executable = shutil.which("rg")
    if executable is None:
        return {}, ["Fixed-string reference scanner rg is unavailable"]
    names = {row["path"]: row for row in candidates}
    arguments = [executable, "--no-config", "--hidden", "--no-ignore", "--threads", "16",
                 "--fixed-strings", "--ignore-case", "--only-matching", "--with-filename",
                 "--no-line-number", "--no-heading", "--color", "never",
                 "--path-separator", "/"]
    for extension in ("json", "md", "py", "rs", "toml", "yml", "yaml", "ps1", "cmd", "c", "h", "asm"):
        arguments.extend(["--glob", "*." + extension])
    for directory in (".ref", ".git", "target", "node_modules", "__pycache__"):
        arguments.extend(["--glob", "!**/" + directory + "/**"])
    for name in (".work/native/*/supplements/**", ".work/asset-releases/uploads/**", *exclude):
        arguments.extend(["--glob", "!" + name])
    # Raw official bytes cannot depend on our scratch. Acquisition receipts remain in scope.
    for directory in ("raw", "raw-by-url", "source", "sources", "decompiled"):
        arguments.extend(["--glob", "!.analysis/inputs/**/" + directory + "/**"])
    paths = [name for name in ("docs", "tools", ".agents", "crates", ".analysis", ".work") if (ROOT / name).exists()]
    paths.extend(path.name for path in ROOT.iterdir() if path.is_file()
                 and path.suffix.lower() in {".md", ".json", ".toml", ".py"})
    references, errors = {}, []
    pattern_owners = {}
    for name in names:
        prefixes = [name]
        if name.startswith(".work/native/"):
            prefixes.append("/".join(name.split("/")[:3]))
        for prefix in prefixes:
            for pattern in (prefix, prefix.replace("/", "\\"), prefix.replace("/", "\\\\")):
                pattern_owners.setdefault(pattern.casefold(), set()).add(name)
    with tempfile.TemporaryDirectory(prefix="razer-clean-references-") as temporary:
        pattern = Path(temporary) / "paths.txt"
        pattern.write_text("\n".join(pattern_owners) + "\n", encoding="utf-8")
        arguments.extend(["--file", str(pattern), "--", *paths])
        options = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}
        result = subprocess.run(arguments, cwd=ROOT, capture_output=True, timeout=1800, **options)
    if result.returncode not in (0, 1):
        return {}, ["Complete cleanup reference scan failed: " + result.stderr.decode("utf-8", errors="replace")[:1000]]
    matched = {}
    for line in result.stdout.decode("utf-8").splitlines():
        filename, match = line.rsplit(":", 1)
        owners = pattern_owners.get(match.casefold())
        if not owners:
            return {}, ["Reference scanner returned an unknown cleanup path"]
        matched.setdefault(filename, set()).update(owners)
    for filename, matched_names in matched.items():
        path = safe(ROOT / filename)
        before = path.stat()
        if _scratch_link(path):
            errors.append("Linked reference owner: " + filename)
            continue
        if before.st_size > 64 * 1024 * 1024 and path.suffix == ".json":
            try:
                remaining = set(matched_names)
                for value in _clean_stream_json_dependencies(path):
                    found = {name for name in remaining if _clean_dependency_mentions(name, value)}
                    for name in found:
                        references.setdefault(name, []).append(filename)
                    remaining.difference_update(found)
                after = path.stat()
                if (before.st_size, before.st_mtime_ns, before.st_ino) != (after.st_size, after.st_mtime_ns, after.st_ino):
                    errors.append("Reference owner changed during scan: " + filename)
            except (ValueError, UnicodeError) as error:
                errors.append("Invalid retained large JSON owner: " + filename + ": " + str(error))
            continue
        raw = path.read_bytes()
        try:
            document = json.loads(raw) if path.suffix == ".json" else None
        except (ValueError, UnicodeError):
            errors.append("Malformed retained JSON reference owner: " + filename)
            continue
        formal_history = _clean_formal_capture_history(document, path, raw, names, bind_history=bind_history)
        verification_owners = set()
        if filename == VERIFICATION_TEMP_LEDGER:
            try:
                if document != _verification_ledger(ROOT):
                    raise ValueError("Verification ledger changed during reference scan")
                for owner in document["records"]:
                    if owner["directory"]["path"] not in names:
                        continue
                    if owner["registration"] == "explicit_legacy_recovery":
                        _verification_recovery_source(owner["recovery"])
                    verification_owners.add(id(owner["directory"]))
            except (OSError, ValueError, KeyError, TypeError) as error:
                errors.append("Invalid verification historical owner: " + str(error))
        for name in matched_names:
            row = names[name]
            ended_batch = (isinstance(document, dict)
                           and re.fullmatch(r"\.work/reverse-pending/[^/]+-capture-batch\.json", filename)
                           and _clean_capture_batch_ended(document, datetime.now(timezone.utc).timestamp()))
            strings = (_clean_json_dependencies(document, own_receipt=filename in row["owner_receipts"]
                                                 and row.get("kind") != "owned_empty_verification_temp",
                                                 ended_batch=ended_batch, formal_stdout_jobs={
                                                      job for job, target in formal_history.items() if target == name},
                                                 verification_owners=verification_owners,
                                                 released_checkouts={name} if row.get("kind") == "old_asset_upload_clone" else ())
                       if document is not None else [raw.decode("utf-8")])
            if any(_clean_dependency_mentions(name, value) for value in strings):
                references.setdefault(name, []).append(filename)
        after = path.stat()
        if (before.st_size, before.st_mtime_ns, before.st_ino) != (after.st_size, after.st_mtime_ns, after.st_ino):
            errors.append("Reference owner changed during scan: " + filename)
    return references, errors


def _clean_process_guard(candidates):
    processes, errors = _scratch_processes()
    references = {}
    for row in candidates:
        name = row["path"].casefold()
        for process in processes:
            command = process["command_line"].replace("\\", "/").casefold()
            first = re.match(r'^\s*(?:"([^"]+)"|(\S+))', command)
            executable = (process.get("executable") or (next((item for item in first.groups() if item), "")
                                                       if first else "")).replace("\\", "/").rsplit("/", 1)[-1].casefold()
            upload_git_active = (row.get("kind") == "old_asset_upload_clone" and process["pid"] != os.getpid()
                                 and re.fullmatch(r"git(?:-[a-z0-9_-]+)?(?:\.exe)?", executable))
            verification_active = (row.get("kind") == "owned_empty_verification_temp" and process["pid"] != os.getpid()
                                   and (re.search(r"(?:^|[/\s\"])(?:cargo|rustc|ml64)(?:\.exe)?(?:[\s\"]|$)", command)
                                        or "workspace.py" in command and "validate-rust" in command
                                        or "test_evidence" in command or "native_source_correspondence" in command))
            if name in command or verification_active or upload_git_active:
                references.setdefault(row["path"], []).append(process["pid"])
    return references, errors


@contextmanager
def _clean_locks():
    """Refuse an active analysis coordinator or resource writer without waiting."""
    import msvcrt
    streams = []
    try:
        for name in (".analysis/reverse-pending.lock", ".work/resource-writer.lock", VERIFICATION_TEMP_LOCK):
            path = safe(ROOT / name)
            path.parent.mkdir(parents=True, exist_ok=True)
            stream = path.open("a+b")
            streams.append(stream)
            if not stream.seek(0, os.SEEK_END):
                stream.write(b"\0")
                stream.flush()
            stream.seek(0)
            msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
        yield
    finally:
        for stream in reversed(streams):
            stream.close()


def _clean_group_final_guard(row, directory_identities):
    """Recheck membership under the held handles; this is not a lock on new child creation."""
    expected_files = {item["path"]: item for item in row["tree"]["files"]}
    directories, files = set(), set()
    for parent, children, leaves in os.walk(_clean_path(row["path"]), followlinks=False):
        for path in [Path(parent), *(Path(parent) / name for name in children + leaves)]:
            _clean_path(relative(path))
            if _scratch_link(path):
                raise ValueError("Cleanup membership contains a new link or reparse point")
        directories.add(relative(Path(parent)))
        files.update(relative(Path(parent) / name) for name in leaves)
    if directories != set(row["tree"]["directories"]) or files != expected_files.keys():
        raise ValueError("Cleanup membership changed before the first deletion")
    for name in directories:
        stat = _clean_path(name).stat()
        if (stat.st_dev, stat.st_ino, stat.st_mtime_ns) != directory_identities[name]:
            raise ValueError("Cleanup directory identity changed before the first deletion")
    for name, expected in expected_files.items():
        stat = _clean_path(name).stat()
        if (stat.st_size, stat.st_mtime_ns, stat.st_ino, stat.st_dev) != tuple(
                expected[key] for key in ("bytes", "mtime_ns", "inode", "device")):
            raise ValueError("Cleanup file identity changed before the first deletion")
    active, errors = _clean_process_guard([row])
    if active or errors:
        raise ValueError("Cleanup activity guard changed before deletion")


def _clean_delete_group(row):
    """Hold each exact file exclusively until the complete group is validated."""
    import ctypes
    from ctypes import wintypes
    from analysis_storage import FileInfo, CompressionInfo
    api = ctypes.WinDLL("kernel32", use_last_error=True)
    api.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, wintypes.LPVOID,
                               wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    api.CreateFileW.restype = wintypes.HANDLE
    api.ReadFile.argtypes = [wintypes.HANDLE, wintypes.LPVOID, wintypes.DWORD,
                            ctypes.POINTER(wintypes.DWORD), wintypes.LPVOID]
    api.SetFileInformationByHandle.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD]
    api.GetFileInformationByHandle.argtypes = [wintypes.HANDLE, ctypes.POINTER(FileInfo)]
    api.GetFileInformationByHandleEx.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD]
    api.CloseHandle.argtypes = [wintypes.HANDLE]
    handles, directory_handles, allocations, marked, failure = [], [], [], [], None
    directory_identities = {}
    try:
        ancestors = set()
        for name in row["tree"]["directories"]:
            path = _clean_path(name)
            ancestors.add(path)
            ancestors.update(parent for parent in path.parents if parent.is_relative_to(ROOT / ".work"))
        for path in sorted(ancestors, key=lambda value: len(value.parts)):
            stat = path.stat()
            identity = (stat.st_dev, stat.st_ino, stat.st_mtime_ns)
            handle = api.CreateFileW(str(path), 0x80, 1, None, 3, 0x02200000, None)
            if handle == ctypes.c_void_p(-1).value:
                raise OSError(ctypes.get_last_error(), "exclusive_directory_guard_unavailable")
            directory_handles.append(handle)
            after = path.stat()
            if (after.st_dev, after.st_ino, after.st_mtime_ns) != identity:
                raise ValueError("Cleanup directory changed while acquiring its handle")
            if relative(path) in row["tree"]["directories"]:
                directory_identities[relative(path)] = identity
            if _scratch_link(path):
                raise ValueError("Cleanup directory changed to a reparse point")
        for expected in row["tree"]["files"]:
            path = _clean_path(expected["path"])
            handle = api.CreateFileW(str(path), 0x80010000, 0, None, 3, 0x00200000, None)
            if handle == ctypes.c_void_p(-1).value:
                raise OSError(ctypes.get_last_error(), "exclusive_file_guard_unavailable")
            handles.append(handle)
            stat = path.stat()
            if (stat.st_size, stat.st_mtime_ns, stat.st_ino, stat.st_dev) != tuple(
                    expected[key] for key in ("bytes", "mtime_ns", "inode", "device")):
                raise ValueError("Cleanup file changed before exclusive deletion")
            checksum, length = hashlib.sha256(), 0
            buffer, count = ctypes.create_string_buffer(1024 * 1024), wintypes.DWORD()
            while True:
                if not api.ReadFile(handle, buffer, len(buffer), ctypes.byref(count), None):
                    raise ctypes.WinError(ctypes.get_last_error())
                if not count.value:
                    break
                checksum.update(buffer.raw[:count.value])
                length += count.value
            if length != expected["bytes"] or checksum.hexdigest() != expected["sha256"]:
                raise ValueError("Cleanup bytes changed before exclusive deletion")
            info, compressed = FileInfo(), CompressionInfo()
            if (not api.GetFileInformationByHandle(handle, ctypes.byref(info))
                    or not api.GetFileInformationByHandleEx(handle, 8, ctypes.byref(compressed), ctypes.sizeof(compressed))):
                raise ctypes.WinError(ctypes.get_last_error())
            if info.links != 1 or compressed.allocated < 0:
                raise ValueError("Cleanup target has additional hard links or invalid allocation")
            allocations.append(compressed.allocated)
        _clean_group_final_guard(row, directory_identities)
        for handle, expected, allocated in zip(handles, row["tree"]["files"], allocations):
            disposition = wintypes.DWORD(0x11)  # DELETE | IGNORE_READONLY_ATTRIBUTE, including Git pack files.
            if not api.SetFileInformationByHandle(handle, 21, ctypes.byref(disposition), ctypes.sizeof(disposition)):
                raise ctypes.WinError(ctypes.get_last_error())
            marked.append({**expected, "allocated_bytes": allocated})
    except (OSError, ValueError) as error:
        failure = error
    finally:
        for handle in reversed(handles + directory_handles):
            api.CloseHandle(handle)
    if failure is None:
        try:
            for name in sorted(row["tree"]["directories"], key=len, reverse=True):
                path = _clean_path(name)
                path.rmdir()  # Empty directories only; a concurrent new file prevents removal.
        except (OSError, ValueError) as error:
            failure = error
    if failure is not None:
        failure.cleanup_removed = [item for item in marked if not (ROOT / item["path"]).exists()]
        raise failure
    if (ROOT / row["path"]).exists() or any((ROOT / item["path"]).exists() for item in marked):
        raise ValueError("Cleanup target still exists after exclusive deletion")
    return {"allocated_before": sum(allocations), "allocated_after": 0, "bytes_reclaimed": sum(allocations)}


def _clean_receipt_path(name, *, applied=False):
    path = ROOT / name
    suffix = r"-plan-applied\.json" if applied else r"-plan\.json"
    if (not isinstance(name, str) or "\\" in name or Path(name).is_absolute()
            or path.parent != ROOT / ".analysis/results"
            or not re.fullmatch(r"scratch-cleanup(?:-[A-Za-z0-9_-]+)?" + suffix, path.name)):
        raise ValueError("Cleanup receipt must use a reserved top-level scratch-cleanup plan name")
    for ancestor in (ROOT / ".analysis", path.parent, path):
        if ancestor.exists() and _scratch_link(ancestor):
            raise ValueError("Cleanup receipt contains a link or reparse point")
    return safe(path)


def _clean_create_receipt(path, report):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(report, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


def clean(stale=False, *, before=None, dry_run=False, plan=None, apply=None, plan_sha256=None,
          verification_recovery=None):
    """Plan conservative scratch cleanup; deletion requires an unchanged explicit plan."""
    if stale:
        if dry_run or plan or apply or plan_sha256 or verification_recovery is not None:
            raise ValueError("--stale cannot be combined with planned cleanup")
        return _clean_stale(before)
    if apply:
        if dry_run or plan or verification_recovery is not None or not re.fullmatch(r"[a-f0-9]{64}", plan_sha256 or ""):
            raise ValueError("Cleanup apply requires only --apply and --plan-sha256")
        source = _clean_receipt_path(apply)
        raw = source.read_bytes()
        if hashlib.sha256(raw).hexdigest() != plan_sha256:
            raise ValueError("Cleanup plan bytes changed after review")
        report = json.loads(raw)
        if (report.get("schema_version") != 1 or report.get("mode") != "scratch_cleanup_plan"
                or report.get("workspace") != str(ROOT.resolve()) or report.get("guard_failures")):
            raise ValueError("Cleanup plan has invalid ownership or failed guards")
        with _clean_locks():
            rows = report["candidates"]
            current, _ = _clean_candidates()
            verification_rows, _ = _verification_candidates()
            current.extend(verification_rows)
            current_names = {row["path"]: row for row in current}
            for row in rows:
                if row.get("kind") not in {"ended_supplement_private_copy", "old_asset_upload_clone", "owned_empty_verification_temp"}:
                    raise ValueError("Cleanup plan contains an unowned target")
                name = row["path"]
                if row["kind"] == "owned_empty_verification_temp":
                    allowed, tree = True, _verification_tree(name)
                    owner = current_names.get(name, {})
                    if any(row.get(key) != owner.get(key) for key in ("basis", "owner_receipts", "ownership")):
                        raise ValueError("Verification cleanup plan omitted or changed ownership evidence")
                else:
                    allowed = (re.fullmatch(r"\.work/native/[a-f0-9]{64}/supplements/[a-f0-9]{16}(?:-[a-f0-9]{16})?", name)
                           if row["kind"] == "ended_supplement_private_copy" else re.fullmatch(
                               r"\.work/asset-releases/uploads/assets-[a-f0-9]{20}-[A-Za-z0-9_-]+", name))
                    tree = _clean_tree(name)
                if not allowed or tree != row["tree"]:
                    raise ValueError("Cleanup target changed or escaped its owned scope")
                if name not in current_names:
                    raise ValueError("Cleanup target acquired a current analysis reservation or lost completion ownership")
                if row["kind"] == "old_asset_upload_clone":
                    owner = current_names[name]
                    if (row.get("rebuild_evidence") != owner.get("rebuild_evidence")
                            or row.get("owner_receipts") != owner.get("owner_receipts")
                            or not all(identity in row.get("basis", []) for identity in owner["basis"])):
                        raise ValueError("Upload cleanup omitted or changed its completed rebuild evidence")
                for basis in row["basis"]:
                    if _clean_file_identity(safe(ROOT / basis["path"])) != basis:
                        raise ValueError("Cleanup completion evidence changed after review")
            references, errors = _clean_references(rows, exclude=(relative(source),))
            active, process_errors = _clean_process_guard(rows)
            if references or errors or active or process_errors:
                raise ValueError("Cleanup dependency or activity guards changed after review")
            receipt = {"schema_version": 1, "mode": "scratch_cleanup_apply", "plan": relative(source),
                       "plan_sha256": plan_sha256, "scope": report["candidates"], "removed": [],
                       "failures": [], "target_execution": False, "status": "started"}
            receipt_path = _clean_receipt_path(relative(source.with_name(source.stem + "-applied.json")), applied=True)
            _clean_create_receipt(receipt_path, receipt)
            for row in rows:
                try:
                    allocation = (_verification_delete_empty(row) if row["kind"] == "owned_empty_verification_temp"
                                  else _clean_delete_group(row))
                    receipt["removed"].append({"path": row["path"], "bytes": row["tree"]["bytes"],
                                               "files": len(row["tree"]["files"]), **(allocation or {})})
                except (OSError, ValueError) as error:
                    partial = getattr(error, "cleanup_removed", [])
                    receipt["failures"].append({"path": row["path"], "reason": str(error),
                        "partial_removed_files": partial, "partial_removed_bytes": sum(item["bytes"] for item in partial),
                        "partial_removed_allocated_bytes": sum(item.get("allocated_bytes", 0) for item in partial)})
                    write_json(receipt_path, receipt)
                    try:
                        _verification_finish_cleanup(receipt)
                    except (OSError, ValueError) as finalize_error:
                        receipt["failures"].append({"path": VERIFICATION_TEMP_LEDGER,
                            "reason": "ownership finalization failed: " + str(finalize_error),
                            "partial_removed_files": [], "partial_removed_bytes": 0,
                            "partial_removed_allocated_bytes": 0})
                        write_json(receipt_path, receipt)
                    raise
                write_json(receipt_path, receipt)
            try:
                _verification_finish_cleanup(receipt)
            except (OSError, ValueError) as finalize_error:
                receipt["failures"].append({"path": VERIFICATION_TEMP_LEDGER,
                    "reason": "ownership finalization failed: " + str(finalize_error),
                    "partial_removed_files": [], "partial_removed_bytes": 0,
                    "partial_removed_allocated_bytes": 0})
                write_json(receipt_path, receipt)
                raise
            receipt["status"] = "completed"
            write_json(receipt_path, receipt)
            print(json.dumps({"receipt": relative(receipt_path), "removed": len(receipt["removed"]),
                              "bytes": sum(row["bytes"] for row in receipt["removed"]),
                              "bytes_reclaimed": sum(row.get("bytes_reclaimed", 0) for row in receipt["removed"])}), flush=True)
            return receipt
    if plan_sha256:
        raise ValueError("--plan-sha256 requires --apply")
    if verification_recovery is not None:
        _verification_register_recovery(verification_recovery)
    default_name = ".analysis/results/scratch-cleanup-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ") + "-plan.json"
    destination = _clean_receipt_path(plan or default_name)
    if destination.exists():
        raise FileExistsError("Cleanup plan already exists; choose a new reserved receipt name")
    rows, retained = _clean_candidates(before)
    verification_rows, verification_retained = _verification_candidates()
    rows.extend(verification_rows)
    retained.extend(verification_retained)
    references, errors = _clean_references(rows, bind_history=True)
    active, process_errors = _clean_process_guard(rows)
    eligible = []
    for row in rows:
        name = row["path"]
        if name in references or name in active:
            retained.append({"path": name, "reason": "referenced_or_active", "references": references.get(name, []),
                             "pids": active.get(name, [])})
        elif not errors and not process_errors:
            row["tree"] = (_verification_tree(name) if row["kind"] == "owned_empty_verification_temp"
                           else _clean_tree(name))
            eligible.append(row)
    report = {"schema_version": 1, "mode": "scratch_cleanup_plan", "workspace": str(ROOT.resolve()),
              "recorded_at_utc": datetime.now(timezone.utc).isoformat(), "target_execution": False,
               "protected": [".analysis/inputs", ".analysis/results", "target/verification-temp",
                             ".work/native/*/batch",
                            ".work/native/*/batch/invalid-database", ".work/reverse-pending"],
              "reference_scope": "all maintained text owners and JSON receipts, including frozen reviews; raw original bodies excluded",
              "historical_fields": ["scratch_cleanup", "ended supplement owner's database and argv",
                                    "exited capture-batch job scratch/argv/command/log",
                                    "verified exited native launch/completion stdout",
                                     "source-bound frozen capture job structured stdout log inventory",
                                     "validated verification ledger directory ownership field"],
              "guard_failures": errors + process_errors, "candidates": eligible, "retained": retained,
              "candidate_bytes": sum(row["tree"]["bytes"] for row in eligible)}
    _clean_create_receipt(destination, report)
    print(json.dumps({"mode": "dry_run", "plan": relative(destination), "sha256": digest(destination),
                      "candidate_directories": len(eligible), "candidate_bytes": report["candidate_bytes"],
                      "retained": len(retained), "guard_failures": report["guard_failures"]}), flush=True)
    return report


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
    reverse_pending_current = {}
    if (ROOT / ".analysis/results/reverse-pending/unresolved-current.json").is_file():
        import reverse_pending
        reverse_pending_current["reverse_pending_current"] = reverse_pending.verify_current_unresolved()
    return {"analysis_artifacts_verified": len(verified), "native_function_bodies_verified": native_functions,
            "frontend_manifests_verified": frontend_manifests, "rust_validation_current": rust_validation_current,
            "analysis_verification_errors": validation_errors, **reverse_pending_current}


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


def product_input(path, source):
    """Resolve every embedded edge inside the installed product roots."""
    path = safe(path)
    parts = path.relative_to(ROOT).parts
    allowed = ("app", "crates", "assets")
    if not parts or parts[0] not in allowed or any(
            part in (".analysis", ".work", ".ref", "target") for part in parts):
        raise ValueError(f"Product include is outside installed inputs: {relative(source)} -> {relative(path)}")
    return path


def rust_inputs(*, include_asset_pack=True):
    """Follow the complete embedded resource closure, independent of analysis inputs.

    In particular, assets/*.rs embed images themselves. Hashing only the first
    include would miss a changed image and would not prove a portable checkout.
    Provenance JSON source fields are descriptions, never filesystem edges.
    """
    import resource_database
    import windows_installer
    files = {ROOT / "Cargo.toml", ROOT / "Cargo.lock"}
    for relative_path in (".cargo/config", ".cargo/config.toml", "rust-toolchain", "rust-toolchain.toml"):
        configuration = safe(ROOT / relative_path)
        if configuration.is_file():
            files.add(configuration)
    if (ROOT / "crates/razer-resource-db/Cargo.toml").is_file():
        files.update({ROOT / resource_database.INSTALLATION, ROOT / resource_database.OUTPUT})
    driver_owner = ROOT / "crates/razer-platform/src/observation/hardware_driver.rs"
    if driver_owner.is_file():
        driver = windows_installer.hardware_driver_policy(root=ROOT)
        files.update({ROOT / windows_installer.INSTALLATION,
                      product_input(ROOT / driver["source"], driver_owner)})
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
        if path == driver_owner:
            # The platform build script emits installation names only. Track
            # its policy and delivered driver bytes, not a machine OUT_DIR.
            generated = 'include!(concat!(env!("OUT_DIR"), "/hardware_driver_installation.rs"));'
            if text.count(generated) != 1:
                raise ValueError("Hardware driver constants need a new build-output audit")
            text = text.replace(generated, "")
        for kind, literal in rust_embed_references(text):
            if re.match(r"(?:[A-Za-z]:|[/\\])", literal):
                raise ValueError(f"Nonportable include in {relative(path)}: {literal}")
            if kind == "locale":
                raise ValueError(f"Product words must use the installed locale snapshot: {relative(path)}")
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
    sys.path.insert(0, str(ROOT / "tools/analyzers/python-static"))
    import json5

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
        properties = {} if attributes == "null" else json5.loads(attributes)
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


def static_svg_group_visibility(content, visibility):
    """Materialize source-owned visibility without changing SVG geometry."""
    if not isinstance(visibility, dict) or not visibility or any(
            not isinstance(name, str) or not name or value not in {"visible", "hidden"}
            for name, value in visibility.items()):
        raise ValueError("SVG group visibility requires explicit named visible/hidden states")
    root = ET.fromstring(content)
    for name, value in visibility.items():
        matches = [node for node in root.iter() if node.get("id") == name]
        if len(matches) != 1 or matches[0].tag.rsplit("}", 1)[-1] != "g":
            raise ValueError("SVG visibility group is absent, ambiguous or not a group: " + name)
        node = matches[0]
        style = node.get("style", "")
        properties = [field for field in style.split(";") if field and field.split(":", 1)[0].strip() != "visibility"]
        node.set("style", ";".join([*properties, "visibility:" + value]))
    return ET.tostring(root, encoding="utf-8", short_empty_elements=False), {
        "operation": "Materialize caller-selected visibility on unique source SVG groups; preserve every geometry node",
        "groups": visibility, "target_execution": False,
    }


def muted_video_display(source):
    """Decode source autoplay/muted/loop media into timed lossless RGBA frames."""
    import io
    sys.path.insert(0, str(ROOT / "tools/analyzers/media-python"))
    sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
    import av
    import PIL
    from PIL import Image, features
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
        frames[0].save(buffer, format="WEBP", save_all=True, append_images=frames[1:], duration=durations, loop=0, lossless=True, quality=100, method=0)
        content = buffer.getvalue()
        return content, {"tool": "PyAV / Pillow", "version": {"av": av.__version__, "pillow": PIL.__version__},
            "codec": "libwebp lossless RGBA", "codec_version": features.version("webp"),
            "width": frames[0].width, "height": frames[0].height, "decoded_frames": len(frames),
            "presentation_timestamps_seconds": timestamps, "frame_durations_ms": durations,
            "muted": True, "loop": True, "target_execution": False}


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
    for folder in ("app", "crates", "assets", "tools", "docs"):
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


def locale_preparation_source(acquisition, source_name):
    """Validate the current deployment reference and actual inert source bytes."""
    import evidence
    import resource_database
    receipt_path = safe(ROOT / acquisition)
    if not receipt_path.is_relative_to(ROOT / ".analysis/inputs"):
        raise ValueError("Locale acquisition must be a retained current input")
    receipt_raw = receipt_path.read_bytes()
    receipt = resource_database.unique_json(receipt_raw)
    stability = receipt.get("stability", [])
    if (receipt.get("source_current_during_acquisition") is not True or not stability
            or any(row.get("unchanged") is not True for row in stability)):
        raise ValueError("Locale acquisition has no deployment-stability proof")
    source = safe(ROOT / source_name)
    if not source.is_relative_to(ROOT / ".analysis/inputs"):
        raise ValueError("Locale source is outside retained JavaScript originals")
    originals = [row for row in receipt.get("files", []) if row.get("path") == relative(source)]
    raw = source.read_bytes()
    if (len(originals) != 1 or originals[0].get("sha256") != hashlib.sha256(raw).hexdigest()
            or originals[0].get("bytes") != len(raw)):
        raise ValueError("Locale original identity does not verify: " + source_name)
    original = originals[0]
    if not urllib.parse.urlsplit(original.get("url", "")).path.lower().endswith(".js"):
        raise ValueError("Locale original URL is not a JavaScript source")
    if (not official_resource_url(original.get("url")) or not official_resource_url(receipt.get("entry_url"))
            or not official_resource_url(original.get("final_url", original.get("url")))):
        raise ValueError("Locale source must use current official HTTPS endpoints")
    revalidation = original.get("current_http_revalidation", {}).get("attempts", [])
    if original.get("http_status") != 200:
        def read_revalidation(path, expected):
            response = path.read_bytes()
            if (len(response) != expected["bytes"] or hashlib.sha256(response).hexdigest() != expected["sha256"]):
                raise ValueError("Locale revalidation bytes differ from retained original")
            return response
        verified_official_http_revalidation(original, raw, read_revalidation)
    deployment = evidence.frontend_deployment_sources(receipt)
    mounted = [row for row in deployment["sources"] if row.get("original", {}).get("path") == relative(source)
               and row.get("identity_verified")]
    if (not deployment.get("entry_original") or len(mounted) != 1
            or any(row["kind"] == "deployment_stability_identity_mismatch" for row in deployment["failures"])):
        raise ValueError("Locale original has no verified current deployment reference")
    identities = [{"path": relative(receipt_path), "bytes": len(receipt_raw),
                   "sha256": hashlib.sha256(receipt_raw).hexdigest()},
                  {key: original[key] for key in ("path", "bytes", "sha256")},
                  deployment["entry_original"], *[row["original"] for row in deployment["manifests"]],
                  *[{key: row[key] for key in ("path", "bytes", "sha256")} for row in revalidation]]
    return raw, {"acquisition": acquisition, "source": original,
                 "deployment_reference": mounted[0], "stability": stability,
                 "deployment_failures": deployment["failures"],
                 "acquisition_unresolved": receipt.get("unresolved", []), "inputs": identities}


def locale_preparation_catalog(*, migrate_legacy=False):
    """Keep legacy bytes only as evidence, and flatten their complete semantics."""
    import resource_database
    destination = ROOT / resource_database.LOCALES
    legacy = sorted((ROOT / "locales").glob("*.json"))
    if legacy and not migrate_legacy:
        raise ValueError("Migrate legacy locale files before preparing the installed catalog")
    current_raw = destination.read_bytes() if destination.is_file() else None
    if current_raw is not None:
        document = resource_database.unique_json(current_raw)
        resource_database.validate_locales(document)
    elif not legacy:
        raise ValueError("Locale maintenance catalog is missing; a legacy migration is required")
    else:
        document = {"schema_version": 1, "locales": []}
    baseline, snapshots = [], []
    if current_raw is not None:
        snapshots.append((destination, current_raw))
    existing = {row["locale"]: row["messages"] for row in document["locales"]}
    for path in legacy:
        raw = path.read_bytes()
        words = resource_database.flatten_locale(resource_database.unique_json(raw))
        if existing:
            if path.stem not in existing or any(existing[path.stem].get(key) != value for key, value in words.items()):
                raise ValueError("Legacy locale differs from its migrated catalog: " + path.name)
        else:
            document["locales"].append({"locale": path.stem, "messages": words})
        target = safe(ROOT / resource_database.LOCALE_BASELINE / path.name)
        if target.is_file() and target.read_bytes() != raw:
            raise ValueError("Retained locale baseline differs: " + path.name)
        snapshots.append((path, raw))
        baseline.append({"original_path": relative(path), "path": relative(target), "bytes": len(raw),
                         "sha256": hashlib.sha256(raw).hexdigest(), "messages": len(words), "_raw": raw})
    resource_database.validate_locales(document)
    return document, snapshots, baseline


def merge_locale_preparation_entries(previous, prepared):
    """Keep the original owner of every key not replaced by this publication."""
    current_kinds = {"current_routed_exports", "current_chunk_exports"}
    entries = list(previous)
    for update in prepared:
        if update["kind"] not in current_kinds:
            entries = [row for row in entries if (row["kind"], row.get("namespace"))
                       != (update["kind"], update.get("namespace"))]
            entries.append(update)
            continue
        keys = update.get("keys")
        if not isinstance(keys, list) or not keys or len(set(keys)) != len(keys):
            raise ValueError("Current locale provenance requires the exact selected keys")
        retained = []
        for row in entries:
            if row["kind"] not in current_kinds or row.get("namespace") != update.get("namespace"):
                retained.append(row)
                continue
            owned = row.get("keys")
            if not isinstance(owned, list) or not owned or len(set(owned)) != len(owned):
                raise ValueError("Retained current locale provenance lacks its selected keys")
            remaining = [key for key in owned if key not in keys]
            if remaining:
                prefix = row.get("namespace")
                destinations = {prefix + "." + key if prefix else key for key in remaining}
                retained.append({**row, "keys": remaining,
                    **({"absent_keys": {locale: [key for key in values if key in remaining]
                                        for locale, values in row["absent_keys"].items()
                                        if any(key in remaining for key in values)}} if "absent_keys" in row else {}),
                    "changes": {locale: {key: value for key, value in values.items() if key in destinations}
                                for locale, values in row.get("changes", {}).items()}})
        entries = retained + [update]
    return entries


def publish_locale_preparation(document, snapshots, baseline, prepared, *, check=False):
    """Publish verified maintenance words, regenerate SQLite, then retire old inputs."""
    import analysis
    import resource_database
    resource_database.validate_locales(document)
    destination = ROOT / resource_database.LOCALES
    report_path = ROOT / resource_database.LOCALE_REPORT
    previous = resource_database.unique_json(report_path.read_bytes()) if report_path.is_file() else {"entries": []}
    entries = merge_locale_preparation_entries(previous["entries"], prepared)
    output = (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    for path, before in snapshots:
        if path.read_bytes() != before:
            raise ValueError("Locale input changed during preparation: " + relative(path))
    for row in prepared:
        for item in row["provenance"]["inputs"]:
            path = ROOT / item["path"]
            if path.stat().st_size != item["bytes"] or digest(path) != item["sha256"]:
                raise ValueError("Locale original changed during preparation: " + item["path"])
    if check:
        if not destination.is_file() or destination.read_text(encoding="utf-8") != output.decode("utf-8"):
            raise ValueError("Locale maintenance words differ; regenerate through workspace.py")
        database = resource_database.prepare(check=True)
    else:
        for row in baseline:
            target = ROOT / row["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            if not target.is_file():
                descriptor, temporary = tempfile.mkstemp(prefix=".locale-baseline-", dir=target.parent)
                try:
                    with os.fdopen(descriptor, "wb") as stream:
                        stream.write(row["_raw"])
                    Path(temporary).replace(target)
                finally:
                    Path(temporary).unlink(missing_ok=True)
        write_json(destination, document)
        # No legacy input is removed until all catalog fields and final SQLite
        # bytes have passed the producer's complete roundtrip verification.
        database = resource_database.prepare()
        if resource_database.localization_document() != document["locales"]:
            raise ValueError("Published locale maintenance semantics differ")
        for row in baseline:
            source, target = ROOT / row["original_path"], ROOT / row["path"]
            if source.read_bytes() != target.read_bytes() or digest(target) != row["sha256"]:
                raise ValueError("Locale baseline changed before legacy retirement")
        for row in baseline:
            (ROOT / row["original_path"]).unlink()
        legacy_directory = ROOT / "locales"
        if baseline and not any(legacy_directory.iterdir()):
            legacy_directory.rmdir()
    if not check:
        previous["entries"] = entries
        if baseline:
            previous["baseline"] = [{key: value for key, value in row.items() if key != "_raw"} for row in baseline]
        from importlib.metadata import version
        previous.update(schema_version=1, recorded_at_utc=datetime.now(timezone.utc).isoformat(),
            tools={"python": sys.version.split()[0],
                   "analyzers": {name: version(name) for name in ("tree-sitter", "tree-sitter-javascript", "json5")},
                   "inputs": [{"path": relative(path), "sha256": digest(path), "bytes": path.stat().st_size}
                              for path in (ROOT / "tools/workspace.py", ROOT / "tools/evidence.py", ROOT / "tools/resource_database.py")]},
            output={"path": resource_database.LOCALES, "sha256": digest(destination), "bytes": destination.stat().st_size},
            database={key: database[key] for key in ("output", "bytes", "sha256", "schema_version")},
            legacy_vendor_source_provenance_established=False,
            legacy_semantics_preserved=True, static_only=True, vendor_javascript_executed=False,
            runtime_acceptance="not performed")
        write_json(report_path, previous)
        indexed = [{"path": row["path"], "bytes": row["bytes"], "sha256": row["sha256"],
                    "kind": "retained_locale_baseline", "maintenance_only": True,
                    "vendor_source_provenance_established": False} for row in previous.get("baseline", [])]
        indexed.append({**previous["output"], "kind": "ui_locale_maintenance", "maintenance_only": True,
                        "preparation_receipt": resource_database.LOCALE_REPORT})
        analysis.verify_source_rows(indexed)
        analysis.index_files(indexed)
    return {"report": resource_database.LOCALE_REPORT, "destination": resource_database.LOCALES,
            "locales": {row["locale"]: len(row["messages"]) for row in document["locales"]},
            "database": database["output"], "check_only": check, "passed": True}


def prepare_locale_exports(acquisition, source_name, *, migrate_legacy=False, check=False):
    """Fill every missing key from complete current exports; retain audited values."""
    import analysis
    import evidence
    if check and migrate_legacy:
        raise ValueError("A locale migration cannot run in check-only mode")
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        document, snapshots, baseline = locale_preparation_catalog(migrate_legacy=migrate_legacy)
        raw, provenance = locale_preparation_source(acquisition, source_name)
        selected = evidence.locale_export_dictionaries(raw, [row["locale"] for row in document["locales"]])
        added, retained_differences = {}, {}
        for row in document["locales"]:
            words, current = row["messages"], selected["values"][row["locale"]]
            added[row["locale"]] = {key: value for key, value in current.items() if key not in words}
            retained_differences[row["locale"]] = {key: {"retained": words[key], "current_export": value}
                for key, value in current.items() if key in words and words[key] != value}
            row["messages"] = dict(sorted((words | added[row["locale"]]).items()))
        prepared = [{"kind": "complete_export_dictionaries", "provenance": provenance, "dictionary": selected,
                     "merge_policy": "Add missing keys; preserve every existing value and namespaced key",
                     "added": added, "retained_differences": retained_differences}]
        report = publish_locale_preparation(document, snapshots, baseline, prepared, check=check)
        return {**report, "added": {locale: len(values) for locale, values in added.items()},
                "retained_differences": {locale: len(values) for locale, values in retained_differences.items()}}


def prepare_locale_updates(updates, *, check=False):
    """Verify all source-owned translations before one installed catalog publication."""
    import analysis
    import evidence
    if not isinstance(updates, list) or not updates:
        raise ValueError("Locale updates must be a nonempty list")
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        document, snapshots, baseline = locale_preparation_catalog()
        by_locale = {row["locale"]: row for row in document["locales"]}
        prepared, changes, destinations = [], [], set()
        for update in updates:
            if (not isinstance(update, dict) or set(update) - {
                    "acquisition", "source", "bindings", "namespace", "keys", "absent_keys"}
                    or not isinstance(update.get("acquisition"), str)
                    or ("source" in update) == ("bindings" in update)):
                raise ValueError("Locale update requires one acquisition and one source or language map")
            namespace, keys = update.get("namespace"), update.get("keys")
            if namespace is not None and (not isinstance(namespace, str)
                    or not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", namespace)):
                raise ValueError("Invalid locale namespace")
            if keys is not None and (not isinstance(keys, list) or not keys
                    or any(not isinstance(key, str) or not key for key in keys) or len(set(keys)) != len(keys)):
                raise ValueError("Locale update keys must be nonempty unique strings")
            absent = update.get("absent_keys", {})
            if (not isinstance(absent, dict) or not set(absent) <= set(by_locale)
                    or any(not isinstance(missing, list) or not missing
                           or any(not isinstance(key, str) for key in missing)
                           or len(set(missing)) != len(missing)
                           or keys is None or not set(missing) <= set(keys)
                           for missing in absent.values())):
                raise ValueError("Absent locale keys require exact selected keys for known languages")
            if absent.get("en"):
                raise ValueError("Selected locale keys must have their source English values")
            acquisition = update["acquisition"]
            if "source" in update:
                raw, provenance = locale_preparation_source(acquisition, update["source"])
                selected = evidence.locale_export_dictionaries(raw, list(by_locale))
                values = selected["values"]
                kind = "current_routed_exports"
            else:
                bindings = update["bindings"]
                if not isinstance(bindings, dict) or set(bindings) != set(by_locale):
                    raise ValueError("Locale chunk bindings must cover every installed language exactly")
                selected, values, provenances = {}, {}, []
                for locale, path in bindings.items():
                    raw, original = locale_preparation_source(acquisition, path)
                    # Logical source identity comes from the acquisition URL;
                    # refreshed originals may be extensionless addressed blobs.
                    leaf = Path(urllib.parse.urlsplit(original["source"]["url"]).path).name
                    if not re.fullmatch(r"trans-" + re.escape(locale) + r"\.[A-Za-z0-9_-]+\.chunk\.js", leaf):
                        raise ValueError("Locale chunk URL differs from its language: " + locale)
                    selected[locale] = evidence.locale_chunk_exports(raw, keys, absent_keys=absent.get(locale, ()))
                    values[locale] = selected[locale]["values"]
                    provenances.append(original)
                provenance = {"sources": provenances,
                              "inputs": [row for original in provenances for row in original["inputs"]]}
                kind = "current_chunk_exports"
            if keys is None:
                keys = sorted(next(iter(values.values())))
                if not keys or any(set(words) != set(keys) for words in values.values()):
                    raise ValueError("Complete locale owners must declare the same keys in every language")
            delta = {}
            for locale, row in by_locale.items():
                current = values[locale]
                missing = set(absent.get(locale, ()))
                if (keys is not None and set(keys) - current.keys() != missing
                        or missing & current.keys()):
                    raise ValueError("Locale source does not provide every requested key: " + locale)
                current = {key: value for key, value in current.items() if keys is None or key in keys}
                words, changed = row["messages"], {}
                for key in keys:
                    observed = key not in missing
                    value = current[key] if observed else None
                    key = namespace + "." + key if namespace else key
                    identity = (locale, key)
                    if identity in destinations:
                        raise ValueError("Locale update repeats a destination: " + locale + "/" + key)
                    destinations.add(identity)
                    if (observed and (key not in words or words[key] != value)) or (not observed and key in words):
                        changed[key] = {"previous": words.get(key), "current": value}
                    if observed:
                        words[key] = value
                    else:
                        words.pop(key, None)
                row["messages"] = dict(sorted(words.items()))
                delta[locale] = changed
            prepared.append({"kind": kind, "namespace": namespace, "provenance": provenance,
                             "dictionary": selected, "keys": keys, "absent_keys": absent,
                             "merge_policy": "Replace only keys selected from each current source owner",
                             "changes": delta})
            changes.append({"namespace": namespace, "keys_changed": {locale: len(rows) for locale, rows in delta.items()}})
        return {**publish_locale_preparation(document, snapshots, baseline, prepared, check=check),
                "updates": changes}


def prepare_locale_keys(acquisition, namespace, bindings, keys):
    """Prepare scoped source words in the sole installed SQLite catalog owner."""
    import analysis
    import evidence
    if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", namespace):
        raise ValueError("Invalid locale namespace")
    if type(bindings) is not dict or not bindings:
        raise ValueError("Locale bindings must be a nonempty language-to-original map")
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        document, snapshots, baseline = locale_preparation_catalog()
        by_locale = {row["locale"]: row for row in document["locales"]}
        prepared = []
        for locale, source_name in bindings.items():
            if locale not in by_locale or type(source_name) is not str:
                raise ValueError("Unknown locale binding")
            raw, provenance = locale_preparation_source(acquisition, source_name)
            selected = evidence.locale_keys(raw, keys)
            words = by_locale[locale]["messages"]
            words.update({namespace + "." + key: value for key, value in selected["values"].items()})
            by_locale[locale]["messages"] = dict(sorted(words.items()))
            prepared.append({"kind": "literal_namespace", "locale": locale, "namespace": namespace,
                             "provenance": provenance, "dictionary": selected})
        return {**publish_locale_preparation(document, snapshots, baseline, prepared), "namespace": namespace, "keys": keys}


def prepare_image_analyzer(*, svg=False):
    """Acquire verified raster/SVG codecs in the existing image analyzer."""
    import sysconfig
    import zipfile
    import analysis
    abi = f"cp{sys.version_info.major}{sys.version_info.minor}"
    platform_tag = sysconfig.get_platform().replace("-", "_").replace(".", "_")
    packages = [("pillow", "12.3.0", f"pillow-12.3.0-{abi}-{abi}-{platform_tag}.whl")]
    if svg:
        packages.append(("resvg-py", "0.5.0", f"resvg_py-0.5.0-cp310-abi3-{platform_tag}.whl"))
    reference = safe(ROOT / ".analysis/references/python-static-wheels")
    destination = safe(ROOT / "tools/analyzers/pillow")
    reference.mkdir(parents=True, exist_ok=True)
    receipt_path = destination / "acquisition.json"
    previous = json.loads(receipt_path.read_bytes()) if receipt_path.is_file() else {}
    replaced = {filename for _, _, filename in packages}
    wheels = [row for row in previous.get("wheels", []) if Path(row["path"]).name not in replaced]
    retained_wheels = {row["path"] for row in wheels}
    records = [row for row in previous.get("files", []) if row["wheel"] in retained_wheels]
    for row in records:
        output = safe(ROOT / row["path"])
        if (not output.is_relative_to(destination) or not output.is_file()
                or output.stat().st_size != row["bytes"] or digest(output) != row["sha256"]):
            raise ValueError("Retained image analyzer member no longer verifies: " + row["path"])
    indexed = []
    for package, version, filename in packages:
        metadata_url = f"https://pypi.org/pypi/{package}/{version}/json"
        with urllib.request.urlopen(metadata_url, timeout=40) as response:
            metadata_bytes = response.read()
        metadata = json.loads(metadata_bytes)
        normalize = lambda value: value.lower().replace("_", "-")
        if metadata["info"]["version"] != version or normalize(metadata["info"]["name"]) != package:
            raise ValueError("Image analyzer metadata differs from its pinned package")
        artifact = next((item for item in metadata["urls"] if item["filename"] == filename
                         and item["packagetype"] == "bdist_wheel"), None)
        if artifact is None:
            raise ValueError("Pinned image analyzer has no wheel for the current Python: " + filename)
        if not artifact["url"].startswith("https://files.pythonhosted.org/"):
            raise ValueError("Image analyzer wheel does not have the published PyPI origin")
        metadata_path = reference / f"{package}-{version}.{hashlib.sha256(metadata_bytes).hexdigest()}.pypi.json"
        metadata_path.write_bytes(metadata_bytes)
        wheel_path = reference / filename
        if not wheel_path.is_file() or digest(wheel_path) != artifact["digests"]["sha256"]:
            with urllib.request.urlopen(artifact["url"], timeout=60) as response:
                wheel_bytes = response.read(artifact["size"] + 1)
            if len(wheel_bytes) != artifact["size"] or hashlib.sha256(wheel_bytes).hexdigest() != artifact["digests"]["sha256"]:
                raise ValueError("Image analyzer wheel differs from its published byte identity")
            wheel_path.write_bytes(wheel_bytes)
        if wheel_path.stat().st_size != artifact["size"]:
            raise ValueError("Image analyzer wheel length differs from its publisher metadata")
        with zipfile.ZipFile(wheel_path) as archive:
            distribution = package.replace("-", "_")
            members = verified_wheel_members(archive, f"{distribution}-{version}.dist-info/RECORD")
            for name in members:
                output = safe(destination / name)
                if not output.is_relative_to(destination):
                    raise ValueError("Image analyzer wheel member escaped its installation directory")
                content = archive.read(name)
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(content)
                records.append({"path": relative(output), "bytes": len(content),
                                "sha256": digest(output), "member": name, "wheel": relative(wheel_path)})
        wheels.append({"name": package, "version": version, "path": relative(wheel_path), "url": artifact["url"],
                       "bytes": wheel_path.stat().st_size, "sha256": digest(wheel_path),
                       "metadata_path": relative(metadata_path), "metadata_url": metadata_url,
                       "metadata_sha256": digest(metadata_path)})
        indexed.extend([{"path": relative(wheel_path), "bytes": wheel_path.stat().st_size, "sha256": digest(wheel_path),
                         "url": artifact["url"], "kind": "static_analyzer_wheel"},
                        {"path": relative(metadata_path), "bytes": len(metadata_bytes), "sha256": digest(metadata_path),
                         "url": metadata_url, "kind": "static_analyzer_metadata"}])
    receipt = {"schema_version": 1, "acquired_at_utc": analysis.stamp(), "python_abi": abi + "-" + platform_tag,
        "python_version": sys.version.split()[0], "target_execution": False,
        "scope": "Project-local static image decoder only; no product runtime dependency",
        "wheels": wheels, "files": records,
        "record_verification": {"members": len(records), "all_record_members_verified": True,
                                "all_extracted_bytes_match": True}}
    write_json(receipt_path, receipt)
    analysis.index_files(indexed)
    return {"receipt": relative(receipt_path), "python_abi": receipt["python_abi"],
            "files": len(records), "packages": [row["name"] for row in wheels]}


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


def prepare_resources(acquisition, resources=None, *, publication_manifest=None):
    import analysis
    with analysis.index_lock(ROOT / ".work/resource-writer.lock"):
        if publication_manifest is not None:
            return prepare_resource_publication(acquisition, resources, publication_manifest)
        return prepare_resources_locked(acquisition, resources)


def keyboard_svg_lettering(producer_body, enums):
    """Read the current producer's literal layout-to-lettering map."""
    return {enums[name]: label for name, label in
            re.findall(r'\[[\w$]+\.[\w$]+\.([\w$]+)\]:"([A-Z]+)"', producer_body) if name in enums}


def keyboard_normalized_svg_layout(layout_id, normalization):
    """Apply the source producer's ANSI/ISO/Japanese resource selection."""
    if layout_id in normalization["ansi"]:
        return normalization["united_states"]
    if layout_id in normalization["iso"]:
        return normalization["united_kingdom"]
    return layout_id if layout_id == normalization["japanese"] else normalization["united_states"]


def keyboard_customize_svg_request(content, asset, producer, *, normalized=False, lettering="US", layout_id=None):
    """Project the current keyboard producer's root-child filter and alias."""
    if not isinstance(asset, str) or not asset.endswith(".svg") or type(normalized) is not bool:
        raise ValueError("Keyboard Customize requires an SVG alias and explicit normalization")
    if normalized and (type(layout_id) is not int or type(lettering) is not str or not lettering):
        raise ValueError("Normalized keyboard Customize requires a layout and lettering")
    root = ET.fromstring(content)
    if root.tag.split("}")[-1] != "svg":
        raise ValueError("Keyboard Customize original is not SVG")
    base_removed = [child.get("id") for child in root if child.get("id")
                    and ("_LED" in child.get("id") or "_Selection" in child.get("id"))]
    alias = asset.replace(".svg", "-customize.svg") if base_removed else asset
    removed = [child.get("id") for child in root if child.get("id") and
               ("_LED" in child.get("id") or "_Selection" in child.get("id") or
                normalized and ("_Lettering" in child.get("id") or "_Characters" in child.get("id"))
                and child.get("id") not in {lettering + "_Lettering", lettering + "_Characters"})]
    if removed and normalized:
        alias = alias.replace(".svg", f"-customize-layout-{layout_id}.svg")
    return ({"display": alias, "svg_remove_root_ids": removed, "producer": producer}
            if removed else {"display": alias})


def filter_svg_root_children(content, root_ids):
    """Remove exactly the acquired producer's named root children."""
    if (not isinstance(root_ids, list) or not root_ids
            or any(type(root_id) is not str or not root_id for root_id in root_ids)
            or len(set(root_ids)) != len(root_ids)):
        raise ValueError("Source SVG root identifiers must be distinct nonempty strings")
    root = ET.fromstring(content)
    if root.tag.split("}")[-1] != "svg":
        raise ValueError("Source SVG root filtering requires an SVG original")
    removed = []
    for child in list(root):
        if child.get("id") in root_ids:
            removed.append(child.get("id"))
            root.remove(child)
    if set(removed) != set(root_ids) or len(removed) != len(root_ids):
        raise ValueError("Source SVG producer filter contains absent or repeated root nodes")
    ET.register_namespace("", "http://www.w3.org/2000/svg")
    ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
    return ET.tostring(root, encoding="utf-8"), {
        "tool": "Python xml.etree.ElementTree", "version": sys.version.split()[0],
        "operation": "Remove only named source SVG root children; serialize preserved tree",
        "removed_root_ids": removed, "root_attributes": dict(root.attrib)}


def verified_svg_root_filter_producer(producer, originals, *, snapshots, cache):
    """Verify the acquired bytes and complete module that owns a root filter."""
    import families
    if (not isinstance(producer, dict)
            or not {"path", "sha256", "anchor", "module_id"} <= set(producer)
            or type(producer["path"]) is not str or type(producer["anchor"]) is not str
            or not producer["anchor"] or type(producer["module_id"]) not in (int, str)
            or not re.fullmatch(r"[0-9a-f]{64}", str(producer["sha256"]))):
        raise ValueError("Source SVG producer requires an acquired module identity")
    producer_path = safe(ROOT / producer["path"])
    if (not producer_path.is_relative_to((ROOT / ".analysis/inputs").resolve())
            or (ROOT / producer["path"]).is_symlink() or producer_path.suffix.lower() != ".js"):
        raise ValueError("Source SVG producer must be a retained acquired JS original")
    original = originals.get(relative(producer_path))
    if not original or original.get("http_status") != 200 or not producer_path.is_file():
        raise ValueError("Source SVG producer acquisition does not verify")
    raw = producer_path.read_bytes()
    if (original.get("sha256") != producer["sha256"]
            or hashlib.sha256(raw).hexdigest() != producer["sha256"]
            or original.get("bytes") != len(raw)):
        raise ValueError("Source SVG producer acquisition does not verify")
    key = (relative(producer_path), producer["sha256"], producer["anchor"],
           producer["module_id"], producer.get("boundary_parser"))
    body = cache.get(key)
    if body is None:
        text = raw.decode("utf-8")
        if producer.get("boundary_parser") == "tree-sitter-javascript" or not isinstance(producer["module_id"], int):
            offsets = [match.start() for match in re.finditer(re.escape(producer["anchor"]), text)]
            owners = [module for module in families.keyboard_source_modules(text)
                      if any(module["character_start"] <= offset < module["character_end"] for offset in offsets)]
            if not owners:
                raise ValueError("Current source SVG producer is outside a complete grammar function")
            owners = [module for module in owners if module["module_id"] == producer["module_id"]]
            if len(owners) != 1:
                raise ValueError("Source SVG producer module does not match or is ambiguous")
            body = owners[0]
        else:
            body = families.complete_raw_module(text, producer["anchor"])
        if body["module_id"] != producer["module_id"]:
            raise ValueError("Source SVG producer module does not match")
        cache[key] = body
    snapshots.append((producer_path, raw))
    families_tool = Path(families.__file__)
    snapshots.append((families_tool, families_tool.read_bytes()))
    return {"input": original, "complete_module": body}


def avif_display_png(source, *, png_optimize=False):
    """Decode an acquired AVIF without executing its originating application."""
    import io
    sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
    import PIL
    from PIL import Image, features
    if type(png_optimize) is not bool:
        raise ValueError("PNG optimization must be an explicit boolean")
    with Image.open(source) as image:
        rgba = image.convert("RGBA")
        buffer = io.BytesIO()
        rgba.save(buffer, format="PNG", optimize=png_optimize)
        content = buffer.getvalue()
        with Image.open(io.BytesIO(content)) as decoded:
            if decoded.mode != "RGBA" or decoded.size != rgba.size or decoded.tobytes() != rgba.tobytes():
                raise ValueError("Prepared PNG does not retain every decoded AVIF pixel")
        return content, {"tool": "Pillow", "version": PIL.__version__,
                         "codec": "libavif", "codec_version": features.version("avif"),
                         "mode": "RGBA", "width": rgba.width, "height": rgba.height,
                         "decoded_rgba_sha256": hashlib.sha256(rgba.tobytes()).hexdigest(),
                          "png_encoding": {"optimize": png_optimize},
                         "png_pixel_roundtrip": True, "target_execution": False}


def extract_inline_resource_png(content, expected_hash):
    """Read one literal PNG and retain its complete static module owners."""
    original_text = content.decode("utf-8")
    literals, selected = [], None
    for match in re.finditer(r"(['\"])data:image/png;base64,([A-Za-z0-9+/=]+)\1", original_text):
        decoded = base64.b64decode(match[2], validate=True)
        if hashlib.sha256(decoded).hexdigest() != expected_hash:
            continue
        literals.append(match)
        selected = decoded
    if not literals or not selected.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("Verified inline PNG literal is absent from the acquired source")
    modules = None
    if len(literals) > 1:
        import families
        modules = families.keyboard_source_modules(original_text)
    matches = []
    for match in literals:
        if modules is None:
            owner = complete_resource_module(original_text, match[0])
        else:
            owners = [module for module in modules
                      if module["character_start"] <= match.start() and match.end() <= module["character_end"]]
            if not owners:
                raise ValueError("Inline PNG literal is outside every complete static module")
            owner = min(owners, key=lambda module: module["character_end"] - module["character_start"])
        matches.append({"character_start": match.start(), "character_end": match.end(), "complete_module": owner})
    return selected, {"tool": "Python base64", "version": sys.version.split()[0],
                      "operation": "Decode a complete literal source PNG data URL; no JavaScript evaluation",
                      "source_regions": matches}


def extract_inline_css_png(content, bitmap):
    """Retain the complete acquired CSS rule that owns one literal PNG."""
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
    return content, {
        "tool": "Python base64", "version": sys.version.split()[0],
        "operation": "Decode the PNG data URL from the verified complete source CSS rule; no CSS execution",
        "complete_css_rules": [{"character_start": rule.start(), "character_end": rule.end(), "body": rule[0]}],
    }


def decoded_resource_rgba(content, expected_format):
    """Decode complete image bytes through the maintained static codec."""
    import io
    sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
    from PIL import Image
    with Image.open(io.BytesIO(content)) as image:
        if image.format != expected_format:
            raise ValueError("Resource format differs from its declared transformation")
        image.verify()
    with Image.open(io.BytesIO(content)) as image:
        image.load()
        rgba = image.convert("RGBA")
        pixels = rgba.tobytes()
        return {"format": image.format, "original_mode": image.mode,
                "comparison_mode": "RGBA", "width": rgba.width, "height": rgba.height,
                "decoded_rgba_sha256": hashlib.sha256(pixels).hexdigest()}, pixels


def compare_resource_pixels(source, output, source_format):
    original, source_pixels = decoded_resource_rgba(source, source_format)
    prepared, output_pixels = decoded_resource_rgba(output, "PNG")
    if (original["width"], original["height"]) != (prepared["width"], prepared["height"]):
        raise ValueError("Converted resource dimensions differ from the original")
    if source_pixels != output_pixels:
        raise ValueError("Converted resource does not retain every decoded original RGBA pixel")
    return {"source_decode": original, "output_decode": prepared,
            "all_rgba_pixels_equal": True}


def verify_pinned_resource_conversion(original, request, locked, storage):
    """Compare each pinned PNG with actual decoded source pixels, without UI."""
    sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
    import PIL
    from PIL import features
    report = {"source": {key: original[key] for key in ("path", "bytes", "sha256")},
              "output": {key: locked[key] for key in ("path", "bytes", "sha256")},
              "alias": request["display"], "resource_revision": storage["revision"],
              "tool": {"path": relative(Path(__file__)), "sha256": digest(Path(__file__)),
                       "python": sys.version.split()[0], "pillow": PIL.__version__,
                       "avif_codec": features.version("avif")},
              "errors": [], "target_execution": False}
    try:
        source = safe(ROOT / original["path"])
        content = source.read_bytes()
        if len(content) != original["bytes"] or hashlib.sha256(content).hexdigest() != original["sha256"]:
            raise ValueError("Original conversion input differs from its acquisition")
        if not request["display"].endswith(".png"):
            raise ValueError("Pinned image conversion requires a PNG display alias")
        if "inline_css_png" in request:
            if source.suffix.lower() != ".css":
                raise ValueError("CSS bitmap extraction requires an acquired CSS original")
            decoded_source, extraction = extract_inline_css_png(content, request["inline_css_png"])
            report["extraction"] = extraction
            source_format = "PNG"
        elif source.suffix.lower() == ".avif":
            decoded_source, source_format = content, "AVIF"
        else:
            raise ValueError("Pinned conversion has no verified static transformation")
        # Maintenance downloads only the selected immutable file. Its cache is
        # analysis scratch and is never a product runtime resource directory.
        payload = safe(ROOT / ".work/resource-closure/payloads" / Path(locked["path"]).name)
        if not payload.is_file() or payload.stat().st_size != locked["bytes"] or digest(payload) != locked["sha256"]:
            url = ("https://huggingface.co/datasets/" + storage["repository"] + "/resolve/"
                   + storage["revision"] + "/" + locked["path"])
            with urllib.request.urlopen(urllib.request.Request(url, headers=resource_sources.hf_headers()), timeout=60) as response:
                raw = response.read(locked["bytes"] + 1)
            if len(raw) != locked["bytes"] or hashlib.sha256(raw).hexdigest() != locked["sha256"]:
                raise ValueError("Pinned converted resource payload byte identity differs")
            payload.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(dir=payload.parent, suffix=".partial", delete=False) as stream:
                temporary = Path(stream.name)
                try:
                    stream.write(raw)
                    stream.close()
                    os.replace(temporary, payload)
                finally:
                    temporary.unlink(missing_ok=True)
        raw = payload.read_bytes()
        report["actual_output"] = {"path": relative(payload), "bytes": len(raw),
                                   "sha256": hashlib.sha256(raw).hexdigest()}
        if len(raw) != locked["bytes"] or hashlib.sha256(raw).hexdigest() != locked["sha256"]:
            raise ValueError("Pinned converted resource changed before decode")
        report.update(compare_resource_pixels(decoded_source, raw, source_format))
        report["inputs_unchanged"] = source.read_bytes() == content and payload.read_bytes() == raw
        if not report["inputs_unchanged"]:
            raise ValueError("Conversion input or payload changed during static decode")
    except (OSError, ValueError, SyntaxError) as error:
        report["errors"].append({"kind": type(error).__name__, "message": str(error)})
    report["passed"] = not report["errors"]
    return report


def resource_conversion_counterexamples():
    """Reject wrong pixels, alpha, dimensions and incomplete encoded images."""
    import io
    sys.path.insert(0, str(ROOT / "tools/analyzers/pillow"))
    from PIL import Image
    def png(size, changed=None):
        image = Image.new("RGBA", size, (17, 29, 43, 255))
        if changed is not None:
            image.putpixel((0, 0), changed)
        stream = io.BytesIO()
        image.save(stream, format="PNG")
        return stream.getvalue()
    original = png((2, 2))
    compare_resource_pixels(original, original, "PNG")
    cases = {"wrong_pixel": png((2, 2), (18, 29, 43, 255)),
             "wrong_alpha": png((2, 2), (17, 29, 43, 254)),
             "wrong_dimensions": png((1, 2)), "truncated_png": original[:40]}
    results = []
    for name, changed in cases.items():
        try:
            compare_resource_pixels(original, changed, "PNG")
        except (OSError, ValueError, SyntaxError) as error:
            results.append({"case": name, "rejected": True,
                            "kind": type(error).__name__, "message": str(error)})
        else:
            raise ValueError("Resource conversion counterexample was accepted: " + name)
    literal = ".bitmap{background:url(data:image/png;base64," + base64.b64encode(original).decode() + ")}"
    spec = {"selector": ".bitmap", "sha256": hashlib.sha256(original).hexdigest()}
    if extract_inline_css_png(literal.encode(), spec)[0] != original:
        raise ValueError("Inline CSS positive control changed its PNG bytes")
    for name, text, request in (
            ("wrong_css_selector", literal, {**spec, "selector": ".other"}),
            ("wrong_css_png_sha256", literal, {**spec, "sha256": "0" * 64}),
            ("ambiguous_css_png", literal + literal, spec)):
        try:
            extract_inline_css_png(text.encode(), request)
        except ValueError as error:
            results.append({"case": name, "rejected": True,
                            "kind": type(error).__name__, "message": str(error)})
        else:
            raise ValueError("Resource conversion counterexample was accepted: " + name)
    return {"positive_controls": ["equal_complete_rgba_png", "unique_complete_css_png"],
            "counterexamples": results, "passed": True, "target_execution": False}


def prepare_resource_publication(acquisition, resources, publication_manifest):
    """Stage verified original media for the existing pinned-file publisher."""
    import analysis
    import asset_release
    input_root = (ROOT / ".analysis/inputs").resolve()
    manifest_path = safe(ROOT / publication_manifest)
    receipt_path = safe(ROOT / acquisition)
    if not manifest_path.is_relative_to(input_root) or not receipt_path.is_relative_to(input_root):
        raise ValueError("Resource publication inputs and manifest must remain under analysis inputs")
    if manifest_path == receipt_path:
        raise ValueError("Resource publication cannot overwrite its acquisition receipt")
    receipt_raw = receipt_path.read_bytes()
    receipt = json.loads(receipt_raw)
    stability = receipt.get("stability", [])
    if (not receipt.get("source_current_during_acquisition") or not stability
            or any(not row.get("unchanged") for row in stability)):
        raise ValueError("Resource publication requires a stable current acquisition")
    originals = {row.get("path"): row for row in receipt.get("files", [])}
    if not isinstance(resources, dict) or not resources:
        raise ValueError("Resource publication requires explicit original-to-alias mappings")
    mappings = resource_sources.load_resource_mappings(root=ROOT)
    table_names = {table["name"] for table in mappings["tables"]}
    lock = asset_release.load_lock()
    group_names = {group["name"] for group in lock["groups"]}
    destination = input_root / "prepared-resources"
    prepared_receipt_path = destination / "acquisition.json"
    if manifest_path == prepared_receipt_path:
        raise ValueError("Resource publication manifest cannot replace its prepared receipt")
    snapshots = [(receipt_path, receipt_raw), (Path(__file__), Path(__file__).read_bytes())]
    analyzer_receipt = ROOT / "tools/analyzers/pillow/acquisition.json"
    prepared, derivations = [], []
    verified_svg_producers = {}
    requests = [(name, request) for name, values in resources.items()
                for request in (values if isinstance(values, list) else [values])]
    for source_name, request in requests:
        required = {"display", "table"}
        optional = {"inline_react_svg", "svg_group_visibility", "muted_video_animation",
                    "svg_remove_root_ids", "producer"}
        if not isinstance(request, dict) or not required <= set(request) or set(request) - required - optional:
            raise ValueError("Publication mapping requires display/table and supported static transformations")
        if "muted_video_animation" in request and (request["muted_video_animation"] is not True
                or any(key in request for key in ("inline_react_svg", "svg_group_visibility", "svg_remove_root_ids", "producer"))):
            raise ValueError("Muted video publication requires one explicit, exclusive transformation")
        if "svg_remove_root_ids" in request or "producer" in request:
            if (not {"svg_remove_root_ids", "producer"} <= set(request)
                    or any(key in request for key in ("inline_react_svg", "svg_group_visibility", "muted_video_animation"))):
                raise ValueError("Source SVG root publication requires one exclusive transformation and its producer")
        source = safe(ROOT / source_name)
        if not source.is_relative_to(input_root) or (ROOT / source_name).is_symlink():
            raise ValueError("Resource publication must use retained original inputs")
        if source == manifest_path:
            raise ValueError("Resource publication cannot overwrite its original media")
        raw = source.read_bytes()
        original = originals.get(relative(source))
        if (not original or original.get("http_status") != 200
                or original.get("bytes") != len(raw) or original.get("sha256") != hashlib.sha256(raw).hexdigest()):
            raise ValueError("Resource publication original bytes differ from acquisition")
        alias, table = request["display"], request["table"]
        asset_pack_key(alias)
        if not alias.startswith("synapse/") or table not in table_names:
            raise ValueError("Resource publication alias or table is not maintained")
        conversion = None
        transformation = None
        content = raw
        if "inline_react_svg" in request:
            if source.suffix.lower() != ".js" or Path(alias).suffix.lower() != ".svg":
                raise ValueError("Inline SVG publication requires acquired JS and an SVG alias")
            content, transformation = static_react_svg(raw.decode("utf-8"), request["inline_react_svg"])
            if "svg_group_visibility" in request:
                content, selection = static_svg_group_visibility(content, request["svg_group_visibility"])
                transformation["group_visibility"] = selection
        elif "svg_group_visibility" in request:
            raise ValueError("Group visibility requires its complete inline SVG producer")
        elif "svg_remove_root_ids" in request:
            if source.suffix.lower() != ".svg" or Path(alias).suffix.lower() != ".svg":
                raise ValueError("Source SVG root publication requires an SVG original and alias")
            producer = verified_svg_root_filter_producer(request["producer"], originals,
                snapshots=snapshots, cache=verified_svg_producers)
            content, transformation = filter_svg_root_children(content, request["svg_remove_root_ids"])
            transformation["producer"] = producer
        elif request.get("muted_video_animation"):
            if source.suffix.lower() != ".mp4" or Path(alias).suffix.lower() != ".webp":
                raise ValueError("Muted source video publication requires MP4 and a WebP alias")
            content, conversion = muted_video_display(source)
        elif source.suffix.lower() == ".avif":
            if Path(alias).suffix.lower() != ".png":
                raise ValueError("Acquired AVIF publication requires an explicit PNG alias")
            if not analyzer_receipt.is_file():
                raise ValueError("Prepare the verified image analyzer before AVIF publication")
            snapshots.append((analyzer_receipt, analyzer_receipt.read_bytes()))
            content, conversion = avif_display_png(source)
        elif source.suffix.lower() != Path(alias).suffix.lower():
            raise ValueError("Exact resource publication cannot change its file type")
        output_hash = hashlib.sha256(content).hexdigest()
        suffix = Path(alias).suffix.lower()
        if suffix not in {".png", ".svg", ".webp", ".jpg", ".jpeg", ".gif", ".mp4", ".woff", ".woff2", ".ttf"}:
            raise ValueError("Resource publication file type is not supported")
        target = f"assets/synapse/shared/products/{output_hash}{suffix}"
        if asset_release.group_name(target) not in group_names:
            raise ValueError("Resource publication target has no existing shared group")
        path = destination / f"{output_hash}{suffix}"
        source_identity = {"path": relative(path), "bytes": len(content), "sha256": output_hash}
        snapshots.append((source, raw))
        prepared.append({"path": target, "source": source_identity, "alias": alias, "table": table,
                         "_content": content})
        derivations.append({"original": original, "acquisition": {
            "path": relative(receipt_path), "bytes": len(receipt_raw),
            "sha256": hashlib.sha256(receipt_raw).hexdigest()},
            "output": source_identity, "alias": alias, "table": table,
            "conversion": conversion, "transformation": transformation, "target_execution": False})
    previous_raw = prepared_receipt_path.read_bytes() if prepared_receipt_path.is_file() else None
    previous = json.loads(previous_raw) if previous_raw is not None else {}
    if previous_raw is not None:
        snapshots.append((prepared_receipt_path, previous_raw))
    retained = {row["path"]: row for row in previous.get("files", [])}
    for name, row in retained.items():
        path = safe(ROOT / name)
        if not path.is_relative_to(destination) or not path.is_file():
            raise ValueError("Retained prepared resource is absent or outside its owner")
        raw = path.read_bytes()
        if len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError("Retained prepared resource bytes do not verify")
        snapshots.append((path, raw))
    for item in prepared:
        path = ROOT / item["source"]["path"]
        if path.exists() and path.read_bytes() != item["_content"]:
            raise ValueError("Prepared resource has conflicting bytes")
    if any(path.read_bytes() != raw for path, raw in snapshots):
        raise ValueError("Resource publication original or tool changed during preparation")
    for item in prepared:
        path = ROOT / item["source"]["path"]
        content = item.pop("_content")
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix(path.suffix + ".partial")
        temporary.write_bytes(content)
        os.replace(temporary, path)
        retained[item["source"]["path"]] = item["source"]
    changed_aliases = {item["alias"] for item in prepared}
    proof = {"schema_version": 1, "recorded_at_utc": analysis.stamp(),
             "tool": {"path": "tools/workspace.py", "sha256": digest(Path(__file__))},
             "python_version": sys.version.split()[0], "files": list(retained.values()),
             "derivations": [row for row in previous.get("derivations", []) if row.get("alias") not in changed_aliases] + derivations,
             "inputs_unchanged": True, "target_execution": False}
    if any(item["conversion"] for item in derivations):
        proof["image_analyzer_acquisition"] = {"path": relative(analyzer_receipt),
            "bytes": analyzer_receipt.stat().st_size, "sha256": digest(analyzer_receipt)}
    write_json(prepared_receipt_path, proof)
    receipt_identity = {"path": relative(prepared_receipt_path), "bytes": prepared_receipt_path.stat().st_size,
                        "sha256": digest(prepared_receipt_path)}
    for item in prepared:
        item["acquisition"] = receipt_identity
    write_json(manifest_path, {"schema_version": 1, "files": prepared})
    analysis.index_files([{**row, "kind": "prepared_resource", "acquisition_receipt": relative(prepared_receipt_path)}
                          for row in retained.values()] + [{**receipt_identity, "kind": "acquisition_receipt"},
                          {"path": relative(manifest_path), "bytes": manifest_path.stat().st_size,
                           "sha256": digest(manifest_path), "kind": "resource_publication_manifest"}])
    asset_release.release_additions(relative(manifest_path))
    return {"publication_manifest": relative(manifest_path), "acquisition": receipt_identity,
            "files": prepared, "inputs_unchanged": True, "target_execution": False}


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
    reuse_only = bool(requests) and all("reuse_locked_alias" in request for _, request in requests)
    if not reuse_only and any("reuse_locked_alias" in request for _, request in requests):
        raise ValueError("Pinned alias reuse must be prepared separately from media generation")
    manifest_path = ROOT / "assets/synapse/manifests/manifest.json"
    mappings = resource_sources.load_resource_mappings(root=ROOT)
    aliases = resource_sources.resource_table("ASSETS", mappings=mappings)
    manifest = {"entries": []} if reuse_only else json.loads(manifest_path.read_text(encoding="utf-8"))
    content_pool = None if reuse_only else resource_payload_index()
    registered_products = registered_resource_products()
    favicon_cache = {}
    favicon_metadata = json.loads(analysis.builtin_data_path("razer-shell", "HOST_DEVICE_FAVICONS").read_bytes())
    prepared = []
    expanded = []
    muted_animation_cache = {}
    verified_svg_producers = {}
    svg_producer_snapshots = []
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
        if "reuse_locked_alias" in request:
            if not receipt.get("source_current_during_acquisition"):
                raise ValueError("Reused media requires a stable current acquisition")
            alias_proof = resource_sources.verified_alias_reuse(content, request["reuse_locked_alias"],
                normalize_source_lf=request.get("source_lf_to_prepared_crlf", False), root=ROOT, mappings=mappings)
            locked = alias_proof["locked_file"]
            entry = {"source": relative(source), "source_url": row["url"],
                     "source_sha256": source_hash, "source_bytes": len(content),
                     "output": locked["path"], "sha256": locked["sha256"], "display_paths": [display],
                     "acquisition": relative(receipt_path), "locked_alias_reuse": alias_proof,
                     "preparation": "Current original bytes verified against the existing fixed resource; no media materialized"}
            manifest["entries"] = [known for known in manifest["entries"] if display not in known.get("display_paths", [])]
            manifest["entries"].append(entry)
            aliases[:] = [known for known in aliases if known["key"] != display]
            aliases.append({"key": display, "path": locked["path"]})
            prepared.append({"display": display, "source_sha256": source_hash,
                             "output_sha256": locked["sha256"], "output": locked["path"],
                             "locked_alias_reuse": alias_proof})
            continue
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
            if source.suffix.lower() != ".js":
                raise ValueError("Inline resource extraction requires an acquired JS original")
            content, transformation = extract_inline_resource_png(content, request["inline_data_sha256"])
        if "inline_css_png" in request:
            if source.suffix.lower() != ".css":
                raise ValueError("CSS bitmap extraction requires an acquired CSS original")
            content, transformation = extract_inline_css_png(content, request["inline_css_png"])
        if "svg_remove_root_ids" in request:
            if source.suffix.lower() != ".svg":
                raise ValueError("Source SVG root filtering requires an SVG original")
            producer = verified_svg_root_filter_producer(request["producer"], original_rows,
                snapshots=svg_producer_snapshots, cache=verified_svg_producers)
            content, transformation = filter_svg_root_children(content, request["svg_remove_root_ids"])
            transformation["producer"] = producer
        if source.suffix.lower() == ".avif":
            content, conversion = avif_display_png(source)
        if request.get("muted_video_animation") and source_hash in muted_animation_cache:
            content, conversion = muted_animation_cache[source_hash]
        elif request.get("muted_video_animation"):
            if source.suffix.lower() != ".mp4" or not display.endswith(".webp"):
                raise ValueError("Muted source video preparation requires MP4 and animated WebP")
            content, conversion = muted_video_display(source)
            muted_animation_cache[source_hash] = content, conversion
        if any(path.read_bytes() != raw for path, raw in svg_producer_snapshots):
            raise ValueError("Source SVG producer or parser changed during preparation")
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
    if any(path.read_bytes() != raw for path, raw in svg_producer_snapshots):
        raise ValueError("Source SVG producer or parser changed during preparation")
    if reuse_only:
        result_path = ROOT / ".analysis/results/typed-data.json"
        result = json.loads(result_path.read_bytes()) if result_path.is_file() else {"schema_version": 1}
        retained = result.setdefault("resource_alias_reuse", [])
        changed_aliases = {display for row in manifest["entries"] for display in row["display_paths"]}
        retained[:] = [row for row in retained if not changed_aliases.intersection(row["display_paths"])]
        retained.extend(manifest["entries"])
        write_json(result_path, result)
    else:
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


def prepare_app_picker(*, check=False):
    """Audit current menu art against pinned files without restoring media."""
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
        host_aliases = json.loads(analysis.builtin_data_path("razer-shell", "HOST_DEVICE_FAVICONS").read_bytes())
        products, proofs, cache = {}, [], {}
        mappings = resource_sources.load_resource_mappings()
        for pid in sorted(registered):
            acquisitions = [path for family in families.FAMILY_CANDIDATES
                            if (path := ROOT / f".analysis/inputs/frontend/{family}-{pid}-current/acquisition.json").is_file()]
            if len(acquisitions) != 1:
                raise ValueError("Current product favicon acquisition is absent or ambiguous: " + str(pid))
            proof = current_product_favicon(pid, relative(acquisitions[0]), cache)
            category, _, subcategory = Path(proof["first_favicon_element"]["href"]).stem.partition("_")
            chosen = selected(category, subcategory)
            # Category fallback supplies presentation only, never device state.
            asset = aliases[chosen] if chosen else host_aliases[str(pid)]
            products[str(pid)] = {"asset": asset}
            record = {"product_id": pid, "first_favicon": proof,
                "selected_category": chosen, "runtime_alias": asset,
                "local_fallback": "current first favicon classification" if chosen else "current first favicon SVG; no category vector exists in U"}
            if chosen is None:
                content = (ROOT / proof["original"]["path"]).read_bytes()
                try:
                    record["locked_alias_reuse"] = resource_sources.verified_alias_reuse(content, asset, mappings=mappings)
                except ValueError:
                    record["locked_alias_reuse"] = resource_sources.verified_alias_reuse(
                        content, asset, normalize_source_lf=True, mappings=mappings)
            proofs.append(record)
        if set(map(int, products)) != registered:
            raise ValueError("App menu local category fallbacks do not cover the exact registry")
        prepared = []
        for category, alias in aliases.items():
            content, transformation = static_app_picker_category_svg(root_text, category)
            prepared.append({"display": alias, "transformation": transformation,
                "locked_alias_reuse": resource_sources.verified_alias_reuse(content, alias, mappings=mappings)})
        output = analysis.builtin_data_path("razer-app-pages", "APP_PICKER_ICONS_DATA")
        document = {"subcategory_categories": list(categories), "icons": aliases, "products": products}
        equivalent = output.is_file() and json.loads(output.read_bytes()) == document
        if check and not equivalent:
            raise ValueError("Current app picker directory differs from its packaged maintenance source")
        if not equivalent:
            write_json(output, document)
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
            "output": families.identity(output), "prepared": prepared,
            "packaged_maintenance_projection_matches_current_source": True,
            "media_materialized": False, "check_only": check,
            "runtime_acceptance": "unverified; no application/window/vendor execution"}
        report_path = ROOT / ".analysis/results/frontend/rz-app-menu-current/device-artwork.json"
        write_json(report_path, report)
        return {"report": relative(report_path), "registered_products": len(products),
                "category_icons": len(aliases), "target_execution": False,
                "media_materialized": False, "check_only": check}


VERIFICATION_TEMP_LEDGER = ".analysis/verification/temporary-directories.json"
VERIFICATION_TEMP_LOCK = ".analysis/verification/temporary-directories.lock"


def _verification_path(name, *, root=None, temporary=False):
    root = ROOT if root is None else root
    if (not isinstance(name, str) or "\\" in name or ":" in name or Path(name).is_absolute()
            or ".." in Path(name).parts or Path(name).as_posix() != name):
        raise ValueError("Invalid verification ownership path")
    if temporary and not re.fullmatch(r"target/verification-temp/(?:v-|source-assemble-)[A-Za-z0-9_-]+", name):
        raise ValueError("Outside verification temporary scope")
    path = root / name
    for item in [root, *(root / Path(*Path(name).parts[:n]) for n in range(1, len(Path(name).parts) + 1))]:
        if (item.exists() or item.is_symlink()) and _scratch_link(item):
            raise ValueError("Verification ownership path contains a link or reparse point")
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("Verification ownership path escaped workspace")
    return path


def _verification_identity(path, root):
    stat = path.stat()
    if not path.is_dir():
        raise ValueError("Verification owner is not a directory")
    return {"path": path.relative_to(root).as_posix(), "inode": stat.st_ino, "device": stat.st_dev,
            "birthtime_ns": getattr(stat, "st_birthtime_ns", None)}


def _verification_record_valid(row):
    if not isinstance(row, dict) or row.get("mode") != "verification_temp_owner":
        return False
    identity = row.get("directory")
    if (not isinstance(identity, dict) or set(identity) != {"path", "inode", "device", "birthtime_ns"}
            or not isinstance(identity.get("path"), str)
            or not re.fullmatch(r"target/verification-temp/(?:v-|source-assemble-)[A-Za-z0-9_-]+", identity["path"])
            or any(type(identity.get(key)) is not int for key in ("inode", "device"))
            or identity["inode"] <= 0 or identity["device"] < 0
            or identity["birthtime_ns"] is not None and type(identity["birthtime_ns"]) is not int
            or not isinstance(row.get("suite"), str) or not row["suite"]
            or row.get("status") not in {"active", "released", "release_failed", "recovery_pending"}):
        return False
    try:
        if datetime.fromisoformat(row["registered_at_utc"]).tzinfo is None:
            return False
    except (KeyError, TypeError, ValueError):
        return False
    if row.get("registration") == "live_owner":
        creator = row.get("creator")
        if not isinstance(creator, dict) or type(creator.get("pid")) is not int or creator["pid"] <= 0:
            return False
        try:
            if datetime.fromisoformat(creator["started_at_utc"]).tzinfo is None:
                return False
        except (KeyError, TypeError, ValueError):
            return False
        creation = creator.get("process_creation")
        return (creation is None or isinstance(creation, dict)
                and type(creation.get("filetime_100ns")) is int and creation["filetime_100ns"] > 0
                and type(creation.get("precision_100ns")) is int and creation["precision_100ns"] > 0
                and creation.get("source") in {"GetProcessTimes", "Win32_Process.CreationDate"})
    if row.get("registration") != "explicit_legacy_recovery" or row.get("creator") is not None:
        return False
    recovery = row.get("recovery")
    return (isinstance(recovery, dict) and recovery.get("association") == "time_and_entry_association"
            and recovery.get("creation_time_owner_record_available") is False
            and isinstance(recovery.get("source"), dict)
            and isinstance(recovery["source"].get("path"), str)
            and recovery["source"]["path"].startswith(".analysis/results/")
            and recovery["source"]["path"].endswith(".json")
            and re.fullmatch(r"[a-f0-9]{64}", recovery["source"].get("sha256", "")) is not None
            and isinstance(recovery.get("pointer"), str) and recovery["pointer"].startswith("/")
            and type(recovery.get("session_id")) is int and recovery["session_id"] > 0
            and isinstance(recovery.get("chunk_id"), str) and bool(recovery["chunk_id"]))


def _verification_ledger(root):
    path = _verification_path(VERIFICATION_TEMP_LEDGER, root=root)
    if not path.exists():
        return {"schema_version": 1, "mode": "verification_temporary_ownership", "records": []}
    result = json.loads(path.read_bytes())
    if (result.get("schema_version") != 1 or result.get("mode") != "verification_temporary_ownership"
            or not isinstance(result.get("records"), list)
            or not all(_verification_record_valid(row) for row in result["records"])
            or len({json.dumps(row["directory"], sort_keys=True) for row in result["records"]}) != len(result["records"])):
        raise ValueError("Invalid verification temporary ownership ledger")
    return result


def _verification_save(root, ledger):
    import analysis
    path = _verification_path(VERIFICATION_TEMP_LEDGER, root=root)
    analysis.save(path, ledger)
    with path.open("r+b") as stream:
        os.fsync(stream.fileno())


@contextmanager
def verification_environment(name, *, root=None):
    """Durably register one live owner before its command can create test files."""
    import analysis
    from reverse_pending import process_creation_time
    if not isinstance(name, str) or not name:
        raise ValueError("Verification owner name must be a nonempty string")
    root = ROOT if root is None else root.resolve()
    lock = _verification_path(VERIFICATION_TEMP_LOCK, root=root)
    directory = _verification_path("target/verification-temp", root=root)
    directory.mkdir(parents=True, exist_ok=True)
    temporary, row, registered = None, None, False
    outcome = "failed"
    try:
        with analysis.index_lock(lock):
            ledger = _verification_ledger(root)
            # A short unique child keeps AF_UNIX fixture paths within sun_path.
            temporary = tempfile.TemporaryDirectory(prefix="v-", dir=directory)
            path = _verification_path(Path(temporary.name).relative_to(root).as_posix(), root=root, temporary=True)
            started = datetime.now(timezone.utc).isoformat()
            row = {"mode": "verification_temp_owner", "directory": _verification_identity(path, root),
                   "suite": name, "registration": "live_owner", "registered_at_utc": started,
                   "creator": {"pid": os.getpid(), "process_creation": process_creation_time(os.getpid()),
                               "started_at_utc": started}, "status": "active"}
            ledger["records"].append(row)
            _verification_save(root, ledger)
            registered = True
        environment = os.environ.copy()
        for variable in ("TEMP", "TMP", "TMPDIR"):
            environment[variable] = str(path)
        try:
            yield environment, row["directory"]["path"]
            outcome = "succeeded"
        except KeyboardInterrupt:
            outcome = "cancelled"
            raise
    finally:
        if temporary is not None:
            released, error = False, None
            try:
                path = _verification_path(Path(temporary.name).relative_to(root).as_posix(), root=root, temporary=True)
                if row is not None and _verification_identity(path, root) != row["directory"]:
                    temporary._finalizer.detach()
                    raise ValueError("Verification temporary identity changed before release")
                temporary.cleanup()  # The live owner also releases nonempty test subtrees.
                released = not path.exists()
            except BaseException as failure:
                temporary._finalizer.detach()
                error = failure
                raise
            finally:
                if registered:
                    with analysis.index_lock(lock):
                        ledger = _verification_ledger(root)
                        owned = next(item for item in ledger["records"] if item["directory"] == row["directory"])
                        owned.update(status="released" if released else "release_failed", outcome=outcome,
                                     finished_at_utc=datetime.now(timezone.utc).isoformat())
                        if error is not None:
                            owned["release_error"] = str(error)
                        _verification_save(root, ledger)


def _verification_recovery_source(recovery):
    source = recovery["source"]
    name = source.get("path")
    if not isinstance(name, str) or not name.startswith(".analysis/results/") or not name.endswith(".json"):
        raise ValueError("Recovery requires an existing maintained interruption receipt")
    path = _verification_path(name)
    actual = _clean_file_identity(path)
    if actual["sha256"] != source.get("sha256") or any(actual.get(key) != value for key, value in source.items()):
        raise ValueError("Verification recovery source changed")
    value = json.loads(path.read_bytes())
    for part in recovery["pointer"].split("/")[1:]:
        part = part.replace("~1", "/").replace("~0", "~")
        value = value[int(part)] if isinstance(value, list) else value[part]
    output_reference = value.get("raw_output_reference") if isinstance(value, dict) else None
    if (not isinstance(value, dict) or value.get("suite_run_interrupted_after_failure") is not True
            or not isinstance(output_reference, dict)
            or output_reference.get("session_id") != recovery["session_id"]
            or output_reference.get("chunk_id") != recovery["chunk_id"]
            or not isinstance(value.get("argv"), list) or value["argv"][:2] not in (["cargo", "check"], ["cargo", "test"])):
        raise ValueError("Recovery source does not record the specified interrupted verification")
    return actual


def _verification_register_recovery(proof):
    import analysis
    if not isinstance(proof, dict) or set(proof) != {"path", "identity", "source", "pointer", "session_id", "chunk_id",
                                                    "association", "creation_time_owner_record_available"}:
        raise ValueError("Invalid explicit verification recovery fields")
    path = _verification_path(proof["path"], temporary=True)
    identity = _verification_identity(path, ROOT)
    if proof["identity"] != {key: value for key, value in identity.items() if key != "path"} or any(path.iterdir()):
        raise ValueError("Explicit recovery identity changed or directory is nonempty")
    recovery = {key: proof[key] for key in ("source", "pointer", "session_id", "chunk_id", "association",
                                          "creation_time_owner_record_available")}
    row = {"mode": "verification_temp_owner", "directory": identity, "suite": "interrupted_verification",
           "registration": "explicit_legacy_recovery", "creator": None, "recovery": recovery,
           "registered_at_utc": datetime.now(timezone.utc).isoformat(), "status": "recovery_pending"}
    if not _verification_record_valid(row):
        raise ValueError("Invalid explicit verification recovery provenance")
    recovery["source"] = _verification_recovery_source(recovery)
    with analysis.index_lock(_verification_path(VERIFICATION_TEMP_LOCK)):
        ledger = _verification_ledger(ROOT)
        previous = next((item for item in ledger["records"] if item["directory"] == identity), None)
        if previous is not None:
            if previous.get("registration") != row["registration"] or previous.get("recovery") != recovery:
                raise ValueError("Verification directory already has a different owner")
        else:
            ledger["records"].append(row)
            _verification_save(ROOT, ledger)


def _verification_tree(name):
    path = _verification_path(name, temporary=True)
    if any(path.iterdir()):
        raise ValueError("Recovered verification directory is nonempty")
    return {"files": [], "directories": [name], "bytes": 0,
            "directory_identity": _verification_identity(path, ROOT), "mtime_ns": path.stat().st_mtime_ns}


def _verification_delete_empty(row):
    """Remove only the unchanged empty directory, holding its Windows delete handle."""
    import ctypes
    from ctypes import wintypes
    from analysis_storage import FileInfo, StandardInfo
    if os.name != "nt":
        raise ValueError("Exclusive verification recovery deletion requires Windows")
    if _verification_tree(row["path"]) != row["tree"]:
        raise ValueError("Verification directory changed before exclusive deletion")
    path = _verification_path(row["path"], temporary=True)
    api = ctypes.WinDLL("kernel32", use_last_error=True)
    api.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, wintypes.LPVOID,
                               wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    api.CreateFileW.restype = wintypes.HANDLE
    api.GetFileInformationByHandle.argtypes = [wintypes.HANDLE, ctypes.POINTER(FileInfo)]
    api.GetFileInformationByHandleEx.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD]
    api.SetFileInformationByHandle.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD]
    api.CloseHandle.argtypes = [wintypes.HANDLE]

    class FileIdInfo(ctypes.Structure):
        _fields_ = [("volume", ctypes.c_ulonglong), ("identity", ctypes.c_ubyte * 16)]

    handles = []
    try:
        for parent in (ROOT / "target", path.parent):
            handle = api.CreateFileW(str(parent), 0x80, 3, None, 3, 0x02200000, None)
            if handle == ctypes.c_void_p(-1).value:
                raise OSError(ctypes.get_last_error(), "verification_parent_guard_unavailable")
            handles.append(handle)
            if _scratch_link(parent):
                raise ValueError("Verification parent became a reparse point")
        handle = api.CreateFileW(str(path), 0x10081, 0, None, 3, 0x02200000, None)
        if handle == ctypes.c_void_p(-1).value:
            raise OSError(ctypes.get_last_error(), "exclusive_verification_directory_guard_unavailable")
        handles.append(handle)
        info, file_id, standard = FileInfo(), FileIdInfo(), StandardInfo()
        if (not api.GetFileInformationByHandle(handle, ctypes.byref(info))
                or not api.GetFileInformationByHandleEx(handle, 18, ctypes.byref(file_id), ctypes.sizeof(file_id))
                or not api.GetFileInformationByHandleEx(handle, 1, ctypes.byref(standard), ctypes.sizeof(standard))):
            raise ctypes.WinError(ctypes.get_last_error())
        expected = row["tree"]["directory_identity"]
        birth = ((info.creation.dwHighDateTime << 32 | info.creation.dwLowDateTime) - 116444736000000000) * 100
        modified = ((info.write.dwHighDateTime << 32 | info.write.dwLowDateTime) - 116444736000000000) * 100
        if (info.attributes & 0x400 or not info.attributes & 0x10 or not standard.directory
                or int.from_bytes(bytes(file_id.identity), "little") != expected["inode"]
                or file_id.volume != expected["device"] or birth != expected["birthtime_ns"]
                or modified != row["tree"]["mtime_ns"] or standard.allocated < 0):
            raise ValueError("Exclusive verification directory identity changed")
        active, errors = _clean_process_guard([row])
        if active or errors:
            raise ValueError("Verification activity changed before deletion")
        for basis in row["basis"]:
            if _clean_file_identity(_verification_path(basis["path"])) != basis:
                raise ValueError("Verification ownership evidence changed before deletion")
        disposition = wintypes.DWORD(1)
        # Windows rejects a nonempty directory; no child is ever traversed or deleted.
        if not api.SetFileInformationByHandle(handle, 21, ctypes.byref(disposition), ctypes.sizeof(disposition)):
            raise ctypes.WinError(ctypes.get_last_error())
    finally:
        for handle in reversed(handles):
            api.CloseHandle(handle)
    if path.exists():
        raise ValueError("Verification directory still exists after deletion")
    return {"allocated_before": standard.allocated, "allocated_after": 0, "bytes_reclaimed": standard.allocated}


def _verification_candidates():
    from reverse_pending import coordinator_instance_absence
    rows, retained = [], []
    ledger = _verification_ledger(ROOT)
    owners = {row["directory"]["path"]: row for row in ledger["records"]}
    directory = _verification_path("target/verification-temp")
    for path in sorted(directory.iterdir()) if directory.is_dir() else []:
        name = path.relative_to(ROOT).as_posix()
        try:
            _verification_path(name, temporary=True)
            row = owners.get(name)
            if row is None or not _verification_record_valid(row):
                raise ValueError("unregistered_verification_directory")
            if row["status"] == "released":
                raise ValueError("released_owner_path_reappeared")
            if _verification_identity(path, ROOT) != row["directory"]:
                raise ValueError("verification_directory_identity_changed")
            _verification_tree(name)
            basis = [_clean_file_identity(_verification_path(VERIFICATION_TEMP_LEDGER))]
            if row["registration"] == "live_owner":
                if coordinator_instance_absence(row["creator"]).get("confirmed_absent") is not True:
                    raise ValueError("verification_creator_still_active_or_unknown")
            else:
                basis.append(_verification_recovery_source(row["recovery"]))
            rows.append({"path": name, "kind": "owned_empty_verification_temp", "basis": basis,
                         "owner_receipts": [VERIFICATION_TEMP_LEDGER], "ownership": row})
        except (OSError, ValueError, KeyError, TypeError) as error:
            retained.append({"path": name, "reason": str(error)})
    return rows, retained


def _verification_finish_cleanup(receipt):
    removed = {item["path"] for item in receipt["removed"]}
    owners = [row["ownership"] for row in receipt["scope"]
              if row["kind"] == "owned_empty_verification_temp" and row["path"] in removed]
    if owners:
        ledger = _verification_ledger(ROOT)
        for row in ledger["records"]:
            if any(row == owner for owner in owners):
                row.update(status="released", outcome="planned_empty_directory_recovery",
                           finished_at_utc=datetime.now(timezone.utc).isoformat(),
                           cleanup_plan=receipt["plan"], cleanup_plan_sha256=receipt["plan_sha256"])
        _verification_save(ROOT, ledger)


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
    if type(workers) is not int or not 2 <= workers <= 12:
        raise ValueError("Dashboard resource workers must be between two and twelve")
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
    # The retired development manifest is not a current source boundary.  Use
    # only indexed retained inputs for exact URL parameters; absent historical
    # entries remain explicit acquisition gaps rather than being resurrected.
    resource_manifest_path = None
    retained_sources = input_index["inputs"]
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
                               "routes": retained_routes,
                               "usage": "Retained official URL parameters only; every selected image is reverified against its official endpoint; no retired development manifest"})
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


def connected_result(raw):
    reports = re.findall(r"RAZER_CONNECTED_RESULT (\{[^\r\n]+\})", raw)
    return json.loads(reports[0]) if len(reports) == 1 else None


def connected_get_response(transfers, index):
    """Select the observed terminal IN after this OUT, retaining busy attempts."""
    outgoing = transfers[index]
    if outgoing["operation"] != "send_feature" or outgoing["error"] is not None:
        raise ValueError("Native comparison requires a successful observed OUT")
    replies = []
    for reply in transfers[index + 1:]:
        if reply["operation"] == "send_feature":
            break
        if reply["operation"] != "get_feature" or reply["error"] is not None or reply["count"] != 91:
            raise ValueError("Native comparison requires complete observed IN reports")
        raw, request = reply["bytes"], outgoing["bytes"]
        if raw[2] != request[2] or raw[7:9] != request[7:9]:
            raise ValueError("Native comparison response transaction or command differs")
        replies.append(reply)
        if raw[1] != 1:
            return raw, len(replies)
    raise ValueError("Native comparison has no observed terminal IN response")


def record_local_runtime(name, receipt):
    import analysis
    path = ROOT / ".analysis/validation.json"
    with analysis.index_lock(path.with_suffix(".lock")):
        validation = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {"schema_version": 1}
        validation.setdefault("local_runtime_checks", {})[name] = receipt
        write_json(path, validation)


def record_service_version_evidence(receipt):
    """Index the verified private COM-server original separately from execution."""
    import analysis
    server = receipt["server"]
    name = server["input"]
    if name != ".analysis/inputs/installed-current/files/elevation-service/razer_elevation_service.exe":
        raise ValueError("Unexpected registered service evidence destination")
    original = safe(ROOT / name)
    if original.is_symlink() or original.stat().st_size != server["bytes"] or digest(original) != server["sha256"]:
        raise ValueError("Registered service original differs from the comparison receipt")
    signature = server["signature"]
    if signature["status"] != "Valid" or not signature["subject"].startswith("CN=Razer USA Ltd.,"):
        raise ValueError("Registered service signature is not verified Razer")
    acquisition = ROOT / ".analysis/inputs/installed-current/service-acquisition.json"
    row = {"path": name, "bytes": server["bytes"], "sha256": server["sha256"],
           "kind": "installed_static_evidence", "source_root": "registered-elevation-service",
           "source_relative_path": original.name,
           "acquired_at_utc": receipt["recorded_at_utc"],
           "registry_key": "CLSID/{" + receipt["evidence"]["com_identity"]["clsid"]["value"] + "}/LocalServer32"}
    write_json(acquisition, {"schema_version": 1, "files": [row], "architecture": server["architecture"],
        "versions": server["versions"], "signature": signature, "target_execution": False,
        "source": "Registered local COM server; private bytes verified before the scoped comparison",
        "runtime_receipt": ".analysis/validation.json#/local_runtime_checks/simple_version"})
    receipt_row = {"path": relative(acquisition), "bytes": acquisition.stat().st_size,
                   "sha256": digest(acquisition), "kind": "acquisition_receipt"}
    analysis.index_files([row, receipt_row], publication={"owner": "tools/workspace.py",
        "reason": "Register the verified COM-server original used by the service-version comparison",
        "receipt": receipt_row["path"], "verification": {"private_bytes_rechecked": True,
            "registered_server_signature": signature["status"], "architecture": server["architecture"]}})


def compare_local_service_version():
    import analysis
    import native_runtime
    receipt = native_runtime.compare_local_service_version(ROOT)
    receipt["recorded_at_utc"] = datetime.now(timezone.utc).isoformat()
    record_service_version_evidence(receipt)
    audit_path = ROOT / ".analysis/results/host-current/local-audit.json"
    with analysis.index_lock(audit_path.with_suffix(".lock")):
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
        audit["simple_version"] = receipt["evidence"]
        write_json(audit_path, audit)
    record_local_runtime("simple_version", receipt)
    print(json.dumps({"receipt": ".analysis/validation.json#/local_runtime_checks/simple_version",
                      "passed": receipt["passed"], "result": receipt["result"],
                      "exit_code": receipt["exit_code"], "timed_out": receipt["timed_out"]}, ensure_ascii=False), flush=True)
    return 0 if receipt["passed"] else 1


def compare_local_lighting():
    import analysis
    import native_runtime
    path = ROOT / ".analysis/results/lighting-runtime-comparison.json"
    with analysis.index_lock(path.with_suffix(".lock")):
        previous = json.loads(path.read_bytes()) if path.exists() else None
        native_runtime.lighting_attempt_history(previous)
    retained = None

    def publish(receipt):
        nonlocal retained
        with analysis.index_lock(path.with_suffix(".lock")):
            previous = json.loads(path.read_bytes()) if path.exists() else None
            retained = native_runtime.retain_lighting_attempt(previous, receipt)
            write_json(path, retained)

    receipt = native_runtime.compare_local_lighting(ROOT, rust_inputs, publish=publish)
    print(json.dumps({"receipt": relative(path), "passed": receipt["passed"],
                      "attempt": retained["latest_attempt"], "attempt_count": len(retained["attempts"]),
                      "comparisons": receipt.get("comparisons", []), "error": receipt.get("error"),
                      "scope": receipt["scope"], "unverified": receipt["unverified"]}, ensure_ascii=False), flush=True)
    return 0 if receipt["passed"] else 1


def compare_local_app_files():
    import analysis
    import native_runtime
    receipt = native_runtime.compare_local_app_files(ROOT)
    receipt["recorded_at_utc"] = datetime.now(timezone.utc).isoformat()
    audit_path = ROOT / ".analysis/results/host-current/local-audit.json"
    with analysis.index_lock(audit_path.with_suffix(".lock")):
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
        audit["simple_app_inventory"] = receipt["evidence"]
        write_json(audit_path, audit)
    record_local_runtime("simple_app_inventory", receipt)
    result = receipt["result"] or {}
    comparisons = result.get("comparisons", {}).get("Ok", [])
    print(json.dumps({"receipt": ".analysis/validation.json#/local_runtime_checks/simple_app_inventory",
                      "passed": receipt["passed"], "comparisons": [
                          {key: item[key] for key in ("name", "matched", "input_unchanged")}
                          for item in comparisons], "cleanup": result.get("cleanup"),
                      "exit_code": receipt["exit_code"], "timed_out": receipt["timed_out"]}, ensure_ascii=False), flush=True)
    return 0 if receipt["passed"] else 1


def compare_connected_hid(library):
    """Compare exact observed GET packets with the verified source C exports."""
    import native_runtime
    source_path = ROOT / ".analysis/results/connected-device-verification.json"
    source_bytes = source_path.read_bytes()
    source = json.loads(source_bytes)
    if source.get("mode") != "read-only" or source.get("result") is None:
        raise ValueError("Run validate-connected-devices read-only first")
    baseline = rust_inputs()
    if source.get("inputs_unchanged") is not True or source.get("inputs") != baseline:
        raise ValueError("Connected-device trace does not match current unchanged Rust inputs; rerun read-only verification")
    if source["result"].get("backend_released") is not True:
        raise ValueError("Connected-device trace has no confirmed backend release")
    requests, expected, skipped, rust_get_attempts = [], [], [], []
    for device in source["result"]["devices"]:
        actual = device.get("result")
        if actual is None:
            skipped.append({"product_id":device["product_id"], "error":device["error"]})
            continue
        transfers = actual["transfers"]
        for index, transfer in enumerate(transfers):
            if transfer["operation"] != "send_feature":
                continue
            correlation = f"{device['product_id']}:{index}"
            request = {"path":actual["path"], "report":transfer["bytes"], "trace_index":correlation}
            try:
                native_runtime.validate_get_trace([request])
            except ValueError as error:
                skipped.append({"trace_index":correlation, "command":transfer["bytes"][6:9], "error":str(error)})
                continue
            reply, attempts = connected_get_response(transfers, index)
            requests.append(request)
            expected.append(reply)
            rust_get_attempts.append(attempts)
    if not requests:
        raise ValueError("No audited complete GET transfers are available for native comparison")
    runtime = native_runtime.replay_get_trace(requests, library, timeout_seconds=30)
    failures = []
    if runtime["status"] != "completed" or not runtime.get("worker_waited"):
        failures.append("native_worker_did_not_complete")
    result = runtime.get("result") or {}
    queries = result.get("queries", [])
    if result.get("error") or result.get("init_return") != 0 or len(queries) != len(requests):
        failures.append("native_queries_incomplete")
    cleanup = result.get("cleanup", [])
    if (not cleanup or any(not row.get("returned") for row in cleanup)
            or not any(row["operation"] == "hid_exit" and row.get("return_value") == 0 for row in cleanup)
            or cleanup[-1].get("operation") != "release_library"):
        failures.append("native_cleanup_unconfirmed")
    comparisons = []
    for query, request, rust_reply, attempts in zip(queries, requests, expected, rust_get_attempts):
        received = query.get("received_bytes")
        same = (query.get("trace_index") == request["trace_index"] and query.get("send_return") == 91
                and query.get("get_return") == 91 and not query.get("native_error") and received == rust_reply)
        comparisons.append({"trace_index":request["trace_index"], "command":request["report"][6:9],
                            "response_status":query.get("response_status"), "bytes_equal":same,
                            "rust_response":rust_reply,
                            "rust_get_attempts":attempts,
                            "different_indices":[index for index, pair in enumerate(zip(rust_reply, received or [])) if pair[0] != pair[1]]})
        if not same:
            failures.append({"trace_index":request["trace_index"], "error":"native_response_differs"})
    trace_unchanged = source_bytes == source_path.read_bytes()
    unchanged = baseline == rust_inputs()
    receipt = {"schema_version":1, "updated_at_utc":datetime.now(timezone.utc).isoformat(),
               "source_trace":relative(source_path), "source_trace_sha256":hashlib.sha256(source_bytes).hexdigest(),
               "source_trace_unchanged":trace_unchanged,
               "inputs":baseline, "inputs_unchanged":unchanged, "runtime":runtime,
               "comparisons":comparisons, "skipped":skipped, "failures":failures,
               "passed":unchanged and trace_unchanged and not failures,
               "scope":"Exact byte parity for the selected GETs; a matched offline/error response does not confirm the unavailable device setting"}
    path = ROOT / ".analysis/results/connected-native-comparison.json"
    write_json(path, receipt)
    print(json.dumps({"receipt":relative(path), "passed":receipt["passed"], "compared":len(comparisons),
                      "skipped":len(skipped), "failures":failures}, ensure_ascii=False), flush=True)
    return 0 if receipt["passed"] else 1


def wait_connected_process(process, output, *, write_restore):
    try:
        return process.wait(timeout=240), False
    except subprocess.TimeoutExpired:
        if write_restore:
            # A SET may already have reached the device. Wait for the test's
            # independent restoration path instead of killing its owner.
            output.write(b"Connected-device deadline exceeded; waiting for original-value restoration and release.\n")
            output.flush()
            return process.wait(), True
        subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                       stdout=output, stderr=subprocess.STDOUT, timeout=30, check=False)
        return process.wait(timeout=30), True


def connected_execution_scope(raw, result, *, write_restore):
    baselines = [json.loads(row) for row in re.findall(r"RAZER_CONNECTED_BASELINE (\{[^\r\n]+\})", raw)]
    admitted = bool(re.search(r"RAZER_CONNECTED_STARTED \{", raw))
    attempted = admitted or bool(result or baselines)
    executed = True if result or baselines else None if admitted else False
    operations = None if admitted else False
    if result:
        operations = any(row.get("result") and row["result"].get("transfers") for row in result["devices"])
    if baselines:
        operations = True
    restoration = ("recorded_per_query" if write_restore and result else
                   "unconfirmed" if write_restore and baselines else "not_attempted")
    return {"target_execution_attempted":attempted, "target_execution":executed,
            "real_device_operations":operations, "write_baselines":baselines,
            "mutation_may_have_started":write_restore and bool(baselines),
            "restoration_confirmation":restoration}


def validate_connected_devices(products, *, write_restore=False):
    """Explicit hardware checks through the production HID adapter."""
    import platform
    if sys.platform != "win32":
        raise ValueError("Connected-device DLL checks currently require Windows")
    if not products or any(product <= 0 for product in products) or len(set(products)) != len(products):
        raise ValueError("Select distinct actual logical product IDs")
    baseline = rust_inputs()
    destination = ROOT / ".analysis/verification"
    destination.mkdir(parents=True, exist_ok=True)
    mode = "write-restore" if write_restore else "read-only"
    suffix = "connected-device-writes" if write_restore else "connected-devices"
    test = "connected_device_write_restore" if write_restore else "connected_device_queries"
    log = destination / (suffix + ".txt")
    command = ["cargo", "test", "--locked", "--offline", "-p", "razer-hid", "--test", "connected_devices",
               test, "--", "--ignored", "--exact", "--nocapture", "--test-threads=1"]
    environment = os.environ.copy()
    environment["RAZER_CONNECTED_DEVICE_TEST"] = mode
    environment["RAZER_CONNECTED_PRODUCTS"] = ",".join(map(str, products))
    started = datetime.now(timezone.utc).isoformat()
    print(json.dumps({"checking":"connected-devices", "mode":mode, "products":products, "argv":command}), flush=True)
    with log.open("wb") as output:
        with subprocess.Popen(command, cwd=ROOT, env=environment, stdout=output, stderr=subprocess.STDOUT) as process:
            exit_code, timed_out = wait_connected_process(process, output, write_restore=write_restore)
    raw = log.read_text(encoding="utf-8", errors="replace")
    result = connected_result(raw)
    failures = []
    if timed_out:
        failures.append("deadline_exceeded_waited_for_restoration" if write_restore else "process_tree_deadline_expired")
    if exit_code:
        failures.append("command_failed")
    if result is None:
        failures.append("missing_runtime_result")
    else:
        if (result.get("mode") != mode or result.get("backend_released") is not True
                or [row["product_id"] for row in result["devices"]] != products):
            failures.append("runtime_scope_or_cleanup_differs")
        for row in result["devices"]:
            if row["error"] is not None:
                failures.append({"product":row["product_id"], "error":row["error"]})
                continue
            if not row["result"]["readings"] or row["result"].get("transport_released") is not True:
                failures.append({"product":row["product_id"], "error":"no readings or transport release receipt"})
            for reading in row["result"]["readings"]:
                value = reading["value"]
                if (reading["error"] is not None or value is None
                        or isinstance(value, dict) and (value.get("kind") == "unknown" or value.get("verified") is False)):
                    failures.append({"product":row["product_id"], "query":reading["query"], "error":reading["error"], "value":value})
    libraries = []
    for filename in ("hid.dll", "setupapi.dll", "cfgmgr32.dll"):
        path = Path(os.environ["SystemRoot"]) / "System32" / filename
        libraries.append({"path":"%SystemRoot%/System32/" + filename, "bytes":path.stat().st_size, "sha256":digest(path)})
    unchanged = baseline == rust_inputs()
    receipt = {"schema_version":1, "started_at_utc":started, "finished_at_utc":datetime.now(timezone.utc).isoformat(),
               "host":platform.platform(), "machine":platform.machine(), "mode":mode, "argv":command,
               "products":products, "inputs":baseline, "inputs_unchanged":unchanged, "exit_code":exit_code,
               "system_libraries":libraries, "log":relative(log), "log_sha256":digest(log), "result":result,
               "failures":failures, **connected_execution_scope(raw, result, write_restore=write_restore),
               "application_runtime_acceptance":"not performed", "write_acceptance":mode if write_restore else "not performed",
               "passed":unchanged and not failures}
    path = ROOT / ".analysis/results" / ("connected-device-write-verification.json" if write_restore else "connected-device-verification.json")
    write_json(path, receipt)
    print(json.dumps({"receipt":relative(path), "passed":receipt["passed"], "inputs_unchanged":unchanged,
                      "failures":failures}, ensure_ascii=False), flush=True)
    return 0 if receipt["passed"] else 1


def rust_test_inventory(package, target=None):
    """Resolve test owners and window calls without executing Rust."""
    import resource_scope
    import rust_data
    parser = resource_scope.parser(Path(__file__).resolve().parent.parent)
    crate = ROOT / "crates" / package
    manifest = tomllib.loads((crate / "Cargo.toml").read_text(encoding="utf-8"))
    if manifest["package"]["name"] != package:
        raise ValueError("Rust test package identity differs: " + package)
    if target is None:
        entry = crate / manifest.get("lib", {}).get("path", "src/lib.rs")
    else:
        declarations = [row for row in manifest.get("test", []) if row["name"] == target]
        if len(declarations) > 1:
            raise ValueError("Rust integration test target is ambiguous: " + target)
        entry = crate / (declarations[0].get("path", "tests/" + target + ".rs") if declarations
                         else "tests/" + target + ".rs")
        if not entry.is_file() and not declarations:
            entry = crate / "tests" / target / "main.rs"
    functions, sources, trees, loaded, unresolved_includes = {}, {}, {}, set(), []

    def text(node):
        return node.text.decode("utf-8") if node is not None else ""

    def attributes(node):
        rows, sibling = [], node.prev_named_sibling
        while sibling is not None and sibling.type in {"attribute_item", "line_comment", "block_comment"}:
            if sibling.type == "attribute_item":
                rows.append("".join(text(sibling).split()))
            sibling = sibling.prev_named_sibling
        return rows

    def walk(node):
        if node is None:
            return
        yield node
        for child in node.named_children:
            yield from walk(child)

    def source(path, ancestors):
        path = product_input(path, ancestors[-1] if ancestors else path)
        if path in ancestors:
            raise ValueError("Recursive Rust test module or include: " + relative(path))
        if not path.is_file():
            raise ValueError("Rust test module or include is absent: " + relative(path))
        if path not in trees:
            raw = path.read_bytes()
            sources[relative(path)] = hashlib.sha256(raw).hexdigest()
            tree = parser.parse(raw)
            if tree.root_node.has_error:
                raise ValueError("Rust test module has unresolved syntax: " + relative(path))
            trees[path] = tree
        return path, trees[path]

    def unresolved(invocation, path, module, reason):
        unresolved_includes.append({"owner": relative(path), "line": invocation.start_point.row + 1,
                                    "module": "::".join(module), "expression": text(invocation), "reason": reason})

    def include_macro(node):
        if node.type == "expression_statement" and len(node.named_children) == 1:
            node = node.named_children[0]
        if node.type == "macro_invocation" and text(node.child_by_field_name("macro")) in {
                "include", "std::include", "core::include", "::std::include", "::core::include"}:
            return node
        return None

    def scope_items(node, path, module, inline, ancestors):
        for child in node.named_children:
            invocation = include_macro(child)
            if invocation is None:
                yield child, path, inline, ancestors
                continue
            token_tree = next(part for part in invocation.named_children if part.type == "token_tree")
            values = [part for part in token_tree.named_children if part.type not in {"line_comment", "block_comment"}]
            if len(values) != 1 or values[0].type not in {"string_literal", "raw_string_literal"}:
                unresolved(invocation, path, module, "nonliteral_include_path")
                continue
            if node.type not in {"source_file", "declaration_list"}:
                unresolved(invocation, path, module, "expression_include_not_expanded")
                continue
            # Decode the AST-verified literal using the shared Rust escape rules.
            _, literal = next(rust_embed_references("include!(" + text(values[0]) + ");"))
            if PureWindowsPath(literal).anchor or Path(literal).is_absolute():
                raise ValueError("Rust test include path must be project relative: " + relative(path))
            included, tree = source(path.parent / literal, ancestors)
            yield from scope_items(tree.root_node, included, module, (), (*ancestors, included))

    def load(path, module, ancestors=()):
        path, tree = source(path, ancestors)
        identity = (path, module)
        if identity in loaded:
            return

        def visit(node, current, path, inline=(), inherited_imports=None, ancestors=()):
            entries = list(scope_items(node, path, current, inline, ancestors))
            imports = dict(inherited_imports or {})
            imports.update(alias_target for child, _, _, _ in entries if child.type == "use_declaration"
                           for alias_target in rust_data.imported_paths(child))
            for child, owner, child_inline, child_ancestors in entries:
                if child.type == "mod_item":
                    name = text(child.child_by_field_name("name"))
                    body = child.child_by_field_name("body")
                    if body is not None:
                        visit(body, (*current, name), owner, (*child_inline, name), imports, child_ancestors)
                        continue
                    base = owner.parent
                    if child_inline:
                        if owner.name not in {"lib.rs", "main.rs", "mod.rs"}:
                            base /= owner.stem
                        base = base.joinpath(*child_inline)
                    literal = next((match[1] for attribute in attributes(child)
                                    if (match := re.fullmatch(r'#\[path="([^"\r\n]+)"\]', attribute))), None)
                    if literal is not None:
                        candidates = [base / literal]
                    else:
                        if not child_inline and owner.name not in {"lib.rs", "main.rs", "mod.rs"}:
                            base /= owner.stem
                        candidates = [base / (name + ".rs"), base / name / "mod.rs"]
                    found = [candidate for candidate in candidates if candidate.is_file()]
                    if len(found) != 1:
                        raise ValueError("Rust test module is absent or ambiguous: " + relative(owner) + "::" + name)
                    load(found[0], (*current, name), child_ancestors)
                elif child.type == "function_item":
                    name = text(child.child_by_field_name("name"))
                    attrs = attributes(child)
                    calls, window_calls = [], []
                    for call in walk(child.child_by_field_name("body")):
                        invocation = include_macro(call) if call.type == "macro_invocation" else None
                        if invocation is not None:
                            unresolved(invocation, owner, current, "expression_include_not_expanded")
                        if call.type != "call_expression":
                            continue
                        function = call.child_by_field_name("function")
                        if function is not None and function.type == "generic_function":
                            function = function.child_by_field_name("function")
                        method = function is not None and function.type == "field_expression"
                        target = text(function.child_by_field_name("field")) if method else text(function)
                        if target.split("::")[-1] in {"open_window", "add_window", "new_window"}:
                            window_calls.append(target)
                        calls.append((method, tuple(target.split("::"))))
                    key = (*current, name)
                    functions.setdefault(key, []).append({
                        "name": "::".join(key), "module": current, "owner": relative(owner),
                        "line": child.start_point.row + 1, "calls": calls, "imports": imports,
                        "test": any(attribute in {"#[test]", "#[::core::prelude::v1::test]", "#[core::prelude::v1::test]"}
                                    or attribute.startswith("#[gpui_kit::test") for attribute in attrs),
                        "ignored": any(attribute.startswith("#[ignore") for attribute in attrs),
                        "window_calls": window_calls,
                    })
                elif child.type not in {"macro_definition", "macro_invocation"}:
                    visit(child, current, owner, child_inline, imports, child_ancestors)

        visit(tree.root_node, module, path, ancestors=(*ancestors, path))
        loaded.add(identity)

    load(entry, ())

    def resolve(module, target):
        if target[0] == "crate":
            return target[1:]
        if target[0] == "self":
            return (*module, *target[1:])
        while target and target[0] == "super":
            module, target = module[:-1], target[1:]
        return (*module, *target)

    rows = [row for owners in functions.values() for row in owners]
    local_methods = {}
    for key, owners in functions.items():
        for owner in owners:
            for scope in (owner["owner"], owner["module"]):
                local_methods.setdefault((scope, key[-1]), set()).add(key)
    for row in rows:
        row["dependencies"] = set()
        for method, called in row["calls"]:
            if not called or not called[0]:
                continue
            if method:
                for scope in (row["owner"], row["module"]):
                    row["dependencies"].update(local_methods.get((scope, called[-1]), ()))
                continue
            if called[0] in row["imports"]:
                called = (*row["imports"][called[0]], *called[1:])
            row["dependencies"].add(resolve(row["module"], called))
    window_owners = {key for key, owners in functions.items() if any(row["window_calls"] for row in owners)}
    while True:
        inherited = {key for key, owners in functions.items()
                     if any(row["dependencies"] & window_owners for row in owners)}
        if inherited <= window_owners:
            break
        window_owners.update(inherited)
    tests = [{key: row[key] for key in ("name", "owner", "line", "ignored")}
             | {"requires_window": tuple(row["name"].split("::")) in window_owners}
             for row in rows if row["test"]]
    return {"package": package, "target": target or "lib", "sources": sources, "tests": tests,
            "unresolved_includes": unresolved_includes,
            "limits": "Rust AST, literal item include! expansion and local callable edges; no type resolution, "
                      "cfg evaluation or other macro expansion; nonliteral and expression includes remain unexpanded"}


def rust_test_scope(command, inventories):
    """Reject window tests before any ordinary verification command starts."""
    if command[1] != "test":
        return None
    packages = [command[index + 1] for index, part in enumerate(command[:-1]) if part in {"-p", "--package"}]
    if not packages or ("--lib" in command) == ("--test" in command) or any(
            part in command for part in ("--ignored", "--include-ignored")):
        raise ValueError("Ordinary Rust verification requires nonignored library or named integration tests")
    target = command[command.index("--test") + 1] if "--test" in command else None
    cargo = command[:command.index("--")] if "--" in command else command
    start = cargo.index("--test") + 2 if target is not None else cargo.index("--lib") + 1
    filters = [part for part in cargo[start:] if not part.startswith("-")]
    if len(filters) > 1:
        raise ValueError("Rust verification has ambiguous test filters")
    skipped = [command[index + 1] for index, part in enumerate(command[:-1]) if part == "--skip"]
    scopes = []
    def matches(value, name):
        return name == value if "--exact" in command else value in name
    for package in packages:
        key = (package, target)
        inventory = inventories.get(key)
        if inventory is None:
            inventory = inventories[key] = rust_test_inventory(package, target)
        selected = [row for row in inventory["tests"] if not row["ignored"]
                    and (not filters or matches(filters[0], row["name"]))
                    and not any(matches(skip, row["name"]) for skip in skipped)]
        windows = [row for row in selected if row["requires_window"]]
        if windows:
            raise ValueError("Rust verification selects prohibited window tests: " +
                             ", ".join(package + "::" + row["name"] for row in windows))
        scopes.append({"package": package, "target": inventory["target"],
                       "selected_tests": [row["name"] for row in selected],
                       "unresolved_includes": inventory["unresolved_includes"],
                       "source_count": len(inventory["sources"]), "limits": inventory["limits"]})
    if not any(scope["selected_tests"] for scope in scopes):
        raise ValueError("Rust verification selects no current tests")
    return {"targets": scopes, "window_tests_selected": 0}


def rust_validation_commands():
    """Current pure/mock suites, shared by execution and owner validation."""
    return [
        ("keyboard-navigation", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "keyboard_navigation", "--",
                                 "--skip", "keyboard_navigation::button::tests::button_navigation_preserves_focus_and_activation_requires_window",
                                 "--skip", "keyboard_navigation::focus::tests::input_destinations_keep_editing_arrows_and_track_disabled_state_requires_window",
                                 "--skip", "keyboard_navigation::tests::guide_closes_through_keyboard_and_pointer_with_window_owned_focus"]),
        ("source-select-keyboard", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "synapse_select::keyboard_tests"]),
        ("source-switch-keyboard", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_switch_arrows"]),
        ("macro-library", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "macro_library"]),
        ("macro-recorder-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping::recorder"]),
        ("macro-shortcut-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping::shortcuts"]),
        ("macro-page-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "macro_page", "--",
                              "--skip", "macro_page::tests::library_lifecycle::two_retained_pages_publish_from_the_latest_shared_library",
                              "--skip", "macro_page::tests::tutorial_buttons_unlock_local_action_editing_and_save",
                              "--skip", "macro_page::tests::tutorial_skip_is_reachable_above_standard_and_phased_lists",
                              "--skip", "macro_page::tests::recording_settings_receive_clicks_above_the_disabled_list",
                              "--skip", "macro_page::tests::profile_commands_resume_after_settings_and_save_the_suspended_new_action",
                              "--skip", "macro_page::tests::unsaved_escape_keeps_draft_and_discard_resumes_new_without_saving",
                              "--skip", "macro_page::tests::profile_commands_stay_disabled_through_recording_lifecycle",
                              "--skip", "macro_page::tests::macro_name_paste_truncates_utf16_and_escape_discards_the_name_draft",
                              "--skip", "macro_page::tests::command_input_keeps_whitespace_through_enter_and_local_save",
                              "--skip", "macro_page::tests::whitespace_only_command_change_is_saved_from_the_pending_editor",
                              "--skip", "macro_page::tests::clearing_delay_keeps_the_previous_valid_timer",
                              "--skip", "macro_page::tests::command_timer_reads_current_text_at_the_first_pending_deadline",
                              "--skip", "macro_page::tests::loop_input_normalizes_and_updates_its_pair_after_150ms"]),
        ("macro-library-page", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "macro_library"]),
        ("ui-preferences-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "ui_preferences"]),
        ("camera-identity-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_identity", "--",
                                   "--skip", "features::source_workspace::camera_identity::tests::camera_workspace_publishes_connection_read_failures_and_restarts_after_canceled_exit"]),
        ("help-camo-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_help::camo"]),
        ("help-camo-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "help_camo", "--",
                             "--skip", "shell::help_camo::tests::camera_delivery_retains_a_worker_joined_after_the_current_reply_drain"]),
        ("help-runtime-versions-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_help::runtime_versions", "--",
                                        "--skip", "features::source_help::runtime_versions::tests::help_runtime_reconnect_and_exit_resume_refresh_the_mounted_request"]),
        ("help-runtime-versions-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "shell::help_runtime_versions", "--",
                                        "--skip", "shell::help_runtime_versions::tests::help_worker_panic_is_a_failed_observation_for_the_current_owner"]),
        ("ipc-client", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "client_tests"]),
        ("ipc-failure-diagnostics", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "failure_diagnostics::tests"]),
        ("hid-broker", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::hid_broker"]),
        ("session-cleanup", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::session::tests"]),
        ("portable-cleanup", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::portable::cleanup_tests"]),
        ("owner-cleanup", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::shutdown::tests"]),
        ("foreground-cleanup", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "foreground_monitor::cleanup_tests"]),
        ("hid-broker-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "hid_broker"]),
        ("hid-exchange", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "feature_exchange::tests"]),
        ("hid-backend-factory", ["cargo", "test", "--locked", "--offline", "-p", "razer-hid", "--lib", "tests"]),
        ("device-topology", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "device_changes::tests"]),
        ("ipc-shutdown-receipts", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "worker_shutdown::tests"]),
        ("mouse-input-monitoring-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mouse_input_monitoring"]),
        ("device-task-locks", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "device_tasks"]),
        ("mouse-matcher-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "dpi_matcher"]),
        ("service-exit-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "shutdown::tests"]),
        ("mouse-matcher-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "dpi_matcher"]),
        ("mouse-input-monitoring-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mouse_input_monitoring::tests"]),
        ("mouse-matcher-model", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "mouse_matcher"]),
        ("mouse-matcher-revision", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "matcher_durable_version"]),
        ("mouse-matcher-exit-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "matcher_exit_"]),
        ("mouse-matcher-page", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "matcher_"]),
        ("mouse-matcher-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_matcher"]),
        ("profile-notification-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "profile_switch_notification::tests"]),
        ("computer-name-platform", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "computer_name_tests"]),
        ("controller-trigger-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "controller_trigger"]),
        ("controller-trigger-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_products::trigger_runtime::tests"]),
        ("controller-profile", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_products::profile::tests"]),
        ("controller-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "controller_"]),
        ("controller-initialization-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "controller_initialization"]),
        ("controller-initialization-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "controller_initialization"]),
        ("controller-socd-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "controller_socd"]),
        ("controller-socd-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "controller_socd"]),
        ("controller-socd-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "controller_socd"]),
        ("controller-calibration-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "controller_calibration::tests"]),
        ("controller-sleep-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "controller_sleep"]),
        ("controller-sleep-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "sleep_runtime::tests"]),
        ("controller-sleep-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "controller_sleep"]),
        ("controller-sleep-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "controller_sleep::tests"]),
        ("controller-feature-discovery", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "controller_feature"]),
        ("controller-lighting-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_products::lighting_"]),
        ("controller-quick-lighting-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "quick_lighting_state::tests"]),
        ("controller-chroma-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "chroma_runtime::tests"]),
        ("controller-calibration-frame", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "queued_frames_cannot_advance"]),
        ("controller-palette", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "custom_colors::tests"]),
        ("receiver-profile-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "receiver_reset"]),
        ("receiver-profile-selection", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "receiver_profile_selection::tests"]),
        ("hyperpolling-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "hyperpolling_receiver"]),
        ("hyperpolling-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "hyperpolling_receiver"]),
        ("hyperpolling-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "hyperpolling_runtime"]),
        ("hyperpolling-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "hyperpolling_profile"]),
        ("hyperpolling-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "hyperpolling_runtime"]),
        ("keyboard-brightness-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "keyboard_brightness"]),
        ("keyboard-brightness-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "brightness::lifecycle_tests"]),
        ("receiver-lighting-owner", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "receiver_lighting_"]),
        ("lighting-engine-session", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "lighting_engine_session::tests"]),
        ("lighting-screen-capture", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::lighting_engine::screen::tests"]),
        ("marci-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "marci_"]),
        ("marci-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "marci_lighting"]),
        ("controller-lighting-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "controller_lighting::"]),
        ("controller-calibration-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::controller_calibration::native_io_tests"]),
        ("windows-hid-transport", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::native::hid_transport::native_io_tests"]),
        ("chroma-connect", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "chroma_connect::tests"]),
        ("receiver-lighting-attribute-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "receiver_lighting_attribute_receipt_wire_requires_the_exact_worker_identity"]),
        ("receiver-brightness-ui", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "receiver_brightness_ui::tests"]),
        ("receiver-brightness-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_brightness::tests"]),
        ("receiver-dock-mode", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_dock_mode::tests"]),
        ("device-query-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "device_query::tests"]),
        ("receiver-lighting-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_lighting::tests"]),
        ("receiver-quick-effect", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_lighting::quick_effect::tests"]),
        ("receiver-battery-peers", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_lighting::battery_peers::tests"]),
        ("receiver-battery-monitor", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_lighting::battery_monitor::tests"]),
        ("receiver-peer-lighting", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_lighting::peer_protocol::tests"]),
        ("dynamic-lighting-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "dynamic_lighting_read_is_independent"]),
        ("controller-lighting-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "controller_lighting"]),
        ("audio-demo-media", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "media::tests"]),
        ("macro-recorder-platform", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "macro_recorder"]),
        ("screen-color-platform", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "screen_color"]),
        ("screen-color-ui", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "features::screen_color::tests::"]),
        ("audio-demo-floating", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "demo::tests"]),
        ("source-feature-transport", ["cargo", "test", "--locked", "--offline", "-p", "razer-hid", "--lib"]),
        ("connected-verification-boundary", ["cargo", "test", "--locked", "--offline", "-p", "razer-hid", "--test", "connected_devices", "connected_result_validation_"]),
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
        ("current-dpi-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_current_dpi"]),
        ("current-dpi-owner", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_dpi::tests::current_dpi_"]),
        ("current-dpi-exit", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_dpi_exit_"]),
        ("current-dpi-temporary-restore", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_dpi_temporary_restore"]),
        ("current-dpi-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_current_dpi::tests"]),
        ("current-dpi-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "direct::mouse_current_dpi::tests"]),
        ("current-dpi-ble-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_current_dpi::ble::tests"]),
        ("current-dpi-ble-initialization", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_current_dpi::ble::initialization::tests"]),
        ("current-dpi-ble-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "current_dpi_ble"]),
        ("current-dpi-ble-discovery", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "discovery::tests::current_dpi_ble"]),
        ("current-dpi-ble-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "current_dpi_ble_"]),
        ("current-polling-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_current_polling::tests"]),
        ("current-polling-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "direct::mouse_current_polling::tests"]),
        ("current-polling-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "device_tasks::tests::current_polling_ipc"]),
        ("current-polling-ingame", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "polling::ingame::tests"]),
        ("current-polling-observations", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "polling::observation_tests"]),
        ("current-polling-runtime-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_polling::runtime_observation_tests"]),
        ("current-polling-profile-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_polling_profile::profile_polling_observation_tests"]),
        ("current-polling-indicator-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "unobserved_polling_has_no_rate_color_and_confirmed_rate_uses_its_source_color"]),
        ("current-polling-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_polling_runtime"]),
        ("current-dpi-observations", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_dpi_observation_"]),
        ("current-dpi-initialization", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_dpi::initialization_tests"]),
        ("dpi-row-hover-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "dpi_row_hover_"]),
        ("help-reset-owner", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "help_reset_identity_"]),
        ("mixer-restart-receipts", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mixer_restart_receipt_"]),
        ("product-chroma-observations", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "product_chroma::tests"]),
        ("product-lighting-worker", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "product_lighting::worker::tests"]),
        ("mouse-brightness-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_brightness::tests"]),
        ("mouse-dynamic-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_dynamic"]),
        ("mouse-dynamic-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mouse_dynamic"]),
        ("mouse-dynamic-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "direct::mouse_dynamic::tests"]),
        ("mouse-dynamic-page", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "features::mouse_products::dynamic::"]),
        ("mouse-advanced-page", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "features::mouse_products::advanced::"]),
        ("runtime-diagnostics", ["cargo", "test", "--locked", "--offline", "-p", "razer-app", "--lib", "diagnostics::runtime::tests"]),
        ("product-arguments", ["cargo", "test", "--locked", "--offline", "-p", "razer-app", "--lib", "arguments::tests"]),
        ("service-runtime", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::"]),
        ("chroma-profile-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "chroma_profiles::tests"]),
        ("host-profile-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "host_local_profile::"]),
        ("user-app-files-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "user_app_files::"]),
        ("lighting-host-page", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "host_observation_tests"]),
        ("lighting-observation-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "product_lighting::observation::tests"]),
        ("mouse-dynamic-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_dynamic"]),
        ("mouse-sensor-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_sensor::tests"]),
        ("help-system-info-domain", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "system_info::tests"]),
        ("help-system-info-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "help_system_info"]),
        ("help-system-info-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "help_system_info"]),
        ("help-file-dialog-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "file_dialog::tests"]),
        ("help-system-info-page", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "help_system_info"]),
        ("help-system-info-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "help_system_info"]),
        ("current-power-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_power::tests"]),
        ("current-idle-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_polling_profile::idle_tests"]),
        ("current-power-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_current_power_runtime"]),
        ("current-power-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_current_power::tests"]),
        ("current-power-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "direct::mouse_current_power::tests"]),
        ("current-power-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "device_tasks::tests::current_power_ipc"]),
        ("current-power-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mouse_current_power::tests"]),
        ("current-polling-persistence", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_polling_write::tests"]),
        ("current-dpi190-intents", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current190_source_separates_stage_table_edits_from_transient_stage_selection"]),
        ("current-dpi182-intents", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current182_stage_actions_do_not_issue_the_separate_otfs_level_task"]),
        ("mouse-onboard-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_onboard::tests"]),
        ("mouse-dpi-slot-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_dpi_stages::tests"]),
        ("mouse190-customize-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "customize_190::tests::"]),
        ("mouse-customize-viewport", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "customize_viewport"]),
        ("mouse-cycle-selectors", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "every_cycle_branch_keeps_its_current_product_and_edition_selector"]),
        ("mouse229-cycle-alias", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "pid229_cycle_artwork_uses_a_registered_runtime_alias"]),
        ("mouse190-mapping-popup", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "customize_190::source_mapping::tests"]),
        ("mouse-mapping-popup", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mapping_menu_current::popup::tests"]),
        ("audio-mapping-popup", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "sound_mapping::popup::tests"]),
        ("storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib"]),
        ("shell-store-migration", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "workspace::tests"]),
        ("catalog-registry", ["cargo", "test", "--locked", "--offline", "-p", "razer-catalog", "--lib", "registry::tests"]),
        ("emoji-search", ["cargo", "test", "--locked", "--offline", "-p", "razer-catalog", "--lib", "emoji::tests"]),
        ("resource-database", ["cargo", "test", "--locked", "--offline", "-p", "razer-resource-db", "--lib"]),
        ("resource-database-index", ["cargo", "test", "--locked", "--offline", "-p", "razer-resources", "--lib", "catalog::database_tests"]),
        ("packaged-catalogs", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "-p", "razer-app-pages", "-p", "razer-assets", "-p", "razer-model", "-p", "razer-catalog", "-p", "razer-shell", "-p", "razer-tray", "-p", "razer-settings", "--lib", "packaged_catalog_"]),
        ("model", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib"]),
        ("existing-workspace-reducers", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "workspace::reducer_state::tests::"]),
        ("mapping-engine", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mapping_engine"]),
        ("mapping-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mapping_native"]),
        ("mapping-native-input", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mapping_native_input"]),
        ("mouse-rotation-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mouse_rotation"]),
        ("mouse-rotation-domain", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_rotation"]),
        ("mouse-rotation-discovery", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "mouse_rotation"]),
        ("mouse-rotation-worker", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping::mouse_rotation::tests"]),
        ("mouse-worker-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mouse_worker::tests"]),
        ("mouse-rotation-page", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_rotation_owner_"]),
        ("mouse-rotation-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "rotation"]),
        ("mouse-power-ble", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_current_power::ble::tests"]),
        ("mouse-artwork-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_runtime_artwork::tests"]),
        ("mapping-rotation-driver", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping_runtime::driver::tests"]),
        ("mapping-dynamic-key-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mapping_native_dynamic_key"]),
        ("mapping-dynamic-key-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "dynamic_key"]),
        ("macro-engine", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "macro_engine"]),
        ("macro-page-phases", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "macro_page::phased::tests"]),
        ("receiver-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_"]),
        ("chroma-studio-construction", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "studio_button_hover_override_constructs_without_duplicate_registration"]),
        ("mapping-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping::"]),
        ("mapping-observation", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping::observation::"]),
        ("mapping-observation-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mapping_observation::"]),
        ("mapping-input-observation-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "input_observation"]),
        ("mapping-input-observation-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "input_observation"]),
        ("mapping-analog-observation-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mapping_native_observation"]),
        ("mapping-analog-observation-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "analog_observation"]),
        ("mapping-analog-observation-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "analog_observation"]),
        ("mapping-gamepad-observation-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mapping_native_gamepad_observation"]),
        ("mouse-receiver-mapping", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_receiver_mapping::tests"]),
        ("mouse-otfs-mapping", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "interdevice_otfs"]),
        ("mapping-document-model", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "mapping::"]),
        ("mapping-reconnect-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mapping_reconnect::tests"]),
        ("mouse-receiver-callbacks", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mapping_runtime_158::tests"]),
        ("mapping-gamepad-observation-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "gamepad_observation"]),
        ("mapping-gamepad-observation-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "gamepad_observation"]),
        ("keyboard-gamepad-observation", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "gamepad_output_"]),
        ("mapping-subscription-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "observation_tasks::tests"]),
        ("mapping-cleanup-failures", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "failed_gamepad_cleanup"]),
        ("mapping-observation-completion", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "observation_completion_distinguishes"]),
        ("mapping-chroma-actions", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mapping_chroma_"]),
        ("mapping-chroma-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "mapped_chroma_"]),
        ("chroma-commit-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "chroma_commit_revisions_"]),
        ("chroma-commit-publication", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "chroma_configuration_commits_"]),
        ("keyboard-mapping-audio-inventory", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "keyboard_mapping_audio_inventory_"]),
        ("mouse-dpi-stage-operations", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "dpi_stage_"]),
        ("mouse-dpi-stage-observations", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "dpi_stage_observation_"]),
        ("keyboard-shared-chroma-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "keyboard_shared_selection_"]),
        ("legacy-dpi-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_dpi_70::"]),
        ("keyboard-snaptap-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "snap_tap::observation::tests"]),
        ("keyboard-snaptap-editing", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "disabling_capture_keeps_the_source_opposite_preview_key"]),
        ("keyboard-analog-feedback", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "analog_feedback"]),
        ("keyboard-controller-observation", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "analog_controller_observation_"]),
        ("keyboard-mapping-observation", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "analog_mapping_observation_"]),
        ("macro-hotkey-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping::hotkeys::"]),
        ("macro-recording-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "macro_library::recording::"]),
        ("macro-recorder-preferences", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "macro_library::recorder_preference_tests"]),
        ("macro-shortcut-registration", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "macro_page::record_shortcut::registration_tests"]),
        ("mapping-control-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mapping_control::tests"]),
        ("global-shortcuts-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "shortcuts::tests"]),
        ("global-shortcut-capture-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "global_shortcut_capture::tests"]),
        ("global-shortcuts-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping_runtime::backend::tests"]),
        ("mapping-control-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping::owner::tests"]),
        ("global-shortcut-capture-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping_runtime::capture::tests"]),
        ("mapping-connections", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "device_discovery::tests"]),
        ("profiles-transfer-codec", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "profile_transfer"]),
        ("audio-profile-import", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "import_1398"]),
        ("mouse-profile-import", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_import"]),
        ("local-profile-codec", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "profile_transfer"]),
        ("help-reset-profile-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "help_owner::tests"]),
        ("help-reset-defaults", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "blackshark_reset::tests"]),
        ("help-reset-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "help_factory_reset1398"]),
        ("help-reset-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "help_factory_reset1398"]),
        ("help-reset-persistence", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "help_hardware_reset"]),
        ("resource-css-filters", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "css_filters_"]),
        ("queued-device-revisions", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "workspace::device_save::tests"]),
        ("chroma-settings-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-state", "--lib", "chroma_settings::tests"]),
        ("local-image", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "local_image::tests"]),
        ("rotated-label-outline", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "rotated_label_"]),
        ("native-diagnostics", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "discovery_diagnostics::tests"]),
        ("linked-executable", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "executable::tests"]),
        ("oled-import-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "oled_presets::tests::import_"]),
        ("ui-preferences-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-state", "--lib", "ui_preferences::"]),
        ("i18n", ["cargo", "test", "--locked", "--offline", "-p", "razer-i18n", "--lib"]),
        ("discovery", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib"]),
        ("mixer-shortcut-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "audio_shortcut_lifecycle_tests"]),
        ("macro-shortcut-resume", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "recorder_source_started_waits_for_both_mapping_processes_and_restores_them"]),
        ("macro-recording-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "macro_recording::tests::"]),
        ("startup-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "-p", "razer-dashboard", "-p", "razer-device", "-p", "razer-discovery", "-p", "razer-shell", "--lib", "startup_state"]),
        ("hid-inventory", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "hid::inventory_tests"]),
        ("device", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib"]),
        ("mouse-idle-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_polling_profile::idle_tests"]),
        ("mouse-receiver-polling", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_receiver_polling::tests"]),
        ("mouse-receiver-polling-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "mouse_receiver_polling"]),
        ("hanbo-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "hanbo_cooler::tests"]),
        ("hanbo-startup-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "hanbo_startup"]),
        ("hanbo-settings", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "hanbo_settings::tests"]),
        ("hanbo-profile-restore", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "hanbo_profile_restore::tests"]),
        ("hanbo-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "hanbo_"]),
        ("hanbo-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "hanbo_cooler::tests"]),
        ("hanbo-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "hanbo_"]),
        ("hanbo-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "shell::hanbo_cooler::tests::"]),
        ("profile-collection", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "profile_collection::tests"]),
        ("system-monitor-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "system_monitor::tests"]),
        ("system-monitor-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "system_monitor_tests"]),
        ("system-monitor-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "system_monitor::tests"]),
        ("keyboard-magnetic-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_magnetic"]),
        ("keyboard-sync-feedback", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "keyboard_products::actuation::sync_feedback::tests::"]),
        ("keyboard-macro-submission", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_submission::macro_submission::tests"]),
        ("onboard-macro", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "onboard_macro"]),
        ("mouse-scroll-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_scroll::tests"]),
        ("mouse-scroll-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mouse_scroll_tests"]),
        ("mouse-scroll-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "mouse_scroll_tests"]),
        ("mouse-scroll-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "scroll_task::tests"]),
        ("mouse-scroll-readback", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "scroll_wheel::tests::mouse_scroll_"]),
        ("mouse-scroll-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mouse_worker::tests"]),
        ("mouse-scroll-persistence", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_scroll::"]),
        ("mouse-tracking-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_tracking::tests::"]),
        ("mouse-tracking-session", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "receiver_session::tests::"]),
        ("mouse-tracking-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mouse_tracking_tests::"]),
        ("mouse-tracking-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "direct::mouse_tracking::tests::"]),
        ("mouse-tracking-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::mouse_tracking_controller::tests::"]),
        ("mouse-tracking-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "request_lifecycle_tests::"]),
        ("mouse-tracking-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "shell::mouse_tracking::tests::"]),
        ("mouse-philips-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "mouse_philips::"]),
        ("mouse-philips-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "mouse_tracking_tests::mouse_philips"]),
        ("mouse-philips-routing", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "philips"]),
        ("mouse-philips-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::mouse_philips_controller::tests::"]),
        ("mouse-philips-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping_runtime::philips::"]),
        ("mouse-philips-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "shell::mouse_tracking::tests::philips"]),
        ("mouse-philips-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "calibration_70::state::"]),
        ("mouse-profile-source", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_profile_source::tests"]),
        ("keyboard-metadata", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_metadata::tests"]),
        ("keyboard-latest-restore-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_latest_restore::tests::"]),
        ("keyboard-latest-restore-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "shell::keyboard_latest_restore::tests::"]),
        ("keyboard-latest-restore-publisher", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "shell::keyboard_latest_restore::publisher::tests::"]),
        ("keyboard-game-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "game_monitor::lifecycle::tests::"]),
        ("keyboard-actuation-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "keyboard_actuation_write::worker::tests::"]),
        ("session-monitor-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "session_changes::tests::"]),
        ("device-change-monitor-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-platform", "--lib", "device_changes"]),
        ("hid-interrupt", ["cargo", "test", "--locked", "--offline", "-p", "razer-hid", "--lib", "native::tests"]),
        ("receiver-events-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "receiver_events::tests"]),
        ("receiver-reconnect-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "receiver_reconnect::tests"]),
        ("mouse-runtime-artwork-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mouse_runtime_artwork_controller"]),
        ("mouse-runtime-artwork-shell", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "mouse_runtime_artwork"]),
        ("mouse-runtime-artwork-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "runtime_artwork_tests"]),
        ("mouse-ble-protocol30-device", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "protocol30_"]),
        ("mouse-ble-protocol30-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "protocol30_"]),
        ("mouse-current-power-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "current_power::tests"]),
        ("mouse-current-power-migration", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_current_power_migration"]),
        ("mouse-connection-navigation", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mouse_connection_navigation"]),
        ("seiren-eq-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "seiren_eq"]),
        ("seiren-audio-operation-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "seiren_audio_"]),
        ("seiren-audio-operation-owner", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "seiren_audio_"]),
        ("audio-notification-cleanup", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "audio_cleanup_"]),
        ("audio-shortcut-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "audio_shortcut_lifecycle_tests"]),
        ("seiren-audio-capture-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "seiren_audio_capture"]),
        ("seiren-audio-owner-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "seiren_audio::"]),
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
        ("shell-product-favicons", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "every_registered_product_tab_has_a_manifested_source_favicon"]),
        ("audio-mixer-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "audio_mixer"]),
        ("audio-local-settings", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "audio_mixer_local_settings::tests"]),
        ("audio-preset-names", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "audio_mixer_presets::preset_name_tests"]),
        ("chroma-layer-names", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "studio_layers::rename_tests"]),
        ("aether-device-names", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "aether_strip::device::name_tests"]),
        ("seiren-local-settings", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "seiren_local_settings::tests"]),
        ("audio-sound-mapping", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "sound_mapping::tests"]),
        ("keyboard-dynamic-metadata", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "all_current_dynamic_mapping_metadata_loads"]),
        ("keyboard-source-artwork", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_artwork::tests"]),
        ("keyboard-geometry", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "keyboard_geometry::tests::", "--", "--skip", "rejected_corners_and_holes_bubble_to_real_overlapping_buttons", "--skip", "real_keyboard_uses_source_geometry_at_scaled_default_and_all_known_layouts"]),
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
        ("receiver-query-lease", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "query_lease_"]),
        ("service-shutdown-receipts", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::shutdown::tests"]),
        ("mapping-execution-completion", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping_runtime::lifecycle_tests"]),
        ("mapping-driver-session", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping_runtime::driver::tests"]),
        ("mapping-hardware-session", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "mapping_runtime::hardware::tests"]),
        ("keyboard-local-profiles-storage", ["cargo", "test", "--locked", "--offline", "-p", "razer-storage", "--lib", "host_keyboard_profiles::tests"]),
        ("keyboard-current-profile-schema", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_current_profile"]),
        ("keyboard-factory-profile-identity", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "factory_profile_identity_does_not_depend_on_a_calibration_page"]),
        ("keyboard-factory-profile-selection", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "active_keyboard_factory_profile_follows_selected_identity_after_library_replacement"]),
        ("accessory-port-names", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "features::argb_name::tests"]),
        ("macro-action-inputs", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "action_input_coercion_matches_current_st_yt_and_ot"]),
        ("macro-randomized-input", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "randomized_action_draft_truncates_before_range_checks"]),
        ("keyboard-profile-inventory-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_profile_inventory"]),
        ("keyboard-profile-inventory-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "keyboard_profile_inventory"]),
        ("keyboard-profile-inventory-discovery", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "keyboard_profile_inventory"]),
        ("keyboard-onboard-profiles-protocol", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_onboard_profiles"]),
        ("keyboard-onboard-profiles-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "keyboard_onboard_profiles"]),
        ("keyboard-onboard-profiles-discovery", ["cargo", "test", "--locked", "--offline", "-p", "razer-discovery", "--lib", "keyboard_onboard_profiles"]),
        ("hue-profile-edits", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "features::hue::profile_edit::tests"]),
        ("standalone-pairing-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-app-pages", "--lib", "pairing_page::tests", "--",
                                     "--skip", "initial_pairing_loading_has_no_unmounted_toolbar_or_status_messages",
                                     "--skip", "confirmation_escape_closes_without_invented_focus_restore_and_enter_cannot_fake_binding",
                                     "--skip", "inline_confirmation_uses_content_height_and_a_centered_compact_button"]),
        ("thx-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "thx"]),
        ("thx-shell-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-shell", "--lib", "thx_sound::tests::"]),
        ("thx-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "thx"]),
        ("camera-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "camera"]),
        ("user-app-files-native", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "runtime::native::user_app_files::write::tests"]),
        ("camera-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "camera"]),
        ("camera-image-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_image_state::tests"]),
        ("camera-exposure-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "camera_exposure_state::tests"]),
        ("source-slider", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_slider::tests"]),
        ("source-select-pointer-placement", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "pointer_placement_tests"]),
        ("source-hypershift-tooltip", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "hypershift_geometry_tests"]),
        ("source-text-layout", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_check_text_preserves_max_content_width"]),
        ("source-text-tracking", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_text::tests"]),
        ("source-input-length", ["cargo", "test", "--locked", "--offline", "-p", "razer-widgets", "--lib", "source_input::tests"]),
        ("source-string-semantics", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "source_string::tests"]),
        ("profile-document-compiler", ["cargo", "test", "--locked", "--offline", "-p", "razer-model", "--lib", "profile_document_compiler_retains_existing_catalogs_and_extensions"]),
        ("profile-import-candidate", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "imported_profile_candidate_"]),
        ("source-locale-choices", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "source_locale_choices"]),
        ("retained-locale-choices", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "retained_source_choices_translate"]),
        ("mouse-inter-device-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mapping_inter_70::locale_tests"]),
        ("keyboard-inter-device-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "mapping_drawer::inter_device::locale_tests"]),
        ("gamepad-number-layout", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "trigger_range_tip_preserves_intrinsic_single_line_width_inside_zero_width_anchor"]),
        ("audio-number-layout", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "audio_mixer_number::tests"]),
        ("mouse-svg-button-motion", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "svg_button_motion::tests"]),
        ("mouse-calibration-state", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "panel_transition_tests"]),
        ("mouse-canvas-projection", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "pid70_canvas_projection"]),
        ("assets", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "--", "--skip", "image_cache::tests"]),
        ("asset-pack-core", ["cargo", "test", "--locked", "--offline", "-p", "razer-asset-pack", "--lib"]),
        ("assets-resource-identity", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "asset_decode_tests"]),
        ("assets-image-lifecycle", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "image_cache::lifecycle_tests"]),
        ("assets-font-registration", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "resource_runtime::tests::"]),
        ("keyboard-product-image-runtime", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "product_image_runtime_tests"]),
        ("keyboard-keypad-observation", ["cargo", "test", "--locked", "--offline", "-p", "razer-pages", "--lib", "keypad_observation::tests::"]),
        ("keypad-observation-domain", ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib", "keyboard_keypad_observation::tests::"]),
        ("keypad-observation-ipc", ["cargo", "test", "--locked", "--offline", "-p", "razer-ipc", "--lib", "keypad_observation::tests::"]),
        ("keypad-observation-service", ["cargo", "test", "--locked", "--offline", "-p", "razer-service", "--lib", "keypad_"]),
        ("assets-resource-runtime", ["cargo", "test", "--locked", "--offline", "-p", "razer-resources", "--lib"]),
        ("assets-resource-diagnostics", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "resource_availability_errors"]),
        ("assets-fixed-icons", ["cargo", "test", "--locked", "--offline", "-p", "razer-assets", "--lib", "icons::tests"]),
        ("workspace", ["cargo", "check", "--locked", "--offline", "--workspace", "--all-targets"]),
    ]


def validate_rust(selected_suites=None, cargo_config=None):
    """Compile all targets and run only pure Rust / simulated library suites."""
    baseline = rust_inputs()
    destination = ROOT / ".analysis/verification"
    destination.mkdir(parents=True, exist_ok=True)
    commands = rust_validation_commands()
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
    inventories = {}
    test_scopes = {name: rust_test_scope(command, inventories) for name, command in commands}
    results = []
    unexecuted_suites = []
    for command_index, (name, command) in enumerate(commands):
        print(json.dumps({"checking": name, "argv": command}), flush=True)
        started = datetime.now(timezone.utc).isoformat()
        with tempfile.TemporaryFile(dir=destination, mode="w+b") as output:
            try:
                with verification_environment(name) as (environment, temporary_directory):
                    completed = subprocess.run(command, cwd=ROOT, env=environment, stdout=output,
                                               stderr=subprocess.STDOUT, check=False)
            finally:
                output.flush()
                output.seek(0)
                log_bytes = output.read()
                log = _publish_immutable_evidence(log_bytes, directory=".analysis/verification", suffix=".txt")
        raw = log_bytes.decode("utf-8", errors="replace")
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
               "exit_code": completed.returncode, "log": log["path"], "log_sha256": log["sha256"],
               "log_bytes": log["bytes"],
               "executed_tests": executed_tests, "errors": errors, "passed": not errors,
               "warning_count": len(warning_lines), "warning_lines": warning_lines,
               "excluded_test_filters": [command[index + 1] for index, part in enumerate(command[:-1]) if part == "--skip"],
               "test_summaries": [{"result": state, "passed": int(passed), "failed": int(failed),
                                   "ignored": int(ignored)} for state, passed, failed, ignored in totals]}
        if test_scopes[name] is not None:
            row["static_test_scope"] = test_scopes[name]
        if name == "assets":
            row["test_scope"] = "Pure codec, catalog and fixed-icon checks; image_cache::tests requires a real GPUI headless window and is excluded"
        results.append(row)
        print(json.dumps(row), flush=True)
        if name == "workspace" and not row["passed"]:
            unexecuted_suites = [
                {"name": blocked_name, "argv": blocked_command,
                 "reason": "blocked_by_workspace_check_failed"}
                for blocked_name, blocked_command in commands[command_index + 1:]
            ]
            break
    unchanged = baseline == rust_inputs()
    receipt = {"schema_version": 1, "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
               "inputs": baseline, "inputs_unchanged": unchanged, "commands": results,
               "suite_selection": [name for name, _ in commands],
               "unexecuted_suites": unexecuted_suites,
               "all_configured_suites": selected_suites is None,
               "warning_count": sum(row["warning_count"] for row in results),
               "target_execution": False, "real_device_operations": False,
               "application_runtime_acceptance": "not performed",
               "passed": unchanged and not unexecuted_suites and all(row["passed"] for row in results)}
    previous_path = ROOT / ".analysis/validation.json"
    import analysis
    with analysis.index_lock(previous_path.with_suffix(".lock")):
        if previous_path.exists():
            previous_bytes = previous_path.read_bytes()
            previous = json.loads(previous_bytes)
            if not isinstance(previous, dict):
                raise ValueError("Previous Rust validation receipt is not an object")
            receipt["previous_receipt"] = _publish_immutable_evidence(
                previous_bytes, directory=".analysis/verification", suffix=".json")
            if "local_runtime_checks" in previous:
                receipt["local_runtime_checks"] = previous["local_runtime_checks"]
        write_json(previous_path, receipt)
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
    for name in ("inventory", "verify", "audit-inputs", "recover-inputs", "prepare-analysis", "prepare-static-python", "prepare-product-names", "prepare-keyboard-identity"):
        commands.add_parser(name)
    revalidate_frontends = commands.add_parser(
        "revalidate-current-deployment",
        help="Read-only two-round HTTP revalidation of retained current frontend deployment owners",
    )
    revalidate_sources = revalidate_frontends.add_mutually_exclusive_group(required=True)
    revalidate_sources.add_argument("--acquisition", action="append",
                                     help="Project-relative retained frontend acquisition.json; repeat to select several")
    revalidate_sources.add_argument("--all-current", action="store_true",
                                     help="Select the newest byte-verified acquisition for every retained official route")
    revalidate_frontends.add_argument("--owner", action="append", default=[],
                                      help="Additional project-relative JavaScript original with a verified deployment reference")
    revalidate_frontends.add_argument("--workers", type=int, default=6,
                                      help="Bounded concurrent HTTP requests per round (default: 6)")
    refresh_frontends = commands.add_parser("refresh-current-deployment",
        help="Batch-acquire changed official deployment routes from an immutable revalidation receipt")
    refresh_frontends.add_argument("--revalidation", required=True,
        help="Project-relative content-addressed revalidation receipt")
    refresh_frontends.add_argument("--source", action="append", default=[],
        help="Select named source sets from the observed changes; default includes every changed or unselected route")
    refresh_frontends.add_argument("--priority-source", action="append", default=[],
        help="Start these source sets first while preserving complete batch scope")
    refresh_frontends.add_argument("--reuse-acquisition", action="append", default=[],
        help="Additional byte-verified immutable acquisition to reuse without copying originals")
    refresh_frontends.add_argument("--workers", type=int, default=4)
    refresh_frontends.add_argument("--network-workers", type=int, default=12)
    refresh_frontends.add_argument("--parse-workers", type=int, default=4)
    resource_verify = commands.add_parser("verify-resources", help="Decode embedded images or the complete typed keyboard artwork atlas without network access")
    resource_verify.add_argument("--scope", choices=("embedded", "keyboard-artwork"), default="embedded")
    resource_verify.add_argument("--product", type=int, action="append", help="Limit keyboard artwork to these typed products; repeat to select several")
    scratch_clean = commands.add_parser("clean", help="Plan referenced and active-session-safe scratch cleanup; default never deletes")
    scratch_clean.add_argument("--stale", action="store_true", help="Remove only old unreferenced root logs/text/output and bytecode; retain all analyzer databases and active files")
    scratch_clean.add_argument("--dry-run", action="store_true", help="Write an exact reviewable cleanup plan without deleting files (default)")
    scratch_clean.add_argument("--plan", help="Project-relative .analysis/results JSON destination for the dry-run plan")
    scratch_clean.add_argument("--apply", help="Apply an unchanged retained cleanup plan after repeating dependency and activity checks")
    scratch_clean.add_argument("--plan-sha256", help="Actual SHA-256 of the reviewed cleanup plan; required with --apply")
    scratch_clean.add_argument("--verification-recovery", help="Register explicit legacy empty-directory recovery JSON, or - for stdin; planning only")
    runtime_audit = commands.add_parser("audit-runtime-resources", help="Trace Rust/catalog resource references against the fixed installed directory")
    runtime_audit.add_argument("--repair", action="store_true", help="Verify current inline ARGB layers and attach missing aliases; do not restore source media")
    pack_audit = commands.add_parser("audit-asset-pack")
    pack_audit.add_argument("--prepare", action="store_true", help="Generate the exact ordered production asset-list.json")
    validation = commands.add_parser("validate-rust")
    validation.add_argument("--suite", action="append", help="Run an applicable pure/mock suite; repeat to select multiple suites")
    validation.add_argument("--cargo-config", action="append", help="Per-command Cargo configuration override, retained in verification argv")
    connected = commands.add_parser("validate-connected-devices", help="Explicit checks on actual connected products through system HID DLLs")
    connected.add_argument("--product", type=int, action="append", required=True, help="Actual logical product ID; no preview or default device")
    connected.add_argument("--write-restore", action="store_true", help="Record original settings, verify reversible changes, restore and confirm exact original values")
    native_compare = commands.add_parser("compare-connected-hid", help="Compare observed GET packet bytes through the verified current source HID exports")
    native_compare.add_argument("--library", required=True, help="Installed HID.node; its complete current input bytes and exports are verified before loading")
    commands.add_parser("compare-local-service-version", help="Compare the verified Simple Service DLL with the direct Rust local COM reader")
    commands.add_parser("compare-local-app-files", help="Compare the verified Simple Service DLL with the direct Rust local Apps inventory")
    commands.add_parser("compare-local-lighting", help="Compare isolated software Static and stationary Wheel frames through the verified current lighting DLL JSON exports")
    pack_prepare = commands.add_parser("prepare-asset-pack")
    pack_prepare.add_argument("--manifest", default=".analysis/results/asset-pack-manifest.json")
    pack_prepare.add_argument("--output", default=GENERATED_ASSET_PACK)
    pack_verify = commands.add_parser("verify-asset-pack")
    pack_verify.add_argument("--manifest", default=".analysis/results/asset-pack-manifest.json")
    pack_verify.add_argument("--pack", default=GENERATED_ASSET_PACK)
    image_analyzer = commands.add_parser("prepare-image-analyzer", help="Verify project-local image decoder wheels for the active Python ABI")
    image_analyzer.add_argument("--svg", action="store_true", help="Also prepare the pinned resvg CPU renderer in the existing image analyzer directory")
    commands.add_parser("prepare-media-analyzer", help="Verify a project-local PyAV wheel for the active Python ABI")
    native_cursors = commands.add_parser("prepare-native-cursors", help="Extract verified PE cursor groups without loading DLLs")
    native_cursors.add_argument("--sources", required=True, help="JSON list of path/acquisition records; '-' reads stdin")
    native_cursors.add_argument("--group", type=int, help="Extract only this numeric RT_GROUP_CURSOR identifier")
    resource = commands.add_parser("prepare-resources")
    resource.add_argument("--acquisition", required=True)
    resource.add_argument("--resources", required=True, help="JSON map of original paths to product display aliases; '-' reads stdin")
    resource.add_argument("--publication-manifest", help="Stage media only under analysis inputs and produce a publish-asset-release --add-files manifest")
    locale_keys = commands.add_parser("prepare-locale-keys")
    locale_keys.add_argument("--acquisition", required=True)
    locale_keys.add_argument("--namespace", required=True)
    locale_keys.add_argument("--bindings", required=True, help="JSON locale-to-original-path map; '-' reads stdin")
    locale_keys.add_argument("--key", action="append", required=True)
    locale_exports = commands.add_parser("prepare-locale-exports", help="Statically merge complete current export dictionaries and prepare the installed SQLite catalog")
    locale_exports.add_argument("--acquisition")
    locale_exports.add_argument("--source")
    locale_exports.add_argument("--updates", help="JSON list of verified source/language-map updates for one atomic preparation; '-' reads stdin")
    locale_exports.add_argument("--migrate-legacy", action="store_true", help="Verify all legacy locale semantics, retain original bytes as evidence and remove old production files")
    locale_exports.add_argument("--check", action="store_true")
    commands.add_parser("prepare-host-favicons", help="Verify all current product HTML first favicons and attach existing bytes to resource provenance")
    app_picker = commands.add_parser("prepare-app-picker", help="Verify current menu category vectors against pinned media and maintain native presentation fallbacks")
    app_picker.add_argument("--check", action="store_true", help="Require the existing typed directory to match; never materialize media")
    module_artwork = commands.add_parser("prepare-module-service-artwork", help="Verify the current More Information image owner and maintain its fixed artwork metadata")
    module_artwork.add_argument("--check", action="store_true")
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
        recovery = (json.loads(sys.stdin.read() if args.verification_recovery == "-" else args.verification_recovery)
                    if args.verification_recovery is not None else None)
        clean(stale=args.stale, dry_run=args.dry_run, plan=args.plan, apply=args.apply,
              plan_sha256=args.plan_sha256, verification_recovery=recovery)
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
        return test_evidence.run(args.test)
    elif args.command == "audit-runtime-resources":
        if args.repair:
            print(json.dumps({"current_source_alias_repair": repair_current_resource_aliases()}), flush=True)
        report = audit_runtime_resources()
        print(json.dumps({"resource_audit": relative(RESOURCE_AUDIT), **report["summary"],
                          "literal_references_passed": report["literal_references_passed"],
                          "dynamic_domain_acceptance": report["dynamic_domain_acceptance"],
                          "inputs_unchanged": report["inputs_unchanged"], "passed": report["passed"]}), flush=True)
        return 0 if report["passed"] else 1
    elif args.command == "verify-resources":
        report = verify_resources(scope=args.scope, product_ids=args.product)
        print(json.dumps({"resource_audit": relative(RESOURCE_AUDIT), "passed": report["passed"],
                          "inputs_unchanged": report["inputs_unchanged"], **report["summary"]}), flush=True)
        return 0 if report["passed"] else 1
    elif args.command == "audit-asset-pack":
        report = asset_pack_manifest(prepare=args.prepare)
        print(json.dumps({"report": ".analysis/results/asset-pack-manifest.json", "static_pack_ready": report["static_pack_ready"], **report["summary"]}), flush=True)
        return 0 if report["static_pack_ready"] else 1
    elif args.command == "validate-rust":
        validate_rust(args.suite, args.cargo_config)
    elif args.command == "validate-connected-devices":
        return validate_connected_devices(args.product, write_restore=args.write_restore)
    elif args.command == "compare-connected-hid":
        return compare_connected_hid(args.library)
    elif args.command == "compare-local-service-version":
        return compare_local_service_version()
    elif args.command == "compare-local-app-files":
        return compare_local_app_files()
    elif args.command == "compare-local-lighting":
        return compare_local_lighting()
    elif args.command == "prepare-asset-pack":
        print(json.dumps(prepare_asset_pack(args.manifest, args.output), ensure_ascii=False))
    elif args.command == "verify-asset-pack":
        print(json.dumps(verify_asset_pack(args.pack, args.manifest), ensure_ascii=False))
    elif args.command in {"prepare-asset-release", "verify-asset-release", "fetch-asset-release", "publish-asset-release", "pin-asset-release"}:
        print(json.dumps(asset_release.run(args), ensure_ascii=False), flush=True)
    elif args.command in {"prepare-device-write-table", "prepare-storage-tables", "prepare-local-protocol-tables", "prepare-mapping-capabilities", "prepare-mapping-producers", "prepare-profile-transfer-table", "prepare-receiver-battery-peers", "prepare-receiver-senders"}:
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
    elif args.command in {"prepare-hardware-driver", "prepare-windows-installer", "verify-windows-installer", "compile-windows-installer"}:
        print(json.dumps(windows_installer.run(args), ensure_ascii=False), flush=True)
    elif args.command == "prepare-image-analyzer":
        print(json.dumps(prepare_image_analyzer(svg=args.svg), ensure_ascii=False), flush=True)
    elif args.command == "prepare-media-analyzer":
        print(json.dumps(prepare_media_analyzer(), ensure_ascii=False), flush=True)
    elif args.command == "prepare-native-cursors":
        sources = json.loads(sys.stdin.read() if args.sources == "-" else args.sources)
        print(json.dumps(prepare_native_cursors(sources, args.group), ensure_ascii=False), flush=True)
    elif args.command == "prepare-resources":
        mappings = json.loads(sys.stdin.read() if args.resources == "-" else args.resources)
        print(json.dumps(prepare_resources(args.acquisition, mappings,
                                         publication_manifest=args.publication_manifest), ensure_ascii=False))
    elif args.command == "prepare-locale-keys":
        bindings = json.loads(sys.stdin.read() if args.bindings == "-" else args.bindings)
        print(json.dumps(prepare_locale_keys(args.acquisition, args.namespace, bindings, args.key), ensure_ascii=False))
    elif args.command == "prepare-locale-exports":
        if args.updates is not None:
            if args.acquisition or args.source or args.migrate_legacy:
                raise ValueError("Locale update batch cannot be combined with a legacy/export source")
            updates = json.loads(sys.stdin.read() if args.updates == "-" else args.updates)
            result = prepare_locale_updates(updates, check=args.check)
        else:
            if not args.acquisition or not args.source:
                raise ValueError("Locale exports require an acquisition and source, or an updates batch")
            result = prepare_locale_exports(args.acquisition, args.source,
                                           migrate_legacy=args.migrate_legacy, check=args.check)
        print(json.dumps(result, ensure_ascii=False), flush=True)
    elif args.command == "prepare-host-favicons":
        print(json.dumps(prepare_host_favicons(), ensure_ascii=False))
    elif args.command == "prepare-app-picker":
        print(json.dumps(prepare_app_picker(check=args.check), ensure_ascii=False))
    elif args.command == "prepare-module-service-artwork":
        report = prepare_module_service_artwork(check=args.check)
        print(json.dumps({"inputs_unchanged": report["inputs_unchanged"], "metadata": report["metadata"],
                          "unresolved": report["unresolved"], "check_only": args.check}, ensure_ascii=False))
    elif args.command == "deduplicate-resources":
        report = deduplicate_resources(apply=args.apply, plan=args.plan)
        print(json.dumps({"report": args.plan, "mode": report["mode"], **report["summary"]}, ensure_ascii=False))
    elif args.command == "organize-sources":
        report = organize_sources(apply=args.apply)
        return 0 if not report["unresolved"] and report.get("resource_scope", {}).get("passed", True) and report.get("architecture", {}).get("passed", True) else 1
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
    elif args.command == "revalidate-current-deployment":
        report = revalidate_current_frontends(args.acquisition or (), owner_names=args.owner,
                                              workers=args.workers, all_current=args.all_current)
        print(json.dumps({"receipt": report["receipt"], "passed": report["passed"],
                          "source_sets": (len(report["source_sets"]) if args.all_current else
                                          [source["name"] for source in report["source_sets"]]),
                          "selection_failures": len(report.get("current_source_selection", {}).get("failures", [])),
                          "failures": len(report["failures"])}, ensure_ascii=False), flush=True)
        return 0 if report["passed"] else 1
    elif args.command == "refresh-current-deployment":
        report = refresh_current_frontends(args.revalidation, workers=args.workers,
            network_workers=args.network_workers, parse_workers=args.parse_workers,
            priority_sources=args.priority_source, source_names=args.source,
            reuse_acquisitions=args.reuse_acquisition)
        print(json.dumps({"report": report["report"], "passed": report["passed"],
                          "sources": len(report["jobs"]),
                          "states": dict(Counter(job["status"] for job in report["jobs"]))}), flush=True)
        return 0 if report["passed"] else 1
    elif args.command == "prepare-dashboard-images":
        if type(args.workers) is not int or not 2 <= args.workers <= 12:
            raise ValueError("Dashboard resource workers must be between two and twelve")
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
