"""Pure byte tests for analyzer wheel identity and extraction boundaries."""
import base64
import csv
import hashlib
import io
import unittest
import warnings
import zipfile
import copy
from contextlib import closing, nullcontext
import json
from pathlib import Path
import sqlite3
import tempfile
from unittest import mock

import workspace
import resource_database
import families
import typed_tables
import catalog_data


class LocalizationCatalogTests(unittest.TestCase):
    def document(self):
        return {"schema_version": 1, "locales": [
            {"locale": "en", "messages": {"EMPTY": "", "RESET": "RESET", "SOURCE.BODY": "first\nsecond"}},
            {"locale": "zh-CN", "messages": {"RESET": "重置", "SHOW_GUIDE": "显示指南"}}]}

    def test_legacy_flattening_preserves_complete_namespaces_and_rejects_loss(self):
        original = {"Z": "same", "SOURCE": {"BODY": "first\nsecond", "NESTED": {"TITLE": "设备"}}, "EMPTY": ""}
        self.assertEqual(resource_database.flatten_locale(original),
                         {"EMPTY": "", "SOURCE.BODY": "first\nsecond", "SOURCE.NESTED.TITLE": "设备", "Z": "same"})
        for value in ({"A": {"B": "one"}, "A.B": "two"}, {"A": 1}, {"A": None},
                      {"A": []}, {"A": {}}, {"": "empty"}, []):
            with self.subTest(value=value), self.assertRaises(ValueError):
                resource_database.flatten_locale(value)
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            resource_database.unique_json('{"A":"one","A":"two"}')

    def test_catalog_requires_full_typed_canonical_language_and_key_schema(self):
        valid = self.document()
        self.assertEqual(resource_database.validate_locales(valid), valid["locales"])
        invalid = []
        for path, value in ((["schema_version"], True), (["locales"], []),
                            (["locales", 0, "locale"], "../en"), (["locales", 0, "messages"], {}),
                            (["locales", 0, "messages", "RESET"], None),
                            (["locales", 0, "messages", "RESET"], {"nested": "wrong"})):
            item = copy.deepcopy(valid)
            owner = item
            for key in path[:-1]:
                owner = owner[key]
            owner[path[-1]] = value
            invalid.append(item)
        invalid += [{**valid, "extra": 1}, {**valid, "locales": list(reversed(valid["locales"]))},
                    {**valid, "locales": valid["locales"][1:]},
                    {**valid, "locales": [valid["locales"][0], valid["locales"][0]]}]
        reordered = copy.deepcopy(valid)
        reordered["locales"][0]["messages"] = dict(reversed(list(reordered["locales"][0]["messages"].items())))
        invalid.append(reordered)
        for item in invalid:
            with self.subTest(document=item), self.assertRaises(ValueError):
                resource_database.validate_locales(item)

    def test_sqlite_verifies_every_language_key_text_namespace_and_position(self):
        locales = resource_database.validate_locales(self.document())
        with closing(sqlite3.connect(":memory:")) as connection:
            connection.executescript((resource_database.ROOT / resource_database.SCHEMA).read_text(encoding="utf-8"))
            self.assertEqual(resource_database.fill_locales(connection, locales), {"en": 3, "zh-CN": 2})
            resource_database.verify_locales(connection, locales)
            with self.assertRaises(sqlite3.IntegrityError):
                connection.execute("INSERT INTO ui_translation VALUES ('en',3,'RESET','duplicate')")
            for statement in ("DELETE FROM ui_translation WHERE locale='en' AND key='EMPTY'",
                              "UPDATE ui_translation SET value='wrong' WHERE key='SOURCE.BODY'",
                              "UPDATE ui_translation SET position=3 WHERE locale='en' AND position=0",
                              "UPDATE ui_locale SET position=3 WHERE id='en'"):
                connection.execute("SAVEPOINT changed")
                connection.execute(statement)
                with self.subTest(statement=statement), self.assertRaisesRegex(ValueError, "locale fields"):
                    resource_database.verify_locales(connection, locales)
                connection.execute("ROLLBACK TO changed")
                connection.execute("RELEASE changed")

    def test_current_reference_and_revalidated_cached_source_are_required(self):
        import evidence
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            folder = root / ".analysis/inputs/frontend/current"
            folder.mkdir(parents=True)
            raw = b"const dictionary={TITLE:'current'}"
            source = folder / "main.current.js"
            source.write_bytes(raw)
            name = source.relative_to(root).as_posix()
            identity = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
            url = "https://apps.razer.com/synapse/products/190/ui/static/js/main.current.js"
            attempts = []
            for index in range(2):
                path = folder / f"revalidation-{index}.js"
                path.write_bytes(raw)
                attempts.append({**identity, "path": path.relative_to(root).as_posix(), "url": url,
                                 "final_url": url, "http_status": 200, "status": 200, "matches_retained": True})
            original = {**identity, "url": url, "kind": "installed_cached_official_resource",
                        "current_http_revalidation": {"attempts": attempts}}
            receipt = {"entry_url": "https://apps.razer.com/synapse/products/190/ui/index.html",
                       "source_current_during_acquisition": True, "stability": [{"unchanged": True}],
                       "files": [original]}
            acquisition = folder / "acquisition.json"
            acquisition.write_text(json.dumps(receipt), encoding="utf-8")
            mounted = {"identity_verified": True, "original": identity, "deployment_role": "html_include"}
            deployment = {"entry_original": identity, "manifests": [], "sources": [mounted], "failures": []}
            with mock.patch.object(workspace, "ROOT", root), \
                    mock.patch.object(evidence, "frontend_deployment_sources", return_value=deployment):
                loaded, provenance = workspace.locale_preparation_source(acquisition.relative_to(root).as_posix(), name)
                self.assertEqual(loaded, raw)
                self.assertEqual(provenance["deployment_reference"], mounted)
                self.assertEqual(len(provenance["inputs"]), 5)
                deployment["sources"] = []
                with self.assertRaisesRegex(ValueError, "current deployment reference"):
                    workspace.locale_preparation_source(acquisition.relative_to(root).as_posix(), name)
                deployment["sources"] = [mounted]
                (folder / "revalidation-1.js").write_bytes(b"different bytes")
                with self.assertRaisesRegex(ValueError, "revalidation bytes differ"):
                    workspace.locale_preparation_source(acquisition.relative_to(root).as_posix(), name)
                original["current_http_revalidation"]["attempts"] = attempts[:1]
                acquisition.write_text(json.dumps(receipt), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "official HTTP revalidation"):
                    workspace.locale_preparation_source(acquisition.relative_to(root).as_posix(), name)

    def test_export_migration_verifies_sqlite_before_retiring_complete_legacy_files(self):
        import analysis
        import sys
        sys.path.insert(0, str(workspace.ROOT / "tools/analyzers/python-static"))
        source = ("(()=>{var en={},zh={};const english='Show Guide',chinese='显示指南',"
                  "old='current source',cn='当前原件';w.d(en,{SHOW_GUIDE:()=>english,OLD:()=>old});"
                  "w.d(zh,{SHOW_GUIDE:()=>chinese,OLD:()=>cn});const words={en:en,'zh-CN':zh};})()").encode()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            legacy = root / "locales"
            legacy.mkdir()
            originals = {"en": {"OLD": "retained", "SOURCE": {"BODY": "first\nsecond"}},
                         "zh-CN": {"OLD": "保留", "EMPTY": ""}}
            raw = {}
            for locale, values in originals.items():
                raw[locale] = (json.dumps(values, ensure_ascii=False, indent=2) + "\r\n").encode()
                (legacy / (locale + ".json")).write_bytes(raw[locale])
            for name in ("tools/workspace.py", "tools/evidence.py", "tools/resource_database.py"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"static maintenance tool fixture")
            database = {"output": "assets/catalog.sqlite3", "bytes": 1, "sha256": "a" * 64, "schema_version": 4}

            def prepare_database(**kwargs):
                self.assertTrue((legacy / "en.json").is_file())
                installed = resource_database.localization_document()
                self.assertEqual(installed[0]["messages"],
                                 {"OLD": "retained", "SHOW_GUIDE": "Show Guide", "SOURCE.BODY": "first\nsecond"})
                self.assertEqual(installed[1]["messages"], {"EMPTY": "", "OLD": "保留", "SHOW_GUIDE": "显示指南"})
                return database

            patches = (mock.patch.object(workspace, "ROOT", root), mock.patch.object(analysis, "ROOT", root),
                       mock.patch.object(resource_database, "ROOT", root),
                       mock.patch.object(workspace, "locale_preparation_source", return_value=(source, {"inputs": []})),
                       mock.patch.object(resource_database, "prepare", side_effect=prepare_database),
                       mock.patch.object(analysis, "index_files"))
            with patches[0], patches[1], patches[2], patches[3], patches[4], patches[5]:
                result = workspace.prepare_locale_exports("acquisition.json", "source.js", migrate_legacy=True)
                self.assertEqual(result["added"], {"en": 1, "zh-CN": 1})
                self.assertEqual(result["retained_differences"], {"en": 1, "zh-CN": 1})
                self.assertFalse(legacy.exists())
                for locale, content in raw.items():
                    self.assertEqual((root / resource_database.LOCALE_BASELINE / (locale + ".json")).read_bytes(), content)
                snapshot, _, baseline = workspace.locale_preparation_catalog()
                self.assertFalse(baseline)
                self.assertEqual(snapshot["locales"], resource_database.localization_document())
                receipt = json.loads((root / resource_database.LOCALE_REPORT).read_bytes())
                self.assertFalse(receipt["legacy_vendor_source_provenance_established"])
                self.assertFalse(receipt["vendor_javascript_executed"])
                with mock.patch.object(resource_database, "prepare", return_value=database) as verify_database:
                    for newline in ("\n", "\r\n"):
                        path = root / resource_database.LOCALES
                        path.write_bytes((json.dumps(snapshot, ensure_ascii=False, indent=2) + "\n")
                                         .replace("\n", newline).encode())
                        checked = workspace.prepare_locale_exports("acquisition.json", "source.js", check=True)
                        self.assertTrue(checked["check_only"])
                    self.assertEqual(verify_database.call_args_list, [mock.call(check=True), mock.call(check=True)])

    def test_failed_database_preparation_retains_every_legacy_input(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            legacy = root / "locales/en.json"
            legacy.parent.mkdir(parents=True)
            legacy.write_bytes(b'{"RESET":"RESET"}')
            with mock.patch.object(workspace, "ROOT", root), mock.patch.object(analysis, "ROOT", root), \
                    mock.patch.object(resource_database, "ROOT", root), \
                    mock.patch.object(resource_database, "prepare", side_effect=ValueError("database preparation failed")):
                document, snapshots, baseline = workspace.locale_preparation_catalog(migrate_legacy=True)
                with self.assertRaisesRegex(ValueError, "database preparation failed"):
                    workspace.publish_locale_preparation(document, snapshots, baseline, [])
                self.assertEqual(legacy.read_bytes(), b'{"RESET":"RESET"}')
                document, _, retry_baseline = workspace.locale_preparation_catalog(migrate_legacy=True)
                self.assertEqual(document["locales"], [{"locale": "en", "messages": {"RESET": "RESET"}}])
                self.assertEqual(len(retry_baseline), 1)

    def test_product_input_closure_requires_the_installed_locale_owner(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True)
            (root / "Cargo.toml").write_bytes(b"workspace manifest")
            (root / "Cargo.lock").write_bytes(b"locked dependencies")
            with mock.patch.object(workspace, "ROOT", root):
                source.write_text('rust_i18n::i18n!("../../locales", fallback="en");', encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "installed locale snapshot"):
                    workspace.rust_inputs()
                source.write_text('pub fn label() { let _ = razer_i18n::t("RESET"); }', encoding="utf-8")
                self.assertEqual(len(workspace.rust_inputs()), 3)
                source.write_text('const COPY: &str = include_str!("../../../locales/en.json");', encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "outside installed inputs"):
                    workspace.rust_inputs()


class HardwareInstallationInputTests(unittest.TestCase):
    def test_generated_driver_constants_track_policy_payload_and_build_script(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            owner = root / "crates/razer-platform/src/observation/hardware_driver.rs"
            owner.parent.mkdir(parents=True)
            source = 'include!(concat!(env!("OUT_DIR"), "/hardware_driver_installation.rs"));'
            owner.write_text(source, encoding="utf-8")
            build = root / "crates/razer-platform/build.rs"
            build.write_text('fn main() {}', encoding="utf-8")
            (root / "Cargo.toml").write_bytes(b"manifest")
            (root / "Cargo.lock").write_bytes(b"lock")
            policy = root / "packaging/windows/installation.json"
            policy.parent.mkdir(parents=True)
            driver = {"service_name": "cpuz165", "filename": "cpuz165.sys", "directory": "drivers",
                      "system_directory": "RazerUI", "source": "assets/native/cpuz165.sys", "architecture": "x64"}
            policy.write_text(json.dumps({"schema_version": 1, "hardware_driver": driver}), encoding="utf-8")
            payload = root / driver["source"]
            payload.parent.mkdir(parents=True)
            payload.write_bytes(b"driver before")
            with mock.patch.object(workspace, "ROOT", root):
                first = {item["path"]: item for item in workspace.rust_inputs()}
                self.assertIn("packaging/windows/installation.json", first)
                self.assertIn("crates/razer-platform/build.rs", first)
                payload.write_bytes(b"driver after")
                second = {item["path"]: item for item in workspace.rust_inputs()}
                self.assertNotEqual(first[driver["source"]]["sha256"], second[driver["source"]]["sha256"])
                owner.write_text(source + '\ninclude!(concat!(env!("OUT_DIR"), "/other.rs"));', encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "not a statically audited literal"):
                    workspace.rust_inputs()
                owner.write_text(source.replace("hardware_driver_installation.rs", "other.rs"), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "new build-output audit"):
                    workspace.rust_inputs()


class ArchitectureBoundaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import resource_scope
        cls.rust_parser = resource_scope.parser(workspace.ROOT)

    def inspect(self, source, owner):
        return workspace.inspect_architecture_source(source.encode(), owner=owner, rust_parser=self.rust_parser)

    def test_workspace_constructors_preserve_missing_profiles_and_selection(self):
        owners = (
            'crates/razer-pages/src/features/workspace/workspace.rs',
            'crates/razer-pages/src/features/workspace/source_workspace.rs',
            'crates/razer-model/src/product_preview.rs',
        )
        for owner in owners:
            for name in ('new', 'new_loaded', 'registered_preview', 'mouse_mat_preview'):
                for body, kind in (
                    ('device.profiles.push(Profile { id, guid, name });', 'workspace_constructor_creates_profile_collection'),
                    ('if device.profiles.is_empty() { device.profiles.extend(defaults); }', 'workspace_constructor_creates_profile_collection'),
                    ('device.active_profile = device.profiles[0].id.clone();', 'workspace_constructor_selects_unobserved_profile'),
                    ('this.saved = this.device.clone();', 'workspace_restore_or_constructor_persists_reducer_state'),
                ):
                    with self.subTest(owner=owner, name=name, body=body):
                        source = 'fn ' + name + '() { ' + body + ' }'
                        self.assertIn(kind, [row['kind'] for row in self.inspect(source, owner)])
                        self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
            self.assertFalse(self.inspect('fn explicit_add() { device.profiles.push(profile); device.active_profile = id; }', owner))
            self.assertFalse(self.inspect('fn new() { let device = Device { profiles: Vec::new(), active_profile: String::new() }; }', owner))
        self.assertFalse(self.inspect('fn new() { cache.profiles.push(row); }', 'crates/razer-storage/src/unrelated.rs'))

    def test_workspace_restore_cannot_store_reducer_defaults(self):
        owners = (
            'crates/razer-pages/src/features/workspace/workspace.rs',
            'crates/razer-pages/src/features/workspace/source_workspace.rs',
        )
        for owner in owners:
            for name in ('restore_active', 'restore_active_with_dpi_observation', 'restore_from_device'):
                for body in ('profile.source_settings = Some(body.snapshot());',
                             'self.device.source_device_settings = Some(body.device_snapshot());',
                             'self.saved = self.device.clone();'):
                    with self.subTest(owner=owner, name=name, body=body):
                        self.assertIn('workspace_restore_or_constructor_persists_reducer_state',
                                      [row['kind'] for row in self.inspect('fn ' + name + '() { ' + body + ' }', owner)])
            self.assertFalse(self.inspect('fn restore_active() { body.restore(input); self.reducer = observed; }', owner))
            self.assertFalse(self.inspect('fn accept_saved() { self.saved = snapshot; }', owner))
            self.assertFalse(workspace.inspect_architecture_source((workspace.ROOT / owner).read_bytes(),
                             owner=owner, rust_parser=self.rust_parser))

    def test_configuration_paths_keep_resolution_errors_and_one_owner(self):
        owner = 'crates/razer-storage/src/paths.rs'
        for body in (
            'std::env::var("APPDATA").unwrap_or_else(|_| ".".into())',
            'std::env::var_os("APPDATA").unwrap_or_default()',
            'directory().or_else(|_| Ok(PathBuf::from(".")))',
            'directory().expect("config")',
        ):
            source = 'fn configuration_directory() { let path = ' + body + '; }'
            with self.subTest(body=body):
                self.assertIn('configuration_path_replaces_a_resolution_failure',
                              [row['kind'] for row in self.inspect(source, owner)])
                self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
        source = 'use std::env as location; fn configuration_directory() { location::current_dir(); }'
        self.assertIn('configuration_path_uses_working_directory',
                      [row['kind'] for row in self.inspect(source, owner)])
        for consumer in ('crates/razer-shell/src/shell.rs', 'crates/razer-platform/src/camera/camera_preview.rs',
                         'crates/razer-storage/src/lib.rs'):
            self.assertIn('configuration_path_has_a_second_owner',
                          [row['kind'] for row in self.inspect('fn store_path() {}', consumer)])
            self.assertFalse(self.inspect('fn save() { let path = razer_storage::store_path()?; }', consumer))
        self.assertFalse(self.inspect('fn restore() { directory().unwrap_or_default(); }',
                                      'crates/razer-storage/src/unrelated.rs'))
        self.assertFalse(workspace.inspect_architecture_source((workspace.ROOT / owner).read_bytes(),
                         owner=owner, rust_parser=self.rust_parser))

    def test_independent_help_report_cannot_reintroduce_vendor_loading(self):
        for owner in ("crates/razer-service/src/native/help_system_info.rs",
                      "crates/razer-service/src/native/help_system_info/windows.rs"):
            for source in (
                "use crate::dll::EngineLibrary;",
                "use crate::dll as backend; fn query() { backend::EngineLibrary::load(path); }",
                "fn query() { crate::native_read::getter(args); }",
                "use libloading::{Library as Backend}; fn query() { Backend::new(path); }",
                "use razer_platform as platform; fn query() { platform::native_paths::EnginePaths::discover(); }",
            ):
                with self.subTest(owner=owner, source=source):
                    self.assertIn("independent_help_report_uses_vendor_loader",
                                  [row["kind"] for row in self.inspect(source, owner)])
            self.assertFalse(self.inspect('fn query() { let comment = "crate::dll::EngineLibrary"; }', owner))
            self.assertFalse(self.inspect('use windows::Win32::System::Wmi::IWbemServices;', owner))
        self.assertFalse(self.inspect('fn unrelated() { local::Library::new(); }',
                                      'crates/razer-storage/src/other.rs'))

    def test_product_protocol_does_not_expose_generic_vendor_execution(self):
        self.assertEqual(self.inspect('pub enum ServiceRequest { NativeLibrarySnapshot { library: String } }',
            'crates/razer-ipc/src/lib.rs')[0]['kind'], 'product_protocol_exposes_vendor_analysis')
        for owner in ('crates/razer-settings/src/runtime_page.rs',
                      'crates/razer-service/src/runtime/windows/native.rs'):
            for source in ('fn read() { ServiceRequest::NativeLibrarySnapshot { library }; }',
                           'use razer_ipc::ServiceRequest as Request; fn read() { Request::NativeLibrarySnapshot { library }; }'):
                self.assertEqual(self.inspect(source, owner)[0]['kind'], 'product_protocol_exposes_vendor_analysis')
        self.assertEqual(self.inspect('', 'crates/razer-service/src/native/native_read.rs')[0]['kind'],
                         'retired_architecture_source')
        self.assertFalse(self.inspect('fn unrelated() { let name = "NativeLibrarySnapshot"; }',
                                      'crates/razer-settings/src/runtime_page.rs'))

    def test_chroma_installation_observation_has_one_shell_owner(self):
        shared = 'crates/razer-shell/src/shell/devices/product_chroma.rs'
        for owner in ('crates/razer-shell/src/shell/gamepad/controller_chroma.rs',
                      'crates/razer-shell/src/shell/devices/product_lighting.rs',
                      'crates/razer-shell/src/shell.rs',
                      'crates/razer-pages/src/features/gamepad/gamepad_chroma_runtime.rs',
                      'crates/razer-settings/src/preferences/settings_lighting.rs'):
            for source in (
                'fn read() { razer_platform::chroma_installation::inspect(); }',
                'use razer_platform::chroma_installation as installation; fn read() { installation::inspect(); }',
                'use razer_platform::chroma_installation::{inspect as observe}; fn read() { observe(); }',
                'use razer_platform as platform; fn read() { platform::chroma_installation::inspect(); }',
            ):
                with self.subTest(owner=owner, source=source):
                    rows = self.inspect(source, owner)
                    self.assertEqual([row['kind'] for row in rows],
                                     ['chroma_installation_has_a_second_observation_owner'])
                    self.assertEqual(rows[0]['expected_owner'], shared)
                    self.assertFalse(self.inspect(source, shared))
                    self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
            for source in (
                'fn read() { super::product_chroma::readback(); }',
                'fn read() { product_chroma::from_snapshot(snapshot); }',
                'fn read() { let hint = "razer_platform::chroma_installation::inspect()"; other::inspect(); }',
            ):
                self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('fn read() { razer_platform::chroma_installation::inspect(); }',
                                     'crates/razer-platform/src/observation/chroma_installation.rs'))
        for owner in (shared, 'crates/razer-shell/src/shell/gamepad/controller_chroma.rs'):
            self.assertFalse(workspace.inspect_architecture_source((workspace.ROOT / owner).read_bytes(),
                             owner=owner, rust_parser=self.rust_parser))

    def test_interaction_styles_cannot_change_display_after_prepaint(self):
        owners = (
            'crates/razer-pages/src/features/mouse/dpi/mouse_dpi_rows.rs',
            'crates/razer-pages/src/features/keyboard/mapping/keyboard_mapping.rs',
            'crates/razer-widgets/src/source_tooltip/source_tooltip.rs',
        )
        for owner in owners:
            for interaction in ('hover', 'group_hover', 'active', 'group_active', 'focus', 'focus_visible', 'focus_within'):
                for change in ('style.flex()', 'style.hidden()', 'style.block()',
                               'style.when(enabled, |s| s.grid())',
                               '{ style.style().display = Some(Display::None); style }'):
                    args = 'group, ' if interaction.startswith('group_') else ''
                    source = f'fn render() {{ div().hidden().{interaction}({args}|style| {change}); }}'
                    with self.subTest(owner=owner, source=source):
                        self.assertEqual([row['kind'] for row in self.inspect(source, owner)],
                                         ['interaction_style_changes_display_between_draw_phases'])
                        self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
            for source in (
                'fn render() { div().when(!row_hovered, |row| row.hidden()).child(content); }',
                'fn render() { div().invisible().group_hover(group, |style| style.visible()); }',
                'fn render() { div().hover(|style| style.bg(color)).active(|style| style.opacity(0.3)); }',
                'fn render() { let text = "hidden flex display"; callback(|| div().hidden()); }',
            ):
                self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('fn domain() { item.hover(|style| style.block()); }',
                                     'crates/razer-device/src/domain.rs'))

    def test_current_dpi_gui_delegates_native_io_and_retains_delivery(self):
        owners = (
            'crates/razer-shell/src/shell/mouse/mouse_current_dpi.rs',
            'crates/razer-shell/src/shell/mouse/mouse_current_dpi_runtime.rs',
            'crates/razer-pages/src/features/mouse/dpi/mouse_current_dpi.rs',
        )
        worker = 'crates/razer-shell/src/shell/mouse/mouse_current_dpi_worker.rs'
        for owner in owners:
            for source in (
                'fn start() { ServiceClient::spawn(); }',
                'use razer_ipc::ServiceClient as Client; fn start() { Client::spawn(); }',
                'use razer_ipc as ipc; fn start() { ipc::ServiceClient::spawn(); }',
                'fn start() { direct::mouse_current_dpi::request(client, route, change); }',
                'use razer_discovery::direct::mouse_current_dpi as dpi; fn start() { dpi::request(client, route, change); }',
                'use razer_discovery::direct::mouse_current_dpi::request as read; fn start() { read(client, route, change); }',
                'fn start() { std::thread::spawn(work); }',
                'use std::thread::{Builder as Worker}; fn start() { Worker::new(); }',
            ):
                with self.subTest(owner=owner, source=source):
                    rows = self.inspect(source, owner)
                    self.assertEqual([row['kind'] for row in rows], ['mouse_dpi_gui_owns_device_io'])
                    self.assertEqual(rows[0]['expected_owner'], worker)
                    self.assertFalse(self.inspect(source, worker))
                    self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
            for call in ('delivery.detach()', 'worker.join()', 'self.worker.take().unwrap().join()'):
                with self.subTest(owner=owner, call=call):
                    self.assertEqual([row['kind'] for row in self.inspect('fn start() { ' + call + '; }', owner)],
                                     ['mouse_dpi_gui_detaches_or_waits_for_native_work'])
            self.assertFalse(self.inspect('fn start() { let message = messages.join(", "); cx.emit(intent); }', owner))
        self.assertFalse(self.inspect('fn start() { ServiceClient::spawn(); }',
                                     'crates/razer-shell/src/shell/mouse/unrelated.rs'))

    def test_current_dpi_dispatch_and_route_use_their_retained_owners(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_current_dpi.rs'
        source = 'impl AppShell { fn submit_current_dpi_device() { self.start_current_dpi_operation(workspace, route, request, cx); } }'
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('self.start_current_dpi_operation', 'other.start_current_dpi_operation'),
                        source.replace('self.start_current_dpi_operation', 'self.start_other_operation')):
            self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                             ['mouse_dpi_bypasses_retained_operation_owner'])
            self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))
        delegated = ('impl AppShell { '
                     'fn submit_current_dpi_device() { self.submit_current_dpi_at(workspace, request, window, cx); } '
                     'fn submit_current_dpi_at() { self.start_current_dpi_operation(workspace, route, request, window, cx); } }')
        self.assertFalse(self.inspect(delegated, owner))
        for changed in (
                delegated.replace('self.submit_current_dpi_at', 'other.submit_current_dpi_at'),
                delegated.replace('self.start_current_dpi_operation', 'other.start_current_dpi_operation'),
                delegated.replace('self.start_current_dpi_operation', 'self.start_other_operation'),
                delegated.replace('fn submit_current_dpi_at()', '#[cfg(test)] fn submit_current_dpi_at()'),
                delegated.replace('fn submit_current_dpi_at()', 'fn unrelated()'),
                delegated.replace('self.start_current_dpi_operation', 'self.submit_current_dpi_at')):
            with self.subTest(source=changed):
                self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                                 ['mouse_dpi_bypasses_retained_operation_owner'])
        owner = 'crates/razer-shell/src/shell/mouse/mouse_current_dpi_runtime.rs'
        source = 'fn current_route() { super::device_discovery::retained_connection(previous, observations, device) }'
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('use super::device_discovery::retained_connection as retained; '
                                     + source.replace('super::device_discovery::retained_connection', 'retained'), owner))
        self.assertEqual([row['kind'] for row in self.inspect(source.replace('super::device_discovery::', 'other::'), owner)],
                         ['mouse_dpi_does_not_retain_the_physical_connection'])
        for source in ('fn current() { shell.discovery_revision == revision }',
                       'fn current() { discovery_revision == revision }'):
            self.assertIn('mouse_dpi_uses_global_discovery_revision',
                          [row['kind'] for row in self.inspect(source, owner)])
        self.assertFalse(self.inspect('fn diagnostic() { log("discovery_revision"); }', owner))

    def test_mouse182_initialization_has_one_connection_owner_before_dispatch(self):
        state = 'crates/razer-pages/src/features/mouse/dpi/mouse_current_dpi.rs'
        good = 'impl State { fn initialize_connection() { self.install_connection(key, scope); self.initialize_profile(input); } }'
        self.assertFalse(self.inspect(good, state))
        for bad in (good.replace('self.install_connection(key, scope);', ''),
                    good.replace('self.install_connection(key, scope); self.initialize_profile(input);',
                                 'self.initialize_profile(input); self.install_connection(key, scope);')):
            self.assertEqual([row['kind'] for row in self.inspect(bad, state)],
                             ['mouse_dpi_initializer_precedes_connection_installation'])
        self.assertFalse(self.inspect('impl Other { fn initialize_connection() {} }', state))
        workspace_owner = 'crates/razer-pages/src/features/workspace/source_workspace.rs'
        good = 'fn initialize_current_mouse_profile() { body.update(cx, |body,cx| { body.initialize_mouse182_profile(key, guid, cx); }); }'
        self.assertFalse(self.inspect(good, workspace_owner))
        bad = good.replace('body.initialize_mouse182_profile(key, guid, cx);',
                           'body.initialize_mouse182_profile(key, guid, cx); body.observe_current_dpi_connection(key, cx);')
        self.assertEqual([row['kind'] for row in self.inspect(bad, workspace_owner)],
                         ['mouse182_initializer_has_a_second_connection_owner'])
        self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + bad + ' }', workspace_owner))
        separate = good.replace('fn initialize_current_mouse_profile() {',
            'fn initialize_current_mouse_profile() { if non182 { body.update(cx, |body,cx| body.observe_current_dpi_connection(key,cx)); return; }')
        self.assertFalse(self.inspect(separate, workspace_owner))
        for owner in (state, workspace_owner):
            self.assertFalse(workspace.inspect_architecture_source((workspace.ROOT / owner).read_bytes(),
                             owner=owner, rust_parser=self.rust_parser))

    def test_idle_and_current_power_use_the_same_retained_io_boundary(self):
        caller = 'crates/razer-shell/src/shell/mouse/mouse_current_power.rs'
        runtime = 'crates/razer-shell/src/shell/mouse/mouse_current_power_runtime.rs'
        page = 'crates/razer-pages/src/features/mouse/power/mouse_current_power.rs'
        worker = 'crates/razer-shell/src/shell/mouse/mouse_current_power_worker.rs'
        discovery = 'crates/razer-shell/src/shell/devices/device_discovery.rs'
        sources = [
            'use razer_ipc::ServiceClient as Client; fn start() { Client::spawn(); }',
            'use razer_discovery::direct::mouse_current_power as power; fn start() { power::request(client, route, change); }',
            'use razer_discovery::direct::mouse_receiver_polling::write_idle as idle; fn start() { idle(client, route, seconds); }',
            'use std::thread::Builder as Worker; fn start() { Worker::new(); }',
        ]
        for owner in (caller, runtime, page):
            for source in sources:
                self.assertEqual([row['kind'] for row in self.inspect(source, owner)], ['mouse_power_gui_owns_device_io'])
                self.assertFalse(self.inspect(source, worker))
                self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
            self.assertEqual(self.inspect('fn start() { task.detach(); }', owner)[0]['kind'],
                             'mouse_power_gui_detaches_or_waits_for_native_work')
            self.assertIn('mouse_power_uses_global_discovery_revision',
                          [row['kind'] for row in self.inspect('fn current() { self.discovery_revision == old }', owner)])
            self.assertFalse(self.inspect('fn status() { messages.join(", "); }', owner))
        good = 'fn write_mouse_idle() { self.start_mouse_power_operation(page, route, request, cx); }'
        self.assertFalse(self.inspect(good, discovery))
        bad = good.replace('self.start_mouse_power_operation(page, route, request, cx);',
                           'ServiceClient::spawn(); task.detach(); self.discovery_revision == old;')
        self.assertEqual(set(row['kind'] for row in self.inspect(bad, discovery)), {
            'mouse_power_bypasses_retained_operation_owner', 'mouse_power_gui_owns_device_io',
            'mouse_power_gui_detaches_or_waits_for_native_work', 'mouse_power_uses_global_discovery_revision'})
        self.assertFalse(self.inspect('fn other_feature() { ServiceClient::spawn(); }', discovery))
        good = good.replace('write_mouse_idle', 'submit_current_power_device')
        self.assertFalse(self.inspect(good, caller))
        self.assertEqual(self.inspect(good.replace('self.start_mouse_power_operation', 'other.start_mouse_power_operation'), caller)[0]['kind'],
                         'mouse_power_bypasses_retained_operation_owner')
        good = 'use super::device_discovery::retained_connection as retained; fn current_route() { retained(old, observed, device) }'
        self.assertFalse(self.inspect(good, runtime))
        self.assertEqual(self.inspect(good.replace('retained(old, observed, device)', 'true'), runtime)[0]['kind'],
                         'mouse_power_does_not_retain_the_physical_connection')

    def test_mouse_polling_gui_delegates_io_without_detaching_or_waiting(self):
        owners = (
            'crates/razer-shell/src/shell/mouse/mouse_polling_write.rs',
            'crates/razer-shell/src/shell/mouse/mouse_polling_runtime.rs',
            'crates/razer-pages/src/features/mouse/polling/mouse_polling.rs',
            'crates/razer-pages/src/features/mouse/polling/mouse_polling_profile.rs',
            'crates/razer-pages/src/features/mouse/polling/mouse_polling_view.rs',
        )
        worker = 'crates/razer-shell/src/shell/mouse/mouse_polling_worker.rs'
        for owner in owners:
            for source in (
                'fn start() { ServiceClient::spawn(); }',
                'use razer_ipc::ServiceClient as Client; fn start() { Client::spawn(); }',
                'fn start() { direct::mouse_current_polling::request(client, route, change); }',
                'use razer_discovery::direct::mouse_current_polling as polling; fn start() { polling::request(client, route, change); }',
                'use razer_discovery::direct::mouse_receiver_polling::write as set; fn start() { set(client, route, hz); }',
                'fn start() { std::thread::spawn(work); }',
                'use std::thread::{Builder as Worker}; fn start() { Worker::new(); }',
            ):
                with self.subTest(owner=owner, source=source):
                    rows = self.inspect(source, owner)
                    self.assertEqual([row['kind'] for row in rows], ['mouse_polling_gui_owns_device_io'])
                    self.assertEqual(rows[0]['expected_owner'], worker)
                    self.assertFalse(self.inspect(source, worker))
                    self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
            for call in ('delivery.detach()', 'worker.join()', 'self.worker.take().unwrap().join()'):
                with self.subTest(owner=owner, call=call):
                    self.assertEqual([row['kind'] for row in self.inspect('fn start() { ' + call + '; }', owner)],
                                     ['mouse_polling_gui_detaches_or_waits_for_native_work'])
            self.assertFalse(self.inspect('fn start() { let message = messages.join(", "); cx.emit(intent); }', owner))
        self.assertFalse(self.inspect('fn start() { ServiceClient::spawn(); }',
                                     'crates/razer-shell/src/shell/mouse/unrelated.rs'))

    def test_mouse_polling_dispatch_and_route_use_retained_owners(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_polling_write.rs'
        source = 'impl AppShell { fn write_mouse_polling_device() { self.start_mouse_polling_operation(workspace, route, request, cx); } }'
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('self.start_mouse_polling_operation', 'other.start_mouse_polling_operation'),
                        source.replace('self.start_mouse_polling_operation', 'self.start_other_operation')):
            self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                             ['mouse_polling_bypasses_retained_operation_owner'])
            self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))
        owner = 'crates/razer-shell/src/shell/mouse/mouse_polling_runtime.rs'
        source = 'fn current_route() { super::device_discovery::retained_connection(previous, observations, device) }'
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('use super::device_discovery::retained_connection as retained; '
                                     + source.replace('super::device_discovery::retained_connection', 'retained'), owner))
        self.assertEqual([row['kind'] for row in self.inspect(source.replace('super::device_discovery::', 'other::'), owner)],
                         ['mouse_polling_does_not_retain_the_physical_connection'])
        for source in ('fn current() { shell.discovery_revision == revision }',
                       'fn current() { discovery_revision == revision }'):
            self.assertIn('mouse_polling_uses_global_discovery_revision',
                          [row['kind'] for row in self.inspect(source, owner)])
        self.assertFalse(self.inspect('fn diagnostic() { log("discovery_revision"); }', owner))

    def test_keyboard_brightness_gui_delegates_io_to_its_retained_worker(self):
        owners = (
            'crates/razer-shell/src/shell/keyboard/keyboard_brightness_read.rs',
            'crates/razer-shell/src/shell/keyboard/keyboard_brightness_write.rs',
            'crates/razer-shell/src/shell/keyboard/keyboard_brightness_runtime.rs',
            'crates/razer-pages/src/features/keyboard/lighting/keyboard_brightness.rs',
        )
        sources = (
            'fn start() { ServiceClient::spawn(); }',
            'use razer_ipc::ServiceClient as Client; fn start() { Client::spawn(); }',
            'use razer_ipc as ipc; fn start() { ipc::ServiceClient::spawn(); }',
            'fn start() { direct::request_keyboard_brightness(client, route, percent); }',
            'use razer_discovery::direct::request_keyboard_brightness as request; fn start() { request(client, route, percent); }',
            'fn start() { client.request_keyboard_brightness(route, percent); }',
            'fn start() { std::thread::spawn(work); }',
            'fn start() { std::thread::Builder::new(); }',
            'use std::thread as worker; fn start() { worker::spawn(work); }',
            'use std::thread::{spawn as launch, Builder as Worker}; fn start() { launch(work); }',
            'use std::thread::{Builder as Worker}; fn start() { Worker::new(); }',
        )
        worker = 'crates/razer-shell/src/shell/keyboard/keyboard_brightness_worker.rs'
        for owner in owners:
            with self.subTest(owner=owner):
                good = ('fn start() { self.start_keyboard_brightness_operation(scope, percent, canceled, complete, cx); '
                        'let message = messages.join(", "); let thread = std::thread::current(); }')
                self.assertFalse(self.inspect(good, owner))
                for source in sources:
                    with self.subTest(source=source):
                        rows = self.inspect(source, owner)
                        self.assertEqual([row['kind'] for row in rows], ['keyboard_brightness_gui_owns_device_io'])
                        self.assertEqual(rows[0]['expected_owner'], worker)
                        self.assertFalse(self.inspect(source, worker))
                        self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
                for call in ('delivery.detach()', 'worker.join()', 'self.worker.take().unwrap().join()'):
                    source = 'fn start() { ' + call + '; }'
                    self.assertEqual([row['kind'] for row in self.inspect(source, owner)],
                                     ['keyboard_brightness_gui_detaches_or_waits_for_native_work'])
                    self.assertFalse(self.inspect('#[cfg(test)] ' + source, owner))
        self.assertFalse(self.inspect(sources[0], owners[0].replace('_read.rs', '_runtime_tests.rs')))
        self.assertFalse(self.inspect(sources[0], 'crates/razer-shell/src/shell/unrelated.rs'))

    def test_keyboard_brightness_callers_use_the_shared_scope_and_operation_owner(self):
        for operation in ('read', 'write'):
            owner = f'crates/razer-shell/src/shell/keyboard/keyboard_brightness_{operation}.rs'
            source = ('use super::keyboard_brightness_runtime::Scope; impl AppShell { '
                      f'fn {operation}_keyboard_brightness() {{ let scope = Scope::new(workspace, route, cx); '
                      'self.start_keyboard_brightness_operation(scope, percent, canceled, complete, cx); } }')
            with self.subTest(operation=operation):
                self.assertFalse(self.inspect(source, owner))
                self.assertFalse(self.inspect(source.replace('::Scope;', '::Scope as RequestScope;')
                                              .replace('Scope::new', 'RequestScope::new'), owner))
                for changed in (source.replace('self.start_keyboard_brightness_operation', 'other.start_keyboard_brightness_operation'),
                                source.replace('self.start_keyboard_brightness_operation', 'self.start_unrelated_operation'),
                                source.replace('Scope::new', 'OtherScope::new')):
                    self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                                     ['keyboard_brightness_bypasses_retained_operation_owner'])
                    self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))
                changed = source.replace('let scope =', 'let revision = self.discovery_revision; let scope =')
                self.assertIn('keyboard_brightness_uses_global_discovery_revision',
                              [row['kind'] for row in self.inspect(changed, owner)])
                self.assertFalse(self.inspect(source.replace(f'fn {operation}_keyboard_brightness', 'fn unrelated')
                                              .replace('Scope::new', 'OtherScope::new'), owner))

    def test_keyboard_brightness_scope_uses_the_physical_connection_owner(self):
        owner = 'crates/razer-shell/src/shell/keyboard/keyboard_brightness_runtime.rs'
        source = ('fn current_route() { super::device_discovery::retained_connection(previous, observations, device) '
                  '&& direct::keyboard_brightness_available(route) }')
        self.assertFalse(self.inspect(source, owner))
        alias = 'use super::device_discovery::retained_connection as retained; '
        self.assertFalse(self.inspect(alias + source.replace('super::device_discovery::retained_connection', 'retained'), owner))
        for changed in (source.replace('super::device_discovery::retained_connection(previous, observations, device)', 'true'),
                        source.replace('super::device_discovery::retained_connection', 'unrelated::retained_connection')):
            self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                             ['keyboard_brightness_does_not_retain_the_physical_connection'])
            self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))
        for source in ('fn current() { shell.discovery_revision == revision }',
                       'fn current() { discovery_revision == revision }'):
            self.assertIn('keyboard_brightness_uses_global_discovery_revision',
                          [row['kind'] for row in self.inspect(source, owner)])
        self.assertFalse(self.inspect('fn diagnostic() { log("discovery_revision"); }', owner))

    def test_keyboard_brightness_state_and_cleanup_belong_to_the_shell(self):
        owner = 'crates/razer-shell/src/shell.rs'
        source = ('struct AppShell { thx_retired_owners: Vec<X>, keyboard_brightness: keyboard_brightness_runtime::State, '
                  'receiver_chroma_connect: std::sync::Arc<std::sync::Mutex<razer_service::chroma_connect::Session>> }')
        self.assertFalse(self.inspect(source, owner))
        alias = 'use crate::shell::keyboard_brightness_runtime::State as Brightness; '
        self.assertFalse(self.inspect(alias + source.replace('keyboard_brightness_runtime::State', 'Brightness'), owner))
        for changed in (source.replace('keyboard_brightness: keyboard_brightness_runtime::State, ', ''),
                        source.replace('keyboard_brightness:', 'other:'),
                        source.replace('keyboard_brightness_runtime::State', 'Option<keyboard_brightness_runtime::State>'),
                        source.replace('keyboard_brightness_runtime::State', 'unrelated::State'),
                        source.replace('keyboard_brightness:', 'duplicate: keyboard_brightness_runtime::State, keyboard_brightness:')):
            self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                             ['keyboard_brightness_has_no_single_retained_shell_owner'])
            self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))
        install = 'impl AppShell { fn new() { this.install_keyboard_brightness_cleanup(cx); } }'
        self.assertFalse(self.inspect(install, owner))
        for changed in (install.replace('this.install_keyboard_brightness_cleanup(cx);', ''),
                        install.replace('this.install_keyboard_brightness_cleanup', 'other.install_keyboard_brightness_cleanup')):
            self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                             ['keyboard_brightness_cleanup_is_not_installed'])
        self.assertFalse(self.inspect(install.replace('AppShell', 'Unrelated'), owner))

    def test_keyboard_brightness_work_contributes_to_the_exit_pending_gate(self):
        owner = 'crates/razer-shell/src/shell.rs'
        source = ('fn continue_save_queue() { if !succeeded { self.cancel_pending_exit(cx); } '
                  'if self.save_task.is_some() { return; } '
                  'let operations_pending = other_pending || self.keyboard_brightness.pending(); '
                  'if succeeded && !queued && !auxiliary_pending { if operations_pending { return; } '
                  'if self.close_requested { if self.finish_keyboard_actuation_exit(cx) || !self.close_requested { return; } '
                  'workspace.finish_exit(cx); self.stop_global_shortcuts(); '
                  'if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; } '
                  'if self.finish_service_exit(cx) || self.finish_resource_exit(cx) { return; } } } '
                  'match continuation { SaveContinuation::Close => cx.quit(), _ => {} } }')
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect(source.replace('other_pending || self.keyboard_brightness.pending()',
                                                    '(self.keyboard_brightness.pending()) || other_pending'), owner))
        for changed in (source.replace('let operations_pending = other_pending || self.keyboard_brightness.pending(); ', ''),
                        source.replace('self.keyboard_brightness.pending()', 'self.other.pending()'),
                        source.replace('self.keyboard_brightness.pending()', '!self.keyboard_brightness.pending()'),
                        source.replace('other_pending || self.keyboard_brightness.pending()', 'other_pending && self.keyboard_brightness.pending()')):
            self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                             ['keyboard_brightness_work_is_not_drained_before_exit'])
            self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))

    def test_camera_pages_delegate_work_to_the_retained_service_owner(self):
        source = ('use razer_service::camera::lifecycle::Owner; '
                  'impl State { fn start(&mut self) { let jobs = Owner::default(); '
                  'let receipt = jobs.spawn("camera-page-ipc", canceled, move || execute()); '
                  'self.delivery = Some(cx.spawn(deliver(receipt))); '
                  'self.retiring = Some(jobs.cleanup("camera-release", move || session.shutdown())); } }')
        for name in ('camera_runtime', 'camera_media', 'camera_mic', 'camera_source_state', 'camera_shortcuts'):
            owner = f'crates/razer-pages/src/features/workspace/source_controls/{name}.rs'
            with self.subTest(owner=owner):
                self.assertFalse(self.inspect(source, owner))
                for call in ('delivery.detach()', 'worker.join()', 'self.worker.take().unwrap().join()'):
                    with self.subTest(call=call):
                        rows = self.inspect('fn start() { ' + call + '; }', owner)
                        self.assertEqual([row['kind'] for row in rows],
                                         ['camera_page_detaches_or_waits_for_native_work'])
                        self.assertEqual(rows[0]['expected_owner'], 'crates/razer-service/src/camera/lifecycle.rs')

    def test_camera_pages_reject_native_thread_and_service_constructors_through_aliases(self):
        owner = 'crates/razer-pages/src/features/workspace/source_controls/camera_runtime.rs'
        sources = (
            'fn start() { std::thread::spawn(work); }',
            'fn start() { std::thread::Builder::new(); }',
            'use std::thread; fn start() { thread::spawn(work); }',
            'use std::thread as worker; fn start() { worker::spawn(work); }',
            'use std::thread::{self as worker}; fn start() { worker::spawn(work); }',
            'use std::thread::spawn as launch; fn start() { launch(work); }',
            'use std::thread::Builder as Thread; fn start() { Thread::new(); }',
            'use std::thread::{Builder as Thread}; fn start() { Thread::new(); }',
            'fn start() { ServiceClient::spawn(); }',
            'fn start() { razer_ipc::ServiceClient::spawn(); }',
            'use razer_ipc::ServiceClient as Client; fn start() { Client::spawn(); }',
            'use razer_ipc as ipc; fn start() { ipc::ServiceClient::spawn(); }',
            'fn start() { SessionMonitor::start(); }',
            'fn start() { razer_platform::session_changes::SessionMonitor::start(); }',
            'use razer_platform::session_changes::SessionMonitor as Monitor; fn start() { Monitor::start(); }',
            'use razer_platform::session_changes as sessions; fn start() { sessions::SessionMonitor::start(); }',
        )
        for source in sources:
            with self.subTest(source=source):
                rows = self.inspect(source, owner)
                self.assertEqual([row['kind'] for row in rows], ['camera_page_starts_unowned_native_work'])
                self.assertEqual(rows[0]['expected_owner'], 'crates/razer-service/src/camera/lifecycle.rs')
                self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
                self.assertFalse(self.inspect(source, 'crates/razer-service/src/camera/commands.rs'))

    def test_camera_task_guard_keeps_string_join_and_test_workers_valid(self):
        owner = 'crates/razer-pages/src/features/workspace/source_controls/camera_media.rs'
        self.assertFalse(self.inspect('fn label() { let text = messages.join(", "); }', owner))
        self.assertFalse(self.inspect('fn label() { let text = ["camera", "ready"].join(" "); }', owner))
        source = 'fn start() { std::thread::spawn(work); worker.join(); delivery.detach(); }'
        self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
        self.assertFalse(self.inspect('#[cfg(test)] ' + source, owner))
        self.assertFalse(self.inspect(source, owner.replace('camera_media.rs', 'camera_media_tests.rs')))
        self.assertFalse(self.inspect(source, 'crates/razer-service/src/camera/lifecycle.rs'))

    def test_camera_exit_closes_task_admission_before_the_shell_drains(self):
        owner = 'crates/razer-pages/src/features/workspace/source_workspace/lifecycle.rs'
        source = ('impl SourceProductWorkspace { fn prepare_exit() { self.exiting = true; '
                  'body.prepare_camera_exit(cx); self.set_active(false); '
                  'body.dismiss_editors(); audio.prepare_exit(); } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('body.prepare_camera_exit(cx);', ''),
                        source.replace('body.prepare_camera_exit(cx);', 'body.prepare_unrelated_exit(cx);')):
            with self.subTest(source=changed):
                self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                                 ['camera_exit_does_not_close_native_task_admission'])
                self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))
                self.assertFalse(self.inspect(changed, 'crates/razer-pages/src/features/workspace/unrelated.rs'))
        self.assertFalse(self.inspect('fn unrelated() { body.prepare_exit(); }', owner))

    def test_camera_activity_cannot_reopen_the_scope_during_exit(self):
        owner = 'crates/razer-pages/src/features/workspace/source_workspace.rs'
        source = ('impl SourceProductWorkspace { fn sync_camera_activity(&self) { '
                  'let active = !self.exiting && device.connected(); body.set_camera_scope(active.then(scope)); } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('!self.exiting && ', ''), source.replace('!self.exiting', 'self.exiting'),
                        source.replace('!self.exiting', '!other.exiting'), source.replace('!self.exiting', '!self.active')):
            with self.subTest(source=changed):
                self.assertEqual([row['kind'] for row in self.inspect(changed, owner)],
                                 ['camera_exit_reopens_the_physical_scope'])
                self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + changed + ' }', owner))
                self.assertFalse(self.inspect(changed, 'crates/razer-pages/src/features/workspace/unrelated.rs'))
        self.assertFalse(self.inspect(source.replace('sync_camera_activity', 'unrelated'), owner))

    def test_image_decode_admission_and_cleanup_share_the_application_stop(self):
        owner = 'crates/razer-assets/src/image_cache.rs'
        for name, field in (('load', 'images'), ('load_filtered', 'filtered')):
            source = (f'impl ResourceImageCache {{ fn {name}() {{ '
                      'if let Err(error) = ensure_decoders_running(cx) { return Some(Err(error)); } '
                      f'self.{field}.get_mut(source); }} }}')
            self.assertFalse(self.inspect(source, owner))
            for changed in (source.replace('if let Err(error) = ensure_decoders_running(cx) { return Some(Err(error)); } ', ''),
                            source.replace('ensure_decoders_running(cx)', 'unrelated(cx)'),
                            source.replace(f'self.{field}.get_mut(source);', '').replace(
                                'if let Err(error)', f'self.{field}.get_mut(source); if let Err(error)')):
                self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'image_decode_bypasses_application_stop')
            self.assertFalse(self.inspect(source.replace('ResourceImageCache', 'Unrelated'), owner))
            self.assertFalse(self.inspect('#[cfg(test)] mod tests {' + source + '}', owner))
        clear = ('impl ResourceImageCache { fn clear() { discard_image_entries(&mut self.images, '
                 '|entry| entry.get(), |image| cx.drop_image(image,None)); self.clear_filtered(cx); } }')
        self.assertFalse(self.inspect(clear, owner))
        for changed in (clear.replace('&mut self.images', '&mut self.filtered'), clear.replace('self.clear_filtered(cx);', ''),
                        clear.replace('cx.drop_image(image,None)', 'drop(image)')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'image_cache_cleanup_leaves_normal_or_filtered_loads')
        stop = ('fn stop_resource_images() { if !cx.has_global::<WindowResourceCaches>() { WindowResourceCaches::install(cx); } '
                'cx.global_mut::<WindowResourceCaches>().lifecycle.stop(); '
                'for cache in caches { cache.update(cx,|cache,cx| cache.clear(cx)); } }')
        self.assertFalse(self.inspect(stop, owner))
        for changed in (stop.replace('WindowResourceCaches::install(cx);', ''),
                        stop.replace('cx.global_mut::<WindowResourceCaches>().lifecycle.stop();', ''),
                        stop.replace('cache.clear(cx)', 'cache.clear_filtered(cx)'),
                        stop.replace('cx.global_mut::<WindowResourceCaches>().lifecycle.stop();', '').replace(
                            'cache.update(cx,|cache,cx| cache.clear(cx));',
                            'cache.update(cx,|cache,cx| { cache.clear(cx); cx.global_mut::<WindowResourceCaches>().lifecycle.stop(); });')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'image_cache_stop_does_not_close_both_load_paths')
        self.assertFalse(self.inspect((workspace.ROOT / owner).read_text(encoding='utf-8'), owner))

    def test_displayed_image_slot_updates_content_before_using_its_asset(self):
        owner = 'crates/razer-assets/src/local_image.rs'
        source = ('fn preview_source() { let lease = window.use_keyed_state(id,cx,|_,cx| ImportedImageLease::new(source,cx)); '
                  'lease.update(cx,|lease,cx| lease.set_source(source.clone(),cx)); window.use_asset::<ImportedImageAsset>(&source,cx) }')
        self.assertFalse(self.inspect(source, owner))
        update = 'lease.update(cx,|lease,cx| lease.set_source(source.clone(),cx));'
        for changed in (source.replace(update, ''),
                        source.replace('lease.set_source', 'other.set_source'), source.replace('lease.set_source', 'lease.unrelated'),
                        source.replace(update, '').replace('window.use_asset::<ImportedImageAsset>(&source,cx)',
                            'window.use_asset::<ImportedImageAsset>(&source,cx); ' + update)):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'displayed_image_slot_keeps_the_previous_content_lease')
        self.assertFalse(self.inspect(source.replace('preview_source', 'unrelated'), owner))
        self.assertFalse(self.inspect('#[cfg(test)] mod tests {' + source + '}', owner))
        self.assertFalse(self.inspect((workspace.ROOT / owner).read_text(encoding='utf-8'), owner))

    def test_chroma_connect_session_is_owned_once_and_shared_by_the_shell(self):
        owner = 'crates/razer-shell/src/shell.rs'
        source = ('struct AppShell { thx_retired_owners: Vec<X>, keyboard_brightness: keyboard_brightness_runtime::State, receiver_chroma_connect: '
                  'std::sync::Arc<std::sync::Mutex<razer_service::chroma_connect::Session>> }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('receiver_chroma_connect:', 'another:'),
                        source.replace('std::sync::Mutex<razer_service::chroma_connect::Session>',
                                       'razer_service::chroma_connect::Session'),
                        source.replace('receiver_chroma_connect:', 'duplicate: razer_service::chroma_connect::Session, receiver_chroma_connect:'),
                        source.replace('razer_service::chroma_connect::Session', 'UnrelatedSession')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'chroma_connect_has_no_single_shared_shell_owner')
        aliases = ('use std::sync::{Arc as Shared, Mutex as Lock}; '
                   'use razer_service::chroma_connect::Session as LocalChroma; ')
        changed = source.replace('std::sync::Arc', 'Shared').replace('std::sync::Mutex', 'Lock') \
                        .replace('razer_service::chroma_connect::Session', 'LocalChroma')
        self.assertFalse(self.inspect(aliases + changed, owner))
        self.assertFalse(self.inspect(source.replace('AppShell', 'Unrelated'), owner))
        self.assertFalse(self.inspect(source.replace('receiver_chroma_connect:', 'another:'), 'crates/razer-pages/src/unrelated.rs'))
        self.assertFalse(self.inspect('#[cfg(test)] mod tests {' + source + '}', owner))
        self.assertFalse(self.inspect((workspace.ROOT / owner).read_text(encoding='utf-8'), owner))

    def test_device_workers_borrow_the_shared_chroma_session_instead_of_constructing_one(self):
        constructor = 'fn connect() { let session = razer_service::chroma_connect::Session::new(); }'
        borrowers = ('crates/razer-shell/src/shell/receiver/receiver_lighting_owner.rs',
                     'crates/razer-service/src/lighting/receiver_lighting_native.rs',
                     'crates/razer-pages/src/features/workspace/source_controls/receiver_brightness_ui.rs')
        alternatives = (constructor,
            'use razer_service::chroma_connect::Session as Chroma; fn connect() { Chroma::default(); }',
            'use razer_service::chroma_connect as local; fn connect() { local::Session::new(); }',
            'fn connect() { crate::chroma_connect::Session::new(); }')
        for owner in borrowers:
            for source in alternatives:
                self.assertEqual(self.inspect(source, owner)[0]['kind'], 'duplicate_chroma_connect_session_owner')
            self.assertFalse(self.inspect('fn connect() { shell.receiver_chroma_connect.clone(); }', owner))
            self.assertFalse(self.inspect('fn connect() { UnrelatedSession::new(); }', owner))
            self.assertFalse(self.inspect('#[cfg(test)] mod tests {' + constructor + '}', owner))
        for owner in ('crates/razer-shell/src/shell.rs', 'crates/razer-service/src/lighting/chroma_connect.rs'):
            self.assertFalse(self.inspect(constructor, owner))
        self.assertFalse(self.inspect(constructor, 'crates/razer-service/src/lighting/chroma_connect_tests.rs'))
        owner = borrowers[0]
        self.assertFalse(self.inspect((workspace.ROOT / owner).read_text(encoding='utf-8'), owner))

    def test_mouse_monitoring_protocol_is_owned_by_the_service_worker(self):
        source = ('use razer_ipc::ServiceRequest as Request; '
                  'fn poll() { client.request(Request::MouseInputMonitoringPoll { owner }); }')
        page = 'crates/razer-pages/src/features/mouse/dpi/mouse_matcher_owner.rs'
        shell = 'crates/razer-shell/src/shell/mouse/mouse_matcher.rs'
        for owner in (page, shell):
            self.assertEqual(self.inspect(source, owner)[0]['kind'], 'gui_owns_mouse_input_monitoring_protocol')
        self.assertFalse(self.inspect(source, 'crates/razer-service/src/mouse/dpi_matcher_worker.rs'))
        self.assertFalse(self.inspect('fn poll() { client.request(Request::OtherOperation); }', page))
        self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', page))

    def test_matcher_transport_finishes_before_releasing_the_native_and_device_leases(self):
        owner = 'crates/razer-service/src/mouse/dpi_matcher_worker.rs'
        source = ("impl NativeBackend<'_> { fn release_transport(&mut self) -> anyhow::Result<()> { "
                  'if let Some(client) = self.client.as_mut() { client.finish_termination()?; } '
                  'self.client.take(); self.device_task.take(); self.lifecycle.take(); Ok(()) } }')
        self.assertFalse(self.inspect(source,owner))
        for changed in (source.replace('client.finish_termination()?;', 'client.finish_termination();'),
                        source.replace('self.device_task.take();', ''),
                        source.replace('client.finish_termination()?;', '')
                              .replace('self.lifecycle.take();', 'self.lifecycle.take(); client.finish_termination()?;')):
            self.assertEqual(self.inspect(changed,owner)[0]['kind'],
                             'matcher_releases_leases_before_transport_termination')
        self.assertFalse(self.inspect(source.replace('NativeBackend', 'Unrelated'),owner))
        self.assertFalse(self.inspect(source,'crates/razer-service/src/unrelated.rs'))

    def test_matcher_completion_publishes_joined_cleanup_and_release_results(self):
        owner = 'crates/razer-service/src/mouse/dpi_matcher_worker.rs'
        source = ('impl Handle { fn finish(mut self) { let joined = self.worker.take().unwrap().join(); '
                  'match joined { Ok(exit) => { self.completion.publish(&exit.cleanup, &exit.released); '
                  'merge_results([exit.operation,exit.cleanup,exit.released]) }, '
                  'Err(error) => { let result = Err(error); self.completion.publish(&result); result } } } }')
        self.assertFalse(self.inspect(source,owner))
        for changed in (source.replace('&exit.cleanup, &exit.released','&exit.released'),
                        source.replace('&exit.cleanup, &exit.released','&exit.cleanup'),
                        source.replace('&exit.cleanup,','&exit.operation,'),
                        source.replace('&exit.released','&exit.operation'),
                        source.replace('.join()',''),
                        source.replace('let joined =','self.completion.publish(&exit.cleanup, &exit.released); let joined =')
                              .replace('Ok(exit) => { self.completion.publish(&exit.cleanup, &exit.released);','Ok(exit) => {')):
            self.assertEqual(self.inspect(changed,owner)[0]['kind'],
                             'matcher_completion_does_not_publish_joined_release')
        self.assertFalse(self.inspect(source.replace('impl Handle','impl Unrelated'),owner))
        self.assertFalse(self.inspect(source,'crates/razer-service/src/unrelated.rs'))

    def test_matcher_save_requires_complete_workspace_scope_session_and_connection(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_matcher.rs'
        source = ('fn mouse_matcher_save_current(&self, save: &LocalSave, cx: &App) -> bool { '
                  'self.mouse_matcher.sessions.get(&save.owner.identity).is_some_and(|session| { '
                  'session.owner.id == save.owner.id && session.owner.scope == save.owner.scope '
                  '&& session.owner.workspace.upgrade().as_ref() == Some(&save.workspace) '
                  '&& save.owner.current(self, session.closing, cx) }) }')
        self.assertFalse(self.inspect(source, owner))
        changes = ('session.owner.id == save.owner.id',
                   'session.owner.scope == save.owner.scope',
                   'session.owner.workspace.upgrade().as_ref() == Some(&save.workspace)',
                   'save.owner.current(self, session.closing, cx)')
        for condition in changes:
            with self.subTest(condition=condition):
                self.assertEqual(self.inspect(source.replace(condition, 'true'), owner)[0]['kind'],
                                 'matcher_save_accepts_stale_session_or_connection')
        self.assertEqual(self.inspect(source.replace('&&', '||', 1), owner)[0]['kind'],
                         'matcher_save_accepts_stale_session_or_connection')
        self.assertFalse(self.inspect(source, 'crates/razer-shell/src/shell/unrelated.rs'))

    def test_retiring_matcher_delivery_cannot_remove_the_replacement_session(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_matcher.rs'
        source = ('fn dispatch_mouse_matcher() { if shell.mouse_matcher.sessions.get(&delivery_owner.identity) '
                  '.is_some_and(|session| session.owner.id == delivery_owner.id) { '
                  'shell.retire_mouse_matcher(&delivery_owner.identity, cx); } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('session.owner.id == delivery_owner.id', 'true'),
                        source.replace('.get(&delivery_owner.identity)', '.get(&other_identity)'),
                        'fn dispatch_mouse_matcher() { shell.retire_mouse_matcher(&delivery_owner.identity, cx); }'):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'matcher_retired_worker_removes_successor')
        self.assertFalse(self.inspect('fn unrelated() { shell.retire_mouse_matcher(&retained_identity, cx); }', owner))

    def test_matcher_exit_drain_keeps_the_exact_profile_scope_and_physical_route(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_matcher.rs'
        conditions = [
            'shell.devices.contains(&workspace)',
            'workspace_state.identity(cx) == self.identity',
            'workspace_state.connection_revision() == self.connection_revision',
            'workspace_state.mouse_polling_scope(cx) == Some(self.domain_scope)',
            'device.active_profile == self.profile_id',
            'device.profiles.iter().any(|profile| profile.id == self.profile_id && profile.guid == self.profile_guid)',
            'workspace_state.retains_mouse_matcher_scope(&self.scope, cx)',
            '(draining || self.scope.is_valid() && workspace_state.mouse_matcher_scope(cx).as_ref() == Some(&self.scope))',
            'super::device_discovery::retained_connection(std::slice::from_ref(&self.route), &shell.device_observations, device)',
        ]
        source = 'impl Owner { fn current() -> bool { ' + ' && '.join(conditions) + ' } }'
        self.assertFalse(self.inspect(source, owner))
        for condition in conditions:
            with self.subTest(condition=condition):
                self.assertEqual(self.inspect(source.replace(condition, 'true'), owner)[0]['kind'],
                                 'matcher_drain_bypasses_profile_or_connection_owner')
        self.assertEqual(self.inspect(source.replace('self.route', 'other_route'), owner)[0]['kind'],
                         'matcher_drain_bypasses_profile_or_connection_owner')
        self.assertFalse(self.inspect(source.replace('impl Owner', 'impl Unrelated'), owner))
        self.assertFalse(self.inspect(source, 'crates/razer-shell/src/shell/unrelated.rs'))

    def test_receiver_and_matcher_rebase_validate_owner_before_applying_the_mutation(self):
        rows = [('crates/razer-shell/src/shell/mouse/mouse_matcher.rs',
                 'rebase_mouse_matcher_save', 'mouse_matcher_save_current', 'mutation.apply(target)?;'),
                ('crates/razer-shell/src/shell/receiver/receiver_profile_save.rs',
                 'rebase_receiver_profile_save', 'receiver_profile_current', '*target = snapshot.project(target)?;')]
        for owner, name, check, mutation in rows:
            validation = 'ensure!(self.' + check + '(owner, cx), "owner"); '
            source = 'fn ' + name + '() { ' + validation + mutation + ' }'
            self.assertFalse(self.inspect(source, owner))
            for changed in (source.replace(validation, ''),
                            'fn ' + name + '() { ' + mutation + validation + ' }'):
                self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'product_profile_rebase_skips_current_owner')

    def test_existing_capability_directory_is_stable_when_root_transport_is_organized(self):
        root = Path("crates/razer-device/src/backend.rs")
        capability = Path("crates/razer-device/src/protocol/onboard_macro.rs")
        self.assertEqual(workspace.source_layout_destination(root), Path("crates/razer-device/src/transport/backend.rs"))
        self.assertEqual(workspace.source_layout_destination(capability), capability)

    def test_gui_filesystem_calls_are_rejected_across_pages_settings_and_shell(self):
        owners = ["crates/razer-pages/src/features/profiles/linked_games.rs",
                  "crates/razer-settings/src/runtime_page/diagnostics.rs",
                  "crates/razer-shell/src/shell/audio/demo_recording.rs",
                  "crates/razer-app-pages/src/profiles/profiles_page/transfer.rs",
                  "crates/razer-dashboard/src/dashboard.rs", "crates/razer-tray/src/lib.rs",
                  "crates/razer-widgets/src/image.rs"]
        for owner in owners:
            for source in ['fn read() { std::fs::read(path); }',
                           'use std::{fs as disk}; fn read() { disk::read(path); }',
                           'use std::fs::File as Input; fn read() { Input::open(path); }',
                           'use std::fs::read as load_file; fn read() { load_file(path); }']:
                with self.subTest(owner=owner, source=source):
                    rows = self.inspect(source, owner)
                    self.assertIn('gui_owns_filesystem_io', [row['kind'] for row in rows])
                    row = next(row for row in rows if row['kind'] == 'gui_owns_filesystem_io')
                    self.assertEqual(row['expected_owner'], 'crates/razer-storage/src/file_transfer.rs')

    def test_gui_path_observation_follows_typed_arguments_and_local_bindings(self):
        owner = "crates/razer-pages/src/features/workspace/source_workspace/profile_linked_games.rs"
        for source in ['use std::path::Path; fn inspect(path:&Path) { path.is_file(); }',
                       'use std::path::PathBuf as LocalPath; fn inspect(path:LocalPath) { path.canonicalize(); }',
                       'use std::path::Path; fn inspect(path:&Path) { let canonical=path.canonicalize()?; canonical.is_file(); }',
                       'fn inspect() { std::path::Path::new("file").metadata(); }']:
            with self.subTest(source=source):
                rows = self.inspect(source, owner)
                self.assertTrue(rows)
                self.assertTrue(all(row['kind'] == 'gui_owns_path_observation' for row in rows))
        self.assertFalse(self.inspect('fn inspect() { self.metadata(); catalog.is_file(); }', owner))
        self.assertFalse(self.inspect('fn inspect() { let text="std::fs::read(path)"; }', owner))

    def test_backend_filesystem_owners_and_real_test_boundaries_remain_legal(self):
        source = 'use std::fs::File; fn read() { File::open(path); }'
        for owner in ('crates/razer-storage/src/file_transfer.rs',
                      'crates/razer-platform/src/observation/executable.rs',
                      'crates/razer-platform/src/observation/discovery_diagnostics.rs',
                      'crates/razer-resources/src/cache.rs', 'crates/razer-assets/src/local_image.rs'):
            self.assertFalse(self.inspect(source, owner))
        owner = 'crates/razer-shell/src/shell/audio/demo_recording.rs'
        self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', owner))
        self.assertFalse(self.inspect('#[test] fn fixture() { std::fs::write(path,bytes); }', owner))
        self.assertFalse(self.inspect(source, 'crates/razer-pages/src/features/profiles/linked_games_tests.rs'))
        self.assertTrue(self.inspect('// #[cfg(test)]\n' + source, owner))

    def test_mapping_product_identity_is_owned_by_the_domain_document(self):
        owner = "crates/razer-device/src/mapping/mapping_document.rs"
        self.assertFalse(self.inspect('pub struct MappingProduct(u32);', owner))
        for consumer in ("crates/razer-pages/src/features/workspace/source_workspace.rs",
                         "crates/razer-shell/src/shell.rs",
                         "crates/razer-service/src/runtime/windows/mapping_device.rs"):
            bad = self.inspect('pub struct MappingProduct(u32);', consumer)
            self.assertEqual([row["kind"] for row in bad], ["duplicate_mapping_product_owner"])
            self.assertEqual(bad[0]["expected_owner"], owner)
            self.assertFalse(self.inspect('fn document(id:u32) { MappingProduct::for_product_id(id); }', consumer))
            self.assertFalse(self.inspect('struct ProductPreview(u32);', consumer))
            for source, kind in [('fn enabled(id:u32)->bool { match id {70|515|679|740|746|756=>true,_=>false} }', 'duplicate_mapping_product_gate'),
                                 ('const FIELD:&str="_mouse70_mapping_document";', 'duplicate_mapping_document_storage_identity')]:
                self.assertEqual([row['kind'] for row in self.inspect(source, consumer)], [kind])
                self.assertFalse(self.inspect(source, owner))
            self.assertFalse(self.inspect('fn another(id:u32)->bool { match id {740|746=>true,_=>false} }', consumer))

    def test_mapping_document_compilation_has_one_shared_owner(self):
        owner = 'crates/razer-model/src/mapping.rs'
        consumers = (
            'crates/razer-pages/src/features/workspace/source_workspace/mapping_reconnect.rs',
            'crates/razer-shell/src/shell/mouse/mouse_dpi_70.rs',
            'crates/razer-shell/src/shell/keyboard/keyboard_mapping.rs',
        )
        for module, method in (
                ('mouse_legacy_mapping', 'prepare_document'),
                ('mouse_receiver_mapping', 'prepare_document_with_device_settings'),
                ('keyboard_legacy_mapping', 'prepare_document'),
                ('keyboard_current_mapping', 'prepare_document'),
                ('keyboard_magnetic_mapping', 'prepare_document'),
                ('keyboard_latest_mapping', 'prepare_document_with_device_settings')):
            source = f'use razer_device::{module} as compiler; fn compile() {{ compiler::{method}(profiles); }}'
            self.assertFalse(self.inspect(source, owner))
            for consumer in consumers:
                violations = self.inspect(source, consumer)
                self.assertEqual([row['kind'] for row in violations], ['gui_duplicates_mapping_document_compiler'])
                self.assertEqual(violations[0]['expected_owner'], owner)
                self.assertFalse(self.inspect('fn compile() { razer_model::mapping::prepare_document(device,macros,ids); }', consumer))
                self.assertFalse(self.inspect('fn unrelated() { another::prepare_document(profiles); }', consumer))
                self.assertFalse(self.inspect('#[cfg(test)] mod tests { ' + source + ' }', consumer))

    def test_product_factory_is_rejected_through_a_type_alias_but_unrelated_constructors_pass(self):
        owner = "crates/razer-shell/src/shell/gamepad/controller_chroma.rs"
        bad = self.inspect('use razer_storage::workspace::WorkspaceFile as File; fn save() { File::new(devices, false); }', owner)
        self.assertEqual([row["kind"] for row in bad], ["product_recomposes_workspace"])
        self.assertEqual(bad[0]["expected_owner"], "crates/razer-shell/src/shell/persistence.rs")
        aliases = self.inspect('type Alias = razer_storage::workspace::WorkspaceFile; type File = Alias; fn save() { File::new(devices, false); }', owner)
        self.assertEqual([row["kind"] for row in aliases], ["product_recomposes_workspace"])
        self.assertFalse(self.inspect('fn unrelated() { FilePicker::new(); }', owner))

    def test_shared_snapshot_requires_host_order_and_all_other_common_fields(self):
        owner = "crates/razer-shell/src/shell/persistence.rs"
        source = 'fn workspace_file() { WorkspaceFile::new(devices,false).with_shortcuts(s).with_retired_shortcuts(r).with_preferences(p).with_custom_colors(c).with_host_tab_order(o).with_dashboard(d).with_module_services(m).with_macros(l); }'
        self.assertFalse(self.inspect(source, owner))
        for missing in ('.with_host_tab_order(o)', '.with_retired_shortcuts(r)'):
            bad = self.inspect(source.replace(missing, ''), owner)
            self.assertEqual(bad[0]["kind"], "incomplete_shared_workspace_snapshot")

    def test_device_acknowledgement_cannot_zip_a_changed_list(self):
        bad = self.inspect('fn finished() { for (owner,device) in self.devices.iter().zip(devices) {} }', "crates/razer-shell/src/shell.rs")
        self.assertEqual(bad[0]["kind"], "positional_device_acknowledgement")
        self.assertFalse(self.inspect('fn unrelated() { pairs.iter().zip(values); }', "crates/razer-shell/src/shell.rs"))
        self.assertFalse(self.inspect('fn unrelated() { self.devices_catalog.iter().zip(values); pairs.map(|_| ".devices").zip(values); }', "crates/razer-shell/src/shell.rs"))

    def test_persisted_contract_has_one_owner_and_test_fixtures_remain_legal(self):
        self.assertFalse(self.inspect('pub struct AppPreferences { flag: bool }', "crates/razer-model/src/preferences.rs"))
        bad = self.inspect('pub struct AppPreferences { flag: bool }', "crates/razer-pages/src/features/settings.rs")
        self.assertEqual(bad[0]["kind"], "duplicate_persisted_contract")
        self.assertFalse(self.inspect('#[cfg(test)] mod tests { struct AppPreferences; }', "crates/razer-pages/src/lib.rs"))
        source = 'pub struct RetiredShortcuts(Vec<Shortcut>);'
        self.assertFalse(self.inspect(source, "crates/razer-storage/src/workspace/retired_shortcuts.rs"))
        self.assertEqual(self.inspect(source, "crates/razer-shell/src/shell.rs")[0]["kind"], "duplicate_persisted_contract")

    def test_resource_sessions_are_not_process_globals_and_catalogs_remain_legal(self):
        owner = "crates/razer-resources/src/runtime.rs"
        bad = self.inspect('static SERVICE: OnceLock<Arc<Service>> = OnceLock::new();', owner)
        self.assertEqual(bad[0]["kind"], "process_global_resource_session")
        self.assertFalse(self.inspect('static CATALOG: OnceLock<Manifest> = OnceLock::new();', owner))
        self.assertEqual(self.inspect('fn load() { razer_resources::inspect_cached(path); }', "crates/razer-pages/src/lib.rs")[0]["kind"], "blocking_cache_inspection_in_product")
        self.assertFalse(self.inspect('#[cfg(test)] mod tests { fn load() { razer_resources::inspect_cached(path); } }', "crates/razer-pages/src/lib.rs"))

    def test_resource_publication_has_one_owner_without_restricting_unrelated_types(self):
        canonical = "crates/razer-resource-db/src/publication.rs"
        self.assertFalse(self.inspect('pub struct StagedResource { temporary: Option<PathBuf> } impl StagedResource { fn publish(self) {} }', canonical))
        for owner in ("crates/razer-resources/src/cache.rs", "crates/razer-pages/src/features/product.rs"):
            bad = self.inspect('struct StagedResource { destination: PathBuf }', owner)
            self.assertEqual(bad[0]["kind"], "duplicate_resource_publication_owner")
            self.assertEqual(bad[0]["expected_owner"], canonical)
            bad = self.inspect('use razer_resource_db::StagedResource as Pending; impl Pending { fn replace() {} }', owner)
            self.assertEqual(bad[0]["kind"], "duplicate_resource_publication_owner")
            self.assertFalse(self.inspect('struct StagedUpload; impl StagedUpload { fn publish(self) {} }', owner))
        bad = self.inspect('fn atomic_replace() {}', "crates/razer-resources/src/cache.rs")
        self.assertEqual(bad[0]["kind"], "duplicate_resource_publication_owner")
        self.assertFalse(self.inspect('fn atomic_replace() {}', "crates/razer-storage/src/workspace.rs"))

    def test_catalog_and_media_publication_require_shared_staging_and_its_publish(self):
        for owner, kind, name in (("crates/razer-resource-db/src/installation.rs", "InstallationPaths", "publish_catalog"),
                                  ("crates/razer-resources/src/cache.rs", "Cache", "publish")):
            source = ('use razer_resource_db::StagedResource as Pending; impl ' + kind + ' { fn ' + name +
                      '() { let staged = Pending::write(path, bytes).context("stage")?; staged.publish()?; } }')
            self.assertFalse(self.inspect(source, owner))
            for invalid in (source.replace('staged.publish()', 'other.publish()'),
                            source.replace('Pending::write', 'Other::write'),
                            'impl ' + kind + ' { fn ' + name + '() { fs::rename(temporary, target); } }'):
                bad = self.inspect(invalid, owner)
                self.assertEqual(bad[0]["kind"], "resource_publication_bypasses_shared_owner")
            self.assertFalse(self.inspect('fn unrelated() { Other::write(path, bytes); other.publish(); }', owner))

    def test_cargo_catalog_copy_uses_the_same_publication_owner(self):
        owner = "crates/razer-resource-db/build.rs"
        good = 'fn stage() { let staged = publication::StagedResource::write(path, bytes).expect("stage"); staged.publish(); }'
        self.assertFalse(self.inspect(good, owner))
        self.assertEqual(self.inspect('fn stage() { fs::write(path, bytes); }', owner)[0]["kind"],
                         "resource_publication_bypasses_shared_owner")
        self.assertEqual(self.inspect('fn atomic_replace() {}', owner)[0]["kind"], "duplicate_resource_publication_owner")
        self.assertFalse(self.inspect('#[path="src/publication.rs"] mod publication;', owner))

    def test_held_interdevice_state_has_one_typed_lifecycle_owner(self):
        canonical = "crates/razer-model/src/mapping/interdevice.rs"
        self.assertFalse(self.inspect('pub struct HeldInterdeviceTarget<O> { target: Option<O> }', canonical))
        owner = "crates/razer-shell/src/shell/devices/product_mapping.rs"
        bad = self.inspect('struct TargetObservation<O> { owner: O }', owner)
        self.assertEqual(bad[0]["kind"], "duplicate_interdevice_lifecycle_owner")
        bad = self.inspect('use razer_model::mapping::interdevice::HeldInterdeviceTarget as Held; impl<O> Held<O> { fn observe() {} }', owner)
        self.assertEqual(bad[0]["kind"], "duplicate_interdevice_lifecycle_owner")
        self.assertFalse(self.inspect('struct TargetPreview; impl TargetPreview { fn observe() {} }', owner))
        good = 'struct InterdeviceHold { origin: MappingTargetOwner, product_id: u32, raw: Value, target: HeldInterdeviceTarget<MappingTargetOwner> }'
        self.assertFalse(self.inspect(good, owner))
        self.assertEqual(self.inspect(good.replace('HeldInterdeviceTarget<MappingTargetOwner>', 'Option<MappingTargetOwner>'), owner)[0]["kind"],
                         "interdevice_hold_bypasses_lifecycle_owner")

    def test_mapping_refresh_replays_only_lifecycle_transitions_and_scopes_connection_revision(self):
        owner = "crates/razer-shell/src/shell/devices/product_mapping.rs"
        self.assertFalse(self.inspect('fn sync_local_mapping_targets() { self.sync_interdevice_holds(cx); }', owner))
        self.assertEqual(self.inspect('fn sync_local_mapping_targets() { target.consume_interdevice_mapping(raw); }', owner)[0]["kind"],
                         "ui_refresh_replays_held_interdevice_mapping")
        good = 'fn sync_interdevice_holds() { hold.origin.matches(workspace); let change = hold.target.observe(self.first_mapping_target(id)); if let Some(owner) = change.released() { release_target(owner); } if let Some(owner) = change.replayed() { dispatch_to_target(owner, raw); } }'
        self.assertFalse(self.inspect(good, owner))
        bad = self.inspect(good.replace('if let Some(owner) = change.replayed() { dispatch_to_target(owner, raw); }',
                                       'let owner = change.replayed(); dispatch_to_target(owner, raw);'), owner)
        self.assertEqual(bad[0]["kind"], "held_interdevice_replay_without_transition")
        good = 'impl MappingTargetOwner { fn matches() { self.workspace == other && self.identity == target.identity(cx) && self.connection_revision == target.connection_revision() } }'
        self.assertFalse(self.inspect(good, owner))
        self.assertEqual(self.inspect(good.replace(' && self.connection_revision == target.connection_revision()', ''), owner)[0]["kind"],
                         "interdevice_owner_ignores_connection_revision")
        self.assertFalse(self.inspect('impl UnrelatedOwner { fn matches() { true } }', owner))

    def test_shortcut_encoding_uses_the_domain_owner_and_shared_input_and_hash(self):
        owner = "crates/razer-model/src/shortcuts/native.rs"
        good = ('use razer_device::mapping_engine::MappingEngine as Engine; '
                'use razer_device::mapping_engine_hash::{canonical_json as canonical, source_hash as digest}; '
                'fn encode_shortcuts() { canonical(&down); digest(&engine); } '
                'fn encode_input() { Engine::global_shortcuts().encode_shortcut_input(key,mods,hs).map(|input|input.into_parts()); }')
        self.assertFalse(self.inspect(good, owner))
        self.assertEqual(self.inspect(good.replace('Engine::global_shortcuts()', 'Engine::new(70)'), owner)[0]["kind"],
                         "shortcut_input_bypasses_shared_engine")
        self.assertEqual(self.inspect(good.replace('digest(&engine)', 'local_digest(&engine)'), owner)[0]["kind"],
                         "shortcut_hash_bypasses_shared_owner")
        page = "crates/razer-pages/src/features/shortcuts/shortcuts.rs"
        self.assertEqual(self.inspect('fn encode_shortcuts() {}', page)[0]["kind"], "duplicate_native_shortcut_encoder")
        self.assertEqual(self.inspect('fn stable_json() {}', page)[0]["kind"], "duplicate_shortcut_hash_encoder")
        self.assertEqual(self.inspect('fn input_modifier_bit() {}', page)[0]["kind"], "duplicate_mapping_modifier_encoder")
        self.assertFalse(self.inspect('struct Example; impl Example { fn encode_shortcuts() {} fn stable_json() {} fn input_modifier_bit() {} }', page))
        self.assertFalse(self.inspect('fn stable_json() {}', "crates/razer-storage/src/unrelated.rs"))

    def test_shortcut_key_data_cannot_be_reintroduced_in_page_or_domain_encoder(self):
        source = 'const KEYS: &[(&str, u32)] = &[("KEY_ESCAPE", 1), ("KEY_A", 30)];'
        for owner in ("crates/razer-pages/src/features/shortcuts/keys.rs", "crates/razer-model/src/shortcuts/native.rs"):
            self.assertEqual(self.inspect(source, owner)[0]["kind"], "duplicate_shortcut_key_table")
        self.assertFalse(self.inspect('const LABELS: &[(&str, &str)] = &[("KEY_ESCAPE", "Escape")];',
                                      "crates/razer-model/src/shortcuts/native.rs"))
        self.assertFalse(self.inspect(source, "crates/razer-device/src/mapping/mapping_engine_data_generated.rs"))

    def test_retired_page_worker_and_encoder_paths_fail_even_when_empty(self):
        for name in ("mapping/mapping_device_runtime.rs", "mapping/mapping_key_capture_runtime.rs",
                     "shortcuts/shortcut_engine.rs", "shortcuts/shortcut_engine_keys.rs"):
            self.assertEqual(self.inspect('', "crates/razer-pages/src/features/" + name)[0]["kind"],
                             "retired_architecture_source")

    def test_mapping_worker_io_and_accounting_are_service_owned(self):
        owner = "crates/razer-pages/src/features/mouse/mapping/mouse_mapping_runtime_70.rs"
        for source in ('fn start() { ServiceClient::spawn(); }',
                       'use razer_ipc::ServiceClient as Native; fn start() { Native::spawn(); }',
                       'use std::thread as worker; fn start() { worker::Builder::new(); }',
                       'use std::thread::Builder as Worker; fn start() { Worker::new(); }',
                       'use std::{thread::{self,Builder},sync::mpsc}; fn start() { Builder::new(); }',
                       'fn start() { std::thread::Builder::new(); }'):
            self.assertEqual(self.inspect(source, owner)[0]["kind"], "page_owns_mapping_native_worker")
        for source in ('struct WorkerGuard;', 'static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);'):
            self.assertEqual(self.inspect(source, owner)[0]["kind"], "duplicate_mapping_worker_owner")
        service = "crates/razer-service/src/mapping/"
        self.assertFalse(self.inspect('struct WorkerGuard; static ACTIVE_WORKERS: usize = 0;', service + "mod.rs"))
        self.assertFalse(self.inspect('use ui::Builder; fn render() { Builder::new(); }', owner))
        self.assertEqual(self.inspect('use super::mapping_device_runtime::Session;', owner)[0]["kind"], "retired_mapping_worker_module")
        self.assertEqual(self.inspect('mod mapping_key_capture_runtime {}', owner)[0]["kind"], "retired_mapping_worker_module")
        for filename in ("device.rs", "key_capture.rs"):
            good = 'use super::WorkerGuard as Guard; use razer_ipc::KEY_CAPTURE_LIFECYCLE as Lifecycle; impl Session { fn start_with() { let guard = Guard::start(); let capture = Lifecycle.lock(); thread::Builder::new(); } }'
            self.assertFalse(self.inspect(good, service + filename))
            self.assertEqual(self.inspect(good.replace('Guard::start()', '()'), service + filename)[0]["kind"],
                             "mapping_worker_bypasses_cleanup_accounting")
            self.assertFalse(self.inspect('impl Unrelated { fn start_with() {} }', service + filename))
        shell = "crates/razer-shell/src/shell.rs"
        self.assertFalse(self.inspect('fn wait() { razer_service::mapping::active_workers(); }', shell))
        self.assertEqual(self.inspect('fn wait() { mapping_device_runtime::active_workers(); }', shell)[0]["kind"],
                         "shell_counts_mapping_workers_outside_service")

    def test_seiren_audio_workers_and_protocol_are_service_owned(self):
        owners = ["crates/razer-shell/src/shell/audio/seiren_audio_capture.rs",
                  "crates/razer-shell/src/shell/audio/seiren_audio_playback.rs",
                  "crates/razer-pages/src/features/audio/seiren/audio_seiren_microphone.rs"]
        service = "crates/razer-service/src/audio/seiren/capture.rs"
        cases = [
            'fn start() { razer_ipc::ServiceClient::spawn(); }',
            'use razer_ipc::ServiceClient as Native; fn start() { Native::spawn(); }',
            'use std::{thread::{self,Builder as Native}}; fn start() { Native::new(); }',
            'use std::thread as native; fn start() { native::spawn(run); }',
            'use razer_storage::audio_recording::RecordingFile as File; fn start() { File::write(samples,canceled); }',
        ]
        for owner in owners:
            for source in cases:
                self.assertEqual(self.inspect(source, owner)[0]["kind"], "gui_owns_seiren_audio_io")
                self.assertFalse(self.inspect(source, service))
            source = 'use razer_ipc::ServiceRequest as Request; fn start() { dispatch(Request::SeirenAudioCaptureStart { session_id: 1 }); }'
            self.assertEqual(self.inspect(source, owner)[0]["kind"], "gui_owns_seiren_audio_protocol")
            self.assertFalse(self.inspect(source, service))
            self.assertFalse(self.inspect('use ui::Builder; fn render() { Builder::new(); }', owner))
            self.assertFalse(self.inspect('use razer_service::seiren_audio::capture; fn start() { capture::spawn(request); }', owner))
            self.assertFalse(self.inspect('#[cfg(test)] fn simulated() { razer_ipc::ServiceClient::spawn(); }', owner))
            self.assertEqual(self.inspect('struct WorkerGuard;', owner)[0]["kind"], "duplicate_seiren_audio_worker_owner")
        for owner in owners + [service]:
            raw = (workspace.ROOT / owner).read_bytes()
            self.assertFalse(workspace.inspect_architecture_source(raw, owner=owner, rust_parser=self.rust_parser))

    def test_held_release_precedes_worker_stop_and_connection_reset(self):
        shell = "crates/razer-shell/src/shell.rs"
        good = 'impl AppShell { fn request_exit() { self.close_requested = true; page.cancel_transfer(cx); razer_state::ui_preferences::begin_exit(cx); self.release_interdevice_holds(cx); shortcuts.dismiss_for_navigation(cx); workspace.prepare_exit(cx); runtime.prepare_exit(cx); workspace.prepare_legacy_dpi_exit(cx); } }'
        self.assertFalse(self.inspect(good, shell))
        self.assertFalse(self.inspect(good.replace('self.close_requested = true;', '').replace('runtime.prepare_exit(cx);', 'runtime.prepare_exit(cx); self.close_requested = true;'), shell))
        for bad in (good.replace('self.close_requested = true;', ''),
                    good.replace('self.release_interdevice_holds(cx);', ''),
                    good.replace('runtime.prepare_exit(cx);', '').replace('self.close_requested = true;', 'runtime.prepare_exit(cx); self.close_requested = true;')):
            self.assertEqual(self.inspect(bad, shell)[0]["kind"], "interdevice_release_after_worker_stop")
        discovery = "crates/razer-shell/src/shell/devices/device_discovery.rs"
        for name in ("observe_discovery", "publish_receiver_query"):
            good = 'fn ' + name + '() { self.retain_interdevice_connections(&retained,cx); workspace.observe_connection(None,cx); }'
            self.assertFalse(self.inspect(good, discovery))
            self.assertEqual(self.inspect(good.replace('self.retain_interdevice_connections(&retained,cx);', ''), discovery)[0]["kind"],
                             "interdevice_connection_reset_before_release")
            self.assertEqual(self.inspect(good.replace('self.retain_interdevice_connections(&retained,cx);', '').replace('workspace.observe_connection(None,cx);',
                'workspace.observe_connection(None,cx); self.retain_interdevice_connections(&retained,cx);'), discovery)[0]["kind"],
                "interdevice_connection_reset_before_release")
        adapter = "crates/razer-shell/src/shell/devices/product_mapping.rs"
        good = 'fn route_mapping_callback() { if self.close_requested { return; } dispatch_to_target(owner,raw,cx); }'
        self.assertFalse(self.inspect(good, adapter))
        self.assertEqual(self.inspect(good.replace('if self.close_requested { return; }', ''), adapter)[0]["kind"],
                         "interdevice_callback_accepted_during_exit")
        self.assertEqual(self.inspect('fn release_interdevice_holds() { self.mapping_interdevice_holds.clear(); }', adapter)[0]["kind"],
                         "interdevice_exit_skips_held_release")
        self.assertEqual(self.inspect('fn retain_interdevice_connections() { self.mapping_interdevice_holds.clear(); }', adapter)[0]["kind"],
                         "interdevice_disconnect_skips_held_release")

    def test_macro_transfer_retains_task_identity_and_cancels_on_drop(self):
        page = "crates/razer-app-pages/src/macros/macro_page.rs"
        owner = "crates/razer-app-pages/src/macros/macro_page/transfer.rs"
        self.assertFalse(self.inspect('struct MacroPage { transfer_task: Option<PendingTransfer> }', page))
        self.assertEqual(self.inspect('struct MacroPage { transfer_busy: bool }', page)[0]["kind"], "macro_transfer_has_no_page_task_owner")
        self.assertFalse(self.inspect('struct PendingTransfer { request: Transfer, _task: Task<()> } impl Drop for PendingTransfer { fn drop(&mut self) { self.request.cancel(); } }', owner))
        self.assertEqual(self.inspect('impl Drop for PendingTransfer { fn drop(&mut self) {} }', owner)[0]["kind"], "macro_transfer_drop_skips_cancellation")
        for method in ("import_xml", "export_xml"):
            good = 'fn ' + method + '() { if pending.request.is_same(&current) { self.transfer_task = Some(task); } }'
            self.assertFalse(self.inspect(good, owner))
            self.assertEqual(self.inspect(good.replace('pending.request.is_same(&current)', 'true'), owner)[0]["kind"],
                             "macro_transfer_accepts_unowned_completion")
            self.assertEqual(self.inspect(good.replace('self.transfer_task = Some(task);', 'task.detach();'), owner)[0]["kind"],
                             "macro_transfer_accepts_unowned_completion")
        for source in ('fn read() { std::fs::read(path); }', 'fn parse() { roxmltree::Document::parse(text); }'):
            self.assertEqual(self.inspect(source, owner)[0]["kind"], "page_owns_macro_xml_io_or_codec")
        self.assertFalse(self.inspect('fn read() { request.read_xml(path); razer_platform::macro_geometry::snapshot(); }', owner))
        self.assertEqual(self.inspect('', "crates/razer-app-pages/src/macros/macro_page/transfer/xml.rs")[0]["kind"],
                         "retired_architecture_source")

    def test_profile_decoder_hashing_has_one_source_owner(self):
        owner = "crates/razer-device/src/profile_transfer.rs"
        good = 'use razer_device::mapping_engine_hash::source_hash as digest; fn entry() { digest(&value); }'
        self.assertFalse(self.inspect(good, owner))
        self.assertEqual(self.inspect('fn entry() { local_hash(&value); }', owner)[0]["kind"], "profile_hash_bypasses_shared_owner")
        self.assertEqual(self.inspect('fn stable_json() {}', owner)[0]["kind"], "duplicate_profile_hash_encoder")
        self.assertEqual(self.inspect('fn compute() { md5::compute(text); }', owner)[0]["kind"], "profile_hash_bypasses_shared_owner")
        self.assertFalse(self.inspect('impl Preview { fn entry() {} fn stable_json() {} }', owner))

    def test_macro_recorder_codec_transport_and_tasks_have_explicit_owners(self):
        page = "crates/razer-app-pages/src/macros/macro_page"
        for name in ("recording_decode.rs", "recording_actor.rs"):
            self.assertEqual(self.inspect('', page + "/" + name)[0]["kind"], "retired_architecture_source")
        for name in ("recording_decode", "recording_actor"):
            self.assertEqual(self.inspect('mod ' + name + ';', page + ".rs")[0]["kind"], "retired_macro_recording_module")
        self.assertFalse(self.inspect('use razer_model::macro_library::recording as recording_decode; use razer_service::mapping::recorder as recording_actor;', page + ".rs"))
        self.assertEqual(self.inspect('struct Decoder;', page + "/recording.rs")[0]["kind"], "page_owns_macro_recording_decoder")
        self.assertFalse(self.inspect('pub struct Decoder;', "crates/razer-model/src/macro_library/recording.rs"))
        self.assertEqual(self.inspect('fn parse_delay() {}', page + "/record_options.rs")[0]["kind"], "page_owns_macro_delay_semantics")
        self.assertFalse(self.inspect('impl Other { fn parse_delay() {} }', page + "/record_options.rs"))
        fields = 'command: Session, countdown_task: Task<()>, updates_task: Task<()>, generation: u64, document: u64, cancelled: bool'
        self.assertFalse(self.inspect('struct RecordingUi {' + fields + '}', page + "/recording.rs"))
        self.assertEqual(self.inspect('struct RecordingUi { command: Session }', page + "/recording.rs")[0]["kind"], "macro_recording_has_no_page_session_owner")
        good = 'fn start_recording() { self.recording.command = Some(session); self.recording.updates_task = Some(cx.spawn(work)); if self.generation == generation && self.document == self.current && !self.cancelled { commit(); } }'
        self.assertFalse(self.inspect(good, page + "/recording.rs"))
        self.assertEqual(self.inspect(good.replace('self.recording.updates_task = Some(cx.spawn(work));', 'cx.spawn(work).detach();'), page + "/recording.rs")[0]["kind"], "macro_recording_accepts_unowned_completion")
        for source in ('fn start() { ServiceClient::spawn(); }', 'use razer_ipc::ServiceClient as Transport; fn start() { Transport::spawn(); }',
                       'use std::thread as native; fn start() { native::Builder::new(); }'):
            self.assertEqual(self.inspect(source, page + "/recording.rs")[0]["kind"], "page_owns_macro_recording_worker")
        service = "crates/razer-service/src/mapping/recorder.rs"
        good = 'use super::WorkerGuard as Guard; use razer_ipc::KEY_CAPTURE_LIFECYCLE as Lifecycle; fn spawn_with() { let guard = Guard::start(); let capture = Lifecycle.lock(); let result = shutdown(&mut client); drop(client); }'
        self.assertFalse(self.inspect(good, service))
        self.assertEqual(self.inspect(good.replace('Guard::start()', '()'), service)[0]["kind"], "mapping_worker_bypasses_cleanup_accounting")
        self.assertEqual(self.inspect(good.replace('shutdown(&mut client)', '()'), service)[0]["kind"], "macro_recorder_skips_transport_cleanup")
        self.assertEqual(self.inspect(good.replace('Lifecycle.lock()', 'OTHER.lock()'), service)[0]["kind"], "macro_recorder_bypasses_shared_capture_lifecycle")
        self.assertFalse(self.inspect('fn unrelated() { drop(client); }', service))

    def test_recorder_mode_and_factory_share_the_actual_input_owner(self):
        recorder = "crates/razer-service/src/mapping/recorder.rs"
        owner = "crates/razer-service/src/mapping/recorder_owner.rs"
        publisher = "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_publisher.rs"
        for name in ("spawn", "mode"):
            good = 'use super::recorder_owner as retained; fn ' + name + '() { retained::' + name + '(); }'
            self.assertFalse(self.inspect(good, recorder))
            bad = 'fn ' + name + '() { ServiceClient::spawn(); }'
            self.assertEqual(self.inspect(bad, recorder)[0]["kind"], "macro_recorder_bypasses_shared_transport_owner")
        query = 'use razer_ipc::ServiceRequest as Request; fn query() { client.request(Request::MacroRecordingMode); }'
        self.assertFalse(self.inspect(query, owner))
        self.assertEqual(self.inspect(query, publisher)[0]["kind"], "macro_mode_queries_an_unrelated_worker")
        self.assertFalse(self.inspect(query.replace('MacroRecordingMode', 'OtherObservation'), publisher))
        good = 'use razer_service::mapping::recorder as input; impl Publisher { fn macro_mode() { input::mode(); } }'
        self.assertFalse(self.inspect(good, publisher))
        self.assertEqual(self.inspect(good.replace('input::mode()', 'cached_mode()'), publisher)[0]["kind"], "mapping_publisher_bypasses_recorder_owner")

    def test_macro_and_focused_capture_share_one_native_lifecycle_through_aliases(self):
        key_capture = "crates/razer-service/src/mapping/key_capture.rs"
        prefix = 'use super::WorkerGuard; use razer_ipc::{KEY_CAPTURE_LIFECYCLE as Shared}; '
        good = prefix + 'impl Session { fn start_with() { let count = WorkerGuard::start(); let lifecycle = Shared.lock(); } }'
        self.assertFalse(self.inspect(good, key_capture))
        self.assertEqual(self.inspect(good.replace('Shared.lock()', 'OTHER.lock()'), key_capture)[0]["kind"], "key_capture_bypasses_shared_lifecycle")
        source = 'pub static KEY_CAPTURE_LIFECYCLE: Mutex<()> = Mutex::new(());'
        self.assertFalse(self.inspect(source, "crates/razer-ipc/src/lib.rs"))
        for owner in (key_capture, "crates/razer-service/src/mapping/recorder.rs"):
            self.assertEqual(self.inspect(source, owner)[0]["kind"], "duplicate_native_capture_lifecycle_owner")
            self.assertEqual(self.inspect(source.replace('KEY_CAPTURE_LIFECYCLE', 'LIFECYCLE'), owner)[0]["kind"], "duplicate_native_capture_lifecycle_owner")
        self.assertFalse(self.inspect(source.replace('KEY_CAPTURE_LIFECYCLE', 'LIFECYCLES'), "crates/razer-service/src/mapping/device.rs"))

    def test_macro_receipt_producer_and_consumer_share_typed_ipc_contracts(self):
        recorder = "crates/razer-service/src/mapping/recorder.rs"
        native = "crates/razer-service/src/runtime/windows/mapping_runtime/capture/recorder.rs"
        contracts = "crates/razer-ipc/src/macro_recording.rs"
        for name in ("SuspensionReceipt", "RestoreReceipt", "ActionReply"):
            source = 'pub struct ' + name + ' { value: bool }'
            self.assertFalse(self.inspect(source, contracts))
            self.assertEqual(self.inspect(source, recorder)[0]["kind"], "duplicate_macro_recording_receipt")
            self.assertFalse(self.inspect(source, "crates/razer-service/src/unrelated.rs"))
        source = ('use razer_ipc::macro_recording::{SuspensionReceipt as Paused, RestoreReceipt as Restored}; '
                  'use serde_json::from_value as decode; fn record() { let paused: Paused = decode(reply)?; '
                  'let restored: Restored = decode(reply)?; paused.suspended(); restored.global_shortcuts().result(); }')
        self.assertFalse(self.inspect(source, recorder))
        for changed in (source.replace('let restored: Restored', 'let restored'), source.replace('decode(reply)?', 'local_decode(reply)?'),
                        source.replace('restored.global_shortcuts().result()', 'restored.result()')):
            self.assertEqual(self.inspect(changed, recorder)[0]["kind"], "macro_recorder_bypasses_typed_receipts")
        source = ('use razer_ipc::macro_recording::{SuspensionReceipt as Paused, RestoreReceipt as Restored}; '
                  'fn recorder_request() { Paused::new(value); Restored::new(value); }')
        self.assertFalse(self.inspect(source, native))
        for changed in (source.replace('Paused::new', 'Other::new'), source.replace('Restored::new', 'Other::new')):
            self.assertEqual(self.inspect(changed, native)[0]["kind"], "native_macro_receipt_bypasses_shared_contract")
        source = 'fn release_recorder() { let receipts = recorder.restore(host)?; confirmed(receipts.0)?; }'
        self.assertFalse(self.inspect(source, native))
        self.assertEqual(self.inspect(source.replace('confirmed(receipts.0)', 'ignore_source_false(receipts.0)'), native)[0]["kind"], "native_macro_cleanup_accepts_unconfirmed_restore")
        self.assertFalse(self.inspect('fn unrelated() { self.restore(); }', native))

    def test_mapping_suppression_receipts_keep_owner_revision_and_target_through_aliases(self):
        contract = "crates/razer-ipc/src/mapping_control.rs"
        owner = "crates/razer-service/src/mapping/control.rs"
        source = ('use razer_ipc::mapping_control::Receipt as Confirmed; use serde_json::from_value as decode; '
                  'fn apply() { for command in owner.drain() { let receipt: Confirmed = decode(client.request(request))?; '
                  'anyhow::ensure!(receipt.confirms(command.owner_guid(), command.revision(), command.target()), "mismatch"); '
                  'command.acknowledge(Ok(())); } }')
        self.assertFalse(self.inspect(source, owner))
        delegated = 'fn apply() { apply_with(owner, |request| client.request(request)); } '
        helper = source.replace('fn apply()', 'fn apply_with()').replace('client.request(request)', 'request(command)')
        self.assertFalse(self.inspect(delegated + helper, owner))
        self.assertEqual(self.inspect(delegated.replace('apply_with(owner', 'other(owner') + helper, owner)[0]['kind'],
                         'mapping_control_bypasses_correlated_receipt')
        public_owner = 'crates/razer-service/src/mapping/mod.rs'
        public = 'fn apply_execution_control() { control::apply_with(owner, request) }'
        self.assertFalse(self.inspect(public, public_owner))
        self.assertEqual(self.inspect(public.replace('control::apply_with', 'other::apply'), public_owner)[0]['kind'],
                         'mapping_control_helper_bypasses_receipt_owner')
        for changed in (source.replace('receipt: Confirmed', 'receipt'), source.replace('decode(client.request(request))', 'other(client.request(request))'),
                        source.replace('receipt.confirms', 'receipt.accepts'), source.replace('command.revision()', '0'),
                        source.replace('command.acknowledge(Ok(()));', '')):
            self.assertEqual(self.inspect(changed, owner)[0]["kind"], "mapping_control_bypasses_correlated_receipt")
        self.assertFalse(self.inspect('fn unrelated() { let receipt = decode(reply); }', owner))
        registry = 'struct State { revision: u64, scopes: Map, owners: Map }'
        self.assertFalse(self.inspect(registry, contract))
        self.assertEqual(self.inspect(registry, owner)[0]["kind"], "duplicate_mapping_suppression_registry")
        self.assertFalse(self.inspect('struct Unrelated { revision: u64, owners: Map }', owner))

    def test_live_mapping_actor_entrypoints_retain_control_and_worker_owners(self):
        device = "crates/razer-service/src/mapping/device.rs"
        source = ('use super::WorkerGuard as Count; impl Session { fn start_with_registry() { '
                  'let count = Count::start(); let registered = registry.register(owner); '
                  'super::control::apply(client,&registered); registered.finish(); } }')
        self.assertFalse(self.inspect(source, device))
        self.assertEqual(self.inspect(source.replace('Count::start()', '()'), device)[0]["kind"], "mapping_worker_bypasses_cleanup_accounting")
        for changed in (source.replace('registry.register(owner)', 'registry.begin(owner)'),
                        source.replace('super::control::apply(client,&registered);', ''), source.replace('registered.finish();', '')):
            self.assertEqual(self.inspect(changed, device)[0]["kind"], "mapping_actor_bypasses_registered_control_owner")
        recorder = "crates/razer-service/src/mapping/recorder.rs"
        source = ('use super::WorkerGuard; fn spawn_with_registry() { WorkerGuard::start(); '
                  'super::lock_lifecycle(&cancel); shutdown(client); drop(client); }')
        self.assertFalse(self.inspect(source, recorder))
        self.assertEqual(self.inspect(source.replace('super::lock_lifecycle(&cancel)', 'other::lock_lifecycle(&cancel)'), recorder)[0]["kind"], "macro_recorder_bypasses_shared_capture_lifecycle")
        lock = 'fn lock_lifecycle() { razer_ipc::KEY_CAPTURE_LIFECYCLE.try_lock(); anyhow::ensure!(!canceled.load(order), "canceled"); }'
        self.assertFalse(self.inspect(lock, "crates/razer-service/src/mapping/mod.rs"))
        self.assertEqual(self.inspect(lock.replace('canceled.load(order)', 'false'), "crates/razer-service/src/mapping/mod.rs")[0]["kind"], "capture_lifecycle_bypasses_cancelable_shared_lock")

    def test_native_mapping_control_uses_observed_receipt_with_identity_checks(self):
        owner = "crates/razer-service/src/runtime/windows/mapping_runtime/backend.rs"
        source = ('use razer_ipc::mapping_control::Receipt as Confirmation; impl Backend { fn request() { '
                  'match request { ServiceRequest::MappingExecution{owner_guid,revision,target} => { '
                  'if self.core.global.as_ref().is_some_and(|current|current.scope==scope && current.owner==owner_guid) { '
                  'anyhow::ensure!(current.execution_revision.is_none_or(|prior| revision > prior), "stale"); '
                  '} else { let current=self.core.registration(scope,&owner_guid)?; current.ensure_active()?; '
                  'anyhow::ensure!(current.revision.is_none_or(|prior| revision > prior), "stale"); } '
                  'self.core.mapping(scope,true); self.core.shortcuts(scope,false); reply.result(); reply.reason(); '
                  'Confirmation::with_observations(owner_guid,revision,target,observations); } _ => {} } } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('Confirmation::with_observations', 'Other::with_observations'),
                        source.replace('current.owner==owner_guid', 'current.owner==other'),
                        source.replace('current.scope==scope', 'current.scope==other'),
                        source.replace('registration(scope,&owner_guid)', 'registration(other,&owner_guid)'),
                        source.replace('current.ensure_active()?', '()'),
                        source.replace('current.execution_revision.is_none_or(', 'current.execution_revision.map('),
                        source.replace('current.revision.is_none_or(', 'current.revision.map(')):
            self.assertEqual(self.inspect(changed, owner)[0]["kind"], "native_mapping_control_bypasses_owner_revision_receipt")
        self.assertFalse(self.inspect('fn unrelated() { Other::with_observations(); }', owner))
        for name in ("mapping_execution.rs", "global_shortcuts.rs", "global_shortcut_capture.rs", "mapping_native_output.rs"):
            retired = "crates/razer-service/src/runtime/windows/" + name
            self.assertEqual(self.inspect('', retired)[0]["kind"], "retired_architecture_source")

    def test_ui_preference_storage_is_only_called_by_its_retained_state_owner(self):
        state = "crates/razer-state/src/ui_preferences.rs"
        page = "crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration.rs"
        for source in ('fn load() { razer_storage::ui_preferences::load(flag); }',
                       'use razer_storage::ui_preferences as disk; fn load() { disk::save(flag,false); }',
                       'use razer_storage::{ui_preferences::{load as read, save}}; fn load() { read(flag); save(flag,true); }'):
            self.assertFalse(self.inspect(source, state))
            self.assertTrue(all(row["kind"] == "ui_preference_io_bypasses_shared_state" for row in self.inspect(source, page)))
            self.assertTrue(self.inspect(source, page))
        self.assertFalse(self.inspect('use other::load; fn callback() { load(flag); }', page))
        self.assertFalse(self.inspect('#[cfg(test)] fn load() { razer_storage::ui_preferences::load(flag); }', page))
        for source in ('fn callback() { std::fs::read(path); }',
                       'use std::fs as disk; fn callback() { disk::write(path, bytes); }',
                       'use std::{fs::{read as read_file}}; fn callback() { read_file(path); }',
                       'fn callback() { razer_storage::source_local::update(path,key,value); }',
                       'fn callback() { std::env::var_os("APPDATA"); }'):
            self.assertEqual(self.inspect(source, page)[0]["kind"], "preference_page_owns_local_file_io")
        self.assertFalse(self.inspect('fn callback() { std::env::var_os("OTHER_CONFIG"); }', page))
        self.assertFalse(self.inspect('fn read() { std::fs::read(path); }', "crates/razer-storage/src/ui_preferences.rs"))

    def test_ui_preference_state_retains_its_worker(self):
        state = "crates/razer-state/src/ui_preferences.rs"
        self.assertFalse(self.inspect('struct UiPreferences { core: Core, worker: Option<Task<()>> }', state))
        self.assertEqual(self.inspect('struct UiPreferences { core: Core }', state)[0]["kind"], "ui_preferences_has_no_retained_worker")
        self.assertEqual(self.inspect('struct UiPreferences { core: Core, worker: Option<Task<()>> }', "crates/razer-pages/src/features/preferences.rs")[0]["kind"], "duplicate_ui_preference_state_owner")
        good = 'impl UiPreferences { fn start_worker() { self.worker = Some(cx.spawn(work)); } }'
        self.assertFalse(self.inspect(good, state))
        self.assertEqual(self.inspect(good.replace('self.worker = Some(cx.spawn(work));', 'cx.spawn(work).detach();'), state)[0]["kind"], "ui_preferences_has_no_retained_worker")
        self.assertFalse(self.inspect('impl Unrelated { fn start_worker() { task.detach(); } }', state))

    def test_exit_closes_admission_and_drains_shared_local_io(self):
        shell = "crates/razer-shell/src/shell.rs"
        good = 'fn request_exit() { page.cancel_transfer(cx); razer_state::ui_preferences::begin_exit(cx); self.release_interdevice_holds(cx); shortcuts.dismiss_for_navigation(cx); workspace.prepare_exit(cx); runtime.prepare_exit(cx); self.close_requested = true; }'
        self.assertFalse(self.inspect(good, shell))
        for bad in (good.replace('page.cancel_transfer(cx);', ''), good.replace('razer_state::ui_preferences::begin_exit(cx);', ''),
                    good.replace('razer_state::ui_preferences::begin_exit(cx);', '').replace('runtime.prepare_exit(cx);', 'runtime.prepare_exit(cx); razer_state::ui_preferences::begin_exit(cx);')):
            self.assertTrue(any(row["kind"] == "shared_local_io_admitted_during_exit" for row in self.inspect(bad, shell)))
        aggregate = 'fn background_exit_pending() { razer_service::mapping::active_workers() + razer_service::seiren_audio::active_workers() + razer_storage::file_transfer::active_io() + razer_state::ui_preferences::active_io() + razer_state::chroma_settings::active_io() + other::active_io() }'
        self.assertFalse(self.inspect(aggregate, shell))
        self.assertFalse(self.inspect('use razer_state::ui_preferences::active_io as preferences_pending; ' + aggregate.replace('razer_state::ui_preferences::active_io()', 'preferences_pending()'), shell))
        for missing in ('razer_service::mapping::active_workers()', 'razer_service::seiren_audio::active_workers()', 'razer_storage::file_transfer::active_io()', 'razer_state::ui_preferences::active_io()', 'razer_state::chroma_settings::active_io()'):
            self.assertEqual(self.inspect(aggregate.replace(missing, '0'), shell)[0]["kind"], "shared_local_io_not_drained_at_exit")

    def test_exit_finishes_mapping_only_after_requested_saves_drain_and_waits_before_quit(self):
        shell = "crates/razer-shell/src/shell.rs"
        source = ('fn continue_save_queue() { if !succeeded { self.cancel_pending_exit(cx); } '
                  'if self.save_task.is_some() { return; } let operations_pending = self.keyboard_brightness.pending(); '
                  'if succeeded && !queued && !auxiliary_pending { if operations_pending { return; } '
                  'if self.close_requested { if self.finish_keyboard_actuation_exit(cx) || !self.close_requested { return; } '
                  'workspace.finish_exit(cx); self.stop_global_shortcuts(); '
                  'if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; } if self.finish_service_exit(cx) { return; } '
                  'if self.finish_resource_exit(cx) { return; } } } '
                  'match continuation { SaveContinuation::Close => cx.quit(), _ => {} } }')
        self.assertFalse(self.inspect(source, shell))
        self.assertFalse(self.inspect(source.replace('succeeded && !queued && !auxiliary_pending',
                                                    '(!auxiliary_pending) && (succeeded && !queued)'), shell))
        for changed in (source.replace('succeeded && !queued', 'succeeded'),
                        source.replace('&& !auxiliary_pending', ''),
                        source.replace('if operations_pending { return; }', ''),
                        source.replace('workspace.finish_exit(cx);', ''),
                        source.replace('self.stop_global_shortcuts();', ''),
                        source.replace('if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; }', ''),
                        source.replace('self.wait_for_background_exit(cx);', ''),
                        source.replace('workspace.finish_exit(cx);', '').replace('if self.close_requested {', 'workspace.finish_exit(cx); if self.close_requested {', 1),
                        source.replace('if operations_pending { return; }', 'workspace.finish_exit(cx); if operations_pending { return; }'),
                        source.replace('if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; }',
                                       'cx.quit(); if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; }')):
            self.assertEqual(self.inspect(changed, shell)[0]['kind'], 'exit_stops_mapping_before_requested_saves_drain')
        for changed in (source.replace('self.cancel_pending_exit(cx);', ''),
                        source.replace('if !succeeded { self.cancel_pending_exit(cx); } ', '').replace(
                            'if self.save_task.is_some() { return; }', 'if self.save_task.is_some() { return; } if !succeeded { self.cancel_pending_exit(cx); }'),
                        source.replace('self.cancel_pending_exit(cx);', 'self.close_requested = false;')):
            self.assertEqual(self.inspect(changed, shell)[0]['kind'], 'failed_exit_keeps_preferences_closed')
        self.assertFalse(self.inspect('fn unrelated() { cx.quit(); }', shell))

    def test_exit_waits_for_resources_after_product_and_shared_work(self):
        owner = 'crates/razer-shell/src/shell.rs'
        source = ('fn continue_save_queue() { if !succeeded { self.cancel_pending_exit(cx); } '
                  'if self.save_task.is_some() { return; } let operations_pending = self.keyboard_brightness.pending(); '
                  'if succeeded && !queued && !auxiliary_pending { if operations_pending { return; } '
                  'if self.close_requested { if self.finish_keyboard_actuation_exit(cx) || !self.close_requested { return; } '
                  'workspace.finish_exit(cx); self.stop_global_shortcuts(); '
                  'if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; } if self.finish_service_exit(cx) { return; } '
                  'if self.finish_resource_exit(cx) { return; } } } '
                  'match continuation { SaveContinuation::Close => cx.quit(), _ => {} } }')
        self.assertFalse(self.inspect(source, owner))
        resource_wait = 'if self.finish_resource_exit(cx) { return; }'
        for changed in (source.replace(resource_wait, ''), source.replace(resource_wait, 'self.finish_resource_exit(cx);'),
                        source.replace(resource_wait, '').replace('if operations_pending { return; }',
                            resource_wait + ' if operations_pending { return; }'),
                        source.replace(resource_wait, '').replace('if Self::background_exit_pending()',
                            resource_wait + ' if Self::background_exit_pending()')):
            self.assertTrue(any(row['kind'] == 'exit_quits_before_resource_owner_is_joined'
                                for row in self.inspect(changed, owner)))

    def test_service_cleanup_precedes_resources_and_platform_quit(self):
        owner = 'crates/razer-shell/src/shell.rs'
        source = ('fn continue_save_queue() { if !succeeded { self.cancel_pending_exit(cx); } '
                  'if self.save_task.is_some() { return; } let operations_pending = self.keyboard_brightness.pending(); '
                  'if succeeded && !queued && !auxiliary_pending { if operations_pending { return; } '
                  'if self.close_requested { if self.finish_keyboard_actuation_exit(cx) || !self.close_requested { return; } '
                  'workspace.finish_exit(cx); self.stop_global_shortcuts(); '
                  'if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; } '
                  'if self.finish_service_exit(cx) || self.finish_resource_exit(cx) { return; } } } '
                  'match continuation { SaveContinuation::Close => cx.quit(), _ => {} } }')
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect(source.replace('if self.finish_service_exit(cx) || self.finish_resource_exit(cx) { return; }',
                                                    'if self.finish_service_exit(cx) { return; } if self.finish_resource_exit(cx) { return; }'), owner))
        for changed in (source.replace('self.finish_service_exit(cx) || ', ''),
                        source.replace('self.finish_service_exit(cx) || self.finish_resource_exit(cx)',
                                       'self.finish_resource_exit(cx) || self.finish_service_exit(cx)'),
                        source.replace('self.finish_service_exit(cx) || self.finish_resource_exit(cx)',
                                       'self.finish_service_exit(cx) && self.finish_resource_exit(cx)'),
                        source.replace('self.finish_service_exit(cx) || ', '').replace('if operations_pending { return; }',
                                       'if self.finish_service_exit(cx) { return; } if operations_pending { return; }')):
            self.assertTrue(any(row['kind'] == 'exit_quits_before_service_cleanups_complete'
                                for row in self.inspect(changed, owner)))

    def test_service_hooks_are_retained_for_retries_and_completions_are_awaited(self):
        owner = 'crates/razer-shell/src/shell/shutdown.rs'
        register = ('fn register_exit_cleanup() { self.service_exit.hooks.push('
                    'Box::new(move |shell,cx| Box::pin(cleanup(shell,cx)))); }')
        self.assertFalse(self.inspect(register, owner))
        for changed in (register.replace('self.service_exit.hooks.push(', 'other.push('),
                        register.replace('cleanup(shell,cx)', 'other(shell,cx)'),
                        register.replace('self.service_exit.hooks.push(', 'drop(')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'exit_cleanup_registration_bypasses_shared_owner')
        take = ('fn take_exit_cleanups() { let mut hooks = std::mem::take(&mut self.service_exit.hooks); '
                'let completions = hooks.iter_mut().map(|cleanup| cleanup(self,cx)).collect(); '
                'hooks.append(&mut self.service_exit.hooks); self.service_exit.hooks = hooks; completions }')
        self.assertFalse(self.inspect(take, owner))
        self.assertFalse(self.inspect('use std::mem::take as consume; ' + take.replace('std::mem::take(', 'consume('), owner))
        for changed in (take.replace('std::mem::take(&mut self.service_exit.hooks)', 'self.service_exit.hooks.clone()'),
                        take.replace('cleanup(self,cx)', 'unrelated(self,cx)'), take.replace('.collect()', ''),
                        take.replace('.iter_mut()', '.iter_mut().take(1)'),
                        take.replace('.iter_mut()', '.into_iter()'),
                        take.replace('self.service_exit.hooks = hooks;', ''),
                        take.replace('self.service_exit.hooks = hooks;', 'self.service_exit.hooks = Vec::new();')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'exit_cleanup_hooks_have_no_single_retained_owner')
        finish = ('fn finish_service_exit() { if self.service_exit.task.is_some() { return true; } '
                  'if self.service_exit.hooks.is_empty() { return false; } let completions = self.take_exit_cleanups(cx); '
                  'self.service_exit.task = Some(cx.spawn(async move |shell,cx| { '
                  'for completion in completions { completion.await; } '
                  'shell.update(cx,|shell,cx| { shell.service_exit.task = None; '
                  'if shell.close_requested { shell.continue_save_queue(true,cx); } }); })); true }')
        self.assertFalse(self.inspect(finish, owner))
        for changed in (finish.replace('if self.service_exit.task.is_some() { return true; }', ''),
                        finish.replace('self.take_exit_cleanups(cx)', 'Vec::new()'),
                        finish.replace('self.service_exit.task = Some(cx.spawn(', 'Some(cx.spawn('),
                        finish.replace('completion.await;', 'completion;'),
                        finish.replace('completion.await;', 'unrelated.await;'),
                        finish.replace('for completion in completions', 'for completion in others'),
                        finish.replace('completion.await;', 'completion.await; break;'),
                        finish.replace('shell.service_exit.task = None;', ''),
                        finish.replace('for completion in completions', 'shell.service_exit.task = None; for completion in completions')
                              .replace('shell.update(cx,|shell,cx| { shell.service_exit.task = None;', 'shell.update(cx,|shell,cx| {'),
                        finish.replace('if shell.close_requested { shell.continue_save_queue(true,cx); }', 'shell.continue_save_queue(true,cx);')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'service_cleanups_have_no_retained_completion_barrier')

    def test_failed_save_completion_waits_for_service_cleanup_before_consuming_exit(self):
        owner = 'crates/razer-shell/src/shell.rs'
        guard = 'if self.service_exit.defer_save_continuation(succeeded) { return; } '
        source = ('fn continue_save_queue() { ' + guard +
                  'if !succeeded { self.cancel_pending_exit(cx); } '
                  'if self.save_task.is_some() { return; } let operations_pending = self.keyboard_brightness.pending(); '
                  'if succeeded && !queued && !auxiliary_pending { if operations_pending { return; } '
                  'if self.close_requested { if self.finish_keyboard_actuation_exit(cx) || !self.close_requested { return; } '
                  'workspace.finish_exit(cx); self.stop_global_shortcuts(); '
                  'if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; } '
                  'if self.finish_service_exit(cx) || self.finish_resource_exit(cx) { return; } } } '
                  'match save_continuation(&mut self.close_requested,succeeded,queued,auxiliary_pending) '
                  '{ SaveContinuation::Close => cx.quit(), _ => {} } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace(guard, ''), source.replace('defer_save_continuation(succeeded)', 'defer_save_continuation(true)'),
                        source.replace(guard, '').replace('if self.save_task.is_some()', guard + 'if self.save_task.is_some()'),
                        source.replace(guard, 'self.service_exit.defer_save_continuation(succeeded); ')):
            self.assertTrue(any(row['kind'] == 'cleanup_completion_consumes_exit_before_owners_resume'
                                for row in self.inspect(changed, owner)))
        self.assertFalse(self.inspect(source.replace(guard, ''), 'crates/other/src/lib.rs'))

    def test_actuation_exit_preserves_admitted_writes_and_rechecks_failure_before_final_cleanup(self):
        owner = 'crates/razer-shell/src/shell.rs'
        wait = 'if self.finish_keyboard_actuation_exit(cx) || !self.close_requested { return; }'
        source = ('fn continue_save_queue() { if !succeeded { self.cancel_pending_exit(cx); } '
                  'if self.save_task.is_some() { return; } let operations_pending = self.keyboard_brightness.pending(); '
                  'if succeeded && !queued && !auxiliary_pending { if operations_pending { return; } '
                  'if self.close_requested { ' + wait + ' workspace.finish_exit(cx); self.stop_global_shortcuts(); '
                  'if Self::background_exit_pending() { self.wait_for_background_exit(cx); return; } '
                  'if self.finish_service_exit(cx) || self.finish_resource_exit(cx) { return; } } } '
                  'match continuation { SaveContinuation::Close => cx.quit(), _ => {} } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace(wait, ''), source.replace(' || !self.close_requested', ''),
                        source.replace(wait, '').replace('if operations_pending { return; }', wait + ' if operations_pending { return; }'),
                        source.replace(wait, '').replace('workspace.finish_exit(cx);', 'workspace.finish_exit(cx); ' + wait),
                        source.replace(' || !self.close_requested', ' && !self.close_requested')):
            self.assertTrue(any(row['kind'] == 'exit_cancels_accepted_actuation_or_skips_failure_gate'
                                for row in self.inspect(changed, owner)))

    def test_actuation_acknowledgements_do_not_cancel_accepted_work_during_close(self):
        owner = 'crates/razer-shell/src/shell/keyboard/keyboard_actuation_write.rs'
        for name in ('finish_keyboard_actuation_save', 'submit_keyboard_actuation_device'):
            good = 'fn ' + name + '() { if !succeeded || !current { return; } self.submit_keyboard_actuation_device(); }'
            self.assertFalse(self.inspect(good, owner))
            self.assertFalse(self.inspect(good.replace('self.submit_keyboard_actuation_device();',
                'cx.spawn(async move |shell,cx| { if shell.close_requested { shell.continue_save_queue(true,cx); } });'), owner))
            bad = good.replace('!succeeded || !current', 'self.close_requested || !succeeded || !current')
            self.assertEqual(self.inspect(bad, owner)[0]['kind'], 'accepted_actuation_is_rejected_by_application_close')
        graceful = ('fn finish_keyboard_actuation_exit() { for session in self.keyboard_actuation_operations.values_mut() { '
                    'session.admission.stop_after_pending(); } self.reap_keyboard_actuation_workers(cx); '
                    '!self.keyboard_actuation_operations.is_empty() || !self.keyboard_actuation_workers.is_empty() '
                    '|| !self.keyboard_actuation_retired.is_empty() }')
        self.assertFalse(self.inspect(graceful, owner))
        for changed in (graceful.replace('session.admission.stop_after_pending();', ''),
                        graceful.replace('session.admission.stop_after_pending();', 'session.valid.store(false,Ordering::Release);'),
                        graceful.replace('self.reap_keyboard_actuation_workers(cx);', 'self.keyboard_actuation_operations.clear();'),
                        graceful.replace('|| !self.keyboard_actuation_retired.is_empty()', '')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'actuation_exit_cancels_accepted_native_fifo')
        self.assertEqual(self.inspect('impl Drop for Session { fn drop() { delivery.detach(); } }', owner)[0]['kind'],
                         'actuation_delivery_escapes_retained_session_lifetime')
        worker = 'crates/razer-shell/src/shell/keyboard/keyboard_actuation_worker.rs'
        stop = 'fn stop_after_pending() { self.commands.take(); }'
        self.assertFalse(self.inspect(stop, worker))
        for changed in (stop.replace('self.commands.take();', ''),
                        stop.replace('self.commands.take();', 'self.commands.take(); self.valid.store(false,Ordering::Release);')):
            self.assertEqual(self.inspect(changed, worker)[0]['kind'], 'actuation_exit_cancels_accepted_native_fifo')
        run = 'fn run_with() { let cleanup = guarded(|| transport.shutdown()); let destruction = guarded(|| { drop(transport); Ok(()) }); replies.send(Reply::Stopped); }'
        self.assertFalse(self.inspect(run, worker))
        for changed in (run.replace('transport.shutdown()', 'Ok(())'), run.replace('drop(transport);', ''),
                        run.replace('drop(transport);', '').replace('replies.send(Reply::Stopped);', 'replies.send(Reply::Stopped); drop(transport);')):
            self.assertEqual(self.inspect(changed, worker)[0]['kind'], 'actuation_stops_before_transport_cleanup')
        self.assertFalse(self.inspect('fn unrelated() { self.valid.store(false,Ordering::Release); }', worker))

    def test_background_cleanup_uses_shell_owner_instead_of_bounded_framework_callbacks(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_polling_worker.rs'
        for callback in ('async move { cleanup.await; }',
                         'let work = cx.background_executor().spawn(async move { worker.join(); }); async move { work.await; }',
                         'client.shutdown(); async {}', 'cx.background_spawn(async move { cleanup(); }); async {}'):
            source = 'fn install_cleanup() { self.subscriptions.push(cx.on_app_quit(|shell,cx| { ' + callback + ' })); }'
            self.assertEqual(self.inspect(source, owner)[0]['kind'], 'background_cleanup_relies_on_bounded_framework_quit')
            self.assertFalse(self.inspect(source.replace('self.subscriptions.push(cx.on_app_quit(', 'self.register_exit_cleanup(').replace('})); }', '}); }'), owner))
        self.assertFalse(self.inspect('fn install_tray() { cx.on_app_quit(|shell,cx| { shell.tray = None; async {} }); }', owner))
        self.assertFalse(self.inspect('fn install_fallback() { cx.on_app_quit(|shell,cx| async move { cleanup.await; }); }',
                                      'crates/razer-shell/src/shell/shutdown.rs'))
        self.assertFalse(self.inspect('fn unrelated() { worker.join(); }', owner))

    def test_session_observer_retains_a_joinable_owner_and_retires_off_the_ui_thread(self):
        platform = 'crates/razer-platform/src/platform/windows/session_changes.rs'
        shutdown = 'fn shutdown() { self.signal_stop(); self.worker.take().unwrap().join(); }'
        self.assertFalse(self.inspect(shutdown, platform))
        self.assertFalse(self.inspect('fn shutdown() { self.shutdown_with(post_stop) }', platform))
        self.assertFalse(self.inspect('fn shutdown_with() { self.signal_stop_with(wake); self.worker.take().unwrap().join(); }', platform))
        for changed in (shutdown.replace('self.signal_stop();', ''), shutdown.replace('.take()', ''), shutdown.replace('.join()', '')):
            self.assertEqual(self.inspect(changed, platform)[0]['kind'], 'session_observer_shutdown_does_not_join_its_owner')
        signal = 'fn signal_stop() { self.shared.stopped.store(true,Ordering::Release); self.shared.hwnd.try_lock(); }'
        self.assertFalse(self.inspect(signal, platform))
        self.assertFalse(self.inspect('fn signal_stop() { self.signal_stop_with(post_stop) }', platform))
        self.assertFalse(self.inspect(signal.replace('fn signal_stop()', 'fn signal_stop_with()'), platform))
        self.assertEqual(self.inspect(signal.replace('.try_lock()', '.lock()'), platform)[0]['kind'],
                         'session_observer_stop_waits_for_native_teardown_on_ui')
        self.assertFalse(self.inspect('impl Drop for Owner { fn drop() { self.signal_stop(); } }', platform))
        self.assertEqual(self.inspect('impl Drop for Owner { fn drop() { self.worker.join(); } }', platform)[0]['kind'],
                         'session_observer_stop_waits_for_native_teardown_on_ui')
        shell = 'crates/razer-shell/src/shell/keyboard/keyboard_lifecycle.rs'
        retire = 'fn retire(self) { let Self { monitor,pump } = self; drop(pump); monitor }'
        self.assertFalse(self.inspect(retire, shell))
        self.assertEqual(self.inspect(retire.replace('drop(pump);', ''), shell)[0]['kind'], 'session_observer_retirement_keeps_its_event_pump')
        install = ('fn install_keyboard_lifecycle() { self.register_exit_cleanup(|shell,cx| { '
                   'let cleanup = cx.background_executor().spawn(async move { monitor.shutdown(); }); async move { cleanup.await } }); }')
        self.assertFalse(self.inspect(install, shell))
        for changed in (install.replace('register_exit_cleanup', 'on_app_quit'), install.replace('monitor.shutdown();', ''),
                        install.replace('cleanup.await', 'cleanup')):
            self.assertTrue(any(row['kind'] == 'session_observer_is_not_drained_by_application_exit'
                                for row in self.inspect(changed, shell)))
        self.assertFalse(self.inspect('fn unrelated() { self.shared.hwnd.lock(); }', platform))
        pump = 'fn pump_until_stopped() { while !stopped.load(Ordering::Acquire) { if matches!(wait_and_dispatch(STOP_RECHECK_MS)?, MessageCycle::Quit) { break; } } Ok(()) }'
        self.assertFalse(self.inspect(pump, platform))
        for changed in (pump.replace('!stopped.load(Ordering::Acquire)', 'true'),
                        pump.replace('STOP_RECHECK_MS', 'u32::MAX'),
                        'fn run() { unsafe { GetMessageW(&mut message,window,0,0); } }'):
            self.assertEqual(self.inspect(changed, platform)[0]['kind'], 'session_observer_wait_has_no_bounded_stop_recheck')

    def test_resource_shutdown_is_retained_and_joined_in_background_before_exit_continues(self):
        owner = 'crates/razer-shell/src/shell.rs'
        source = ('fn finish_resource_exit() { if self.resource_exit_task.is_some() { return true; } '
                  'let Some(mut resources) = razer_assets::take_resource_runtime_for_shutdown(cx) else { return false; }; '
                  'self.resource_exit_task = Some(cx.spawn(async move |shell,cx| { '
                  'let result = cx.background_spawn(async move { resources.shutdown() }).await; '
                  'shell.update(cx,|shell,cx| { shell.resource_exit_task = None; '
                  'if shell.close_requested { shell.continue_save_queue(true,cx); } }); })); true }')
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('use razer_assets::take_resource_runtime_for_shutdown as take; '
                                      + source.replace('razer_assets::take_resource_runtime_for_shutdown(cx)', 'take(cx)'), owner))
        for changed in (source.replace('if self.resource_exit_task.is_some() { return true; }', ''),
                        source.replace('self.resource_exit_task = Some(cx.spawn(', 'Some(cx.spawn('),
                        source.replace('cx.background_spawn(async move { resources.shutdown() }).await', 'resources.shutdown()'),
                        source.replace('cx.background_spawn(async move { resources.shutdown() }).await',
                                       'cx.background_spawn(async move { resources.shutdown() })'),
                        source.replace('resources.shutdown()', 'other.shutdown()'),
                        source.replace('shell.resource_exit_task = None;', ''),
                        source.replace('let result = ', 'shell.resource_exit_task = None; let result = ').replace(
                                       'shell.update(cx,|shell,cx| { shell.resource_exit_task = None;', 'shell.update(cx,|shell,cx| {'),
                        source.replace('if shell.close_requested { shell.continue_save_queue(true,cx); }',
                                       'shell.continue_save_queue(true,cx);'),
                        source.replace('shell.resource_exit_task = None;', 'shell.resource_exit_task = None; cx.quit();')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'resource_shutdown_has_no_retained_completion_barrier')
        self.assertFalse(self.inspect('fn unrelated() { owner.shutdown(); }', owner))

    def test_resource_owner_transfer_closes_admission_and_notifications_without_joining_on_ui(self):
        owner = 'crates/razer-assets/src/resource_runtime.rs'
        source = ('fn take_resource_runtime_for_shutdown() { crate::image_cache::stop_resource_images(cx); let runtime = cx.global_mut::<Runtime>(); '
                  'let owner = runtime.owner.take()?; owner.begin_shutdown(); runtime.results.take(); Some(owner) }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('runtime.owner.take()', 'runtime.owner.clone()'),
                        source.replace('crate::image_cache::stop_resource_images(cx);', ''),
                        source.replace('owner.begin_shutdown();', ''), source.replace('runtime.results.take();', ''),
                        source.replace('owner.begin_shutdown(); runtime.results.take();', 'runtime.results.take(); owner.begin_shutdown();'),
                        source.replace('owner.begin_shutdown();', 'owner.begin_shutdown(); owner.shutdown();'),
                        source.replace('owner.begin_shutdown();', 'owner.begin_shutdown(); worker.join();')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'resource_owner_transfer_keeps_admission_or_blocks_ui')
        self.assertFalse(self.inspect('fn unrelated() { owner.shutdown(); }', owner))

    def test_canceling_exit_restores_shared_owners_and_rechecks_the_deferred_exit_identity(self):
        owner = 'crates/razer-shell/src/shell/persistence.rs'
        source = ('fn cancel_pending_exit() { if !std::mem::take(&mut self.close_requested) { return; } '
                  'razer_state::ui_preferences::cancel_exit(cx); self.background_exit_task = None; '
                  'self.runtime.update(cx, |runtime,cx| runtime.cancel_exit(cx)); '
                  'self.resume_global_shortcuts(cx); cx.defer(|cx| shell.update(cx, |shell,cx| { '
                  'if shell.close_requested { return; } shell.cancel_keyboard_game_exit(cx); shell.cancel_keyboard_latest_exit(window,cx); '
                  'workspace.cancel_exit(active,window,cx); })); cx.notify(); }')
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('use std::mem::take as reset; ' + source.replace('std::mem::take(', 'reset('), owner))
        for changed in (source.replace('!std::mem::take(&mut self.close_requested)', '!self.close_requested'),
                        source.replace('razer_state::ui_preferences::cancel_exit(cx);', ''),
                        source.replace('self.runtime.update(cx, |runtime,cx| runtime.cancel_exit(cx)); ', ''),
                        source.replace('runtime.cancel_exit(cx)', 'workspace.cancel_exit(cx)'),
                        source.replace('self.runtime.update(cx, |runtime,cx| runtime.cancel_exit(cx)); ', '').replace(
                            'cx.defer(|cx|', 'cx.defer(|cx| self.runtime.update(cx, |runtime,cx| runtime.cancel_exit(cx))); cx.defer(|cx|'),
                        source.replace('self.background_exit_task = None;', ''),
                        source.replace('self.resume_global_shortcuts(cx);', ''),
                        source.replace('shell.cancel_keyboard_game_exit(cx);', ''),
                        source.replace('shell.cancel_keyboard_latest_exit(window,cx);', ''),
                        source.replace('workspace.cancel_exit(active,window,cx);', ''),
                        source.replace('if shell.close_requested { return; }', ''),
                        source.replace('if shell.close_requested { return; }', '').replace(
                            'workspace.cancel_exit(active,window,cx);', 'workspace.cancel_exit(active,window,cx); if shell.close_requested { return; }')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'failed_exit_skips_shared_admission_recovery')
        self.assertFalse(self.inspect('fn unrelated() { self.close_requested = false; }', owner))

    def test_discovery_exit_preparation_retains_accepted_observations_and_stops_new_queries(self):
        owner = 'crates/razer-settings/src/runtime_page.rs'
        source = ('impl RuntimePanel { fn prepare_exit() { self.exit_prepared = true; self.cancel_read(); '
                  'self.pending_refresh = None; } fn refresh_queries() { '
                  'if self.exit_prepared || self.disconnecting { return; } self.bridge = RuntimeBridge::start(); '
                  'this.retired_bridges.push(bridge); this.retired_bridges.extend(this.bridge.take()); } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('self.exit_prepared = true;', ''),
                        source.replace('self.cancel_read();', ''),
                        source.replace('self.cancel_read();', 'self.cancel_read(); self.disconnect(cx);'),
                        source.replace('self.cancel_read();', 'self.cancel_read(); cx.emit(DiscoveryObserved(Err(error),None));'),
                        'use crate::DiscoveryObserved as Invalidated; ' + source.replace(
                            'self.cancel_read();', 'self.cancel_read(); cx.emit(Invalidated(Err(error),None));')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'exit_preparation_invalidates_accepted_device_observations')
        for changed in (source.replace('self.exit_prepared || ', ''),
                        source.replace('self.exit_prepared ||', 'self.exit_prepared &&'),
                        source.replace('self.exit_prepared ||', '!self.exit_prepared ||'),
                        source.replace('if self.exit_prepared || self.disconnecting { return; } ', '').replace(
                            'self.bridge = RuntimeBridge::start();', 'self.bridge = RuntimeBridge::start(); if self.exit_prepared { return; }')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'discovery_queries_admitted_during_exit')
        self.assertFalse(self.inspect('impl Unrelated { fn prepare_exit() { self.disconnect(cx); } fn refresh_queries() {} }', owner))
        self.assertFalse(self.inspect('impl RuntimePanel { fn unrelated() { cx.emit(DiscoveryObserved(Err(error),None)); } }', owner))
        for changed in (source.replace('this.retired_bridges.push(bridge);', ''),
                        source.replace('this.retired_bridges.extend(this.bridge.take());', 'this.bridge = None;')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'discovery_refresh_failure_discards_connection_join_owner')

    def test_discovery_connection_retains_and_joins_the_worker_after_failed_acknowledgement(self):
        owner = 'crates/razer-settings/src/runtime_page.rs'
        declarations = ('struct RuntimeBridge(Arc<ConnectionOwner>); struct ConnectionOwner { '
                        'commands: Sender<Command>, worker: Mutex<Option<std::thread::JoinHandle<()>>> }')
        self.assertFalse(self.inspect(declarations, owner))
        self.assertFalse(self.inspect('use std::thread::JoinHandle as Worker; ' + declarations.replace(
            'std::thread::JoinHandle<()>', 'Worker<()>'), owner))
        self.assertEqual(self.inspect(declarations.replace('Arc<ConnectionOwner>', 'Sender<Command>'), owner)[0]['kind'],
                         'discovery_connection_has_no_shared_worker_owner')
        self.assertEqual(self.inspect(declarations.replace('Option<std::thread::JoinHandle<()>>', 'bool'), owner)[0]['kind'],
                         'discovery_connection_has_no_retained_worker')
        start = ('impl RuntimeBridge { fn start_worker() { let worker = std::thread::Builder::new().spawn(run)?; '
                 'Ok(Self(Arc::new(ConnectionOwner { commands, worker: Mutex::new(Some(worker)) }))) } }')
        self.assertFalse(self.inspect(start, owner))
        for changed in (start.replace('Some(worker)', 'None'), start.replace('Some(worker)', 'Some(other)'),
                        start.replace('Ok(Self(', 'worker.detach(); Ok(Self(')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'discovery_connection_discards_worker_handle')
        shutdown = ('impl RuntimeBridge { fn disconnect() { let acknowledged = commands.send(request).and_then(|()| response.recv()?); '
                    'let worker = self.0.worker.lock().unwrap().take(); '
                    'let completed = worker.map_or(Ok(()), |worker| worker.join().map_err(|_| error)); '
                    'match (acknowledged, completed) { (Ok(()), result) | (result, Ok(())) => result, '
                    '(Err(acknowledged), Err(completed)) => Err((acknowledged, completed)), } } }')
        self.assertFalse(self.inspect(shutdown, owner))
        for changed in (shutdown.replace('.take()', '.clone()'), shutdown.replace('worker.join()', 'worker.is_finished()'),
                        shutdown.replace('let worker =', 'acknowledged?; let worker ='),
                        shutdown.replace('let worker =', 'if acknowledged.is_err() { return acknowledged; } let worker ='),
                        shutdown.replace('match (acknowledged, completed)', 'match acknowledged')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'discovery_connection_shutdown_does_not_join_after_ack_failure')
        self.assertFalse(self.inspect('impl RuntimeBridge { fn unrelated() { response.recv()?; } }', owner))

    def test_device_observation_worker_is_retained_joined_and_closes_delivery_without_waiting_on_ui(self):
        owner = 'crates/razer-platform/src/observation/device_changes.rs'
        source = ('struct DeviceChangeMonitor { stop: Option<Sender<()>>, worker: Option<JoinHandle<Result<(),String>>> } '
                  'impl DeviceChangeMonitor { fn start_worker() { let worker = Builder::new().spawn(run)?; '
                  'Ok(Self { stop: Some(stop), worker: Some(worker) }) } '
                  'fn shutdown(mut self) { self.stop.take(); self.worker.take().unwrap().join()? } } '
                  'impl Drop for DeviceChangeMonitor { fn drop(&mut self) { self.stop.take(); } } '
                  'impl Drop for EventDelivery { fn drop(&mut self) { self.0.close(); } } '
                  'fn wait_for_stop() { let _ = events.force_send(DeviceChange::Ready); let _ = stopped.recv(); }')
        self.assertFalse(self.inspect(source, owner))
        changes = {
            'device_observer_has_no_retained_worker': source.replace('Option<JoinHandle<Result<(),String>>>', 'bool'),
            'device_observer_discards_worker_handle': source.replace('worker: Some(worker)', 'worker: None'),
            'device_observer_shutdown_does_not_stop_then_join': source.replace('self.stop.take(); self.worker', 'self.worker'),
            'device_observer_drop_waits_for_native_teardown_on_ui': source.replace(
                'fn drop(&mut self) { self.stop.take();', 'fn drop(&mut self) { self.worker.take().unwrap().join(); self.stop.take();'),
            'device_observer_delivery_outlives_its_worker': source.replace('self.0.close();', ''),
            'device_observer_stop_waits_for_ui_to_drain_queue': source.replace('events.force_send(', 'events.send_blocking('),
        }
        for expected, changed in changes.items():
            with self.subTest(kind=expected):
                self.assertEqual(self.inspect(changed, owner)[0]['kind'], expected)
        self.assertFalse(self.inspect('impl Unrelated { fn shutdown() { worker.join(); } }', owner))

    def test_discovery_observer_retirement_and_final_cleanup_keep_the_join_owner_and_disconnect_intent(self):
        owner = 'crates/razer-settings/src/runtime_page.rs'
        retirement = ('impl RuntimePanel { fn monitor_device_changes() { match event { '
                      'DeviceChange::Failed(error) => { this.retired_monitors.extend(this.monitor.take()); } } } }')
        self.assertFalse(self.inspect(retirement, owner))
        self.assertEqual(self.inspect(retirement.replace('this.retired_monitors.extend(this.monitor.take());',
            'this.monitor = None;'), owner)[0]['kind'], 'failed_discovery_observer_discards_its_join_owner')
        shutdown = ('impl RuntimePanel { fn disconnect() { self.begin_disconnect(DisconnectIntent::User,cx); } '
                    'fn shutdown_for_app_quit() { self.exit_prepared = true; '
                    'self.begin_disconnect(DisconnectIntent::ApplicationExit,cx); self.disconnect_task.take() } }')
        self.assertFalse(self.inspect(shutdown, owner))
        self.assertEqual(self.inspect(shutdown.replace('DisconnectIntent::User,', 'DisconnectIntent::ApplicationExit,'), owner)[0]['kind'],
                         'manual_discovery_disconnect_loses_user_intent')
        for changed in (shutdown.replace('self.exit_prepared = true;', ''),
                        shutdown.replace('DisconnectIntent::ApplicationExit,', 'DisconnectIntent::User,'),
                        shutdown.replace('self.disconnect_task.take()', 'self.disconnect_task.clone()')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'application_discovery_shutdown_loses_completion_or_intent')
        cleanup = ('impl RuntimePanel { fn begin_disconnect() { if self.disconnecting { return; } '
                   'if self.bridge.is_none() && self.retired_bridges.is_empty() && self.monitor.is_none() && self.retired_monitors.is_empty() { return; } '
                   'let mut bridges = std::mem::take(&mut self.retired_bridges); bridges.extend(self.bridge.take()); '
                   'let mut monitors = std::mem::take(&mut self.retired_monitors); '
                   'monitors.extend(self.monitor.take()); self.disconnect_task = Some(cx.spawn(async move { '
                   'let result = cx.background_spawn(async move { let mut errors = Vec::new(); '
                   'for monitor in monitors { if let Err(error) = monitor.shutdown() { errors.push(error); } } '
                   'for bridge in bridges { if let Err(error) = bridge.disconnect() { errors.push(error); } } errors }).await; '
                   'this.update(cx, |this,cx| { this.finish_disconnect(result); '
                   'if matches!(intent, DisconnectIntent::User) { cx.emit(DiscoveryObserved(Err(error),None)); } }); })); } }')
        self.assertFalse(self.inspect(cleanup, owner))
        for changed in (cleanup.replace(' && self.retired_monitors.is_empty()', ''),
                        cleanup.replace(' && self.retired_bridges.is_empty()', ''),
                        cleanup.replace('std::mem::take(&mut self.retired_bridges)', 'self.retired_bridges.clone()'),
                        cleanup.replace('bridges.extend(self.bridge.take());', ''),
                        cleanup.replace('std::mem::take(&mut self.retired_monitors)', 'self.retired_monitors.clone()'),
                        cleanup.replace('monitors.extend(self.monitor.take());', ''),
                        cleanup.replace('monitor.shutdown()', 'monitor.shutdown()?'),
                        cleanup.replace('errors.push(error);', 'errors.push(error); break;'),
                        cleanup.replace('cx.background_spawn(', 'cx.spawn('),
                        cleanup.replace('self.disconnect_task = ', 'let detached = ')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'discovery_shutdown_skips_or_blocks_observation_join')
        for changed in (cleanup.replace('DisconnectIntent::User)', 'DisconnectIntent::ApplicationExit)'),
                        cleanup.replace('if matches!(intent, DisconnectIntent::User)', 'if result.is_err()')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'application_exit_republishes_artificial_device_disconnect')

    def test_thx_release_spawn_failure_retains_the_owner_and_source_close_for_background_cleanup(self):
        owner = 'crates/razer-shell/src/shell/audio/thx_sound.rs'
        declaration = 'struct RetiredOwner { owner: OwnerSlot, source_close: bool }'
        self.assertFalse(self.inspect(declaration, owner))
        self.assertEqual(self.inspect(declaration.replace('source_close: bool', 'reason: String'), owner)[0]['kind'],
                         'thx_retirement_discards_owner_or_source_close')
        retirement = ('impl AppShell { fn retire_thx_owner() { let owner = self.thx_owners.remove(identity); '
                      'let release = owner.clone(); match thread::Builder::new().spawn(move || release_owner(release,source_close,&history)) { '
                      'Ok(worker) => self.thx_cleanup.push(worker), Err(error) => { '
                      'self.thx_retired_owners.push(RetiredOwner { owner, source_close }); } } } }')
        self.assertFalse(self.inspect(retirement, owner))
        self.assertFalse(self.inspect(retirement.replace('RetiredOwner { owner, source_close }',
            'RetiredOwner { owner: owner, source_close: source_close }'), owner))
        for changed in (retirement.replace('let release = owner.clone();', 'let release = owner;'),
                        retirement.replace('release_owner(release,', 'release_owner(owner,'),
                        retirement.replace('self.thx_retired_owners.push(RetiredOwner { owner, source_close });', ''),
                        retirement.replace('self.thx_cleanup.push(worker)', 'drop(worker)')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'thx_release_spawn_failure_discards_its_owner')
        cleanup = ('impl AppShell { fn install_thx_cleanup() { self.register_exit_cleanup(|this,cx| { '
                   'let retired = std::mem::take(&mut this.thx_retired_owners); '
                   'let cleanup = cx.background_executor().spawn(async move { '
                   'for retired in retired { release_owner(retired.owner,retired.source_close,&history); } }); '
                   'async move { cleanup.await } }); } }')
        self.assertFalse(self.inspect(cleanup, owner))
        for changed in (cleanup.replace('std::mem::take(&mut this.thx_retired_owners)', 'this.thx_retired_owners.clone()'),
                        cleanup.replace('cx.background_executor().spawn(', 'cx.spawn('),
                        cleanup.replace('retired.source_close', 'true')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'thx_retired_owners_not_released_by_background_exit')
        successor = ('impl AppShell { fn submit_thx_job() { let worker = match Builder::new().spawn(move || { '
                     'for retired in backlog { release_owner(retired.owner,retired.source_close,&history); } }) { '
                     'Ok(worker) => worker, Err(error) => { self.thx_retired_owners.extend(std::mem::take(&mut retired)); return; } }; } }')
        self.assertFalse(self.inspect(successor, owner))
        for changed in (successor.replace('self.thx_retired_owners.extend(std::mem::take(&mut retired));', ''),
                        successor.replace('retired.source_close', 'true')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'thx_successor_discards_failed_release_backlog')
        self.assertFalse(self.inspect('fn unrelated() { owner.clone(); }', owner))

    def test_ipc_process_and_pipe_workers_keep_one_owner_until_exit_and_join(self):
        owner = 'crates/razer-ipc/src/lib.rs'
        declaration = ('struct ServiceClient { child: Option<Child>, requests: Option<Sender<Vec<u8>>>, '
                       'input_worker: Option<JoinHandle<anyhow::Result<()>>>, output_worker: Option<JoinHandle<anyhow::Result<()>>>, '
                       'diagnostic_worker: Option<JoinHandle<anyhow::Result<()>>> }')
        self.assertFalse(self.inspect(declaration, owner))
        self.assertEqual(self.inspect(declaration.replace('input_worker: Option<JoinHandle<anyhow::Result<()>>>', 'input_worker: bool'), owner)[0]['kind'],
                         'ipc_client_discards_owned_process_or_pipe_workers')
        shutdown = ('impl ServiceClient { fn finish_process() { self.requests.take(); self.stopped = true; self.job.take(); '
                    'reap_child(&mut self.child)?; let input = join_pipe(self.input_worker.take(),"input"); '
                    'let output = join_pipe(self.output_worker.take(),"output"); '
                    'let diagnostics = join_pipe(self.diagnostic_worker.take(),"diagnostics"); '
                    'combine_cleanup([input,output,diagnostics]) } }')
        self.assertFalse(self.inspect(shutdown, owner))
        for changed in (shutdown.replace('self.requests.take();', ''), shutdown.replace('self.stopped = true;', ''),
                        shutdown.replace('self.job.take();', ''),
                        shutdown.replace('self.stopped = true;', '').replace('reap_child(&mut self.child)?;',
                            'reap_child(&mut self.child)?; self.stopped = true;'),
                        shutdown.replace('join_pipe(self.input_worker.take(),"input");', 'join_pipe(self.input_worker.take(),"input")?;'),
                        shutdown.replace('reap_child(&mut self.child)?;', 'reap_child(&mut self.child);'),
                        shutdown.replace('reap_child(&mut self.child)?;', '').replace(
                            'combine_cleanup([input,output,diagnostics])', 'reap_child(&mut self.child)?; combine_cleanup([input,output,diagnostics])'),
                        shutdown.replace('self.output_worker.take()', 'self.output_worker.clone()'),
                        shutdown.replace('self.diagnostic_worker.take()', 'self.diagnostic_worker.clone()'),
                        shutdown.replace('let diagnostics = join_pipe(self.diagnostic_worker.take(),"diagnostics"); ', ''),
                        shutdown.replace('combine_cleanup([input,output,diagnostics])', 'combine_cleanup([input,output])'),
                        shutdown.replace('combine_cleanup([input,output,diagnostics])', 'input')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'ipc_cleanup_does_not_preserve_owner_until_reap_and_pipe_joins')
        graceful = ('impl ServiceClient { fn stop() { self.stopped = true; '
                    'let cleanup = worker_shutdown::drain(&self.responses, id, timeout); '
                    'let process = self.finish_process(); combine_cleanup([cleanup, process]) } }')
        self.assertFalse(self.inspect(graceful, owner))
        for changed in (graceful.replace('worker_shutdown::drain(&self.responses, id, timeout)', 'Ok(())'),
                        graceful.replace('self.stopped = true;', ''),
                        graceful.replace('self.finish_process()', 'Ok(())'),
                        graceful.replace('combine_cleanup([cleanup, process])', 'process'),
                        graceful.replace('let cleanup', 'self.job.take(); let cleanup')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'ipc_stop_skips_graceful_owner_cleanup')
        destructor = 'impl Drop for ServiceClient { fn drop(&mut self) { if let Err(error) = self.stop() { log(error); } } }'
        self.assertFalse(self.inspect(destructor, owner))
        self.assertEqual(self.inspect(destructor.replace('if let Err(error) = self.stop()', 'if !self.stopped'), owner)[0]['kind'],
                         'ipc_drop_skips_unfinished_cleanup_after_admission_closed')
        reaping = 'fn reap_child() { if let Some(child) = child.as_mut() { terminate_child(child)?; } child.take(); Ok(()) }'
        self.assertFalse(self.inspect(reaping, owner))
        self.assertEqual(self.inspect(reaping.replace('terminate_child(child)?', 'terminate_child(child)'), owner)[0]['kind'],
                         'ipc_reap_discards_unconfirmed_process_owner')
        retained = ('use std::thread; fn spawn_with_program() { client.input_worker = Some(thread::Builder::new().spawn(run)?); '
                    'client.output_worker = Some(thread::spawn(run)); client.diagnostic_worker = Some(thread::spawn(run)); }')
        self.assertFalse(self.inspect(retained, owner))
        for bad in ('use std::thread; fn cleanup() { thread::Builder::new().name("reaper".into()).spawn(run); }',
                    'use std::thread::spawn as run; fn cleanup() { run(reap); }'):
            self.assertEqual(self.inspect(bad, owner)[0]['kind'], 'ipc_thread_has_no_retained_pipe_owner')
        self.assertFalse(self.inspect('fn unrelated() { Command::new(program).spawn(); }', owner))

    def test_auxiliary_save_storage_failure_reopens_exit_admission(self):
        owner = 'crates/razer-shell/src/shell.rs'
        source = ('fn save_auxiliary_preferences() { if self.save_task.is_some() { return; } '
                  'if let Some(error) = &self.storage_error { self.status = error; '
                  'self.cancel_pending_exit(cx); cx.notify(); return; } write_preferences(); }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('self.cancel_pending_exit(cx);', ''),
                        'fn save_auxiliary_preferences() { if self.save_task.is_some() || self.storage_error.is_some() { return; } }'):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'auxiliary_save_failure_keeps_exit_admission_closed')
        self.assertFalse(self.inspect('fn unrelated() { if self.storage_error.is_some() { return; } }', owner))

    def test_legacy_profile_early_storage_failure_reopens_exit_admission(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_dpi_70.rs'
        source = ('fn persist_legacy_profile() { if let Some(error) = &self.storage_error { '
                  'self.status = error; self.cancel_pending_exit(cx); cx.notify(); return; } '
                  'self.pending_saves.push_back(self.prepare_save()); self.start_pending_save(cx); }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('self.cancel_pending_exit(cx);', ''),
                        'fn persist_legacy_profile() { if self.storage_error.is_some() { return; } }'):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'legacy_profile_save_failure_keeps_exit_admission_closed')
        self.assertFalse(self.inspect('fn unrelated() { if self.storage_error.is_some() { return; } }', owner))

    def test_product_exit_stops_retained_sessions_before_wait_and_reopens_on_cancel(self):
        lifecycle = "crates/razer-pages/src/features/workspace/source_workspace/lifecycle.rs"
        source = ('impl SourceProductWorkspace { fn prepare_exit() { self.exiting = true; self.set_active(false); '
                  'body.prepare_camera_exit(cx); body.dismiss_editors(); audio.prepare_exit(); } fn finish_exit() { body.finish_mapping_service(); } '
                  'fn cancel_exit() { self.exiting = false; body.cancel_mapping_service_exit(); self.set_active(active); } }')
        self.assertFalse(self.inspect(source, lifecycle))
        for changed in (source.replace('self.exiting = true;', ''), source.replace('body.dismiss_editors();', ''),
                        source.replace('body.dismiss_editors();', 'body.dismiss_editors(); body.finish_mapping_service();'),
                        source.replace('self.exiting = true;', '').replace('self.set_active(false);', 'self.set_active(false); self.exiting = true;')):
            self.assertEqual(self.inspect(changed, lifecycle)[0]["kind"], "product_exit_keeps_session_admission_or_workers")
        self.assertEqual(self.inspect(source.replace('fn finish_exit() { body.finish_mapping_service(); }', 'fn finish_exit() {}'), lifecycle)[0]["kind"], "product_exit_skips_pending_mapping_drain")
        for changed in (source.replace('self.exiting = false;', ''), source.replace('body.cancel_mapping_service_exit();', '')):
            self.assertEqual(self.inspect(changed, lifecycle)[0]["kind"], "failed_exit_does_not_restore_product_admission")
        self.assertFalse(self.inspect('fn unrelated() { self.exiting = false; }', lifecycle))

    def test_global_capture_preserves_one_typed_input_and_mode_poll(self):
        owner = "crates/razer-service/src/mapping/key_capture.rs"
        source = ('use razer_ipc::ServiceRequest as Request; fn start_with_registry() { match kind { '
                  'Kind::GlobalShortcut => { let batch = client.request(Request::GlobalShortcutCaptureEvents); '
                  'for event in batch.into_events() { send(event); } }, '
                  'Kind::Mapping => client.request(Request::MappingKeyCaptureEvents), } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('Request::GlobalShortcutCaptureEvents', 'Request::MappingKeyCaptureEvents'),
                        source.replace('for event in batch.into_events()', 'for event in batch.events()'),
                        source.replace('send(event);', 'send(event); client.request(Request::DeviceModeEvents);')):
            self.assertEqual(self.inspect(changed, owner)[0]["kind"], "global_capture_splits_input_and_mode_order")
        self.assertFalse(self.inspect('fn unrelated() { client.request(Request::DeviceModeEvents); }', owner))

    def test_mapping_drain_closes_the_shared_admission_before_enqueuing_finish(self):
        owner = "crates/razer-service/src/mapping/device.rs"
        source = ('impl Session { fn request_stop_after_pending() { '
                  'let mut admission = self.admission.lock()?; '
                  'if matches!(*admission, Admission::Draining) { return Ok(()); } '
                  '*admission = Admission::Draining; self.commands.send(Command::Finish) } }')
        self.assertFalse(self.inspect(source, owner))
        renamed = 'type Gate = Admission; type Work = Command; ' + source.replace('Admission::', 'Gate::').replace('Command::', 'Work::')
        self.assertFalse(self.inspect(renamed, owner))
        for changed in (source.replace('self.admission.lock()', 'other.lock()'),
                        source.replace('*admission = Admission::Draining;', ''),
                        source.replace('*admission = Admission::Draining;', 'self.canceled.store(true); *admission = Admission::Draining;'),
                        source.replace('*admission = Admission::Draining; self.commands.send(Command::Finish)',
                                       'self.commands.send(Command::Finish); *admission = Admission::Draining;')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'mapping_drain_bypasses_fifo_admission')
        self.assertFalse(self.inspect(source, "crates/razer-service/src/unrelated.rs"))

    def test_mapping_publish_rejects_work_after_the_shared_admission_closes(self):
        owner = "crates/razer-service/src/mapping/device.rs"
        source = ('impl Session { fn publish_with_restore() { let admission = self.admission.lock()?; '
                  'if !matches!(*admission, Admission::Accepting) { return Err(error); } '
                  'self.commands.send(Command::Publish(document)) } }')
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('type Gate = Admission; type Work = Command; ' +
                                     source.replace('Admission::', 'Gate::').replace('Command::', 'Work::'), owner))
        for changed in (source.replace('self.admission.lock()', 'other.lock()'),
                        source.replace('!matches!', 'matches!'),
                        source.replace('Admission::Accepting', 'Admission::Draining'),
                        source.replace('return Err(error);', 'diagnostic();'),
                        source.replace('let admission = self.admission.lock()?;', '').replace(
                            'return Err(error); }', 'return Err(error); } let admission = self.admission.lock()?;')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'mapping_publish_bypasses_closed_admission')
        self.assertFalse(self.inspect('fn unrelated() { self.commands.send(Command::Publish(document)); }', owner))

    def test_global_shortcut_drain_keeps_admission_and_each_compiler_publication(self):
        owner = 'crates/razer-service/src/mapping/shortcuts.rs'
        source = ('impl Session { fn publish() { let admission = self.admission.lock()?; '
                  'if !matches!(*admission, Admission::Accepting) || self.stop.load(order) || self.finished.load(order) '
                  '{ return Err(error); } self.commands.send(Command::Publish(document)) } '
                  'fn request_stop_after_pending() { let mut admission = self.admission.lock()?; '
                  'if matches!(*admission, Admission::Draining) || self.finished.load(order) { return Ok(()); } '
                  '*admission = Admission::Draining; self.commands.send(Command::Finish) } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('!matches!', 'matches!'),
                        source.replace('Admission::Accepting', 'Admission::Draining')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'mapping_publish_bypasses_closed_admission')
        for changed in (source.replace('*admission = Admission::Draining;', ''),
                        source.replace('*admission = Admission::Draining;', '*admission = Admission::Draining; self.stop.store(true, order);'),
                        source.replace('*admission = Admission::Draining;', '*admission = Admission::Draining; self.request_stop();')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'mapping_drain_bypasses_fifo_admission')
        run = ('fn run(input: &mpsc::Receiver<Command>) { let control = registry.register(); '
               'super::control::apply(client, control); input.recv_timeout(timeout); control.finish(result); }')
        self.assertFalse(self.inspect(run, owner))
        self.assertEqual(self.inspect(run.replace('input.recv_timeout(timeout)', 'input.try_recv()'), owner)[0]['kind'],
                         'shortcut_worker_coalesces_compiler_publications')
        self.assertFalse(self.inspect(run.replace('input.recv_timeout(timeout)', 'other.try_recv()'), owner))

    def test_keyboard_exit_waits_for_both_queued_work_and_accepted_recovery(self):
        owner = 'crates/razer-shell/src/shell/keyboard/keyboard_latest_restore.rs'
        source = ('impl Owner { fn stop_if_drained() { if self.coordinator.is_idle() '
                  '&& self.recoveries.load(Ordering::Acquire) == 0 { self.prepare_exit(); '
                  'self.valid.store(false, Ordering::Release); self.stopping = true; } } }')
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect(source.replace('self.recoveries.load(Ordering::Acquire) == 0',
                                                    '0 == self.recoveries.load(OtherOrdering::Acquire)'), owner))
        for changed in (source.replace('self.coordinator.is_idle() && ', ''),
                        source.replace('&& self.recoveries.load(Ordering::Acquire) == 0', ''),
                        source.replace('== 0', '!= 0'), source.replace('&&', '||'),
                        source.replace('self.prepare_exit();', 'self.prepare_exit(); } {')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'keyboard_exit_stops_before_queue_and_recovery_drain')
        self.assertFalse(self.inspect(source.replace('impl Owner', 'impl Unrelated'), owner))

    def test_keyboard_publisher_uses_shared_control_and_finishes_after_transport_release(self):
        owner = 'crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_publisher.rs'
        source = ('impl Publisher { fn start_with() { registry.register(owner); } '
                  'fn apply_control() { razer_service::mapping::apply_execution_control(owner, request); } '
                  'fn cleanup() { let cleanup = shutdown(&mut client); drop(client); self.control.finish(cleanup); } }')
        self.assertFalse(self.inspect(source, owner))
        self.assertFalse(self.inspect('use razer_service::mapping::apply_execution_control as apply; use std::mem::drop as release; ' +
                                     source.replace('razer_service::mapping::apply_execution_control(owner', 'apply(owner').replace('drop(client)', 'release(client)'), owner))
        self.assertEqual(self.inspect(source.replace('registry.register(owner);', ''), owner)[0]['kind'],
                         'keyboard_publisher_bypasses_mapping_control_registry')
        self.assertEqual(self.inspect(source.replace('razer_service::mapping::apply_execution_control', 'other::apply'), owner)[0]['kind'],
                         'keyboard_publisher_bypasses_receipt_owner')
        for changed in (source.replace('let cleanup = shutdown(&mut client);', ''), source.replace('drop(client);', ''),
                        source.replace('drop(client); self.control.finish(cleanup);', 'self.control.finish(cleanup); drop(client);')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'keyboard_publisher_releases_registry_before_native_cleanup')
        self.assertFalse(self.inspect('fn unrelated() { registry.finish(); }', owner))

    def test_keyboard_worker_keeps_due_recovery_before_automatic_polling_stops(self):
        owner = 'crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_worker.rs'
        source = ('fn run() { loop { let recovery = PendingRecovery::start(recoveries); '
                  'if reset_due { reset_navigation_feedback(client); } '
                  'if !automatic.load(Ordering::Acquire) { continue; } poll_events(); } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('PendingRecovery::start(recoveries)', 'untracked_recovery()'),
                        source.replace('reset_navigation_feedback(client)', 'skip_recovery()'),
                        source.replace('if reset_due { reset_navigation_feedback(client); } ', '').replace(
                            'continue; }', 'continue; } if reset_due { reset_navigation_feedback(client); }')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'keyboard_exit_skips_accepted_navigation_recovery')
        self.assertFalse(self.inspect('fn unrelated() { if !automatic.load(order) { return; } }', owner))

    def test_legacy_keyboard_admits_initial_publication_without_waiting_for_ui_ready(self):
        owner = 'crates/razer-pages/src/features/keyboard/mapping/keyboard_mapping_runtime_515.rs'
        source = ('fn start_legacy_mapping_owner() { self.legacy_mapping.owner = Some(owner); '
                  'self.legacy_mapping.poll = Some(cx.spawn(|view,cx| { match reply { Reply::Ready => {}, _ => {} } })); '
                  'self.submit_legacy_mapping(cx); }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('self.submit_legacy_mapping(cx);', ''),
                        source.replace('self.legacy_mapping.owner = Some(owner);', ''),
                        source.replace('self.legacy_mapping.poll = Some(cx.spawn', 'other = Some(cx.spawn'),
                        source.replace('self.submit_legacy_mapping(cx);', '').replace('Reply::Ready => {}', 'Reply::Ready => { self.submit_legacy_mapping(cx); }'),
                        source.replace('self.submit_legacy_mapping(cx);', '').replace('self.legacy_mapping.owner = Some(owner);',
                            'self.submit_legacy_mapping(cx); self.legacy_mapping.owner = Some(owner);')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'keyboard_initial_publication_waits_for_presentation_ready')
        publication = ('fn publish_legacy_mapping() { if different_container { '
                       'self.legacy_mapping.document = Some(document); '
                       'self.set_legacy_mapping_connection(Some(container),cx); self.start_legacy_mapping_owner(cx); return; } }')
        self.assertFalse(self.inspect(publication, owner))
        self.assertEqual(self.inspect(publication.replace('self.legacy_mapping.document = Some(document);', ''), owner)[0]['kind'],
                         'keyboard_replacement_registers_before_current_document')
        self.assertEqual(self.inspect(publication.replace('self.start_legacy_mapping_owner(cx);', ''), owner)[0]['kind'],
                         'keyboard_replacement_registers_before_current_document')
        connection = 'fn set_legacy_mapping_connection() { self.legacy_mapping.container = container; }'
        self.assertFalse(self.inspect(connection, owner))
        self.assertEqual(self.inspect(connection.replace('container; }', 'container; self.start_legacy_mapping_owner(cx); }'), owner)[0]['kind'],
                         'keyboard_connection_starts_with_retained_stale_document')
        self.assertFalse(self.inspect('fn unrelated() { self.set_legacy_mapping_connection(container,cx); }', owner))

    def test_target_and_test_dependency_edges_cannot_smuggle_gui_into_storage(self):
        packages = [{"name": "razer-storage", "internal_dependencies": [{"name": "bridge", "kind": "cfg(windows):test"}]},
                    {"name": "bridge", "internal_dependencies": [{"name": "razer-pages", "kind": "build"}]},
                    {"name": "razer-pages"}]
        rows = workspace.dependency_boundaries(packages)
        self.assertTrue(any(row["owner"] == "razer-storage" and row["chain"] == ["razer-storage", "bridge", "razer-pages"] for row in rows))
        packages[1]["internal_dependencies"][0]["name"] = "razer-model"
        self.assertFalse(workspace.dependency_boundaries(packages))

    def test_ui_foundations_cannot_depend_on_pages_or_application_composition(self):
        for foundation in ("razer-assets", "razer-state", "razer-widgets"):
            for target in ("razer-pages", "razer-app-pages", "razer-dashboard", "razer-settings", "razer-shell", "razer-app"):
                with self.subTest(foundation=foundation, target=target):
                    rows = workspace.dependency_boundaries([{"name": foundation, "internal_dependencies": [{"name": target, "kind": "test"}]}])
                    self.assertEqual(rows[0]["kind"], "presentation_foundation_depends_on_feature")
        self.assertFalse(workspace.dependency_boundaries([{"name": "razer-widgets", "internal_dependencies": [{"name": "razer-model", "kind": "normal"}]}]))
        rows = workspace.dependency_boundaries([{"name": "razer-model", "external_dependencies": [{"name": "gpui", "kind": "build"}]}])
        self.assertEqual(rows[0]["kind"], "non_gui_owner_depends_on_presentation")
        self.assertFalse(workspace.dependency_boundaries([{"name": "razer-resources", "external_dependencies": [{"name": "gpui-pre-reqwest", "kind": "normal"}]}]))

    def test_same_crate_includes_and_installation_generator_are_legal_but_private_cross_crate_includes_fail(self):
        owner = "crates/razer-pages/src/features/workspace.rs"
        self.assertFalse(self.inspect('include!("source.rs");', owner))
        self.assertFalse(self.inspect('include!(concat!(env!("OUT_DIR"), "/installation.rs"));', "crates/razer-resource-db/src/lib.rs"))
        driver = 'include!(concat!(env!("OUT_DIR"), "/hardware_driver_installation.rs"));'
        self.assertFalse(self.inspect(driver, "crates/razer-platform/src/observation/hardware_driver.rs"))
        self.assertEqual(self.inspect(driver, owner)[0]["kind"], "include_path_requires_resolution")
        for source in ('include!("../../../razer-model/src/settings.rs");',
                       '#[cfg(test)] mod tests { include!("../../../razer-model/src/settings.rs"); }'):
            self.assertEqual(self.inspect(source, owner)[0]["kind"], "cross_crate_private_include")
        self.assertEqual(self.inspect('include!(concat!("../", "private.rs"));', owner)[0]["kind"], "include_path_requires_resolution")
        self.assertFalse(self.inspect('const TEXT: &str = r#"include!(\"../../../private.rs\");"#;', owner))

    def test_only_verified_include_fragments_use_an_expression_wrapper_and_preserve_offsets(self):
        source = b'&[include!("../../../razer-model/src/settings.rs")]'
        owner = "crates/razer-pages/src/features/fragment.rs"
        self.assertEqual(workspace.inspect_architecture_source(source, owner=owner, rust_parser=self.rust_parser)[0]["kind"], "architecture_rust_parse_error")
        rows = workspace.inspect_architecture_source(source, owner=owner, rust_parser=self.rust_parser, expression_fragment=True)
        self.assertEqual(rows[0]["kind"], "cross_crate_private_include")
        self.assertEqual(rows[0]["byte_start"], 2)
        self.assertEqual(source[rows[0]["byte_start"]:rows[0]["byte_end"]], b'include!("../../../razer-model/src/settings.rs")')
        self.assertFalse(workspace.inspect_architecture_source(b'&[(1, 2)]', owner=owner, rust_parser=self.rust_parser, expression_fragment=True))
        self.assertEqual(workspace.inspect_architecture_source(b'fn broken {', owner=owner, rust_parser=self.rust_parser, expression_fragment=True)[0]["kind"], "architecture_rust_parse_error")

    def test_native_registration_shortcut_is_a_distinct_contract(self):
        owner = "crates/razer-platform/src/input/global_shortcuts.rs"
        source = 'pub struct Shortcut { pub virtual_key: u32, pub modifiers: u32, pub argument: String }'
        self.assertFalse(self.inspect(source, owner))
        service_owner = "crates/razer-service/src/mapping/hotkeys.rs"
        self.assertEqual(self.inspect(source, service_owner)[0]["kind"], "duplicate_persisted_contract")
        self.assertFalse(self.inspect('pub use razer_platform::global_shortcuts::Shortcut;', service_owner))
        self.assertEqual(self.inspect(source, "crates/razer-pages/src/features/shortcuts.rs")[0]["kind"], "duplicate_persisted_contract")
        self.assertEqual(self.inspect(source.replace('virtual_key', 'input'), owner)[0]["kind"], "duplicate_persisted_contract")

    def test_save_owners_are_captured_before_queue_and_validated_before_rebase(self):
        owner = "crates/razer-shell/src/shell/persistence.rs"
        declaration = ('struct PreparedSave { window: Window, file: WorkspaceFile, owners: Owners, '
                       'product: Option<ProductSave>, device_draft: Option<DeviceSaveDraft> }')
        self.assertFalse(self.inspect(declaration, owner))
        for missing in ('owners: Owners, ', ', device_draft: Option<DeviceSaveDraft>'):
            self.assertEqual(self.inspect(declaration.replace(missing, ''), owner)[0]['kind'],
                             'multiple_product_save_owners')
        self.assertEqual(self.inspect(declaration, 'crates/razer-pages/src/features/workspace.rs')[0]['kind'],
                         'multiple_product_save_owners')
        self.assertFalse(self.inspect(declaration.replace('PreparedSave', 'UnrelatedRequest'),
                                      'crates/razer-pages/src/features/workspace.rs'))
        self.assertEqual(self.inspect('struct PreparedSave { window: Window, file: WorkspaceFile, product: Option<ProductSave> }', owner)[0]["kind"], "multiple_product_save_owners")
        self.assertFalse(self.inspect('fn prepare_save() { PreparedSave { window, file, owners: self.capture_save_owners(cx), product } }', owner))
        self.assertEqual(self.inspect('fn prepare_save() { PreparedSave { window, file, owners: Owners::new(), product } }', owner)[0]["kind"], "uncaptured_save_owners")
        shell = "crates/razer-shell/src/shell.rs"
        self.assertEqual(self.inspect('fn request() { PreparedSave { window, file, owners, product }; }', shell)[0]["kind"], "product_recomposes_save_owners")
        self.assertEqual(self.inspect('type Request = PreparedSave; fn request() { Request { window, file, owners, product }; }', shell)[0]["kind"], "product_recomposes_save_owners")
        valid = 'fn start_pending_save() { if !self.prepared_save_owners_current(&request.owners,cx) { return; } self.rebase_current_dpi_save(request,cx); }'
        self.assertFalse(self.inspect(valid, shell))
        late = 'fn start_pending_save() { self.rebase_current_dpi_save(request,cx); self.prepared_save_owners_current(&request.owners,cx); }'
        self.assertEqual(self.inspect(late, shell)[0]["kind"], "queued_save_not_validated_before_rebase")
        self.assertEqual(self.inspect(valid.replace('return;', 'self.capture_save_owners(cx); return;'), shell)[0]["kind"], "save_owners_recaptured_after_queue")

    def test_save_acknowledgements_bind_the_entity_and_connection_revision(self):
        owner = "crates/razer-shell/src/shell/persistence.rs"
        declaration = 'struct SavedRevisionOwner { workspace: WeakEntity<ProductWorkspace>, connection_revision: u64 }'
        self.assertFalse(self.inspect(declaration, owner))
        self.assertEqual(self.inspect(declaration, 'crates/razer-pages/src/features/workspace.rs')[0]['kind'],
                         'duplicate_save_revision_owner')
        self.assertEqual(self.inspect(declaration.replace(', connection_revision: u64', ''), owner)[0]['kind'],
                         'duplicate_save_revision_owner')
        valid = '''impl SavedRevisionOwner {
            fn current(&self, identity: &str, devices: &[Entity<ProductWorkspace>], cx: &App) -> Option<Entity<ProductWorkspace>> {
                let owner = self.workspace.upgrade()?;
                (devices.contains(&owner) && owner.read(cx).identity(cx) == identity
                    && owner.read(cx).connection_revision() == self.connection_revision).then_some(owner)
            }
        }'''
        self.assertFalse(self.inspect(valid, owner))
        for changed in (valid.replace(' && owner.read(cx).identity(cx) == identity', ''),
                        valid.replace('&& owner.read(cx).connection_revision() == self.connection_revision', ''),
                        valid.replace('self.connection_revision', 'other.connection_revision'),
                        valid.replace('self.workspace.upgrade()?', 'other.workspace.upgrade()?'),
                        valid.replace('.then_some(owner)', '.then_some(owner); Some(owner)'),
                        valid.replace('self.connection_revision', 'self.connection_revision || true')):
            self.assertIn('save_acknowledgement_ignores_connection_revision',
                          [row['kind'] for row in self.inspect(changed, owner)])
        self.assertFalse(self.inspect(valid.replace('SavedRevisionOwner', 'UnrelatedOwner'), owner))
        capture = '''fn capture_save_owners(&self, cx: &App) -> Owners {
            self.devices.iter().map(|owner| (owner.read(cx).identity(cx), SavedRevisionOwner {
                workspace: owner.downgrade(), connection_revision: owner.read(cx).connection_revision()
            })).collect()
        }'''
        self.assertFalse(self.inspect(capture, owner))
        for changed in (capture.replace('owner.read(cx).connection_revision()', '0'),
                        capture.replace('owner.read(cx).connection_revision()', 'other.read(cx).connection_revision()'),
                        capture.replace('owner.downgrade()', 'other.downgrade()')):
            self.assertIn('save_owner_epoch_is_not_captured_with_entity',
                          [row['kind'] for row in self.inspect(changed, owner)])

    def test_moving_a_catalog_owner_preserves_installed_identity_and_constant(self):
        original = {"crate": "razer-pages", "macro": "shortcuts_current", "symbol": "SHORTCUTS_CURRENT", "catalog_id": "razer-pages/shortcuts_current"}
        moved = {**original, "crate": "razer-model"}
        self.assertEqual(catalog_data.catalog_identity(moved), "razer-pages/shortcuts_current")
        self.assertEqual(catalog_data.constant(moved), catalog_data.constant(original))
        self.assertTrue(catalog_data.selected(original))
        self.assertTrue(catalog_data.selected(moved))
        self.assertFalse(catalog_data.selected({"crate": "razer-model", "symbol": "PAIRING_DATA"}))
        recording = {"crate": "razer-model", "symbol": "KEYBOARD_DATA", "macro": "recording_keyboard_data",
                     "catalog_id": "razer-app-pages/recording_keyboard_data"}
        self.assertTrue(catalog_data.selected(recording))
        self.assertEqual(catalog_data.constant(recording), "RAZER_APP_PAGES_RECORDING_KEYBOARD_DATA")
        self.assertFalse(catalog_data.selected({**recording, "macro": "protocol_data"}))
        capture = {"crate": "razer-model", "symbol": "SHORTCUT_CAPTURE_KEYS", "macro": "shortcut_capture_keys"}
        self.assertFalse(catalog_data.selected(capture))

    def test_catalog_reader_replacement_uses_installed_identity_after_owner_moves(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(catalog_data, "ROOT", Path(directory)):
            path = Path(directory) / "shortcuts.rs"
            marker = "// Generated catalog reader: razer-pages/shortcuts_current.\n"
            ending = "// End generated catalog reader: razer-pages/shortcuts_current.\n"
            path.write_bytes(("fn retained() {}\n" + marker + "fn read_shortcuts_current_catalog() { old(); }\n" + ending).replace("\n", "\r\n").encode())
            body = (marker + "fn read_shortcuts_current_catalog() { updated(); }\n" + ending).encode()
            artifacts = {}
            catalog_data.place_reader(artifacts, {"crate": "razer-model", "macro": "shortcuts_current", "catalog_id": "razer-pages/shortcuts_current", "module": "shortcuts.rs"}, body)
            self.assertEqual(artifacts[path], b"fn retained() {}\n" + body)

    def test_catalog_identity_generator_formats_long_names_and_preserves_installed_identity(self):
        row = {"crate": "razer-model", "macro": "capture",
               "catalog_id": "razer-app-pages/recording_keyboard_data_with_a_long_stable_installed_identity"}
        body = catalog_data.catalog_identities([row])
        source = body.decode()
        self.assertIn('CatalogId("' + row['catalog_id'] + '")', source)
        self.assertNotIn('razer-model/capture', source)
        self.assertTrue(all(len(line) <= 100 for line in source.splitlines()))
        self.assertEqual(body, catalog_data.format_rust(source, context="identity regeneration"))

    def test_current_workspace_uses_the_guarded_owners(self):
        for owner in ("crates/razer-shell/src/shell/persistence.rs", "crates/razer-shell/src/shell/shutdown.rs", "crates/razer-resources/src/runtime.rs", "crates/razer-assets/src/resource_runtime.rs", "crates/razer-model/src/shortcuts.rs",
                      "crates/razer-model/src/shortcuts/native.rs", "crates/razer-device/src/mapping/mapping_engine.rs",
                      "crates/razer-shell/src/shell/devices/product_mapping.rs", "crates/razer-service/src/mapping/mod.rs",
                      "crates/razer-service/src/mapping/device.rs", "crates/razer-service/src/mapping/key_capture.rs",
                      "crates/razer-shell/src/shell.rs", "crates/razer-shell/src/shell/devices/device_discovery.rs",
                       "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore.rs",
                       "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_publisher.rs",
                       "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_worker.rs",
                      "crates/razer-shell/src/shell/keyboard/keyboard_actuation_write.rs",
                      "crates/razer-shell/src/shell/keyboard/keyboard_actuation_worker.rs",
                      "crates/razer-shell/src/shell/keyboard/keyboard_lifecycle.rs",
                      "crates/razer-shell/src/shell/keyboard/keyboard_brightness_read.rs",
                      "crates/razer-shell/src/shell/keyboard/keyboard_brightness_write.rs",
                      "crates/razer-shell/src/shell/keyboard/keyboard_brightness_runtime.rs",
                      "crates/razer-shell/src/shell/keyboard/keyboard_brightness_worker.rs",
                      "crates/razer-pages/src/features/keyboard/lighting/keyboard_brightness.rs",
                      "crates/razer-platform/src/platform/windows/session_changes.rs",
                      "crates/razer-platform/src/camera/camera_preview.rs",
                      "crates/razer-platform/src/camera/camera_preview/windows.rs",
                      "crates/razer-service/src/camera/commands.rs", "crates/razer-service/src/camera/lifecycle.rs",
                      "crates/razer-pages/src/features/workspace/source_controls/camera_runtime.rs",
                      "crates/razer-pages/src/features/workspace/source_controls/camera_media.rs",
                      "crates/razer-pages/src/features/workspace/source_controls/camera_mic.rs",
                      "crates/razer-pages/src/features/workspace/source_controls/camera_source_state.rs",
                      "crates/razer-pages/src/features/workspace/source_controls/camera_shortcuts.rs",
                      "crates/razer-pages/src/features/workspace/source_workspace.rs",
                       "crates/razer-platform/src/observation/device_changes.rs",
                       "crates/razer-settings/src/runtime_page.rs", "crates/razer-shell/src/shell/mouse/mouse_dpi_70.rs",
                      "crates/razer-shell/src/shell/audio/thx_sound.rs",
                      "crates/razer-shell/src/shell/mouse/mouse_matcher.rs",
                      "crates/razer-shell/src/shell/receiver/receiver_profile_save.rs",
                      "crates/razer-service/src/mouse/dpi_matcher_worker.rs",
                      "crates/razer-service/src/runtime/windows/mouse/input_monitoring.rs",
                      "crates/razer-app-pages/src/macros/macro_page.rs", "crates/razer-app-pages/src/macros/macro_page/transfer.rs",
                      "crates/razer-app-pages/src/macros/macro_page/recording.rs", "crates/razer-service/src/mapping/recorder.rs",
                      "crates/razer-model/src/macro_library/recording.rs",
                      "crates/razer-model/src/shortcuts/capture_keys.rs", "crates/razer-model/src/shortcuts/capture.rs",
                      "crates/razer-ipc/src/mapping_control.rs", "crates/razer-ipc/src/shortcuts.rs",
                       "crates/razer-ipc/src/lib.rs",
                      "crates/razer-service/src/mapping/control.rs", "crates/razer-service/src/mapping/shortcuts.rs",
                      "crates/razer-service/src/runtime/windows/mapping_runtime/backend.rs",
                      "crates/razer-service/src/runtime/windows/mapping_runtime/device.rs",
                      "crates/razer-ipc/src/global_shortcut_capture.rs",
                      "crates/razer-service/src/runtime/windows/mapping_runtime/capture.rs",
                      "crates/razer-pages/src/features/workspace/product_workspace.rs",
                      "crates/razer-pages/src/features/workspace/source_workspace/lifecycle.rs",
                      "crates/razer-ipc/src/macro_recording.rs", "crates/razer-service/src/runtime/windows/mapping_runtime/capture/recorder.rs",
                      "crates/razer-device/src/profile_transfer.rs",
                      "crates/razer-model/src/profile_transfer.rs", "crates/razer-storage/src/file_transfer.rs",
                      "crates/razer-storage/src/discovery_diagnostics.rs", "crates/razer-platform/src/observation/discovery_diagnostics.rs",
                      "crates/razer-platform/src/observation/executable.rs", "crates/razer-assets/src/local_image.rs",
                      "crates/razer-settings/src/runtime_page/diagnostics.rs",
                      "crates/razer-pages/src/features/workspace/source_controls/oled_crop.rs",
                      "crates/razer-pages/src/features/workspace/source_controls/oled_presets.rs",
                      "crates/razer-pages/src/features/profiles/linked_games.rs",
                      "crates/razer-pages/src/features/workspace/source_workspace/profile_linked_games.rs",
                      "crates/razer-storage/src/workspace/retired_shortcuts.rs", "crates/razer-storage/src/ui_preferences.rs",
                      "crates/razer-state/src/ui_preferences.rs", "crates/razer-pages/src/features/keyboard/mod.rs",
                      "crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration.rs",
                       "crates/razer-pages/src/features/keyboard/mapping/keyboard_mapping_runtime_515.rs",
                      "crates/razer-pages/src/features/keyboard/calibration/keyboard_calibration_v2.rs",
                      "crates/razer-pages/src/features/accessories/hue.rs", "crates/razer-pages/src/features/accessories/hue/effects.rs",
                      "crates/razer-app-pages/src/armory/armory_page.rs"):
            with self.subTest(owner=owner):
                self.assertFalse(workspace.inspect_architecture_source((workspace.ROOT / owner).read_bytes(), owner=owner, rust_parser=self.rust_parser))


class ReceiverLocalEvidenceTests(unittest.TestCase):
    def fixture(self, directory, *, receiver='runtime.razerWindow.rzDevice', status='4',
                parameters='()', runtime_module='95524', flag='scanning=false', second_call='', guarded=False):
        root = Path(directory)
        name = '.analysis/inputs/frontend/receiver-current/raw/main.js'
        raw = ('self.webpackChunk.push([[1],{34340:(mod,exports,require)=>{'
               'const runtime=require(' + runtime_module + ');let scanning=true;'
               'const commands={DUALLINK_CANCEL:' + parameters + '=>Promise.resolve().then(async()=>{'
               'const target=' + receiver + ';' + flag + ';' + ('if(scanning)' if guarded else '') + 'await target.deviceScan(' + status + ');'
               + second_call + '})};require.d(exports,{ZP:()=>commands});}}]);').encode()
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return root, {'path': name, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}

    def query(self, root, source):
        import host
        audit = host.HostLocalAudit()
        with mock.patch.object(families.a, 'ROOT', root):
            exported = audit.webpack_export('command_table', [source], 34340, 'ZP')
            owner = audit.retain('cancel', source, 'DUALLINK_CANCEL:', {'pair'})[0]
            contract = families.receiver_cancel_scan_contract(audit, source, owner)
            return audit, exported, contract

    def test_arrow_factory_retains_complete_cancel_and_resolves_primary_runtime(self):
        for flag in ('scanning=false', 'scanning=!1'):
            with self.subTest(flag=flag), tempfile.TemporaryDirectory() as directory:
                root, source = self.fixture(directory, flag=flag)
                audit, exported, contract = self.query(root, source)
                self.assertEqual(contract['target'], 'primary_runtime_device')
                self.assertEqual(contract['runtime_module_id'], 95524)
                self.assertEqual(contract['scan_status'], 4)
                self.assertFalse(contract['category_parameter'])
                factory = audit.owners[exported['complete_factory_owner']]
                self.assertTrue(factory['body'].startswith('34340:(mod,exports,require)=>'))
                cancel = audit.owners[contract['cancel_owner']]
                self.assertTrue(cancel['body'].startswith('DUALLINK_CANCEL:()=>'))
                self.assertIn('await target.deviceScan(4)', cancel['body'])
                self.assertIn('const runtime=require(95524)', factory['body'])
                self.assertEqual(audit.owners[contract['scan_call_owner']]['body'], 'target.deviceScan(4)')
                self.assertEqual(audit.owners[contract['scanning_flag_cleanup_owner']]['body'], flag)

    def test_category_selected_or_foreign_runtime_cannot_be_reported_as_primary_cancel(self):
        for mutation in ({'receiver': 'pickFactory(category,true)'},
                         {'receiver': 'runtime.razerWindow.secondaryDevice'},
                         {'receiver': 'runtime.razerWindow["rzDevice"]'},
                         {'runtime_module': '95525'}):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root, source = self.fixture(directory, **mutation)
                with self.assertRaises(ValueError):
                    self.query(root, source)

    def test_changed_payload_ambiguous_dispatch_or_missing_cleanup_requires_source_review(self):
        for mutation in ({'status': '1'}, {'status': 'category'}, {'status': '4,category'},
                         {'parameters': '(category)'}, {'flag': 'scanning=true'},
                         {'second_call': 'await target.deviceScan(4);'}, {'guarded': True}):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root, source = self.fixture(directory, **mutation)
                with self.assertRaises(ValueError):
                    self.query(root, source)

    def test_receipt_cannot_retain_replaced_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root, source = self.fixture(directory)
            (root / source['path']).write_bytes(b'changed original source')
            with self.assertRaises(ValueError):
                self.query(root, source)


class ReceiverChromaEvidenceTests(unittest.TestCase):
    def prepare(self, directory):
        root = Path(directory)
        def save(name, value):
            raw = value if isinstance(value, bytes) else json.dumps(value).encode()
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        package = save('.analysis/inputs/official-native-current/blobs/core/core.exe', b'verified static package')
        package.update(status='verified', url='https://app-assets.razer.com/files/core.exe',
                       declared_sha256=package['sha256'], declared_bytes=package['bytes'])
        name = 'ExtraInstaller_Razer Chroma SDK Core'
        document = {'resourceManifest': {'resources': [{'resourceName': name, 'resourceVersion': '1.0',
            'url': package['url'], 'size': package['bytes'], 'sha256': package['sha256'].upper()}]}}
        manifest = save('.analysis/inputs/official-native-current/manifests/current/manifest.json', document)
        manifest.update(url='https://apps.razer.com/chroma-app/dashboard/manifest.json',
                        resource_name=name, resource_version='1.0')
        package['manifests'] = [manifest]
        original = save('.analysis/inputs/official-native-current/extracted/core/RzAPIChromaSDK.dll', b'verified native bytes')
        original.update(container=package['path'], container_sha256=package['sha256'],
            declared_container_sha256=package['sha256'], container_extraction_complete=True,
            verified_against_manifest=True)
        server = save('.analysis/inputs/official-native-current/extracted/core/RzSDKServer.exe', b'verified server bytes')
        server.update(container=package['path'], container_sha256=package['sha256'],
            declared_container_sha256=package['sha256'], container_extraction_complete=True,
            verified_against_manifest=True)
        acquisition = {'resources': [package], 'stability': [{'url': manifest['url'],
            'before_sha256': manifest['sha256'], 'after_sha256': manifest['sha256'], 'unchanged': True}]}
        extraction = {'container': {key: package[key] for key in ('path', 'bytes', 'sha256')},
            'extracted': True, 'extraction_exit_code': 0, 'listing_exit_code': 0,
            'input_bytes_unchanged': True, 'duplicate_entry_paths': {}, 'target_execution': False,
            'files': [original, server]}
        acquisition_name = '.analysis/inputs/official-native-current/acquisition.json'
        extraction_name = '.analysis/results/native-packages/' + package['sha256'] + '/inspection.json'
        save(acquisition_name, acquisition)
        save(extraction_name, extraction)
        capture = save('.analysis/results/native/native/capture.json', {})
        return {'root': root, 'save': save, 'package': package, 'manifest': manifest, 'original': original, 'server': server,
            'acquisition': acquisition, 'extraction': extraction, 'acquisition_name': acquisition_name,
            'extraction_name': extraction_name, 'native': {'capture_receipt': capture, 'complete_selected_functions': []}}

    def query(self, fixture):
        import host
        audit = host.HostLocalAudit()
        with mock.patch.object(families.a, 'ROOT', fixture['root']), \
                mock.patch.object(families, 'verify_native_functions', return_value=fixture['native']) as native:
            result = families.receiver_chroma_connect_native_sources(audit)
            return result, native.call_args_list

    def test_current_manifest_extraction_and_complete_native_roles_are_bound_together(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = self.prepare(directory)
            result, calls = self.query(fixture)
            self.assertEqual(len(calls), 2)
            self.assertEqual(calls[0].args[0], fixture['original'])
            self.assertTrue({0x22820, 0x1530, 0x22840, 0x1990, 0x22bc0, 0x48e0, 0x22fc0, 0x4bb0,
                             0x22c50, 0x22d50, 0x22f50, 0x62a0} <= set(calls[0].args[1]))
            self.assertEqual(calls[0].kwargs['exports']['UnInit'], 0x22840)
            self.assertEqual(calls[1].args[0], fixture['server'])
            self.assertTrue({0x73c30, 0x452f0, 0x75280, 0x76ba0, 0x46440, 0x46c20, 0x4cff0,
                             0x4f020, 0x524a0, 0x44df0, 0x72ac0, 0x76130} <= set(calls[1].args[1]))
            self.assertEqual(len(calls[1].kwargs['slots']), 7)
            self.assertEqual(next(row for row in calls[1].kwargs['slots']
                                  if row['role'] == 'endpoint_ProcessMessage')['target_rva'], 0x452f0)
            self.assertFalse(result['target_execution'])
            self.assertEqual(result['runtime_acceptance'], 'unverified')
            self.assertEqual(result['selected_dispatch']['status'], 'statically_traced')
            self.assertEqual(result['unresolved'], [])
            self.assertIn('retaining the old endpoint', result['semantics']['endpoint_release'])
            self.assertIn('do not provide independent cleanup confirmation', result['limitations'][0])

    def test_package_replacement_and_unstable_manifest_do_not_establish_current_provenance(self):
        for mutation in ('package_bytes', 'manifest_bytes', 'changed_deployment', 'declared_size', 'duplicate_package'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                fixture = self.prepare(directory)
                if mutation == 'package_bytes':
                    fixture['save'](fixture['package']['path'], b'replaced package')
                elif mutation == 'manifest_bytes':
                    fixture['save'](fixture['manifest']['path'], {'resourceManifest': {'resources': []}})
                elif mutation == 'changed_deployment':
                    fixture['acquisition']['stability'][0]['unchanged'] = False
                elif mutation == 'declared_size':
                    fixture['package']['declared_bytes'] += 1
                else:
                    fixture['acquisition']['resources'].append(copy.deepcopy(fixture['package']))
                fixture['save'](fixture['acquisition_name'], fixture['acquisition'])
                with self.assertRaises(ValueError):
                    self.query(fixture)

    def test_failed_extraction_or_replaced_member_cannot_supply_cleanup_evidence(self):
        for mutation in ('member_bytes', 'container', 'incomplete', 'duplicate_member', 'failed', 'target_executed'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                fixture = self.prepare(directory)
                if mutation == 'member_bytes':
                    fixture['save'](fixture['original']['path'], b'replaced DLL')
                elif mutation == 'container':
                    fixture['original']['container_sha256'] = '0' * 64
                elif mutation == 'incomplete':
                    fixture['original']['container_extraction_complete'] = False
                elif mutation == 'duplicate_member':
                    fixture['extraction']['files'].append(copy.deepcopy(fixture['original']))
                elif mutation == 'failed':
                    fixture['extraction']['extraction_exit_code'] = 1
                else:
                    fixture['extraction']['target_execution'] = True
                fixture['save'](fixture['extraction_name'], fixture['extraction'])
                with self.assertRaises(ValueError):
                    self.query(fixture)

    def test_server_member_requires_its_own_manifest_bound_identity_and_extraction(self):
        for mutation in ('member_bytes', 'container', 'incomplete', 'duplicate_member', 'absent'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                fixture = self.prepare(directory)
                if mutation == 'member_bytes':
                    fixture['save'](fixture['server']['path'], b'replaced server')
                elif mutation == 'container':
                    fixture['server']['declared_container_sha256'] = '0' * 64
                elif mutation == 'incomplete':
                    fixture['server']['verified_against_manifest'] = False
                elif mutation == 'duplicate_member':
                    fixture['extraction']['files'].append(copy.deepcopy(fixture['server']))
                else:
                    fixture['extraction']['files'].remove(fixture['server'])
                fixture['save'](fixture['extraction_name'], fixture['extraction'])
                with self.assertRaises(ValueError):
                    self.query(fixture)


class NativeDispatchEvidenceTests(unittest.TestCase):
    def prepare(self, directory, *, export_rva=0x1700, slot_rva=0x1700, body_byte=b'\xc3', forwarder=False):
        import struct
        from test_host import pe_fixture
        root = Path(directory)
        def save(name, value):
            raw = value if isinstance(value, bytes) else json.dumps(value).encode()
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {'path': name, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
        raw, data_directory = pe_fixture()
        struct.pack_into('<II', raw, data_directory, 0x1200, 0x100)
        struct.pack_into('<IIHHIIIIIII', raw, 0x400, 0, 0, 0, 0, 0, 1, 1, 1, 0x1250, 0x1260, 0x1270)
        struct.pack_into('<I', raw, 0x450, 0x1280 if forwarder else export_rva)
        struct.pack_into('<I', raw, 0x460, 0x1290)
        struct.pack_into('<H', raw, 0x470, 0)
        raw[0x480:0x48d] = b'OTHER.Target\0'
        raw[0x490:0x497] = b'UnInit\0'
        image_base = 0x180000000
        struct.pack_into('<Q', raw, 0x800, image_base + slot_rva)
        raw[0x900:0x901] = b'\xc3'
        original = save('.analysis/inputs/native.dll', bytes(raw))
        capture_root = '.analysis/results/native/' + original['sha256'] + '/'
        address = image_base + 0x1700
        function = {'rva': '0x1700', 'address': hex(address), 'name': 'UnInit',
            'chunks': [{'start': hex(address), 'end': hex(address + 1), 'raw_hex': body_byte.hex(),
                        'sha256': hashlib.sha256(body_byte).hexdigest()}],
            'assembly': [{'address': hex(address), 'text': 'ret'}], 'pseudocode': None}
        artifact = save(capture_root + 'functions/1700.json', function)
        save(capture_root + 'capture.json', {'input_sha256': original['sha256'], 'input_bytes': original['bytes'],
            'image_base': hex(image_base), 'capture_complete': True, 'functions_captured': 1, 'functions_total': 1,
            'ida_version': 'synthetic byte fixture', 'hexrays_version': None, 'capture_tool': 'unit fixture',
            'failures': ['retained unavailable pseudocode'], 'functions': [
                {'rva': '0x1700', 'address': hex(address), 'name': 'UnInit', 'path': 'functions/1700.json',
                 'sha256': artifact['sha256'], 'assembly_captured': True, 'pseudocode_captured': False,
                 'new_failures': ['retained unavailable pseudocode']} ]})
        return root, original

    def verify(self, root, original):
        with mock.patch.object(families.a, 'ROOT', root):
            return families.verify_native_functions(original, {0x1700: 'UnInit'}, slots=(
                {'role': 'ProcessMessage', 'vtable_rva': 0x1600, 'slot_byte_offset': 0, 'target_rva': 0x1700},
            ), exports={'UnInit': 0x1700})

    def test_export_slot_and_full_function_bytes_are_verified_without_executing_native_code(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.verify(*self.prepare(directory))
            self.assertTrue(result['verified_exports'][0]['actual_pe_bytes_verified'])
            self.assertTrue(result['verified_indirect_slots'][0]['actual_pe_bytes_verified'])
            self.assertTrue(result['complete_selected_functions'][0]['input_chunks_verified'])
            self.assertEqual(result['failures'], ['retained unavailable pseudocode'])
            self.assertFalse(result['complete_selected_functions'][0]['pseudocode_captured'])
            self.assertFalse(result['target_execution'])

    def test_changed_exports_forwarders_indirect_slots_and_captured_bytes_are_rejected(self):
        for mutation, kwargs in (
            ('export', {'export_rva': 0x1710}), ('forwarder', {'forwarder': True}),
            ('slot', {'slot_rva': 0x1710}), ('function bytes', {'body_byte': b'\x90'}),
        ):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                with self.assertRaises(ValueError):
                    self.verify(*self.prepare(directory, **kwargs))


class ConnectedDeviceVerificationTests(unittest.TestCase):
    def setUp(self):
        scope = mock.patch.object(workspace, 'rust_test_scope', return_value={'targets': [], 'window_tests_selected': 0})
        scope.start()
        self.addCleanup(scope.stop)

    def test_native_comparison_uses_correlated_terminal_response_after_busy_reads(self):
        outgoing = [0] * 91
        outgoing[2], outgoing[7:9] = 7, [4, 133]
        busy = outgoing.copy()
        busy[1] = 1
        complete = outgoing.copy()
        complete[1], complete[10:14] = 2, [7, 208, 7, 208]
        transfers = [
            {'operation':'send_feature', 'bytes':outgoing, 'count':91, 'error':None},
            {'operation':'get_feature', 'bytes':busy, 'count':91, 'error':None},
            {'operation':'get_feature', 'bytes':busy, 'count':91, 'error':None},
            {'operation':'get_feature', 'bytes':complete, 'count':91, 'error':None},
        ]
        self.assertEqual(workspace.connected_get_response(transfers, 0), (complete, 3))
        for invalid in [transfers[:2], transfers[:1], transfers[:2] + transfers[:1]]:
            with self.assertRaisesRegex(ValueError, 'terminal IN'):
                workspace.connected_get_response(invalid, 0)
        for offset in (2, 7, 8):
            wrong = copy.deepcopy(transfers)
            wrong[1]['bytes'][offset] ^= 1
            with self.assertRaisesRegex(ValueError, 'transaction or command'):
                workspace.connected_get_response(wrong, 0)

    def test_write_deadline_waits_for_restoration_without_killing_the_owner(self):
        process = mock.Mock(pid=123, wait=mock.Mock(side_effect=[workspace.subprocess.TimeoutExpired('test', 240), 1]))
        output = io.BytesIO()
        with mock.patch.object(workspace.subprocess, 'run') as kill:
            code, exceeded = workspace.wait_connected_process(process, output, write_restore=True)
        self.assertEqual((code, exceeded), (1, True))
        self.assertEqual(process.wait.call_args_list, [mock.call(timeout=240), mock.call()])
        self.assertIn(b'waiting for original-value restoration', output.getvalue())
        kill.assert_not_called()

    def test_read_deadline_reclaims_and_waits_for_the_whole_test_process(self):
        process = mock.Mock(pid=123, wait=mock.Mock(side_effect=[workspace.subprocess.TimeoutExpired('test', 240), 1]))
        output = io.BytesIO()
        with mock.patch.object(workspace.subprocess, 'run') as kill:
            self.assertEqual(workspace.wait_connected_process(process, output, write_restore=False), (1, True))
        self.assertEqual(kill.call_args.args[0], ['taskkill', '/PID', '123', '/T', '/F'])
        self.assertEqual(process.wait.call_args_list, [mock.call(timeout=240), mock.call(timeout=30)])

    def test_incomplete_runtime_output_preserves_attempted_hardware_and_unknown_restoration(self):
        admitted = 'RAZER_CONNECTED_STARTED {"mode":"write-restore","products":[679]}\n'
        scope = workspace.connected_execution_scope(admitted, None, write_restore=True)
        self.assertTrue(scope['target_execution_attempted'])
        self.assertIsNone(scope['target_execution'])
        self.assertIsNone(scope['real_device_operations'])
        raw = admitted + 'RAZER_CONNECTED_BASELINE {"product_id":679,"baseline":{"raw_value":255},"target_percent":99}\n'
        scope = workspace.connected_execution_scope(raw, None, write_restore=True)
        self.assertTrue(scope['real_device_operations'])
        self.assertTrue(scope['mutation_may_have_started'])
        self.assertEqual(scope['restoration_confirmation'], 'unconfirmed')
        self.assertEqual(scope['write_baselines'][0]['baseline']['raw_value'], 255)
        scope = workspace.connected_execution_scope('compilation failed', None, write_restore=True)
        self.assertFalse(scope['target_execution_attempted'])
        self.assertFalse(scope['real_device_operations'])

    def test_native_comparison_rejects_unstable_or_stale_rust_trace_before_native_execution(self):
        import native_runtime
        baseline = [{'path':'crates/razer-hid/tests/connected_devices.rs', 'bytes':10, 'sha256':'current'}]
        for inputs, unchanged, released in [([], True, True), (baseline, False, True), (baseline, True, False)]:
            with self.subTest(inputs=inputs, unchanged=unchanged, released=released), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                trace = root / '.analysis/results/connected-device-verification.json'
                trace.parent.mkdir(parents=True)
                trace.write_text(json.dumps({'mode':'read-only', 'inputs':inputs, 'inputs_unchanged':unchanged,
                                             'result':{'devices':[], 'backend_released':released}}), encoding='utf-8')
                with mock.patch.object(workspace, 'ROOT', root), \
                        mock.patch.object(workspace, 'rust_inputs', return_value=baseline), \
                        mock.patch.object(native_runtime, 'replay_get_trace') as replay:
                    with self.assertRaisesRegex(ValueError, 'unchanged Rust inputs|backend release'):
                        workspace.compare_connected_hid('verified-library')
                replay.assert_not_called()

    def test_native_comparison_pins_the_trace_bytes_during_native_queries(self):
        import native_runtime
        baseline = [{'path':'fixture.rs', 'bytes':10, 'sha256':'current'}]
        packet, reply = [0] * 91, [0] * 91
        reply[1] = 2
        document = {'mode':'read-only', 'inputs':baseline, 'inputs_unchanged':True,
                    'result':{'backend_released':True, 'devices':[{'product_id':679, 'error':None,
                        'result':{'path':'source-collection', 'transfers':[
                            {'operation':'send_feature', 'bytes':packet, 'error':None, 'count':91},
                            {'operation':'get_feature', 'bytes':reply, 'error':None, 'count':91}]}}]}}
        for replace in [False, True]:
            with self.subTest(replace=replace), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = root / '.analysis/results/connected-device-verification.json'
                source.parent.mkdir(parents=True)
                source.write_text(json.dumps(document), encoding='utf-8')
                original_hash = workspace.digest(source)
                def completed(*args, **kwargs):
                    if replace:
                        source.write_text('{}', encoding='utf-8')
                    return {'status':'completed', 'worker_waited':True, 'result':{
                        'init_return':0, 'queries':[{'trace_index':'679:0', 'send_return':91,
                                                   'get_return':91, 'received_bytes':reply}],
                        'cleanup':[{'operation':'hid_exit', 'return_value':0, 'returned':True},
                                   {'operation':'release_library', 'returned':True}]}}
                with mock.patch.object(workspace, 'ROOT', root), \
                        mock.patch.object(workspace, 'rust_inputs', return_value=baseline), \
                        mock.patch.object(native_runtime, 'validate_get_trace'), \
                        mock.patch.object(native_runtime, 'replay_get_trace', side_effect=completed), \
                        mock.patch.object(workspace, 'write_json') as save, \
                        mock.patch('sys.stdout', new=io.StringIO()):
                    self.assertEqual(workspace.compare_connected_hid('verified-library'), int(replace))
                receipt = save.call_args.args[1]
                self.assertEqual(receipt['source_trace_sha256'], original_hash)
                self.assertEqual(receipt['source_trace_unchanged'], not replace)
                self.assertEqual(receipt['passed'], not replace)

    def test_connected_result_validation_suite_never_selects_ignored_hardware_tests(self):
        def completed(command, **kwargs):
            kwargs['stdout'].write(b'test result: ok. 2 passed; 0 failed; 0 ignored\n')
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, 'ROOT', Path(directory)), \
                mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace, 'verification_environment', side_effect=lambda _: nullcontext((None, None))), \
                mock.patch.object(workspace.subprocess, 'run', side_effect=completed) as runner, \
                mock.patch.object(workspace, 'write_json'), \
                mock.patch('sys.stdout', new=io.StringIO()):
            workspace.validate_rust(selected_suites=['connected-verification-boundary'])
        command = runner.call_args.args[0]
        self.assertEqual(command[-3:], ['--test', 'connected_devices', 'connected_result_validation_'])
        self.assertNotIn('--ignored', command)

    def test_real_test_harness_prefix_is_accepted_without_losing_its_report(self):
        document = {"mode":"read-only", "devices":[], "backend_released":True}
        output = "running 1 test\ntest connected_device_queries ... RAZER_CONNECTED_RESULT " + json.dumps(document) + "\nok\n"
        self.assertEqual(workspace.connected_result(output), document)
        self.assertIsNone(workspace.connected_result("test result: ok. 1 passed; 0 failed\n"))
        self.assertIsNone(workspace.connected_result(output + output))

    def test_selected_current_observation_suites_only_run_pure_filters(self):
        expected = {
            'macro-recorder-platform': ('razer-platform', 'macro_recorder'),
            'current-dpi-observations': ('razer-pages', 'current_dpi_observation_'),
            'current-dpi-initialization': ('razer-pages', 'current_dpi::initialization_tests'),
            'dpi-row-hover-state': ('razer-pages', 'dpi_row_hover_'),
            'current-power-state': ('razer-pages', 'current_power::tests'),
            'current-idle-state': ('razer-pages', 'mouse_polling_profile::idle_tests'),
            'current-power-shell': ('razer-shell', 'mouse_current_power_runtime'),
            'current-power-protocol': ('razer-device', 'mouse_current_power::tests'),
            'current-power-routing': ('razer-discovery', 'direct::mouse_current_power::tests'),
            'current-power-ipc': ('razer-ipc', 'device_tasks::tests::current_power_ipc'),
            'current-power-service': ('razer-service', 'mouse_current_power::tests'),
            'current-polling-observations': ('razer-pages', 'polling::observation_tests'),
            'current-polling-runtime-state': ('razer-pages', 'mouse_polling::runtime_observation_tests'),
            'current-polling-profile-state': ('razer-pages', 'mouse_polling_profile::profile_polling_observation_tests'),
            'current-polling-indicator-state': ('razer-pages', 'unobserved_polling_has_no_rate_color_and_confirmed_rate_uses_its_source_color'),
            'current-polling-shell': ('razer-shell', 'mouse_polling_runtime'),
            'current-dpi-routing': ('razer-discovery', 'direct::mouse_current_dpi::tests'),
        }
        def completed(command, **kwargs):
            kwargs['stdout'].write(b'test result: ok. 1 passed; 0 failed; 0 ignored\n')
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, 'ROOT', Path(directory)), \
                mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace, 'verification_environment', side_effect=lambda _: nullcontext((None, None))), \
                mock.patch.object(workspace.subprocess, 'run', side_effect=completed) as runner, \
                mock.patch.object(workspace, 'write_json'), \
                mock.patch('sys.stdout', new=io.StringIO()):
            workspace.validate_rust(selected_suites=list(expected))
        self.assertEqual(len(runner.call_args_list), len(expected))
        actual_filters = []
        for call in runner.call_args_list:
            command = call.args[0]
            actual_filters.append((command[command.index('-p') + 1], command[-1]))
            self.assertIn('--lib', command)
            self.assertNotIn('--ignored', command)
        self.assertCountEqual(actual_filters, expected.values())


class LocalRuntimeReceiptTests(unittest.TestCase):
    def test_app_comparison_records_failure_without_overwriting_other_evidence(self):
        import native_runtime
        receipt = {"evidence": {"apps_inventory": {"complete": True}}, "mode": "read-only",
            "passed": False, "result": None, "exit_code": 1, "timed_out": False}
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, "ROOT", Path(directory)), \
                mock.patch.object(native_runtime, "compare_local_app_files", return_value=receipt), \
                mock.patch("sys.stdout", new=io.StringIO()):
            audit = Path(directory) / ".analysis/results/host-current/local-audit.json"
            workspace.write_json(audit, {"existing_query": {"complete": False}})
            self.assertEqual(workspace.compare_local_app_files(), 1)
            self.assertEqual(json.loads(audit.read_bytes())["existing_query"], {"complete": False})
            validation = json.loads((Path(directory) / ".analysis/validation.json").read_bytes())
            self.assertFalse(validation["local_runtime_checks"]["simple_app_inventory"]["passed"])
            self.assertNotIn("passed", validation)

    def test_scoped_runtime_receipt_preserves_static_checks_and_other_operations(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(workspace, "ROOT", Path(directory)):
            path = Path(directory) / ".analysis/validation.json"
            workspace.write_json(path, {"schema_version": 1, "passed": False,
                "commands": [{"name": "check", "passed": False}],
                "local_runtime_checks": {"other": {"passed": False}}})
            workspace.record_local_runtime("simple_version", {"passed": True, "mode": "read-only"})
            result = json.loads(path.read_bytes())
            self.assertFalse(result["passed"])
            self.assertEqual(result["commands"], [{"name": "check", "passed": False}])
            self.assertEqual(result["local_runtime_checks"], {
                "other": {"passed": False}, "simple_version": {"passed": True, "mode": "read-only"}})

    def test_service_original_is_indexed_only_after_byte_and_signature_verification(self):
        import analysis
        raw = b"private verified service fixture"
        name = ".analysis/inputs/installed-current/files/elevation-service/razer_elevation_service.exe"
        receipt = {"recorded_at_utc": "2026-10-18T00:00:00Z",
            "evidence": {"com_identity": {"clsid": {"value": "verified-clsid"}}},
            "server": {"input": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                "architecture": "x64", "versions": [],
                "signature": {"status": "Valid", "subject": "CN=Razer USA Ltd., O=Razer USA Ltd."}}}
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, "ROOT", Path(directory)), \
                mock.patch.object(analysis, "index_files") as publish:
            original = Path(directory) / name
            original.parent.mkdir(parents=True)
            original.write_bytes(raw)
            workspace.record_service_version_evidence(receipt)
            rows = publish.call_args.args[0]
            self.assertEqual(rows[0]["sha256"], hashlib.sha256(raw).hexdigest())
            self.assertEqual(rows[0]["registry_key"], "CLSID/{verified-clsid}/LocalServer32")
            evidence = json.loads((Path(directory) / rows[1]["path"]).read_bytes())
            self.assertFalse(evidence["target_execution"])
            publish.reset_mock()
            original.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "differs"):
                workspace.record_service_version_evidence(receipt)
            publish.assert_not_called()
            original.write_bytes(raw)
            receipt["server"]["signature"]["status"] = "NotTrusted"
            with self.assertRaisesRegex(ValueError, "signature"):
                workspace.record_service_version_evidence(receipt)
            publish.assert_not_called()


class RustTestScopeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        root = mock.patch.object(workspace, 'ROOT', self.root)
        root.start()
        self.addCleanup(root.stop)

    def crate(self, library, files=None, package='fixture'):
        root = self.root / 'crates' / package
        root.mkdir(parents=True)
        (root / 'Cargo.toml').write_text('[package]\nname = "' + package + '"\nversion = "0.0.0"\n', encoding='utf-8')
        for path, text in {'src/lib.rs': library, **(files or {})}.items():
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding='utf-8')

    def scope(self, *arguments, package='fixture', inventories=None):
        return workspace.rust_test_scope(['cargo', 'test', '--locked', '-p', package, *arguments],
                                         {} if inventories is None else inventories)

    def test_broad_scroll_filter_rejects_windows_and_module_filter_keeps_both_pure_tests(self):
        self.crate('''
mod scroll_wheel { mod tests {
    #[test] fn mouse_scroll_readback() {}
    #[test] fn mouse_scroll_missing_readback() {}
}}
mod mapping { mod tests {
    #[gpui_kit::test] fn mouse_scroll_controls(cx: &mut TestAppContext) { cx.open_window(); }
}}
''')
        with self.assertRaisesRegex(ValueError, 'mapping::tests::mouse_scroll_controls'):
            self.scope('--lib', 'mouse_scroll_')
        scope = self.scope('--lib', 'scroll_wheel::tests::mouse_scroll_')
        self.assertEqual(scope['targets'][0]['selected_tests'], [
            'scroll_wheel::tests::mouse_scroll_readback', 'scroll_wheel::tests::mouse_scroll_missing_readback'])

    def test_inline_path_modules_and_imported_helpers_preserve_the_real_owner(self):
        self.crate('''
#[path = "support.rs"] mod helper;
mod nested {
    use crate::helper::open as present;
    #[path = "case.rs"] mod renamed;
    #[gpui_kit::test] fn window(cx: &mut TestAppContext) { present(cx); }
}
''', {'src/support.rs': 'pub fn open(cx: &mut TestAppContext) { cx.open_window(); }',
      'src/nested/case.rs': '#[test] fn pure() {}'})
        with self.assertRaisesRegex(ValueError, 'nested::window'):
            self.scope('--lib', 'nested::window')
        scope = self.scope('--lib', 'nested::renamed::pure')
        self.assertEqual(scope['targets'][0]['selected_tests'], ['nested::renamed::pure'])

    def test_indirect_local_method_window_dependency_is_rejected(self):
        self.crate('''
struct Owner;
impl Owner { fn present(&self, cx: &mut TestAppContext) { cx.add_window(); } }
fn helper(cx: &mut TestAppContext) { Owner.present(cx); }
#[gpui_kit::test] fn uses_helper(cx: &mut TestAppContext) { helper(cx); }
''')
        with self.assertRaisesRegex(ValueError, 'uses_helper'):
            self.scope('--lib')

    def test_entity_only_gpui_tests_and_window_words_in_text_are_allowed(self):
        self.crate('''
#[gpui_kit::test] fn entity_only(cx: &mut TestAppContext) {
    // cx.open_window() is documentation, not a call.
    let text = "cx.add_window()";
    cx.new(|_| State::new());
}
#[test] fn literal_only() { let text = "new_window()"; }
''')
        scope = self.scope('--lib')
        self.assertEqual(scope['targets'][0]['selected_tests'], ['entity_only', 'literal_only'])
        self.assertEqual(scope['window_tests_selected'], 0)

    def test_ignored_tests_and_explicit_skip_follow_test_harness_selection(self):
        self.crate('''
#[test] fn pure() {}
#[test] fn window() { cx.open_window(); }
#[test] #[ignore] fn ignored_window() { cx.open_window(); }
''')
        scope = self.scope('--lib', '--', '--skip', 'window')
        self.assertEqual(scope['targets'][0]['selected_tests'], ['pure'])
        scope = self.scope('--lib', 'pure', '--', '--exact')
        self.assertEqual(scope['targets'][0]['selected_tests'], ['pure'])
        with self.assertRaisesRegex(ValueError, 'nonignored'):
            self.scope('--lib', '--', '--ignored')

    def test_multi_package_command_checks_every_package(self):
        self.crate('#[test] fn packaged_catalog_pure() {}', package='first')
        self.crate('#[test] fn packaged_catalog_window() { cx.open_window(); }', package='second')
        with self.assertRaisesRegex(ValueError, 'second::packaged_catalog_window'):
            self.scope('-p', 'second', '--lib', 'packaged_catalog_', package='first')

    def test_named_integration_filter_does_not_select_ignored_device_operations(self):
        self.crate('', {'tests/connected.rs': '''
#[test] fn connected_result_validation() {}
#[test] #[ignore] fn connected_device_queries() { cx.open_window(); }
'''})
        scope = self.scope('--test', 'connected', 'connected_result_validation')
        self.assertEqual(scope['targets'][0]['target'], 'connected')
        self.assertEqual(scope['targets'][0]['selected_tests'], ['connected_result_validation'])

    def test_preflight_rejects_windows_before_any_compile_or_test_command_executes(self):
        self.crate('#[test] fn window() { cx.open_window(); }', package='razer-model')
        with mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace.subprocess, 'run') as runner:
            with self.assertRaisesRegex(ValueError, 'prohibited window'):
                workspace.validate_rust(selected_suites=['workspace', 'model'])
        runner.assert_not_called()

    def test_missing_filter_or_only_ignored_matches_cannot_select_a_suite(self):
        self.crate('''
#[test] fn pure() {}
#[test] #[ignore] fn offline_only() {}
''')
        for value in ('missing_module::tests', 'offline_only'):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'no current tests'):
                self.scope('--lib', value)

    def test_preflight_rejects_empty_filter_before_any_compile_or_test_command_executes(self):
        self.crate('#[test] fn unrelated() {}', package='razer-widgets')
        with mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace.subprocess, 'run') as runner:
            with self.assertRaisesRegex(ValueError, 'no current tests'):
                workspace.validate_rust(selected_suites=['workspace', 'source-input-length'])
        runner.assert_not_called()


