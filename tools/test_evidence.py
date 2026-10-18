"""Pure tests for source provenance and the connected analysis index."""
import argparse
from contextlib import closing, redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import MagicMock, patch

import analysis
import evidence
import host
import families


class SemanticQueryGapTests(unittest.TestCase):
    def test_acquisition_and_analyzer_findings_do_not_become_semantic_failures(self):
        data = {"queries": {"local": {"products": [{"product_id": 740,
            "acquisitions": [{"failed_acquisition": [{"url": "missing.js.map", "error": "404"}],
                              "unresolved_acquisition": ["missing.js.map"],
                              "capture_failures": [{"stage": "unpack", "exit_code": 1}]}],
            "native": {"failures": [{"stage": "hexrays", "error": "call analysis failed"}]},
            "unresolved_chains": ["required local setter is absent"]}]}}}
        rows = analysis.semantic_query_gaps(data, "audit.json")
        self.assertEqual([row["kind"] for row in rows], ["acquisition_query_failure",
            "acquisition_query_reference_unresolved", "analyzer_query_failure", "semantic_query_failure",
            "semantic_query_chain_unresolved"])
        self.assertTrue(all(row["product_id"] == 740 for row in rows))
        self.assertEqual(rows[3]["finding"]["stage"], "hexrays")
        self.assertTrue(rows[3]["pointer"].endswith("/native/failures"))

    def test_nested_products_and_json_pointer_escaping_keep_the_actual_owner(self):
        rows = analysis.semantic_query_gaps({"products": [{"product_id": 691,
            "child/with~name": {"failures": ["missing getter"], "products": [{"product_id": 746,
                "remaining_local_chains": ["serial storage"]}]},
            "irrelevant": {"failures": "unverified"}}]}, "receipt.json")
        self.assertEqual(len(rows), 2)
        self.assertEqual([row["product_id"] for row in rows], [691, 746])
        self.assertEqual(rows[0]["pointer"], "/products/0/child~1with~0name/failures")
        self.assertTrue(all(row["evidence"] == "receipt.json" for row in rows))

    def test_maintenance_gaps_use_actual_recipe_identity_and_keep_projection_limits(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            source = Path(directory) / ".analysis/inputs/typed-tables/fixture/table.json"
            source.parent.mkdir(parents=True)
            source.write_bytes(b'{"value":3}')
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            recipe = {"crate": "fixture", "symbol": "TABLE", "source": source.relative_to(directory).as_posix(),
                      "source_sha256": digest, "maintenance_recovery": {
                          "kind": "current_rust_constructor_projection", "original_source_sha256": "0" * 64},
                      "excluded_evidence_fields": ["$.source"]}
            rows = analysis.typed_maintenance_gaps({"tables": [recipe]})
            self.assertEqual([row["kind"] for row in rows], ["typed_maintenance_source_evidence_pending",
                                                            "typed_maintenance_regeneration_unverified"])
            self.assertEqual(rows[0]["recovery"]["original_source_sha256"], "0" * 64)
            self.assertEqual(rows[0]["maintenance_sha256"], digest)
            source.write_bytes(b'{"value":4}')
            self.assertEqual(analysis.typed_maintenance_gaps({"tables": [recipe]})[0]["kind"],
                             "typed_maintenance_input_identity_mismatch")
            source.unlink()
            self.assertEqual(analysis.typed_maintenance_gaps({"tables": [recipe]})[0]["kind"],
                             "typed_maintenance_input_missing")

    def test_maintenance_regeneration_requires_current_complete_recorded_inputs(self):
        import rust_data
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            source = root / "tools/catalog_data.py"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"inert parser input")
            report = root / ".analysis/results/typed-data.json"
            report.parent.mkdir(parents=True)
            def snapshot(_):
                return [{"path": path.relative_to(root).as_posix(), "bytes": path.stat().st_size,
                         "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                        for path in sorted((root / "tools").glob("*.py"))]
            report.write_text(json.dumps({"maintenance_inputs": {"passed": True, "tables": 0,
                                                                  "inputs": snapshot(None)}}), encoding="utf-8")
            with patch.object(rust_data, "maintenance_verification_inputs", side_effect=snapshot):
                self.assertFalse(analysis.typed_maintenance_gaps({"tables": []}))
                added = root / "tools/new_owner.py"
                added.write_bytes(b"new schema owner")
                self.assertEqual(analysis.typed_maintenance_gaps({"tables": []})[0]["kind"],
                                 "typed_maintenance_regeneration_unverified")
                added.unlink()
                source.write_bytes(b"changed parser input")
                self.assertEqual(analysis.typed_maintenance_gaps({"tables": []})[0]["kind"],
                                 "typed_maintenance_regeneration_unverified")


class NativeChainEvidenceTests(unittest.TestCase):
    def setUp(self):
        import copy
        self.copy = copy.deepcopy
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        patcher = patch.object(analysis, "ROOT", self.root)
        patcher.start()
        self.addCleanup(patcher.stop)

    def record(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return {"path": name, "bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}

    def module(self, name="mapping-engine"):
        original = self.record(".analysis/inputs/" + name + ".bin", b"original")
        source = self.record(".analysis/inputs/" + name + ".js", b"source")
        body = {**self.record(".analysis/results/" + name + ".json", b"function"),
                "full_assembly": True}
        owner = self.record("crates/example/" + name + ".rs", b"implementation")
        return {"name": name, "original": original, "sources": [{"original": source}],
                "native_functions": [body], "native_edges": [], "unresolved_indirect_calls": [],
                "resolved_indirect_calls": [], "failures": [], "rust_owners": [owner],
                "semantic_evidence": {"frontend": source}, "implementation_acceptance": "reviewed partial scope"}

    def fresh(self, previous):
        value = self.copy(previous)
        del value["semantic_evidence"]
        value["implementation_acceptance"] = "not established by source/evidence presence"
        value["native_functions"] = []
        return value

    def test_targeted_refresh_keeps_verified_other_modules_and_supplemental_closure(self):
        old = self.module()
        other = self.module("hid-feature")
        current = self.fresh(old)
        old["unresolved_indirect_calls"] = [{"address": "0x18001", "instruction": "call rax"}]
        current["resolved_indirect_calls"] = [{"address": "0x18001", "resolution": "verified table"}]
        result = analysis.merge_native_chain_modules([old, other], [current])
        self.assertEqual([row["name"] for row in result], ["mapping-engine", "hid-feature"])
        self.assertEqual(result[0]["semantic_evidence"], old["semantic_evidence"])
        self.assertEqual(result[0]["native_functions"], old["native_functions"])
        self.assertEqual(result[0]["implementation_acceptance"], old["implementation_acceptance"])
        self.assertEqual(result[0]["unresolved_indirect_calls"], [])
        self.assertEqual(result[1], other)

    def test_changed_or_removed_rust_owner_does_not_inherit_implementation_acceptance(self):
        old = self.module()
        current = self.fresh(old)
        owner = old["rust_owners"][0]["path"]
        changed = self.record(owner, b"replacement implementation")
        result = analysis.merge_native_chain_modules([old], [current])[0]
        self.assertEqual(result["rust_owners"], [changed])
        self.assertIn("requires a new review", result["implementation_acceptance"])
        self.assertEqual(result["semantic_evidence"], old["semantic_evidence"])
        (self.root / owner).unlink()
        result = analysis.merge_native_chain_modules([old], [self.fresh(old)])[0]
        self.assertEqual(result["rust_owners"], [])
        self.assertIn("requires a new review", result["implementation_acceptance"])

    def test_changed_supplemental_or_unselected_evidence_is_rejected(self):
        old = self.module()
        path = old["semantic_evidence"]["frontend"]["path"]
        self.record(path, b"changed source")
        with self.assertRaisesRegex(ValueError, "Retained native-chain evidence changed"):
            analysis.merge_native_chain_modules([old], [self.fresh(old)])
        with self.assertRaisesRegex(ValueError, "Retained native-chain evidence changed"):
            analysis.merge_native_chain_modules([old], [])

    def test_new_native_identity_does_not_reuse_old_semantics(self):
        old = self.module()
        current = self.fresh(old)
        current["original"] = self.record(".analysis/inputs/new.bin", b"new version")
        result = analysis.merge_native_chain_modules([old], [current])[0]
        self.assertNotIn("semantic_evidence", result)
        self.assertEqual(result["native_functions"], [])
        self.assertEqual(result["implementation_acceptance"], current["implementation_acceptance"])

    def test_capture_tool_identity_is_historical_but_capture_bytes_must_still_match(self):
        old = self.module()
        tool = self.record("tools/ida/capture.py", b"capturing tool")
        old["analyzer"] = {"capture_tool": tool}
        old["capture"] = self.record(".analysis/results/capture.json", b"manifest")
        self.record(tool["path"], b"new tool version")
        result = analysis.merge_native_chain_modules([old], [])[0]
        self.assertEqual(result["analyzer"]["capture_tool"], tool)
        self.record(old["capture"]["path"], b"changed manifest")
        with self.assertRaisesRegex(ValueError, "Retained native-chain evidence changed"):
            analysis.merge_native_chain_modules([old], [])

    def test_failed_refresh_retains_verified_closure_and_reports_the_actual_failure(self):
        old = self.module()
        current = self.fresh(old)
        current["failures"] = [{"kind": "native_chain_query_failure", "error": "missing parser input"}]
        result = analysis.merge_native_chain_modules([old], [current])[0]
        self.assertEqual(result["failures"], current["failures"])
        self.assertEqual(result["semantic_evidence"], old["semantic_evidence"])
        self.assertEqual(result["native_functions"], old["native_functions"])
        missing_original = {"name": old["name"], "failures": current["failures"]}
        result = analysis.merge_native_chain_modules([old], [missing_original])[0]
        self.assertEqual(result["original"], old["original"])
        self.assertEqual(result["native_functions"], old["native_functions"])

    def test_duplicate_modules_and_external_evidence_paths_are_rejected(self):
        old = self.module()
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            analysis.merge_native_chain_modules([old, old], [])
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            analysis.merge_native_chain_modules([], [old, old])
        old["semantic_evidence"]["frontend"]["path"] = "../outside.js"
        with self.assertRaisesRegex(ValueError, "Path outside"):
            analysis.merge_native_chain_modules([old], [])

    def test_incomplete_query_is_saved_before_nonzero_exit_and_retains_verified_evidence(self):
        old = self.module()
        old["ffi_exports"] = []
        destination = self.root / ".analysis/results/native/chain-audit.json"
        analysis.save(destination, {"modules": [old]})
        current = self.fresh(old)
        current["failures"] = [{"kind": "native_chain_query_failure", "error": "missing current original"}]
        with redirect_stdout(io.StringIO()) as output, self.assertRaises(SystemExit) as failed:
            analysis.save_native_chain_report(destination, {"modules": [current], "failures": []})
        self.assertNotEqual(failed.exception.code, 0)
        saved = json.loads(destination.read_bytes())
        self.assertFalse(saved["evidence_query_complete"])
        self.assertEqual(saved["modules"][0]["native_functions"], old["native_functions"])
        self.assertEqual(saved["modules"][0]["failures"], current["failures"])
        self.assertFalse(json.loads(output.getvalue())["evidence_query_complete"])

    def test_complete_evidence_query_returns_without_claiming_runtime_acceptance(self):
        current = self.module()
        current["ffi_exports"] = []
        destination = self.root / ".analysis/results/native/chain-audit.json"
        with redirect_stdout(io.StringIO()):
            result = analysis.save_native_chain_report(destination, {
                "modules": [current], "failures": [], "runtime_acceptance": "unverified"})
        self.assertTrue(result["evidence_query_complete"])
        self.assertEqual(result["runtime_acceptance"], "unverified")
        self.assertEqual(json.loads(destination.read_bytes()), result)


class FeatureReportInventoryTests(unittest.TestCase):
    def test_default_inventory_uses_all_typed_products_without_legacy_json_writes(self):
        import typed_tables
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            path = root / typed_tables.READ_OUTPUT
            path.parent.mkdir(parents=True)
            raw = b'''pub(crate) fn products() -> Vec<DeviceReadCapability> {
                vec![DeviceReadCapability { product_id: 124 }, DeviceReadCapability { product_id: 190 },
                     DeviceReadCapability { product_id: 679 }, DeviceReadCapability { product_id: 742 }]
            }'''
            path.write_bytes(raw)
            with patch.object(analysis, "builtin_data_path", side_effect=AssertionError("retired JSON access")), \
                    patch.object(typed_tables, "current_read_sources", side_effect=AssertionError("narrow source subset")), \
                    patch.object(analysis, "save", side_effect=AssertionError("evidence query mutated runtime data")):
                result = analysis.feature_report_callers(None, {})
            self.assertEqual([row["product_id"] for row in result["products"]],
                             [124, 190, 679, 742, 631, 658, 756])
            self.assertEqual(result["capabilities"]["sha256"], hashlib.sha256(raw).hexdigest())
            self.assertFalse(result["source_owners_complete"])
            self.assertTrue(all(row["failures"] for row in result["products"]))
            self.assertEqual(path.read_bytes(), raw)
            self.assertFalse(list(root.rglob("*.json")))

    def test_explicit_selection_does_not_require_typed_inventory_or_add_other_products(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            result = analysis.feature_report_callers(None, {}, selected_capabilities=[{"product_id": 190}])
        self.assertEqual([row["product_id"] for row in result["products"]], [190])
        self.assertFalse(result["source_owners_complete"])

    def test_typed_inventory_rejects_dynamic_duplicate_or_unowned_product_rows(self):
        import typed_tables
        for content in [
            b'fn products() { vec![DeviceReadCapability { product_id: 124 }, delegated()] }',
            b'fn products() { vec![DeviceReadCapability { product_id: 124 }, DeviceReadCapability { product_id: 124 }] }',
            b'fn test_only() { vec![DeviceReadCapability { product_id: 124 }] }',
            b'fn products() { vec![DeviceReadCapability { product_id: SELECTED_PRODUCT }] }',
        ]:
            with self.subTest(content=content), self.assertRaises(ValueError):
                typed_tables.read_table_constructors(content)


class FontFixtureEvidenceTests(unittest.TestCase):
    @staticmethod
    def fonts():
        import zlib
        def checksum(raw):
            raw += b'\0' * (-len(raw) % 4)
            return sum(struct.unpack('>' + 'I' * (len(raw) // 4), raw)) & 0xffffffff
        tables = [(b'head', b'\0' * 12), (b'name', b'font name ' * 8)]
        offsets, data, offset = [], bytearray(), 44
        for tag, body in tables:
            offsets.append((tag, checksum(body), offset, len(body)))
            data.extend(body)
            data.extend(b'\0' * (-len(body) % 4))
            offset += (len(body) + 3) & ~3
        fixture = bytearray(struct.pack('>IHHHH', 0x10000, 2, 32, 1, 0))
        fixture.extend(b''.join(struct.pack('>4sIII', *row) for row in offsets))
        fixture.extend(data)
        struct.pack_into('>I', fixture, 44 + 8, (0xb1b0afba - checksum(bytes(fixture))) & 0xffffffff)
        entries, packed, offset = [], bytearray(), 84
        for tag, body in tables:
            compressed = zlib.compress(body)
            payload = compressed if len(compressed) < len(body) else body
            entries.append(struct.pack('>4sIIII', tag, offset, len(payload), len(body), checksum(body)))
            packed.extend(payload)
            packed.extend(b'\0' * (-len(payload) % 4))
            offset += (len(payload) + 3) & ~3
        original = (struct.pack('>4sIIHHIHHIIIII', b'wOFF', 0x10000, offset, 2, 0, len(fixture), 1, 0, 0, 0, 0, 0, 0)
                    + b''.join(entries) + packed)
        return bytes(original), bytes(fixture)

    def test_all_font_tables_and_whole_sfnt_checksum_are_independent_requirements(self):
        original, fixture = self.fonts()
        verified = families.verify_woff_sfnt_fixture(original, fixture)
        self.assertEqual(verified['tables'], 2)
        self.assertTrue(verified['complete_table_bytes_equal'])
        changed_table = bytearray(fixture)
        changed_table[-1] ^= 1
        changed_checksum = bytearray(fixture)
        changed_checksum[52] ^= 1
        repeated_tag = bytearray(original)
        repeated_tag[64:68] = b'head'
        escaped_offset = bytearray(original)
        struct.pack_into('>I', escaped_offset, 48, len(original) + 4)
        for source, target in ((original, changed_table), (original, changed_checksum),
                               (repeated_tag, fixture), (escaped_offset, fixture),
                               (original[:50], fixture), (original, fixture[:12])):
            with self.subTest(source_bytes=len(source), target_bytes=len(target)), self.assertRaises(ValueError):
                families.verify_woff_sfnt_fixture(source, target)


class KeyboardLocalControlQueryTests(unittest.TestCase):
    def test_complete_utf8_callables_keep_only_their_actual_roles(self):
        sys.path.insert(0, str(analysis.ROOT / "tools/analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        parser = Parser(Language(tree_sitter_javascript.language()))
        raw = ('const label="校准"; const modules={31:(e,t,n)=>{'
               'function getter(){return "getIndicator";} '
               'const setter=v=>{if(v)return "setIndicator";return false;};'
               'function unrelated(){return 42;} }};').encode()
        regions = families.keyboard_local_control_regions(raw, {"getter": "getIndicator", "setter": "setIndicator"}, parser)
        self.assertEqual(len(regions), 1)
        self.assertEqual(regions[0]["owner"]["module_id"], 31)
        callables = regions[0]["callables"]
        self.assertEqual([roles for _, roles in callables], [["getter", "setter"], ["getter"], ["setter"]])
        self.assertEqual([raw[node.start_byte:node.end_byte] for node, _ in callables[1:]], [
            b'function getter(){return "getIndicator";}',
            b'v=>{if(v)return "setIndicator";return false;}'])
        with self.assertRaisesRegex(ValueError, "grammar failed"):
            families.keyboard_local_control_regions(b"const invalid = {", {"getter": "getIndicator"}, parser)


class MappingEngineAuditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, str(Path(__file__).parent / "analyzers/python-static"))

    @staticmethod
    def bundle(scancode=30, factory=77898):
        key = {"name": "A", "inputID": "KEY_A", "type": "Alphanumeric", "scancode": scancode,
               "analogKeyID": 31, "virtualKey": "65", "keyCode": "65", "flag": 0}
        return ('({101:(e,t)=>{t.KEYS=' + json.dumps([key]) + ';},'
                '102:(e,t)=>{var scroll="ScrollUp",si=[scroll],sc=[scroll],delay=30;'
                'var exports={SINGLE_STATE_INPUT_ID:()=>si,SCROLL_INPUT_ID:()=>sc,SCROLL_TURBO_DELAY_MS:()=>delay};},'
                '103:(e,t)=>{t.DoubleClickEvent=class{getData(){return[{type:"mouse",id:1,data:0}]}}},'
                '104:(e,t,n)=>{function select(e){switch(e){case"DoubleClick":return new(0,n.DoubleClickEvent);'
                'case"Play":return new(0,n.PlayPauseEvent);}}},'
                + str(factory) + ':(e,t)=>{this.generateDefaultTurbos=()=>(this.defaultTurbos=["DoubleClick"]);},'
                '106:(e,t)=>{function system(e){return e.systemGroup.systemAssignment;}}})').encode()

    def fixture(self, root, *, wrapper_factory=77898):
        tool = root / "tools/families.py"
        tool.parent.mkdir()
        tool.write_bytes(b"inert analyzer identity")
        folder = root / ".analysis/inputs/frontend/mouse-mw-182-current"
        folder.mkdir(parents=True)
        wrapper = ('({32226:(e,t,n)=>{var o=n(' + str(wrapper_factory) + ');t.A=o.RzAppEngineMapping_default},'
                   '30525:(e,t,n)=>{var a=n(32226);a.A.generateDefaultTurbos();a.A.generateAppEngineMultiMappings(e);'
                   'a.A.generateAppEngineMappings(e);a.A.generateAppEngineData(t);}})').encode()
        rows = []
        for name, raw in (("7795.current.js", self.bundle()), ("2520.unused.js", self.bundle(99)), ("6259.current.js", wrapper)):
            path = folder / name
            path.write_bytes(raw)
            rows.append({"path": path.relative_to(root).as_posix(), "bytes": len(raw),
                         "sha256": hashlib.sha256(raw).hexdigest()})
        (folder / "acquisition.json").write_text(json.dumps({"source_current_during_acquisition": True, "files": rows}))
        return rows

    def test_182_decodes_its_actual_chunk_and_retains_active_factory_chain(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            self.fixture(root)
            args = argparse.Namespace(family="mouse", function="mw", product=[182], prepare=False, receipt_only=True)
            with patch.object(families, "__file__", str(root / "tools/families.py")), redirect_stdout(io.StringIO()):
                families.audit_mapping_engine_literals(args)
            data = json.loads((root / ".analysis/results/frontend/mapping-engine-current-data.json").read_bytes())
            self.assertEqual(list(data), ["182"])
            self.assertEqual(data["182"]["keys"][0]["scancode"], 30)
            query = json.loads((root / ".analysis/results/frontend/mouse-mw-audit.json").read_bytes())["queries"]["mapping-engine"]
            product = query["products"][0]
            self.assertTrue(product["original"]["path"].endswith("7795.current.js"))
            self.assertEqual(product["active_factory"]["factory_module"], 77898)
            for module in product["complete_modules"].values():
                self.assertEqual(module["byte_end"] - module["byte_start"], len(module["body"].encode()))
                self.assertEqual(module["sha256"], hashlib.sha256(module["body"].encode()).hexdigest())

    def test_182_missing_corrupt_or_unselected_factory_cannot_fall_back_to_unused_chunk(self):
        for issue in ("missing", "corrupt", "wrong_wrapper"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                rows = self.fixture(root, wrapper_factory=999 if issue == "wrong_wrapper" else 77898)
                path = root / rows[0]["path"]
                if issue == "missing":
                    path.unlink()
                elif issue == "corrupt":
                    path.write_bytes(path.read_bytes() + b"changed")
                args = argparse.Namespace(family="mouse", function="mw", product=[182], prepare=False, receipt_only=True)
                with self.assertRaises(ValueError), redirect_stdout(io.StringIO()):
                    families.audit_mapping_engine_literals(args)
                self.assertFalse((root / ".analysis/results/frontend/mapping-engine-current-data.json").exists())


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.root_patch = patch.object(analysis, "ROOT", self.root)
        self.root_patch.start()

    def tearDown(self):
        self.root_patch.stop()
        self.directory.cleanup()

    def source(self, name, raw=b"original"):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def native_capture(self, digest, input_bytes):
        folder = ".analysis/results/native/" + digest
        manifest = {"input_sha256": digest, "input_bytes": input_bytes, "image_base": "0x180000000",
                    "capture_complete": True, "functions": [], "functions_captured": 1,
                    "functions_total": 1, "settled_functions_total": 1, "failures": [], "outputs": []}
        function = self.source(folder + "/functions/1000.json", b'{"synthetic":"inert function capture"}')
        manifest["functions"].append({"path": "functions/1000.json", "rva": "0x1000", "address": "0x180001000",
            "assembly_captured": True, "pseudocode_captured": False,
            "sha256": hashlib.sha256(function.read_bytes()).hexdigest()})
        for name in ("segments", "imports", "strings", "entrypoints", "names", "local_types", "structures"):
            output = self.source(folder + "/" + name + ".json", b"[]")
            manifest[name + "_count"] = 0
            manifest["outputs"].append({"path": name + ".json", "bytes": output.stat().st_size,
                                         "sha256": hashlib.sha256(output.read_bytes()).hexdigest()})
        for name in ("all-functions.c", "all-functions.asm"):
            output = self.source(folder + "/" + name, b"synthetic inert static output")
            manifest["outputs"].append({"path": name, "bytes": output.stat().st_size,
                                         "sha256": hashlib.sha256(output.read_bytes()).hexdigest()})
        self.source(folder + "/capture.json", json.dumps(manifest).encode())
        return manifest, folder

    def ida_runtime(self, members, expected=None):
        stream = io.BytesIO()
        with zipfile.ZipFile(stream, "w") as archive:
            for name, payload in members.items():
                archive.writestr(name, payload)
        raw = stream.getvalue()
        archive_name = "python-3.11.9-embed-amd64.zip"
        files = members if expected is None else expected
        receipt = {"url": "https://www.python.org/ftp/python/3.11.9/" + archive_name,
                   "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                   "files": [{"path": name, "bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
                             for name, payload in {**files, archive_name: raw}.items()]}
        self.source("tools/analyzers/python-3.11.9/acquisition.json", json.dumps(receipt).encode())
        self.source(".work/analyzers/" + archive_name, raw)
        return receipt

    def test_ida_runtime_restores_exact_members_and_archive_without_target_execution_or_network(self):
        receipt = self.ida_runtime({"python311.dll": b"inert analyzer fixture", "python311._pth": b"python311.zip"})
        with patch.object(analysis.urllib.request, "urlopen", side_effect=AssertionError("Verified cache must not download")):
            result = analysis.prepare_ida_python()
            self.assertEqual(result["restored_members"], 3)
            for member in receipt["files"]:
                path = self.root / "tools/analyzers/python-3.11.9" / member["path"]
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), member["sha256"])
            self.assertEqual(analysis.prepare_ida_python()["restored_members"], 0)

    def test_ida_runtime_rejects_member_mismatch_before_installing_any_file(self):
        self.ida_runtime({"python311.dll": b"different"}, {"python311.dll": b"expected"})
        with self.assertRaisesRegex(ValueError, "member differs"):
            analysis.prepare_ida_python()
        self.assertFalse((self.root / "tools/analyzers/python-3.11.9/python311.dll").exists())

    def test_ida_runtime_rejects_unconfined_receipt_paths(self):
        self.ida_runtime({"../outside.dll": b"inert"})
        with self.assertRaisesRegex(ValueError, "invalid or duplicate"):
            analysis.prepare_ida_python()

    def test_identity_claims_do_not_turn_missing_or_changed_bytes_into_success(self):
        name = ".analysis/inputs/test/source.js"
        self.source(name)
        graph = evidence.EvidenceGraph()
        graph.artifact(name, {"bytes": 8, "sha256": hashlib.sha256(b"original").hexdigest()}, "owner", "/input")
        self.assertEqual(graph.nodes[name]["status"], "verified")
        graph.artifact(name, {"sha256": "0" * 64}, "other", "/input")
        self.assertEqual(graph.nodes[name]["status"], "identity_mismatch")
        missing = ".analysis/inputs/test/absent.js"
        graph.artifact(missing, {"sha256": "0" * 64})
        self.assertEqual(graph.nodes[missing]["status"], "missing")

    def test_unchanged_index_batch_does_not_republish_and_changes_keep_other_inputs(self):
        first = {"path": ".analysis/inputs/a", "bytes": 1, "sha256": "a" * 64}
        second = {"path": ".analysis/inputs/b", "bytes": 2, "sha256": "b" * 64}
        path = self.source(".analysis/inputs.json", json.dumps({"inputs": [first, second]}).encode())
        before = path.read_bytes()
        with patch.object(analysis, "save", side_effect=AssertionError("Unchanged index must not be republished")):
            analysis.index_files([dict(first), dict(second)])
        self.assertEqual(path.read_bytes(), before)
        changed = {**first, "bytes": 3, "sha256": "c" * 64}
        analysis.index_files([changed])
        self.assertEqual(json.loads(path.read_bytes())["inputs"], [changed, second])

    def test_verified_publication_records_prior_identity_and_rejects_unverified_bytes(self):
        name = ".analysis/inputs/test/acquisition.json"
        source = self.source(name, b"{}")
        old = {"path": name, "bytes": 3, "sha256": "0" * 64, "kind": "acquisition_receipt"}
        index = self.source(".analysis/inputs.json", json.dumps({"inputs": [old]}).encode())
        row = {**old, "bytes": 2, "sha256": analysis.sha(source)}
        publication = {"owner": "tools/families.py", "receipt": name, "reason": "Verified supplemental source",
                       "verification": {"source_membership": True}}
        analysis.index_files([row], publication=publication)
        recorded = json.loads(index.read_bytes())
        change = recorded["source_publications"][0]["changes"][0]
        self.assertEqual(change["previous"], old)
        self.assertEqual(change["current"], row)
        self.assertEqual(recorded["source_publications"][0]["reason"], publication["reason"])
        before = index.read_bytes()
        source.write_bytes(b"changed after receipt")
        with self.assertRaisesRegex(ValueError, "byte identity differs"):
            analysis.index_files([row], publication=publication)
        self.assertEqual(index.read_bytes(), before)

    def frontend_publication(self):
        endpoint = "https://apps.razer.com/test/"
        rows = []
        for filename, raw in (("index.html", b'<script src="main.js"></script>'),
                              ("asset-manifest.json", b'{"files":{"main":"main.js"},"entrypoints":["main.js"]}'),
                              ("main.js", b"/* inert synthetic source */")):
            path = self.source(".analysis/inputs/frontend/test/raw/" + filename, raw)
            rows.append({"path": analysis.rel(path), "bytes": len(raw), "sha256": analysis.sha(path),
                         "url": endpoint + ("" if filename == "index.html" else filename),
                         "acquired_at_utc": "synthetic", "http_status": 200})
        rows.append({"url": endpoint + "main.js.map", "http_status": 404, "error": "synthetic missing debug map"})
        receipt = {"schema_version": 1, "entry_url": endpoint, "files": rows,
                   "stability": [{"url": row["url"], "before_sha256": row["sha256"],
                                  "after_sha256": row["sha256"], "unchanged": True} for row in rows[:2]],
                   "source_current_during_acquisition": True, "declared_references_fetched": False,
                   "edges": [{"from": endpoint + "main.js", "to": endpoint + "main.js.map", "kind": "source-map"}]}
        path = self.source(".analysis/inputs/frontend/test/acquisition.json", json.dumps(receipt).encode())
        index = self.source(".analysis/inputs.json", b'{"inputs": []}')
        return path, receipt, index

    def test_frontend_publication_indexes_complete_receipt_preserves_failures_and_checks_stability(self):
        path, receipt, index = self.frontend_publication()
        before = path.read_bytes()
        with patch.object(analysis.urllib.request, "urlopen", side_effect=AssertionError("Static reconciliation must not download")):
            result = families.publish_frontend_acquisition(path)
        self.assertEqual(path.read_bytes(), before)
        self.assertEqual(len(json.loads(index.read_bytes())["inputs"]), 4)
        self.assertFalse(result["declared_references_fetched"])
        self.assertEqual(result["reference_gaps"]["declared_source_maps_missing"], 1)
        self.assertTrue(result["all_retained_bytes_verified"])
        receipt["stability"][0]["after_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "stability claim differs"):
            families.publish_frontend_acquisition(path, receipt)
        self.assertEqual(path.read_bytes(), before)

    def test_frontend_publication_rejects_any_changed_original_before_republishing_receipt(self):
        path, receipt, index = self.frontend_publication()
        before = path.read_bytes()
        (self.root / receipt["files"][2]["path"]).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "byte identity differs"):
            families.publish_frontend_acquisition(path, receipt)
        self.assertEqual(path.read_bytes(), before)
        self.assertEqual(json.loads(index.read_bytes())["inputs"], [])

    def frontend_revalidation_publication(self):
        path, receipt, index = self.frontend_publication()
        original = receipt["files"][2]
        response = self.source(".analysis/inputs/frontend/test/official-byte-revalidation/main.js",
                               (self.root / original["path"]).read_bytes())
        attempt = {**{key: original[key] for key in ("bytes", "sha256", "url", "acquired_at_utc")},
                   "path": analysis.rel(response), "final_url": original["url"],
                   "status": 200, "http_status": 200, "matches_retained": True}
        original["current_http_revalidation"] = {"attempts": [dict(attempt), dict(attempt)]}
        path.write_text(json.dumps(receipt), encoding="utf-8")
        return path, receipt, index, response

    def test_frontend_publication_indexes_verified_revalidation_alias_once_with_its_source(self):
        path, receipt, index, response = self.frontend_revalidation_publication()
        before = path.read_bytes()
        with patch.object(analysis.urllib.request, "urlopen", side_effect=AssertionError("Static reconciliation must not download")):
            families.publish_frontend_acquisition(path)
        self.assertEqual(path.read_bytes(), before)
        published = json.loads(index.read_bytes())
        self.assertEqual(len(published["inputs"]), 5)
        aliases = [row for row in published["inputs"] if row.get("revalidation_of")]
        self.assertEqual(len(aliases), 1)
        self.assertEqual(aliases[0]["path"], analysis.rel(response))
        self.assertEqual(aliases[0]["revalidation_of"], receipt["files"][2]["path"])
        self.assertEqual(aliases[0]["acquisition_receipt"], analysis.rel(path))
        self.assertTrue(published["source_publications"][0]["verification"]["all_retained_bytes_verified"])
        self.assertEqual(published["source_publications"][0]["verification"]["retained_http_response_aliases"], 1)

    def test_frontend_publication_rejects_missing_or_changed_revalidation_bytes_before_any_publication(self):
        for mutation in ("changed", "missing"):
            with self.subTest(mutation=mutation):
                path, receipt, index, response = self.frontend_revalidation_publication()
                before = path.read_bytes()
                response.write_bytes(b"changed") if mutation == "changed" else response.unlink()
                with self.assertRaises((ValueError, FileNotFoundError)):
                    families.publish_frontend_acquisition(path, receipt)
                self.assertEqual(path.read_bytes(), before)
                self.assertEqual(json.loads(index.read_bytes())["inputs"], [])

    def test_frontend_publication_rejects_unverified_revalidation_claims_without_rewriting_evidence(self):
        for field, value in (("url", "https://other.invalid/main.js"),
                             ("final_url", "https://other.invalid/main.js"),
                             ("status", 404), ("http_status", 404),
                             ("matches_retained", False), ("sha256", "0" * 64),
                             ("bytes", 1), ("path", None), ("acquired_at_utc", None)):
            with self.subTest(field=field):
                path, receipt, index, _ = self.frontend_revalidation_publication()
                receipt["files"][2]["current_http_revalidation"]["attempts"][0][field] = value
                before = path.read_bytes()
                with self.assertRaisesRegex(ValueError, "response does not verify"):
                    families.publish_frontend_acquisition(path, receipt)
                self.assertEqual(path.read_bytes(), before)
                self.assertEqual(json.loads(index.read_bytes())["inputs"], [])

    def test_frontend_publication_rejects_single_or_foreign_response_owner(self):
        for mutation in ("single", "foreign", "original"):
            with self.subTest(mutation=mutation):
                path, receipt, index, _ = self.frontend_revalidation_publication()
                row = receipt["files"][2]
                attempts = row["current_http_revalidation"]["attempts"]
                if mutation == "single":
                    attempts.pop()
                elif mutation == "original":
                    attempts[0]["path"] = row["path"]
                else:
                    foreign = self.source(".analysis/inputs/frontend/other/response.js",
                                          (self.root / row["path"]).read_bytes())
                    attempts[0]["path"] = analysis.rel(foreign)
                before = path.read_bytes()
                with self.assertRaisesRegex(ValueError, "two retained official responses|outside its acquisition owner"):
                    families.publish_frontend_acquisition(path, receipt)
                self.assertEqual(path.read_bytes(), before)
                self.assertEqual(json.loads(index.read_bytes())["inputs"], [])

    def test_reference_audit_indexes_full_tree_and_preserves_adaptations(self):
        folder = ".analysis/references/test"
        commit = "f" * 40
        raw_files = {"Cargo.toml": b'[workspace]\nmembers=[]\n', "src/inert.rs": b"// inert reference bytes\n"}
        entries = []
        for name, raw in raw_files.items():
            self.source(folder + "/" + name, raw)
            blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
            entries.append(f"100644 blob {blob} {len(raw)}\t{name}".encode())
        receipt = self.source(folder + "/acquisition.json", json.dumps({"url": "https://github.com/owner/test",
            "commit": commit, "adaptations": [{"scope": "architecture only"}]}).encode())
        index = self.source(".analysis/inputs.json", b'{"inputs": []}')
        def run(command, **kwargs):
            if command == ["git", "--version"]:
                return argparse.Namespace(stdout="git synthetic static fixture")
            operation = command[3]
            if operation == "rev-parse" and command[4] == "--show-toplevel":
                return argparse.Namespace(stdout=str(self.root / folder).encode())
            return argparse.Namespace(stdout={"rev-parse": commit.encode(), "diff": b"", "ls-tree": b"\0".join(entries) + b"\0"}[operation])
        with patch.object(analysis.subprocess, "run", side_effect=run), redirect_stdout(io.StringIO()):
            result = analysis.audit_reference(argparse.Namespace(directory=folder, commit=commit,
                check_upstream=False, restore_original_bytes=False))
        self.assertEqual(result["adaptations"], [{"scope": "architecture only"}])
        self.assertEqual(len(result["files"]), 2)
        self.assertEqual(len(json.loads(index.read_bytes())["inputs"]), 3)
        self.assertEqual(json.loads(receipt.read_bytes())["adaptations"], result["adaptations"])
        with patch.object(analysis.subprocess, "run", side_effect=AssertionError("Detached snapshot must not invoke Git")), redirect_stdout(io.StringIO()):
            retained = analysis.audit_reference(argparse.Namespace(directory=folder, commit=commit,
                check_upstream=False, restore_original_bytes=False, retained_snapshot=True))
        self.assertEqual(retained["audited_at_utc"], result["audited_at_utc"])
        self.assertFalse(retained["retained_snapshot_verification"]["upstream_rechecked"])
        (self.root / folder / "src/inert.rs").write_bytes(b"changed bytes")
        with self.assertRaisesRegex(ValueError, "byte identity differs"):
            analysis.audit_reference(argparse.Namespace(directory=folder, commit=commit,
                check_upstream=False, restore_original_bytes=False, retained_snapshot=True))

    def test_historical_and_outside_paths_are_never_read(self):
        graph = evidence.EvidenceGraph()
        with patch.object(analysis, "sha", side_effect=AssertionError("Must not hash excluded evidence")):
            graph.artifact(".ref/frontend/source.js", owner="current")
            graph.artifact(".work/latest-source-check/host-4.0.821/source.js", owner="current")
            self.assertIsNone(graph.artifact("../outside.js"))
            self.assertIsNone(graph.artifact("C:/outside.js"))
        self.assertFalse(any(node.get("sha256") for node in graph.nodes.values()))

    def test_native_function_paths_are_relative_to_the_capture(self):
        owner = ".analysis/results/native/abc/capture.json"
        name = ".analysis/results/native/abc/functions/1000.json"
        self.source(name, b"{}")
        graph = evidence.EvidenceGraph()
        graph.walk({"functions": [{"path": "functions/1000.json", "bytes": 2,
                                   "sha256": hashlib.sha256(b"{}").hexdigest()}]}, owner)
        self.assertEqual(graph.nodes[name]["status"], "verified")
        self.assertEqual(next(iter(graph.edges.values()))["from"], owner)

    def test_modifications_during_collection_invalidate_the_snapshot(self):
        name = ".analysis/inputs/test/source.js"
        path = self.source(name)
        graph = evidence.EvidenceGraph()
        graph.artifact(name)
        os.utime(path, ns=(path.stat().st_atime_ns, path.stat().st_mtime_ns + 10_000_000))
        self.assertEqual(graph.final_snapshot(), [name])
        self.assertEqual(graph.nodes[name]["status"], "changed_during_connected_audit")

    def test_long_paths_keep_hashes_receipts_and_snapshots_portable(self):
        name = ".analysis/inputs/test/" + "SyntheticType_" + "a" * 225 + ".cs"
        receipt = ".analysis/results/test/" + "SyntheticReceipt_" + "b" * 225 + ".json"
        raw = b"synthetic managed output"
        expected = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        paths = [self.root / name, self.root / receipt]

        # Create the fixtures independently of the evidence helper so a broken
        # conversion cannot make this test silently exercise short filenames.
        def fixture_path(path):
            return Path("\\\\?\\" + str(path)) if os.name == "nt" else path

        for path in paths:
            self.assertGreater(len(str(path)), 260)
            self.assertLessEqual(len(path.name), 255)
            path.parent.mkdir(parents=True, exist_ok=True)
        try:
            fixture_path(paths[0]).write_bytes(raw)
            fixture_path(paths[1]).write_text(json.dumps({"outputs": [expected]}), encoding="utf-8")
            graph = evidence.EvidenceGraph()
            self.assertEqual(graph.document(paths[1], required=True), {"outputs": [expected]})
            self.assertEqual(graph.nodes[name]["status"], "verified")
            self.assertEqual(graph.nodes[name]["sha256"], expected["sha256"])
            self.assertEqual(graph.final_snapshot(), [])
            self.assertEqual(set(graph.nodes), {name, receipt})
            self.assertEqual(graph.nodes[name]["claims"][0]["owner"], receipt)
            self.assertEqual(next(iter(graph.edges.values()))["to"], name)
            self.assertFalse(graph.failures)

            fixture_path(paths[0]).write_bytes(b"changed synthetic output with another length")
            self.assertEqual(graph.final_snapshot(), [name])
            self.assertEqual(graph.nodes[name]["status"], "changed_during_connected_audit")
        finally:
            # Only these two owned synthetic files use long leaf names; remove
            # them before TemporaryDirectory cleans its short parent paths.
            for path in paths:
                fixture_path(path).unlink(missing_ok=True)

    def test_rejected_or_failed_receipts_are_never_opened(self):
        historical = self.source(".ref/frontend/receipt.json", b"{}")
        allowed = self.source(".analysis/results/test/receipt.json", b"{}")
        graph = evidence.EvidenceGraph()
        with patch.object(Path, "read_bytes", side_effect=AssertionError("Must not read rejected receipts")):
            self.assertIsNone(graph.document(historical))
            with self.assertRaises(ValueError):
                graph.document(self.root.parent / "synthetic-outside-receipt.json")
            with patch.object(evidence, "_evidence_filesystem_path", side_effect=ValueError("Synthetic boundary failure")):
                self.assertIsNone(graph.document(allowed))
        self.assertEqual([row["kind"] for row in graph.failures],
                         ["receipt_path_rejected", "receipt_read_prevented"])


class CaptureStreamingTests(unittest.TestCase):
    setUp = EvidenceTests.setUp
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source
    native_capture = EvidenceTests.native_capture

    def test_chain_edge_links_complete_function_bytes_without_quadratic_xref_copies(self):
        raw = json.dumps({"assembly": [{"bytes": "c3", "text": "retn"}],
                          "pseudocode": {"lines": [{"text": "return;"}]},
                          "xrefs_to_entry": [{"from": hex(index), "to": "0x140001000"} for index in range(1000)]}).encode()
        path = self.source(".analysis/results/native/test/functions/1000.json", raw)
        original = {"path": analysis.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                    "name": "synthetic_function", "rva": "0x1000", "address": "0x140001000", "end": "0x140001001",
                    "full_assembly": True, "full_pseudocode": True, "xrefs_to_entry": json.loads(raw)["xrefs_to_entry"],
                    "chunks": [{"raw_hex": "c3"}]}
        reference = analysis.native_chain_function_reference(original)
        self.assertEqual(hashlib.sha256((self.root / reference["path"]).read_bytes()).hexdigest(), reference["sha256"])
        self.assertEqual(reference["bytes"], len(raw))
        self.assertTrue(reference["full_assembly"] and reference["full_pseudocode"])
        self.assertNotIn("xrefs_to_entry", reference)
        self.assertNotIn("chunks", reference)
        self.assertEqual(len(original["xrefs_to_entry"]), 1000)
        self.assertLess(len(json.dumps([reference] * 1000)), len(json.dumps([original] * 10)))

    def test_dataset_writer_keeps_one_row_and_publishes_complete_unicode_json(self):
        import gc
        import weakref
        sys.path.insert(0, str(Path(__file__).resolve().parent / "ida"))
        import capture_io

        class Row(dict):
            pass

        refs, progress = [], []
        path = self.root / "dataset.json"
        path.write_bytes(b"prior dataset")

        def rows():
            for index in range(12):
                row = Row(index=index, text="设备\nstring")
                refs.append(weakref.ref(row))
                yield row
                del row
                gc.collect()
                self.assertLessEqual(sum(ref() is not None for ref in refs), 1)

        count = capture_io.write_json_array(path, rows(), progress=progress.append, interval=4)
        self.assertEqual(count, 12)
        self.assertEqual(progress, [4, 8, 12])
        self.assertEqual(json.loads(path.read_bytes()), [{"index": index, "text": "设备\nstring"} for index in range(12)])
        self.assertEqual(capture_io.file_sha256(path), hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertFalse(path.with_name(path.name + ".tmp").exists())

    def test_interrupted_dataset_keeps_previous_complete_output(self):
        sys.path.insert(0, str(Path(__file__).resolve().parent / "ida"))
        import capture_io
        path = self.source("dataset.json", b"previous complete bytes")

        def rows():
            yield {"index": 1}
            raise RuntimeError("synthetic static enumeration failure")

        with self.assertRaisesRegex(RuntimeError, "enumeration failure"):
            capture_io.write_json_array(path, rows())
        self.assertEqual(path.read_bytes(), b"previous complete bytes")
        self.assertFalse(path.with_name(path.name + ".tmp").exists())

    def test_aggregate_writer_keeps_all_bodies_and_decompilation_failures_explicit(self):
        sys.path.insert(0, str(Path(__file__).resolve().parent / "ida"))
        import capture_io
        rows = [
            {"name": "配置", "rva": "0x10", "address": "0x140000010",
             "pseudocode": {"lines": [{"text": "int setting() { return 7; }"}]},
             "assembly": [{"address": "0x140000010", "bytes": "b807000000", "text": "mov eax, 7"}]},
            {"name": "failed_hexrays", "rva": "0x20", "address": "0x140000020",
             "pseudocode": None,
             "assembly": [{"address": "0x140000020", "bytes": "c3", "text": "retn"}]},
        ]
        self.assertEqual(capture_io.write_function_aggregates(self.root, iter(rows)), 2)
        c = (self.root / "all-functions.c").read_text(encoding="utf-8")
        asm = (self.root / "all-functions.asm").read_text(encoding="utf-8")
        self.assertIn("配置 RVA 0x10 VA 0x140000010", c)
        self.assertIn("int setting() { return 7; }", c)
        self.assertIn("Hex-Rays did not decompile this function", c)
        self.assertIn("b807000000  mov eax, 7", asm)
        self.assertIn("0x140000020  c3  retn", asm)

    def test_interrupted_aggregate_keeps_both_previous_complete_files(self):
        sys.path.insert(0, str(Path(__file__).resolve().parent / "ida"))
        import capture_io
        paths = [self.source("all-functions.c", b"complete C"),
                 self.source("all-functions.asm", b"complete ASM")]

        def rows():
            yield {"name": "first", "rva": "0x10", "address": "0x140000010",
                   "pseudocode": None, "assembly": []}
            raise RuntimeError("synthetic aggregate interruption")

        with self.assertRaisesRegex(RuntimeError, "aggregate interruption"):
            capture_io.write_function_aggregates(self.root, rows())
        self.assertEqual([path.read_bytes() for path in paths], [b"complete C", b"complete ASM"])
        self.assertFalse(any(self.root.glob("*.tmp")))

    def test_native_corpus_change_during_capture_prevents_full_snapshot_claim(self):
        raw = b"synthetic native original"
        digest = hashlib.sha256(raw).hexdigest()
        alias = ".analysis/inputs/test/device.dll"
        self.source(alias, raw)
        corpus_path = self.source(".analysis/results/corpus.json", json.dumps({
            "native_inputs": [{"sha256": digest, "bytes": len(raw), "aliases": [alias]}],
            "mixed_inputs": [],
        }).encode())

        def capture(args):
            corpus_path.write_bytes(corpus_path.read_bytes() + b"\n")
            self.native_capture(digest, len(raw))
            return {"passed": True, "exit_code": 0}

        with patch.object(analysis, "capture_native", side_effect=capture), redirect_stdout(io.StringIO()):
            analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[],
                                                       ida_root="unused", no_refresh_gaps=True))
        queue = json.loads((self.root / ".analysis/results/native-corpus-capture.json").read_bytes())
        self.assertTrue(queue["capture_complete"])
        self.assertFalse(queue["snapshot_unchanged"])
        self.assertFalse(queue["full_corpus_captured"])

    def test_reused_native_capture_requires_actual_original_identity_and_unchanged_snapshot(self):
        for issue in (None, "changed_original", "changed_during_verification", "capture_input_size", "changed_output", "output_escape"):
            with self.subTest(issue=issue):
                raw = b"synthetic inert original"
                digest = hashlib.sha256(raw).hexdigest()
                alias = ".analysis/inputs/test/device.dll"
                original = self.source(alias, raw + (b" changed" if issue == "changed_original" else b""))
                self.source(".analysis/results/corpus.json", json.dumps({
                    "native_inputs": [{"sha256": digest, "bytes": len(raw), "aliases": [alias]}],
                    "mixed_inputs": [],
                }).encode())
                manifest, folder = self.native_capture(digest, len(raw) + (1 if issue == "capture_input_size" else 0))
                output = self.root / folder / "all-functions.c"
                if issue == "output_escape":
                    manifest["outputs"].append({"path": "../../outside.txt", "sha256": "0" * 64})
                self.source(folder + "/capture.json", json.dumps(manifest).encode())
                if issue == "changed_output":
                    output.write_bytes(b"different static output")
                hashing = analysis.sha

                def digest_file(path):
                    value = hashing(path)
                    if issue == "changed_during_verification" and path == output:
                        os.utime(original, ns=(original.stat().st_atime_ns, original.stat().st_mtime_ns + 10_000_000))
                    return value

                with patch.object(analysis, "sha", side_effect=digest_file), \
                     patch.object(analysis, "capture_native") as capture, redirect_stdout(io.StringIO()):
                    analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[],
                                                               ida_root="unused", no_refresh_gaps=True))
                capture.assert_not_called()
                queue = json.loads((self.root / ".analysis/results/native-corpus-capture.json").read_bytes())
                self.assertEqual(queue["capture_complete"], issue is None)
                self.assertEqual(queue["full_corpus_captured"], issue is None)
                self.assertEqual(queue["modules"][0]["status"], "reused_complete_capture" if issue is None else "failed")

    def test_original_changed_after_its_module_finishes_prevents_complete_batch_claim(self):
        rows = []
        for name, raw in (("first.dll", b"inert first"), ("second.dll", b"inert second original")):
            path = self.source(".analysis/inputs/test/" + name, raw)
            rows.append({"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw),
                         "aliases": [path.relative_to(self.root).as_posix()]})
        self.source(".analysis/results/corpus.json", json.dumps({"native_inputs": rows}).encode())

        def capture(args):
            selected = next(row for row in rows if row["aliases"][0] == args.input)
            self.native_capture(selected["sha256"], selected["bytes"])
            if args.input.endswith("second.dll"):
                (self.root / rows[0]["aliases"][0]).write_bytes(b"changed after first module finished")
            return {"passed": True, "exit_code": 0}

        with patch.object(analysis, "capture_native", side_effect=capture), redirect_stdout(io.StringIO()):
            analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[],
                                                       ida_root="unused", no_refresh_gaps=True))
        queue = json.loads((self.root / ".analysis/results/native-corpus-capture.json").read_bytes())
        self.assertFalse(queue["original_inputs_snapshot_unchanged"])
        self.assertFalse(queue["capture_complete"])
        self.assertFalse(queue["full_corpus_captured"])
        self.assertEqual(queue["original_inputs_changed_during_capture"], [rows[0]["aliases"][0]])

    def test_complete_native_capture_requires_full_unique_inventories_and_required_outputs(self):
        for issue in (None, "missing_functions", "duplicate_function", "wrong_rva", "missing_assembly",
                      "missing_dataset", "dataset_count", "missing_aggregate", "aggregate_count", "duplicate_output"):
            with self.subTest(issue=issue):
                raw = b"synthetic inert native original"
                digest = hashlib.sha256(raw).hexdigest()
                resource = {"sha256": digest, "bytes": len(raw)}
                manifest, folder = self.native_capture(digest, len(raw))
                if issue == "missing_functions":
                    manifest["functions"] = []
                elif issue == "duplicate_function":
                    manifest["functions"].append(dict(manifest["functions"][0]))
                    for field in ("functions_captured", "functions_total", "settled_functions_total"):
                        manifest[field] = 2
                elif issue == "wrong_rva":
                    manifest["functions"][0]["rva"] = "0x2000"
                elif issue == "missing_assembly":
                    manifest["functions"][0]["assembly_captured"] = False
                elif issue in {"missing_dataset", "missing_aggregate"}:
                    name = "strings.json" if issue == "missing_dataset" else "all-functions.asm"
                    manifest["outputs"] = [row for row in manifest["outputs"] if row["path"] != name]
                elif issue == "dataset_count":
                    manifest["strings_count"] = None
                elif issue == "aggregate_count":
                    manifest["aggregate_functions_written"] = 0
                elif issue == "duplicate_output":
                    manifest["outputs"].append(dict(manifest["outputs"][0]))
                path = self.source(folder + "/capture.json", json.dumps(manifest).encode())
                if issue is None:
                    _, identity = analysis.verify_native_capture(self.root / folder, resource)
                    self.assertEqual(identity["sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
                else:
                    with self.assertRaises(ValueError):
                        analysis.verify_native_capture(self.root / folder, resource)

    def test_new_native_capture_success_still_requires_present_valid_outputs(self):
        for issue in ("absent_manifest", "missing_function_output"):
            with self.subTest(issue=issue):
                raw = b"synthetic inert original"
                digest = hashlib.sha256(raw).hexdigest()
                alias = ".analysis/inputs/test/device.dll"
                self.source(alias, raw)
                self.source(".analysis/results/corpus.json", json.dumps({"native_inputs": [
                    {"sha256": digest, "bytes": len(raw), "aliases": [alias]}]}).encode())
                folder = ".analysis/results/native/" + digest
                previous = self.root / folder / "capture.json"
                if previous.exists():
                    previous.unlink()

                def capture(args):
                    if issue == "missing_function_output":
                        _, capture_folder = self.native_capture(digest, len(raw))
                        (self.root / capture_folder / "functions/1000.json").unlink()
                    return {"passed": True, "exit_code": 0}

                with patch.object(analysis, "capture_native", side_effect=capture), redirect_stdout(io.StringIO()):
                    analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[],
                                                               ida_root="unused", no_refresh_gaps=True))
                queue = json.loads((self.root / ".analysis/results/native-corpus-capture.json").read_bytes())
                self.assertFalse(queue["capture_complete"])
                self.assertFalse(queue["full_corpus_captured"])
                self.assertEqual(queue["modules"][0]["status"], "failed")

    def test_native_resume_uses_only_nonempty_packed_or_recoverable_database(self):
        private = self.source(".work/native/test/batch/original.exe", b"synthetic original bytes")
        self.assertIsNone(analysis.native_database_resume(private))
        self.source(".work/native/test/batch/original.exe.id0", b"first database component")
        self.source(".work/native/test/batch/original.exe.id1", b"")
        self.assertIsNone(analysis.native_database_resume(private))
        self.source(".work/native/test/batch/original.exe.id1", b"second database component")
        unpacked = analysis.native_database_resume(private)
        self.assertEqual(unpacked["kind"], "recover_unpacked_database")
        self.assertEqual(unpacked["input"], analysis.rel(private))
        self.assertEqual(len(unpacked["files_before_resume"]), 2)
        packed = self.source(".work/native/test/batch/original.exe.i64", b"packed private database")
        selected = analysis.native_database_resume(private)
        self.assertEqual(selected["kind"], "packed_database")
        self.assertEqual(selected["input"], analysis.rel(packed))
        self.assertEqual(selected["files_before_resume"][0]["sha256_at_resume"], hashlib.sha256(packed.read_bytes()).hexdigest())

    def test_native_resume_preserves_failed_session_and_raw_logs_without_recreating_database(self):
        name = ".analysis/inputs/test/original.dll"
        source = self.source(name, b"synthetic original bytes; never loaded")
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        row = {"path": name, "bytes": source.stat().st_size, "sha256": digest}
        self.source(".analysis/inputs.json", json.dumps({"inputs": [row]}).encode())
        output = ".analysis/results/native/" + digest
        private = self.source(".work/native/" + digest + "/batch/original.dll", source.read_bytes())
        database = self.source(analysis.rel(private) + ".i64", b"packed private database")
        pe = {"kind": "native", "architecture": "x64", "image_base": "0x140000000"}
        loader = {"pe_kind": "native", "architecture": "x64", "image_base": pe["image_base"], "selection": "automatic"}
        self.source(output + "/plan.json", json.dumps({"sha256": digest, "loader": loader}).encode())
        self.source(output + "/batch-session.json", json.dumps({"pid": 11, "passed": False, "exit_code": 1,
                                                               "stop_reason": "synthetic memory limit"}).encode())
        raw_log = b"synthetic static failure log\x00\xff"
        self.source(analysis.rel(private.parent / "ida.log"), raw_log)
        self.source(analysis.rel(private.parent / "process.log"), b"old process log")
        executable = self.source("tools/static/idat.exe", b"synthetic static analyzer; never executed")
        process = MagicMock(pid=123)

        def finish():
            self.source(output + "/capture.json", b'{"capture_complete":true}')
            return 0

        process.wait.side_effect = finish
        with patch.object(host, "inspect_pe", return_value=pe), patch.object(analysis, "configure_ida"), \
             patch.object(analysis.subprocess, "Popen", return_value=process) as start, redirect_stdout(io.StringIO()):
            result = analysis.capture_native(argparse.Namespace(input=name, resume=True, mcp_only=False,
                ida_root=str(executable.parent), no_refresh_gaps=True))
        command = start.call_args.args[0]
        self.assertNotIn("-c", command)
        self.assertEqual(command[-1], str(database))
        self.assertIn("-Opdb:off", command)
        self.assertEqual(private.read_bytes(), source.read_bytes())
        self.assertTrue(result["passed"])
        prior = result["previous_attempts"][0]
        self.assertFalse(prior["passed"])
        self.assertEqual(prior["stop_reason"], "synthetic memory limit")
        self.assertEqual(prior["retained_logs"][0]["sha256"], hashlib.sha256(raw_log).hexdigest())
        import base64
        self.assertEqual(base64.b64decode(prior["retained_logs"][0]["raw_base64"]), raw_log)

    def test_failed_database_integrity_retains_bytes_then_forces_a_clean_private_analysis(self):
        source = self.source(".analysis/inputs/test/original.dll", b"inert original; never loaded")
        name = analysis.rel(source)
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        self.source(".analysis/inputs.json", json.dumps({"inputs": [
            {"path": name, "bytes": source.stat().st_size, "sha256": digest}]}).encode())
        output = ".analysis/results/native/" + digest
        private = self.source(".work/native/" + digest + "/batch/original.dll", source.read_bytes())
        bad_database = self.source(analysis.rel(private) + ".i64", b"inert invalid database bytes")
        self.source(analysis.rel(private) + ".id0", b"inert unpacked component")
        failed_capture = json.dumps({"input_sha256": digest, "capture_complete": False,
                                    "database_integrity": {"verified": False, "records_with_issues": 2}}).encode()
        self.source(output + "/capture.json", failed_capture)
        pe = {"kind": "native", "architecture": "x64", "image_base": "0x140000000"}
        loader = {"pe_kind": "native", "architecture": "x64", "image_base": pe["image_base"], "selection": "automatic"}
        self.source(output + "/plan.json", json.dumps({"sha256": digest, "loader": loader}).encode())
        self.source(output + "/batch-session.json", json.dumps({"pid": 11, "passed": False, "exit_code": 1}).encode())
        executable = self.source("tools/static/idat.exe", b"synthetic static analyzer; never executed")
        process = MagicMock(pid=123)

        def finish():
            self.source(output + "/capture.json", b'{"capture_complete":true}')
            return 0

        process.wait.side_effect = finish
        with patch.object(host, "inspect_pe", return_value=pe), patch.object(analysis, "configure_ida"), \
             patch.object(analysis.subprocess, "Popen", return_value=process) as start, redirect_stdout(io.StringIO()):
            result = analysis.capture_native(argparse.Namespace(input=name, resume=True, mcp_only=False,
                ida_root=str(executable.parent), no_refresh_gaps=True))
        command = start.call_args.args[0]
        self.assertIn("-c", command)
        self.assertEqual(command[-1], str(private))
        self.assertFalse(bad_database.exists())
        self.assertEqual(private.read_bytes(), source.read_bytes())
        prior = result["previous_attempts"][0]
        rebuild = prior["failed_database_after_attempt"]
        self.assertEqual(len(rebuild["retained_invalid_database"]), 2)
        for record in rebuild["retained_invalid_database"]:
            retained = self.root / record["path"]
            self.assertTrue(retained.is_relative_to(private.parent))
            self.assertEqual(hashlib.sha256(retained.read_bytes()).hexdigest(), record["sha256"])
        import base64
        self.assertEqual(base64.b64decode(prior["failed_capture_bytes"]["raw_base64"]), failed_capture)
        plan = json.loads((self.root / output / "plan.json").read_bytes())
        self.assertIsNone(plan["database_resume"])
        self.assertIsNotNone(plan["database_rebuild"])

    def test_rejected_database_retention_refuses_non_scratch_input_without_moving_it(self):
        original = self.source(".analysis/inputs/test/original.dll", b"inert original")
        sibling = self.source(analysis.rel(original) + ".i64", b"must stay in place")
        with self.assertRaisesRegex(ValueError, "outside private"):
            analysis.reject_native_database(original)
        self.assertEqual(sibling.read_bytes(), b"must stay in place")


class HostArchiveTests(unittest.TestCase):
    def test_managed_capture_includes_device_originals_without_promoting_manifest_mismatches(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            name = ".analysis/inputs/official-native-current/quarantine/device.exe"
            path = root / name
            path.parent.mkdir(parents=True)
            raw = b"inert synthetic managed device original; never executed"
            path.write_bytes(raw)
            row = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                   "verified_against_manifest": False, "declared_sha256": "0" * 64}
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [row]}), encoding="utf-8")
            tool = root / "tools/analyzers/ilspy"
            tool.mkdir(parents=True)
            (tool / "acquisition.json").write_bytes(b"{}")
            pe = {"kind": "managed", "clr": {}, "version_resources": []}
            with patch.object(analysis, "ROOT", root), patch.object(host, "prepare_managed_analyzer", return_value=(tool, {}, [])), \
                 patch.object(host, "inspect_pe", return_value=pe) as inspect, \
                 patch.object(host.subprocess, "run", return_value=argparse.Namespace(returncode=0)) as run, \
                 redirect_stdout(io.StringIO()):
                host.analyze_host(argparse.Namespace(managed_scope="all"))
            inspect.assert_called_once_with(raw)
            self.assertEqual(run.call_count, 2)
            self.assertTrue(all(call.args[0][0] == "dotnet" and "--disable-updatecheck" in call.args[0]
                                for call in run.call_args_list))
            result = json.loads((root / ".analysis/results/host-current/managed/capture.json").read_bytes())
            self.assertEqual(len(result["assemblies"]), 1)
            self.assertFalse(result["assemblies"][0]["acquisition_bindings"][0]["verified_against_manifest"])
            self.assertTrue(result["assemblies"][0]["capture_complete"])
            self.assertEqual(path.read_bytes(), raw)

    def test_inno_short_listing_preserves_all_payload_and_embedded_names(self):
        raw = (b"\xef\xbb\xbfInno Setup archive: setup.exe\r\nInno Setup version detected: 6.5.2 (Unicode)\r\n\r\n"
               b"  {app}\\GameManagerService3.dll\r\n  {app}\\MasterGameList,1.xml\r\n"
               b"  embedded\\CompiledCode.bin\r\n  install_script.iss\r\n")
        self.assertEqual(host.inno_archive_entries(raw), ["{app}\\GameManagerService3.dll", "{app}\\MasterGameList,1.xml",
                                                        "embedded\\CompiledCode.bin", "install_script.iss"])

    def test_inno_listing_rejects_unsafe_entries_and_unrecognized_or_empty_output(self):
        for name in ("../outside.dll", "{app}\\..\\outside.dll", "C:\\outside.dll", "\\\\host\\outside.dll", "/outside.dll"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                host.inno_archive_entries(("  " + name + "\r\n").encode())
        for raw in (b"", b"Inno Setup archive: setup.exe\r\n", b"Warning: truncated header\r\n  file.dll\r\n"):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                host.inno_archive_entries(raw)

    def test_retained_archive_revalidation_runs_only_the_static_analyzer_and_keeps_real_results(self):
        for inno, exit_code, change in ((True, 0, False), (False, 7, False), (True, 0, True)):
            with self.subTest(inno=inno, exit_code=exit_code, changed=change), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                source = root / ".analysis/inputs/host-current/package.exe"
                tool = root / "tools/analyzers/static-unpacker.exe"
                output = root / ".analysis/results/host-current/package"
                for path in (source, tool):
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(b"Synthetic fixture; never executed")
                output.mkdir(parents=True)

                def run(argv, **kwargs):
                    if change:
                        source.write_bytes(b"changed synthetic package")
                    return argparse.Namespace(returncode=exit_code, stdout=b"static analyzer output", stderr=b"static diagnostic")

                with patch.object(analysis, "ROOT", root), patch.object(host.subprocess, "run", side_effect=run) as process:
                    result = host.retained_host_archive_integrity(tool, source, output, inno)
                argv = process.call_args.args[0]
                self.assertEqual(argv[0], str(tool))
                self.assertEqual(argv[-1], str(source))
                self.assertEqual(argv[1], "-t" if inno else "t")
                self.assertNotIn("-x", argv)
                self.assertNotIn("x", argv)
                self.assertEqual(result["exit_code"], exit_code)
                self.assertEqual(result["input_bytes_unchanged"], not change)
                self.assertFalse(result["target_execution"])
                for artifact in result["outputs"]:
                    raw = (root / artifact["path"]).read_bytes()
                    self.assertEqual(artifact["bytes"], len(raw))
                    self.assertEqual(artifact["sha256"], hashlib.sha256(raw).hexdigest())


class HostBootstrapTests(unittest.TestCase):
    setUp = EvidenceTests.setUp
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source

    def prepare(self):
        self.source("tools/host.py", b"synthetic analyzer identity")
        self.source(".analysis/inputs.json", b'{"inputs":[]}')
        results = self.root / ".analysis/results/host-current"
        results.mkdir(parents=True, exist_ok=True)
        return results

    def test_recheck_uses_no_cache_official_https_and_preserves_prior_observations(self):
        self.prepare()
        page = ('<a href="' + host.STABLE_BOOTSTRAP + '">download</a>').encode()
        package = b"MZ synthetic archive evidence; never executed"
        requests = []

        def response(request, timeout):
            requests.append(request)
            raw = page if request.full_url == host.STABLE_DOWNLOAD_PAGE else package
            value = io.BytesIO(raw)
            value.geturl = lambda: (request.full_url if raw == page else "https://dl.razerzone.com/current.exe")
            value.status = 200
            value.headers = {"x-amz-meta-checksum-sha256": hashlib.sha256(raw).hexdigest()}
            return value

        opener = MagicMock()
        opener.open.side_effect = response
        args = argparse.Namespace(revalidate_bootstrap=True)
        with patch.object(host.urllib.request, "build_opener", return_value=opener), redirect_stdout(io.StringIO()):
            for _ in range(3):
                host.acquire_host(args)
        self.assertEqual(len(requests), 12)
        self.assertTrue(all(request.get_header("Cache-control") == "no-cache" and
                            request.get_header("Pragma") == "no-cache" for request in requests))
        report = json.loads((self.root / ".analysis/inputs/host-current/bootstrap-recheck.json").read_bytes())
        self.assertTrue(report["bootstrap_stable"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(len(report["previous_observations"]), 2)
        self.assertTrue(all("previous_observations" not in item and "previous_receipt" not in item
                            for item in report["previous_observations"]))
        for row in report["files"]:
            actual = (self.root / row["path"]).read_bytes()
            self.assertEqual(hashlib.sha256(actual).hexdigest(), row["sha256"])

    def test_bad_redirect_changed_page_or_failed_download_remains_a_failed_receipt(self):
        for issue in ("redirect", "changed", "download_failure"):
            with self.subTest(issue=issue):
                self.prepare()
                calls = 0

                def response(request, timeout):
                    nonlocal calls
                    calls += 1
                    if issue == "download_failure" and request.full_url == host.STABLE_BOOTSTRAP:
                        raise OSError("synthetic acquisition failure")
                    raw = (host.STABLE_BOOTSTRAP + (" changed" if issue == "changed" and calls == 3 else "")).encode()
                    if request.full_url == host.STABLE_BOOTSTRAP:
                        raw = b"MZ inert archive"
                    value = io.BytesIO(raw)
                    value.geturl = lambda: "https://unrelated.test/archive.exe" if issue == "redirect" else request.full_url
                    value.status, value.headers = 200, {}
                    return value

                opener = MagicMock()
                opener.open.side_effect = response
                with patch.object(host.urllib.request, "build_opener", return_value=opener), redirect_stdout(io.StringIO()), \
                     self.assertRaises(SystemExit):
                    host.acquire_host(argparse.Namespace(revalidate_bootstrap=True))
                report = json.loads((self.root / ".analysis/inputs/host-current/bootstrap-recheck.json").read_bytes())
                self.assertFalse(report["bootstrap_stable"])
                self.assertFalse(report["target_execution"])
                self.assertFalse(all(row["unchanged"] for row in report["stability"]))


class CachedDownloadFragmentTests(unittest.TestCase):
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source
    policy = b'''const Unit=1024*1024,Chunk=Math.floor(.1*Unit),Download=async(n,r,e)=>{const s=await caches.open("downloaded"),a=await caches.open("razer-installer-files");const one=(index=0,start=0,end,b=!1)=>{let offset=start;const url=new URL(n);url.searchParams.set("pindex",index.toString());const range=`bytes=${offset}-${end||""}`;r.Range=range;return url};const chunks=()=>{const parts=[];for(let offset=0;offset<e;offset+=Chunk){const end=Math.min(offset+Chunk-1,e-1);parts.push(i=>one(i,offset,end))}return parts};return chunks()};'''

    def setUp(self):
        # Analyzer packages belong to the real maintained tool tree, while the
        # acquired fixture originals and evidence index use the temporary root.
        sys.path.insert(0, str(analysis.ROOT / "tools/analyzers/python-static"))
        EvidenceTests.setUp(self)

    def fixture(self):
        rows = []

        def artifact(name, raw, **fields):
            self.source(name, raw)
            row = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), **fields}
            rows.append(row)
            return row

        entry_url = "https://apps.razer.com/installer/"
        entry = artifact(".analysis/inputs/frontend/installer-current/index.html", b'<script src="main.js"></script>', url=entry_url)
        script = artifact(".analysis/inputs/frontend/installer-current/main.js", self.policy, url=entry_url + "main.js")
        acquired = {"entry_url": entry_url, "source_current_during_acquisition": True, "files": [entry, script],
            "stability": [{"url": entry_url, "unchanged": True, "before_sha256": entry["sha256"], "after_sha256": entry["sha256"]}],
            "edges": [{"from": entry_url, "to": script["url"], "literal": "main.js", "kind": "html:script:src"}]}
        receipt = artifact(".analysis/inputs/frontend/installer-current/acquisition.json", json.dumps(acquired).encode())
        whole_bytes = b"MZ" + bytes((index * 37) % 251 for index in range(104857 * 2))
        url = "https://app-assets.razer.com/current.dll"
        complete = artifact(".analysis/inputs/official-native-current/full.dll", whole_bytes, url=url, status="verified")
        complete.update(declared_sha256=complete["sha256"], declared_bytes=complete["bytes"])
        fragment = artifact(".analysis/inputs/installed-current/cache/partial.dll", whole_bytes[:104857],
            kind="installed_cached_official_resource", url=url + "?pindex=0", canonical_url=url)
        native = {"finished_at_utc": "synthetic", "resources": [complete]}
        native_receipt = artifact(".analysis/inputs/official-native-current/acquisition.json", json.dumps(native).encode())
        return {"inputs": rows}, acquired, receipt, script, fragment, native, native_receipt

    def test_policy_retains_complete_owner_and_literal_numeric_bindings_without_execution(self):
        result = analysis.cached_download_policy(self.policy)
        self.assertEqual(result["chunk_bytes"], 104857)
        self.assertIn('searchParams.set("pindex"', result["complete_owner"]["body"])
        self.assertEqual([row["body"] for row in result["constant_declarations"]],
                         ["Chunk=Math.floor(.1*Unit)", "Unit=1024*1024"])
        with self.assertRaisesRegex(ValueError, "unresolved static binding"):
            analysis.cached_download_policy(self.policy.replace(b"Chunk=Math.floor(.1*Unit)", b"Chunk=Math.floor(.1*unknown)"))
        with self.assertRaisesRegex(ValueError, "not constant"):
            analysis.cached_download_policy(self.policy.replace(b"const Unit", b"let Unit"))
        with self.assertRaisesRegex(ValueError, "does not bind"):
            analysis.cached_download_policy(self.policy.replace(b'"pindex",index.toString()', b'"pindex",start.toString()'))
        with self.assertRaisesRegex(ValueError, "parameters are unresolved"):
            analysis.cached_download_policy(self.policy.replace(b"(index=0,start=0,end,b=!1)=>", b"index=>"))

    def test_gap_owner_preserves_fragment_failures_and_unproven_parent_without_whole_file_claim(self):
        report = {"failures": [{"kind": "cached_download_fragment_policy_unverified", "error": "synthetic"}],
                  "snapshot_unchanged": False, "changed_during_audit": [".analysis/inputs/changed.bin"],
                  "fragments": [{"artifact": {"path": ".analysis/inputs/cached.bin"},
                                 "complete_resource": {"path": ".analysis/inputs/whole.exe", "sha256": "a" * 64}}]}
        rows = analysis.cached_download_fragment_gaps(report, ".analysis/results/corpus.json")
        self.assertEqual([row["kind"] for row in rows], ["cached_download_fragment_policy_unverified",
            "cached_download_fragment_snapshot_changed", "cached_download_fragment_parent_unverified"])
        self.assertTrue(all(row["evidence"] == ".analysis/results/corpus.json" for row in rows))
        report["failures"] = []
        report["snapshot_unchanged"] = True
        report["fragments"][0]["complete_resource_provenance"] = {"status": "proven_current_resource_bytes", "root_ids": ["synthetic"]}
        self.assertEqual(analysis.cached_download_fragment_gaps(report, "synthetic"), [])
        self.assertEqual(analysis.cached_download_fragment_gaps({}, "synthetic"), [])

    def test_fragment_proof_requires_current_policy_indexed_originals_and_exact_full_resource_range(self):
        for issue in (None, "changed_script", "changed_fragment", "wrong_index", "missing_link", "unstable", "failed_resource"):
            with self.subTest(issue=issue):
                index, acquired, receipt, script, fragment, native, native_receipt = self.fixture()

                def update(row, data):
                    raw = json.dumps(data).encode()
                    self.source(row["path"], raw)
                    row.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())

                if issue == "changed_script":
                    self.source(script["path"], self.policy + b" ")
                elif issue == "changed_fragment":
                    self.source(fragment["path"], b"changed chunk")
                elif issue == "wrong_index":
                    fragment["url"] = fragment["url"].replace("pindex=0", "pindex=1")
                elif issue == "missing_link":
                    acquired["edges"] = []
                    update(receipt, acquired)
                elif issue == "unstable":
                    acquired["stability"][0]["after_sha256"] = "0" * 64
                    update(receipt, acquired)
                elif issue == "failed_resource":
                    native["resources"][0]["status"] = "failed"
                    update(native_receipt, native)
                report = analysis.cached_download_fragments(index)
                self.assertEqual(len(report["fragments"]), 1 if issue is None else 0)
                self.assertEqual(bool(report["failures"]), issue is not None)
                if issue is None:
                    self.assertEqual(report["fragments"][0]["offset"], 0)
                    self.assertFalse(report["fragments"][0]["whole_file_identity"])

    def test_corpus_excludes_only_exact_proven_fragments_and_retains_complete_parent_provenance(self):
        for wrong_index in (False, True):
            with self.subTest(wrong_index=wrong_index):
                index, _, _, _, fragment, native, _ = self.fixture()
                if wrong_index:
                    fragment["url"] = fragment["url"].replace("pindex=0", "pindex=1")
                self.source(".analysis/inputs.json", json.dumps(index).encode())
                self.source("tools/host.py", b"inert analyzer identity")
                whole = native["resources"][0]
                lineage = {"roots": [{"id": "synthetic-parent", "artifact": whole}],
                           "snapshot_unchanged": True, "failures": []}
                with patch.object(host, "inspect_pe", return_value={"kind": "native"}), \
                     patch.object(analysis, "corpus_current_lineage", return_value=lineage), \
                     patch.object(analysis, "refresh_gaps") as refresh, redirect_stdout(io.StringIO()):
                    report = analysis.inspect_corpus(argparse.Namespace(no_refresh_gaps=True))
                refresh.assert_not_called()
                self.assertEqual(len(report["native_inputs"]), 2 if wrong_index else 1)
                excluded = [row for row in report["excluded_inputs"] if row["path"] == fragment["path"]]
                self.assertEqual(bool(excluded), not wrong_index)
                proof = report["cached_download_fragments"]
                self.assertEqual(bool(proof["failures"]), wrong_index)
                if not wrong_index:
                    self.assertEqual(proof["fragments"][0]["complete_resource_provenance"], {
                        "status": "proven_current_resource_bytes", "root_ids": ["synthetic-parent"]})


