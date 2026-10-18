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


class ArchitectureBoundaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import resource_scope
        cls.rust_parser = resource_scope.parser(workspace.ROOT)

    def inspect(self, source, owner):
        return workspace.inspect_architecture_source(source.encode(), owner=owner, rust_parser=self.rust_parser)

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
        source = ('struct AppShell { thx_retired_owners: Vec<X>, receiver_chroma_connect: '
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

    def test_matcher_completion_publishes_only_the_joined_worker_release_result(self):
        owner = 'crates/razer-service/src/mouse/dpi_matcher_worker.rs'
        source = ('impl Handle { fn finish(mut self) { let joined = self.worker.take().unwrap().join(); '
                  'match joined { Ok(exit) => { self.completion.publish(&exit.released); '
                  'merge_results([exit.operation,exit.cleanup,exit.released]) }, '
                  'Err(error) => { let result = Err(error); self.completion.publish(&result); result } } } }')
        self.assertFalse(self.inspect(source,owner))
        for changed in (source.replace('&exit.released','&exit.operation'),
                        source.replace('&exit.released','&exit.cleanup'),
                        source.replace('.join()',''),
                        source.replace('let joined =','self.completion.publish(&exit.released); let joined =')
                              .replace('Ok(exit) => { self.completion.publish(&exit.released);','Ok(exit) => {')):
            self.assertEqual(self.inspect(changed,owner)[0]['kind'],
                             'matcher_completion_does_not_publish_joined_release')
        self.assertFalse(self.inspect(source.replace('impl Handle','impl Unrelated'),owner))

    def test_matcher_save_requires_complete_workspace_scope_session_and_connection(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_matcher.rs'
        source = ('fn mouse_matcher_save_current(&self, save: &LocalSave, cx: &App) -> bool { '
                  'save.scope.is_valid() '
                  '&& save.workspace.read(cx).connection_revision() == save.connection_revision '
                  '&& save.workspace.read(cx).mouse_matcher_scope(cx).as_ref() == Some(&save.scope) '
                  '&& self.mouse_matcher.sessions.get(&save.identity).is_some_and(|session| { '
                  'session.id == save.session_id && session.workspace.upgrade().as_ref() == Some(&save.workspace) }) }')
        self.assertFalse(self.inspect(source, owner))
        changes = ('save.scope.is_valid()',
                   'save.workspace.read(cx).connection_revision() == save.connection_revision',
                   'save.workspace.read(cx).mouse_matcher_scope(cx).as_ref() == Some(&save.scope)',
                   'session.id == save.session_id',
                   'session.workspace.upgrade().as_ref() == Some(&save.workspace)')
        for condition in changes:
            with self.subTest(condition=condition):
                self.assertEqual(self.inspect(source.replace(condition, 'true'), owner)[0]['kind'],
                                 'matcher_save_accepts_stale_session_or_connection')
        self.assertEqual(self.inspect(source.replace('&&', '||', 1), owner)[0]['kind'],
                         'matcher_save_accepts_stale_session_or_connection')
        self.assertFalse(self.inspect(source, 'crates/razer-shell/src/shell/unrelated.rs'))

    def test_retiring_matcher_delivery_cannot_remove_the_replacement_session(self):
        owner = 'crates/razer-shell/src/shell/mouse/mouse_matcher.rs'
        source = ('fn dispatch_mouse_matcher() { if shell.mouse_matcher.sessions.get(&retained_identity) '
                  '.is_some_and(|session| session.id == session_id) { '
                  'shell.retire_mouse_matcher(&retained_identity, cx); } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('session.id == session_id', 'true'),
                        source.replace('.get(&retained_identity)', '.get(&other_identity)'),
                        'fn dispatch_mouse_matcher() { shell.retire_mouse_matcher(&retained_identity, cx); }'):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'matcher_retired_worker_removes_successor')
        self.assertFalse(self.inspect('fn unrelated() { shell.retire_mouse_matcher(&retained_identity, cx); }', owner))

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

    def test_existing_capability_directory_is_stable_when_root_protocol_is_organized(self):
        root = Path("crates/razer-device/src/protocol.rs")
        capability = Path("crates/razer-device/src/protocol/onboard_macro.rs")
        self.assertEqual(workspace.source_layout_destination(root), Path("crates/razer-device/src/transport/protocol.rs"))
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
        native = "crates/razer-service/src/runtime/windows/macro_recording.rs"
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
        for method, contract in (("suspend_macro_mappings", "SuspensionReceipt"), ("resume_macro_mappings", "RestoreReceipt")):
            source = 'use razer_ipc::macro_recording::' + contract + ' as Receipt; impl NativeRuntime { fn ' + method + '() { Receipt::new(value); } }'
            self.assertFalse(self.inspect(source, native))
            self.assertEqual(self.inspect(source.replace('Receipt::new', 'Other::new'), native)[0]["kind"], "native_macro_receipt_bypasses_shared_contract")
        source = 'fn shutdown_macro_recording() { self.resume_macro_mapping_receipts().and_then(confirm_mapping_restoration); }'
        self.assertFalse(self.inspect(source, native))
        self.assertEqual(self.inspect(source.replace('confirm_mapping_restoration', 'ignore_source_false'), native)[0]["kind"], "native_macro_cleanup_accepts_unconfirmed_restore")
        self.assertFalse(self.inspect('fn unrelated() { self.resume_macro_mapping_receipts(); }', native))

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
        owner = "crates/razer-service/src/runtime/windows/mapping_execution.rs"
        source = ('use razer_ipc::mapping_control::Receipt as Confirmation; impl NativeRuntime { fn set_mapping_execution() { '
                  'anyhow::ensure!(self.global_shortcuts.owner() == owner || self.mapping_device.owner_guid() == owner, "owner"); '
                  'anyhow::ensure!(self.revision.is_none_or(|prior| revision > prior), "stale"); '
                  'Confirmation::with_observations(owner,revision,target,observations); } }')
        self.assertFalse(self.inspect(source, owner))
        for changed in (source.replace('Confirmation::with_observations', 'Other::with_observations'),
                        source.replace('self.mapping_device.owner_guid()', 'device'), source.replace('.is_none_or(', '.map(')):
            self.assertEqual(self.inspect(changed, owner)[0]["kind"], "native_mapping_control_bypasses_owner_revision_receipt")
        self.assertFalse(self.inspect('fn unrelated() { Other::with_observations(); }', owner))

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
                  'if self.save_task.is_some() { return; } '
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
                  'if self.save_task.is_some() { return; } '
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
                  'if self.save_task.is_some() { return; } '
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

    def test_service_hooks_are_consumed_once_and_all_completions_are_retained_until_exit(self):
        owner = 'crates/razer-shell/src/shell/shutdown.rs'
        register = ('fn register_exit_cleanup() { self.service_exit.hooks.push('
                    'Box::new(move |shell,cx| Box::pin(cleanup(shell,cx)))); }')
        self.assertFalse(self.inspect(register, owner))
        for changed in (register.replace('self.service_exit.hooks.push(', 'other.push('),
                        register.replace('cleanup(shell,cx)', 'other(shell,cx)'),
                        register.replace('self.service_exit.hooks.push(', 'drop(')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'exit_cleanup_registration_bypasses_shared_owner')
        take = ('fn take_exit_cleanups() { std::mem::take(&mut self.service_exit.hooks).into_iter()'
                '.map(|cleanup| cleanup(self,cx)).collect() }')
        self.assertFalse(self.inspect(take, owner))
        self.assertFalse(self.inspect('use std::mem::take as consume; ' + take.replace('std::mem::take(', 'consume('), owner))
        for changed in (take.replace('std::mem::take(&mut self.service_exit.hooks)', 'self.service_exit.hooks.clone()'),
                        take.replace('cleanup(self,cx)', 'unrelated(self,cx)'), take.replace('.collect()', ''),
                        take.replace('.into_iter()', '.into_iter().take(1)')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'exit_cleanup_hooks_have_no_single_consuming_owner')
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

    def test_actuation_exit_preserves_admitted_writes_and_rechecks_failure_before_final_cleanup(self):
        owner = 'crates/razer-shell/src/shell.rs'
        wait = 'if self.finish_keyboard_actuation_exit(cx) || !self.close_requested { return; }'
        source = ('fn continue_save_queue() { if !succeeded { self.cancel_pending_exit(cx); } '
                  'if self.save_task.is_some() { return; } '
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
        owner = 'crates/razer-shell/src/shell/mouse/mouse_polling_write.rs'
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
                       'input_worker: Option<JoinHandle<anyhow::Result<()>>>, output_worker: Option<JoinHandle<anyhow::Result<()>>> }')
        self.assertFalse(self.inspect(declaration, owner))
        self.assertEqual(self.inspect(declaration.replace('input_worker: Option<JoinHandle<anyhow::Result<()>>>', 'input_worker: bool'), owner)[0]['kind'],
                         'ipc_client_discards_owned_process_or_pipe_workers')
        shutdown = ('impl ServiceClient { fn stop() { self.requests.take(); self.stopped = true; self.job.take(); '
                    'reap_child(&mut self.child)?; let input = join_pipe(self.input_worker.take(),"input"); '
                    'let output = join_pipe(self.output_worker.take(),"output"); combine_cleanup([input,output]) } }')
        self.assertFalse(self.inspect(shutdown, owner))
        for changed in (shutdown.replace('self.requests.take();', ''), shutdown.replace('self.stopped = true;', ''),
                        shutdown.replace('self.job.take();', ''),
                        shutdown.replace('self.stopped = true;', '').replace('reap_child(&mut self.child)?;',
                            'reap_child(&mut self.child)?; self.stopped = true;'),
                        shutdown.replace('join_pipe(self.input_worker.take(),"input");', 'join_pipe(self.input_worker.take(),"input")?;'),
                        shutdown.replace('reap_child(&mut self.child)?;', 'reap_child(&mut self.child);'),
                        shutdown.replace('reap_child(&mut self.child)?;', '').replace(
                            'combine_cleanup([input,output])', 'reap_child(&mut self.child)?; combine_cleanup([input,output])'),
                        shutdown.replace('self.output_worker.take()', 'self.output_worker.clone()'),
                        shutdown.replace('combine_cleanup([input,output])', 'input')):
            self.assertEqual(self.inspect(changed, owner)[0]['kind'], 'ipc_cleanup_does_not_preserve_owner_until_reap_and_pipe_joins')
        destructor = 'impl Drop for ServiceClient { fn drop(&mut self) { if let Err(error) = self.stop() { log(error); } } }'
        self.assertFalse(self.inspect(destructor, owner))
        self.assertEqual(self.inspect(destructor.replace('if let Err(error) = self.stop()', 'if !self.stopped'), owner)[0]['kind'],
                         'ipc_drop_skips_unfinished_cleanup_after_admission_closed')
        reaping = 'fn reap_child() { if let Some(child) = child.as_mut() { terminate_child(child)?; } child.take(); Ok(()) }'
        self.assertFalse(self.inspect(reaping, owner))
        self.assertEqual(self.inspect(reaping.replace('terminate_child(child)?', 'terminate_child(child)'), owner)[0]['kind'],
                         'ipc_reap_discards_unconfirmed_process_owner')
        retained = ('use std::thread; fn spawn_with_program() { client.input_worker = Some(thread::Builder::new().spawn(run)?); '
                    'client.output_worker = Some(thread::spawn(run)); }')
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
                  'body.dismiss_editors(); audio.prepare_exit(); } fn finish_exit() { body.finish_mapping_service(); } '
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
                      "crates/razer-platform/src/platform/windows/session_changes.rs",
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
                      "crates/razer-service/src/runtime/windows/global_shortcuts.rs",
                      "crates/razer-service/src/runtime/windows/mapping_execution.rs",
                      "crates/razer-ipc/src/global_shortcut_capture.rs",
                      "crates/razer-service/src/runtime/windows/global_shortcut_capture.rs",
                      "crates/razer-pages/src/features/workspace/product_workspace.rs",
                      "crates/razer-pages/src/features/workspace/source_workspace/lifecycle.rs",
                      "crates/razer-ipc/src/macro_recording.rs", "crates/razer-service/src/runtime/windows/macro_recording.rs",
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
                parameters='()', runtime_module='95524', flag='scanning=false', second_call=''):
        root = Path(directory)
        name = '.analysis/inputs/frontend/receiver-current/raw/main.js'
        raw = ('self.webpackChunk.push([[1],{34340:(mod,exports,require)=>{'
               'const runtime=require(' + runtime_module + ');let scanning=true;'
               'const commands={DUALLINK_CANCEL:' + parameters + '=>Promise.resolve().then(async()=>{'
               'const target=' + receiver + ';' + flag + ';await target.deviceScan(' + status + ');'
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
        with tempfile.TemporaryDirectory() as directory:
            root, source = self.fixture(directory)
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
            self.assertEqual(audit.owners[contract['scanning_flag_cleanup_owner']]['body'], 'scanning=false')

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
                         {'second_call': 'await target.deviceScan(4);'}):
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