class ConfiguredRustSuiteOwnerTests(unittest.TestCase):
    def test_every_configured_filter_selects_current_nonwindow_tests(self):
        commands = workspace.rust_validation_commands()
        self.assertEqual(len(commands), len({name for name, _ in commands}))
        inventories = {}
        for name, command in commands:
            with self.subTest(suite=name):
                scope = workspace.rust_test_scope(command, inventories)
                if command[1] == 'test':
                    self.assertGreater(sum(len(row['selected_tests']) for row in scope['targets']), 0)
                    self.assertEqual(scope['window_tests_selected'], 0)


class RustValidationBoundaryTests(unittest.TestCase):
    def setUp(self):
        scope = mock.patch.object(workspace, 'rust_test_scope', return_value={'targets': [], 'window_tests_selected': 0})
        scope.start()
        self.addCleanup(scope.stop)

    def test_assets_suite_excludes_window_tests_and_records_the_scope(self):
        def completed(command, **kwargs):
            kwargs["stdout"].write(b"test result: ok. 8 passed; 0 failed; 0 ignored\n")
            return mock.Mock(returncode=0)

        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, "ROOT", Path(directory)), \
                mock.patch.object(workspace, "rust_inputs", return_value=[]), \
                mock.patch.object(workspace, "verification_environment", side_effect=lambda _: nullcontext((None, None))), \
                mock.patch.object(workspace.subprocess, "run", side_effect=completed) as runner, \
                mock.patch.object(workspace, "write_json") as write_receipt, \
                mock.patch("sys.stdout", new=io.StringIO()):
            workspace.validate_rust(selected_suites=["assets"])
            command = runner.call_args.args[0]
            self.assertEqual(command[-3:], ["--", "--skip", "image_cache::tests"])
            receipt = write_receipt.call_args.args[1]
            self.assertTrue(receipt["passed"])
            self.assertEqual(receipt["commands"][0]["excluded_test_filters"], ["image_cache::tests"])
            self.assertIn("headless window", receipt["commands"][0]["test_scope"])

    def test_calibration_and_restore_suites_cover_measurement_and_retained_owners(self):
        expected = {
            'mouse-philips-protocol': ('razer-device', 'mouse_philips::'),
            'mouse-philips-routing': ('razer-discovery', 'philips'),
            'mouse-philips-state': ('razer-pages', 'calibration_70::state::'),
            'mouse-philips-native': ('razer-service', 'mapping_runtime::philips::'),
            'keyboard-latest-restore-state': ('razer-device', 'keyboard_latest_restore::tests::'),
            'keyboard-latest-restore-shell': ('razer-shell', 'shell::keyboard_latest_restore::tests::'),
            'keyboard-latest-restore-publisher': ('razer-shell', 'shell::keyboard_latest_restore::publisher::tests::'),
            'keyboard-game-lifecycle': ('razer-shell', 'game_monitor::lifecycle::tests::'),
            'keyboard-actuation-lifecycle': ('razer-shell', 'keyboard_actuation_write::worker::tests::'),
            'keyboard-brightness-shell': ('razer-shell', 'keyboard_brightness'),
            'keyboard-brightness-state': ('razer-pages', 'brightness::lifecycle_tests'),
            'session-monitor-lifecycle': ('razer-platform', 'session_changes::tests::'),
            'device-change-monitor-lifecycle': ('razer-platform', 'device_changes'),
            'settings-runtime-state': ('razer-settings', 'runtime_page::tests::'),
            'ipc-client': ('razer-ipc', 'client_tests'),
            'mouse-input-monitoring-ipc': ('razer-ipc', 'mouse_input_monitoring'),
            'mouse-input-monitoring-native': ('razer-service', 'mouse_input_monitoring::tests'),
            'device-task-locks': ('razer-ipc', 'device_tasks'),
            'mouse-matcher-service': ('razer-service', 'dpi_matcher'),
            'service-exit-lifecycle': ('razer-shell', 'shutdown::tests'),
            'mouse-matcher-protocol': ('razer-device', 'dpi_matcher'),
            'mouse-matcher-model': ('razer-model', 'mouse_matcher'),
            'mouse-matcher-revision': ('razer-pages', 'matcher_durable_version'),
            'mouse-matcher-exit-state': ('razer-pages', 'matcher_exit_'),
            'profile-notification-lifecycle': ('razer-shell', 'profile_switch_notification::tests'),
            'computer-name-platform': ('razer-platform', 'computer_name_tests'),
            'receiver-profile-storage': ('razer-storage', 'receiver_reset'),
            'receiver-profile-selection': ('razer-pages', 'receiver_profile_selection::tests'),
            'hyperpolling-protocol': ('razer-device', 'hyperpolling_receiver'),
            'hyperpolling-ipc': ('razer-ipc', 'hyperpolling_receiver'),
            'hyperpolling-state': ('razer-pages', 'hyperpolling_runtime'),
            'hyperpolling-storage': ('razer-storage', 'hyperpolling_profile'),
            'hyperpolling-shell': ('razer-shell', 'hyperpolling_runtime'),
            'receiver-lighting-owner': ('razer-shell', 'receiver_lighting_'),
            'lighting-engine-session': ('razer-service', 'lighting_engine_session::tests'),
            'controller-lighting-service': ('razer-service', 'controller_lighting::'),
            'chroma-connect': ('razer-service', 'chroma_connect::tests'),
            'receiver-lighting-attribute-ipc': ('razer-ipc', 'receiver_lighting_attribute_receipt_wire_requires_the_exact_worker_identity'),
            'receiver-brightness-ui': ('razer-pages', 'receiver_brightness_ui::tests'),
            'receiver-brightness-protocol': ('razer-device', 'receiver_brightness::tests'),
            'receiver-dock-mode': ('razer-device', 'receiver_dock_mode::tests'),
            'device-query-protocol': ('razer-device', 'device_query::tests'),
            'receiver-lighting-protocol': ('razer-device', 'receiver_lighting::tests'),
            'receiver-quick-effect': ('razer-device', 'receiver_lighting::quick_effect::tests'),
            'receiver-battery-peers': ('razer-device', 'receiver_lighting::battery_peers::tests'),
            'receiver-battery-monitor': ('razer-device', 'receiver_lighting::battery_monitor::tests'),
            'receiver-peer-lighting': ('razer-device', 'receiver_lighting::peer_protocol::tests'),
            'dynamic-lighting-storage': ('razer-storage', 'dynamic_lighting_read_is_independent'),
            'controller-quick-lighting-state': ('razer-pages', 'quick_lighting_state::tests'),
            'controller-chroma-state': ('razer-pages', 'chroma_runtime::tests'),
            'controller-calibration-frame': ('razer-pages', 'queued_frames_cannot_advance'),
            'controller-palette': ('razer-model', 'custom_colors::tests'),
            'help-reset-owner': ('razer-pages', 'help_reset_identity_'),
            'mixer-restart-receipts': ('razer-shell', 'mixer_restart_receipt_'),
            'product-chroma-observations': ('razer-shell', 'product_chroma::tests'),
            'product-lighting-worker': ('razer-shell', 'product_lighting::worker::tests'),
            'mouse-brightness-protocol': ('razer-device', 'mouse_brightness::tests'),
            'mouse-dynamic-protocol': ('razer-device', 'mouse_dynamic'),
            'mouse-dynamic-service': ('razer-service', 'mouse_dynamic'),
            'mouse-dynamic-routing': ('razer-discovery', 'direct::mouse_dynamic::tests'),
            'mouse-dynamic-page': ('razer-pages', 'features::mouse_products::dynamic::'),
            'runtime-diagnostics': ('razer-app', 'diagnostics::runtime::tests'),
            'product-arguments': ('razer-app', 'arguments::tests'),
            'service-runtime': ('razer-service', 'runtime::'),
            'chroma-profile-storage': ('razer-storage', 'chroma_profiles::tests'),
            'host-profile-storage': ('razer-storage', 'host_local_profile::'),
            'user-app-files-storage': ('razer-storage', 'user_app_files::'),
            'lighting-host-page': ('razer-pages', 'host_observation_tests'),
            'lighting-observation-shell': ('razer-shell', 'product_lighting::observation::tests'),
            'mouse-dynamic-shell': ('razer-shell', 'mouse_dynamic'),
            'mouse-sensor-protocol': ('razer-device', 'mouse_sensor::tests'),
            'help-system-info-domain': ('razer-device', 'system_info::tests'),
            'help-system-info-ipc': ('razer-ipc', 'help_system_info'),
            'help-system-info-native': ('razer-service', 'help_system_info'),
            'help-file-dialog-lifecycle': ('razer-platform', 'file_dialog::tests'),
            'help-system-info-page': ('razer-pages', 'help_system_info'),
            'help-system-info-shell': ('razer-shell', 'help_system_info'),
            'thx-service': ('razer-service', 'thx'),
            'thx-shell-lifecycle': ('razer-shell', 'thx_sound::tests::'),
            'mouse-receiver-polling-routing': ('razer-discovery', 'mouse_receiver_polling'),
            'hanbo-protocol': ('razer-device', 'hanbo_cooler::tests'),
            'hanbo-startup-protocol': ('razer-device', 'hanbo_startup'),
            'hanbo-settings': ('razer-device', 'hanbo_settings::tests'),
            'hanbo-profile-restore': ('razer-device', 'hanbo_profile_restore::tests'),
            'hanbo-ipc': ('razer-ipc', 'hanbo_'),
            'hanbo-service': ('razer-service', 'hanbo_cooler::tests'),
            'hanbo-state': ('razer-pages', 'hanbo_'),
            'hanbo-shell': ('razer-shell', 'shell::hanbo_cooler::tests::'),
            'profile-collection': ('razer-pages', 'profile_collection::tests'),
            'profiles-transfer-codec': ('razer-device', 'profile_transfer'),
            'local-profile-codec': ('razer-model', 'profile_transfer'),
            'chroma-settings-state': ('razer-state', 'chroma_settings::tests'),
            'local-image': ('razer-assets', 'local_image::tests'),
            'rotated-label-outline': ('razer-assets', 'rotated_label_'),
            'native-diagnostics': ('razer-platform', 'discovery_diagnostics::tests'),
            'linked-executable': ('razer-platform', 'executable::tests'),
            'oled-import-lifecycle': ('razer-pages', 'oled_presets::tests::import_'),
            'system-monitor-protocol': ('razer-device', 'system_monitor::tests'),
            'system-monitor-ipc': ('razer-ipc', 'system_monitor_tests'),
            'system-monitor-native': ('razer-service', 'system_monitor::tests'),
            'keyboard-magnetic-protocol': ('razer-device', 'keyboard_magnetic'),
            'keyboard-sync-feedback': ('razer-pages', 'keyboard_products::actuation::sync_feedback::tests::'),
            'seiren-audio-owner-service': ('razer-service', 'seiren_audio::'),
            'seiren-audio-operation-state': ('razer-pages', 'seiren_audio_'),
            'seiren-audio-operation-owner': ('razer-shell', 'seiren_audio_'),
            'audio-notification-cleanup': ('razer-shell', 'audio_cleanup_'),
            'audio-shortcut-lifecycle': ('razer-shell', 'audio_shortcut_lifecycle_tests'),
            'source-text-tracking': ('razer-widgets', 'source_text::tests'),
            'assets-image-lifecycle': ('razer-assets', 'image_cache::lifecycle_tests'),
            'keyboard-macro-submission': ('razer-device', 'keyboard_submission::macro_submission::tests'),
            'onboard-macro': ('razer-device', 'onboard_macro'),
            'macro-page-phases': ('razer-app-pages', 'macro_page::phased::tests'),
            'mapping-observation': ('razer-service', 'mapping::observation::'),
            'global-shortcuts-native': ('razer-service', 'mapping_runtime::backend::tests'),
            'global-shortcut-capture-native': ('razer-service', 'mapping_runtime::capture::tests'),
            'mapping-observation-ipc': ('razer-ipc', 'mapping_observation::'),
            'mapping-dynamic-key-state': ('razer-device', 'mapping_native_dynamic_key'),
            'mapping-dynamic-key-service': ('razer-service', 'dynamic_key'),
            'mapping-input-observation-ipc': ('razer-ipc', 'input_observation'),
            'mapping-input-observation-service': ('razer-service', 'input_observation'),
            'mapping-analog-observation-state': ('razer-device', 'mapping_native_observation'),
            'mapping-analog-observation-ipc': ('razer-ipc', 'analog_observation'),
            'mapping-analog-observation-service': ('razer-service', 'analog_observation'),
            'mapping-gamepad-observation-state': ('razer-device', 'mapping_native_gamepad_observation'),
            'mouse-receiver-mapping': ('razer-device', 'mouse_receiver_mapping::tests'),
            'mouse-otfs-mapping': ('razer-device', 'interdevice_otfs'),
            'mapping-document-model': ('razer-model', 'mapping::'),
            'mapping-reconnect-state': ('razer-pages', 'mapping_reconnect::tests'),
            'chroma-commit-storage': ('razer-storage', 'chroma_commit_revisions_'),
            'chroma-commit-publication': ('razer-shell', 'chroma_configuration_commits_'),
            'keyboard-mapping-audio-inventory': ('razer-shell', 'keyboard_mapping_audio_inventory_'),
            'mouse-dpi-stage-operations': ('razer-shell', 'dpi_stage_'),
            'mouse-dpi-stage-observations': ('razer-pages', 'dpi_stage_observation_'),
            'mouse-receiver-callbacks': ('razer-pages', 'mapping_runtime_158::tests'),
            'mapping-gamepad-observation-ipc': ('razer-ipc', 'gamepad_observation'),
            'mapping-gamepad-observation-service': ('razer-service', 'gamepad_observation'),
            'keyboard-gamepad-observation': ('razer-pages', 'gamepad_output_'),
            'mapping-subscription-lifecycle': ('razer-pages', 'observation_tasks::tests'),
            'mapping-cleanup-failures': ('razer-service', 'failed_gamepad_cleanup'),
            'keyboard-snaptap-state': ('razer-pages', 'snap_tap::observation::tests'),
            'keyboard-snaptap-editing': ('razer-pages', 'disabling_capture_keeps_the_source_opposite_preview_key'),
            'keyboard-analog-feedback': ('razer-pages', 'analog_feedback'),
            'macro-hotkey-lifecycle': ('razer-service', 'mapping::hotkeys::'),
            'macro-recording-state': ('razer-model', 'macro_library::recording::'),
            'macro-recorder-preferences': ('razer-model', 'macro_library::recorder_preference_tests'),
            'macro-shortcut-registration': ('razer-app-pages', 'macro_page::record_shortcut::registration_tests'),
        }
        def completed(command, **kwargs):
            kwargs['stdout'].write(b'test result: ok. 1 passed; 0 failed; 0 ignored\n')
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, 'ROOT', Path(directory)), \
                mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace, 'verification_environment', side_effect=lambda _: nullcontext((None, None))), \
                mock.patch.object(workspace.subprocess, 'run', side_effect=completed) as runner, \
                mock.patch.object(workspace, 'write_json') as write_receipt, \
                mock.patch('sys.stdout', new=io.StringIO()):
            workspace.validate_rust(selected_suites=list(expected))
            actual = {(call.args[0][call.args[0].index('-p') + 1], call.args[0][call.args[0].index('--lib') + 1])
                      for call in runner.call_args_list}
            self.assertEqual(actual, set(expected.values()))
            macro_phases = next(call.args[0] for call in runner.call_args_list
                                if 'macro_page::phased::tests' in call.args[0])
            self.assertEqual(macro_phases, ['cargo', 'test', '--locked', '--offline', '-p',
                                            'razer-app-pages', '--lib', 'macro_page::phased::tests'])
            settings = next(call.args[0] for call in runner.call_args_list if 'razer-settings' in call.args[0])
            self.assertEqual([settings[ix + 1] for ix, argument in enumerate(settings) if argument == '--skip'], [
                'runtime_page::tests::partial_enumeration_is_visible_without_starting_a_service',
                'runtime_page::tests::connection_page_does_not_start_services_on_render'])
            self.assertEqual(settings[-1], '--test-threads=1')
            self.assertTrue(write_receipt.call_args.args[1]['passed'])

    def test_keyboard_feedback_filter_follows_declared_modules_and_real_tests(self):
        import resource_scope
        parser = resource_scope.parser(workspace.ROOT)
        root = workspace.ROOT / 'crates/razer-pages/src'
        modules = []
        for owner, target in (
            ('features/mod.rs', 'keyboard/mod.rs'),
            ('features/keyboard/mod.rs', 'actuation/keyboard_actuation.rs'),
            ('features/keyboard/actuation/keyboard_actuation.rs', 'keyboard_sync_feedback.rs'),
        ):
            raw = (root / owner).read_bytes()
            found = []
            for node in parser.parse(raw).root_node.named_children:
                if node.type != 'mod_item':
                    continue
                sibling = node.prev_named_sibling
                while sibling is not None and sibling.type in ('attribute_item', 'line_comment', 'block_comment'):
                    attribute = ''.join(raw[sibling.start_byte:sibling.end_byte].decode().split())
                    if sibling.type == 'attribute_item' and attribute == '#[path="' + target + '"]':
                        name = node.child_by_field_name('name')
                        found.append(raw[name.start_byte:name.end_byte].decode())
                    sibling = sibling.prev_named_sibling
            self.assertEqual(len(found), 1, owner)
            modules.extend(found)
        raw = (root / 'features/keyboard/actuation/keyboard_sync_feedback.rs').read_bytes()
        tests = next(node for node in parser.parse(raw).root_node.named_children
                     if node.type == 'mod_item' and raw[node.child_by_field_name('name').start_byte:
                                                       node.child_by_field_name('name').end_byte] == b'tests')
        functions = [raw[node.child_by_field_name('name').start_byte:node.child_by_field_name('name').end_byte].decode()
                     for node in tests.child_by_field_name('body').named_children if node.type == 'function_item'
                     and node.prev_named_sibling is not None and node.prev_named_sibling.type == 'attribute_item'
                     and raw[node.prev_named_sibling.start_byte:node.prev_named_sibling.end_byte] == b'#[test]']
        self.assertGreater(len(functions), 0)

        def completed(command, **kwargs):
            kwargs['stdout'].write(f'test result: ok. {len(functions)} passed; 0 failed; 0 ignored\n'.encode())
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, 'ROOT', Path(directory)), \
                mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace, 'verification_environment', side_effect=lambda _: nullcontext((None, None))), \
                mock.patch.object(workspace.subprocess, 'run', side_effect=completed) as runner, \
                mock.patch.object(workspace, 'write_json'), \
                mock.patch('sys.stdout', new=io.StringIO()):
            workspace.validate_rust(selected_suites=['keyboard-sync-feedback'])
            command = runner.call_args.args[0]
        test_filter = command[command.index('--lib') + 1]
        expected = '::'.join([*modules, 'tests']) + '::'
        self.assertEqual(test_filter, expected)
        self.assertTrue(all(test_filter in expected + name for name in functions))

    def test_seiren_worker_filter_follows_service_owner_and_real_tests(self):
        import resource_scope
        parser = resource_scope.parser(workspace.ROOT)
        root = workspace.ROOT / 'crates/razer-service/src'
        raw = (root / 'lib.rs').read_bytes()
        owners = []
        for node in parser.parse(raw).root_node.named_children:
            if node.type == 'mod_item' and node.prev_named_sibling is not None:
                attribute = ''.join(raw[node.prev_named_sibling.start_byte:node.prev_named_sibling.end_byte].decode().split())
                if attribute == '#[path="audio/seiren/mod.rs"]':
                    owners.append(raw[node.child_by_field_name('name').start_byte:node.child_by_field_name('name').end_byte].decode())
        self.assertEqual(len(owners), 1)
        names = []
        for branch in ('capture', 'playback'):
            raw = (root / f'audio/seiren/{branch}/tests.rs').read_bytes()
            functions = [raw[node.child_by_field_name('name').start_byte:node.child_by_field_name('name').end_byte].decode()
                         for node in parser.parse(raw).root_node.named_children
                         if node.type == 'function_item' and node.prev_named_sibling is not None
                         and node.prev_named_sibling.type == 'attribute_item'
                         and raw[node.prev_named_sibling.start_byte:node.prev_named_sibling.end_byte] == b'#[test]']
            self.assertGreater(len(functions), 0, branch)
            names.extend('::'.join([owners[0], branch, 'tests', name]) for name in functions)

        def completed(command, **kwargs):
            kwargs['stdout'].write(f'test result: ok. {len(names)} passed; 0 failed; 0 ignored\n'.encode())
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, 'ROOT', Path(directory)), \
                mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace, 'verification_environment', side_effect=lambda _: nullcontext((None, None))), \
                mock.patch.object(workspace.subprocess, 'run', side_effect=completed) as runner, \
                mock.patch.object(workspace, 'write_json'), \
                mock.patch('sys.stdout', new=io.StringIO()):
            workspace.validate_rust(selected_suites=['seiren-audio-owner-service'])
            command = runner.call_args.args[0]
        test_filter = command[command.index('--lib') + 1]
        self.assertEqual(command[command.index('-p') + 1], 'razer-service')
        self.assertEqual(test_filter, owners[0] + '::')
        self.assertTrue(all(test_filter in name for name in names))

    def test_rotated_label_filter_matches_actual_outline_tests(self):
        import resource_scope
        parser = resource_scope.parser(workspace.ROOT)
        root = workspace.ROOT / 'crates/razer-assets/src'
        raw = (root / 'lib.rs').read_bytes()
        modules = [raw[node.child_by_field_name('name').start_byte:node.child_by_field_name('name').end_byte]
                   for node in parser.parse(raw).root_node.named_children if node.type == 'mod_item']
        self.assertIn(b'rotated_label', modules)
        raw = (root / 'rotated_label/tests.rs').read_bytes()
        names = [raw[node.child_by_field_name('name').start_byte:node.child_by_field_name('name').end_byte].decode()
                 for node in parser.parse(raw).root_node.named_children if node.type == 'function_item'
                 and node.prev_named_sibling is not None and node.prev_named_sibling.type == 'attribute_item'
                 and raw[node.prev_named_sibling.start_byte:node.prev_named_sibling.end_byte] in
                     (b'#[test]', b'#[::core::prelude::v1::test]')]
        self.assertGreater(len(names), 0)

        def completed(command, **kwargs):
            kwargs['stdout'].write(f'test result: ok. {len(names)} passed; 0 failed; 0 ignored\n'.encode())
            return mock.Mock(returncode=0)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(workspace, 'ROOT', Path(directory)), \
                mock.patch.object(workspace, 'rust_inputs', return_value=[]), \
                mock.patch.object(workspace, 'verification_environment', side_effect=lambda _: nullcontext((None, None))), \
                mock.patch.object(workspace.subprocess, 'run', side_effect=completed) as runner, \
                mock.patch.object(workspace, 'write_json'), \
                mock.patch('sys.stdout', new=io.StringIO()):
            workspace.validate_rust(selected_suites=['rotated-label-outline'])
            command = runner.call_args.args[0]
        test_filter = command[command.index('--lib') + 1]
        self.assertEqual(command[command.index('-p') + 1], 'razer-assets')
        self.assertEqual(test_filter, 'rotated_label_')
        self.assertTrue(all(test_filter in name for name in names))