class HostRecheckGapTests(unittest.TestCase):
    setUp = EvidenceTests.setUp
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source

    def fixture(self, scope):
        urls = ((host.STABLE_DOWNLOAD_PAGE, host.STABLE_BOOTSTRAP) if scope == "bootstrap" else
                ("https://apps.razer.com/current/manifest.json", "https://manifest3.razerapi.com/current/products"))
        originals = []
        for number, url in enumerate(urls):
            raw = ("inert original " + str(number)).encode()
            name = f".analysis/inputs/host-current/original-{number}.bin"
            self.source(name, raw)
            originals.append({"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                              "url": url, "final_url": url, "http_status": 200})
        stable, matches = (("bootstrap_stable", "matches_previous_bootstrap") if scope == "bootstrap" else
                           ("deployment_metadata_stable", "matches_previous_metadata"))
        data = {"finished_at_utc": "synthetic", "failures": [], stable: True, matches: True,
                "files": originals + [dict(row) for row in originals],
                "stability": [{"url": row["url"], "unchanged": True, "before_sha256": row["sha256"],
                               "after_sha256": row["sha256"]} for row in originals]}
        return data, originals

    def test_independent_recheck_owners_keep_failures_stability_and_actual_byte_identity(self):
        for scope in ("bootstrap", "metadata"):
            for issue in (None, "http_failure", "unstable", "changed_original", "changed_receipt", "missing_sample", "new_deployment"):
                with self.subTest(scope=scope, issue=issue):
                    data, originals = self.fixture(scope)
                    if issue == "http_failure":
                        data["failures"] = [{"stage": "after", "error_kind": "synthetic"}]
                    elif issue == "unstable":
                        data["stability"][0]["after_sha256"] = "0" * 64
                    elif issue == "missing_sample":
                        data["files"] = data["files"][:2]
                    elif issue == "new_deployment":
                        data["matches_previous_bootstrap" if scope == "bootstrap" else "matches_previous_metadata"] = False
                    name = f".analysis/inputs/host-current/{scope}-recheck.json"
                    raw = json.dumps(data).encode()
                    self.source(name, raw)
                    index = {"inputs": originals + [{"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}]}
                    if issue == "changed_original":
                        self.source(originals[0]["path"], b"changed original")
                    elif issue == "changed_receipt":
                        self.source(name, raw + b" ")
                    rows = analysis.host_recheck_gaps(scope, index)
                    self.assertEqual(bool(rows), issue is not None)
                    self.assertTrue(all(row["evidence"] == name for row in rows))
                    if issue == "new_deployment":
                        self.assertEqual(rows[0]["kind"], f"host_{scope}_changed_since_acquisition")

    def test_missing_recheck_remains_explicit(self):
        self.assertEqual(analysis.host_recheck_gaps("bootstrap", {"inputs": []})[0]["kind"],
                         "host_bootstrap_recheck_pending")


class AcquisitionBatchTests(unittest.TestCase):
    def test_host_native_manifests_require_actual_html_indexed_bytes_and_stability(self):
        for issue in (None, "no_link", "changed_manifest", "unstable", "outside_origin"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                folder = root / ".analysis/inputs/frontend/chroma-current"
                folder.mkdir(parents=True)
                url = "https://apps.razer.com/chroma-app/dashboard/"
                manifest_url = url + "manifest.json"
                href = "https://unrelated.test/manifest.json" if issue == "outside_origin" else "manifest.json"
                html = ("<html>" + ("" if issue == "no_link" else '<link rel="manifest" href="' + href + '">') + "</html>").encode()
                raw = json.dumps({"resourceManifest": {"resources": [{"url": "https://app-assets.razer.com/local.dll"}]}}).encode()
                rows = []
                for name, content, endpoint in (("index.html", html, url), ("manifest.json", raw, manifest_url)):
                    path = folder / name
                    path.write_bytes(content)
                    rows.append({"path": path.relative_to(root).as_posix(), "bytes": len(content),
                                 "sha256": hashlib.sha256(content).hexdigest(), "url": endpoint})
                acquired = {"entry_url": url, "source_current_during_acquisition": True, "files": rows,
                    "stability": [{"url": row["url"], "unchanged": issue != "unstable",
                                   "before_sha256": row["sha256"], "after_sha256": row["sha256"]} for row in rows]}
                receipt = folder / "acquisition.json"
                receipt.write_text(json.dumps(acquired), encoding="utf-8")
                index = {"inputs": rows + [{"path": receipt.relative_to(root).as_posix(), "bytes": receipt.stat().st_size,
                                            "sha256": hashlib.sha256(receipt.read_bytes()).hexdigest()}]}
                if issue == "changed_manifest":
                    (folder / "manifest.json").write_bytes(b"changed manifest")
                with patch.object(analysis, "ROOT", root):
                    sources, failures = analysis.host_app_native_manifest_sources(index)
                self.assertEqual(bool(sources), issue is None)
                self.assertEqual(bool(failures), issue not in (None, "no_link"))

    def test_host_native_acquisition_keeps_device_scope_and_does_not_promote_host_manifest_drift(self):
        device = "https://apps.razer.com/synapse/products/1/mw/manifest.json"
        host_url = "https://apps.razer.com/chroma-app/dashboard/manifest.json"
        device_raw = b'{"version":"1","resourceManifest":{"resources":[]}}'
        host_raw = b'{"version":"2","resourceManifest":{"resources":[]}}'
        for drift in (False, True):
            with self.subTest(drift=drift), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                index = root / ".analysis/inputs.json"
                index.parent.mkdir()
                index.write_bytes(b'{"inputs":[]}')
                receipt = root / ".analysis/inputs/official-native-current/acquisition.json"
                receipt.parent.mkdir(parents=True)
                receipt.write_text(json.dumps({"manifests": [{"url": device}], "resources": []}), encoding="utf-8")
                current = {host_url: [{"manifest": {"path": ".analysis/inputs/frontend/current/manifest.json",
                                                      "sha256": "0" * 64 if drift else hashlib.sha256(host_raw).hexdigest()}}]}
                requested = []

                def fetch(request, **kwargs):
                    requested.append(request.full_url)
                    response = MagicMock()
                    response.__enter__.return_value = response
                    response.read.return_value = device_raw if request.full_url == device else host_raw
                    response.geturl.return_value = request.full_url
                    response.status, response.headers = 200, {}
                    return response

                with patch.object(analysis, "ROOT", root), \
                     patch.object(analysis, "host_app_native_manifest_sources", return_value=(current, [])), \
                     patch.object(analysis.urllib.request, "urlopen", side_effect=fetch), \
                     patch.object(analysis, "inspect_corpus"), redirect_stdout(io.StringIO()):
                    analysis.acquire_native(argparse.Namespace(include_host_app_resources=True, no_refresh_gaps=True))
                report = json.loads(receipt.read_bytes())
                self.assertEqual(set(requested), {device, host_url})
                self.assertTrue(report["includes_host_app_resources"])
                self.assertEqual(report["complete"], not drift)
                item = next(row for row in report["manifests"] if row["url"] == host_url)
                self.assertEqual(item["manifest_scope"], "host_app")
                self.assertEqual("error" in item, drift)
                self.assertTrue((root / item["path"]).is_file())

    def test_driver_inputs_are_classified_from_verified_bytes_without_loading(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            name = ".analysis/inputs/driver/RzSynthetic.sys"
            driver = root / name
            driver.parent.mkdir(parents=True)
            raw = b"MZ synthetic inert driver fixture"
            driver.write_bytes(raw)
            row = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [row]}), encoding="utf-8")
            tool = root / "tools/host.py"
            tool.parent.mkdir()
            tool.write_bytes(b"# Synthetic analyzer identity; never executed.\n")
            with patch.object(analysis, "ROOT", root), \
                 patch.object(host, "inspect_pe", return_value={"kind": "native"}) as inspect, \
                 patch.object(analysis, "refresh_gaps") as refresh, redirect_stdout(io.StringIO()):
                result = analysis.inspect_corpus(argparse.Namespace(no_refresh_gaps=True))
                inspect.assert_called_once_with(raw)
                self.assertEqual(result["native_inputs"][0]["verified_aliases"], [name])
                self.assertFalse(result["failures"])
                driver.write_bytes(b"changed driver fixture")
                changed = analysis.inspect_corpus(argparse.Namespace(no_refresh_gaps=True))
                self.assertEqual(len(changed["failures"]), 1)
                self.assertEqual(inspect.call_count, 1)
                self.assertFalse(changed["native_inputs"])
                refresh.assert_not_called()

    def test_native_acquisition_defers_gap_refresh_only_when_requested(self):
        url = "https://apps.razer.com/synapse/products/1/mw/manifest.json"
        manifest = b'{"version":"1","resourceManifest":{"resources":[]}}'
        parser = argparse.ArgumentParser()
        analysis.add_commands(parser.add_subparsers(dest="command", required=True))
        for deferred in (False, True):
            with self.subTest(deferred=deferred), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                index = root / ".analysis/inputs.json"
                index.parent.mkdir()
                index.write_text('{"inputs":[]}', encoding="utf-8")
                tool = root / "tools/host.py"
                tool.parent.mkdir()
                tool.write_bytes(b"# Synthetic analyzer identity; never executed.\n")
                argv = ["acquire-native", "--manifest-url", url]
                if deferred:
                    argv.append("--no-refresh-gaps")
                response = MagicMock()
                response.__enter__.return_value = response
                response.read.return_value = manifest
                response.geturl.return_value = url
                response.status = 200
                response.headers = {}
                with patch.object(analysis, "ROOT", root), \
                     patch.object(analysis.urllib.request, "urlopen", return_value=response), \
                     patch.object(analysis, "refresh_gaps") as refresh, redirect_stdout(io.StringIO()):
                    analysis.acquire_native(parser.parse_args(argv))
                self.assertEqual(refresh.call_count, 0 if deferred else 1)
                receipt = json.loads((root / ".analysis/inputs/official-native-current/acquisition.json").read_bytes())
                self.assertTrue(receipt["complete"])
                self.assertTrue((root / ".analysis/results/corpus.json").is_file())

    def test_native_rechecks_keep_mismatched_http_observations_and_request_revalidation(self):
        manifest_url = "https://apps.razer.com/synapse/products/1/mw/manifest.json"
        artifact_url = "https://app-assets.razer.com/files/synapse/devices/1/driver.exe"
        expected = b"MZ synthetic expected driver bytes"
        observed = b"MZ synthetic mismatched endpoint bytes"
        digest = hashlib.sha256(expected).hexdigest()
        manifest = json.dumps({"version": "1.2", "resourceManifest": {"resources": [{
            "url": artifact_url, "size": len(expected), "sha256": digest,
            "resourceName": "local-driver", "resourceVersion": "1.2"}]}}).encode()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            index = root / ".analysis/inputs.json"
            index.parent.mkdir()
            index.write_text('{"inputs":[]}', encoding="utf-8")
            served = [observed]
            requests = []

            def fetch(request, **kwargs):
                requests.append(request)
                response = MagicMock()
                response.__enter__.return_value = response
                response.read.return_value = manifest if request.full_url == manifest_url else served[0]
                response.geturl.return_value = request.full_url
                response.status = 200
                response.headers = {"X-Synthetic-Observation": str(len(requests))}
                return response

            args = argparse.Namespace(manifest_url=[manifest_url], no_refresh_gaps=True)
            receipt_path = root / ".analysis/inputs/official-native-current/acquisition.json"
            with patch.object(analysis, "ROOT", root), \
                 patch.object(analysis.urllib.request, "urlopen", side_effect=fetch), \
                 patch.object(analysis, "inspect_corpus"), redirect_stdout(io.StringIO()):
                analysis.acquire_native(args)
                before = receipt_path.read_bytes()
                first = json.loads(before)["resources"][0]
                self.assertEqual(first["status"], "failed")
                self.assertEqual(first["observed_artifact"]["sha256"], hashlib.sha256(observed).hexdigest())
                self.assertEqual((root / first["observed_artifact"]["path"]).read_bytes(), observed)
                self.assertFalse(first["observed_artifact"]["verified_against_manifest"])

                analysis.acquire_native(args)
                second_receipt = json.loads(receipt_path.read_bytes())
                second = second_receipt["resources"][0]
                self.assertEqual(second["status"], "failed")
                self.assertEqual(second["previous_observations"], [first])
                self.assertEqual(second_receipt["previous_receipt"]["sha256"], hashlib.sha256(before).hexdigest())

                served[0] = expected
                analysis.acquire_native(args)
                third = json.loads(receipt_path.read_bytes())["resources"][0]
                self.assertEqual(third["status"], "verified")
                self.assertEqual(third["sha256"], digest)
                self.assertEqual((root / third["path"]).read_bytes(), expected)
                self.assertEqual(len(third["previous_observations"]), 2)
                self.assertTrue(all(row["status"] == "failed" for row in third["previous_observations"]))
                self.assertTrue(all("previous_observations" not in row for row in third["previous_observations"]))
                self.assertEqual(third["previous_observations"][0], first)

                downloaded = sum(request.full_url == artifact_url for request in requests)
                analysis.acquire_native(args)
                reused = json.loads(receipt_path.read_bytes())["resources"][0]
                self.assertEqual(reused["acquisition"], "verified retained bytes")
                self.assertEqual(len(reused["previous_observations"]), 3)
                self.assertEqual(sum(request.full_url == artifact_url for request in requests), downloaded)

            self.assertEqual(downloaded, 3)
            for request in requests:
                headers = {name.lower(): value for name, value in request.header_items()}
                self.assertEqual(headers["cache-control"], "no-cache")
                self.assertEqual(headers["pragma"], "no-cache")
                self.assertEqual(headers["accept-encoding"], "identity")


class CorpusLineageTests(unittest.TestCase):
    setUp = EvidenceTests.setUp
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source

    def artifact(self, name, raw, **fields):
        self.source(name, raw)
        row = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), **fields}
        self.rows.append(row)
        return row

    def receipt(self, name, value, indexed=False):
        raw = json.dumps(value).encode()
        if indexed:
            return self.artifact(name, raw, kind="acquisition_receipt")
        self.source(name, raw)

    @staticmethod
    def group(*aliases):
        first = aliases[0]
        return {"sha256": first["sha256"], "bytes": first["bytes"],
                "verified_aliases": [row["path"] for row in aliases]}

    def native_fixture(self):
        self.rows = []
        parent_raw, child_raw, leaf_raw = b"prefixMZ embedded cabinet suffix", b"MZ embedded cabinet", b"MZ extracted native leaf"
        parent = self.artifact(".analysis/inputs/official-native-current/blobs/parent.exe", parent_raw,
                               kind="official_device_resource")
        child = self.artifact(".analysis/inputs/official-native-current/embedded/child.cab", child_raw,
                              kind="static_embedded_native_input", container=parent["path"],
                              container_sha256=parent["sha256"], container_offset=6, verified_against_manifest=True)
        leaf = self.artifact(".analysis/inputs/official-native-current/extracted/leaf.dll", leaf_raw,
                             kind="static_extracted_native_input", container=child["path"],
                             container_sha256=child["sha256"], container_extraction_complete=True)
        alias = self.artifact(".analysis/inputs/installed-current/files/leaf.dll", leaf_raw,
                              kind="installed_static_evidence")
        endpoint = "https://apps.razer.com/synapse/products/1/mw/manifest.json"
        artifact_url = "https://app-assets.razer.com/files/synapse/devices/1/parent.exe"
        manifest = self.artifact(".analysis/inputs/official-native-current/manifests/manifest.json",
            json.dumps({"version": "2", "resourceManifest": {"resources": [{"url": artifact_url,
                "sha256": parent["sha256"], "size": parent["bytes"], "resourceName": "driver",
                "resourceVersion": "3"}]}}).encode(), kind="official_device_manifest", url=endpoint)
        native = {"finished_at_utc": "synthetic", "manifests": [manifest], "resources": [{**parent,
            "url": artifact_url, "declared_sha256": parent["sha256"], "declared_bytes": parent["bytes"],
            "status": "verified"}], "stability": [{"url": endpoint, "unchanged": True,
                "before_sha256": manifest["sha256"], "after_sha256": manifest["sha256"]}]}
        inspection = {"container": child, "files": [leaf], "extracted": True, "listing_exit_code": 0,
                      "extraction_exit_code": 0, "input_bytes_unchanged": True}
        self.receipt(".analysis/results/native-packages/" + child["sha256"] + "/inspection.json", inspection)
        self.receipt(".analysis/inputs/official-native-current/acquisition.json", native, indexed=True)
        return parent, child, leaf, alias, native, inspection

    def test_actual_manifest_container_spans_complete_extraction_and_equal_byte_aliases_join(self):
        parent, child, leaf, alias, native, inspection = self.native_fixture()
        groups = [self.group(leaf, alias)]
        result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        self.assertEqual(len(result["roots"]), 1)
        self.assertEqual(len(result["container_edges"]), 2)
        self.assertTrue(result["snapshot_unchanged"])
        self.assertTrue(result["retained_pe_provenance_complete"])
        self.assertFalse(result["failures"])
        self.assertEqual(groups[0]["current_version_provenance"]["status"], "proven_current_resource_bytes")
        self.assertEqual(groups[0]["current_version_provenance"]["root_ids"], [result["roots"][0]["id"]])
        self.assertEqual(result["roots"][0]["authorities"][0]["version"], "2")
        self.assertEqual(result["roots"][0]["authorities"][0]["resource_version"], "3")

    def test_host_native_lineage_uses_actual_html_declaration_instead_of_device_url_shape(self):
        for issue in (None, "no_html_link", "manifest_drift", "device_scope"):
            with self.subTest(issue=issue):
                parent, unused_child, unused_leaf, unused_alias, native, unused_inspection = self.native_fixture()
                entry_url = "https://apps.razer.com/chroma-app/dashboard/"
                endpoint = entry_url + "manifest.json"
                manifest = native["manifests"][0]
                manifest.update(url=endpoint, manifest_scope="device" if issue == "device_scope" else "host_app")
                native["stability"][0]["url"] = endpoint
                raw = (self.root / manifest["path"]).read_bytes()
                if issue == "manifest_drift":
                    raw += b" "
                html = b"<html></html>" if issue == "no_html_link" else b'<link rel="manifest" href="manifest.json">'
                entry = self.artifact(".analysis/inputs/frontend/chroma-current/index.html", html, url=entry_url)
                declaration = self.artifact(".analysis/inputs/frontend/chroma-current/manifest.json", raw, url=endpoint)
                frontend = {"entry_url": entry_url, "source_current_during_acquisition": True,
                    "files": [entry, declaration], "stability": [{"url": row["url"], "unchanged": True,
                        "before_sha256": row["sha256"], "after_sha256": row["sha256"]} for row in (entry, declaration)]}
                self.receipt(".analysis/inputs/frontend/chroma-current/acquisition.json", frontend, indexed=True)
                self.receipt(".analysis/inputs/official-native-current/acquisition.json", native, indexed=True)
                groups = [self.group(parent)]
                result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertEqual(result["retained_pe_provenance_complete"], issue is None)
                self.assertEqual(bool(result["failures"]), issue is not None)
                if issue is None:
                    self.assertEqual(result["roots"][0]["source_kind"], "official_host_app_resource")
                    self.assertTrue(any(row["path"].endswith("chroma-current/acquisition.json") for row in result["receipts"]))
                    self.assertTrue(result["snapshot_unchanged"])

    def test_mismatch_unstable_manifest_bad_span_and_partial_output_never_become_current(self):
        for issue in ("manifest_mismatch", "unstable", "bad_span", "partial", "failed_extraction"):
            with self.subTest(issue=issue):
                parent, child, leaf, alias, native, inspection = self.native_fixture()
                if issue == "manifest_mismatch":
                    native["resources"][0]["status"] = "failed"
                    native["resources"][0]["error"] = "Resource bytes do not match production manifest"
                    self.receipt(".analysis/inputs/official-native-current/acquisition.json", native, indexed=True)
                elif issue == "unstable":
                    native["stability"][0]["after_sha256"] = "0" * 64
                    self.receipt(".analysis/inputs/official-native-current/acquisition.json", native, indexed=True)
                elif issue == "bad_span":
                    child["container_offset"] += 1
                elif issue == "partial":
                    leaf["container_extraction_complete"] = False
                    leaf["source_bytes_verified"] = False
                else:
                    inspection["extraction_exit_code"] = 2
                    self.receipt(".analysis/results/native-packages/" + child["sha256"] + "/inspection.json", inspection)
                groups = [self.group(leaf, alias)]
                result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertFalse(result["retained_pe_provenance_complete"])
                self.assertEqual(groups[0]["current_version_provenance"]["root_ids"], [])
                self.assertEqual(len(result["unresolved_pe_inputs"]), 1)
                if issue != "partial":
                    self.assertTrue(result["failures"])

    def host_fixture(self):
        import base64
        self.rows = []
        packed = b"MZ packed native entry"
        raw = b"ASAR synthetic prefix" + packed
        bundle = self.artifact(".analysis/inputs/host-current/blobs/app.asar", raw,
                               kind="official_host_artifact", url="https://app-assets.razer.com/app.asar")
        installed = self.artifact(".analysis/inputs/installed-current/files/app.asar", raw,
                                  kind="installed_static_evidence")
        packed_row = self.artifact(".analysis/inputs/host-current/asar/packed.dll", packed,
            kind="static_extracted_host_asar_input", container=installed["path"], container_sha256=installed["sha256"],
            unpacked=False, offset=len(b"ASAR synthetic prefix"))
        unpacked_row = self.artifact(".analysis/inputs/host-current/asar/unpacked.dll", b"MZ separate sibling",
            kind="static_extracted_host_asar_input", container=installed["path"], container_sha256=installed["sha256"],
            unpacked=True, integrity_verified=True)
        manifest_url = "https://apps.razer.com/host/releases/current.json"
        manifest = self.artifact(".analysis/inputs/host-current/blobs/resources.json", json.dumps({"resources": [{
            "url": bundle["url"], "size": bundle["bytes"], "sha256": bundle["sha256"],
            "integrity": "sha256-" + base64.b64encode(bytes.fromhex(bundle["sha256"])).decode()}]}).encode(),
            kind="official_host_resource_manifest", url=manifest_url)
        release = self.artifact(".analysis/inputs/host-current/blobs/installer-manifest.json",
            json.dumps({"baseURL": "host/releases", "latest": {"url": "current.json", "version": "4.9"}}).encode(),
            kind="official_host_release_manifest", url="https://apps.razer.com/host/installer-manifest.json")
        host_data = {"finished_at_utc": "synthetic", "failures": [], "files": [bundle, manifest, release],
                     "stability": [{"url": item["url"], "unchanged": True, "before_sha256": item["sha256"],
                                    "after_sha256": item["sha256"]} for item in (manifest, release)]}
        self.receipt(".analysis/inputs/host-current/acquisition.json", host_data, indexed=True)
        capture = {"input": installed["path"], "input_sha256": installed["sha256"], "input_bytes": installed["bytes"],
                   "complete": True, "unresolved": [], "files": [packed_row, unpacked_row]}
        self.receipt(".analysis/results/host-current/asar/" + installed["sha256"] + "/capture.json", capture)
        return bundle, packed_row, unpacked_row, host_data

    def test_current_host_manifest_reaches_packed_asar_bytes_through_verified_installed_alias(self):
        bundle, packed, unpacked, host_data = self.host_fixture()
        groups = [self.group(packed), self.group(unpacked)]
        result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        self.assertEqual(len(result["roots"]), 1)
        self.assertEqual(groups[0]["current_version_provenance"]["status"], "proven_current_resource_bytes")
        self.assertEqual(groups[1]["current_version_provenance"]["status"], "unresolved")
        self.assertTrue(any("unpacked sibling" in row.get("error", "") for row in result["failures"]))

    def test_unpacked_entry_uses_independently_proven_equal_bytes_without_header_identity_transfer(self):
        bundle, packed, unpacked, host_data = self.host_fixture()
        raw = (self.root / packed["path"]).read_bytes()
        (self.root / unpacked["path"]).write_bytes(raw)
        unpacked.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
        capture_path = ".analysis/results/host-current/asar/" + bundle["sha256"] + "/capture.json"
        capture = json.loads((self.root / capture_path).read_bytes())
        capture["files"] = [unpacked, packed]
        self.receipt(capture_path, capture)
        groups = [self.group(unpacked)]
        result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        self.assertTrue(result["retained_pe_provenance_complete"])
        self.assertFalse(result["failures"])
        alias_edges = [edge for edge in result["container_edges"] if edge["kind"] == "verified_current_equal_byte_alias"]
        self.assertEqual(len(alias_edges), 1)
        self.assertEqual(alias_edges[0]["parent"]["path"], packed["path"])
        self.assertEqual(alias_edges[0]["child"]["path"], unpacked["path"])
        self.assertEqual(alias_edges[0]["declared_asar"]["sha256"], bundle["sha256"])

    def test_failed_new_host_metadata_never_falls_back_to_an_older_stable_receipt(self):
        bundle, packed, unpacked, host_data = self.host_fixture()
        self.receipt(".analysis/inputs/host-current/metadata-recheck.json",
                     {"finished_at_utc": "synthetic", "failures": [{"error": "current manifest unavailable"}]}, indexed=True)
        groups = [self.group(packed)]
        result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        self.assertFalse(result["roots"])
        self.assertEqual(groups[0]["current_version_provenance"]["status"], "unresolved")
        self.assertTrue(any(row["kind"] == "current_host_acquisition_unverified" for row in result["failures"]))

    def test_production_discovery_host_packages_use_the_actual_source_md5_contract(self):
        for valid in (False, True):
            with self.subTest(valid=valid):
                self.rows = []
                raw = b"MZ synthetic host package"
                bundle = self.artifact(".analysis/inputs/host-current/blobs/package.exe", raw,
                    kind="official_stable_host_package", url="https://manifest-assets.razersynapse.com/package.exe")
                declaration = {"download_url": bundle["url"], "file_size": len(raw),
                               "download_file_checksum": hashlib.md5(raw).hexdigest() if valid else "0" * 32}
                release = self.artifact(".analysis/inputs/host-current/blobs/products.json",
                    json.dumps({"data": {"items": [{"name": "Anne", "version": "4.9", "modules": [declaration]}]}}).encode(),
                    kind="official_host_product_release",
                    url="https://manifest3.razerapi.com/api/v1/releases/abc/tags/prod/products?os=WINDOWS")
                discovery = self.artifact(".analysis/inputs/host-current/blobs/endpoints.json",
                    b'{"data":{"items":[{"name":"prod","hash":"abc"}]}}',
                    kind="official_host_discovery", url="https://discovery3.razerapi.com/api/v1/endpoints")
                host_data = {"finished_at_utc": "synthetic", "failures": [], "files": [bundle, release, discovery],
                    "stability": [{"url": row["url"], "unchanged": True, "before_sha256": row["sha256"],
                                   "after_sha256": row["sha256"]} for row in (release, discovery)]}
                self.receipt(".analysis/inputs/host-current/acquisition.json", host_data, indexed=True)
                groups = [self.group(bundle)]
                result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertEqual(bool(result["roots"]), valid)
                self.assertEqual(result["retained_pe_provenance_complete"], valid)
                self.assertEqual(bool(result["failures"]), not valid)

    def test_inputs_changed_during_lineage_collection_invalidate_its_snapshot(self):
        parent, child, leaf, alias, native, inspection = self.native_fixture()
        groups = [self.group(leaf, alias)]
        actual_sha = analysis.sha

        def changed_after_hash(path):
            result = actual_sha(path)
            if path == self.root / child["path"]:
                target = self.root / parent["path"]
                before = target.stat()
                os.utime(target, ns=(before.st_atime_ns, before.st_mtime_ns + 10_000_000))
            return result

        with patch.object(analysis, "sha", side_effect=changed_after_hash):
            result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        self.assertFalse(result["snapshot_unchanged"])
        self.assertIn(parent["path"], result["changed_during_audit"])
        self.assertFalse(result["retained_pe_provenance_complete"])
        self.assertEqual(groups[0]["current_version_provenance"]["status"], "snapshot_changed")

    def test_bootstrap_requires_actual_current_page_and_two_matching_official_samples(self):
        for issue in (None, "missing_sample", "unstable", "failed_latest", "missing_endpoint", "redirect", "not_pe"):
            with self.subTest(issue=issue):
                self.rows = []
                page = self.artifact(".analysis/inputs/host-current/page.html",
                    ("current official page " + ("no entry" if issue == "missing_endpoint" else host.STABLE_BOOTSTRAP)).encode(),
                    url=host.STABLE_DOWNLOAD_PAGE, final_url=host.STABLE_DOWNLOAD_PAGE,
                    kind="official_stable_download_page", http_status=200)
                package = self.artifact(".analysis/inputs/host-current/package.exe",
                    b"HTML error" if issue == "not_pe" else b"MZ inert installer archive",
                    url=host.STABLE_BOOTSTRAP, final_url="https://dl.razerzone.com/current.exe",
                    kind="official_stable_bootstrap_installer", http_status=200)
                files = [dict(item) for item in (page, package, page, package)]
                if issue == "missing_sample":
                    files.pop()
                if issue == "redirect":
                    files[-1]["final_url"] = "https://unrelated.test/current.exe"
                stability = [{"url": item["url"], "before_sha256": item["sha256"],
                              "after_sha256": item["sha256"], "unchanged": True} for item in (page, package)]
                if issue == "unstable":
                    stability[-1]["after_sha256"] = "0" * 64
                receipt = {"finished_at_utc": "synthetic", "bootstrap_stable": True,
                           "failures": [{"error": "failed current check"}] if issue == "failed_latest" else [],
                           "files": files, "stability": stability}
                self.receipt(".analysis/inputs/host-current/bootstrap-recheck.json", receipt, indexed=True)
                groups = [self.group(package)]
                result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertEqual(bool(result["roots"]), issue is None)
                self.assertEqual(result["retained_pe_provenance_complete"], issue is None)
                self.assertEqual(bool(result["failures"]), issue is not None)

    def test_retained_host_archive_proof_requires_test_bytes_and_complete_inno_entries(self):
        for issue in (None, "missing_output", "changed_output", "failed_test", "missing_entry"):
            with self.subTest(issue=issue):
                bundle, packed, unpacked, host_data = self.host_fixture()
                leaf = self.artifact(".analysis/inputs/host-current/extracted/leaf.dll", b"MZ extracted leaf",
                    kind="static_extracted_host_input", container=bundle["path"], container_sha256=bundle["sha256"])
                outputs = []
                for name in ("stdout", "stderr"):
                    path = self.source(".analysis/results/host-current/integrity." + name, b"static archive test")
                    outputs.append({"path": analysis.rel(path), "bytes": path.stat().st_size,
                                    "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
                if issue == "missing_output":
                    outputs.pop()
                elif issue == "changed_output":
                    (self.root / outputs[0]["path"]).write_bytes(b"changed static test")
                integrity = {"exit_code": 7 if issue == "failed_test" else 0, "input_bytes_unchanged": True,
                             "input": bundle, "outputs": outputs}
                inspection = {"containers": [{"input_record": bundle, "sha256": bundle["sha256"],
                    "extracted": True, "list_exit_code": 0, "input_bytes_unchanged": True,
                    "retained_outputs_verified": True, "retained_archive_integrity": integrity, "files": [leaf],
                    "inno_setup": {"list_exit_code": 0, "archive_entry_closure": {"complete": issue != "missing_entry"}}}]}
                self.receipt(".analysis/results/host-current/inspection.json", inspection)
                groups = [self.group(leaf)]
                result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertEqual(bool(groups[0]["current_version_provenance"]["root_ids"]), issue is None)


class NativeSupplementTests(unittest.TestCase):
    def fixture(self):
        raw = bytearray(0x400)
        raw[:2] = b"MZ"
        struct.pack_into("<I", raw, 0x3c, 0x80)
        raw[0x80:0x84] = b"PE\0\0"
        struct.pack_into("<HHIIIHH", raw, 0x84, 0x8664, 2, 0, 0, 0, 0xf0, 0x22)
        struct.pack_into("<H", raw, 0x98, 0x20b)
        struct.pack_into("<Q", raw, 0x98 + 24, 0x140000000)
        struct.pack_into("<I", raw, 0x98 + 60, 0x200)
        for index, (name, rva, at, flags) in enumerate([
                (b".text", 0x1000, 0x200, 0x60000020),
                (b".rdata", 0x2000, 0x300, 0x40000040)]):
            struct.pack_into("<8sIIIIIIHHI", raw, 0x188 + index * 40,
                             name, 0x100, rva, 0x100, at, 0, 0, 0, 0, flags)
        raw[0x200:0x205] = b"\xe9\x00\x00\x00\x00"
        struct.pack_into("<Q", raw, 0x320, 0x140001000)
        return raw

    def test_actual_pointer_slot_bytes_resolve_to_file_backed_executable_code(self):
        raw = self.fixture()
        result = analysis.native_pointer_targets(bytes(raw), 0x2020, 1)
        self.assertEqual(result["pointer_bytes"], 8)
        self.assertEqual(result["raw_hex"], bytes(raw[0x320:0x328]).hex())
        self.assertEqual(result["targets"][0]["rva"], "0x1000")
        self.assertEqual(result["targets"][0]["file_offset"], 0x200)
        self.assertEqual(result["sha256"], hashlib.sha256(raw[0x320:0x328]).hexdigest())

    def test_data_zero_pointer_and_unbacked_ranges_are_not_function_targets(self):
        raw = self.fixture()
        for address, message in [(0x140002000, "not in an executable"), (0, "not uniquely file-backed"),
                                 (0x140001100, "not uniquely file-backed")]:
            struct.pack_into("<Q", raw, 0x320, address)
            with self.assertRaisesRegex(ValueError, message):
                analysis.native_pointer_targets(bytes(raw), 0x2020, 1)
        with self.assertRaisesRegex(ValueError, "not uniquely file-backed"):
            analysis.native_pointer_targets(bytes(raw), 0x20fc, 1)
        for count in (0, -1, 4097):
            with self.assertRaisesRegex(ValueError, "range is invalid"):
                analysis.native_pointer_targets(bytes(raw), 0x2020, count)

    def test_function_reanalysis_keeps_exact_boundaries_and_reviewed_type_evidence(self):
        capture = {"functions": [{"rva": "0x1000", "address": "0x140001000"}]}
        declaration = "void __fastcall recovered(void *a1, void *a2);"
        result = analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
            ["0x1000=" + declaration], "Complete assembly reads RCX and RDX; no stack argument reads")
        self.assertEqual(result["mode"], "retained_function_reanalysis")
        self.assertEqual(result["targets"][0]["file_offset"], 0x200)
        self.assertEqual(result["type_recoveries"][0]["declaration"], declaration)
        self.assertIn("no stack argument", result["type_recoveries"][0]["assembly_evidence"])

    def test_function_reanalysis_rejects_interior_entries_data_and_unreviewed_types(self):
        capture = {"functions": [{"rva": "0x1000", "address": "0x140001000"},
                                 {"rva": "0x2000", "address": "0x140002000"}]}
        for selected, specifications, note, message in [
                ([0x1001], [], None, "exact function boundary"),
                ([0x2000], [], None, "not in executable"),
                ([0x1000, 0x1000], [], None, "unique, bounded"),
                ([0x1000], ["0x1000=void recovered();"], None, "evidence note"),
                ([0x1000], ["0x2000=void recovered();"], "assembly", "explicitly selected"),
                ([0x1000], ["0x1000=void recovered();"] * 2, "assembly", "unique explicitly")]:
            with self.subTest(selected=selected, specifications=specifications):
                with self.assertRaisesRegex(ValueError, message):
                    analysis.native_function_selection(bytes(self.fixture()), capture, selected, specifications, note)

    def reanalysis_fixture(self, root):
        raw = bytes(self.fixture())
        digest = hashlib.sha256(raw).hexdigest()
        original = root / ".analysis/inputs/native/fixture.dll"
        result = root / ".analysis/results/native" / digest
        supplemental = result / "supplements/selected"

        def write(path, data):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(json.dumps(data).encode())
            return {"path": path.relative_to(root).as_posix(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}

        original.parent.mkdir(parents=True)
        original.write_bytes(raw)
        body = {"address": "0x140001000", "rva": "0x1000", "end": "0x140001005",
                "chunks": [{"start": "0x140001000", "end": "0x140001005", "raw_hex": raw[0x200:0x205].hex()}],
                "assembly": [{"address": "0x140001000", "bytes": raw[0x200:0x205].hex()}],
                "xrefs_to_entry": [], "pseudocode": None}
        baseline = write(result / "functions/1000.json", body)
        baseline.update(path="functions/1000.json", rva="0x1000", address="0x140001000")
        capture = {"input": original.relative_to(root).as_posix(), "input_bytes": len(raw), "input_sha256": digest,
                   "functions": [baseline], "failures": [{"stage": "1000:hexrays", "error": "call analysis -12"}]}
        capture_ref = write(result / "capture.json", capture)
        selection = analysis.native_function_selection(raw, capture, [0x1000])
        body["pseudocode"] = {"lines": [{"line": 0, "text": "void recovered() {}"}], "locals": []}
        recovered = write(supplemental / "functions/1000.json", body)
        recovered.update(path="functions/1000.json", rva="0x1000", address="0x140001000",
                         assembly_captured=True, pseudocode_captured=True)
        source_database = {"path": ".work/private.i64", "sha256": "a" * 64}
        tool = {"path": "tools/ida/supplement.py", "sha256": "b" * 64}
        supplement = {"input_sha256": digest, "base_capture": capture_ref, "capture_complete": True,
                      "all_pseudocode_captured": True, "failures": [], "database_integrity": {"verified": True},
                      "base_capture_unchanged": True, "base_database_unchanged": True, "target_execution": False,
                      "capture_tool": tool, "function_selection": selection, "functions": [recovered],
                      "type_recoveries": [], "source_database": source_database,
                      "ida_version": "synthetic", "hexrays_version": "synthetic"}
        write(supplemental / "capture.json", supplement)
        write(supplemental / "session.json", {"passed": True, "exit_code": 0, "capture_tool": tool})
        write(supplemental / "plan.json", {"function_selection": selection, "base_capture": capture_ref,
                                          "source_database": source_database})
        return result, supplemental, capture, supplement, body, write

    def test_reanalysis_resolves_only_exact_verified_hexrays_failure_and_preserves_history(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            result, _, capture, _, _, _ = self.reanalysis_fixture(Path(directory))
            resolved = analysis.native_reanalysis_resolution(result)
            self.assertFalse(resolved["verification_failures"])
            self.assertFalse(resolved["remaining_failures"])
            self.assertEqual(resolved["resolved_failures"][0]["original_failure"], capture["failures"][0])
            self.assertEqual(set(resolved["complete_functions"]), {"0x1000"})
            self.assertEqual(json.loads((result / "capture.json").read_bytes()), capture)

    def test_reanalysis_changed_or_incomplete_evidence_does_not_hide_original_failure(self):
        for issue in ["changed_original", "changed_base", "changed_body", "incomplete_locals", "changed_instruction",
                      "failed_session", "unverified_database", "unselected_boundary", "unapplied_type"]:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    result, supplemental, capture, supplement, body, write = self.reanalysis_fixture(root)
                    if issue == "changed_original":
                        (root / capture["input"]).write_bytes(b"changed original")
                    elif issue == "changed_base":
                        (result / "capture.json").write_bytes((result / "capture.json").read_bytes() + b" ")
                    elif issue == "changed_body":
                        (supplemental / "functions/1000.json").write_bytes(b"changed function")
                    elif issue in {"incomplete_locals", "changed_instruction"}:
                        if issue == "incomplete_locals":
                            del body["pseudocode"]["locals"]
                        else:
                            body["assembly"][0]["bytes"] = "90"
                        ref = write(supplemental / "functions/1000.json", body)
                        supplement["functions"][0]["sha256"] = ref["sha256"]
                        write(supplemental / "capture.json", supplement)
                    elif issue == "failed_session":
                        write(supplemental / "session.json", {"passed": False, "exit_code": 1,
                                                              "capture_tool": supplement["capture_tool"]})
                    elif issue == "unverified_database":
                        supplement["database_integrity"]["verified"] = False
                        write(supplemental / "capture.json", supplement)
                    elif issue == "unselected_boundary":
                        supplement["functions"] = []
                        write(supplemental / "capture.json", supplement)
                    else:
                        supplement["type_recoveries"] = [{"rva": "0x1000", "applied_type": "invented"}]
                        write(supplemental / "capture.json", supplement)
                    resolved = analysis.native_reanalysis_resolution(result)
                    self.assertTrue(resolved["verification_failures"])
                    self.assertEqual(resolved["remaining_failures"], capture["failures"])
                    self.assertFalse(resolved["resolved_failures"])


class FrontendAcquisitionTests(unittest.TestCase):
    def test_chunk_capture_reuses_only_exact_input_analyzer_and_complete_output_bytes(self):
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()):
            root = Path(directory).resolve()
            source, tool, reports = root / "source.js", root / "analyzer.exe", root / "reports"
            source.write_bytes(b"/* never executed */")
            tool.write_bytes(b"synthetic static analyzer")
            result = argparse.Namespace(returncode=0, stdout=b'{"enumeration":null}', stderr=b"diagnostic")
            with patch.object(analysis, "ROOT", root), patch.object(analysis.subprocess, "run", return_value=result) as run:
                invoke = lambda: analysis.enumerate_frontend_chunks(source, tool, reports, "source")
                invoke()
                self.assertEqual(invoke().stdout, result.stdout)
                self.assertEqual(run.call_count, 1)
                (reports / "source.json").write_bytes(b"different retained output")
                invoke()
                self.assertEqual(run.call_count, 2)
                (reports / "source.stderr.txt").unlink()
                invoke()
                self.assertEqual(run.call_count, 3)
                source.write_bytes(b"/* changed source */")
                invoke()
                self.assertEqual(run.call_count, 4)
                tool.write_bytes(b"changed analyzer")
                invoke()
                self.assertEqual(run.call_count, 5)
                (reports / "source.capture.json").write_bytes(b"broken receipt")
                invoke()
                self.assertEqual(run.call_count, 6)

    def test_chunk_capture_preserves_failure_and_rejects_input_mutation_during_analysis(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source, tool, reports = root / "source.js", root / "analyzer.exe", root / "reports"
            source.write_bytes(b"/* inert source */")
            tool.write_bytes(b"synthetic static analyzer")
            result = argparse.Namespace(returncode=9, stdout=b"", stderr=b"unresolved parser failure")
            with patch.object(analysis, "ROOT", root), patch.object(analysis.subprocess, "run", return_value=result) as run:
                analysis.enumerate_frontend_chunks(source, tool, reports, "source")
                cached = analysis.enumerate_frontend_chunks(source, tool, reports, "source")
                self.assertEqual((cached.returncode, cached.stderr), (9, result.stderr))
                self.assertEqual(run.call_count, 1)
                source.write_bytes(b"/* new source */")
                def mutate(*args, **kwargs):
                    source.write_bytes(b"/* concurrent mutation */")
                    return result
                run.side_effect = mutate
                with self.assertRaisesRegex(ValueError, "changed during static enumeration"):
                    analysis.enumerate_frontend_chunks(source, tool, reports, "source")
                receipt = json.loads((reports / "source.capture.json").read_bytes())
                self.assertFalse(receipt["input_bytes_unchanged"])

    def test_app_versions_come_from_actual_manifest_and_missing_maps_do_not_hide_runtime_resources(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            name = ".analysis/inputs/frontend/app-current/manifest.json"
            path = root / name
            path.parent.mkdir(parents=True)
            body = b'{"version":"0.0.86","buildVersion":2610120714,"gitMetadata":{"commit":"actual-app"}}'
            path.write_bytes(body)
            original = {"path": name, "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest(),
                        "url": "https://apps.razer.com/app/manifest.json"}
            acquisition = {"files": [original, {"url": "https://apps.razer.com/app/main.js.map", "http_status": 404}],
                           "source_current_during_acquisition": True, "unresolved": []}
            with patch.object(analysis, "ROOT", root):
                result = analysis.host_app_deployment_audit(acquisition, {"failures": []})
                self.assertEqual(result["declared_versions"][0]["declaration"]["version"], "0.0.86")
                self.assertTrue(result["declared_non_map_resources_available"])
                self.assertFalse(result["resource_dependency_closure_complete"])
                probe = "https://apps.razer.com/app/asset-manifest.json"
                acquisition["files"].append({"url": probe, "http_status": 404})
                acquisition["probes"] = [{"url": probe, "declared": False}]
                result = analysis.host_app_deployment_audit(acquisition, {"failures": []})
                self.assertTrue(result["declared_non_map_resources_available"])
                self.assertEqual(len(result["undeclared_availability_probes"]), 1)
                self.assertFalse(result["unavailable_non_map_resources"])
                acquisition["files"].append({"url": "https://apps.razer.com/app/missing.css", "http_status": 404})
                result = analysis.host_app_deployment_audit(acquisition, {"failures": []})
                self.assertFalse(result["declared_non_map_resources_available"])
                self.assertEqual(len(result["unavailable_source_maps"]), 1)
                self.assertEqual(len(result["unavailable_non_map_resources"]), 1)
                path.write_bytes(b"changed original")
                result = analysis.host_app_deployment_audit(acquisition, {"failures": []})
                self.assertFalse(result["all_retained_bytes_verified"])
                self.assertFalse(result["declared_versions"])

    def test_native_es_module_declarations_keep_their_actual_window_owners(self):
        source = 'const label="设备",windows=[{windowName:"local",url:"/local/",openParam:[label]}];async function release(){window.open("/release-patch-note/","local")} '
        references, unresolved = analysis.host_app_descriptors(source)
        self.assertFalse(unresolved)
        self.assertEqual([row["url_literal"] for row in references], ["/local/", "/release-patch-note/"])
        for row in references:
            owner = row["complete_owner"]
            self.assertEqual(source.encode()[owner["start_byte"]:owner["end_byte"]], owner["body"].encode())
            self.assertTrue(owner["module_id"].startswith("esm-declaration@"))
        self.assertEqual(references[0]["complete_owner"]["body"], 'windows=[{windowName:"local",url:"/local/",openParam:[label]}]')
        self.assertTrue(references[1]["complete_owner"]["body"].startswith("async function release()"))

    def test_application_routes_require_complete_window_owners_and_exact_urls(self):
        text = ('(self.chunks=self.chunks||[]).push([[1],{'
                '10:(e,t,n)=>{const label="设备";const app={windowName:"profiles",url:"/synapse/profiles/",openParam:"policy=3"};'
                'const api={url:"/api/"};const dynamic={windowName:"feedback",url:"/feedback/?path="+window.location.pathname,openParam:"policy=3"}},'
                '20:(e,t,n)=>{window.open("/synapse/profiles/","profiles");const other={windowName:"macro",url:"/synapse/macro/",openParam:"policy=3"}}}]);')
        references, unresolved = analysis.host_app_descriptors(text)
        self.assertEqual([row["url_literal"] for row in references], ["/synapse/profiles/", "/synapse/profiles/", "/synapse/macro/"])
        self.assertEqual([row["complete_owner"]["module_id"] for row in references], [10, 20, 20])
        self.assertEqual([row["kind"] for row in unresolved], ["host_app_dynamic_url_suffix"])
        raw = text.encode()
        for row in references + unresolved:
            owner = row["complete_owner"]
            self.assertEqual(raw[owner["start_byte"]:owner["end_byte"]].decode(), owner["body"])
        expression = unresolved[0]["expression"]
        self.assertEqual(expression["body"], '"/feedback/?path="+window.location.pathname')
        self.assertEqual(raw[expression["start_byte"]:expression["end_byte"]].decode(), expression["body"])
        self.assertNotIn("/api/", [row["url_literal"] for row in references])

    def test_directory_html_includes_and_shared_resource_index_are_acquired_statically(self):
        for entry, content_type, response_url in (("https://apps.razer.com/synapse/test/", "application/octet-stream", None),
                                    ("https://apps.razer.com/synapse/test/entry", "text/html; charset=utf-8", None),
                                    ("https://apps.razer.com/synapse/test", "text/html", "https://apps.razer.com/synapse/test/")):
            with self.subTest(entry=entry), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                (root / ".analysis").mkdir()
                (root / ".analysis/inputs.json").write_text('{"inputs":[]}', encoding="utf-8")
                index_url = "https://apps.razer.com/synapse/assets/index.json"
                shared = "https://apps.razer.com/synapse/assets/shared.js"
                binary = "https://apps.razer.com/synapse/assets/fixture.bin"
                manifest_url = "https://apps.razer.com/synapse/test/asset-manifest.json"
                bodies = {entry: b'<script src="/synapse/assets/shared.js"></script><link rel="preload" href="/synapse/assets/index.json"><script src="./child.js"></script>',
                          manifest_url: b'{"files":{},"entrypoints":[]}', shared: b"/* inert synthetic source; never executed */",
                          index_url: json.dumps([{"url": "/synapse/assets/fixture.bin", "revision": "opaque-source-revision"},
                                                 {"url": "data:ignored", "revision": "rejected-reference"}]).encode(),
                          binary: b"synthetic shared resource bytes",
                          "https://apps.razer.com/synapse/test/child.js": b"/* inert child source */"}
                calls = []

                def download(url, output):
                    calls.append(url)
                    raw = bodies[url]
                    path = analysis.raw_path(output, url)
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(raw)
                    return {"url": url, "path": analysis.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                            "final_url": response_url if url == entry and response_url else url,
                            "content_type": content_type if url == entry else "application/octet-stream",
                            "acquired_at_utc": "synthetic"}

                def urlopen(request, **kwargs):
                    response = MagicMock()
                    response.__enter__.return_value = response
                    response.read.return_value = bodies[request.full_url]
                    return response

                args = argparse.Namespace(url=entry, output=".analysis/inputs/test", no_refresh_gaps=True,
                                          download_artifact=download)
                tool = root / "synthetic-wakaru.exe"
                tool.write_bytes(b"synthetic static analyzer; never executed")
                indexed = []
                args.index_artifacts = indexed.extend
                with patch.object(analysis, "ROOT", root), patch.object(analysis, "wakaru", return_value=tool), \
                     patch.object(analysis.subprocess, "run", return_value=argparse.Namespace(returncode=0, stdout=b'{"enumeration":{}}', stderr=b"")) as run, \
                     patch.object(analysis.urllib.request, "urlopen", side_effect=urlopen), \
                     patch.object(analysis, "refresh_gaps") as refresh, redirect_stdout(io.StringIO()):
                    analysis.fetch_frontend(args)
                self.assertFalse(json.loads((root / ".analysis/inputs.json").read_bytes())["inputs"])
                self.assertEqual(len(indexed), len(bodies) + 1)
                for original in indexed:
                    self.assertEqual(analysis.sha(root / original["path"]), original["sha256"])
                self.assertEqual(set(calls), set(bodies))
                self.assertEqual(run.call_count, 2)
                self.assertEqual(run.call_args.args[0][1:3], ["debug", "enumerate-chunks"])
                result = json.loads((root / ".analysis/inputs/test/acquisition.json").read_bytes())
                self.assertEqual(result["reference_parser_version"], analysis.FRONTEND_REFERENCE_VERSION)
                resource = next(row for row in result["edges"] if row["to"] == binary)
                self.assertEqual(resource["kind"], "shared-resource-index")
                self.assertEqual(resource["declared_revision"], "opaque-source-revision")
                self.assertEqual([row["kind"] for row in result["unresolved"]], ["shared_resource_index_url_rejected"])
                self.assertEqual({row["url"] for row in result["stability"]}, {entry, manifest_url, index_url})
                self.assertTrue(result["source_current_during_acquisition"])
                self.assertEqual(result["entry_response_url"], response_url or entry)
                child = next(row for row in result["edges"] if row["literal"] == "./child.js")
                self.assertEqual(child["from"], entry)
                self.assertEqual(child["to"], "https://apps.razer.com/synapse/test/child.js")
                self.assertEqual((root / next(row["path"] for row in result["files"] if row["url"] == entry)).read_bytes(), bodies[entry])
                refresh.assert_not_called()


class NativePackageTests(unittest.TestCase):
    def test_host_app_command_passes_parser_options_to_the_shared_static_workflow(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            families, prepared, receipt, unused, extractor = self.prepare(root, verified_alias=True)
            acquisition = json.loads(receipt.read_bytes())
            host_manifest = {"url": "https://apps.razer.com/chroma-app/dashboard/manifest.json",
                             "manifest_scope": "host_app"}
            acquisition["manifests"] = [acquisition["resources"][0]["manifests"][0], host_manifest]
            acquisition["resources"][1]["manifests"] = [host_manifest]
            receipt.write_text(json.dumps(acquisition), encoding="utf-8")
            parser = argparse.ArgumentParser()
            analysis.add_commands(parser.add_subparsers(dest="command"))
            args = parser.parse_args(["extract-native-packages", "--scope", "host-apps",
                "--seven-zip", prepared.seven_zip, "--no-refresh-gaps"])
            with patch.object(analysis, "ROOT", root), patch.object(analysis, "refresh_gaps") as refresh, \
                 patch.object(families.subprocess, "run", side_effect=extractor) as run, redirect_stdout(io.StringIO()):
                analysis.extract_native_packages(args)
            report = json.loads((root / ".analysis/results/families/host-apps-native.json").read_bytes())
            self.assertEqual(run.call_count, 2)
            self.assertEqual(report["summary"]["packages"], 1)
            self.assertTrue(report["packages"][0]["extracted"])
            self.assertEqual(report["packages"][0]["current_resource_binding"]["manifests"], [host_manifest])
            refresh.assert_not_called()

    def test_inno_device_payload_uses_static_extractor_and_keeps_reconstructed_script_separate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            families, args, receipt, unused, unused_extractor = self.prepare(root)
            acquisition = json.loads(receipt.read_bytes())
            container = acquisition["resources"][0]["observed_artifact"]
            body = b"inert prefix Inno Setup Setup Data (6.1.0)\0 original bytes"
            (root / container["path"]).write_bytes(body)
            container.update(bytes=len(body), sha256=hashlib.sha256(body).hexdigest())
            receipt.write_text(json.dumps(acquisition), encoding="utf-8")
            inno = root / "tools/static/innounp.exe"
            inno.write_bytes(b"synthetic static analyzer identity")
            (inno.parent / "acquisition.json").write_bytes(b"{}")

            def extractor(command, **kwargs):
                if command[0] == args.seven_zip:
                    self.assertEqual(command[1], "l")
                    return argparse.Namespace(returncode=0, stdout=b"Type = PE\n----------\nPath = .text\n", stderr=b"")
                self.assertEqual(command[0], str(inno))
                if command[1] == "-s":
                    return argparse.Namespace(returncode=0, stdout=b"  {app}\\driver.dll\n  install_script.iss\n", stderr=b"")
                self.assertEqual(command[1], "-x")
                destination = Path(next(value[2:] for value in command if value.startswith("-d")))
                (destination / "{app}").mkdir()
                (destination / "{app}/driver.dll").write_bytes(b"inert original driver")
                (destination / "install_script.iss").write_bytes(b"reconstructed analyzer output")
                return argparse.Namespace(returncode=0, stdout=b"", stderr=b"")

            with patch.object(analysis, "ROOT", root), patch.object(host, "prepare_inno_analyzer", return_value=(inno, {"name": "innounp", "version": "test"})), \
                 patch.object(families.subprocess, "run", side_effect=extractor) as run, redirect_stdout(io.StringIO()):
                families.extract_native_packages(args, [1])
            result = json.loads((root / ".analysis/results/families/synthetic-native.json").read_bytes())
            self.assertEqual(run.call_count, 3)
            record = result["packages"][0]
            self.assertTrue(record["extracted"])
            self.assertTrue(record["inno_setup"]["archive_entry_closure"]["complete"])
            indexed = json.loads((root / ".analysis/inputs.json").read_bytes())["inputs"]
            self.assertEqual(len(indexed), 1)
            self.assertTrue(indexed[0]["path"].endswith("driver.dll"))
            self.assertFalse(record["reconstructed_outputs"][0]["is_original_input"])

    def test_failed_decompression_and_descendants_are_not_promoted_by_byte_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            families, args, receipt, raw, extractor = self.prepare(root)
            index_path = root / ".analysis/inputs.json"
            partial = {"path": ".analysis/inputs/official-native-current/extracted/failed/payload.dll",
                       "sha256": "1" * 64, "bytes": 12, "kind": "static_extracted_native_input",
                       "container": ".analysis/inputs/official-native-current/source.cab", "container_sha256": "2" * 64}
            child = {"path": ".analysis/inputs/official-native-current/embedded/child.dll",
                     "sha256": "3" * 64, "bytes": 4, "kind": "static_embedded_native_input",
                     "container": partial["path"], "container_sha256": partial["sha256"]}
            index_path.write_text(json.dumps({"inputs": [partial, child]}), encoding="utf-8")
            inspection = root / ".analysis/results/native-packages/failed/inspection.json"
            inspection.parent.mkdir(parents=True)
            inspection.write_text(json.dumps({"container": {"path": partial["container"],
                "sha256": "2" * 64, "bytes": 42}, "extracted": False, "extraction_exit_code": 2,
                "files": [partial]}), encoding="utf-8")
            with patch.object(analysis, "ROOT", root), patch.object(analysis, "refresh_gaps") as refresh, \
                 redirect_stdout(io.StringIO()):
                result = analysis.reconcile_native_extractions(argparse.Namespace(no_refresh_gaps=True))
            rows = json.loads(index_path.read_bytes())["inputs"]
            self.assertEqual(set(result["rejected_inputs"]), {partial["path"], child["path"]})
            self.assertTrue(all(row["container_extraction_complete"] is False for row in rows))
            self.assertTrue(all(row["source_bytes_verified"] is False for row in rows))
            self.assertEqual({row["sha256"] for row in rows}, {"1" * 64, "3" * 64})
            refresh.assert_not_called()

    def test_recursive_archive_payloads_keep_root_manifest_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            families, args, receipt, raw, unused = self.prepare(root)

            def extractor(command, **kwargs):
                if command[1] == "l":
                    name = "u4" if Path(command[-1]).suffix == ".exe" else "driver.dll"
                    return argparse.Namespace(returncode=0, stdout=("Type = zip\n----------\nPath = " + name + "\n").encode(), stderr=b"")
                destination = Path(next(value[2:] for value in command if value.startswith("-o")))
                name = "u4" if Path(command[-2]).suffix == ".exe" else "driver.dll"
                (destination / name).write_bytes(b"PK\x03\x04inert nested payload" if name == "u4" else b"inert original driver")
                return argparse.Namespace(returncode=0, stdout=b"", stderr=b"")

            with patch.object(analysis, "ROOT", root), patch.object(families.subprocess, "run", side_effect=extractor) as run, \
                 redirect_stdout(io.StringIO()):
                families.extract_native_packages(args, [1])
            result = json.loads((root / ".analysis/results/families/synthetic-native.json").read_bytes())
            self.assertEqual(run.call_count, 4)
            self.assertEqual(len(result["packages"]), 2)
            self.assertTrue(all(row["extracted"] for row in result["packages"]))
            self.assertTrue(all(row["current_resource_binding"]["verified_against_manifest"] is False for row in result["packages"]))
            self.assertEqual(receipt.read_bytes(), raw)

    def test_embedded_alias_edges_keep_verified_and_mismatched_manifest_roots_separate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            families, args, receipt, raw, extractor = self.prepare(root, verified_alias=True)
            acquisition = json.loads(receipt.read_bytes())
            parents = [acquisition["resources"][0]["observed_artifact"], acquisition["resources"][1]]
            path = root / ".analysis/inputs/official-native-current/embedded/child.cab"
            path.parent.mkdir(parents=True)
            path.write_bytes(b"MSCF inert retained slice")
            child = {"path": path.relative_to(root).as_posix(), "bytes": path.stat().st_size,
                     "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                     "container": ".analysis/inputs/host-current/unbound-parent.dll",
                     "container_sha256": parents[0]["sha256"], "container_offset": 32,
                     "container_aliases": [{**{key: parent[key] for key in ("path", "bytes", "sha256")},
                                            "byte_identity_verified": True} for parent in parents]}
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [child]}), encoding="utf-8")
            with patch.object(analysis, "ROOT", root), patch.object(families.subprocess, "run", side_effect=extractor), \
                 redirect_stdout(io.StringIO()):
                families.extract_native_packages(args, [1])
            result = json.loads((root / ".analysis/results/families/synthetic-native.json").read_bytes())
            record = next(row for row in result["packages"] if row["container"]["sha256"] == child["sha256"])
            self.assertTrue(record["extracted"])
            self.assertTrue(record["current_resource_binding"]["verified_against_manifest"])
            bindings = record["current_resource_bindings"]
            self.assertEqual({item["verified_against_manifest"] for item in bindings}, {True, False})
            self.assertEqual({item["container"] for item in bindings}, {item["path"] for item in parents})
            self.assertEqual({item["root_resource"] for item in bindings}, {item["path"] for item in parents})
            self.assertEqual(receipt.read_bytes(), raw)

    def test_embedded_cabinet_candidates_are_bounded_before_static_decompression(self):
        import struct
        cabinet = struct.pack("<4sIIIII BBHHHHH", b"MSCF", 0, 48, 0, 40, 0, 3, 1, 1, 1, 0, 7, 0) + b"inertpayload"
        self.assertEqual(len(cabinet), 48)
        raw = b"prefix" + cabinet + b"trailerMSCF"
        rows = analysis.embedded_cabinets(raw)
        self.assertEqual(len(rows), 1)
        self.assertEqual((rows[0]["offset"], rows[0]["bytes"]), (6, 48))
        for broken in (cabinet[:-1], cabinet[:4] + b"\x01" + cabinet[5:],
                       cabinet[:24] + b"\x03\x02" + cabinet[26:]):
            self.assertEqual(analysis.embedded_cabinets(broken), [])

    def cabinet_fixture(self, payload, *, compression=1, reserves=False):
        """Inert CAB framing; payload is never decompressed or executed."""
        header_extra = struct.pack("<HBB", 2, 1, 2) + b"hr" if reserves else b""
        folder_extra = b"f" if reserves else b""
        data_extra = b"dr" if reserves else b""
        files_offset = 36 + len(header_extra) + 8 + len(folder_extra)
        file = struct.pack("<IIHHHH", len(payload), 0, 0, 0, 0, 0) + b"payload.bin\0"
        data_offset = files_offset + len(file)
        folder = struct.pack("<IHH", data_offset, 1, compression) + folder_extra
        data = struct.pack("<IHH", 0, len(payload), min(len(payload), 32768)) + data_extra + payload
        length = 36 + len(header_extra) + len(folder) + len(file) + len(data)
        header = struct.pack("<4sIIIII BBHHHHH", b"MSCF", 0, length, 0, files_offset, 0, 3, 1, 1, 1,
                             4 if reserves else 0, 7, 0)
        return header + header_extra + folder + file + data

    def test_cabinet_header_in_compressed_cfdata_is_not_an_independent_container(self):
        nested = struct.pack("<4sIIIII BBHHHHH", b"MSCF", 0, 48, 0, 40, 0, 3, 1, 1, 1, 0, 7, 0) + b"inertpayload"
        outer = self.cabinet_fixture(b"prefix" + nested + b"suffix", reserves=True)
        raw = b"MZ inert wrapper" + outer
        rows = analysis.embedded_cabinets(raw)
        self.assertEqual(len(rows), 2)
        whole, inner = rows
        self.assertTrue(whole["layout"]["complete"])
        self.assertEqual((whole["layout"]["header_reserve_bytes"], whole["layout"]["folder_reserve_bytes"],
                          whole["layout"]["data_reserve_bytes"]), (2, 1, 2))
        self.assertFalse(inner["layout"]["complete"])
        parent = inner["containing_compressed_blocks"][0]
        self.assertEqual(parent["outer_sha256"], hashlib.sha256(outer).hexdigest())
        self.assertEqual(parent["compression"], "MSZip")
        self.assertTrue(parent["candidate_entirely_within_block"])
        block = parent["block"]
        self.assertEqual(raw[block["payload_offset"]:block["payload_offset"] + block["compressed_bytes"]],
                         b"prefix" + nested + b"suffix")
        self.assertFalse(whole["layout"]["compression_verified"])

    def test_cabinet_literal_member_in_uncompressed_folder_is_not_flagged_as_compressed_stream(self):
        nested = self.cabinet_fixture(b"inert nested payload", compression=0)
        rows = analysis.embedded_cabinets(self.cabinet_fixture(nested, compression=0))
        self.assertEqual(len(rows), 2)
        self.assertTrue(all(row["layout"]["complete"] for row in rows))
        self.assertFalse(any(row["containing_compressed_blocks"] for row in rows))

    def test_cabinet_layout_rejects_truncated_cfdata_without_dropping_the_bounded_header(self):
        raw = bytearray(self.cabinet_fixture(b"inert compressed payload"))
        block = analysis.embedded_cabinets(raw)[0]["layout"]["folders"][0]["data_blocks"][0]
        struct.pack_into("<H", raw, block["header_offset"] + 4, 65535)
        rows = analysis.embedded_cabinets(raw)
        self.assertEqual(len(rows), 1)
        self.assertFalse(rows[0]["layout"]["complete"])
        self.assertIn("exceeds", rows[0]["layout"]["failure"])

    def test_embedded_resources_are_original_byte_ranges_and_keep_manifest_failures(self):
        import struct
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            body = b"PK inert synthetic original payload"
            raw = b"synthetic managed prefix" + struct.pack("<I", len(body)) + body + b"suffix"
            name = ".analysis/inputs/official-native-current/quarantine/managed.exe"
            path = root / name
            path.parent.mkdir(parents=True)
            path.write_bytes(raw)
            parent = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                      "verified_against_manifest": False, "declared_sha256": "0" * 64}
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [parent]}), encoding="utf-8")
            analyzer = root / "tools/analysis.py"
            analyzer.parent.mkdir()
            analyzer.write_bytes(b"synthetic analyzer identity")
            resource = root / ".analysis/results/host-current/managed/fixture/source/payload.zip"
            resource.parent.mkdir(parents=True)
            resource.write_bytes(body)
            output = {"path": resource.relative_to(root).as_posix(), "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest()}
            capture = root / ".analysis/results/host-current/managed/capture.json"
            capture.write_text(json.dumps({"assemblies": [{"input": name, "capture_complete": True, "outputs": [output]}]}), encoding="utf-8")
            with patch.object(analysis, "ROOT", root), patch.object(analysis, "__file__", str(analyzer)), \
                 patch.object(analysis.subprocess, "run") as run, \
                 patch.object(analysis, "refresh_gaps") as refresh, redirect_stdout(io.StringIO()):
                result = analysis.extract_embedded_inputs(argparse.Namespace(no_refresh_gaps=True))
            self.assertEqual(result["summary"]["managed_resources"], 1)
            retained = result["inputs"][0]
            self.assertEqual((root / retained["path"]).read_bytes(), body)
            self.assertEqual(raw[retained["container_offset"]:retained["container_offset"] + retained["bytes"]], body)
            self.assertFalse(retained["verified_against_manifest"])
            self.assertEqual(path.read_bytes(), raw)
            run.assert_not_called()
            refresh.assert_not_called()

    def test_embedded_cabinet_scan_uses_mz_bytes_including_dll_and_opaque_payloads(self):
        cabinet = struct.pack("<4sIIIII BBHHHHH", b"MSCF", 0, 48, 0, 40, 0, 3, 1, 1, 1, 0, 7, 0) + b"inertpayload"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            rows = []
            for name, raw, extra in (
                ("custom-action.dll", b"MZ inert DLL prefix" + cabinet, {}),
                ("a0", b"MZ inert opaque prefix" + cabinet, {}),
                ("not-pe.exe", b"inert non-PE prefix" + cabinet, {}),
                ("partial.dll", b"MZ inert partial prefix" + cabinet, {"source_bytes_verified": False}),
                ("changed.dll", b"MZ inert changed prefix" + cabinet, {"sha256": "0" * 64}),
            ):
                path = root / ".analysis/inputs/official-native-current/extracted" / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(raw)
                rows.append({"path": path.relative_to(root).as_posix(), "bytes": len(raw),
                             "sha256": hashlib.sha256(raw).hexdigest(), **extra})
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": rows}), encoding="utf-8")
            analyzer = root / "tools/analysis.py"
            analyzer.parent.mkdir()
            analyzer.write_bytes(b"synthetic analyzer identity")
            with patch.object(analysis, "ROOT", root), patch.object(analysis, "__file__", str(analyzer)), \
                 patch.object(analysis.subprocess, "run") as run, redirect_stdout(io.StringIO()):
                result = analysis.extract_embedded_inputs(argparse.Namespace(no_refresh_gaps=True))
            self.assertEqual(result["summary"]["portable_executables_scanned"], 2)
            self.assertEqual(result["summary"]["cabinet_candidates"], 2)
            self.assertEqual(result["summary"]["excluded_partial_inputs"], 1)
            self.assertEqual(len(result["failures"]), 1)
            self.assertTrue(result["failures"][0]["input"].endswith("changed.dll"))
            self.assertEqual({Path(row["container"]).name for row in result["inputs"]}, {"custom-action.dll", "a0"})
            for artifact in result["inputs"]:
                self.assertEqual((root / artifact["path"]).read_bytes(), cabinet)
            run.assert_not_called()

    def test_embedded_same_byte_host_and_manifest_parents_preserve_all_verified_edges(self):
        cabinet = self.cabinet_fixture(b"inert compressed bytes")
        body = b"MZ inert same-byte parents" + cabinet
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            rows = []
            for name, extra in (
                ("host-current/driver.dll", {}),
                ("official-native-current/driver.dll", {"verified_against_manifest": True,
                                                         "container_extraction_complete": True}),
                ("official-native-current/partial.dll", {"source_bytes_verified": False}),
            ):
                path = root / ".analysis/inputs" / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(body)
                rows.append({"path": path.relative_to(root).as_posix(), "bytes": len(body),
                             "sha256": hashlib.sha256(body).hexdigest(), **extra})
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": rows}), encoding="utf-8")
            analyzer = root / "tools/analysis.py"
            analyzer.parent.mkdir()
            analyzer.write_bytes(b"synthetic analyzer identity")
            with patch.object(analysis, "ROOT", root), patch.object(analysis, "__file__", str(analyzer)), \
                 patch.object(analysis.subprocess, "run") as run, redirect_stdout(io.StringIO()):
                result = analysis.extract_embedded_inputs(argparse.Namespace(
                    source_prefix=".analysis/inputs/", no_refresh_gaps=True))
            self.assertFalse(result["failures"])
            self.assertEqual(result["summary"]["portable_executables_scanned"], 1)
            artifact = result["inputs"][0]
            self.assertEqual(artifact["container"], rows[1]["path"])
            self.assertTrue(artifact["verified_against_manifest"])
            self.assertEqual({row["path"] for row in artifact["container_aliases"]},
                             {rows[0]["path"], rows[1]["path"]})
            self.assertTrue(all(row["byte_identity_verified"] for row in artifact["container_aliases"]))
            self.assertEqual((root / artifact["path"]).read_bytes(), cabinet)
            run.assert_not_called()

    def test_embedded_gap_classification_never_calls_a_compressed_header_a_damaged_package(self):
        report = {"inputs": [
            {"path": "compressed.cab", "sha256": "1" * 64, "extraction": {
                "kind": "bounded_cabinet_candidate", "cabinet": {
                    "layout": {"complete": False, "failure": "inert table bounds failure"},
                    "containing_compressed_blocks": [{"outer_static_extraction": {"current_complete": True}}]}}},
            {"path": "complete.cab", "sha256": "2" * 64, "extraction": {
                "kind": "bounded_cabinet_candidate", "cabinet": {"layout": {"complete": True}},
                "static_extraction": {"current_complete": True}}},
            {"path": "pending.cab", "sha256": "3" * 64, "extraction": {
                "kind": "bounded_cabinet_candidate", "cabinet": {"layout": {"complete": True}},
                "static_extraction": {"current_complete": False}}},
        ]}
        rows = analysis.embedded_input_gaps(report, "synthetic receipt")
        self.assertEqual([row["kind"] for row in rows], [
            "embedded_cabinet_header_inside_compressed_stream", "embedded_cabinet_extraction_pending",
            "embedded_installation_chain_semantics_pending"])
        self.assertFalse(rows[0]["independent_cabinet_established"])
        self.assertTrue(rows[0]["outer_extraction_verified_at_capture"])
        self.assertEqual(rows[1]["input"], "pending.cab")

    def prepare(self, root, *, verified_alias=False):
        import families

        def source(name, raw):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

        tool = source("tools/static/7z.exe", b"synthetic static extractor identity; never executed")
        source("tools/static/7z.dll", b"synthetic extractor library identity")
        container = source(".analysis/inputs/official-native-current/quarantine/synthetic.exe", b"inert synthetic container")
        manifest = {"url": "https://apps.razer.com/synapse/products/1/mw/manifest.json"}
        url = "https://dl.razerzone.com/synthetic-driver.exe"
        failed = {"url": url, "status": "failed", "declared_sha256": "f" * 64,
                  "declared_bytes": container["bytes"] + 1, "manifests": [manifest],
                  "error": "Synthetic manifest bytes differ", "observed_artifact": {**container, "url": url}}
        resources = [failed]
        if verified_alias:
            alias = source(".analysis/inputs/official-native-current/resources/verified.exe", b"inert synthetic container")
            resources.append({**alias, "url": "https://dl.razerzone.com/synthetic-verified.exe", "status": "verified",
                              "declared_sha256": alias["sha256"], "declared_bytes": alias["bytes"], "manifests": [manifest]})
        receipt = root / ".analysis/inputs/official-native-current/acquisition.json"
        raw = json.dumps({"resources": resources}).encode()
        receipt.write_bytes(raw)
        (root / ".analysis/inputs.json").write_text('{"inputs":[]}', encoding="utf-8")
        args = argparse.Namespace(seven_zip=str(root / tool["path"]), family="synthetic", workers=2,
                                  include_manifest_mismatches=True)

        def static_extractor(command, **kwargs):
            self.assertEqual(command[0], args.seven_zip)
            if command[1] == "l":
                return argparse.Namespace(returncode=0, stdout=b"Type = Nsis\n----------\nPath = RzSynthetic.sys\nSize = 20\n", stderr=b"")
            self.assertEqual(command[1], "x")
            destination = Path(next(value[2:] for value in command if value.startswith("-o")))
            (destination / "RzSynthetic.sys").write_bytes(b"inert synthetic driver")
            return argparse.Namespace(returncode=0, stdout=b"Everything is Ok\n", stderr=b"")

        return families, args, receipt, raw, static_extractor

    def test_manifest_mismatches_require_explicit_inclusion_and_remain_unverified_on_reuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            families, args, receipt, raw, extractor = self.prepare(root)
            with patch.object(analysis, "ROOT", root), patch.object(families.subprocess, "run", side_effect=extractor) as run, \
                 redirect_stdout(io.StringIO()):
                args.include_manifest_mismatches = False
                families.extract_native_packages(args, [1])
                run.assert_not_called()
                result_path = root / ".analysis/results/families/synthetic-native.json"
                self.assertFalse(json.loads(result_path.read_bytes())["packages"])
                args.include_manifest_mismatches = True
                families.extract_native_packages(args, [1])
                included = json.loads(result_path.read_bytes())
                row = included["packages"][0]
                self.assertTrue(row["extracted"])
                self.assertFalse(row["current_resource_binding"]["verified_against_manifest"])
                self.assertEqual(row["current_resource_binding"]["declared_sha256"], "f" * 64)
                self.assertTrue(all(item["verified_against_manifest"] is False for item in row["files"]))
                self.assertEqual(included["summary"]["manifest_mismatch_packages"], 1)
                inspection = root / ".analysis/results/native-packages" / row["container"]["sha256"] / "inspection.json"
                original_inspection = inspection.read_bytes()
                families.extract_native_packages(args, [1])
                self.assertEqual(run.call_count, 2)
                self.assertEqual(inspection.read_bytes(), original_inspection)
                indexed = json.loads((root / ".analysis/inputs.json").read_bytes())["inputs"]
                self.assertTrue(all(item["verified_against_manifest"] is False for item in indexed))
                self.assertEqual(receipt.read_bytes(), raw)

    def test_identical_observed_payload_does_not_replace_verified_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            families, args, receipt, raw, extractor = self.prepare(root, verified_alias=True)
            with patch.object(analysis, "ROOT", root), patch.object(families.subprocess, "run", side_effect=extractor) as run, \
                 redirect_stdout(io.StringIO()):
                families.extract_native_packages(args, [1])
                result = json.loads((root / ".analysis/results/families/synthetic-native.json").read_bytes())
                self.assertEqual(run.call_count, 2)
                self.assertEqual(len(result["packages"]), 1)
                row = result["packages"][0]
                self.assertTrue(row["current_resource_binding"]["verified_against_manifest"])
                self.assertEqual({item["verified_against_manifest"] for item in row["current_resource_bindings"]}, {True, False})
                self.assertEqual(result["summary"]["manifest_mismatch_packages"], 1)
                self.assertEqual(receipt.read_bytes(), raw)


class AcquisitionPathTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "nt", "Win32 path aliases only")
    def test_extended_drive_and_unc_spelling_keep_exact_resolved_identity(self):
        for plain, extended in (
            (r"E:\workspace\inputs\source.js", r"\\?\E:\workspace\inputs\source.js"),
            (r"\\server\share\inputs\source.js", r"\\?\UNC\server\share\inputs\source.js"),
        ):
            with self.subTest(plain=plain):
                self.assertEqual(analysis._resolved_path_identity(Path(extended)), Path(plain))
                self.assertEqual(analysis._resolved_path_identity(Path(plain)), Path(plain))
        device = Path(r"\\?\GLOBALROOT\Device\HarddiskVolume1\source.js")
        self.assertEqual(analysis._resolved_path_identity(device), device)

    @unittest.skipUnless(os.name == "nt", "Win32 path aliases only")
    def test_project_relative_receipts_and_input_boundary_use_same_path_identity(self):
        root = Path(r"E:\workspace")
        source = root / ".analysis/inputs/current/source.js"
        extended = Path("\\\\?\\" + str(source))
        with patch.object(analysis, "ROOT", root):
            with patch.object(Path, "resolve", return_value=extended):
                self.assertEqual(analysis.rel(source), ".analysis/inputs/current/source.js")
            boundary = root / ".analysis/inputs"
            with patch.object(Path, "resolve", side_effect=[extended, boundary]):
                self.assertEqual(analysis.resolve(".analysis/inputs/current/source.js", ".analysis/inputs"), extended)
            for outside in (root / "outside.js", Path("\\\\?\\" + str(root / "outside.js"))):
                with patch.object(Path, "resolve", side_effect=[outside, boundary]), self.assertRaises(ValueError):
                    analysis.resolve("outside.js", ".analysis/inputs")
            with patch.object(Path, "resolve", side_effect=[boundary, boundary]), self.assertRaises(ValueError):
                analysis.resolve(".analysis/inputs", ".analysis/inputs")

    @unittest.skipUnless(os.name == "nt", "Win32 path aliases only")
    def test_url_containment_accepts_aliases_and_still_rejects_reparse_escape(self):
        output = Path(r"E:\workspace\inputs\current")
        url = "https://apps.razer.com/synapse/assets/fonts/example.woff2"
        expected = output / "raw/apps.razer.com/synapse/assets/fonts/example.woff2"
        for candidate, root in (
            (Path("\\\\?\\" + str(expected)), output),
            (expected, Path("\\\\?\\" + str(output))),
            (Path("\\\\?\\" + str(expected)), Path("\\\\?\\" + str(output))),
        ):
            with self.subTest(candidate=candidate, root=root):
                with patch.object(Path, "resolve", side_effect=[candidate, root]):
                    self.assertEqual(analysis.raw_path(output, url), expected)
        for escaped in (Path(r"E:\workspace\elsewhere\source.js"),
                        Path(r"\\?\E:\workspace\elsewhere\source.js"),
                        Path(r"\\?\UNC\outside\share\source.js")):
            with self.subTest(escaped=escaped):
                with patch.object(Path, "resolve", side_effect=[escaped, output]), self.assertRaises(ValueError):
                    analysis.raw_path(output, url)
        for unsafe in ("https://apps.razer.com/%2e%2e/outside.js",
                       "https://apps.razer.com/assets/%2f..%2foutside.js",
                       "https://apps.razer.com/assets/a%5cb.js"):
            with self.subTest(unsafe=unsafe), self.assertRaises(ValueError):
                analysis.raw_path(output, unsafe)