class RustValidationBoundaryTests(unittest.TestCase):
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
            'mouse-philips-native': ('razer-service', 'philips_measurement::tests::'),
            'keyboard-latest-restore-state': ('razer-device', 'keyboard_latest_restore::tests::'),
            'keyboard-latest-restore-shell': ('razer-shell', 'shell::keyboard_latest_restore::tests::'),
            'keyboard-latest-restore-publisher': ('razer-shell', 'shell::keyboard_latest_restore::publisher::tests::'),
            'keyboard-game-lifecycle': ('razer-shell', 'game_monitor::lifecycle::tests::'),
            'keyboard-actuation-lifecycle': ('razer-shell', 'keyboard_actuation_write::worker::tests::'),
            'session-monitor-lifecycle': ('razer-platform', 'session_changes::tests::'),
            'device-change-monitor-lifecycle': ('razer-platform', 'device_changes'),
            'settings-runtime-state': ('razer-settings', 'runtime_page::tests::'),
            'ipc-client': ('razer-ipc', 'client_tests'),
            'mouse-input-monitoring-ipc': ('razer-ipc', 'mouse_input_monitoring'),
            'mouse-input-monitoring-native': ('razer-service', 'mouse_input_monitoring::tests'),
            'device-task-locks': ('razer-ipc', 'device_tasks'),
            'mouse-matcher-service': ('razer-service', 'dpi_matcher'),
            'mouse-matcher-protocol': ('razer-device', 'dpi_matcher'),
            'mouse-matcher-model': ('razer-model', 'mouse_matcher'),
            'mouse-matcher-revision': ('razer-pages', 'matcher_durable_version'),
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
            'source-text-tracking': ('razer-widgets', 'source_text::tests'),
            'assets-image-lifecycle': ('razer-assets', 'image_cache::lifecycle_tests'),
            'keyboard-macro-submission': ('razer-device', 'keyboard_macro_submission'),
            'onboard-macro': ('razer-device', 'onboard_macro'),
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