def wheel_bytes(members, *, records=None, extra=()):
    record_name = "analyzer-1.dist-info/RECORD"
    if records is None:
        records = [(name, "sha256=" + base64.urlsafe_b64encode(hashlib.sha256(raw).digest()).rstrip(b"=").decode(), str(len(raw)))
                   for name, raw in members]
        records.append((record_name, "", ""))
    table = io.StringIO()
    csv.writer(table).writerows(records)
    raw = io.BytesIO()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)
        with zipfile.ZipFile(raw, "w") as archive:
            for name, content in [*members, *extra, (record_name, table.getvalue().encode())]:
                archive.writestr(name, content)
    return raw.getvalue(), record_name


class AnalyzerWheelTests(unittest.TestCase):
    def verify(self, raw, record):
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            return workspace.verified_wheel_members(archive, record)

    def test_complete_member_hashes_and_lengths_verify(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"inert source"), ("analyzer/native.pyd", b"inert binary")])
        self.assertEqual(self.verify(raw, record), ["analyzer/module.py", "analyzer/native.pyd", record])

    def test_archive_member_missing_from_record_is_rejected(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"source")], extra=[("analyzer/hidden.pyd", b"unlisted")])
        with self.assertRaisesRegex(ValueError, "cover every"):
            self.verify(raw, record)

    def test_record_cannot_accept_mutated_member(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"altered")], records=[
            ("analyzer/module.py", "sha256=" + base64.urlsafe_b64encode(hashlib.sha256(b"original").digest()).rstrip(b"=").decode(), "8"),
            ("analyzer-1.dist-info/RECORD", "", "")])
        with self.assertRaisesRegex(ValueError, "does not verify"):
            self.verify(raw, record)

    def test_unsigned_payload_is_rejected(self):
        raw, record = wheel_bytes([("analyzer/native.pyd", b"payload")], records=[
            ("analyzer/native.pyd", "", ""), ("analyzer-1.dist-info/RECORD", "", "")])
        with self.assertRaisesRegex(ValueError, "lacks a RECORD identity"):
            self.verify(raw, record)

    def test_unsafe_paths_are_rejected_before_any_publication(self):
        for name in ("../escape", "/absolute", "C:/absolute", "folder\\escape"):
            with self.subTest(name=name):
                raw, record = wheel_bytes([(name, b"inert")])
                with self.assertRaisesRegex(ValueError, "escaped"):
                    self.verify(raw, record)

    def test_duplicate_archive_members_are_rejected(self):
        raw, record = wheel_bytes([("analyzer/module.py", b"first"), ("analyzer/module.py", b"second")])
        with self.assertRaisesRegex(ValueError, "duplicate members"):
            self.verify(raw, record)


