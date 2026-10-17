"""Pure byte tests for analyzer wheel identity and extraction boundaries."""
import base64
import csv
import hashlib
import io
import unittest
import warnings
import zipfile
import copy
import json
from pathlib import Path
import sqlite3
import tempfile
from unittest import mock

import workspace
import resource_database
import families
import typed_tables


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
        with sqlite3.connect(":memory:") as connection:
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