class LocaleKeysTests(unittest.TestCase):
    def test_strings_preserve_newlines_unicode_and_complete_dictionary_owner(self):
        raw = 'const locale={TITLE:"系统需要重启",BODY:"first\\nsecond",OTHER:()=>1};'.encode()
        result = evidence.locale_keys(raw, ["TITLE", "BODY"])
        self.assertEqual(result["values"], {"TITLE": "系统需要重启", "BODY": "first\nsecond"})
        owner = result["owner"]
        self.assertEqual(raw[owner["start_byte"]:owner["end_byte"]].decode(), owner["body"])
        self.assertIn("OTHER:()=>1", owner["body"])

    def test_ambiguous_dynamic_or_broken_dictionaries_are_rejected(self):
        for raw in (b'const a={TITLE:"a"},b={TITLE:"b"}', b'const a={TITLE:getText()}',
                    b'const a={TITLE:"a",TITLE:"b"}', b'const a={TITLE:'):
            with self.assertRaises(ValueError):
                evidence.locale_keys(raw, ["TITLE"])


class LocaleExportDictionaryTests(unittest.TestCase):
    def source(self):
        return ("const outside='unrelated'; const other=()=>{const en={},zh={};w.d(en,{TITLE:()=>outside});};"
                "(()=>{var en={},zh={};const english='en',chinese='zh-CN',title='Show Guide',alias=title,"
                "cn='显示指南',line='first\\nsecond';w.d(en,{SHOW_GUIDE:()=>alias,LINE:()=>line});"
                "w.d(zh,{SHOW_GUIDE:()=>cn,LINE:()=>line});const words={[english]:en,[chinese]:zh};"
                "function Localizer(key,lang){return words[lang][key]||words.en[key]}})()").encode()

    def test_complete_routing_getters_and_lexical_strings_preserve_source_bytes(self):
        raw = self.source()
        result = evidence.locale_export_dictionaries(raw, ["en", "zh-CN"])
        self.assertEqual(result["values"], {
            "en": {"SHOW_GUIDE": "Show Guide", "LINE": "first\nsecond"},
            "zh-CN": {"SHOW_GUIDE": "显示指南", "LINE": "first\nsecond"}})
        self.assertEqual(result["locale_order"], ["en", "zh-CN"])
        self.assertFalse(result["javascript_executed"])
        regions = [result["routing_owner"], result["complete_factory"]]
        regions += [row["export_object"] for row in result["dictionaries"]]
        regions += [pair["export"] for row in result["dictionaries"] for pair in row["resolved_exports"]]
        for region in regions:
            body = raw[region["start_byte"]:region["end_byte"]]
            self.assertEqual(body.decode(), region["body"])
            self.assertEqual(hashlib.sha256(body).hexdigest(), region["sha256"])
        self.assertIn("function Localizer", result["complete_factory"]["body"])

    def test_dynamic_cyclic_ambiguous_and_escaped_owner_dictionaries_are_rejected(self):
        source = self.source()
        invalid = (
            source.replace(b"title='Show Guide'", b"title=window.guide"),
            source.replace(b"title='Show Guide'", b"title=alias"),
            source.replace(b"SHOW_GUIDE:()=>alias,LINE", b"SHOW_GUIDE:()=>alias,SHOW_GUIDE:()=>alias,LINE"),
            source.replace(b"SHOW_GUIDE:()=>alias", b"SHOW_GUIDE:(key)=>alias"),
            source.replace(b"SHOW_GUIDE:()=>alias", b"SHOW_GUIDE:()=>outside"),
            source.replace(b"w.d(zh,{", b"w.other(zh,{"),
            source.replace(b"var en={},zh={}", b"var en=getWords(),zh={}"),
            source + b"const a={},b={};const duplicate={en:a,'zh-CN':b}",
            source.replace(b"const words={[english]:en,[chinese]:zh}", b"const words={[english]:en,[chinese]:en}"),
            source[:-1],
        )
        for raw in invalid:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                evidence.locale_export_dictionaries(raw, ["en", "zh-CN"])
        for locales in (["en", "en"], ["zh-CN"], []):
            with self.subTest(locales=locales), self.assertRaises(ValueError):
                evidence.locale_export_dictionaries(source, locales)