class ProductRegistryTests(unittest.TestCase):
    def records(self):
        return {"schema_version": 1, "products": [{"id": 756, "name": "Keyboard", "categories": ["KEYBOARD"],
            "edition_ids": [0, 128], "navigations": [
                {"key": "keyboard-default", "display_mode": "default", "primary": True, "pages": [
                    {"key": "keyboard-calibration", "kind": "TAB_CALIBRATION", "role": "Page", "help_selector": 10},
                    {"key": "keyboard-help", "kind": "HELP", "role": "Help", "help_selector": 11}]},
                {"key": "keyboard-ble", "display_mode": "default", "primary": False, "pages": [
                    {"key": "keyboard-ble-help", "kind": "HELP", "role": "Help", "help_selector": 21}]}]}]}

    def load(self, document):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "registry.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            return resource_database.product_registry(path)

    def test_complete_connection_branches_and_help_selectors_roundtrip_through_sql(self):
        products = self.load(self.records())
        with closing(sqlite3.connect(":memory:")) as connection:
            connection.execute("PRAGMA foreign_keys=ON")
            connection.executescript((resource_database.ROOT / resource_database.SCHEMA).read_text(encoding="utf-8"))
            counts = resource_database.fill_product_registry(connection, products)
            self.assertEqual(counts["ui_page"], 3)
            self.assertEqual(connection.execute("SELECT key, display_mode, is_primary FROM ui_navigation ORDER BY position").fetchall(),
                             [("keyboard-default", "default", 1), ("keyboard-ble", "default", 0)])
            self.assertEqual(connection.execute("SELECT key, help_selector FROM ui_page WHERE role='Help' ORDER BY help_selector").fetchall(),
                             [("keyboard-help", 11), ("keyboard-ble-help", 21)])
            with self.assertRaises(sqlite3.IntegrityError):
                connection.execute("INSERT INTO ui_page VALUES ('orphan', 'missing-navigation', 0, 'HELP', 'Help', 1)")

    def test_source_reaudit_retains_help_identity_when_source_positions_change(self):
        previous = self.records()["products"][0]["navigations"]
        current = [{"source": {"path": "new-current.chunk.js"}, "branch": "wired", "offset": 1000,
                    "display_mode": "default", "primary": True,
                    "pages": [{"kind": "TAB_CALIBRATION", "offset": 1000}, {"kind": "HELP", "offset": 2000}]},
                   {"source": {"path": "new-current.chunk.js"}, "branch": "ble", "offset": 3000,
                    "display_mode": "default", "primary": False, "pages": [{"kind": "HELP", "offset": 4000}]}]
        product = families.registry_product_record(756, "Keyboard", "KEYBOARD", [0, 128], current, previous)
        self.assertEqual(product, self.records()["products"][0])
        bindings = [{"product_id": 756, "pages": [{"offset": 11}, {"offset": 21}]}]
        self.assertEqual(resource_database.validate_product_help_bindings([product], bindings), 2)
        product["navigations"][1]["pages"][0]["help_selector"] = 11
        bindings[0]["pages"] = [{"offset": 21}]
        with self.assertRaisesRegex(ValueError, "no local presentation record"):
            resource_database.validate_product_help_bindings([product], bindings)

    def test_help_binding_rejects_ambiguous_presentation_records(self):
        products = self.records()["products"]
        for records in ([{"product_id": 756, "pages": [{"offset": 11}, {"offset": 11}]}],
                        [{"product_id": 756, "pages": []}, {"product_id": 756, "pages": []}]):
            with self.subTest(records=records):
                with self.assertRaisesRegex(ValueError, "Ambiguous"):
                    resource_database.validate_product_help_bindings(products, records)

    def test_duplicate_stable_ids_missing_primary_unknown_fields_and_bad_editions_fail(self):
        cases = []
        duplicate = self.records()
        duplicate["products"][0]["navigations"][1]["pages"][0]["key"] = "keyboard-help"
        cases.append(duplicate)
        no_primary = self.records()
        no_primary["products"][0]["navigations"][0]["primary"] = False
        cases.append(no_primary)
        unknown = self.records()
        unknown["products"][0]["navigations"][0]["source"] = ".ref/never-a-runtime-input"
        cases.append(unknown)
        editions = self.records()
        editions["products"][0]["edition_ids"] = [0, True]
        cases.append(editions)
        unsorted = self.records()
        unsorted["products"].append(copy.deepcopy(unsorted["products"][0]))
        unsorted["products"][1]["id"] = 12
        cases.append(unsorted)
        for document in cases:
            with self.subTest(document=document):
                with self.assertRaises(ValueError):
                    self.load(document)


class FrontendClosureTests(unittest.TestCase):
    def test_declared_resources_maps_and_exploratory_probes_remain_separate(self):
        document = {"files": [{"url": "https://official/main.js", "path": "original.js"},
                              {"url": "https://official/main.js.map", "error": "404"},
                              {"url": "https://official/favicon.svg", "error": "404"},
                              {"url": "https://official/asset-manifest.json", "error": "404"},
                              {"url": "https://official/unknown.bin", "error": "404"}],
                    "edges": [{"from": "https://official/main.js", "to": "https://official/main.js.map", "kind": "source-map"},
                              {"from": "https://official/index.html", "to": "https://official/favicon.svg", "kind": "html:link"}],
                    "probes": [{"url": "https://official/asset-manifest.json", "declared": False}],
                    "unresolved": [{"kind": "chunk_unresolved", "asset": {"template": "[id].js"}}]}
        gaps = families.frontend_reference_gaps(document)
        self.assertEqual(gaps["declared_source_maps_missing"], 1)
        self.assertEqual([row["url"] for row in gaps["declared_non_map_missing"]], ["https://official/favicon.svg"])
        self.assertEqual([row["url"] for row in gaps["undeclared_probes_unavailable"]], ["https://official/asset-manifest.json"])
        self.assertEqual([row["url"] for row in gaps["unattributed_non_map_missing"]], ["https://official/unknown.bin"])
        self.assertFalse(gaps["all_declared_non_map_references_retained"])
        self.assertEqual(gaps["unresolved_by_kind"], {"chunk_unresolved": 1})

    def test_opaque_original_keeps_parameter_reuse_free_exports_and_unicode_byte_ranges(self):
        for body in ('21:function(e,t,n){e.exports=t=function(e,t){return n(e,t)};t.methods=exports}',
                     '21(e,t,n){n.d(t,{A:()=>exports});e=n.hmd(e)}'):
            source = 'const label="原始";const modules={' + body + '};'
            module = next(row for row in families.keyboard_source_modules(source) if row["module_id"] == 21)
            capture = families.webpack_factory_snapshot(module)
            raw = source.encode("utf-8")
            self.assertEqual(capture["body"], body)
            self.assertEqual(capture["parameters"], "(e,t,n)")
            self.assertEqual(raw[capture["byte_start"]:capture["byte_end"]], body.encode())
            self.assertEqual(raw[capture["statements_byte_start"]:capture["statements_byte_end"]], capture["statements"].encode())
            self.assertIn("exports", capture["statements"])