class JavascriptBindingTests(unittest.TestCase):
    def test_repeated_index_preserves_shadowing_and_rejects_invalid_references(self):
        sys.path.insert(0, str(analysis.ROOT / "tools/analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        raw = b'const child="outer";function f(child){inside(child)}outside(child)'
        tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
        bindings = evidence.JavascriptBindings(raw, tree)
        self.assertEqual(bindings.resolve("child", raw.index(b"inside"))["resolution"], "parameter_requires_caller_analysis")
        outer = bindings.resolve("child", raw.index(b"outside"))
        self.assertEqual(outer["body"], 'child="outer"')
        self.assertEqual(outer, evidence.javascript_binding(raw, tree, "child", raw.index(b"outside")))
        for offset in (-1, len(raw)):
            with self.assertRaises(ValueError):
                bindings.resolve("child", offset)

    def test_profile_root_report_id_numeric_keys_are_literal_data(self):
        import families
        self.assertEqual(
            families.keyboard_static_literal('{version:1,reportIDs:{4:"razerKeyReportID",5:"hardwareEventReportID"},enabled:!1}'),
            {"version": 1, "reportIDs": {"4": "razerKeyReportID", "5": "hardwareEventReportID"}, "enabled": False},
        )
        with self.assertRaisesRegex(ValueError, "Nonliteral"):
            families.keyboard_static_literal('{reportIDs:{4:readDevice()}}')

    def binding(self, raw, name, marker):
        import sys
        sys.path.insert(0, str(analysis.ROOT / "tools/analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
        return evidence.javascript_binding(raw, tree, name, raw.index(marker))

    def test_other_webpack_function_is_never_used_as_a_binding(self):
        raw = b'const modules={1:(m)=>{const icon="wrong";use(icon)},2:(m)=>{use(icon)}}'
        result = self.binding(raw, "icon", b'use(icon)}}')
        self.assertEqual(result["resolution"], "unresolved_identifier")

    def test_enclosing_scope_and_utf8_complete_declaration_are_preserved(self):
        raw = '(()=>{const copy="设备";const Panel=()=>{return R.jsx("div",{children:copy})}})()'.encode()
        result = self.binding(raw, "copy", b'children:copy')
        self.assertEqual(result["resolution"], "declaration")
        self.assertEqual(result["body"], 'copy="设备"')
        self.assertEqual(result["body"].encode(), raw[result["start_byte"]:result["end_byte"]])

    def test_parameter_shadowing_does_not_resolve_to_an_outer_constant(self):
        raw = b'const data="wrong";const Panel=data=>R.jsx("div",{children:data})'
        result = self.binding(raw, "data", b'children:data')
        self.assertEqual(result["resolution"], "parameter_requires_caller_analysis")

    def test_var_hoisting_and_inner_lexical_bindings_are_separate(self):
        raw = b'function f(){if(ok){var data="outer"} {const data="inner";inside(data)}outside(data)}'
        self.assertEqual(self.binding(raw, "data", b'inside(data)')["body"], 'data="inner"')
        self.assertEqual(self.binding(raw, "data", b'outside(data)')["body"], 'data="outer"')


class GamepadLightingTreeTests(unittest.TestCase):
    def tree(self, raw):
        sys.path.insert(0, str(analysis.ROOT / "tools/analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
        source = {"path": ".analysis/inputs/fixture/main.js", "bytes": len(raw),
                  "sha256": hashlib.sha256(raw).hexdigest()}
        bodies = {}
        def keep(node):
            digest = hashlib.sha256(node.text).hexdigest()
            bodies[digest] = node.text.decode()
            return {"byte_start": node.start_byte, "byte_end": node.end_byte, "body_sha256": digest}
        result = families.gamepad_lighting_component_tree(raw, tree, source,
            families.keyboard_source_modules(raw.decode()), keep)
        return result, bodies

    def test_tabs_iframe_wrappers_and_conditional_effects_keep_complete_owners(self):
        raw = (b'(()=>{const Static=()=>R.jsx("div",{}),Wave=()=>R.jsx("svg",{});'
               b'class Effects{renderEffect(){let picked;switch(this.id){case 1:picked=Static;break;'
               b'case 2:picked=Wave}return function(Component,props){return R.jsx(Component,props)}(picked,{})}}'
               b'class Root{render(){return (0,R.jsx)(Effects,{})}}'
               b'const Connected=connect(e=>({lightingEffectState:e.controllerLightingEffectReducer}))(Root);'
               b'const tabs=[{renderComponent:()=>(0,R.jsx)(Connected,{})}];'
               b'class Frame{render(){return (0,R.jsx)(Connected,{isIframe:!0})}}})()')
        result, bodies = self.tree(raw)
        self.assertTrue(result["product_tab_lighting_present"])
        self.assertEqual(len(result["product_tab_mounts"]), 1)
        self.assertEqual(len(result["iframe_mounts"]), 1)
        complete = [bodies[row["body_sha256"]] for row in result["components"]]
        self.assertTrue(any(value.startswith("class Root") for value in complete))
        self.assertTrue(any(value.startswith("class Effects") for value in complete))
        self.assertTrue(any(value.startswith("Static=") for value in complete))
        self.assertTrue(any(value.startswith("Wave=") for value in complete))
        self.assertTrue(any(edge["kind"] == "immediate_parameter" for edge in result["edges"]))
        for row in result["components"]:
            self.assertEqual(raw[row["byte_start"]:row["byte_end"]].decode(), bodies[row["body_sha256"]])

    def test_bundled_iframe_component_does_not_prove_product_tab_mount(self):
        raw = (b'(()=>{const Other=()=>R.jsx("div",{}),Root=()=>R.jsx(Other,{});'
               b'const Connected=connect(e=>({lightingEffectState:e.controllerLightingEffectReducer}))(Root);'
               b'class Frame{render(){return (0,R.jsx)(Connected,{isIframe:!0})}}})()')
        result, _ = self.tree(raw)
        self.assertFalse(result["product_tab_lighting_present"])
        self.assertEqual(result["product_tab_mounts"], [])
        self.assertTrue(result["iframe_mounts"])


class FrontendDeploymentTests(unittest.TestCase):
    setUp = EvidenceTests.setUp
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source

    def test_host_capture_module_resolution_uses_verified_asar_input_and_real_command_root(self):
        original_path = self.source(".analysis/inputs/host-current/asar/archive/electron/common.js", b"complete source")
        original = families.identity(original_path)
        output = self.source(".analysis/results/host-current/javascript/modules/common.js", b"complete parsed source")
        provenance = self.source(".analysis/results/host-current/javascript/modules/provenance.json", json.dumps({
            "modules": {"common.js": {"input": str(original_path.resolve()), "ranges": [[0, 15]], "extraction": "heuristic"}}}).encode())
        capture = {"original_inputs": [original], "outputs": [families.identity(output), families.identity(provenance)],
            "application_js_inputs": 1, "integration_dependency_js_inputs": 0, "target_execution": False,
            "commands": [{"stage": "unpack", "exit_code": 0, "argv": [original["path"]]}]}
        resolved = families.source_capture_modules(capture, original)
        self.assertEqual(len(resolved), 1)
        self.assertEqual(resolved[0]["original_ranges"], [[0, 15]])
        other = {**original, "path": ".analysis/inputs/frontend/elsewhere/common.js"}
        with self.assertRaisesRegex(ValueError, "outside its retained input root"):
            families.source_capture_modules({**capture, "original_inputs": [other]}, other)
        output.write_bytes(b"damaged complete output")
        with self.assertRaisesRegex(ValueError, "differs from its receipt"):
            families.source_capture_modules(capture, original)

    def test_receiver_manifest_records_primary_failure_and_fallback_without_repeating_requests(self):
        receipt = self.source(".analysis/inputs/frontend/accessories-mw-164-current/acquisition.json",
                              b'{"files":[]}')
        root = receipt.parent
        fallback = self.source(str((root / "raw/manifest.json").relative_to(analysis.ROOT)).replace("\\", "/"),
            b'{"productId":164,"vendorId":5426,"rzDeviceConfig":{"claimInterface":0}}')
        primary_url = "https://apps.razer.com/synapse/products/164/mw/lighting-manifests/DeviceManifest_164_0.json"
        fallback_url = "https://apps.razer.com/synapse/lighting-engine/manifest/DeviceManifest_164_0.json"
        attempts = [{"url": primary_url, "error": "HTTP Error 404"},
                    {"url": fallback_url, "http_status": 200, **families.identity(fallback)}]
        def publication(path, data, **kwargs):
            analysis.save(path, data)
        with patch.object(analysis, "download", side_effect=attempts) as fetch, \
                patch.object(families, "publish_frontend_acquisition", side_effect=publication):
            result = families.receiver_lighting_manifest_sources(164, acquire=True)
            self.assertEqual(fetch.call_count, 2)
        self.assertEqual(result["actual_attempts"], attempts)
        self.assertEqual(result["complete_manifest"]["rzDeviceConfig"]["claimInterface"], 0)
        with patch.object(analysis, "download", side_effect=AssertionError("cache must be reused")):
            self.assertEqual(families.receiver_lighting_manifest_sources(164)["original"], attempts[1])
        fallback.write_bytes(b"another product")
        with self.assertRaisesRegex(ValueError, "identity differs"):
            families.receiver_lighting_manifest_sources(164)

    def test_middleware_audit_uses_mounted_mw_capture_and_preserves_query_sections(self):
        sys.path.insert(0, str(Path(__file__).resolve().parent / "analyzers/python-static"))
        name = "gamepad-mw-2636-current"
        base = "https://apps.razer.com/synapse/products/2636/mw/"
        files = []
        for filename, raw in (("index.html", b'<script src="main.current.js"></script>'),
                              ("main.current.js", b"current()"), ("child.js", b"child()")):
            path = self.source(f".analysis/inputs/frontend/{name}/raw/{filename}", raw)
            files.append({"url": base + filename, **families.identity(path)})
        acquisition = {"entry_url": base + "index.html", "files": files,
            "source_current_during_acquisition": True, "declared_references_fetched": False,
            "edges": [{"from": base + "main.current.js", "to": base + "child.js", "kind": "js-literal"}],
            "unresolved": [{"kind": "source-map", "url": base + "main.js.map"}]}
        self.source(f".analysis/inputs/frontend/{name}/acquisition.json", json.dumps(acquisition).encode())
        tool = self.source("tools/analyzers/wakaru-1.14.0/wakaru.exe", b"inert synthetic analyzer")
        outputs = []
        for filename in ("main.current.js", "child.js"):
            output = self.source(f".analysis/results/frontend/{name}/modules/{filename}", b"complete inert fixture")
            outputs.append(families.identity(output))
        capture = {"input": f".analysis/inputs/frontend/{name}", "original_inputs": files[1:],
            "original_js_inputs": [row["path"] for row in files[1:]], "outputs": outputs,
            "tool": {"version": "1.14.0", "sha256": analysis.sha(tool)}, "commands": []}
        self.source(f".analysis/results/frontend/{name}/analysis.json", json.dumps(capture).encode())
        report = self.source(".analysis/results/frontend/gamepad-mw-audit.json", b'{"queries":{"retained":{"products":[]}}}')
        args = argparse.Namespace(family="gamepad", function="mw", product=[2636], query=None)
        maintainer = self.source("tools/families.py", b"inert synthetic maintainer")
        with redirect_stdout(io.StringIO()), patch.object(families, "__file__", str(maintainer)):
            families.audit_family_function(args)
        result = json.loads(report.read_bytes())
        row = result["products"][0]
        self.assertEqual(row["failures"], [])
        self.assertTrue(row["mounted_main"]["path"].endswith("main.current.js"))
        self.assertEqual(len(row["original_inputs"]), 2)
        self.assertEqual(row["complete_capture"]["outputs_verified"], 2)
        self.assertIn("retained", result["queries"])
        self.assertEqual(row["source_set"]["unresolved_acquisition"], acquisition["unresolved"])
        self.source(files[1]["path"], b"changed")
        with redirect_stdout(io.StringIO()), patch.object(families, "__file__", str(maintainer)):
            families.audit_family_function(args)
        self.assertTrue(json.loads(report.read_bytes())["products"][0]["failures"])

    def fixture(self, entry_text, manifests=None, scripts=None):
        base = "https://apps.razer.com/synapse/products/190/ui/"
        files = []
        bodies = {"index.html": entry_text.encode(), **{key: json.dumps(value).encode()
                  for key, value in (manifests or {}).items()}, **(scripts or {})}
        for name, raw in bodies.items():
            relative = ".analysis/inputs/test/raw/" + name
            self.source(relative, raw)
            files.append({"url": base + name, "path": relative, "bytes": len(raw),
                          "sha256": hashlib.sha256(raw).hexdigest()})
        return {"entry_url": base + "index.html", "files": files, "edges": [],
                "source_current_during_acquisition": True}, base

    def dashboard_routes(self, current, old=b""):
        import families
        acquired, _ = self.fixture('<script src="App.current.js"></script>', scripts={
            "App.current.js": current, "App.old.js": old})
        self.source(".analysis/inputs/frontend/synapse-dashboard-current/acquisition.json",
                    json.dumps(acquired).encode())
        return families.route_evidence()

    def test_dashboard_routes_retain_every_current_owner_and_ignore_old_app(self):
        raw = ('const label="设备";function open(p){return `https://apps.razer.com/synapse/products/${p}/ui/index.html`}'
               'const accessory=p=>`/synapse/products/${p}/ui/index.html?accessory=1`;'
               'class Preview{open(p){return `/synapse/products/${p}/ui/index.html?preview=1`}}').encode()
        old = b'function stale(p){return `/synapse/products/${p}/ui/index.html?old=1`}'
        result = self.dashboard_routes(raw, old)
        self.assertEqual(len(result["routes"]), 3)
        self.assertEqual({row["owner"]["node_type"] for row in result["routes"]},
                         {"function_declaration", "arrow_function", "method_definition"})
        for row in result["routes"]:
            self.assertTrue(row["path"].endswith("App.current.js"))
            self.assertTrue(raw.decode()[row["character_offset"]:].startswith("/synapse/products/${"))
            for region in (row["owner"], row["route"]):
                self.assertEqual(raw[region["byte_start"]:region["byte_end"]], region["body"].encode())
            self.assertEqual(row["owner"]["sha256"], hashlib.sha256(row["owner"]["body"].encode()).hexdigest())
        self.assertTrue(any(row["url"].endswith("App.old.js")
                            for row in result["retained_without_current_reference"]))

    def test_dashboard_route_cannot_fall_back_to_retained_or_invalid_source(self):
        old = b'function stale(p){return `/synapse/products/${p}/ui/index.html`}'
        with self.assertRaisesRegex(ValueError, "No current Dashboard product route"):
            self.dashboard_routes(b"function current(){}", old)
        with self.assertRaisesRegex(ValueError, "AST error"):
            self.dashboard_routes(b"function broken({", old)

    def test_old_retained_main_and_explicit_seed_are_not_current_evidence(self):
        acquired, base = self.fixture('<script src="main.new.js"></script>',
            {"asset-manifest.json": {"files": {"main.js": "main.new.js", "page.js": "page.js"},
                                     "entrypoints": ["main.new.js"]}},
            {"main.new.js": b"current()", "main.old.js": b"old()", "page.js": b"conditional()"})
        acquired["edges"] = [{"from": base + "index.html", "to": base + "main.old.js",
                              "kind": "explicit-retained-manifest", "literal": "main.old.js"}]
        result = evidence.frontend_deployment_sources(acquired)
        self.assertEqual(result["failures"], [])
        sources = {row["url"]: row for row in result["sources"]}
        self.assertEqual(sources[base + "main.new.js"]["deployment_role"], "html_include")
        self.assertEqual(sources[base + "page.js"]["deployment_role"], "conditional_or_declared_candidate")
        self.assertNotIn(base + "main.old.js", sources)
        self.assertEqual(result["retained_without_current_reference"][0]["url"], base + "main.old.js")

    def test_redirected_entry_resolves_includes_and_manifests_from_actual_response_url(self):
        acquired, base = self.fixture('<script src="main.js"></script>',
            {"asset-manifest.json": {"files": {"main.js": "main.js"}, "entrypoints": ["main.js"]}},
            {"main.js": b"inert_source"})
        requested = base.rstrip("/")
        entry = next(row for row in acquired["files"] if row["url"] == acquired["entry_url"])
        entry["url"], entry["final_url"] = requested, base
        acquired["entry_url"] = requested
        result = evidence.frontend_deployment_sources(acquired)
        self.assertFalse(result["failures"])
        self.assertEqual(result["entry_response_url"], base)
        self.assertEqual([row["url"] for row in result["sources"]], [base + "main.js"])
        self.assertEqual(result["sources"][0]["deployment_role"], "html_include")
        self.assertEqual(result["manifests"][0]["url"], base + "asset-manifest.json")

    def test_base_tag_shared_script_preload_and_mw_manifest_remain_distinct(self):
        entry = '<base href="sub/"><script src="../shared.js"></script>' \
            '<link rel="preload" href="../lazy.js" as="script"><script>const text="设备";inline()</script>'
        acquired, base = self.fixture(entry,
            {"webpackManifest.json": {"main.js": "shared.js", "lazy.js": "lazy.js"}},
            {"shared.js": b"shared()", "lazy.js": b"lazy()"})
        result = evidence.frontend_deployment_sources(acquired)
        self.assertFalse(result["failures"])
        sources = {row["url"]: row for row in result["sources"]}
        self.assertEqual(sources[base + "shared.js"]["deployment_role"], "html_include")
        self.assertEqual(sources[base + "lazy.js"]["deployment_role"], "conditional_or_declared_candidate")
        self.assertEqual(len(result["inline_scripts"]), 1)
        inline = result["inline_scripts"][0]
        self.assertEqual(inline["body"], 'const text="设备";inline()')
        self.assertEqual(inline["body"].encode(), entry.encode()[inline["start_byte"]:inline["end_byte"]])
        self.assertFalse(result["source_semantics_complete"])

    def test_bad_bytes_missing_chunk_and_wrong_deployment_digest_are_explicit(self):
        acquired, base = self.fixture('<script src="main.js"></script>',
            {"asset-manifest.json": {"files": {"chunk.js": "missing.js"}}}, {"main.js": b"main()"})
        acquired["stability"] = [{"url": base + "index.html", "unchanged": True,
                                  "before_sha256": "0" * 64, "after_sha256": "0" * 64}]
        self.source(".analysis/inputs/test/raw/main.js", b"changed()")
        result = evidence.frontend_deployment_sources(acquired)
        kinds = {row["kind"] for row in result["failures"]}
        self.assertEqual(kinds, {"deployment_original_invalid", "deployment_original_unavailable",
                                "deployment_stability_identity_mismatch"})
        self.assertFalse(any(row["identity_verified"] for row in result["sources"]))

    def test_manifest_entrypoint_does_not_silently_become_an_html_mount(self):
        acquired, base = self.fixture('<script src="current.js"></script>',
            {"asset-manifest.json": {"files": {}, "entrypoints": ["other.js"]}},
            {"current.js": b"current()", "other.js": b"other()", "dependency.js": b"dependency()"})
        acquired["edges"] = [{"from": base + "current.js", "to": base + "dependency.js",
                              "kind": "js-literal", "literal": "dependency.js"}]
        result = evidence.frontend_deployment_sources(acquired)
        self.assertEqual(result["failures"], [{"kind": "manifest_entrypoint_missing_from_html", "url": base + "other.js"}])
        sources = {row["url"]: row for row in result["sources"]}
        self.assertEqual(sources[base + "dependency.js"]["deployment_role"], "conditional_or_declared_candidate")


class HostLocalAuditTests(unittest.TestCase):
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source

    def setUp(self):
        # Import the maintained parser from the actual tool root before the
        # synthetic evidence root is substituted. No source input is executed.
        sys.path.insert(0, str(analysis.ROOT / "tools/analyzers/python-static"))
        EvidenceTests.setUp(self)
        self.audit = host.HostLocalAudit()

    def original(self, name, raw):
        self.source(name, raw)
        return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    def test_host_index_is_exactly_allowed_and_its_change_invalidates_the_snapshot(self):
        path = self.source(".analysis/inputs.json", b'{"inputs":[]}')
        self.assertEqual(self.audit.artifact(path), b'{"inputs":[]}')
        self.assertEqual(self.audit.final_snapshot(), [])
        outside = self.source(".analysis/other.json", b"{}")
        with self.assertRaisesRegex(ValueError, "outside maintained evidence"):
            self.audit.artifact(outside)
        path.write_bytes(b'{"inputs":[{"changed":true}]}')
        self.assertEqual(self.audit.final_snapshot(), [".analysis/inputs.json"])
        with self.assertRaisesRegex(ValueError, "changed during query"):
            self.audit.artifact(path)

    def test_exact_class_field_never_collects_the_unregistration_suffix(self):
        item = self.original(".analysis/inputs/test/host.js", b'class Host{'
            b'registerMacroRecorderEvent=async()=>{return "register"};'
            b'unregisterMacroRecorderEvent=async()=>{return "unregister"}}')
        owner = self.audit.retain_class_field("register", item, "registerMacroRecorderEvent")
        self.assertEqual(self.audit.owners[owner]["body"],
            'registerMacroRecorderEvent=async()=>{return "register"}')
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            self.audit.retain_class_field("missing", item, "register")
        ambiguous = self.original(".analysis/inputs/test/ambiguous.js",
            b'class A{register=()=>1}class B{register=()=>2}')
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            self.audit.retain_class_field("ambiguous", ambiguous, "register")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_browser_input_gate_retains_sender_owners_without_inventing_dom_suppression(self):
        common = ('const ie=(e,o)=>{return(i,s)=>{'
                  'if(o&&!B.noBrowserInputHandlerView.includes(e.webContents.id)){'
                  'if("number"==typeof e.id){const o=pe(e.id);'
                  'if(o?.windowId===e.id&&B.noBrowserInputHandlerView.includes(o.webContents?.id))return}'
                  'runHostShortcut(s)}else console.log("handleBeforeInput ~ blocked")}};'
                  'class BrowserUtil{static registerNoBrowserInputHandler(e){if(e?.sender?.id){const o=e.sender.id;'
                  'B.noBrowserInputHandlerView.includes(o)||B.noBrowserInputHandlerView.push(o)}}'
                  'static unRegisterNoBrowserInputHandler(e){if(e?.sender?.id){const o=e.sender.id;'
                  'B.noBrowserInputHandlerView=B.noBrowserInputHandlerView.filter(e=>e!==o)}}}'
                  'module.exports={handleBeforeInput:ie};')
        main = ('switch(message.action){case"registerNoBrowserInputHandler":Ro.registerNoBrowserInputHandler(n);break;'
                'case"unRegisterNoBrowserInputHandler":Ro.unRegisterNoBrowserInputHandler(n);break;}')
        for index, (change, expected) in enumerate((
                (lambda value: value, None),
                (lambda value: value.replace('runHostShortcut(s)', 's.preventDefault()'), 'suppresses input delivery'),
                (lambda value: value.replace('const o=e.sender.id', 'const o=e.id'), 'sender identity'),
                (lambda value: value.replace('if(o&&!B.', 'if(o&&B.'), 'sender/view branches'))):
            files = {
                'electron/components/Tab/common.js': change(common),
                'electron/components/Tab/TabStore.js': 'module.exports=new class{constructor(){this.noBrowserInputHandlerView=[];this.other=[]}};',
                'electron/main.js': main,
            }
            sources = {suffix: {'original': self.original(f'.analysis/inputs/test/gate-{index}/{Path(suffix).name}', body.encode())}
                       for suffix, body in files.items()}
            with self.subTest(index=index), patch.object(host, 'verified_host_owners', return_value=sources):
                if expected:
                    with self.assertRaisesRegex(ValueError, expected):
                        host.retain_no_browser_input_handler(self.audit)
                else:
                    result = host.retain_no_browser_input_handler(self.audit)
                    self.assertEqual(len(result['owners']), 7)
                    handler = self.audit.owners[result['owners']['handle_before_input']]
                    self.assertTrue(handler['body'].startswith('ie=(e,o)=>'))
                    self.assertTrue(handler['body'].endswith('blocked")}}'))
                    self.assertEqual(handler['body'].encode(), self.audit.read(sources['electron/components/Tab/common.js']['original'])[
                        handler['byte_start']:handler['byte_end']])
                    self.assertIn('cannot justify disabling', result['semantics'])
                    self.assertFalse(result['target_execution'])
                    self.assertEqual(result['runtime_acceptance'], 'unverified')

    def test_callable_member_retains_whole_assignment_field_method_and_object_callback(self):
        source = self.original(".analysis/inputs/test/members.js",
            b'class Engine{constructor(){this.start=async value=>{return value}}'
            b'stop=()=>{return "stopped"};onExit(){return "exit"}}'
            b'const state={onEnter:function(){return "entered"}};')
        root = self.audit.tree(source).root_node
        for name, expected in (("start", 'this.start=async value=>{return value}'),
                               ("stop", 'stop=()=>{return "stopped"}'),
                               ("onExit", 'onExit(){return "exit"}'),
                               ("onEnter", 'onEnter:function(){return "entered"}')):
            node, owner = self.audit.callable_member(name, source, root, name)
            self.assertEqual(node.text.decode(), expected)
            self.assertEqual(self.audit.owners[owner]["body"], expected)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_callable_member_requires_real_receiver_and_rejects_nested_ambiguity_or_noncallable(self):
        source = self.original(".analysis/inputs/test/member-owners.js",
            b'class Outer{constructor(){this.start=()=>1;other.start=()=>2;this.value=3}'
            b'onExit(){const inner={onExit:()=>4};return inner}}')
        root = self.audit.tree(source).root_node
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            self.audit.callable_member("unscoped_start", source, root, "start")
        _, actual = self.audit.callable_member("own_start", source, root, "start", receiver="this")
        self.assertEqual(self.audit.owners[actual]["body"], "this.start=()=>1")
        for name in ("onExit", "value", "missing"):
            with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
                self.audit.callable_member(name, source, root, name)
        class_body = root.named_children[0].child_by_field_name("body")
        method = next(node for node in class_body.named_children
            if node.type == "method_definition" and node.child_by_field_name("name").text == b"onExit")
        inner = next(node for node in method.child_by_field_name("body").named_children[0].named_children
            if node.type == "variable_declarator").child_by_field_name("value")
        _, actual = self.audit.callable_member("nested_callback", source, inner, "onExit")
        self.assertEqual(self.audit.owners[actual]["body"], "onExit:()=>4")

    def test_conflicting_cached_claims_and_changes_to_receipts_remain_failures(self):
        item = self.original(".analysis/inputs/test/a.js", b"const value=1;")
        self.audit.read(item)
        with self.assertRaisesRegex(ValueError, "Conflicting"):
            self.audit.read({**item, "sha256": "0" * 64})
        receipt = self.source(".analysis/results/test/capture.json", b"{}")
        self.audit.artifact(receipt)
        self.assertEqual(self.audit.final_snapshot(), [])
        receipt.write_bytes(b'{"changed":true}')
        self.assertEqual(self.audit.final_snapshot(), [".analysis/results/test/capture.json"])
        with self.assertRaisesRegex(ValueError, "changed during query"):
            self.audit.artifact(receipt)

    def test_cross_chunk_export_uses_the_actual_factory_and_lexical_binding(self):
        first = self.original(".analysis/inputs/test/main.js",
            'self.chunks.push([[1],{1(e,t,n){n.d(t,{A:()=>v});const v="设备"}}]);'.encode())
        second = self.original(".analysis/inputs/test/lazy.js",
            b'self.chunks.push([[2],{2(e,t,n){n.d(t,{A:()=>v});const v="correct"}}]);')
        result = self.audit.webpack_export("test", [first, second], 2, "A")
        self.assertEqual(result["binding"]["body"], 'v="correct"')
        self.assertEqual(result["original"]["path"], second["path"])
        owner = self.audit.owners[result["complete_factory_owner"]]
        self.assertEqual(owner["body"], '2(e,t,n){n.d(t,{A:()=>v});const v="correct"}')
        self.assertEqual(owner["body"].encode(), self.audit.read(second)[owner["byte_start"]:owner["byte_end"]])

    def test_function_export_getters_and_nested_namespace_are_static_and_unambiguous(self):
        source = self.original(".analysis/inputs/test/function-getter.js",
            b'self.chunks.push([[1],{7:function(e,t,n){var nested={};'
            b'n.d(t,{A:function(){return value}});n.d(nested,{MATS:function(){return mats}});'
            b'const value="actual",mats=[{id:"source"}]}}]);')
        result = self.audit.webpack_export("function_getter", [source], 7, "A")
        self.assertEqual(result["binding"]["body"], 'value="actual"')
        result = self.audit.webpack_export("nested_getter", [source], 7, "MATS", namespace="nested")
        self.assertEqual(result["binding"]["body"], 'mats=[{id:"source"}]')
        with self.assertRaisesRegex(ValueError, "absent, ambiguous or dynamic"):
            self.audit.webpack_export("wrong_namespace", [source], 7, "MATS")
        for number, replacement in enumerate((b'function(){sideEffect();return value}', b'function(){return value+1}', b'function(arg){return value}')):
            changed = self.original(".analysis/inputs/test/dynamic-getter-" + str(number) + ".js",
                self.audit.read(source).replace(b'function(){return value}', replacement))
            with self.assertRaises(ValueError):
                self.audit.webpack_export("dynamic_getter", [changed], 7, "A")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_numeric_word_helpers_use_observed_mask_and_shift_directions(self):
        def extracted(expression):
            source = self.original(".analysis/inputs/test/numeric-" + hashlib.sha256(expression).hexdigest() + ".js",
                b'const helper=e=>' + expression + b';')
            node = self.audit.tree(source).root_node.named_children[0].named_children[0].child_by_field_name("value")
            return families.numeric_word_helper(node)
        self.assertEqual(extracted(b'e>>16&65535'), {"shift": 16, "mask": 65535})
        self.assertEqual(extracted(b'65535&e'), {"shift": 0, "mask": 65535})
        self.assertEqual(extracted(b'e>>8&255'), {"shift": 8, "mask": 255})
        self.assertEqual(extracted(b'255&e'), {"shift": 0, "mask": 255})
        for changed in (b'e>>16|65535', b'external&255', b'e>>shift&255'):
            with self.assertRaises(ValueError):
                extracted(changed)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_philips_mats_require_the_complete_branch_and_field_schema(self):
        mats = [{"id": str(i), "surfaceDistance": i + 1, "sqList": list(range(10))} for i in range(16)]
        families.validate_philips_mats(mats)
        for change in (lambda x: x.pop(), lambda x: x[1].update(id=x[0]["id"]),
                       lambda x: x[0].update(extra=True), lambda x: x[0].update(surfaceDistance=True),
                       lambda x: x[0].update(surfaceDistance=0), lambda x: x[0]["sqList"].pop(),
                       lambda x: x[0]["sqList"].__setitem__(0, 65536), lambda x: x[0]["sqList"].__setitem__(0, False)):
            changed = json.loads(json.dumps(mats))
            change(changed)
            with self.assertRaisesRegex(ValueError, "complete mat table"):
                families.validate_philips_mats(changed)

    def test_philips_measurement_requires_complete_literal_timeouts_and_enum_branches(self):
        selectors = {"SURFACE_CALIBRATION_SAMPLE_RATE": 10, "SURFACE_CALIBRATION_SAMPLE_COUNT": 0,
            "SURFACE_CALIBRATION_MEASURED_RESULT": 0, "CALIBRATION_INITIATING_TIMEOUT": 35000,
            "CALIBRATION_GENERAL_TIMEOUT": 60000, "TEMP_MINI_CALIBRATION_TIMEOUT": 5000,
            "CALIBRATION_CONTROL_TYPE": {"Preset": 1, "Manual": 0, "SmartTracking": 3, "Mini_Calibration": 4},
            "CALIBRATION_CONTROL_STAGE": {"Start": 0, "Stop": 1},
            "CALIBRATION_EVENTS_ENUM": {"Start": 0, "End": 1, "Halt": 2, "Reset": 3,
                "Normal_Speed": 4, "Too_Fast": 5, "Cancelled": 6}}
        actual = families.philips_measurement_constants(selectors)
        self.assertEqual(actual["timeouts_ms"], {"initiating": 35000, "general": 60000, "mini_calibrating": 5000})
        self.assertEqual(actual["control_types"]["Mini_Calibration"], 4)
        for change in (lambda x: x.pop("SURFACE_CALIBRATION_SAMPLE_COUNT"),
                       lambda x: x.update(CALIBRATION_GENERAL_TIMEOUT=True),
                       lambda x: x["CALIBRATION_CONTROL_TYPE"].pop("Mini_Calibration"),
                       lambda x: x["CALIBRATION_EVENTS_ENUM"].update(Cancelled=False),
                       lambda x: x["CALIBRATION_CONTROL_STAGE"].update(extra=2)):
            changed = json.loads(json.dumps(selectors))
            change(changed)
            with self.assertRaisesRegex(ValueError, "Philips .* differs or is absent"):
                families.philips_measurement_constants(changed)

    def test_shortcut_capture_catalog_preserves_optional_fields_and_duplicate_order(self):
        row = {"name": "Num Lock", "inputID": "KEY_NUMPAD_NUM_LOCK", "type": "Modifiers",
               "scancode": 69, "virtualKey": "144", "keyCode": "144", "flag": 0}
        rows = [row, {**row, "type": "Navigation", "flag": 2, "outputFlag": 0, "HID": "83"}]
        self.assertIs(families.validate_shortcut_capture_keys(rows), rows)
        self.assertNotIn("outputFlag", rows[0])
        self.assertEqual(rows[1]["outputFlag"], 0)
        self.assertEqual([item["type"] for item in rows], ["Modifiers", "Navigation"])
        excluded = {"$[].type", "$[].virtualKey", "$[].HID"}
        families.validate_shortcut_capture_projection(rows, excluded)
        for changed in (excluded | {"$[].keyCode"}, excluded - {"$[].type"}, set()):
            with self.assertRaisesRegex(ValueError, "Mounted recorder projection"):
                families.validate_shortcut_capture_projection(rows, changed)
        for change in (lambda x: x.clear(), lambda x: x[0].pop("keyCode"),
                       lambda x: x[0].update(scancode=True), lambda x: x[0].update(scancode=-1),
                       lambda x: x[0].update(flag=2**32), lambda x: x[0].update(outputFlag="2"),
                       lambda x: x[0].update(analogKeyID=69), lambda x: x[0].update(inputID=None)):
            changed = json.loads(json.dumps(rows))
            change(changed)
            with self.assertRaisesRegex(ValueError, "Mounted recorder key"):
                families.validate_shortcut_capture_keys(changed)
            with self.assertRaisesRegex(ValueError, "Mounted recorder key"):
                families.validate_shortcut_capture_projection(changed, excluded)

    def test_preference_calls_have_exact_storage_key_and_complete_callback_owners(self):
        source = self.original(".analysis/inputs/test/preferences.js",
            b'const mounted=()=>{const flag=storage.A.get(key);storage.A.set(key,false);other.A.get(key);storage.A.get(otherKey)};')
        operations = families.preference_storage_operations(self.audit, source, self.audit.tree(source).root_node,
            namespace="storage.A", key_expression="key", expected_count=2)
        self.assertEqual([op["method"] for op in operations], ["get", "set"])
        self.assertEqual([op["value_source"] for op in operations], [None, "false"])
        self.assertEqual(len({op["callback_owner"] for op in operations}), 1)
        self.assertEqual(self.audit.owners[operations[0]["callback_owner"]]["body"],
            '()=>{const flag=storage.A.get(key);storage.A.set(key,false);other.A.get(key);storage.A.get(otherKey)}')
        for body, message in ((b'const mounted=()=>storage.A.set(key);', "argument count"),
                              (b'const flag=storage.A.get(key);', "callback owner"),
                              (b'const mounted=()=>other.A.get(key);', "call count")):
            changed = self.original(".analysis/inputs/test/preferences-" + hashlib.sha256(body).hexdigest() + ".js", body)
            with self.assertRaisesRegex(ValueError, message):
                families.preference_storage_operations(self.audit, changed, self.audit.tree(changed).root_node,
                    namespace="storage.A", key_expression="key", expected_count=1)

    def test_tracking_headers_follow_actual_methods_and_last_comma_expression(self):
        names = ("getProximitySensorState", "setProximitySensorState", "getProximitySensorConfiguration",
                 "setProximitySensorConfiguration", "setProximitySensorLiftSetting")
        headers = ([3, 11, 131], [3, 11, 3], [10, 11, 133], [10, 11, 5], [4, 11, 11])
        declarations = ";".join("const h" + str(i) + "=" +
            ("(new Uint8Array([4,11,139]),new Uint8Array(" + json.dumps(header) + "))" if i == 4 else
             "new Uint8Array(" + json.dumps(header) + ")") for i, header in enumerate(headers))
        body = declarations + ";class Effective{" + "".join(
            name + "(){return device.sendCommand(h" + str(i) + ",name,bytes)}" for i, name in enumerate(names)) + "}"
        source = self.original(".analysis/inputs/test/tracking.js", body.encode())
        result, owners = families.tracking_command_headers(self.audit, source, self.audit.tree(source).root_node)
        self.assertEqual(result, dict(zip(names, headers)))
        self.assertEqual(len(owners), 5)
        changed = self.original(".analysis/inputs/test/changed-tracking.js", body.replace('[3, 11, 131]', '[3, 11, 132]').encode())
        result, _ = families.tracking_command_headers(self.audit, changed, self.audit.tree(changed).root_node)
        self.assertEqual(result["getProximitySensorState"], [3, 11, 132])
        duplicated = self.original(".analysis/inputs/test/duplicated-tracking.js", (body +
            'class Misleading{getProximitySensorState(){return device.sendCommand(h0,name,bytes)}}').encode())
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            families.tracking_command_headers(self.audit, duplicated, self.audit.tree(duplicated).root_node)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_shortcut_defaults_are_scoped_to_the_complete_actual_generator(self):
        body = (b'self.chunks.push([[1],{42358(e,t,n){class Factory{constructor(){'
                b'this.defaultTurbos=["misleading"];'
                b'this.generateDefaultTurbos=()=>{this.defaultTurbos = ["first","second"];return this.defaultTurbos}'
                b'}}}}]);')
        source = self.original(".analysis/inputs/test/shortcut-defaults.js", body)
        module = families.keyboard_source_modules(body.decode())[0]
        values, owner, builder = families.global_shortcut_default_turbos(self.audit, source, module)
        self.assertEqual(values, ["first", "second"])
        self.assertIsNone(builder)
        self.assertEqual(self.audit.owners[owner]["body"],
            'this.generateDefaultTurbos=()=>{this.defaultTurbos = ["first","second"];return this.defaultTurbos}')
        duplicated = self.original(".analysis/inputs/test/duplicate-shortcut-defaults.js",
            body.replace(b';return this.defaultTurbos', b';this.defaultTurbos=["other"];return this.defaultTurbos'))
        with self.assertRaisesRegex(ValueError, "list is absent or ambiguous"):
            families.global_shortcut_default_turbos(self.audit, duplicated,
                families.keyboard_source_modules(self.audit.read(duplicated).decode())[0])
        missing = self.original(".analysis/inputs/test/missing-shortcut-defaults.js",
            body.replace(b'generateDefaultTurbos', b'generateOtherTurbos'))
        with self.assertRaisesRegex(ValueError, "generator is absent or ambiguous"):
            families.global_shortcut_default_turbos(self.audit, missing,
                families.keyboard_source_modules(self.audit.read(missing).decode())[0])
        mapped = self.original(".analysis/inputs/test/mapped-shortcut-defaults.js", body.replace(
            b'class Factory', b'const makeRecord=key=>({name:key});class Factory').replace(
            b'["first","second"];', b'["first","second"].map(key=>makeRecord(key));'))
        values, _, builder = families.global_shortcut_default_turbos(self.audit, mapped,
            families.keyboard_source_modules(self.audit.read(mapped).decode())[0])
        self.assertEqual(values, ["first", "second"])
        self.assertEqual(builder["body"], 'makeRecord=key=>({name:key})')
        self.assertEqual(self.audit.owners[builder["owner_id"]]["body"], builder["body"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_shortcut_comparison_checks_all_key_fields_event_order_and_real_branches(self):
        def copied(value):
            return json.loads(json.dumps(value))
        actual = {"keys": [{"inputID": "KEY_A", "scancode": 30, "analogKeyID": 31, "virtualKey": "65"}],
                  "default_turbo_ids": ["first", "second"],
                  "turbo_events": {"first": [{"type": "keyboard", "scancode": 30, "flag": 2}, {"type": "delay", "ms": 10}], "second": []},
                  "constants": {"SINGLE_STATE_INPUT_ID": ["ScrollUp"], "SCROLL_INPUT_ID": ["ScrollUp"], "SCROLL_TURBO_DELAY_MS": 30}}
        oracle = {"70": copied(actual), "190": copied(actual)}
        oracle["190"]["constants"]["SINGLE_STATE_INPUT_ID"].append("different-actual-branch")
        comparisons = families.compare_global_shortcut_mapping(actual, oracle)
        self.assertTrue(all(comparisons["70"].values()))
        self.assertFalse(comparisons["190"]["constants"])
        changed = copied(oracle)
        changed["190"]["keys"][0]["analogKeyID"] = 32
        with self.assertRaisesRegex(ValueError, "differs"):
            families.compare_global_shortcut_mapping(actual, changed)
        for field in ("keys", "default_turbo_ids", "turbo_events", "constants"):
            changed = copied(actual)
            if field == "keys":
                changed[field][0]["virtualKey"] = "66"
            elif field == "default_turbo_ids":
                changed[field].reverse()
            elif field == "turbo_events":
                changed[field]["first"].reverse()
            else:
                changed[field]["SCROLL_TURBO_DELAY_MS"] = 31
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "differs"):
                families.compare_global_shortcut_mapping(changed, oracle)
        with self.assertRaisesRegex(ValueError, "differs"):
            families.compare_global_shortcut_mapping(actual, {"190": copied(actual)})
        changed = copied(actual)
        changed["default_turbo_ids"].append("first")
        with self.assertRaisesRegex(ValueError, "incomplete or ambiguous"):
            families.compare_global_shortcut_mapping(changed, oracle)
        changed = copied(oracle)
        del changed["190"]["constants"]
        with self.assertRaisesRegex(ValueError, "incomplete fields"):
            families.compare_global_shortcut_mapping(actual, changed)

    def test_complete_frontend_capture_checks_all_bytes_and_keeps_failed_stages(self):
        original = self.original(".analysis/inputs/frontend/test/raw/a.js",
            'const label="设备";function owner(){return label;}'.encode())
        output = self.original(".analysis/results/frontend/test/unpack/a.js", b"inert static output")
        executable = self.original("tools/analyzers/wakaru-1.14.0/wakaru.exe", b"inert analyzer fixture")
        capture = {"input": ".analysis/inputs/frontend/test", "original_inputs": [original, original],
            "original_js_inputs": [original["path"]], "outputs": [output],
            "tool": {"name": "Wakaru", "version": "1.14.0", "sha256": executable["sha256"]},
            "commands": [{"stage": "unpack", "exit_code": 0}, {"stage": "validate", "exit_code": 1}],
            "semantic_audit": "pending", "implementation_acceptance": "pending"}
        self.source(".analysis/results/frontend/test/analysis.json", json.dumps(capture).encode())
        result = self.audit.frontend_capture("test", [original])
        self.assertEqual(result["originals_verified"], 1)
        self.assertEqual(result["outputs_verified"], 1)
        self.assertEqual(result["repeated_identical_original_entries"], [original["path"]])
        self.assertEqual(result["capture_failures"], [capture["commands"][1]])
        self.assertFalse(result["successful_full_capture"])
        self.assertFalse(result["target_execution"])
        self.assertEqual(result["semantic_audit"], "pending")
        owners = self.audit.retain("complete_utf8_function", original, "return label", {"function_declaration"})
        owner = self.audit.owners[owners[0]]
        self.assertEqual(owner["byte_start"], self.audit.read(original).index(b"function"))
        self.assertEqual(owner["body"], "function owner(){return label;}")
        self.assertEqual(self.audit.final_snapshot(), [])
        self.source(output["path"], b"corrupt static output")
        with self.assertRaisesRegex(ValueError, "identity differs"):
            host.HostLocalAudit().frontend_capture("test", [original])

    def test_transform_failure_recovery_retains_original_factory_and_analyzer_failure(self):
        raw = b'const factories={284:(module,exports,require)=>{exports.value=module;}};'
        source = self.original('.analysis/inputs/frontend/test/raw/apps.razer.com/synapse/assets/js/@sentry/tracing/test.js', raw)
        start = raw.index(b'exports.value')
        end = raw.index(b'}', start)
        issue = {'filename': 'module-284.js', 'kind': 'esm_commonjs_residual', 'line': 1, 'column': 1}
        stdout = '.analysis/results/frontend/test/validate.stdout.txt'
        self.source(stdout, json.dumps({'findings': [issue]}).encode())
        self.source('.analysis/results/frontend/test/modules/provenance.json', json.dumps({'modules': {
            'module-284.js': {'input': 'apps.razer.com/synapse/assets/js/@sentry/tracing/test.js',
                              'ranges': [[start, end]]}}}).encode())
        capture = {'capture_failures': [{'stage': 'validate', 'exit_code': 1, 'stdout': stdout}]}
        result = families.recover_frontend_validation_owners(self.audit, 'test', [source], capture)
        self.assertTrue(result['raw_analyzer_failure_preserved'])
        self.assertFalse(result['original_source_blocked'])
        recovered = result['recovered'][0]
        self.assertEqual(recovered['factory_parameters'], [['module', 'exports', 'require']])
        self.assertFalse(recovered['transformed_output_authoritative'])
        owner = self.audit.owners[recovered['lexical_owners'][0]]
        self.assertEqual(owner['body'], '(module,exports,require)=>{exports.value=module;}')
        self.assertEqual(self.audit.owners[recovered['complete_runtime']]['body'], raw.decode())
        self.assertEqual(capture['capture_failures'][0]['exit_code'], 1)

    def test_transform_failure_recovery_keeps_unknown_findings_and_missing_inputs_blocked(self):
        raw = b'const factories={1:(module,exports,require)=>{exports.value=module;}};'
        source = self.original('.analysis/inputs/frontend/test/raw/unknown.js', raw)
        start = raw.index(b'exports.value'); end = raw.index(b'}', start)
        stdout = '.analysis/results/frontend/test/validate.stdout.txt'
        self.source(stdout, json.dumps({'findings': [{'filename': 'module.js', 'kind': 'unknown_failure'}]}).encode())
        provenance = {'modules': {'module.js': {'input': 'unknown.js', 'ranges': [[start, end]]}}}
        self.source('.analysis/results/frontend/test/modules/provenance.json', json.dumps(provenance).encode())
        capture = {'capture_failures': [{'stage': 'validate', 'exit_code': 1, 'stdout': stdout}]}
        result = families.recover_frontend_validation_owners(self.audit, 'test', [source], capture)
        self.assertTrue(result['original_source_blocked'])
        self.assertEqual(result['unresolved'][0]['reason'], 'unclassified_transform_finding')
        with self.assertRaisesRegex(ValueError, 'outside verified deployment'):
            families.recover_frontend_validation_owners(host.HostLocalAudit(), 'test', [], capture)

    def test_mouse_profile_schema_audit_preserves_edition_methods_and_unversioned_identity_mismatch(self):
        for product, declared, schema in [(229, 229, 12), (162, 163, None), (105, 104, 13)]:
            with self.subTest(product=product):
                body = ('n.d(t,{' + ('currentSchemaVersion:()=>schema,' if schema is not None else '') +
                    'getDeviceDefaultLocalData:()=>make,getDefaultProfile:()=>profile,'
                    'DeviceInfo:()=>info,EDITION_DEFAULT_PROFILE:()=>profiles});'
                    f'const schema={schema or 0},info={{productId:{declared}' + (',dongleId:105' if product == 105 else '') +
                    '},profiles={0:{dpiStages:{enable:false}}};'
                    'function make(){return {profiles:[profile()]}}function profile(){return profiles[0]}'
                    'function cache(e){if(e.serialNumber)return e.deviceMetadatas;'
                    'console.log("no serial number, do not get from localstorage")}')
                raw = (('const factories={1(e,t,n){' + body + '}};') if schema is not None
                       else ('(function(){const t={},n={};' + body + '})();')).encode()
                name = f'mouse-mw-{product}-current'
                self.source(f'.analysis/inputs/frontend/{name}/acquisition.json', b'{}')
                source = self.original(f'.analysis/inputs/frontend/{name}/raw/main.js', raw)
                args = argparse.Namespace(family='mouse', function='mw', product=[product])
                with patch.object(host.HostLocalAudit, 'source_set', return_value=({'deployment': {'failures': []}}, [source])), \
                        patch.object(families, 'identity', return_value={'path': 'tools/families.py', 'sha256': 'fixture'}), \
                        redirect_stdout(io.StringIO()):
                    families.audit_mouse_local_profile_schema(args)
                receipt = json.loads((self.root / '.analysis/results/frontend/mouse-mw-audit.json').read_bytes())
                row = next(row for row in receipt['queries']['local_profile_schemas']['products'] if row['product_id'] == product)
                self.assertEqual(row['schema_version'], schema)
                self.assertEqual(row['declared_product_id'], declared)
                self.assertIn('EDITION_DEFAULT_PROFILE', row['product_exports'])
                self.assertEqual(row['failures'], ['official_deployment_product_identity_mismatch'] if product == 162 else [])
                if product == 105:
                    self.assertEqual(row['product_identity_relation'], 'dongleId')
                    self.assertEqual(len(receipt['queries']['local_profile_schemas']['products']), 3)
                self.assertTrue(row['inputs_unchanged'])

    def test_complete_frontend_capture_rejects_missing_deployment_and_conflicting_originals(self):
        original = self.original(".analysis/inputs/frontend/test/raw/a.js", b"const value=1;")
        missing = self.original(".analysis/inputs/frontend/test/raw/new.js", b"const value=2;")
        capture = {"input": ".analysis/inputs/frontend/test", "original_inputs": [original],
            "original_js_inputs": [original["path"]]}
        path = ".analysis/results/frontend/test/analysis.json"
        self.source(path, json.dumps(capture).encode())
        with self.assertRaisesRegex(ValueError, "does not cover current deployment"):
            self.audit.frontend_capture("test", [original, missing])
        capture["original_inputs"].append({**original, "sha256": "0" * 64})
        self.source(path, json.dumps(capture).encode())
        with self.assertRaisesRegex(ValueError, "Conflicting"):
            host.HostLocalAudit().frontend_capture("test", [original])

    def test_application_url_follows_only_unmodified_verified_webpack_string_exports(self):
        exported = self.original(".analysis/inputs/test/constants.js",
            b'self.chunks.push([[1],{1(e,t,n){n.d(t,{A:()=>v});const v="local-studio"}}]);')
        caller = self.original(".analysis/inputs/test/caller.js",
            b'self.chunks.push([[2],{2(e,t,n){var a=n(1);window.open("/synapse/"+a.A,a.A)}}]);')
        references, pending = analysis.host_app_descriptors(self.audit.read(caller).decode())
        self.assertFalse(references)
        self.assertEqual(len(pending), 1)
        resolved = analysis.host_app_static_url(pending[0], caller, [exported, caller], self.audit)
        self.assertEqual(resolved["url_literal"], "/synapse/local-studio")
        self.assertTrue(any(row.get("export") == "A" for row in resolved["binding_chain"]))
        self.assertFalse(self.audit.final_snapshot())
        for name, body in (
            ("reassigned", b'const modules={2:(e,t,n)=>{var a=n(1);a=n(3);window.open("/synapse/"+a.A,a.A)}};'),
            ("mutated", b'const modules={2:(e,t,n)=>{var a=n(1);a.A="changed";window.open("/synapse/"+a.A,a.A)}};'),
            ("arbitrary_call", b'const modules={2:(e,t,n)=>{var a=lookup(1);window.open("/synapse/"+a.A,a.A)}};'),
            ("runtime", b'const modules={2:(e,t,n)=>{window.open("/feedback/?path="+window.location.pathname)}};'),
        ):
            with self.subTest(name=name):
                source = self.original(".analysis/inputs/test/" + name + ".js", body)
                _, unresolved = analysis.host_app_descriptors(body.decode())
                with self.assertRaises(ValueError):
                    analysis.host_app_static_url(unresolved[0], source, [exported, source], self.audit)

    def test_ambiguous_factories_and_dynamic_exports_are_not_silently_resolved(self):
        first = self.original(".analysis/inputs/test/a.js", b'const modules={2:(e,t,n)=>{n.d(t,{A:()=>v});const v=1}};')
        duplicate = self.original(".analysis/inputs/test/b.js", b'const modules={2:(e,t,n)=>{n.d(t,{A:()=>v});const v=2}};')
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            self.audit.webpack_export("duplicate", [first, duplicate], 2, "A")
        dynamic = self.original(".analysis/inputs/test/dynamic.js", b'const modules={3:(e,t,n)=>{n.d(t,{A:()=>lookup()})}};')
        with self.assertRaisesRegex(ValueError, "expression analysis"):
            self.audit.webpack_export("dynamic", [dynamic], 3, "A")
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            self.audit.webpack_export("missing", [first], 4, "A")

    def native_fixture(self, *, captured_bytes=b"\x90\xc3", captured_size=2):
        raw = bytearray(96)
        raw[:2] = b"\x90\xc3"
        base = 0x10000000
        struct.pack_into("<QQ", raw, 32, base + 0x2030, base + 0x1000)
        raw[80:85] = b"mode\0"
        original = self.original(".analysis/inputs/test/library.dll", bytes(raw))
        directory = ".analysis/results/native/" + original["sha256"] + "/"
        function = {"address": hex(base + 0x1000), "end": hex(base + 0x1002), "name": "handler", "assembly": [],
            "chunks": [{"start": hex(base + 0x1000), "end": hex(base + 0x1000 + captured_size),
                        "raw_hex": captured_bytes.hex(), "sha256": hashlib.sha256(captured_bytes).hexdigest()}]}

        def dataset(name, value):
            body = json.dumps(value).encode()
            self.source(directory + name, body)
            return {"path": name, "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest()}

        item = dataset("functions/1000.json", function)
        capture = {"input_sha256": original["sha256"], "image_base": hex(base), "ida_version": "synthetic", "hexrays_version": "synthetic",
            "functions": [{**item, "address": function["address"]}], "outputs": [
                dataset("entrypoints.json", [{"name": "Configure", "address": function["address"]}])], "failures": []}
        dataset("capture.json", capture)
        pe = {"image_base": hex(base), "architecture": "x64", "sections": [
            {"rva": 0x1000, "file_offset": 0, "bytes": 32}, {"rva": 0x2000, "file_offset": 32, "bytes": 64}]}
        return original, pe

    def test_native_dispatch_and_functions_are_checked_against_actual_input_bytes(self):
        original, pe = self.native_fixture()
        with patch.object(host, "inspect_pe", return_value=pe):
            result = self.audit.native_slice(original, ["Configure"], regions=(
                {"role": "synthetic_dispatch", "rva": 0x2000, "bytes": 16, "decode": "pointer_pairs"},))
        self.assertTrue(result["complete_functions"][0]["input_chunks_verified"])
        self.assertEqual(result["verified_data_regions"][0]["pointer_pairs"][0]["key"], "mode")
        self.assertEqual(result["verified_data_regions"][0]["pointer_pairs"][0]["target_rva"], "0x1000")
        self.assertFalse(result["native_semantics_complete"])
        self.assertFalse(result["target_execution"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_native_capture_hashes_cannot_hide_forged_bytes_or_chunk_boundaries(self):
        for value, size, message in ((b"\x90\xc2", 2, "actual PE bytes"), (b"\x90\xc3", 3, "boundary")):
            with self.subTest(value=value, size=size):
                original, pe = self.native_fixture(captured_bytes=value, captured_size=size)
                audit = host.HostLocalAudit()
                with patch.object(host, "inspect_pe", return_value=pe), self.assertRaisesRegex(ValueError, message):
                    audit.native_slice(original, ["Configure"])


class SourceSvgTests(unittest.TestCase):
    def test_react_svg_decimal_gradient_and_group_visibility_keep_geometry(self):
        import workspace
        import xml.etree.ElementTree as ET
        source = 'const Diagram=(e,t)=>{return R.createElement("svg",{viewBox:"0 0 20 20"},R.createElement("defs",null,R.createElement("stop",{offset:.5,stopColor:"#111"})),R.createElement("g",{id:"primary",style:{opacity:.3}},R.createElement("path",{d:"M1 2H3"})),R.createElement("g",{id:"surround"},R.createElement("path",{d:"M4 5H6"})))};'
        content, receipt = workspace.static_react_svg(source, 'Diagram=(e,t)=>{')
        converted, selection = workspace.static_svg_group_visibility(content, {"primary": "hidden", "surround": "visible"})
        root = ET.fromstring(converted)
        self.assertEqual([node.get("d") for node in root.iter("path")], ["M1 2H3", "M4 5H6"])
        self.assertEqual(root.find("defs/stop").get("offset"), "0.5")
        self.assertEqual(root.find("g[@id='primary']").get("style"), "opacity:0.3;visibility:hidden")
        self.assertEqual(selection["groups"]["surround"], "visible")
        self.assertIn('offset:.5', receipt["source_regions"][0]["body"])
        with self.assertRaisesRegex(ValueError, "absent"):
            workspace.static_svg_group_visibility(content, {"missing": "visible"})
        with self.assertRaises(ValueError):
            workspace.static_svg_group_visibility(content, {"primary": "inherit"})

    @staticmethod
    def fixture(kind):
        # Synthetic source only: the maintained extractor parses these bytes;
        # neither a browser nor a JavaScript runtime evaluates the factories.
        declaration = ('Pulse=e=>(0,r.jsxs)("svg",{viewBox:"0 0 40 40",children:['
            '(0,r.jsx)("defs",{children:(0,r.jsx)("clipPath",{id:"clip",children:'
            '(0,r.jsx)("path",{id:"clip-path",d:"M0 0H40V40Z"})})}),'
            '(0,r.jsxs)("g",{id:"geometry",transform:"translate(2 3)",children:['
            '(0,r.jsx)("path",{id:"base-path",d:"M1 1H2V2Z"}),'
            '(0,r.jsxs)("g",{id:"sleeve",clipPath:"url(#clip)",children:['
            '(0,r.jsxs)("g",{id:"pulse-a",opacity:0,children:['
            '(0,r.jsx)("animate",{attributeName:"opacity",values:"0;1;0",dur:"2s"}),'
            '(0,r.jsx)("path",{id:"pulse-a-path",d:"M3 3H4V4Z"})]}),'
            '(0,r.jsxs)("g",{id:"pulse-b",opacity:0.25,children:['
            '(0,r.jsx)("animate",{attributeName:"opacity",values:"1;0;1",dur:"3s"}),'
            '(0,r.jsx)("path",{id:"pulse-b-path",d:"M5 5H6V6Z"})]})]})]})]})')
        owner = 'const label="设备";const ' + declaration + ';'
        if kind == "numeric":
            return 'self.chunks.push([[1],{371:(e,t,n)=>{' + owner + '}}]);', 371
        if kind == "numeric_exponent":
            return 'self.chunks.push([[1],{36e3:(e,t,n)=>{' + owner + '}}]);', 36000
        if kind == "concatenated":
            return '(()=>{' + owner + '})()', "concatenated-entry@1"
        raise AssertionError(kind)

    def extract(self, source, module, layer):
        import workspace
        return workspace.static_jsx_svg(source, {"anchor": "Pulse=e=>", "module_id": module,
                                                "svg_index": 0, "opacity_layer": layer})

    def test_numeric_and_concatenated_base_preserve_static_geometry_and_definitions(self):
        import xml.etree.ElementTree as ET
        for kind in ("numeric", "numeric_exponent", "concatenated"):
            with self.subTest(kind=kind):
                source, module = self.fixture(kind)
                content, evidence = self.extract(source, module, "base")
                svg = ET.fromstring(content)
                ids = {node.get("id") for node in svg.iter() if node.get("id")}
                self.assertEqual(ids, {"clip", "clip-path", "geometry", "base-path", "sleeve"})
                self.assertFalse(any(node.tag.split("}")[-1] == "animate" for node in svg.iter()))
                self.assertEqual([node["id"] for node in evidence["source_opacity_layers"]], ["pulse-a", "pulse-b"])
                self.assertEqual(evidence["source_opacity_layers"][1]["initial_opacity"], .25)
                self.assertEqual(evidence["source_opacity_layers"][0]["animation"]["values"], "0;1;0")
                owner = evidence["producer"]
                self.assertEqual(source[owner["character_start"]:owner["character_end"]], owner["body"])
                self.assertEqual(hashlib.sha256(owner["body"].encode()).hexdigest(), owner["body_sha256"])
                self.assertEqual(evidence["module"]["module_id"], module)
                self.assertFalse(evidence["target_execution"])

    def test_selected_layer_keeps_ancestors_and_complete_defs_without_animation_or_other_layers(self):
        import xml.etree.ElementTree as ET
        for kind in ("numeric", "numeric_exponent", "concatenated"):
            with self.subTest(kind=kind):
                source, module = self.fixture(kind)
                content, evidence = self.extract(source, module, "pulse-a")
                svg = ET.fromstring(content)
                by_id = {node.get("id"): node for node in svg.iter() if node.get("id")}
                self.assertEqual(set(by_id), {"clip", "clip-path", "geometry", "sleeve", "pulse-a", "pulse-a-path"})
                self.assertEqual(by_id["geometry"].get("transform"), "translate(2 3)")
                self.assertEqual(by_id["sleeve"].get("clip-path"), "url(#clip)")
                self.assertEqual(by_id["pulse-a"].get("opacity"), "1")
                self.assertEqual(by_id["clip-path"].get("d"), "M0 0H40V40Z")
                self.assertFalse(any(node.tag.split("}")[-1] == "animate" for node in svg.iter()))
                self.assertEqual(evidence["opacity_layer"], "pulse-a")

    def test_ambiguous_anchor_missing_layer_and_invalid_source_are_rejected(self):
        for kind in ("numeric", "numeric_exponent", "concatenated"):
            with self.subTest(kind=kind):
                source, module = self.fixture(kind)
                with self.assertRaisesRegex(ValueError, "ambiguous"):
                    self.extract(source + source, module, "base")
                with self.assertRaisesRegex(ValueError, "absent/ambiguous"):
                    self.extract(source, module, "missing")
                with self.assertRaises(ValueError):
                    self.extract(source.replace('const label="设备";', 'const label=;'), module, "base")
                with self.assertRaises(ValueError):
                    self.extract(source + 'const broken=;', module, "base")


class UiTreeTests(unittest.TestCase):
    def test_full_owner_and_utf8_byte_ranges_keep_events_and_conditions(self):
        raw = ('const title="设备";function Panel(p){return p.ready?R.jsx("button",'
               '{disabled:p.busy,onClick:()=>send("ON_SAVE"),children:title}):R.jsx("span",{children:"wait"})}').encode()
        result = evidence.ui_tree(raw)
        self.assertTrue(result["parse_passed"])
        self.assertEqual(len(result["elements"]), 2)
        self.assertEqual([row["action"] for row in result["actions"]], ["ON_SAVE"])
        owner = result["owners"][0]
        self.assertEqual(owner["body"].encode(), raw[owner["start_byte"]:owner["end_byte"]])
        self.assertTrue(owner["body"].startswith("function Panel"))
        button = result["elements"][0]
        self.assertEqual(next(prop for prop in button["properties"] if prop["name"] == "onClick")["body"], '()=>send("ON_SAVE")')
        self.assertEqual(button["conditional_context"][0]["condition"]["body"], "p.ready")

    def test_dynamic_bindings_and_spreads_remain_explicit(self):
        result = evidence.ui_tree(b"function Panel(p){return R.jsx(p.widget,{...p.props,value:p.value})}")
        kinds = {row["kind"] for row in result["elements"][0]["unresolved"]}
        self.assertEqual(kinds, {"component_binding_requires_module_resolution", "spread_or_shorthand_requires_binding_resolution"})

    def test_babel_comma_factory_keeps_render_class_and_callback_owners(self):
        raw = b'class Panel extends React.Component{render(){return items.map(x=>(0,R.jsx)("button",{onClick:()=>save(x)}))}}'
        result = evidence.ui_tree(raw)
        self.assertTrue(result["parse_passed"])
        self.assertEqual(len(result["elements"]), 1)
        element = result["elements"][0]
        self.assertEqual(element["factory"], "(0,R.jsx)")
        owners = {owner["id"]: owner for owner in result["owners"]}
        self.assertEqual([owners[key]["syntax"] for key in element["lexical_owners"]],
                         ["arrow_function", "method_definition", "class_declaration"])
        self.assertEqual(owners[element["lexical_owners"][-1]]["body"].encode(), raw)

    def test_invalid_input_does_not_produce_a_successful_empty_page(self):
        result = evidence.ui_tree(b"function Panel( { return R.jsx(")
        self.assertFalse(result["parse_passed"])
        self.assertTrue(result["failures"])


class PhysicalFactoryTests(unittest.TestCase):
    def factory(self, *, interface=None, dongle=None, mika=False, default_interface=0, clamp=True):
        sys.path.insert(0, str(Path(analysis.__file__).parent / "analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        parser = Parser(Language(tree_sitter_javascript.language()))
        parent = ('class Parent{constructor(e,t){this.name="rzDevice",this.claimInterface=t,'
                  f'void 0===this.claimInterface&&(this.claimInterface={default_interface}),'
                  'this.maxRetryIn=40,this.maxRetryOut=40'
                  + (',this.maxRetryIn>k.IN&&(this.maxRetryIn=k.IN),'
                     'this.maxRetryOut>k.OUT&&(this.maxRetryOut=k.OUT)' if clamp else '') + '}}')
        base = ('class Device extends b.default{constructor(e,t,n="",o=i.A,r=i.B,s=i.C,l=i.IN,c=i.OUT,'
                'u={owner:"",reportId:0}){super(e,t);this.name="rzDevice25",this.reportLength=91}'
                '_getTransactionId(){return 31===this.transactionId&&(this.transactionId=0),this.transactionId++}'
                'sendCommand(){return {reportId:this.reportId}}}')
        raw = ('const modules={55:(e,t,n)=>{var k=n(9);' + parent + '},'
               '77:(e,t,n)=>{var b=n(55),i=n(9);' + base + '},'
               '9:(e,t,n)=>{n.d(t,{A:()=>a,B:()=>b,C:()=>c,IN:()=>d,OUT:()=>e});'
               'const a=5,b=5,c=5,d=10,e=20;}};').encode()
        info = 'info={productId:100'
        if interface is not None:
            info += ',claimInterface:' + str(interface)
        if dongle is not None:
            info += ',dongleId:' + str(dongle)
        info += '}'
        # Inert structural fixtures exercise analysis only; these are not
        # Razer input evidence and are never published as runtime capabilities.
        owners = [{"role": "device_info", "body": info},
                  {"role": "physical_transport_factory", "body":
                   '(e)=>{return DeviceInfo.dongleId?rzDevice25DualLink(e,5*i.A,5*i.B,5*i.C):rzDevice25()}'},
                  {"role": "transport_feature_registration", "body":
                   'f.useFeature("rzDeviceType","rzDevice25",{maxRetryOut:30' + (',isMikaDevice:!0' if mika else '') + '})'}]
        for role, body in (("complete_protocol_base_class", parent), ("complete_protocol_class", base)):
            owners.append({"role": role, "body": body, "byte_start": raw.index(body.encode())})
        return parser, raw, owners

    def query(self, **options):
        parser, raw, owners = self.factory(**options)
        with tempfile.TemporaryDirectory() as temporary, patch.object(analysis, "ROOT", Path(temporary)):
            path = Path(temporary) / "source.js"
            path.write_bytes(raw)
            original = {"path": "source.js", "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
            row = {"product_id": 100, "namespace": "mouse-mw-100-current", "failures": [],
                   "sources": [{"original": original, "owners": owners}], "current_originals": [original]}
            return analysis.feature_physical_transports(row, parser, {"source.js": original})

    def test_wired_omissions_follow_the_selected_parent_and_do_not_create_a_dongle(self):
        branches = self.query()
        self.assertEqual(set(branches), {"100"})
        self.assertEqual(branches["100"]["claim_interface"], 0)
        self.assertEqual(branches["100"]["max_retry_out"], 20)
        with self.assertRaisesRegex(ValueError, "default interface"):
            self.query(default_interface=1)

    def test_mika_and_explicit_options_still_obey_inherited_retry_limits(self):
        branches = self.query(interface=3, dongle=101, mika=True)
        self.assertEqual(branches["100"]["sleep_between_out_ms"], 5)
        self.assertEqual(branches["101"]["sleep_between_out_ms"], 25)
        self.assertEqual(branches["100"]["max_retry_out"], 20)
        self.assertEqual(branches["101"]["max_retry_in"], 10)
        self.assertEqual(branches["101"]["claim_interface"], 3)
        with self.assertRaisesRegex(ValueError, "retry limit"):
            self.query(interface=3, clamp=False)


class DpiPublicationTests(unittest.TestCase):
    def test_missing_current_captures_preserve_every_runtime_product_without_revalidating_them(self):
        import families
        for selected in (None, [80]):
            with self.subTest(selected=selected), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                specs = root / "crates/razer-pages/src/features/mouse/mouse_products_data.json"
                destination = root / "crates/razer-device/src/mouse/mouse_current_dpi_data.json"
                specs.parent.mkdir(parents=True)
                destination.parent.mkdir(parents=True)
                (root / "crates/razer-pages/src/data.rs").write_text(
                    'pub(crate) const MOUSE_PRODUCTS_DATA: &str = include_str!("features/mouse/mouse_products_data.json");\n',
                    encoding="utf-8")
                (root / "crates/razer-device/src/data.rs").write_text(
                    'pub(crate) const MOUSE_CURRENT_DPI_DATA: &str = include_str!("mouse/mouse_current_dpi_data.json");\n',
                    encoding="utf-8")
                specs.write_text(json.dumps([{"product_id": 80}, {"product_id": 83}]), encoding="utf-8")
                original = b'[ {"product_id": 80, "minimum": 100}, {"product_id": 83, "minimum": 200} ]\n'
                destination.write_bytes(original)
                args = argparse.Namespace(family="mouse", function="mw", product=selected, receipt_only=False)
                with patch.object(analysis, "ROOT", root), patch.object(families, "identity", lambda p: {"path": p.name}), redirect_stdout(io.StringIO()):
                    with self.assertRaisesRegex(ValueError, "No current source DPI capabilities verified"):
                        families.audit_mouse_dpi_backend(args)
                self.assertEqual(destination.read_bytes(), original)
                report = json.loads((root / ".analysis/results/frontend/mouse-dpi-backend-audit.json").read_bytes())
                self.assertEqual(report["verified_products"], [])
                self.assertEqual(report["retained_pending_products"], selected or [80, 83])
                self.assertFalse(report["runtime_data_updated"])
                self.assertTrue(all(row["failures"] for row in report["products"]))


class AssetReleaseTests(unittest.TestCase):
    def setUp(self):
        import asset_release
        self.assets = asset_release
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.root_patch = patch.object(asset_release, "ROOT", self.root)
        self.root_patch.start()
        self.source(sorted(self.assets.GIT_ICONS)[0], b"retained icon")
        self.source("assets/synapse/shared/common.svg", b"shared bytes")
        for pid in (1, 2, 3):
            self.source(f"assets/synapse/products/{pid}/image.png", bytes([pid]) * 257)
            self.source(f"assets/synapse/products/{pid}/resources.json", json.dumps({
                "product_id": pid,
                "resources": ([{"physical_path": "assets/synapse/products/2/image.png"}]
                              if pid == 1 else []),
            }).encode())
        self.result = asset_release.prepare("test/assets")
        self.release = self.root / self.result["directory"]

    def tearDown(self):
        self.root_patch.stop()
        self.directory.cleanup()

    def source(self, name, raw):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def test_product_restore_includes_shared_and_cross_product_dependencies(self):
        wanted = "assets/synapse/products/1/image.png"
        dependency = "assets/synapse/products/2/image.png"
        other = "assets/synapse/products/3/image.png"
        shared = "assets/synapse/shared/common.svg"
        for name in (wanted, dependency, other, shared):
            (self.root / name).unlink()
        report = self.assets.restore([1], self.release)
        self.assertEqual(report["groups"], ["product-1", "product-2", "shared"])
        self.assertEqual(report["restored_files"], 3)
        self.assertEqual((self.root / wanted).read_bytes(), b"\x01" * 257)
        self.assertEqual((self.root / dependency).read_bytes(), b"\x02" * 257)
        self.assertEqual((self.root / shared).read_bytes(), b"shared bytes")
        self.assertFalse((self.root / other).exists())
        self.assertEqual(self.assets.restore([1], self.release)["restored_files"], 0)
        with self.assertRaisesRegex(ValueError, "Unknown asset PID"):
            self.assets.restore([99], self.release)

    def test_raw_source_identities_are_verified_and_repeatable(self):
        first = (self.root / self.assets.LOCK).read_bytes()
        again = self.assets.prepare("test/assets")
        self.assertEqual(again["tag"], self.result["tag"])
        self.assertEqual((self.root / self.assets.LOCK).read_bytes(), first)
        self.assertTrue(self.assets.verify(self.release, check_local=True)["passed"])
        self.source("assets/synapse/products/3/image.png", b"changed")
        with self.assertRaisesRegex(ValueError, "Local asset sources differ"):
            self.assets.verify(self.release, check_local=True)
        with self.assertRaisesRegex(ValueError, "Local asset was edited"):
            self.assets.restore([3], self.release)
        self.assets.restore([3], self.release, replace=True)
        self.assertTrue(self.assets.verify(self.release, check_local=True)["passed"])
        self.source(sorted(self.assets.GIT_ICONS)[0], b"different checkout")
        with self.assertRaisesRegex(ValueError, "matching checkout"):
            self.assets.restore([3], self.release)

    def test_later_raw_file_corruption_keeps_all_requested_sources_untouched(self):
        shared = self.root / "assets/synapse/shared/common.svg"
        product = self.root / "assets/synapse/products/3/image.png"
        shared.unlink()
        product.unlink()
        (self.release / "assets/synapse/shared/common.svg").write_bytes(b"corrupt")
        with self.assertRaisesRegex(ValueError, "Snapshot file SHA-256 differs"):
            self.assets.restore(directory=self.release)
        self.assertFalse(shared.exists())
        self.assertFalse(product.exists())

    def test_file_replacement_failure_rolls_back_previous_replacements(self):
        one = self.source("assets/synapse/products/1/image.png", b"user edit one")
        two = self.source("assets/synapse/products/2/image.png", b"user edit two")
        real_replace = os.replace
        def failing_replace(source, target):
            if Path(target) == two and "new" in Path(source).parts:
                raise OSError("injected installation failure")
            return real_replace(source, target)
        with patch.object(self.assets.os, "replace", side_effect=failing_replace):
            with self.assertRaisesRegex(OSError, "injected installation failure"):
                self.assets.restore([1], self.release, replace=True)
        self.assertEqual(one.read_bytes(), b"user edit one")
        self.assertEqual(two.read_bytes(), b"user edit two")

    def test_raw_paths_reject_traversal_windows_aliases_and_manifest_tampering(self):
        for name in ("../outside", "assets/../outside", "assets/C:/outside", "assets/a\\b",
                     "assets/NUL.txt", "assets/a.", "assets/a?b"):
            with self.assertRaisesRegex(ValueError, "Noncanonical"):
                self.assets.canonical(name)
        lock = self.assets.load_lock()
        lock["groups"][0]["files"][0]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "identity differs"):
            self.assets.validate_lock(lock)

    def test_directory_destination_never_moves_or_deletes_user_contents(self):
        target = self.root / "assets/synapse/products/1/image.png"
        target.unlink()
        target.mkdir()
        (target / "user-file").write_bytes(b"keep this")
        with self.assertRaisesRegex(ValueError, "not a regular file"):
            self.assets.restore([1], self.release, replace=True)
        self.assertEqual((target / "user-file").read_bytes(), b"keep this")

    def test_rollback_failure_preserves_backups_and_still_restores_other_files(self):
        one = self.source("assets/synapse/products/1/image.png", b"user edit one")
        two = self.source("assets/synapse/products/2/image.png", b"user edit two")
        real_replace = os.replace
        def failing_replace(source, target):
            if Path(target) == two:
                raise OSError("injected install and recovery failure")
            return real_replace(source, target)
        with patch.object(self.assets.os, "replace", side_effect=failing_replace):
            with self.assertRaisesRegex(RuntimeError, "backups preserved at"):
                self.assets.restore([1], self.release, replace=True)
        self.assertEqual(one.read_bytes(), b"user edit one")
        backups = list((self.root / ".work/asset-recovery").glob("restore-*/assets/synapse/products/2/image.png"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_bytes(), b"user edit two")

    def test_remote_verification_requires_every_lfs_hash_and_no_extra_files(self):
        lock = self.assets.load_lock()
        tree = {r["path"]: {"size": r["bytes"], "lfs": {"oid": r["sha256"], "size": r["bytes"]}}
                for r in self.assets.rows_for(lock)}
        tree.update({name: {} for name in self.assets.META_FILES})
        self.assets.verify_remote_files(lock, tree)
        name = next(iter(tree))
        tree[name]["lfs"]["oid"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "LFS SHA-256 differs"):
            self.assets.verify_remote_files(lock, tree)
        tree["unexpected"] = {}
        with self.assertRaisesRegex(ValueError, "file set differs"):
            self.assets.verify_remote_files(lock, tree)

    def test_immutable_storage_pin_is_not_self_referential(self):
        lock = self.assets.load_lock()
        manifest = self.assets.published_manifest(lock)
        lock["storage"]["revision"] = "a" * 40
        self.assertEqual(self.assets.published_manifest(lock), manifest)
        self.assets.validate_lock(lock)
        lock["storage"]["revision"] = "main"
        with self.assertRaisesRegex(ValueError, "immutable HF commit"):
            self.assets.validate_lock(lock)

    def runtime_sources(self):
        entries = [{"key": "synapse/duplicate.svg", "path": "assets/synapse/shared/common.svg"},
                   {"key": "synapse/other.svg", "path": "assets/synapse/products/2/image.png"}]
        demo = []
        for name in ("play", "pause", "fullscreen", "fullscreen-exit"):
            path = "assets/synapse/products/1/control-" + name + ".svg"
            self.source(path, name.encode())
            demo.append({"key": "synapse/audio-demo-control-" + name + ".svg", "path": path})
        sprite = {"key": "synapse/audio-volume-external-sprite.svg", "path": "assets/synapse/products/1/sprite.svg"}
        self.source(sprite["path"], b'<svg><view id="link" viewBox="0 0 20 20" /><view id="hover" viewBox="0 20 20 20" /></svg>')
        for name in self.assets.REMOTE_FONT_PATHS:
            self.source(name, name.encode())
        selectors = {}
        for table, columns in self.assets.SELECTOR_FIELDS.items():
            row = {column: 1 for column in columns}
            row["key"] = entries[0]["key"]
            if "part" in row: row["part"] = "Product"
            if "kind" in row: row["kind"] = "prd"
            if "wrist" in row:
                row.update(folder_edition_id=128, filename_edition_id=0, wrist=False, density=3)
            selectors[table] = [dict(row), dict(row)]
        self.assets.save_resource_mappings({"schema_version": 1,
            "tables": [{"name": "FIRST", "entries": entries},
                       {"name": "SECOND", "entries": [{"key": entries[0]["key"], "path": "assets/synapse/products/1/image.png"}]}],
            "audio_demo_controls": demo, "audio_volume_sprite": sprite, "selectors": selectors})
        self.assets.prepare("test/assets")

    def test_runtime_metadata_keeps_alias_precedence_list_duplicates_and_sprite_views(self):
        self.runtime_sources()
        value = self.assets.runtime_metadata(check_local=True)
        self.assertEqual(self.assets.runtime_metadata_summary(value)["fonts"], 14)
        duplicate = [row for row in value["entries"] if row["key"] == "synapse/duplicate.svg"]
        self.assertEqual([row["path"] for row in duplicate], [
            "assets/synapse/shared/common.svg", "assets/synapse/products/1/image.png"])
        self.assertNotEqual(duplicate[0]["sha256"], duplicate[1]["sha256"])
        self.assertEqual(value["entries"][1]["view_box"], "0 0 20 20")
        self.assertEqual(value["entries"][2]["view_box"], "0 20 20 20")
        self.assertEqual(value["entries"][0]["sha256"], value["entries"][2]["sha256"])
        self.assertEqual([row["provider"] for row in value["list_entries"]],
                         ["audio_demo_controls"] * 4 + ["audio_volume_sprite"] * 3 + ["tables"] * 3)
        self.assertEqual(value["list_entries"][-3]["key"], value["list_entries"][-1]["key"])
        self.assertEqual([row["path"] for row in value["fonts"]], list(self.assets.REMOTE_FONT_PATHS))
        self.assertIsNone(value["storage"]["revision"])
        self.assertTrue(all(not row["path"].startswith(".analysis/") for row in value["files"]))

    def test_runtime_metadata_verifies_without_resource_payloads_or_analysis(self):
        self.runtime_sources()
        before = self.assets.runtime_metadata(check_local=True)
        lock_before = (self.root / self.assets.LOCK).read_bytes()
        for row in self.assets.rows_for(self.assets.load_lock()):
            (self.root / row["path"]).unlink()
        self.assertEqual(self.assets.runtime_metadata(), before)
        self.assertEqual((self.root / self.assets.LOCK).read_bytes(), lock_before)
        with self.assertRaisesRegex(ValueError, "Runtime source length or SHA-256 differs"):
            self.assets.runtime_metadata(check_local=True)

    def test_runtime_metadata_rejects_unknown_files_and_unparsed_fields(self):
        self.runtime_sources()
        value = self.assets.load_resource_mappings()
        value["tables"][0]["entries"][0]["path"] = "assets/synapse/products/1/absent.svg"
        self.assets.save_resource_mappings(value)
        with self.assertRaisesRegex(ValueError, "absent from the resource lock"):
            self.assets.runtime_metadata()
        value["tables"][0]["entries"][0]["unresolved_function"] = True
        with self.assertRaisesRegex(ValueError, "mapping fields"):
            self.assets.save_resource_mappings(value)

    def test_selector_fields_duplicates_and_folder_filename_editions_roundtrip(self):
        import resource_database
        import sqlite3
        self.runtime_sources()
        value = self.assets.runtime_metadata()
        rows = resource_database.selector_rows(value)
        self.assertEqual(rows["dashboard_plugin_image_selector"], [
            (0, 1, 128, 0, 1, 0, 3, "synapse/duplicate.svg"),
            (1, 1, 128, 0, 1, 0, 3, "synapse/duplicate.svg")])
        schema = Path(__file__).resolve().parent.parent / resource_database.SCHEMA
        with closing(sqlite3.connect(":memory:")) as connection:
            connection.executescript(schema.read_text(encoding="utf-8"))
            resource_database.fill_selectors(connection, rows)
        mappings = self.assets.load_resource_mappings()
        row = mappings["selectors"]["dashboard_plugin_image_selector"][0]
        for key, invalid in (("wrist", 0), ("density", 2), ("product_id", 0),
                             ("folder_edition_id", -1), ("filename_edition_id", 2**32),
                             ("key", "synapse/missing.svg")):
            original = row[key]
            row[key] = invalid
            with self.assertRaises(ValueError, msg=key):
                self.assets.save_resource_mappings(mappings)
            row[key] = original

    def test_compiled_data_paths_and_shared_regeneration_use_owner_tables(self):
        for crate, literal in (("razer-device", "data/shared.json"),
                               ("razer-pages", "features/shared.json")):
            self.source(f"crates/{crate}/src/data.rs", (
                "pub(crate) const SHARED: &str =\n"
                f'    include_str!(\n        "{literal}"\n    );\n').encode())
        self.assertEqual(self.assets.builtin_data_path("razer-device", "SHARED"),
                         self.root / "crates/razer-device/src/data/shared.json")
        targets = self.assets.save_builtin_data("SHARED", {"product_id": 42})
        self.assertEqual(len(targets), 2)
        self.assertTrue(all(json.loads(path.read_bytes()) == {"product_id": 42} for path in targets))
        with self.assertRaisesRegex(ValueError, "missing or repeated"):
            self.assets.builtin_data_path("razer-device", "MISSING")

    def test_compiled_data_table_rejects_path_escape_and_duplicate_bindings(self):
        registry = "crates/razer-device/src/data.rs"
        self.source(registry, b'pub(crate) const SHARED: &str = include_str!("../outside.json");\n')
        with self.assertRaisesRegex(ValueError, "escaped its owning crate"):
            self.assets.builtin_data_path("razer-device", "SHARED")
        self.source(registry, b'pub(crate) const SHARED: &str = include_str!("one.json");\n' * 2)
        with self.assertRaisesRegex(ValueError, "missing or repeated"):
            self.assets.builtin_data_path("razer-device", "SHARED")


class RustValidationTests(unittest.TestCase):
    def test_command_overrides_stay_with_cargo_and_receipt_requires_real_test_totals(self):
        import workspace
        with tempfile.TemporaryDirectory() as directory, patch.object(workspace, "ROOT", Path(directory)), \
                patch.object(workspace, "rust_inputs", return_value=[]), redirect_stdout(io.StringIO()):
            def completed(command, **kwargs):
                kwargs["stdout"].write(b"test result: ok. 1 passed; 0 failed; 0 ignored\n")
                return argparse.Namespace(returncode=0)
            with patch.object(workspace.subprocess, "run", side_effect=completed):
                workspace.validate_rust(["workspace", "shell-save-queue", "mouse-idle-state"],
                                        ["profile.dev.package.ring.debug=0"])
            receipt_path = Path(directory) / ".analysis/validation.json"
            receipt = json.loads(receipt_path.read_bytes())
            self.assertTrue(receipt["passed"])
            self.assertEqual(receipt["commands"][0]["name"], "workspace")
            queue = next(row for row in receipt["commands"] if row["name"] == "shell-save-queue")
            self.assertEqual(queue["argv"][2:4], ["--config", "profile.dev.package.ring.debug=0"])
            self.assertEqual(queue["argv"][-2:], ["--", "--test-threads=1"])
            self.assertTrue(all(row["log_sha256"] for row in receipt["commands"]))
            with patch.object(workspace.subprocess, "run", return_value=argparse.Namespace(returncode=0)):
                with self.assertRaisesRegex(ValueError, "Validation failed"):
                    workspace.validate_rust(["mouse-idle-state"])
            receipt = json.loads(receipt_path.read_bytes())
            self.assertFalse(receipt["passed"])
            self.assertIn("no_tests_executed", receipt["commands"][0]["errors"])


def run():
    suite = unittest.defaultTestLoader.loadTestsFromModule(__import__(__name__))
    for module in ("test_rust_data", "test_typed_tables", "test_windows_installer", "test_workspace",
                   "test_resource_scope", "test_asset_release", "test_host", "test_native_runtime", "test_families"):
        suite.addTests(unittest.defaultTestLoader.loadTestsFromName(module))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    path = analysis.ROOT / ".analysis/results/tool-verification.json"
    report = {"schema_version": 1, "recorded_at_utc": analysis.stamp(), "tests_run": result.testsRun,
              "failures": len(result.failures), "errors": len(result.errors), "passed": result.wasSuccessful(),
              "inputs": [{"path": name, "sha256": analysis.sha(analysis.ROOT / name)}
                         for name in ("tools/evidence.py", "tools/test_evidence.py", "tools/analysis.py",
                                      "tools/host.py", "tools/families.py", "tools/workspace.py",
                                      "tools/asset_release.py", "tools/asset-release.json",
                                      "tools/rust_data.py", "tools/catalog_data.py", "tools/resource_database.py",
                                      "tools/typed_tables.py", "tools/typed-data.json",
                                      "tools/test_workspace.py", "tools/resource_scope.py", "tools/test_resource_scope.py",
                                      "tools/test_asset_release.py", "tools/test_host.py",
                                      "tools/native_runtime.py", "tools/test_native_runtime.py",
                                      "tools/test_families.py",
                                      "tools/windows_installer.py", "tools/test_rust_data.py",
                                      "tools/test_typed_tables.py", "tools/test_windows_installer.py",
                                      "tools/ida/capture.py", "tools/ida/capture_io.py")],
              "target_execution": False, "real_device_operations": False}
    analysis.save(path, report)
    return 0 if result.wasSuccessful() else 1