class MappingCapabilityProjectionTests(unittest.TestCase):
    def test_complete_factory_resolves_array_bindings_and_records_sparse_membership(self):
        factory = '51:(e,t,n)=>{n.d(t,{DeviceInfo:()=>info});const allowed=["ScrollUp",,"ScrollDown"],info={productId:51,category:"MOUSE",isOBMDevice:!0,unconsumed:n(12),OBMSpecs:{noFirmwareInputId:[],supportedMappings:{mouseGroup:allowed,macroGroup:!1}},ble:{obmMacrosAvailable:!1}};}'
        projected, evidence = typed_tables.mapping_source_projection(factory)
        self.assertEqual(projected["supportedMappings"]["mouseGroup"], ["ScrollUp", {"source_array_hole": True}, "ScrollDown"])
        self.assertEqual(projected["supportedMappings"]["macroGroup"], False)
        self.assertEqual(projected["bleMacrosAvailable"], False)
        self.assertNotIn("isXenaT2Series", projected)
        self.assertNotIn("MOUSE_FUNC_REQUIRED_SYNAPSE_VALUES", projected)
        self.assertEqual([hole["array_index"] for hole in evidence["array_holes"]], [1])
        for binding in evidence["bindings"]:
            body = factory.encode()[binding["byte_start"]:binding["byte_end"]]
            self.assertEqual(body.decode(), binding["body"])
            self.assertEqual(hashlib.sha256(body).hexdigest(), binding["body_sha256"])

    def test_consumed_indirect_expression_is_rejected(self):
        factory = '51:(e,t,n)=>{n.d(t,{DeviceInfo:()=>info});const info={productId:51,category:"MOUSE",OBMSpecs:{supportedMappings:{mouseGroup:n(12)}}};}'
        with self.assertRaisesRegex(ValueError, "Unsupported consumed JavaScript capability node"):
            typed_tables.mapping_source_projection(factory)

    def test_rust_constructor_roundtrip_rejects_changed_capability_semantics(self):
        products = [{"product_id": 51, "projection": {"productId": 51, "category": "MOUSE", "isOBMDevice": True,
            "noFirmwareInputId": [], "supportedMappings": {"mouseGroup": ["ScrollUp", {"source_array_hole": True}, "ScrollDown"]}}}]
        rendered = typed_tables.render_mapping_capabilities(products)
        self.assertTrue(typed_tables.verify_mapping_constructor(rendered, products)["all_fields_and_order_verified"])
        with self.assertRaisesRegex(ValueError, "differ from current source projection"):
            typed_tables.verify_mapping_constructor(rendered.replace(b"is_obm_device: Some(true)", b"is_obm_device: Some(false)"), products)


class ModuleServiceArtworkTests(unittest.TestCase):
    def source(self):
        return ('const label="原件";const modules={44442:(e,t,n)=>{'
            'const h=n.p+"static/media/installation_cover_placeholder.c5d354c7.avif";'
            'const w=(0,r.connect)(e=>e)(e=>{const [A,j]=(0,i.useState)(!1),'
            '[M,I]=(0,i.useState)(!1),O=()=>I(!0);return A?(0,b.jsx)("img",'
            '{className:"item-description-image",src:M?h:n.detail.srcImage,onError:O}):null});'
            'const H=e=>e.items.map(item=>(0,b.jsx)(w,{item}));'
            'const ae=(0,r.connect)(e=>e)(e=>{const v=e=>({...e,srcImage:"thumbnail"});return v(e)});'
            '}};').encode()

    def test_complete_row_proof_preserves_original_utf8_boundaries_and_sticky_error_state(self):
        raw = self.source()
        proof = workspace.module_service_artwork_projection(raw)
        self.assertEqual(proof["fallback_source_path"], "/synapse/dashboard/static/media/installation_cover_placeholder.c5d354c7.avif")
        self.assertEqual(proof["detail_source"], "n.detail.srcImage")
        for key, capture in proof.items():
            if isinstance(capture, dict) and "body" in capture:
                body = raw[capture["byte_start"]:capture["byte_end"]]
                self.assertEqual(body.decode(), capture["body"], key)
                self.assertEqual(hashlib.sha256(body).hexdigest(), capture["body_sha256"], key)

    def test_unverified_reset_primary_identity_and_state_initialization_fail(self):
        for old, new in ((b"O=()=>I(!0)", b"O=()=>I(!0),reset=()=>I(!1)"),
                         (b"n.detail.srcImage", b"n.srcImage"),
                         (b"[M,I]=(0,i.useState)(!1)", b"[M,I]=(0,i.useState)(!0)")):
            with self.subTest(change=new), self.assertRaises(ValueError):
                workspace.module_service_artwork_projection(self.source().replace(old, new))


class KeyboardArtworkVerificationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import sys
        sys.path.insert(0, str(Path(__file__).resolve().parent / "analyzers/pillow"))
        from PIL import Image
        cls.Image = Image

    def png(self, size=(2, 1), changed=None):
        image = self.Image.new("RGBA", size, (17, 29, 43, 255))
        if changed is not None:
            image.putpixel((0, 0), changed)
        stream = io.BytesIO()
        image.save(stream, format="PNG")
        return stream.getvalue()

    def image(self, pid=720, layout=1, alias=None, svg=False):
        return {"edition_id": 0, "layout_id": layout, "resolution": 1, "svg": svg,
                "asset": alias or f"synapse/keyboard-current/{pid}/{layout}.fixture." + ("svg" if svg else "png")}

    def product(self, pid=720, images=None, customize=None):
        return {"product_id": pid, "image_mode_svg": False,
                "images": images if images is not None else [self.image(pid)],
                "customize_images": customize or []}

    def test_atlas_enumerates_every_density_layout_and_customize_use_with_exact_product_filter(self):
        base = self.image(svg=True)
        customize = {**base, "asset": base["asset"].replace(".svg", "-customize.svg")}
        three_x = {**self.image(), "resolution": 3, "asset": "synapse/keyboard-current/720/1-3x.fixture.png"}
        document = {"products": [self.product(images=[base, self.image(), three_x], customize=[customize]), self.product(728)]}
        atlas = workspace.keyboard_artwork_atlas(document)
        self.assertEqual(len(atlas), 5)
        self.assertEqual({use["product_id"] for uses in atlas.values() for use in uses}, {720, 728})
        self.assertEqual(len(workspace.keyboard_artwork_atlas(document, product_ids=[728])), 1)
        for selected in ([], [999]):
            with self.subTest(selected=selected), self.assertRaises(ValueError):
                workspace.keyboard_artwork_atlas(document, product_ids=selected)
        for products in ([self.product(), self.product()], [self.product(images=[base, base])],
                         [self.product(images=[{**base, "resolution": True}])],
                         [self.product(images=[{**base, "asset": "synapse/keyboard-current/720/../escape.svg"}])]):
            with self.subTest(products=products), self.assertRaises(ValueError):
                workspace.keyboard_artwork_atlas({"products": products})

    def test_alias_requires_unique_mapping_and_pinned_bytes_without_materializing_media(self):
        alias = self.image()["asset"]
        row = {"path": "assets/synapse/products/720/pinned.png", "bytes": 3, "sha256": "a" * 64}
        mappings = {"tables": [{"entries": [{"key": alias, "path": row["path"]}]}]}
        self.assertEqual(workspace.keyboard_artwork_alias_identity(alias, mappings, [row]), row)
        with self.assertRaises(FileNotFoundError):
            workspace.keyboard_artwork_alias_identity(alias, {"tables": []}, [row])
        with self.assertRaises(FileNotFoundError):
            workspace.keyboard_artwork_alias_identity(alias, mappings, [])
        conflict = copy.deepcopy(mappings)
        conflict["tables"][0]["entries"].append({"key": alias, "path": "assets/synapse/products/720/other.png"})
        with self.assertRaisesRegex(ValueError, "conflicting"):
            workspace.keyboard_artwork_alias_identity(alias, conflict, [row])
        with self.assertRaisesRegex(ValueError, "repeated"):
            workspace.keyboard_artwork_alias_identity(alias, mappings, [row, row])

    def test_auxiliary_catalogs_enumerate_source_wrist_dial_keypad_and_every_mapping_state(self):
        source = {"product_id": 679, "images": [
            {"role": "base", "edition_id": 0, "layout_id": 1, "resolution": 1, "asset": "synapse/keyboard679-current/base.png"},
            {"role": "dial", "edition_id": 0, "asset": "synapse/keyboard679-current/dial.png"}],
            "wrist": {"asset": "synapse/keyboard679-current/wrist.png"}}
        extra = {"KEYBOARD_SOURCE_ARTWORK_DATA": source,
                 "KEYBOARD_KEYPAD_ARTWORK_DATA": [{"product_id": 555, "images": [
                     {"edition_id": 0, "role": "scroll-1x.png", "asset": "synapse/keyboard-keypad-artwork/555-0123456789abcdef.png"}],
                     "icons": [{"role": "four", "asset": "synapse/keyboard-keypad-artwork/555-fedcba9876543210.svg"}]}],
                 "KEYBOARD_KEYPAD_DEVICE_IMAGE_DATA": [{"product_id": 555, "edition_id": 0, "density": 1,
                                                        "asset": "synapse/keyboard-keypad-device-image/555-1111111111111111.png"}],
                 "KEYBOARD_MAPPING_BLADE_ARTWORK_DATA": [{"product_id": 555, "assets": {"DEFAULT": {
                     "normal": "synapse/keyboard-mapping-blade/default.svg", "active": "synapse/keyboard-mapping-blade/default-active.svg"}}}]}
        atlas = workspace.keyboard_artwork_atlas({"products": [self.product()]}, additional_catalogs=extra)
        self.assertEqual(len(atlas), 9)
        self.assertEqual(sum(len(uses) for uses in atlas.values()), 9)
        self.assertEqual(len(workspace.keyboard_artwork_atlas({"products": [self.product()]}, additional_catalogs=extra, product_ids=[555])), 5)
        extra["KEYBOARD_MAPPING_BLADE_ARTWORK_DATA"][0]["assets"]["DEFAULT"].pop("normal")
        with self.assertRaisesRegex(ValueError, "incomplete"):
            workspace.keyboard_artwork_atlas({"products": [self.product()]}, additional_catalogs=extra)

    def test_enum_import_ast_uses_last_static_comma_import_and_rejects_dynamic_expressions(self):
        for literal, expected in (("n(13254)", 13254), ("(n(61350),n(13254))", 13254),
                                  ("(n(1),n(2),n(68e3))", 68000)):
            body = "78766:(e,t,n)=>{const r=" + literal + ";}"
            self.assertEqual(workspace.keyboard_svg_import_module(body, "r"), expected)
        for literal in ("(n(61350),chosen())", "n(moduleId)", "object.require(13254)", "n(13254,extra)"):
            with self.subTest(literal=literal), self.assertRaises(ValueError):
                workspace.keyboard_svg_import_module("78766:(e,t,n)=>{const r=" + literal + ";}", "r")
        for body in ("const r=n(1);function scoped(){const r=n(2);}",
                     "const r=n(1);{const r=n(2);}",
                     "const r=n(1);class Scoped{method(){const r=n(2);}}"):
            with self.subTest(body=body):
                self.assertEqual(workspace.keyboard_svg_import_module("78766:(e,t,n)=>{" + body + "}", "r"), 1)
        for body in ("var r=n(1);{var r=n(2);}", "var r=n(1);var r=n(2);", "const r=another(1);"):
            with self.subTest(body=body), self.assertRaises(ValueError):
                workspace.keyboard_svg_import_module("78766:(e,t,n)=>{" + body + "}", "r")

    def test_normalized_svg_layout_and_lettering_consume_exact_current_enum_branches(self):
        normalization = {"ansi": [1, 5, 8], "iso": [3, 6], "japanese": 12,
                         "united_states": 1, "united_kingdom": 6}
        self.assertEqual([workspace.keyboard_normalized_svg_layout(layout, normalization) for layout in (1, 5, 3, 6, 12, 999)],
                         [1, 1, 6, 6, 12, 1])
        body = 'const c={[r.Z7j.UnitedStates]:"US",[r.Z7j.German]:"GER",[r.Z7j.NotInCurrentEnum]:"XXX"};'
        self.assertEqual(workspace.keyboard_svg_lettering(body, {"UnitedStates": 1, "German": 3}), {1: "US", 3: "GER"})

    def test_inline_png_extraction_retains_complete_module_without_executing_javascript(self):
        content = self.png()
        encoded = base64.b64encode(content)
        source = b'(self.webpackChunk=self.webpackChunk||[]).push([[7],{42:(e,t,n)=>{const bitmap="data:image/png;base64,' + encoded + b'";n.d(t,{bitmap:()=>bitmap});}}]);'
        decoded, proof = workspace.extract_inline_resource_png(source, hashlib.sha256(content).hexdigest())
        self.assertEqual(decoded, content)
        self.assertEqual(proof["source_regions"][0]["complete_module"]["module_id"], 42)
        self.assertIn(encoded.decode(), proof["source_regions"][0]["complete_module"]["body"])
        with self.assertRaises(ValueError):
            workspace.extract_inline_resource_png(source, "0" * 64)

    def test_repeated_inline_png_literals_retain_each_actual_factory_and_quote_form(self):
        content = self.png()
        literal = b"data:image/png;base64," + base64.b64encode(content)
        source = (b'(self.webpackChunk=self.webpackChunk||[]).push([[7],{'
                  b'42:e=>{e.exports="' + literal + b'"},43:e=>{e.exports=\'' + literal + b"'}}]);")
        decoded, proof = workspace.extract_inline_resource_png(source, hashlib.sha256(content).hexdigest())
        self.assertEqual(decoded, content)
        self.assertEqual([region["complete_module"]["module_id"] for region in proof["source_regions"]], [42, 43])
        for region in proof["source_regions"]:
            owner = region["complete_module"]
            self.assertLessEqual(owner["character_start"], region["character_start"])
            self.assertLessEqual(region["character_end"], owner["character_end"])

    def test_inline_keyboard_png_uses_the_exact_css_consumer_and_pinned_hash(self):
        content = self.png()
        digest = hashlib.sha256(content).hexdigest()
        literal = b"data:image/png;base64," + base64.b64encode(content)
        selector = ".action-wrapper.SWITCH_LIGHTING.open .head:before"
        css = selector.encode() + b"{background-image:url(" + literal + b")}.other{background:url(" + literal + b")}"
        original = {"path": "current.css", "bytes": len(css), "sha256": hashlib.sha256(css).hexdigest()}
        found = workspace.keyboard_artwork_inline_png([original], lambda row: css, digest, css_selector=selector)
        self.assertEqual(found[0][1], content)
        self.assertEqual(found[0][2]["complete_css_rules"][0]["body"],
                         selector + "{background-image:url(" + literal.decode() + ")}")
        for kwargs in ({}, {"css_selector": ".absent"}):
            with self.subTest(kwargs=kwargs), self.assertRaises(FileNotFoundError):
                workspace.keyboard_artwork_inline_png([original], lambda row: css, digest, **kwargs)
        with self.assertRaises(FileNotFoundError):
            workspace.keyboard_artwork_inline_png([original], lambda row: css, "0" * 64, css_selector=selector)

    def test_customize_keeps_original_alias_protocol_and_removes_only_source_root_children(self):
        original = (b'<svg xmlns="http://www.w3.org/2000/svg" width="3" height="1">'
                    b'<g id="Body"><g id="Nested_LED"/><path d="M0 0H3V1H0Z"/></g>'
                    b'<g id="Key_LED"/><g id="Key_Selection"/><g id="US_Lettering"/>'
                    b'<g id="GER_Lettering"/><g id="US_Characters"/><g id="GER_Characters"/></svg>')
        producer = {"module_id": 42}
        asset = "synapse/keyboard-current/720/1.fixture.svg"
        base = workspace.keyboard_customize_svg_request(original, asset, producer)
        self.assertEqual(base["display"], "synapse/keyboard-current/720/1.fixture-customize.svg")
        self.assertEqual(base["svg_remove_root_ids"], ["Key_LED", "Key_Selection"])
        request = workspace.keyboard_customize_svg_request(original, asset, producer, normalized=True,
                                                         lettering="GER", layout_id=3)
        self.assertEqual(request["display"], "synapse/keyboard-current/720/1.fixture-customize-customize-layout-3.svg")
        self.assertEqual(request["svg_remove_root_ids"], ["Key_LED", "Key_Selection", "US_Lettering", "US_Characters"])
        content, receipt = workspace.filter_svg_root_children(original, request["svg_remove_root_ids"])
        import xml.etree.ElementTree as ET
        tree = ET.fromstring(content)
        self.assertEqual([child.get("id") for child in tree], ["Body", "GER_Lettering", "GER_Characters"])
        self.assertTrue(any(child.get("id") == "Nested_LED" for child in tree.iter()))
        self.assertEqual(receipt["removed_root_ids"], request["svg_remove_root_ids"])
        for invalid in ([], ["absent"], ["Nested_LED"], ["Key_LED", "Key_LED"]):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                workspace.filter_svg_root_children(original, invalid)

    def test_svg_is_actually_rasterized_with_independent_exact_color_and_alpha_pixels(self):
        original = (b'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1">'
                    b'<rect width="1" height="1" fill="#112233"/></svg>')
        metadata, pixels = workspace.decoded_svg_rgba(original)
        self.assertEqual((metadata["width"], metadata["height"], metadata["visible_pixels"]), (2, 1, 1))
        self.assertEqual(pixels, bytes([17, 34, 51, 255, 0, 0, 0, 0]))
        self.assertFalse(metadata["system_fonts"])
        with self.assertRaises(ValueError):
            workspace.decoded_svg_rgba(b'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"/>')
        for dependency in (b'<image href="file:///private.png"/>', b'<image href="https://example.invalid/a.png"/>',
                           b'<use href="#absent"/>', b'<text>font dependent</text>',
                           b'<style>@import "https://example.invalid/a.css";</style>',
                           b'<rect style="fill:url(https://example.invalid/fill.svg)"/>'):
            with self.subTest(dependency=dependency), self.assertRaises(ValueError):
                workspace.decoded_svg_rgba(b'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1">' + dependency + b'</svg>')

    def test_pixel_comparison_rejects_wrong_color_alpha_dimensions_and_truncation(self):
        original = self.png()
        self.assertTrue(workspace.compare_resource_pixels(original, original, "PNG")["all_rgba_pixels_equal"])
        for invalid in (self.png(changed=(18, 29, 43, 255)), self.png(changed=(17, 29, 43, 254)),
                        self.png(size=(1, 1)), original[:40]):
            with self.subTest(bytes=len(invalid)), self.assertRaises((OSError, ValueError, SyntaxError)):
                workspace.compare_resource_pixels(original, invalid, "PNG")

    def fixture(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name).resolve()
        inputs = root / ".analysis/inputs/frontend/keyboard-720-current"
        inputs.mkdir(parents=True)
        def store(name, raw):
            path = inputs / name
            path.write_bytes(raw)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(raw),
                    "sha256": hashlib.sha256(raw).hexdigest(), "http_status": 200,
                    "url": "https://apps.razer.com/synapse/products/720/ui/" + name}
        media = store("1.fixture.png", self.png())
        entry = store("index.html", b'<html><body></body></html>')
        manifest = store("asset-manifest.json", json.dumps({"files": {"product.png": "1.fixture.png"}}).encode())
        receipt = inputs / "acquisition.json"
        acquisition = {"entry_url": entry["url"], "source_current_during_acquisition": True,
                       "stability": [{"unchanged": True}], "files": [entry, manifest, media], "edges": []}
        receipt.write_text(json.dumps(acquisition), encoding="utf-8")
        catalog = root / ".analysis/inputs/typed-tables/keyboard.json"
        catalog.parent.mkdir(parents=True)
        catalog.write_text(json.dumps({"products": [self.product()]}), encoding="utf-8")
        for symbol in ("KEYBOARD_SOURCE_ARTWORK_DATA", "KEYBOARD_KEYPAD_ARTWORK_DATA",
                       "KEYBOARD_KEYPAD_DEVICE_IMAGE_DATA", "KEYBOARD_MAPPING_BLADE_ARTWORK_DATA"):
            (catalog.parent / (symbol + ".json")).write_text("null" if symbol == "KEYBOARD_SOURCE_ARTWORK_DATA" else "[]", encoding="utf-8")
        physical = "assets/synapse/products/720/pinned.png"
        locked = {"path": physical, "bytes": media["bytes"], "sha256": media["sha256"]}
        mappings = {"tables": [{"name": "ASSETS", "entries": [{"key": self.image()["asset"], "path": physical}]}]}
        tools = root / "tools"
        tools.mkdir()
        (tools / "resource-mappings.json").write_text(json.dumps(mappings), encoding="utf-8")
        lock = {"schema_version": 2, "format": "files", "git_files": [],
                "groups": [{"name": "shared", "requires": [], "files": []},
                           {"name": "product-720", "requires": ["shared"], "files": [locked]}],
                "storage": {"provider": "huggingface", "repository": "owner/assets", "revision": "a" * 40}}
        lock["source_sha256"] = workspace.resource_sources.content_hash({"groups": lock["groups"], "git_files": []})
        lock["tag"] = "assets-" + lock["source_sha256"][:20]
        (tools / "asset-release.json").write_text(json.dumps(lock), encoding="utf-8")
        return root, media, receipt, acquisition, catalog, mappings

    def verify_fixture(self, fixture, decoder=None):
        root, _, _, _, catalog, mappings = fixture
        def catalog_path(crate, symbol, **kwargs):
            return catalog if symbol == "KEYBOARD_ARTWORK_CURRENT_DATA" else catalog.parent / (symbol + ".json")
        def load_mappings(*, input_rows, **kwargs):
            path = root / "tools/resource-mappings.json"
            raw = path.read_bytes()
            input_rows.append({"path": "tools/resource-mappings.json", "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
            return mappings
        with (mock.patch.object(workspace, "ROOT", root),
              mock.patch.object(workspace, "RESOURCE_AUDIT", root / ".analysis/results/resource-audit.json"),
              mock.patch.object(workspace.resource_sources, "builtin_data_path", side_effect=catalog_path),
              mock.patch.object(workspace.resource_sources, "load_resource_mappings", side_effect=load_mappings),
              mock.patch.object(workspace, "image_analyzer_verification_inputs", return_value=[]),
              mock.patch.object(workspace.urllib.request, "urlopen", side_effect=AssertionError("offline verification attempted network")),
              mock.patch.object(workspace, "decoded_resource_rgba", side_effect=decoder) if decoder else nullcontext()):
            return workspace.verify_resources(scope="keyboard-artwork", product_ids=[720])

    def test_offline_full_verification_reconstructs_pinned_bytes_without_assets_pack_or_cargo_cache(self):
        fixture = self.fixture()
        report = self.verify_fixture(fixture)
        self.assertTrue(report["passed"], report)
        self.assertEqual(report["summary"]["verified"], 1)
        self.assertTrue(report["entries"][0]["pixel_decode_performed"])
        self.assertFalse((fixture[0] / "assets").exists())
        self.assertFalse((fixture[0] / "target").exists())
        self.assertFalse(report["network_access"])

    def test_mismatching_delivery_retains_actual_reconstruction_without_marking_success(self):
        fixture = self.fixture()
        root, media, _, _, _, _ = fixture
        path = root / "tools/asset-release.json"
        lock = json.loads(path.read_bytes())
        lock["groups"][1]["files"][0]["sha256"] = "b" * 64
        lock["source_sha256"] = workspace.resource_sources.content_hash({"groups": lock["groups"], "git_files": []})
        lock["tag"] = "assets-" + lock["source_sha256"][:20]
        path.write_text(json.dumps(lock), encoding="utf-8")
        report = self.verify_fixture(fixture)
        entry = report["entries"][0]
        self.assertEqual(entry["actual_output"], {"bytes": media["bytes"], "sha256": media["sha256"]})
        self.assertEqual(entry["output"]["sha256"], "b" * 64)
        self.assertTrue(entry["source_verified"])
        self.assertFalse(entry["conversion_verified"])
        self.assertFalse(entry["passed"])
        self.assertFalse(report["passed"])

    def test_cached_original_requires_two_complete_http_responses_and_actual_matching_bytes(self):
        fixture = self.fixture()
        root, media, receipt_path, acquisition, _, _ = fixture
        media.pop("http_status")
        media["kind"] = "installed_cached_official_resource"
        media["deployment_current_verified"] = False
        attempts = []
        for index in range(2):
            response = root / media["path"]
            path = response.with_name(f"sample-{index}.png")
            path.write_bytes(response.read_bytes())
            attempts.append({**media, "path": path.relative_to(root).as_posix(), "final_url": media["url"],
                             "http_status": 200, "status": 200, "matches_retained": True})
        media["current_http_revalidation"] = {"attempts": attempts}
        receipt_path.write_text(json.dumps(acquisition), encoding="utf-8")
        report = self.verify_fixture(fixture)
        self.assertTrue(report["passed"], report)
        proof = report["official_http_revalidations"][media["path"]]
        self.assertEqual([row["path"] for row in proof], [row["path"] for row in attempts])
        self.assertTrue(all(row in report["inputs"] for row in proof))
        for field, value in (("status", 500), ("http_status", 500), ("matches_retained", False),
                             ("url", "https://example.invalid/wrong.png"), ("final_url", None),
                             ("sha256", "0" * 64), ("bytes", 0)):
            with self.subTest(field=field):
                previous = attempts[1][field]
                attempts[1][field] = value
                receipt_path.write_text(json.dumps(acquisition), encoding="utf-8")
                self.assertFalse(self.verify_fixture(fixture)["passed"])
                attempts[1][field] = previous
        receipt_path.write_text(json.dumps(acquisition), encoding="utf-8")
        (root / attempts[1]["path"]).write_bytes(self.png(changed=(18, 29, 43, 255)))
        self.assertFalse(self.verify_fixture(fixture)["passed"])

    def test_changed_missing_and_unstable_originals_are_failures_and_do_not_fetch(self):
        for failure in ("changed", "missing", "unstable", "missing_delivery", "unverified_installed_cache"):
            with self.subTest(failure=failure):
                fixture = self.fixture()
                root, media, receipt, acquisition, _, mappings = fixture
                if failure == "changed":
                    (root / media["path"]).write_bytes(self.png(changed=(17, 29, 43, 254)))
                elif failure == "missing":
                    (root / media["path"]).unlink()
                elif failure == "unstable":
                    acquisition["stability"][0]["unchanged"] = False
                    receipt.write_text(json.dumps(acquisition), encoding="utf-8")
                elif failure == "unverified_installed_cache":
                    acquired_media = acquisition["files"][2]
                    acquired_media.pop("http_status")
                    acquired_media.update(kind="installed_cached_official_resource", deployment_current_verified=False)
                    receipt.write_text(json.dumps(acquisition), encoding="utf-8")
                else:
                    mappings["tables"][0]["entries"].clear()
                report = self.verify_fixture(fixture)
                self.assertFalse(report["passed"])
                self.assertEqual(report["summary"]["failed"], 1)
                if failure == "missing_delivery":
                    self.assertEqual(report["summary"]["missing_delivery_identities"], 1)
                    self.assertTrue(report["entries"][0]["pixel_decode_performed"])
                else:
                    self.assertFalse(report["entries"][0]["source_verified"])

    def test_input_mutation_during_real_decode_invalidates_the_receipt(self):
        fixture = self.fixture()
        original_decoder = workspace.decoded_resource_rgba
        def mutate(content, expected_format):
            result = original_decoder(content, expected_format)
            fixture[2].write_bytes(fixture[2].read_bytes() + b" ")
            return result
        report = self.verify_fixture(fixture, decoder=mutate)
        self.assertFalse(report["passed"])
        self.assertFalse(report["inputs_unchanged"])
        self.assertIn(fixture[2].relative_to(fixture[0]).as_posix(), report["changed_inputs"])


class ResourcePublicationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.source = self.root / ".analysis/inputs/original/image.png"
        self.source.parent.mkdir(parents=True)
        self.source.write_bytes(b"inert image payload")
        self.receipt = self.source.parent / "acquisition.json"
        self.source_row = {"path": self.source.relative_to(self.root).as_posix(),
            "bytes": self.source.stat().st_size, "sha256": hashlib.sha256(self.source.read_bytes()).hexdigest(), "http_status": 200}
        self.receipt.write_text(json.dumps({"source_current_during_acquisition": True,
            "stability": [{"unchanged": True}], "files": [self.source_row]}), encoding="utf-8")
        self.manifest = ".analysis/inputs/prepared-resources/publication.json"
        self.mapping = {self.source_row["path"]: {"display": "synapse/retained.png", "table": "MODULE_SERVICE_ASSETS"}}
        import analysis
        import asset_release
        for patch in (mock.patch.object(workspace, "ROOT", self.root),
                      mock.patch.object(workspace.resource_sources, "load_resource_mappings", return_value={"tables": [{"name": "MODULE_SERVICE_ASSETS"}]}),
                      mock.patch.object(asset_release, "load_lock", return_value={"groups": [{"name": "shared"}]}),
                      mock.patch.object(asset_release, "release_additions", return_value=[]),
                      mock.patch.object(analysis, "index_files")):
            patch.start()
            self.addCleanup(patch.stop)

    def prepare(self, manifest=None):
        return workspace.prepare_resource_publication(self.receipt.relative_to(self.root).as_posix(),
            self.mapping, manifest or self.manifest)

    def test_payload_and_provenance_are_staged_only_under_analysis_inputs(self):
        report = self.prepare()
        item = report["files"][0]
        self.assertEqual((self.root / item["source"]["path"]).read_bytes(), self.source.read_bytes())
        self.assertTrue(item["source"]["path"].startswith(".analysis/inputs/prepared-resources/"))
        self.assertFalse((self.root / "assets").exists())
        prepared = json.loads((self.root / item["acquisition"]["path"]).read_bytes())
        self.assertEqual(prepared["derivations"][0]["original"], self.source_row)
        self.assertEqual(hashlib.sha256((self.root / item["acquisition"]["path"]).read_bytes()).hexdigest(), item["acquisition"]["sha256"])

    def test_inline_source_can_publish_two_visibility_branches_with_complete_geometry(self):
        import sys
        import xml.etree.ElementTree as ET
        analyzer_path = str(Path(__file__).resolve().parent / "analyzers/python-static")
        analyzer_imports = mock.patch.object(sys, "path", [analyzer_path, *sys.path])
        analyzer_imports.start()
        self.addCleanup(analyzer_imports.stop)
        self.source = self.source.with_suffix(".js")
        self.source.write_text('const Diagram=(e,t)=>{return R.createElement("svg",{viewBox:"0 0 20 20"},R.createElement("g",{id:"primary"},R.createElement("path",{d:"M1 2H3"})),R.createElement("g",{id:"surround"},R.createElement("path",{d:"M4 5H6"})))};', encoding="utf-8")
        self.source_row = {"path": self.source.relative_to(self.root).as_posix(),
            "bytes": self.source.stat().st_size, "sha256": hashlib.sha256(self.source.read_bytes()).hexdigest(), "http_status": 200}
        self.receipt.write_text(json.dumps({"source_current_during_acquisition": True,
            "stability": [{"unchanged": True}], "files": [self.source_row]}), encoding="utf-8")
        self.mapping = {self.source_row["path"]: [{"display": f"synapse/{branch}.svg", "table": "MODULE_SERVICE_ASSETS",
            "inline_react_svg": 'Diagram=(e,t)=>{', "svg_group_visibility": {
                "primary": "visible" if branch == "primary" else "hidden",
                "surround": "visible" if branch == "surround" else "hidden"}} for branch in ("primary", "surround")]}
        report = self.prepare()
        self.assertEqual(len(report["files"]), 2)
        for item in report["files"]:
            tree = ET.fromstring((self.root / item["source"]["path"]).read_bytes())
            self.assertEqual([node.get("d") for node in tree.iter("path")], ["M1 2H3", "M4 5H6"])
            visible = Path(item["alias"]).stem
            self.assertEqual(tree.find(f"g[@id='{visible}']").get("style"), "visibility:visible")
        proof = json.loads((self.root / report["acquisition"]["path"]).read_bytes())
        self.assertEqual(len(proof["derivations"]), 2)
        self.assertTrue(all(row["transformation"]["source_regions"] for row in proof["derivations"]))
        self.assertFalse((self.root / "assets").exists())

    def test_incompatible_static_transformations_fail_before_staging(self):
        for values in ({"muted_video_animation": False},
                       {"muted_video_animation": True, "inline_react_svg": "Diagram="},
                       {"muted_video_animation": True, "svg_group_visibility": {"primary": "visible"}},
                       {"muted_video_animation": True, "svg_remove_root_ids": ["Key_LED"], "producer": {}},
                       {"svg_remove_root_ids": ["Key_LED"]},
                       {"producer": {}},
                       {"svg_remove_root_ids": ["Key_LED"], "producer": {}, "inline_react_svg": "Diagram="}):
            with self.subTest(values=values):
                self.mapping[self.source_row["path"]] = {
                    "display": "synapse/invalid.png", "table": "MODULE_SERVICE_ASSETS", **values}
                with self.assertRaisesRegex(ValueError, "exclusive transformation"):
                    self.prepare()
                self.assertFalse((self.root / self.manifest).exists())

    def svg_filter_fixture(self):
        self.source = self.source.with_suffix(".svg")
        self.source.write_bytes(b'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><g id="Device"><path d="M1 2H3"/></g><g id="EU_Selection"><path d="M4 5H6"/></g></svg>')
        self.source_row = {"path": self.source.relative_to(self.root).as_posix(),
            "bytes": self.source.stat().st_size, "sha256": hashlib.sha256(self.source.read_bytes()).hexdigest(), "http_status": 200}
        self.producer = self.source.with_name("producer.js")
        self.producer.write_text('self.webpackChunkexample.push([[1],{42:(e,t,n)=>{const label="设备";function filter(svg){return svg.children.filter(e=>e.id.includes("_Selection"))}console.error("Error fetching/parsing SVG:")}}]);', encoding="utf-8")
        self.producer_row = {"path": self.producer.relative_to(self.root).as_posix(),
            "bytes": self.producer.stat().st_size, "sha256": hashlib.sha256(self.producer.read_bytes()).hexdigest(), "http_status": 200}
        self.receipt.write_text(json.dumps({"source_current_during_acquisition": True,
            "stability": [{"unchanged": True}], "files": [self.source_row, self.producer_row]}), encoding="utf-8")
        producer = {"path": self.producer_row["path"], "sha256": self.producer_row["sha256"],
            "anchor": '"Error fetching/parsing SVG:"', "module_id": 42, "boundary_parser": "tree-sitter-javascript"}
        self.mapping = {self.source_row["path"]: [
            {"display": "synapse/base.svg", "table": "MODULE_SERVICE_ASSETS"},
            {"display": "synapse/customize.svg", "table": "MODULE_SERVICE_ASSETS",
             "svg_remove_root_ids": ["EU_Selection"], "producer": producer}]}
        return self.mapping[self.source_row["path"]][1]

    def test_svg_root_filter_retains_current_producer_and_base_original(self):
        import xml.etree.ElementTree as ET
        self.svg_filter_fixture()
        report = self.prepare()
        self.assertEqual(len(report["files"]), 2)
        base, customize = [(self.root / item["source"]["path"]).read_bytes() for item in report["files"]]
        self.assertEqual(base, self.source.read_bytes())
        svg = ET.fromstring(customize)
        self.assertEqual(svg.attrib, {"viewBox": "0 0 20 10"})
        self.assertEqual([node.get("id") for node in svg], ["Device"])
        self.assertEqual(next(svg.iter("{http://www.w3.org/2000/svg}path")).get("d"), "M1 2H3")
        proof = json.loads((self.root / report["acquisition"]["path"]).read_bytes())
        producer = proof["derivations"][1]["transformation"]["producer"]
        self.assertEqual(producer["input"], self.producer_row)
        module = producer["complete_module"]
        self.assertEqual(module["module_id"], 42)
        self.assertEqual(self.producer.read_bytes()[module["byte_start"]:module["byte_end"]].decode(), module["body"])
        self.assertIn('function filter(svg)', module["body"])
        self.assertFalse((self.root / "assets").exists())

    def test_svg_root_filter_rejects_unacquired_mutated_and_wrong_module_producers(self):
        for kind in ("unacquired", "mutated", "wrong_module", "wrong_anchor"):
            with self.subTest(kind=kind):
                request = self.svg_filter_fixture()
                if kind == "unacquired":
                    receipt = json.loads(self.receipt.read_bytes())
                    receipt["files"] = [self.source_row]
                    self.receipt.write_text(json.dumps(receipt), encoding="utf-8")
                elif kind == "mutated":
                    self.producer.write_bytes(self.producer.read_bytes() + b"\n// mutation")
                elif kind == "wrong_module":
                    request["producer"]["module_id"] = 43
                else:
                    request["producer"]["anchor"] = '"absent owner"'
                with self.assertRaises(ValueError):
                    self.prepare()
                self.assertFalse((self.root / ".analysis/inputs/prepared-resources").exists())

    def test_svg_root_filter_rejects_nested_or_absent_nodes_before_staging(self):
        request = self.svg_filter_fixture()
        request["svg_remove_root_ids"] = ["missing-root"]
        with self.assertRaisesRegex(ValueError, "absent or repeated root nodes"):
            self.prepare()
        self.assertFalse((self.root / ".analysis/inputs/prepared-resources").exists())

    def test_mutated_original_and_retained_payload_cannot_be_published(self):
        item = self.prepare()["files"][0]
        retained = self.root / item["source"]["path"]
        retained.write_bytes(b"mutated cache")
        with self.assertRaisesRegex(ValueError, "Retained prepared resource bytes"):
            self.prepare()
        self.source.write_bytes(b"mutated original")
        with self.assertRaisesRegex(ValueError, "original bytes differ"):
            self.prepare()

    def test_output_cannot_replace_original_receipt_or_escape_analysis_inputs(self):
        for manifest in ("assets/payload.json", self.source_row["path"],
                         self.receipt.relative_to(self.root).as_posix(),
                         ".analysis/inputs/prepared-resources/acquisition.json"):
            with self.subTest(manifest=manifest), self.assertRaises(ValueError):
                self.prepare(manifest)


class RuntimeResourceReferenceTests(unittest.TestCase):
    def test_rust_ast_distinguishes_comments_raw_strings_macros_and_test_owners(self):
        source = b'''// "synapse/comment.svg"
fn display() { let _ = img(r#"synapse/current.svg"#); format!("synapse/current-{frame:03}.svg"); }
#[cfg(test)]
mod tests { fn fake() { let _ = "synapse/fixture.svg"; } }
#[test]
fn verify() { let _ = "synapse/check.svg"; }
'''
        rows = workspace.rust_resource_literals(source, owner="fixture.rs")
        self.assertEqual([r["literal"] for r in rows], ["synapse/current.svg", "synapse/current-{frame:03}.svg",
            "synapse/fixture.svg", "synapse/check.svg"])
        self.assertEqual([r["test_only"] for r in rows], [False, False, True, True])
        for row in rows:
            self.assertIn(row["literal"].encode(), source[row["byte_start"]:row["byte_end"]])
            for owner in row["context"]:
                self.assertEqual(hashlib.sha256(source[owner["byte_start"]:owner["byte_end"]]).hexdigest(), owner["body_sha256"])

    def test_include_expression_preserves_original_literal_byte_offsets(self):
        source = '// 数据\n&[("asset", "synapse/expression.svg")]'.encode()
        [row] = workspace.rust_resource_literals(source, owner="expression.rs")
        self.assertEqual(row["line"], 2)
        self.assertEqual(source[row["byte_start"]:row["byte_end"]], b'"synapse/expression.svg"')

    def test_named_branch_item_keeps_complete_bytes_and_ignores_other_items(self):
        raw = '// 分支\nconst OTHER: &str = "unrelated.svg";\nconst NAMES: &[&str] = &["one", r#"two"#];'.encode()
        with tempfile.TemporaryDirectory(dir=workspace.ROOT) as directory:
            path = Path(directory) / "branches.rs"
            path.write_bytes(raw)
            names, owner = workspace.rust_named_item_strings(workspace.relative(path), "NAMES")
        self.assertEqual(names, ["one", "two"])
        body = raw[owner["byte_start"]:owner["byte_end"]]
        self.assertEqual(body.decode(), owner["body"])
        self.assertEqual(hashlib.sha256(body).hexdigest(), owner["body_sha256"])

    def test_ambiguous_named_branch_item_is_rejected(self):
        with tempfile.TemporaryDirectory(dir=workspace.ROOT) as directory:
            path = Path(directory) / "branches.rs"
            path.write_text('mod a { const NAMES: &str = "one"; } mod b { const NAMES: &str = "two"; }', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "found 2"):
                workspace.rust_named_item_strings(workspace.relative(path), "NAMES")


if __name__ == "__main__":
    unittest.main()
