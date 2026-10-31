"""Pure tests for source provenance and the connected analysis index."""
import argparse
import ast
from contextlib import closing, redirect_stdout
import hashlib
import io
import inspect
import json
import os
from pathlib import Path
import struct
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
import zipfile
from unittest.mock import MagicMock, patch

import analysis
import evidence
import host
import families
import frontend


class FrontendModuleRecoveryTests(unittest.TestCase):
    def test_capture_passes_every_js_and_mjs_original_to_the_static_analyzer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_dir = root / ".analysis/inputs/frontend/app-current"
            records = []
            for name, raw in (("main.js", b"const main=1"),
                              ("index.mjs", b"export const moduleValue=2"),
                              ("settings.json", b'{"value":3}')):
                target = input_dir / "raw/apps.razer.com/example" / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(raw)
                records.append({"url": "https://apps.razer.com/example/" + name,
                    "path": target.relative_to(root).as_posix(), "bytes": len(raw),
                    "sha256": hashlib.sha256(raw).hexdigest()})
            (input_dir / "acquisition.json").write_text(json.dumps({"files": records}), encoding="utf-8")
            tool = root / "static-analyzer.exe"
            tool.write_bytes(b"inert analyzer identity; never executed")
            args = argparse.Namespace(input=input_dir.relative_to(root).as_posix(), no_refresh_gaps=True)
            with patch.object(analysis, "ROOT", root), patch.object(analysis, "wakaru", return_value=tool), \
                 patch.object(analysis.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run, \
                 patch.object(frontend, "recover_frontend", return_value=[]), redirect_stdout(io.StringIO()):
                analysis.analyze_frontend(args, refresh_ledger=False)
            actual = json.loads((root / ".analysis/results/frontend/app-current/analysis.json").read_bytes())
            self.assertEqual(actual["original_js_inputs"], [row["path"] for row in records[:2]])
            self.assertEqual(actual["original_inputs"], [{key: row[key] for key in ("path", "bytes", "sha256")}
                                                       for row in records])
            self.assertEqual(run.call_count, 1)
            argv = run.call_args.args[0]
            self.assertTrue(any(name.endswith("index.mjs") for name in argv))
            self.assertFalse(any(name.endswith("settings.json") for name in argv))
            self.assertTrue(actual["input_bytes_unchanged"])

    def test_frontend_recovery_always_uses_bounded_thread_pool(self):
        source = inspect.getsource(frontend.recover_frontend)
        self.assertIn("getattr(args, \"workers\", 2)", source)
        self.assertIn("ThreadPoolExecutor(max_workers=workers)", source)
        self.assertNotIn("ProcessPoolExecutor", source)

    def test_frontend_recovery_rejects_serial_or_non_integer_workers(self):
        for workers in (0, 1, -1, 9, True, 2.5, "2"):
            with self.subTest(workers=workers), self.assertRaisesRegex(
                    ValueError, "between two and eight"):
                frontend.recover_frontend(argparse.Namespace(
                    all_acquired=False, input="missing", workers=workers,
                    no_refresh_gaps=True))


    def parse(self, raw):
        return host.HostLocalAudit().parser.parse(raw).root_node

    def test_monitoring_sequence_recovers_complete_factories_and_bootstrap(self):
        raw = (b'!function(){monitor()}(),(self.webpackChunkapp=self.webpackChunkapp||[])'
               b'.push([[7],{101:(m,e,r)=>{r(102);e.value="device"},102:function(m){m.exports=3}}]);')
        segments = frontend.recovery_segments(self.parse(raw))
        self.assertEqual(len(segments), 1)
        self.assertEqual(segments[0]["kind"], "webpack_chunk_registration")
        self.assertTrue(raw[slice(*segments[0]["range"])].startswith(b"(self.webpackChunkapp"))
        self.assertEqual([row["module_id"] for row in segments[0]["factories"]], [101, 102])
        self.assertEqual(segments[0]["factories"][0]["parameters"], ["m", "e", "r"])
        factory = segments[0]["factories"][0]
        self.assertEqual(raw[slice(*factory["body_range"])], b'r(102);e.value="device"')
        bootstrap = (b'!function(){monitor()}(),(()=>{var t={101:(m,e,r)=>{e.value=r(102)}};'
                     b'var queue=self.webpackChunkapp=self.webpackChunkapp||[];queue.push=register})()')
        found = frontend.recovery_segments(self.parse(bootstrap))
        self.assertEqual([row["kind"] for row in found], ["webpack_bootstrap"])
        self.assertEqual(found[0]["factories"][0]["module_id"], 101)

    def test_ordinary_arrays_and_standalone_chunks_are_not_reextracted(self):
        for raw in (b'!function(){}(),items.push([[1],{2:(x)=>{x()}}])',
                    b'(self.webpackChunkapp=self.webpackChunkapp||[]).push([[1],{2:(x)=>{x()}}]);',
                    b'!function(){}(),self.webpackChunkapp.push([[1],{2:"not a factory"}])'):
            with self.subTest(raw=raw):
                self.assertEqual(frontend.recovery_segments(self.parse(raw)), [])

    def test_source_position_counts_unicode_as_utf16_and_normalizes_line_endings(self):
        raw = '前😀缀\r\n二行\u2028三'.encode()
        self.assertEqual(frontend.source_position(raw, len('前😀缀'.encode())), (0, 4))
        self.assertEqual(frontend.source_position(raw, len('前😀缀\r\n二'.encode())), (1, 1))
        self.assertEqual(frontend.source_position(raw, len(raw)), (2, 1))
        with self.assertRaises(UnicodeDecodeError):
            frontend.source_position(raw, 1)




    def test_invalid_mapping_and_ambiguous_module_tables_fail(self):
        for segment in ("!", "g", "gggggggggggggg"):
            with self.subTest(segment=segment), self.assertRaises(ValueError):
                frontend.decode_vlq(segment)
        duplicate = b'!function(){}(),self.webpackChunkapp.push([[1],{2:(x)=>{x()},2:(x)=>{x()}}])'
        with self.assertRaisesRegex(ValueError, "duplicate module"):
            frontend.recovery_segments(self.parse(duplicate))

    def test_factory_maps_preserve_cross_line_coordinates_names_and_unmapped_wrapper(self):
        raw = b'prefix\n(m,e)=>{\n e.value=7;\n}\nnext'
        factory = {"module_id": 7, "factory_range": [7, raw.index(b'}') + 1], "syntax": "arrow_function"}
        document = {"version": 3, "sources": ["bundle.js"], "names": ["factory", "value"],
                    "mappings": ";".join(frontend.encode_vlq(row) for row in
                        ([0, 0, 9, 0], [0, 0, 1, 20, 0], [1, 0, 1, -18, 1], [0, 0, 1, -2]))}
        result = frontend.factory_source_maps(document, raw, [factory], ["../official.js"])[7]
        actual = [[frontend.decode_vlq(value) for value in line.split(",")] for line in result["mappings"].split(";")]
        self.assertEqual(actual, [[[0], [1, 0, 10, 20, 0]], [[1, 0, 1, -18, 1]], [[0, 0, 1, -2]]])
        self.assertEqual(result["sources"], ["../official.js"])
        self.assertEqual(result["names"], ["factory", "value"])

    def test_adjacent_factories_reset_original_and_name_deltas_and_use_utf16_columns(self):
        raw = '😀 a=()=>{}, b=()=>{};'.encode()
        first = raw.index(b'()=>{}'); second = raw.index(b'()=>{}', first + 1)
        factories = [{"module_id": index, "factory_range": [start, start + 6], "syntax": "arrow_function"}
                     for index, start in ((1, first), (2, second))]
        document = {"version": 3, "sources": ["source"], "names": ["a", "outside", "first", "second"],
                    "mappings": ",".join(frontend.encode_vlq(row) for row in
                        ([0, 0, 3, 40, 1], [5, 0, 0, 10, 1], [5, 0, 0, 5, 0], [5, 0, 0, 45, 1]))}
        result = frontend.factory_source_maps(document, raw, factories, ["official.js"])
        self.assertEqual([frontend.decode_vlq(row) for row in result[1]["mappings"].split(",")],
                         [[0], [1, 0, 3, 50, 2], [5, 0, 0, 5, 0]])
        self.assertEqual([frontend.decode_vlq(row) for row in result[2]["mappings"].split(",")],
                         [[0], [1, 0, 3, 100, 3]])

    def test_factory_map_rejects_bad_segments_foreign_sources_names_and_overlap(self):
        factory = {"module_id": 1, "factory_range": [0, 6], "syntax": "arrow_function"}
        for mappings in ("AA", frontend.encode_vlq([0, 2, 0, 0]),
                         frontend.encode_vlq([0, 0, -1, 0]), frontend.encode_vlq([0, 0, 0, 0, 0]), ""):
            with self.subTest(mappings=mappings), self.assertRaises(ValueError):
                frontend.factory_source_maps({"version": 3, "sources": ["a"], "mappings": mappings},
                                             b'()=>{}', [factory], ["a"])
        with self.assertRaisesRegex(ValueError, "overlap"):
            frontend.factory_source_maps({"version": 3, "sources": ["a"], "mappings": "AAAA"},
                                         b'()=>{}', [factory, {**factory, "module_id": 2}], ["a"])

    def test_complete_capture_is_reused_and_changed_original_is_rejected(self):
        audit = host.HostLocalAudit()
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            input_dir = root / '.analysis/inputs/frontend/fixture'
            source = input_dir / 'raw/app/main.js'
            source.parent.mkdir(parents=True)
            raw = b'!function(){monitor()}(),self.webpackChunkui.push([[1],{2:(m,e)=>{e.value=7}}]);'
            source.write_bytes(raw)
            def identity(path):
                body = path.read_bytes()
                return {"path": path.relative_to(root).as_posix(), "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest()}
            original = identity(source)
            (input_dir/'acquisition.json').write_text(json.dumps({"files": [original]}))
            output = root/'.analysis/results/frontend/fixture'
            readable = output/'modules/app/main.js'
            readable.parent.mkdir(parents=True)
            readable.write_bytes(raw)
            start = raw.index(b'(m,e)=>')
            map_path = readable.with_name(readable.name+'.map')
            map_path.write_text(json.dumps({"version": 3, "sources": ['../../../../../inputs/frontend/fixture/raw/app/main.js'],
                                           "names": [], "mappings": frontend.encode_vlq([start, 0, 0, start])}))
            provenance = output/'modules/provenance.json'
            provenance.write_text(json.dumps({"format": "webpack5", "strategy": "structural", "modules": {
                "app/main.js": {"input": "app/main.js", "ranges": [[0, len(raw)]], "extraction": "structural"}}}))
            tool = root/'tools/analyzer.exe'; tool.parent.mkdir(); tool.write_bytes(b'verified-static-analyzer')
            capture = output/'analysis.json'
            capture.write_text(json.dumps({"input": input_dir.relative_to(root).as_posix(),
                "tool": {"sha256": identity(tool)["sha256"]}, "original_inputs": [original],
                "original_js_inputs": [original["path"]], "commands": [],
                "outputs": [identity(p) for p in (readable, map_path, provenance)]}))
            with patch('frontend.subprocess.run', side_effect=AssertionError('Complete readable captures must be reused')):
                result = frontend.recover_one(input_dir, tool, audit)
            self.assertEqual((result['registered_factories'], result['recovered_modules'], result['missing_factories']), (1, 1, 0))
            restored = json.loads(capture.read_bytes())
            name = restored['module_recovery']['factories'][0]['output']
            self.assertEqual((output/'modules'/name).read_bytes(), b'((m,e)=>{e.value=7});\n')
            self.assertEqual(source.read_bytes(), raw)
            current_audit = host.HostLocalAudit()
            self.assertEqual(frontend.recover_one(input_dir, tool, current_audit)['status'], 'reused_verified_recovery')
            source.write_bytes(raw.replace(b'value=7', b'value=8'))
            with self.assertRaisesRegex(ValueError, 'identity differs'):
                frontend.recover_one(input_dir, tool, host.HostLocalAudit())


class BatchPythonConcurrencyTests(unittest.TestCase):
    def test_typed_profile_generation_always_uses_thread_pool(self):
        source = (Path(__file__).with_name("typed_tables.py")).read_text(encoding="utf-8")
        self.assertIn("ThreadPoolExecutor(max_workers=pool_workers)", source)
        self.assertIn("workers=4", source)
        self.assertNotIn("ProcessPoolExecutor", source)
        self.assertNotIn("for item in items:\n            product, source_inputs = _capture_profile_transfer_product(item)", source)

    def test_dashboard_and_family_acquisition_require_multiple_workers(self):
        workspace = (Path(__file__).with_name("workspace.py")).read_text(encoding="utf-8")
        families_source = (Path(__file__).with_name("families.py")).read_text(encoding="utf-8")
        resource_scope_source = (Path(__file__).with_name("resource_scope.py")).read_text(encoding="utf-8")
        installer_source = (Path(__file__).with_name("windows_installer.py")).read_text(encoding="utf-8")
        self.assertIn("not 2 <= args.workers <= 12", workspace)
        self.assertIn('choices=range(2, 17)', families_source)
        self.assertIn("Native package workers must be between two and sixteen", families_source)
        self.assertIn("ThreadPoolExecutor(max_workers=workers)", resource_scope_source)
        self.assertIn("choices=range(2, 17)", installer_source)
        self.assertIn("ThreadPoolExecutor(max_workers=workers)", installer_source)

    def test_host_and_typed_batch_entries_require_bounded_parallelism(self):
        host_source = (Path(__file__).with_name("host.py")).read_text(encoding="utf-8")
        analysis_source = (Path(__file__).with_name("analysis.py")).read_text(encoding="utf-8")
        typed_source = (Path(__file__).with_name("typed_tables.py")).read_text(encoding="utf-8")
        producer_source = (Path(__file__).with_name("mapping_producers.py")).read_text(encoding="utf-8")
        for source in (host_source, analysis_source, typed_source, producer_source):
            self.assertIn("ThreadPoolExecutor", source)
            self.assertNotIn("ProcessPoolExecutor", source)
        self.assertIn("Host inspection workers must be between two and sixteen", host_source)
        self.assertIn("Managed host analysis workers must be between two and sixteen", host_source)
        self.assertIn("Frontend analysis workers must be between two and eight", analysis_source)
        self.assertIn("Local protocol table workers must be between two and sixteen", typed_source)
        self.assertIn("Mapping capability workers must be between two and sixteen", typed_source)
        self.assertIn("Mapping producer workers must be between two and sixteen", producer_source)

    def test_family_product_audits_use_stable_threaded_batches(self):
        source = (Path(__file__).with_name("families.py")).read_text(encoding="utf-8")
        self.assertGreaterEqual(source.count("ThreadPoolExecutor(max_workers=max(2, min(workers, len(jobs))))"), 3)
        self.assertIn("def audit_one(spec):", source)
        self.assertIn("def audit_one(surface):", source)
        self.assertIn("local_geometry_index", source)
        self.assertIn("parser_bodies.update(local_parser_bodies)", source)

    def test_family_product_audits_reject_serial_workers_before_io(self):
        for entry, function in (("mouse-dpi", families.audit_mouse_dpi),
                                ("mouse-customize", families.audit_mouse_customize),
                                ("keyboard-artwork", families.audit_keyboard_artwork)):
            for workers in (0, 1, True, 2.5, "2", 17):
                with self.subTest(entry=entry, workers=workers), self.assertRaisesRegex(
                        ValueError, "Family audit workers"):
                    function(argparse.Namespace(workers=workers, product=[]))

    def test_host_and_typed_batch_entries_reject_serial_workers_before_io(self):
        typed_tables = __import__("typed_tables")
        mapping_producers = __import__("mapping_producers")
        for workers in (0, 1, True, 2.5, "2", 17):
            with self.subTest(entry="inspect-host", workers=workers), self.assertRaisesRegex(
                    ValueError, "Host inspection workers"):
                host.inspect_host(argparse.Namespace(workers=workers))
            with self.subTest(entry="analyze-host", workers=workers), self.assertRaisesRegex(
                    ValueError, "Managed host analysis workers"):
                host.analyze_host(argparse.Namespace(workers=workers, managed_scope="all"))
            with self.subTest(entry="resolve-managed-dependencies", workers=workers), self.assertRaisesRegex(
                    ValueError, "Managed dependency workers"):
                host.resolve_managed_dependencies(workers=workers)
            with self.subTest(entry="protocol-tables", workers=workers), self.assertRaisesRegex(
                    ValueError, "Local protocol table workers"):
                typed_tables.prepare_local_protocol_tables(workers=workers)
            with self.subTest(entry="mapping-capabilities", workers=workers), self.assertRaisesRegex(
                    ValueError, "Mapping capability workers"):
                typed_tables.prepare_mapping_capabilities(workers=workers)
            with self.subTest(entry="mapping-producers", workers=workers), self.assertRaisesRegex(
                    ValueError, "Mapping producer workers"):
                mapping_producers.prepare(workers=workers)

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
            "acquisition_query_reference_unresolved", "analyzer_query_failure", "analyzer_query_failure",
            "semantic_query_chain_unresolved"])
        self.assertTrue(all(row["product_id"] == 740 for row in rows))
        self.assertEqual(rows[3]["finding"]["stage"], "hexrays")
        self.assertTrue(rows[3]["pointer"].endswith("/native/failures"))

    def test_historical_decompiler_failures_keep_their_actual_capture_owner(self):
        native = {"stage": "30888:hexrays", "error": "call analysis failed", "traceback": "complete error"}
        supplement = {"rva": "0x30888", "stage": "hexrays", "error": "complete traceback"}
        data = {"product_id": 190, "capture_states": {
            "base": {"failures": [native]}, "previous_failed_supplement": {"failures": [supplement]}}}
        rows = analysis.semantic_query_gaps(data, "current.json")
        self.assertEqual([row["kind"] for row in rows], ["analyzer_query_failure"] * 2)
        self.assertEqual([row["finding"] for row in rows], [native, supplement])
        self.assertEqual([row["pointer"] for row in rows],
                         ["/capture_states/base/failures", "/capture_states/previous_failed_supplement/failures"])
        self.assertTrue(all(row["product_id"] == 190 and row["evidence"] == "current.json" for row in rows))

    def test_unknown_failures_and_semantic_conditions_are_not_reclassified_from_error_words(self):
        findings = ["Hex-Rays result consumer is missing", {"stage": "contract", "error": "hexrays"},
            {"stage": "not-hexrays", "error": "call analysis failed"},
            {"stage": "30888:hexrays"}, {"stage": "hexrays", "error": ""},
            {"stage": 3, "error": "hexrays"}]
        rows = analysis.semantic_query_gaps({"failures": findings,
            "unresolved_chains": ["Original hexrays failure still needs a result consumer"]}, "audit.json")
        self.assertEqual([row["kind"] for row in rows],
                         ["semantic_query_failure"] * len(findings) + ["semantic_query_chain_unresolved"])
        self.assertEqual([row["finding"] for row in rows[:len(findings)]], findings)

    def test_nested_products_and_json_pointer_escaping_keep_the_actual_owner(self):
        rows = analysis.semantic_query_gaps({"products": [{"product_id": 691,
            "child/with~name": {"failures": ["missing getter"], "products": [{"product_id": 746,
                "remaining_local_chains": ["serial storage"]}]},
            "irrelevant": {"failures": "unverified"}}]}, "receipt.json")
        self.assertEqual(len(rows), 2)
        self.assertEqual([row["product_id"] for row in rows], [691, 746])
        self.assertEqual(rows[0]["pointer"], "/products/0/child~1with~0name/failures")
        self.assertTrue(all(row["evidence"] == "receipt.json" for row in rows))

    def test_mapping_remaining_closure_preserves_every_finding_and_its_owner_pointer(self):
        data = {"keypad_mapping": {"remaining_closure": ["setter reply", {"event": "profile changed"}]},
                "mapping_descriptors": {"native/with~branch": {"remaining_closure": ["save confirmation"]}}}
        rows = analysis.semantic_query_gaps(data, "receipt.json")
        self.assertEqual([row["finding"] for row in rows],
                         ["setter reply", {"event": "profile changed"}, "save confirmation"])
        self.assertEqual([row["pointer"] for row in rows], ["/keypad_mapping/remaining_closure",
            "/keypad_mapping/remaining_closure", "/mapping_descriptors/native~1with~0branch/remaining_closure"])
        self.assertTrue(all(row["kind"] == "semantic_query_chain_unresolved"
                            and row["evidence"] == "receipt.json" for row in rows))
        self.assertEqual(analysis.semantic_query_gaps({"keypad_mapping": {"remaining_closure": []},
            "mapping_descriptors": {"remaining_closure": []}}, "receipt.json"), [])

    def test_host_queries_reach_the_shared_ledger_without_promoting_static_capture(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            receipt = root / ".analysis/results/host-current/local-audit.json"
            receipt.parent.mkdir(parents=True)
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [], "missing_current_roots": []}))
            ledger_path = root / ".analysis/results/gaps.json"
            ledger_path.write_text(json.dumps({"owners": {"manual": [{"kind": "unrelated_gap"}]}}))
            report = {"window/lifecycle": {"failures": [], "snapshot_stable": True,
                "source_chain_complete": False, "remaining_chains": ["native worker join"],
                "runtime_acceptance": "unverified"},
                "macro": {"native": {"failures": ["indirect ABI unresolved"]}}}
            receipt.write_text(json.dumps(report))
            name = ".analysis/results/host-current/local-audit.json"
            ledger = analysis.refresh_gaps()
            rows = ledger["owners"][name]
            self.assertIn({"kind": "unrelated_gap"}, ledger["owners"]["manual"])
            self.assertTrue(any(row.get("finding") == "native worker join"
                                and row["pointer"] == "/window~1lifecycle/remaining_chains" for row in rows))
            self.assertTrue(any(row.get("finding") == "indirect ABI unresolved" for row in rows))
            self.assertTrue(any(row["kind"] == "host_local_source_chain_pending" for row in rows))
            report["window/lifecycle"]["remaining_chains"] = []
            report["macro"]["native"]["failures"] = []
            receipt.write_text(json.dumps(report))
            rows = analysis.refresh_gaps()["owners"][name]
            self.assertFalse(any(row.get("finding") for row in rows))
            self.assertTrue(any(row["kind"] == "host_implementation_acceptance_pending" for row in rows))

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

    def instruction_manual(self):
        manual = self.source(".analysis/inputs/platform-abi-current/test/manual.pdf", b"%PDF-1.7\nsynthetic static fixture")
        url = "https://www.intel.com/content/dam/www/public/us/en/documents/manuals/test.pdf"
        member = {"path": analysis.rel(manual), "bytes": manual.stat().st_size, "sha256": analysis.sha(manual),
                  "url": url, "final_url": url, "http_status": 200, "content_type": "application/pdf",
                  "document_number": "synthetic", "revision": "1", "publication_date": "synthetic", "volume": 3, "pdf_pages": 1}
        record = {"schema_version": 1, "target_execution": False, "files": [member]}
        receipt = self.source(".analysis/inputs/platform-abi-current/test/acquisition.json", json.dumps(record).encode())
        index = self.source(".analysis/inputs.json", b'{"inputs": []}')
        return manual, receipt, record, index

    def test_instruction_manual_publication_retains_revision_complete_bytes_and_static_boundary(self):
        manual, receipt, record, index = self.instruction_manual()
        with patch.object(analysis.urllib.request, "urlopen", side_effect=AssertionError("Static publication must not download")):
            result = analysis.publish_instruction_manual(receipt)
        self.assertEqual(result["documents"], 1)
        published = json.loads(index.read_bytes())
        self.assertEqual(len(published["inputs"]), 2)
        item = next(row for row in published["inputs"] if row["path"] == analysis.rel(manual))
        self.assertEqual(item["sha256"], analysis.sha(manual))
        self.assertEqual(item["revision"], record["files"][0]["revision"])
        self.assertFalse(published["source_publications"][0]["target_execution"])
        before = index.read_bytes()
        analysis.publish_instruction_manual(receipt)
        self.assertEqual(index.read_bytes(), before)
        manual.write_bytes(b"%PDF-1.7\nchanged")
        with self.assertRaisesRegex(ValueError, "byte identity differs"):
            analysis.publish_instruction_manual(receipt)
        self.assertEqual(index.read_bytes(), before)

    def test_instruction_manual_publication_rejects_changed_origin_metadata_and_paths(self):
        _, receipt, record, index = self.instruction_manual()
        original = json.dumps(record)
        mutations = [lambda row: row.update(final_url="https://untrusted.invalid/manual.pdf"),
                     lambda row: row.update(http_status=206), lambda row: row.update(revision=""),
                     lambda row: row.update(pdf_pages=True),
                     lambda row: row.update(path=".analysis/inputs/platform-abi-current/other/manual.pdf")]
        before = index.read_bytes()
        for change in mutations:
            altered = json.loads(original)
            change(altered["files"][0])
            receipt.write_text(json.dumps(altered), encoding="utf-8")
            with self.subTest(member=altered["files"][0]), self.assertRaises(ValueError):
                analysis.publish_instruction_manual(receipt)
            self.assertEqual(index.read_bytes(), before)
        altered = json.loads(original)
        altered["target_execution"] = True
        receipt.write_text(json.dumps(altered), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "static acquisition"):
            analysis.publish_instruction_manual(receipt)
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


class NativeCaptureCapabilityTests(unittest.TestCase):
    setUp = EvidenceTests.setUp
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source

    def capture_fixture(self, name, *, available=False, architecture="ia64", processor="ia64l", functions=3):
        from datetime import datetime, timezone
        import traceback

        source = Path(__file__).with_name("ida") / "capture.py"
        support_source = source.with_name("capture_io.py")
        runtime_tool = self.source("tools/ida/capture.py", source.read_bytes())
        self.source("tools/ida/capture_io.py", support_source.read_bytes())
        values_source = self.source("tools/ida/capture_values.py", source.with_name("capture_values.py").read_bytes())
        typed_values = {"__file__": str(values_source)}
        exec(compile(values_source.read_text(encoding="utf-8"), str(values_source), "exec"), typed_values)
        raw = b"inert fake-IDA input: " + name.encode()
        digest = hashlib.sha256(raw).hexdigest()
        original = self.source(".analysis/inputs/official-native-current/test/" + name + ".sys", raw)
        private = self.source(".work/native/" + digest + "/batch/driver.sys", raw)
        database = self.source(".work/native/" + digest + "/batch/driver.sys.i64", b"inert database fixture")
        output = self.root / ".analysis/results/native" / digest
        plan = {"input": original.relative_to(self.root).as_posix(), "sha256": digest,
                "output": output.relative_to(self.root).as_posix(), "loader": {"architecture": architecture},
                "database_resume": {"input": database.relative_to(self.root).as_posix()}}
        plan_path = self.source(".work/native/" + digest + "/plan.json", json.dumps(plan).encode())
        base = 0x10000
        addresses = [base + 0x1000 + index * 0x10 for index in range(functions)]

        class Strings:
            def setup(self, **options):
                pass

            def __iter__(self):
                return iter(())

        def decompile(ea, failure):
            return SimpleNamespace(entry_ea=ea, get_lvars=lambda: [],
                                   mba=SimpleNamespace(entry_ea=ea), get_stkoff_delta=lambda: 0,
                                   get_pseudocode=lambda: [SimpleNamespace(line="return;")])

        sdk = SimpleNamespace(
            init_hexrays_plugin=MagicMock(return_value=available),
            get_hexrays_version=MagicMock(return_value="synthetic-decompiler"),
            hexrays_failure_t=MagicMock(return_value=SimpleNamespace(
                desc=lambda: "synthetic call analysis failed", code=-12, errea=base + 0x1000)),
            decompile=MagicMock(side_effect=decompile), clear_cached_cfuncs=MagicMock())
        if not available:
            for api in (sdk.get_hexrays_version, sdk.hexrays_failure_t, sdk.decompile, sdk.clear_cached_cfuncs):
                api.side_effect = AssertionError("Unavailable decompiler API must not be called")
        support = {}
        exec(compile(support_source.read_text(encoding="utf-8"), str(support_source), "exec"), support)
        namespace = {
            "__file__": str(runtime_tool), "Path": Path, "hashlib": hashlib, "json": json,
            "datetime": datetime, "timezone": timezone, "traceback": traceback, "sys": sys,
            "capture_values": SimpleNamespace(**{key: typed_values[key] for key in ("source_identity", "local_variables")}),
            "os": SimpleNamespace(environ={"RAZER_IDA_CAPTURE_PLAN": str(plan_path)}, sep=os.sep),
            **{name: support[name] for name in ("file_sha256", "write_json_array", "write_function_aggregates")},
            "idaapi": SimpleNamespace(get_input_file_path=lambda: str(private), get_imagebase=lambda: base),
            "ida_auto": SimpleNamespace(auto_wait=MagicMock()),
            "ida_ida": SimpleNamespace(validate_idb=MagicMock(return_value=0), VLD_SILENT=0,
                                       inf_get_procname=MagicMock(return_value=processor)),
            "ida_kernwin": SimpleNamespace(get_kernel_version=lambda: "synthetic-ida"), "ida_hexrays": sdk,
            "ida_idp": SimpleNamespace(),
            "ida_loader": SimpleNamespace(get_path=lambda kind: str(database), PATH_TYPE_IDB=0, DBFL_COMP=0,
                                          save_database=MagicMock(return_value=True)),
            "ida_nalt": SimpleNamespace(retrieve_input_file_sha256=lambda: bytes.fromhex(digest),
                                        get_import_module_qty=lambda: 0),
            "ida_typeinf": SimpleNamespace(get_idati=lambda: None, get_ordinal_limit=lambda til: 1),
            "ida_funcs": SimpleNamespace(get_func=lambda ea: SimpleNamespace(end_ea=ea + 1, flags=0),
                                         get_func_name=lambda ea: "function_" + hex(ea),
                                         get_func_cmt=lambda func, repeatable: None),
            "ida_bytes": SimpleNamespace(get_bytes=lambda ea, size: b"\xc3" * size, get_item_size=lambda ea: 1),
            "ida_lines": SimpleNamespace(tag_remove=lambda text: text),
            "ida_gdl": SimpleNamespace(FlowChart=lambda func: []),
            "idc": SimpleNamespace(get_type=lambda ea: None, generate_disasm_line=lambda ea, flags: "retn",
                                   get_cmt=lambda ea, repeatable: None, STRTYPE_C=0, STRTYPE_C_16=1),
            "idautils": SimpleNamespace(Functions=lambda: addresses, Segments=lambda: [], Strings=Strings,
                                        Entries=lambda: [], Names=lambda: [], Structs=lambda: [],
                                        Chunks=lambda ea: [(ea, ea + 1)], FuncItems=lambda ea: [ea],
                                        XrefsTo=lambda ea, flags: [], XrefsFrom=lambda ea, flags: []),
        }
        definitions = [node for node in ast.parse(source.read_text(encoding="utf-8")).body
                       if isinstance(node, ast.FunctionDef) and node.name in {"write_json", "capture"}]
        exec(compile(ast.Module(body=definitions, type_ignores=[]), str(source), "exec"), namespace)

        def run_capture():
            with redirect_stdout(io.StringIO()):
                return namespace["capture"]()

        return {"run": run_capture, "sdk": sdk, "decompile": decompile, "output": output,
                "original": original, "private": private, "raw": raw, "digest": digest}

    def assert_unavailable(self, fixture, manifest, count=3):
        self.assertTrue(manifest["capture_complete"] and manifest["functions_complete"])
        self.assertFalse(manifest["all_pseudocode_captured"])
        self.assertIsNone(manifest["hexrays_version"])
        self.assertEqual(len(manifest["functions"]), count)
        self.assertEqual(len(manifest["failures"]), count)
        self.assertEqual({row["stage"] for row in manifest["failures"]},
                         {row["rva"][2:] + ":hexrays" for row in manifest["functions"]})
        for failure in manifest["failures"]:
            self.assertEqual(failure["reason"], "decompiler_unavailable")
            self.assertFalse(failure["decompilation_attempted"])
            self.assertIn("remains unverified", failure["error"])
            self.assertNotIn("traceback", failure)
        for row in manifest["functions"]:
            self.assertTrue(row["assembly_captured"])
            self.assertFalse(row["pseudocode_captured"])
            self.assertEqual(row["pseudocode_status"], "unverified")
            body = json.loads((fixture["output"] / row["path"]).read_bytes())
            self.assertIsNone(body["pseudocode"])
            self.assertEqual(body["assembly"][0]["bytes"], "c3")
        for api in (fixture["sdk"].get_hexrays_version, fixture["sdk"].hexrays_failure_t,
                    fixture["sdk"].decompile, fixture["sdk"].clear_cached_cfuncs):
            api.assert_not_called()
        self.assertEqual(fixture["original"].read_bytes(), fixture["raw"])
        self.assertEqual(fixture["private"].read_bytes(), fixture["raw"])

    def test_unavailable_decompiler_keeps_every_function_unverified_and_static_outputs_complete(self):
        fixture = self.capture_fixture("unavailable")
        manifest = fixture["run"]()
        self.assertEqual((manifest["architecture"], manifest["processor"]), ("ia64", "ia64l"))
        self.assert_unavailable(fixture, manifest)
        verified, _ = analysis.verify_native_capture(fixture["output"],
            {"sha256": fixture["digest"], "bytes": len(fixture["raw"])})
        self.assertEqual(verified["failures"], manifest["failures"])

    def test_matching_unavailable_capture_reuses_assembly_without_dropping_unverified_functions(self):
        fixture = self.capture_fixture("reuse-unavailable")
        first = fixture["run"]()
        second = fixture["run"]()
        self.assertTrue(second["previous_capture"]["reuse_allowed"])
        self.assertEqual(second["retained_functions_reused"], 3)
        self.assertEqual(first["functions"], second["functions"])
        self.assert_unavailable(fixture, second)

    def test_resume_rejects_changed_or_missing_architecture_processor_and_availability(self):
        changes = [("architecture", "x86"), ("processor", "metapc"), ("hexrays_available", True),
                   ("hexrays_available", 0),
                   ("architecture", None), ("processor", None), ("hexrays_available", None)]
        for index, (field, value) in enumerate(changes):
            with self.subTest(field=field, value=value):
                fixture = self.capture_fixture("identity-" + str(index))
                first = fixture["run"]()
                path = fixture["output"] / "capture.json"
                previous = json.loads(path.read_bytes())
                if value is None:
                    previous.pop(field)
                else:
                    previous[field] = value
                path.write_text(json.dumps(previous), encoding="utf-8")
                current = fixture["run"]()
                self.assertFalse(current["previous_capture"]["reuse_allowed"])
                self.assertEqual(current["retained_functions_reused"], 0)
                self.assertEqual(current["functions_total"], first["functions_total"])
                self.assert_unavailable(fixture, current)

    def test_gaining_and_losing_decompiler_capability_never_reuses_stale_pseudocode(self):
        fixture = self.capture_fixture("capability-transition", architecture="x64", processor="metapc")
        self.assert_unavailable(fixture, fixture["run"]())
        sdk = fixture["sdk"]
        sdk.init_hexrays_plugin.return_value = True
        sdk.get_hexrays_version.side_effect = None
        sdk.hexrays_failure_t.side_effect = None
        sdk.decompile.side_effect = fixture["decompile"]
        sdk.clear_cached_cfuncs.side_effect = None
        captured = fixture["run"]()
        self.assertFalse(captured["previous_capture"]["reuse_allowed"])
        self.assertEqual(captured["retained_functions_reused"], 0)
        self.assertTrue(captured["all_pseudocode_captured"])
        self.assertFalse(captured["failures"])
        self.assertEqual(sdk.decompile.call_count, 3)
        self.assertEqual(sdk.clear_cached_cfuncs.call_count, 3)
        sdk.init_hexrays_plugin.return_value = False
        for api in (sdk.get_hexrays_version, sdk.hexrays_failure_t, sdk.decompile, sdk.clear_cached_cfuncs):
            api.reset_mock()
            api.side_effect = AssertionError("Unavailable decompiler API must not be called")
        unavailable = fixture["run"]()
        self.assertFalse(unavailable["previous_capture"]["reuse_allowed"])
        self.assertEqual(unavailable["retained_functions_reused"], 0)
        self.assert_unavailable(fixture, unavailable)

    def test_available_decompiler_preserves_actual_decompilation_failure_details(self):
        fixture = self.capture_fixture("decompilation-failure", available=True,
                                       architecture="x86", processor="metapc", functions=1)
        fixture["sdk"].decompile.side_effect = None
        fixture["sdk"].decompile.return_value = None
        manifest = fixture["run"]()
        self.assertTrue(manifest["hexrays_available"] and manifest["capture_complete"])
        self.assertFalse(manifest["all_pseudocode_captured"])
        self.assertEqual(manifest["functions"][0]["pseudocode_status"], "failed")
        self.assertEqual(manifest["functions"][0]["new_failures"], 1)
        self.assertEqual(len(manifest["failures"]), 1)
        failure = manifest["failures"][0]
        self.assertIn("synthetic call analysis failed (code -12, address 0x11000)", failure["error"])
        self.assertIn("traceback", failure)
        self.assertNotIn("reason", failure)
        fixture["sdk"].get_hexrays_version.assert_called_once()
        fixture["sdk"].clear_cached_cfuncs.assert_called_once()


class NativeExceptionRegionCaptureTests(unittest.TestCase):
    setUp = EvidenceTests.setUp
    tearDown = EvidenceTests.tearDown
    source = EvidenceTests.source

    def fixture(self):
        from datetime import datetime, timezone
        from test_host import Ia64ExceptionRegionTests

        raw, _ = Ia64ExceptionRegionTests.fixture()
        raw = bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        native = ".analysis/results/native/" + digest
        original = self.source(".analysis/inputs/official-native-current/test/ia64.sys", raw)
        tool_source = Path(__file__).with_name("ida") / "capture.py"
        tool = self.source("tools/ida/capture.py", tool_source.read_bytes())
        parser = self.source("tools/host.py", Path(host.__file__).read_bytes())
        entry = self.source("tools/ida/ida-start-mcp.py", tool_source.with_name("ida-start-mcp.py").read_bytes())
        support = self.source("tools/ida/capture_io.py", tool_source.with_name("capture_io.py").read_bytes())
        source_database = self.source(".work/native/" + digest + "/batch/ia64.sys.i64", b"inert immutable base IDB")
        database = self.source(".work/native/" + digest + "/exception-regions/private.i64", source_database.read_bytes())
        base_capture = self.source(native + "/capture.json", json.dumps(
            {"capture_complete": True, "input_sha256": digest, "functions": []}).encode())
        def identity(path):
            return {"path": path.relative_to(self.root).as_posix(), "bytes": path.stat().st_size,
                    "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
        installation = self.root / ".work/ida-fixture"
        def retained(path, *, installed=False):
            row = identity(path)
            if installed:
                row.pop("path")
                row["installation_path"] = path.relative_to(installation).as_posix()
            snapshot = self.source(native + "/tool-sources/" + row["sha256"] + path.suffix, path.read_bytes())
            return {**row, "snapshot": identity(snapshot)}
        tool_sources = {name: retained(path) for name, path in {
            "capture": tool, "parser": parser, "entry": entry, "support": support}.items()}
        tool_sources["analyzer"] = retained(self.source(".work/ida-fixture/idat.exe", b"inert analyzer"), installed=True)
        for name, relative in host.IA64_EXCEPTION_ANALYZER_FILES.items():
            tool_sources[name] = retained(self.source(".work/ida-fixture/" + relative, ("inert " + name).encode()), installed=True)
        sdk = {}
        tool_sources["sdk"] = {}
        for name in host.IA64_EXCEPTION_SDK_MODULES:
            path = self.source(".work/ida-fixture/python/" + name + ".py", ("inert " + name).encode())
            tool_sources["sdk"][name] = retained(path, installed=True)
            sdk[name] = SimpleNamespace(__file__=str(path))
        pe = host.inspect_pe(raw)
        output = self.root / native / "exception-regions/fixture"
        plan = {"input": identity(original)["path"], "sha256": digest,
            "output": output.relative_to(self.root).as_posix(), "database": identity(database),
            "source_database": identity(source_database), "base_capture": identity(base_capture),
            "capture_tool": tool_sources["capture"], "parser_tool": tool_sources["parser"], "tool_sources": tool_sources,
            "exception_regions": {"directory": pe["exception_directory"], "tool_sources": tool_sources,
                "source_database": identity(source_database),
                "processor": {"name": "ia64l", "module": "ia", "bits": 64, "big_endian": False}}}
        base = int(pe["image_base"], 0)
        void = SimpleNamespace(type=0)
        operand = SimpleNamespace(type=7, dtype=2, reg=4, value=0x21, addr=0x80,
                                  specval=1, specflag1=2, specflag2=3, specflag3=4, specflag4=5)
        def instruction():
            return SimpleNamespace(ops=[operand, void], itype=77, auxpref=9,
                                   get_canon_mnem=lambda: "synthetic_ia64")
        def decode(insn, ea):
            return {0: 6, 6: 6, 12: 4}.get((ea - base) % 16, 0)
        def get_bytes(ea, count):
            section = pe["sections"][0]
            offset = section["file_offset"] + ea - base - section["rva"]
            return raw[offset:offset + count]
        enabled = [True]
        def enable_auto(value):
            previous, enabled[0] = enabled[0], value
            return previous
        sdk["ida_auto"].enable_auto = MagicMock(side_effect=enable_auto)
        sdk["ida_auto"].is_auto_enabled = lambda: enabled[0]
        sdk["ida_loader"].get_path = lambda kind: str(database)
        sdk["ida_loader"].PATH_TYPE_IDB = 0
        sdk["ida_nalt"].retrieve_input_file_sha256 = lambda: bytes.fromhex(plan["sha256"])
        sdk["ida_ida"].validate_idb = lambda flags: 0
        sdk["ida_ida"].VLD_SILENT = 0
        sdk["ida_ida"].inf_get_procname = lambda: "ia64l"
        sdk["ida_ida"].inf_is_64bit = lambda: True
        sdk["ida_ida"].inf_is_be = lambda: False
        sdk["ida_idp"].get_idp_name = lambda: "IA"
        sdk["ida_diskio"].idadir = lambda relative: str(installation / relative)
        sdk["ida_diskio"].getsysfile = lambda name, relative: str(installation / relative / name)
        sdk["idaapi"].get_imagebase = lambda: base
        sdk["ida_kernwin"].get_kernel_version = lambda: "fake-static-ida"
        sdk["ida_bytes"].get_bytes = MagicMock(side_effect=get_bytes)
        sdk["ida_bytes"].is_code = lambda flags: flags == 1
        sdk["ida_bytes"].is_align = lambda flags: flags == 3
        sdk["ida_bytes"].is_head = lambda flags: flags in (1, 3)
        sdk["ida_bytes"].is_tail = lambda flags: flags == 2
        sdk["ida_bytes"].get_item_head = lambda ea: ea
        sdk["ida_bytes"].get_item_end = lambda ea: ea + 1
        sdk["ida_bytes"].get_full_flags = lambda ea: 1
        sdk["ida_ua"].insn_t = instruction
        sdk["ida_ua"].decode_insn = MagicMock(side_effect=decode)
        sdk["ida_ua"].o_void = 0
        sdk["ida_ua"].print_operand = lambda ea, index: "r4"
        sdk["ida_lines"].tag_remove = lambda text: text
        sdk["idc"].generate_disasm_line = lambda ea, flags: "synthetic_ia64 r4"
        sdk["idautils"].XrefsFrom = lambda ea, flags: []
        sdk["idautils"].XrefsTo = lambda ea, flags: []
        owners = {base + 0x1100: [(base + 0x1100, base + 0x1180)]}
        sdk["idautils"].Functions = lambda: list(owners)
        sdk["idautils"].Chunks = lambda ea: list(owners[ea])
        namespace = {"__file__": str(tool), "Path": Path, "hashlib": hashlib, "json": json, "sys": sys,
            "datetime": datetime, "timezone": timezone, "file_sha256": analysis.sha,
            "capture_io": SimpleNamespace(__file__=str(support)), **sdk,
        }
        definitions = [node for node in ast.parse(tool_source.read_text(encoding="utf-8")).body
                       if isinstance(node, ast.FunctionDef) and node.name in {"write_json", "capture_exception_regions"}]
        exec(compile(ast.Module(body=definitions, type_ignores=[]), str(tool_source), "exec"), namespace)
        parser_module = SimpleNamespace(__file__=str(parser), inspect_pe=host.inspect_pe,
            IA64_EXCEPTION_ANALYZER_FILES=host.IA64_EXCEPTION_ANALYZER_FILES,
            IA64_EXCEPTION_SDK_MODULES=host.IA64_EXCEPTION_SDK_MODULES)
        def run(plan):
            with patch.dict(sys.modules, {"host": parser_module}), patch.object(sys, "path", list(sys.path)):
                return namespace["capture_exception_regions"](plan)
        return {"plan": plan, "run": run, "namespace": namespace,
                "output": output, "raw": raw, "get_bytes": get_bytes, "original": original,
                "source_database": source_database, "base_capture": base_capture, "decode": decode,
                "installation": installation, "parser_module": parser_module, "owners": owners,
                "enabled": enabled, "identity": identity}

    def test_explicit_cli_mode_and_private_capture_dispatch(self):
        parser = argparse.ArgumentParser()
        analysis.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["capture-native", "--input", "fixture.sys", "--exception-regions-only"])
        self.assertTrue(args.exception_regions_only)
        source = Path(__file__).with_name("ida") / "capture.py"
        tree = ast.parse(source.read_text(encoding="utf-8"))
        capture = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "capture")
        branch = capture.body[1]
        self.assertIsInstance(branch, ast.If)
        self.assertEqual(ast.unparse(branch.test), "'exception_regions' in plan")
        self.assertEqual(ast.unparse(branch.body[0].value), "capture_exception_regions(plan)")

    def test_full_read_only_decode_preserves_operands_and_never_promotes_regions_to_functions(self):
        fixture = self.fixture()
        manifest = fixture["run"](fixture["plan"])
        self.assertTrue(manifest["capture_complete"])
        self.assertEqual(len(manifest["regions"]), 2)
        self.assertEqual(sum(r["decoded_bytes"] for r in manifest["regions"]), 192)
        for key in ("function_discovery_complete", "pseudocode_captured", "decompilation_attempted",
                    "functions_created", "target_execution", "real_device_operations"):
            self.assertFalse(manifest[key])
        row = json.loads((fixture["output"] / manifest["regions"][0]["path"]).read_bytes())
        self.assertEqual(row["raw_hex"], bytes(range(128)).hex())
        self.assertEqual(row["assembly"][0]["operands"][0],
            {"index": 0, "type": 7, "dtype": 2, "register": 4, "value": 0x21, "address": 0x80,
             "specval": 1, "specflags": [2, 3, 4, 5], "text": "r4"})
        self.assertEqual(row["assembly"][0]["auxpref"], 9)
        self.assertEqual(fixture["source_database"].read_bytes(), b"inert immutable base IDB")
        self.assertEqual(fixture["original"].read_bytes(), fixture["raw"])

    def test_decode_failure_and_boundary_overrun_remain_failed_with_original_bytes_retained(self):
        for returned in (0, 129):
            with self.subTest(returned=returned):
                fixture = self.fixture()
                fixture["namespace"]["ida_ua"].decode_insn.side_effect = lambda insn, ea: returned
                manifest = fixture["run"](fixture["plan"])
                self.assertFalse(manifest["capture_complete"])
                self.assertEqual(len(manifest["failures"]), 12)
                self.assertTrue(all(r["decoded_bytes"] == 0 and not r["decode_complete"] for r in manifest["regions"]))
                self.assertEqual(sum(r["undecoded_bytes"] for r in manifest["regions"]), 192)
                row = json.loads((fixture["output"] / manifest["regions"][0]["path"]).read_bytes())
                self.assertEqual(row["raw_hex"], bytes(range(128)).hex())
                self.assertEqual(b"".join(bytes.fromhex(r["raw_hex"]) for r in row["undecoded_ranges"]), bytes(range(128)))

    def test_failed_slot_retains_exact_gap_and_continues_at_next_original_bundle(self):
        fixture = self.fixture()
        original_decode = fixture["decode"]
        failed = int(fixture["plan"]["exception_regions"]["directory"]["regions"][0]["begin_rva"], 0) + 0x26
        base = fixture["namespace"]["idaapi"].get_imagebase()
        fixture["namespace"]["ida_ua"].decode_insn.side_effect = (
            lambda insn, ea: 0 if ea == base + failed else original_decode(insn, ea))
        manifest = fixture["run"](fixture["plan"])
        self.assertFalse(manifest["capture_complete"])
        self.assertEqual(len(manifest["failures"]), 1)
        self.assertEqual(sum(r["decoded_bytes"] for r in manifest["regions"]), 182)
        self.assertEqual(sum(r["undecoded_bytes"] for r in manifest["regions"]), 10)
        row = json.loads((fixture["output"] / manifest["regions"][0]["path"]).read_bytes())
        gap = {"start_rva": hex(failed), "end_rva": hex(failed + 10),
               "bytes": 10, "raw_hex": bytes(range(0x26, 0x30)).hex()}
        self.assertEqual(row["undecoded_ranges"], [gap])
        self.assertEqual(manifest["failures"][0]["undecoded_range"], gap)
        self.assertEqual(manifest["failures"][0]["resumed_at_rva"], hex(failed + 10))
        decoded = {int(insn["rva"], 0): insn for insn in row["assembly"]}
        self.assertIn(failed + 10, decoded)
        self.assertNotIn(failed + 6, decoded)
        self.assertFalse(row["decode_complete"])
        self.assertTrue(manifest["regions"][1]["decode_complete"])
        spans = [(int(insn["rva"], 0), bytes.fromhex(insn["bytes"])) for insn in row["assembly"]]
        spans += [(int(gap["start_rva"], 0), bytes.fromhex(gap["raw_hex"]))]
        self.assertEqual(b"".join(raw for _, raw in sorted(spans)), bytes(range(128)))
        self.assertTrue(manifest["function_chunks_unchanged"])

    def test_exact_existing_alignment_item_is_retained_without_claiming_instruction_decoding(self):
        fixture = self.fixture()
        sdk = fixture["namespace"]["ida_bytes"]
        base = fixture["namespace"]["idaapi"].get_imagebase()
        start = base + 0x1120
        stop = start + 16
        sdk.get_full_flags = lambda ea: 3 if ea == start else 2 if start < ea < stop else 1
        sdk.get_item_head = lambda ea: start if start <= ea < stop else ea
        sdk.get_item_end = lambda ea: stop if start <= ea < stop else ea + 1
        fixture["namespace"]["idc"].generate_disasm_line = lambda ea, flags: ".align 5" if ea == start else "instruction"
        edge = SimpleNamespace(frm=start - 4, to=start + 6, type=19, iscode=True, user=False)
        fixture["namespace"]["idautils"].XrefsTo = lambda ea, flags: [edge] if ea == start + 6 else []
        manifest = fixture["run"](fixture["plan"])
        self.assertTrue(manifest["capture_complete"])
        self.assertFalse(manifest["instruction_decode_complete"])
        self.assertEqual(sum(row["alignment_bytes"] for row in manifest["regions"]), 16)
        self.assertEqual(sum(row["decoded_bytes"] for row in manifest["regions"]), 176)
        self.assertEqual(sum(row["undecoded_bytes"] for row in manifest["regions"]), 0)
        row = json.loads((fixture["output"] / manifest["regions"][0]["path"]).read_bytes())
        self.assertTrue(row["coverage_complete"])
        self.assertFalse(row["decode_complete"])
        self.assertFalse(manifest["pseudocode_captured"])
        item = row["alignment_items"][0]
        self.assertEqual(bytes.fromhex(item["raw_hex"]), bytes(range(0x20, 0x30)))
        self.assertEqual(item["sha256"], hashlib.sha256(bytes(range(0x20, 0x30))).hexdigest())
        self.assertEqual(item["classification"], "existing_idb_alignment_item")
        self.assertFalse(item["instruction_semantics_inferred"])
        self.assertEqual(item["xrefs_to"], [{"from": hex(start - 4), "to": hex(start + 6),
                                          "type": 19, "code": True, "user": False}])
        decoded_addresses = {int(call.args[1]) for call in fixture["namespace"]["ida_ua"].decode_insn.call_args_list}
        self.assertTrue(decoded_addresses.isdisjoint(range(start, stop)))
        spans = [(int(insn["rva"], 0), bytes.fromhex(insn["bytes"])) for insn in row["assembly"]]
        spans += [(int(item["start_rva"], 0), bytes.fromhex(item["raw_hex"]))]
        self.assertEqual(b"".join(raw for _, raw in sorted(spans)), bytes(range(128)))

    def test_inexact_alignment_metadata_cannot_hide_a_failed_instruction_slot(self):
        for changed in ("head", "end", "unaligned_end", "outside_region", "code", "tail", "tail_owner", "tail_end"):
            with self.subTest(changed=changed):
                fixture = self.fixture()
                sdk = fixture["namespace"]["ida_bytes"]
                base = fixture["namespace"]["idaapi"].get_imagebase()
                start, stop = base + 0x1120, base + 0x1130
                sdk.get_full_flags = lambda ea: 3 if ea == start else 2 if start < ea < stop else 1
                sdk.get_item_head = lambda ea: start if start <= ea < stop else ea
                sdk.get_item_end = lambda ea: stop if start <= ea < stop else ea + 1
                if changed == "head":
                    sdk.get_item_head = lambda ea: start - 16
                elif changed == "end":
                    sdk.get_item_end = lambda ea: start
                elif changed == "unaligned_end":
                    sdk.get_item_end = lambda ea: stop - 1
                elif changed == "outside_region":
                    sdk.get_item_end = lambda ea: base + 0x1190
                elif changed == "code":
                    sdk.is_code = lambda flags: flags in (1, 3)
                elif changed == "tail":
                    sdk.is_tail = lambda flags: False
                elif changed == "tail_owner":
                    sdk.get_item_head = lambda ea: start + 1 if ea == start + 7 else start
                elif changed == "tail_end":
                    sdk.get_item_end = lambda ea: stop + 16 if ea == start + 7 else stop
                original_decode = fixture["decode"]
                fixture["namespace"]["ida_ua"].decode_insn.side_effect = (
                    lambda insn, ea: 0 if ea == start + 6 else original_decode(insn, ea))
                manifest = fixture["run"](fixture["plan"])
                self.assertFalse(manifest["capture_complete"])
                self.assertEqual(sum(row["alignment_bytes"] for row in manifest["regions"]), 0)
                self.assertEqual(sum(row["undecoded_bytes"] for row in manifest["regions"]), 10)
                row = json.loads((fixture["output"] / manifest["regions"][0]["path"]).read_bytes())
                self.assertEqual(row["alignment_items"], [])
                self.assertEqual(row["undecoded_ranges"][0]["raw_hex"], bytes(range(0x26, 0x30)).hex())

    def test_changed_original_loaded_bytes_tool_and_base_evidence_are_rejected(self):
        for changed in ("original", "loaded", "capture_tool", "parser_tool", "base_capture", "source_database", "selection"):
            with self.subTest(changed=changed):
                fixture = self.fixture()
                if changed == "loaded":
                    fixture["namespace"]["ida_bytes"].get_bytes.side_effect = lambda ea, count: b"\xff" * count
                elif changed == "selection":
                    fixture["plan"]["exception_regions"]["directory"]["regions"][0]["end_rva"] = "0x1190"
                elif changed == "original":
                    fixture["original"].write_bytes(b"changed source")
                else:
                    path = self.root / fixture["plan"][changed]["path"]
                    path.write_bytes(b"changed evidence")
                with self.assertRaises(ValueError):
                    fixture["run"](fixture["plan"])

    def test_mid_capture_evidence_mutation_cannot_publish_a_complete_manifest(self):
        fixture = self.fixture()
        def mutate(insn, ea):
            fixture["base_capture"].write_bytes(b"changed during capture")
            return 16
        fixture["namespace"]["ida_ua"].decode_insn.side_effect = mutate
        with self.assertRaisesRegex(ValueError, "changed during capture"):
            fixture["run"](fixture["plan"])
        self.assertFalse((fixture["output"] / "capture.json").exists())

    def test_real_ia64_short_and_long_instruction_steps_cover_exact_region_boundaries(self):
        for steps in ({0: 6, 6: 6, 12: 4}, {0: 12, 12: 4}):
            with self.subTest(steps=steps):
                fixture = self.fixture()
                fixture["namespace"]["ida_ua"].decode_insn.side_effect = lambda insn, ea: steps.get(ea % 16, 0)
                manifest = fixture["run"](fixture["plan"])
                self.assertTrue(manifest["capture_complete"])
                for record in manifest["regions"]:
                    row = json.loads((fixture["output"] / record["path"]).read_bytes())
                    self.assertEqual(sum(x["size"] for x in row["assembly"]), row["bytes"])
                    self.assertEqual(bytes.fromhex("".join(x["bytes"] for x in row["assembly"])), bytes.fromhex(row["raw_hex"]))
                    self.assertTrue(all(steps[int(x["address"], 0) % 16] == x["size"] for x in row["assembly"]))
                self.assertTrue(manifest["function_chunks_unchanged"])
                snapshot = manifest["function_chunks_snapshot"]
                self.assertEqual((snapshot["functions"], snapshot["chunks"]), (1, 1))
                self.assertEqual(snapshot["sha256_before"], snapshot["sha256_after"])
                self.assertFalse(manifest["automatic_analysis_enabled"])

    def test_last_slot_overrun_does_not_count_bytes_past_either_region(self):
        fixture = self.fixture()
        endings = {int(row["end_rva"], 0) - 4 for row in fixture["plan"]["exception_regions"]["directory"]["regions"]}
        base = 0x180000000
        fixture["namespace"]["ida_ua"].decode_insn.side_effect = lambda insn, ea: (
            5 if ea - base in endings else fixture["decode"](insn, ea))
        manifest = fixture["run"](fixture["plan"])
        self.assertFalse(manifest["capture_complete"])
        self.assertEqual([row["decoded_bytes"] for row in manifest["regions"]], [124, 60])
        self.assertEqual([row["decoded_size"] for row in manifest["failures"]], [5, 5])

    def test_wrong_actual_processor_module_bitness_or_byte_order_cannot_claim_decode_completion(self):
        for module, method, result in (("ida_ida", "inf_get_procname", "metapc"),
                ("ida_idp", "get_idp_name", "pc"), ("ida_ida", "inf_is_64bit", False),
                ("ida_ida", "inf_is_be", True)):
            with self.subTest(method=method):
                fixture = self.fixture()
                setattr(fixture["namespace"][module], method, lambda: result)
                with self.assertRaisesRegex(ValueError, "processor"):
                    fixture["run"](fixture["plan"])
                fixture["namespace"]["ida_ua"].decode_insn.assert_not_called()
                self.assertFalse((fixture["output"] / "capture.json").exists())

    def test_changed_entry_support_processor_sdk_or_retained_snapshot_is_rejected(self):
        for name in ("entry", "support", "processor", "sdk", "snapshot"):
            with self.subTest(name=name):
                fixture = self.fixture()
                tools = fixture["plan"]["tool_sources"]
                row = tools["sdk"]["ida_ua"] if name == "sdk" else tools["capture" if name == "snapshot" else name]
                if name == "snapshot":
                    path = self.root / row["snapshot"]["path"]
                elif "installation_path" in row:
                    path = fixture["installation"] / row["installation_path"]
                else:
                    path = self.root / row["path"]
                path.write_bytes(b"changed tool identity")
                with self.assertRaises(ValueError):
                    fixture["run"](fixture["plan"])
                fixture["namespace"]["ida_ua"].decode_insn.assert_not_called()

    def test_loaded_parser_support_and_sdk_paths_must_match_bound_files(self):
        for name in ("parser", "capture_io", "ida_ua", "processor_search"):
            with self.subTest(name=name):
                fixture = self.fixture()
                if name == "parser":
                    fixture["parser_module"].__file__ = str(self.root / "other-host.py")
                elif name == "processor_search":
                    fixture["namespace"]["ida_diskio"].getsysfile = lambda name, relative: str(self.root / "other-ia.dll")
                else:
                    fixture["namespace"][name].__file__ = str(self.root / "other.py")
                with self.assertRaises(ValueError):
                    fixture["run"](fixture["plan"])
                fixture["namespace"]["ida_ua"].decode_insn.assert_not_called()

    def test_tool_or_snapshot_changes_during_decode_cannot_publish_completion(self):
        for name in ("support", "processor", "sdk", "snapshot"):
            with self.subTest(name=name):
                fixture = self.fixture()
                tools = fixture["plan"]["tool_sources"]
                row = tools["sdk"]["ida_ua"] if name == "sdk" else tools["support" if name == "snapshot" else name]
                path = (self.root / row["snapshot"]["path"] if name == "snapshot" else
                        fixture["installation"] / row["installation_path"] if "installation_path" in row else
                        self.root / row["path"])
                changed = [False]
                def mutate(insn, ea):
                    if not changed[0]:
                        changed[0] = True
                        path.write_bytes(b"changed while decoding")
                    return fixture["decode"](insn, ea)
                fixture["namespace"]["ida_ua"].decode_insn.side_effect = mutate
                with self.assertRaises(ValueError):
                    fixture["run"](fixture["plan"])
                self.assertFalse((fixture["output"] / "capture.json").exists())

    def test_function_chunk_changes_or_reenabled_analysis_prevent_publication(self):
        for change in ("function", "chunk", "automatic_analysis"):
            with self.subTest(change=change):
                fixture = self.fixture()
                changed = [False]
                def mutate(insn, ea):
                    if not changed[0]:
                        changed[0] = True
                        if change == "function":
                            fixture["owners"][0x180001200] = [(0x180001200, 0x180001240)]
                        elif change == "chunk":
                            fixture["owners"][0x180001100].append((0x180001200, 0x180001240))
                        else:
                            fixture["enabled"][0] = True
                    return fixture["decode"](insn, ea)
                fixture["namespace"]["ida_ua"].decode_insn.side_effect = mutate
                with self.assertRaisesRegex(ValueError, "ownership|automatic analysis"):
                    fixture["run"](fixture["plan"])
                self.assertFalse((fixture["output"] / "capture.json").exists())

    def test_bootstrap_disables_only_exception_analysis_before_any_wait(self):
        import runpy

        for mode in ("exception", "normal", "mcp"):
            with self.subTest(mode=mode):
                fixture = self.fixture()
                entry = self.root / fixture["plan"]["tool_sources"]["entry"]["path"]
                plan_path = self.source(".work/bootstrap-plan.json", json.dumps(
                    fixture["plan"] if mode == "exception" else {}).encode())
                calls = []
                auto = SimpleNamespace(auto_wait=lambda: calls.append("wait"),
                    enable_auto=lambda enabled: calls.append(("auto", enabled)))
                api = SimpleNamespace(load_and_run_plugin=lambda name, arg: calls.append("plugin"))
                idc = SimpleNamespace(qexit=lambda code: calls.append(("exit", code)))
                environment = {"RAZER_IDA_CAPTURE_PLAN": str(plan_path)} if mode != "mcp" else {}
                with patch.dict(os.environ, environment, clear=True), patch.dict(sys.modules,
                        {"ida_auto": auto, "idaapi": api, "idc": idc}), patch.object(runpy, "run_path",
                        return_value={"capture": lambda: calls.append("capture")}), redirect_stdout(io.StringIO()):
                    exec(compile(entry.read_text(encoding="utf-8"), str(entry), "exec"), {"__file__": str(entry)})
                self.assertEqual(calls, {"exception": [("auto", False), "capture", ("exit", 0)],
                    "normal": ["wait", "capture", ("exit", 0)], "mcp": ["wait", "plugin"]}[mode])

    def test_export_binds_snapshots_and_source_database_in_immutable_attempt_key(self):
        fixture = self.fixture()
        plans, commands = [], []
        def launch(command, *, cwd, env, **kwargs):
            commands.append(command)
            plan = json.loads(Path(env["RAZER_IDA_CAPTURE_PLAN"]).read_bytes())
            plans.append(plan)
            def wait():
                analysis.save(self.root / plan["output"] / "capture.json", {"capture_complete": True,
                    "input_sha256": plan["sha256"], "exception_regions": plan["exception_regions"],
                    "tool_sources": plan["tool_sources"], "function_chunks_unchanged": True})
                return 0
            return SimpleNamespace(pid=1234, wait=wait)
        args = argparse.Namespace(ida_root=str(fixture["installation"]), no_refresh_gaps=True)
        original = {"sha256": fixture["plan"]["sha256"], "bytes": len(fixture["raw"])}
        with patch.object(analysis, "configure_ida"), patch.object(analysis.subprocess, "Popen", side_effect=launch), redirect_stdout(io.StringIO()):
            first = analysis.capture_native_exception_regions(args, fixture["original"], original, host.inspect_pe(fixture["raw"]))
            fixture["source_database"].write_bytes(b"different retained base database")
            second = analysis.capture_native_exception_regions(args, fixture["original"], original, host.inspect_pe(fixture["raw"]))
            with self.assertRaisesRegex(ValueError, "immutable"):
                analysis.capture_native_exception_regions(args, fixture["original"], original, host.inspect_pe(fixture["raw"]))
        self.assertTrue(first["passed"] and second["passed"])
        self.assertNotEqual(plans[0]["output"], plans[1]["output"])
        self.assertNotEqual(plans[0]["source_database"]["sha256"], plans[1]["source_database"]["sha256"])
        self.assertTrue(all(command[1:3] == ["-a", "-A"] for command in commands))
        for plan in plans:
            self.assertEqual(plan["exception_regions"]["source_database"], plan["source_database"])
            tools = plan["tool_sources"]
            rows = [row for name, row in tools.items() if name != "sdk"] + list(tools["sdk"].values())
            self.assertEqual(set(tools["sdk"]), set(host.IA64_EXCEPTION_SDK_MODULES))
            for row in rows:
                retained = self.root / row["snapshot"]["path"]
                self.assertEqual(analysis.sha(retained), row["sha256"])
                self.assertEqual(retained.stat().st_size, row["bytes"])


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

        with patch.object(analysis, "capture_native", side_effect=capture), \
             patch.object(analysis, "configure_ida"), redirect_stdout(io.StringIO()):
            analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[],
                                                       ida_root="unused", workers=2, no_refresh_gaps=True))
        queue = json.loads((self.root / ".analysis/results/native-corpus-capture.json").read_bytes())
        self.assertTrue(queue["capture_complete"])
        self.assertFalse(queue["snapshot_unchanged"])
        self.assertFalse(queue["full_corpus_captured"])

    def test_parallel_native_corpus_isolates_modules_and_keeps_failures_in_one_receipt(self):
        rows = []
        for index in range(4):
            raw = ("inert original " + str(index)).encode()
            alias = ".analysis/inputs/test/device" + str(index) + ".dll"
            self.source(alias, raw)
            rows.append({"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw), "aliases": [alias]})
        self.source(".analysis/results/corpus.json", json.dumps({"native_inputs": rows}).encode())
        by_path = {row["aliases"][0]: row for row in rows}
        barrier = threading.Barrier(2, timeout=10)
        lock = threading.Lock()
        active, peak, configured = set(), 0, []
        publisher = threading.get_ident()
        publications = []
        original_save = analysis.save

        def save(path, value):
            if path.name == "native-corpus-capture.json":
                publications.append(threading.get_ident())
            return original_save(path, value)

        def configure():
            configured.append(threading.get_ident())

        def capture(args):
            nonlocal peak
            row = by_path[args.input]
            self.assertTrue(args.ida_configuration_prepared)
            self.assertTrue(args.no_refresh_gaps)
            with lock:
                self.assertEqual(len(configured), 1)
                self.assertNotIn(row["sha256"], active)
                active.add(row["sha256"])
                peak = max(peak, len(active))
            try:
                barrier.wait()
                if args.input.endswith("device1.dll"):
                    raise RuntimeError("synthetic isolated analyzer failure")
                self.native_capture(row["sha256"], row["bytes"])
                return {"passed": True, "exit_code": 0}
            finally:
                with lock:
                    active.remove(row["sha256"])

        with patch.object(analysis, "capture_native", side_effect=capture), \
             patch.object(analysis, "configure_ida", side_effect=configure), \
             patch.object(analysis, "save", side_effect=save), redirect_stdout(io.StringIO()):
            analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[], workers=2,
                                                       ida_root="unused", no_refresh_gaps=True))
        queue = json.loads((self.root / ".analysis/results/native-corpus-capture.json").read_bytes())
        self.assertEqual(peak, 2)
        self.assertFalse(active)
        self.assertEqual(len(configured), 1)
        self.assertEqual(set(publications), {publisher})
        self.assertEqual(len(queue["modules"]), 4)
        self.assertEqual([row["sha256"] for row in queue["modules"]], sorted(by_path[path]["sha256"] for path in by_path))
        failed = [row for row in queue["modules"] if row["status"] == "failed"]
        self.assertEqual([row["input"] for row in failed], [rows[1]["aliases"][0]])
        self.assertIn("synthetic isolated analyzer failure", failed[0]["error"])
        self.assertTrue(all(row["original_snapshot_unchanged"] for row in queue["modules"] if row["status"] == "captured"))
        self.assertTrue(queue["snapshot_unchanged"] and queue["original_inputs_snapshot_unchanged"])
        self.assertFalse(queue["capture_complete"] or queue["full_corpus_captured"])

    def test_parallel_native_corpus_reuse_skips_configuration_and_analyzer(self):
        rows = []
        for index in range(3):
            raw = ("inert retained capture " + str(index)).encode()
            alias = ".analysis/inputs/test/reused" + str(index) + ".dll"
            self.source(alias, raw)
            row = {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw), "aliases": [alias]}
            rows.append(row)
            self.native_capture(row["sha256"], len(raw))
        self.source(".analysis/results/corpus.json", json.dumps({"native_inputs": rows}).encode())
        with patch.object(analysis, "capture_native") as capture, \
             patch.object(analysis, "configure_ida") as configure, redirect_stdout(io.StringIO()):
            analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[], workers=3,
                                                       ida_root="unused", no_refresh_gaps=True))
        capture.assert_not_called()
        configure.assert_not_called()
        queue = json.loads((self.root / ".analysis/results/native-corpus-capture.json").read_bytes())
        self.assertTrue(queue["full_corpus_captured"])
        self.assertEqual(len(queue["modules"]), 3)
        self.assertTrue(all(row["status"] == "reused_complete_capture" for row in queue["modules"]))

    def test_native_corpus_rejects_invalid_parallelism_before_publishing(self):
        for workers in (0, 1, -1, 5, True, 2.5):
            with self.subTest(workers=workers), self.assertRaisesRegex(ValueError, "between two and four"):
                analysis.capture_corpus(argparse.Namespace(workers=workers))
        self.assertFalse((self.root / ".analysis/results/native-corpus-capture.json").exists())

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

        with patch.object(analysis, "capture_native", side_effect=capture), \
             patch.object(analysis, "configure_ida"), redirect_stdout(io.StringIO()):
            analysis.capture_corpus(argparse.Namespace(source="retained", include_path=[],
                                                       ida_root="unused", workers=2, no_refresh_gaps=True))
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

    def test_release_gap_owners_keep_acquisition_failures_and_all_remaining_dimensions(self):
        name = ".analysis/inputs/host-current/releases/4.0.896/acquisition.json"
        for complete in (False, True):
            with self.subTest(complete=complete):
                data = {"version": "4.0.896", "complete": complete, "host_package_closure_complete": False,
                        "failures": [] if complete else [{"stage": "download", "error": "canceled"}]}
                raw = json.dumps(data).encode()
                self.source(name, raw)
                index = {"missing_current_roots": [], "inputs": [{"path": name, "bytes": len(raw),
                    "sha256": hashlib.sha256(raw).hexdigest(), "kind": "official_host_release_acquisition"}]}
                self.source(".analysis/inputs.json", json.dumps(index).encode())
                ledger = {"schema_version": 1, "owners": {"manual-owner": [{"kind": "preserved"}]}}
                self.source(".analysis/results/gaps.json", json.dumps(ledger).encode())
                result = analysis._refresh_gaps()
                rows = result["owners"][name]
                kinds = {row["kind"] for row in rows}
                self.assertEqual("host_acquisition_failure" in kinds, not complete)
                self.assertEqual("host_release_acquisition_incomplete" in kinds, not complete)
                self.assertIn("host_package_closure_pending", kinds)
                self.assertIn("host_release_semantics_and_acceptance_pending", kinds)
                self.assertEqual(result["owners"]["manual-owner"], [{"kind": "preserved"}])
                self.assertIn(name, result["derived_owners"])

    def test_changed_release_gap_receipt_cannot_replace_the_valid_ledger(self):
        name = ".analysis/inputs/host-current/releases/4.0.896/acquisition.json"
        raw = b'{"complete":true,"failures":[]}'
        self.source(name, raw)
        self.source(".analysis/inputs.json", json.dumps({"missing_current_roots": [], "inputs": [{
            "path": name, "bytes": len(raw), "sha256": "0" * 64,
            "kind": "official_host_release_acquisition"}]}).encode())
        previous = b'{"schema_version":1,"owners":{"existing":[{"kind":"preserved"}]}}'
        self.source(".analysis/results/gaps.json", previous)
        with self.assertRaisesRegex(ValueError, "differs from its indexed identity"):
            analysis._refresh_gaps()
        self.assertEqual((self.root / ".analysis/results/gaps.json").read_bytes(), previous)


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

    def release_fixture(self):
        import base64
        unused_bundle, unused_packed, unused_unpacked, previous = self.host_fixture()
        package = self.artifact(".analysis/inputs/host-current/blobs/current-package.exe", b"MZ current host package",
            kind="official_host_artifact", url="https://app-assets.razer.com/current-package.exe")
        prior_release = next(row for row in previous["files"] if row.get("kind") == "official_host_release_manifest")
        manifest = self.artifact(".analysis/inputs/host-current/blobs/current-resources.json", json.dumps({
            "resources": [{"url": package["url"], "size": package["bytes"], "sha256": package["sha256"],
                           "integrity": "sha256-" + base64.b64encode(bytes.fromhex(package["sha256"])).decode()}]
        }).encode(), kind="official_host_resource_manifest", url="https://apps.razer.com/host/releases/current.json")
        metadata = {"finished_at_utc": "synthetic", "failures": [], "files": [manifest, prior_release],
                    "stability": [{"url": row["url"], "unchanged": True,
                                   "before_sha256": row["sha256"], "after_sha256": row["sha256"]}
                                  for row in (manifest, prior_release)]}
        self.receipt(".analysis/inputs/host-current/metadata-recheck.json", metadata, indexed=True)
        snapshot = self.receipt(".analysis/inputs/host-current/releases/4.0.896/metadata-recheck.json", metadata, indexed=True)
        acquired = {"files": [package], "complete": True, "failures": [], "verified_dependencies": [],
                    "metadata_snapshot": {key: snapshot[key] for key in ("path", "bytes", "sha256")}}
        claim = self.receipt(".analysis/inputs/host-current/releases/4.0.896/acquisition.json", acquired, indexed=True)
        acquired["receipt"] = {key: claim[key] for key in ("path", "bytes", "sha256")}
        return package, acquired

    def test_current_release_receipt_seeds_new_bytes_without_overwriting_or_requiring_legacy_acquisition(self):
        for legacy_present in (True, False):
            with self.subTest(legacy_present=legacy_present):
                package, acquired = self.release_fixture()
                legacy = self.root / ".analysis/inputs/host-current/acquisition.json"
                before = legacy.read_bytes()
                if not legacy_present:
                    legacy.unlink()
                groups = [self.group(package)]
                with patch.object(host, "host_release_metadata", return_value={"verified_dependencies": []}), \
                     patch.object(host, "verified_host_release_receipt", return_value=acquired):
                    result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertFalse(result["failures"])
                self.assertTrue(result["retained_pe_provenance_complete"])
                self.assertEqual(result["roots"][0]["artifact"]["path"], package["path"])
                self.assertIn(acquired["receipt"], result["receipts"])
                if legacy_present:
                    self.assertEqual(legacy.read_bytes(), before)
                else:
                    self.assertFalse(legacy.exists())

    def test_rejected_current_metadata_cannot_seed_legacy_artifacts_or_attempt_a_new_release(self):
        bundle, unused_packed, unused_unpacked, previous = self.host_fixture()
        self.receipt(".analysis/inputs/host-current/metadata-recheck.json", previous, indexed=True)
        groups = [self.group(bundle)]
        with patch.object(host, "host_release_metadata", side_effect=ValueError("actual samples disagree")), \
             patch.object(host, "verified_host_release_receipt") as release:
            result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        release.assert_not_called()
        self.assertFalse(result["roots"])
        self.assertFalse(result["retained_pe_provenance_complete"])
        self.assertTrue(any(row["kind"] == "current_host_acquisition_unverified" for row in result["failures"]))

    def test_helper_only_metadata_dependencies_remain_in_the_final_snapshot(self):
        package, acquired = self.release_fixture()
        dependency = self.artifact(".analysis/inputs/host-current/releases/baseline.json", b'{"baseline":true}')
        acquired["verified_dependencies"] = [dependency]
        actual_sha = analysis.sha

        def changed_after_package_hash(path):
            result = actual_sha(path)
            if path == self.root / package["path"]:
                target = self.root / dependency["path"]
                before = target.stat()
                os.utime(target, ns=(before.st_atime_ns, before.st_mtime_ns + 10_000_000))
            return result

        groups = [self.group(package)]
        with patch.object(host, "host_release_metadata", return_value={"verified_dependencies": [dependency]}), \
             patch.object(host, "verified_host_release_receipt", return_value=acquired), \
             patch.object(analysis, "sha", side_effect=changed_after_package_hash):
            result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        self.assertFalse(result["snapshot_unchanged"])
        self.assertIn(dependency["path"], result["changed_during_audit"])
        self.assertFalse(result["retained_pe_provenance_complete"])
        self.assertEqual(groups[0]["current_version_provenance"]["status"], "snapshot_changed")

    def test_rejected_changed_or_undeclared_release_bytes_do_not_become_current(self):
        for issue in ("rejected", "changed_receipt", "changed_metadata", "undeclared", "changed_artifact"):
            with self.subTest(issue=issue):
                package, acquired = self.release_fixture()
                if issue == "changed_receipt":
                    acquired["receipt"]["sha256"] = "0" * 64
                elif issue == "changed_metadata":
                    acquired["metadata_snapshot"]["sha256"] = "0" * 64
                elif issue == "undeclared":
                    path = self.root / acquired["receipt"]["path"]
                    body = json.loads(path.read_bytes())
                    body["files"][0]["url"] = "https://app-assets.razer.com/undeclared.exe"
                    claim = self.receipt(acquired["receipt"]["path"], body, indexed=True)
                    acquired["receipt"] = {key: claim[key] for key in ("path", "bytes", "sha256")}
                elif issue == "changed_artifact":
                    (self.root / package["path"]).write_bytes(b"changed current host bytes")
                groups = [self.group(package)]
                with patch.object(host, "host_release_metadata", return_value={"verified_dependencies": []}), \
                     patch.object(host, "verified_host_release_receipt", return_value=acquired,
                                  side_effect=ValueError("current metadata declaration differs") if issue == "rejected" else None):
                    result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertFalse(result["retained_pe_provenance_complete"])
                self.assertFalse(result["roots"])
                if issue != "undeclared":
                    self.assertTrue(result["failures"])

    def test_release_extractions_use_each_exact_receipt_and_keep_global_inspection_intact(self):
        bundle, unused_packed, unused_unpacked, unused_host = self.host_fixture()
        global_path = ".analysis/results/host-current/inspection.json"
        self.receipt(global_path, {"containers": []})
        before = (self.root / global_path).read_bytes()
        leaves, identities = [], []
        for number in (1, 2):
            leaf = self.artifact(f".analysis/inputs/host-current/extracted/leaf-{number}.dll",
                f"MZ release leaf {number}".encode(), kind="static_extracted_host_input",
                container=bundle["path"], container_sha256=bundle["sha256"])
            name = f".analysis/results/host-current/releases/4.0.{number}/{bundle['sha256']}/inspection.json"
            self.receipt(name, {"containers": [{"input_record": bundle, "sha256": bundle["sha256"],
                "extracted": True, "list_exit_code": 0, "extraction_exit_code": 0,
                "input_bytes_unchanged": True, "files": [dict(leaf)]}]})
            raw = (self.root / name).read_bytes()
            identity = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
            leaf["extraction_receipt"] = identity
            leaves.append(leaf)
            identities.append(identity)
        groups = [self.group(leaf) for leaf in leaves]
        result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
        self.assertTrue(result["retained_pe_provenance_complete"])
        self.assertTrue(all(row["current_version_provenance"]["root_ids"] for row in groups))
        self.assertEqual([edge["receipt"] for edge in result["container_edges"] if edge["child"]["path"] in
                          {leaf["path"] for leaf in leaves}], identities)
        self.assertEqual((self.root / global_path).read_bytes(), before)

    def test_release_extraction_proof_rejects_wrong_identity_ambiguous_containers_and_failed_outputs(self):
        for issue in ("hash", "bytes", "missing", "outside_owner", "duplicate", "partial", "wrong_output"):
            with self.subTest(issue=issue):
                bundle, unused_packed, unused_unpacked, unused_host = self.host_fixture()
                leaf = self.artifact(".analysis/inputs/host-current/extracted/release-leaf.dll", b"MZ release leaf",
                    kind="static_extracted_host_input", container=bundle["path"], container_sha256=bundle["sha256"])
                record = {"input_record": bundle, "sha256": bundle["sha256"], "extracted": True,
                          "list_exit_code": 0, "extraction_exit_code": 2 if issue == "partial" else 0,
                          "input_bytes_unchanged": True, "files": [] if issue == "wrong_output" else [dict(leaf)]}
                name = f".analysis/results/host-current/releases/4.0.896/{bundle['sha256']}/inspection.json"
                self.receipt(name, {"containers": [record, record] if issue == "duplicate" else [record]})
                raw = (self.root / name).read_bytes()
                identity = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
                if issue == "hash":
                    identity["sha256"] = "0" * 64
                elif issue == "bytes":
                    identity["bytes"] += 1
                elif issue == "missing":
                    (self.root / name).unlink()
                elif issue == "outside_owner":
                    identity["path"] = ".analysis/results/other/inspection.json"
                    self.source(identity["path"], raw)
                leaf["extraction_receipt"] = identity
                groups = [self.group(leaf)]
                result = analysis.corpus_current_lineage({"inputs": self.rows}, groups)
                self.assertFalse(result["retained_pe_provenance_complete"])
                self.assertFalse(groups[0]["current_version_provenance"]["root_ids"])
                self.assertTrue(any(row["kind"] == "current_container_edge_unverified" for row in result["failures"]))

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
    @staticmethod
    def typed_fixture(body):
        body["type"] = body["type_before_decompile"] = body["type_after_decompile"] = "void __cdecl()"
        body["assembly_after_decompile"] = json.loads(json.dumps(body["assembly"]))
        body["pseudocode"]["locals_capture"] = {"complete": True, "mba_entry_ea": body["address"]}

    def test_member_dispatch_thunk_requires_all_three_exact_x64_instructions(self):
        # mov rax,[rcx+8]; mov rcx,[rcx+24]; jmp rax.
        raw = bytes.fromhex("488b4108488b491848ffe0")
        self.assertEqual(analysis.native_member_dispatch_thunk_size(raw, "x64"), 11)
        for arch in ("x86", "arm64", "unknown"):
            with self.subTest(architecture=arch):
                self.assertIsNone(analysis.native_member_dispatch_thunk_size(raw, arch))
        for length in range(11):
            with self.subTest(truncated=length):
                self.assertIsNone(analysis.native_member_dispatch_thunk_size(raw[:length], "x64"))
        self.assertIsNone(analysis.native_member_dispatch_thunk_size(raw + b"\xcc", "x64"))
        for index in range(len(raw)):
            changed = bytearray(raw)
            changed[index] ^= 1
            with self.subTest(changed_byte=index):
                self.assertIsNone(analysis.native_member_dispatch_thunk_size(bytes(changed), "x64"))

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

    def test_exception_boundary_review_routes_exact_evidence_and_rejects_mixed_repairs(self):
        import native_boundaries
        capture = {"functions": [{"rva": "0x1000", "address": "0x140001000"}]}
        references = [".analysis/results/review.json#/helper"]
        reviewed = [{"owner_rva": "0x1000", "verified_complete_original_chain": True}]
        with patch.object(native_boundaries, "select", return_value=reviewed) as select:
            result = analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
                                                        boundary_reviews=references)
        self.assertEqual(result["function_boundary_recoveries"], reviewed)
        select.assert_called_once_with(analysis, bytes(self.fixture()), capture, [0x1000], references)
        for extra in ({"type_specs": ["0x1000=void reviewed();"], "type_note": "original bytes"},
                      {"import_specs": ["0x2020=void (*call)(void);"]}, {"merge_specs": ["0x1000=0x1005"]},
                      {"outline_rvas": [0x1000]}, {"partition_specs": ["0x1000=0x1005:0x1006"]},
                      {"shared_tail_specs": ["review"]}, {"fresh_inline_rvas": [0x1000]},
                      {"callsite_reviews": references}, {"reanalyze_type_owners": True},
                      {"propagate_direct_calls": True}):
            with self.subTest(extra=extra), patch.object(native_boundaries, "select") as select:
                with self.assertRaisesRegex(ValueError, "separate supplement"):
                    analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
                                                        boundary_reviews=references, **extra)
                select.assert_not_called()

    def test_original_instruction_review_is_explicit_separate_and_reconstructed_by_its_owner(self):
        import native_boundaries
        capture = {"functions": [{"rva": "0x1000", "address": "0x140001000"}]}
        references = [".analysis/results/review.json#/instruction"]
        reviewed = [{"owner_rva": "0x1000", "instruction": {"bytes": "e800000000"}}]
        with patch.object(native_boundaries, "select_instructions", return_value=reviewed) as select:
            selected = analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
                                                           instruction_reviews=references)
        self.assertEqual(selected["instruction_boundary_recoveries"], reviewed)
        select.assert_called_once_with(analysis, bytes(self.fixture()), capture, [0x1000], references)
        for extra in ({"type_specs": ["0x1000=void reviewed();"], "type_note": "complete assembly"},
                      {"boundary_reviews": references}, {"partition_specs": ["0x1000=0x1005:0x1006"]},
                      {"callsite_reviews": references}, {"reanalyze_type_owners": True}):
            with self.subTest(extra=extra), patch.object(native_boundaries, "select_instructions") as select:
                with self.assertRaisesRegex(ValueError, "separate supplement"):
                    analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
                                                        instruction_reviews=references, **extra)
                select.assert_not_called()

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

    def test_function_size_limit_is_bounded_and_part_of_exact_selection(self):
        capture = {"functions": [{"rva": "0x1000", "address": "0x140001000"}]}
        base = analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000])
        self.assertNotIn("decompiler_limits", base)
        for limit in (64, 512, 1024):
            with self.subTest(limit=limit):
                selected = analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
                                                             max_function_kib=limit)
                self.assertEqual(selected["targets"], base["targets"])
                self.assertEqual(selected["decompiler_limits"], {"max_function_kib": limit})
                self.assertNotEqual(selected, base)
        for limit in (True, 0, 63, 1025, 512.0, "512"):
            with self.subTest(limit=limit), self.assertRaisesRegex(ValueError, "size limit"):
                analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
                                                   max_function_kib=limit)

    def shared_tail_fixture(self, root):
        raw = bytearray(self.fixture())
        base = 0x140000000
        raw[0x200:0x207] = bytes.fromhex("558bec9031c0c3")
        raw[0x240:0x245] = bytes.fromhex("e9bfffffff")
        raw = bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        result = root / ".analysis/results/native" / digest
        source = root / ".analysis/inputs/native/shared.dll"

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            data = json.dumps(value).encode()
            path.write_bytes(data)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest()}

        source.parent.mkdir(parents=True)
        source.write_bytes(raw)

        def edge(frm, to, kind):
            return {"from": hex(base + frm), "to": hex(base + to), "type": kind,
                    "code": True, "user": False}

        primary_rows = [(0x1000, "55", "push ebp", [edge(0x1000, 0x1001, 21)], []),
                        (0x1001, "8bec", "mov ebp, esp", [edge(0x1001, 0x1003, 21)], [edge(0x1000, 0x1001, 21)]),
                        (0x1003, "90", "nop", [edge(0x1003, 0x1004, 21)], [edge(0x1001, 0x1003, 21)]),
                        (0x1004, "31c0", "xor eax, eax", [edge(0x1004, 0x1006, 21)],
                         [edge(0x1003, 0x1004, 21), edge(0x1040, 0x1004, 19)]),
                        (0x1006, "c3", "retn", [], [edge(0x1004, 0x1006, 21)])]
        secondary_rows = [(0x1040, "e9bfffffff", "jmp loc_140001004", [edge(0x1040, 0x1004, 19)], [])]
        bodies = []
        for rva, rows in ((0x1000, primary_rows), (0x1040, secondary_rows)):
            start = base + rva
            end = base + (0x1007 if rva == 0x1000 else 0x1045)
            file_start = 0x200 + rva - 0x1000
            code = raw[file_start:file_start + end - start]
            assembly = [{"address": hex(base + at), "rva": hex(at), "bytes": value, "text": text,
                         "xrefs_from": outgoing, "xrefs_to": incoming}
                        for at, value, text, outgoing, incoming in rows]
            body = {"address": hex(start), "rva": hex(rva), "end": hex(end), "flags": 0,
                    "chunks": [{"start": hex(start), "end": hex(end), "raw_hex": code.hex(),
                                "sha256": hashlib.sha256(code).hexdigest()}],
                    "assembly": assembly, "xrefs_to_entry": assembly[0]["xrefs_to"], "pseudocode": None}
            reference = write(result / "functions" / (hex(rva)[2:] + ".json"), body)
            reference.update(path="functions/" + hex(rva)[2:] + ".json", rva=hex(rva), address=hex(start),
                            assembly_captured=True, pseudocode_captured=False)
            bodies.append((reference, body))
        capture = {"input": source.relative_to(root).as_posix(), "input_sha256": digest,
                   "input_bytes": len(raw), "image_base": hex(base), "capture_complete": True,
                   "functions": [reference for reference, _ in bodies],
                   "failures": [{"stage": "1000:hexrays", "error": "synthetic shared tail"}]}
        write(result / "capture.json", capture)
        return raw, result, capture, bodies, write

    def test_shared_tail_selection_keeps_both_entries_and_exact_tail_ownership(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            raw, result, capture, _, _ = self.shared_tail_fixture(Path(directory))
            before = (result / "capture.json").read_bytes()
            selected = analysis.native_function_selection(bytes(raw), capture, [0x1000],
                type_note="Complete shared-entry control-flow review",
                shared_tail_specs=["0x1000=0x1004:0x1007:0x1000,0x1040:inline"])
            shared = selected["function_shared_tails"][0]
            self.assertEqual(shared["owner_rvas"], ["0x1000", "0x1040"])
            self.assertEqual(shared["primary_owner_rva"], "0x1000")
            self.assertEqual(shared["expected_tail"]["start"], "0x140001004")
            self.assertEqual(shared["expected_tail"]["end"], "0x140001007")
            self.assertEqual([owner["expected_end"] for owner in shared["owners"]],
                             ["0x140001004", "0x140001045"])
            self.assertEqual([len(owner["expected_chunks"]) for owner in shared["owners"]], [2, 2])
            self.assertEqual([edge["from"] for edge in shared["incoming_edges"]],
                             ["0x140001003", "0x140001040"])
            self.assertEqual(analysis.native_shared_tail_specifications([shared]),
                             ["0x1000=0x1004:0x1007:0x1000,0x1040:inline"])
            recomputed = analysis.native_function_selection(raw, capture, [0x1000], shared_tail_specs=[shared])
            self.assertEqual(recomputed, selected)
            self.assertEqual((result / "capture.json").read_bytes(), before)

    def test_shared_tail_selection_rejects_interior_start_foreign_edge_wrong_owner_and_changed_bytes(self):
        issues = ("interior", "end", "foreign", "owner", "bytes", "duplicate", "source_hash",
                  "missing_instruction", "missing_incoming", "missing_outgoing", "call_entry", "outline_overlap")
        for issue in issues:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    raw, result, capture, bodies, write = self.shared_tail_fixture(root)
                    specs = ["0x1000=0x1004:0x1007:0x1000,0x1040:inline"]
                    if issue == "interior":
                        specs = ["0x1000=0x1005:0x1007:0x1000,0x1040:inline"]
                    elif issue == "end":
                        specs = ["0x1000=0x1004:0x1006:0x1000,0x1040:inline"]
                    elif issue == "owner":
                        specs = ["0x1000=0x1004:0x1007:0x1000:inline"]
                    elif issue == "duplicate":
                        specs *= 2
                    else:
                        body = bodies[0][1]
                        if issue == "foreign":
                            body["assembly"][3]["xrefs_to"].append({"from": "0x140001080", "to": "0x140001004",
                                                                       "type": 19, "code": True, "user": False})
                        elif issue == "bytes":
                            body["chunks"][0]["raw_hex"] = "90" * 7
                        elif issue == "missing_instruction":
                            body["assembly"].pop(3)
                        elif issue == "missing_incoming":
                            body["assembly"][3]["xrefs_to"].pop()
                        elif issue == "missing_outgoing":
                            body["assembly"][3]["xrefs_from"] = []
                        elif issue == "call_entry":
                            body["assembly"][3]["xrefs_to"][1]["type"] = 17
                        identity = write(result / "functions/1000.json", body)
                        if issue != "source_hash":
                            capture["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                        else:
                            capture["functions"][0]["sha256"] = "0" * 64
                    with self.assertRaises(ValueError):
                        analysis.native_function_selection(bytes(raw), capture, [0x1000],
                            type_note="Complete shared-entry control-flow review", shared_tail_specs=specs,
                            outline_rvas=[0x1040] if issue == "outline_overlap" else ())

    def shared_tail_reanalysis_fixture(self, root, inline=False):
        raw, result, capture, bodies, write = self.shared_tail_fixture(root)
        capture["failures"].append({"stage": "1040:hexrays", "error": "unselected helper has no pseudocode"})
        write(result / "capture.json", capture)
        mode = "inline" if inline else "retain"
        selection = analysis.native_function_selection(raw, capture, [0x1000],
            type_note="Complete shared-entry control-flow review",
            shared_tail_specs=["0x1000=0x1004:0x1007:0x1000,0x1040:" + mode])
        shared = selection["function_shared_tails"][0]
        output = result / "supplements/shared"
        primary, secondary = [json.loads(json.dumps(body)) for _, body in bodies]
        primary["end"] = shared["owners"][0]["expected_end"]
        primary["chunks"] = shared["owners"][0]["expected_chunks"]
        secondary["chunks"] = shared["owners"][1]["expected_chunks"]
        secondary["assembly"] = sorted(secondary["assembly"] + primary["assembly"][3:],
                                       key=lambda item: int(item["address"], 16))
        for body in (primary, secondary):
            body["flags"] = 0x40000 if inline else 0
        primary["pseudocode"] = {"lines": [{"line": 0, "text": "int recovered(void) { return 0; }"}], "locals": []}
        self.typed_fixture(primary)
        function = write(output / "functions/1000.json", primary)
        function.update(path="functions/1000.json", rva="0x1000", address=primary["address"],
                        assembly_captured=True, pseudocode_captured=True)
        helper = write(output / "shared-tail-helpers/1040.json", secondary)
        helper.update(path="shared-tail-helpers/1040.json", rva="0x1040", address=secondary["address"],
                      assembly_captured=True, pseudocode_captured=False)
        database = root / ".work/native" / capture["input_sha256"] / "batch/shared.dll.i64"
        database.parent.mkdir(parents=True)
        database.write_bytes(b"immutable synthetic retained IDB")
        database_ref = {"path": analysis.rel(database), "sha256": analysis.sha(database)}
        capture_ref = {"path": analysis.rel(result / "capture.json"), "sha256": analysis.sha(result / "capture.json")}
        applied = {**shared, "base_database_modified": False, "applied_tail": shared["expected_tail"],
                   "applied_primary_owner_rva": shared["primary_owner_rva"], "applied_owner_rvas": shared["owner_rvas"],
                   "applied_owners": [{"rva": owner["rva"], "chunks": owner["expected_chunks"],
                        "end": owner["expected_end"], "previous_flags": owner["expected_flags"],
                        "applied_flags": 0x40000 if inline else 0, "outline_flag": 0x40000,
                        "previous_frame": {"frame_id": 0, "local_bytes": 0, "saved_register_bytes": 0,
                                           "argument_bytes": 0, "frame_pointer_delta": 0},
                        "applied_frame": {"frame_id": 0, "local_bytes": 0, "saved_register_bytes": 0,
                                          "argument_bytes": 0, "frame_pointer_delta": 0},
                        "previous_stack_points": [{"address": hex(0x140000000 + int(owner["rva"], 16)), "delta": 0}],
                        "applied_stack_points": [{"address": hex(0x140000000 + int(owner["rva"], 16)), "delta": 0},
                                                 {"address": shared["expected_tail"]["start"], "delta": -4}],
                        "previous_type": None, "applied_type": None,
                        "base_database_modified": False} for owner in shared["owners"]]}
        supplement = {"input_sha256": capture["input_sha256"], "base_capture": capture_ref,
            "capture_complete": True, "all_pseudocode_captured": True, "failures": [], "database_integrity": {"verified": True},
            "base_capture_unchanged": True, "base_database_unchanged": True, "target_execution": False, "capture_tool": {},
            "source_database": database_ref, "function_selection": selection, "functions": [function],
            "type_recoveries": [], "function_shared_tail_recoveries": [applied], "shared_tail_helpers": [helper],
            "ida_version": "synthetic", "hexrays_version": "synthetic"}
        write(output / "capture.json", supplement)
        write(output / "session.json", {"passed": True, "exit_code": 0, "capture_tool": {}})
        write(output / "plan.json", {"function_selection": selection, "base_capture": capture_ref,
                                      "source_database": database_ref})
        return result, output, capture, supplement, primary, secondary, database, write

    def test_shared_tail_resolution_keeps_unselected_failure_and_requires_every_owner_and_assembly_snapshot(self):
        issues = (None, "missing_tail", "wrong_primary", "missing_owner", "duplicate_owner", "tail_bytes",
                  "primary_end", "owner_chunks", "original_flags", "applied_flags", "missing_helper", "duplicate_helper",
                  "helper_bytes", "helper_xrefs", "helper_flags", "helper_pseudocode", "parent_failed",
                  "parent_instruction", "parent_xrefs", "parent_flags", "modified_base", "database", "retained_source",
                  "frame_changed", "frame_invalid", "type_changed", "type_invalid", "stack_changed",
                  "stack_missing", "stack_foreign", "stack_duplicate")
        for inline in (False, True):
            for issue in issues:
                with self.subTest(inline=inline, issue=issue), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    with patch.object(analysis, "ROOT", root):
                        result, output, capture, supplement, parent, helper, database, write = self.shared_tail_reanalysis_fixture(root, inline)
                        before = (result / "capture.json").read_bytes()
                        applied = supplement["function_shared_tail_recoveries"][0]
                        if issue == "missing_tail": supplement["function_shared_tail_recoveries"] = []
                        elif issue == "wrong_primary": applied["applied_primary_owner_rva"] = "0x1040"
                        elif issue == "missing_owner": applied["applied_owner_rvas"] = ["0x1000"]
                        elif issue == "duplicate_owner": applied["applied_owners"].append(dict(applied["applied_owners"][0]))
                        elif issue == "tail_bytes": applied["applied_tail"] = {**applied["applied_tail"], "raw_hex": "9090c3"}
                        elif issue == "primary_end": applied["applied_owners"][0]["end"] = "0x140001007"
                        elif issue == "owner_chunks": applied["applied_owners"][0]["chunks"] = []
                        elif issue == "original_flags": applied["applied_owners"][0]["previous_flags"] = 1
                        elif issue == "applied_flags": applied["applied_owners"][0]["applied_flags"] ^= 0x40000
                        elif issue == "frame_changed": applied["applied_owners"][0]["applied_frame"]["local_bytes"] = 8
                        elif issue == "frame_invalid": applied["applied_owners"][0]["previous_frame"]["local_bytes"] = -1
                        elif issue == "type_changed": applied["applied_owners"][0]["applied_type"] = "int invented(void);"
                        elif issue == "type_invalid": applied["applied_owners"][0]["previous_type"] = False
                        elif issue == "stack_changed": applied["applied_owners"][0]["applied_stack_points"][0]["delta"] = -8
                        elif issue == "stack_missing": applied["applied_owners"][0]["applied_stack_points"].pop(0)
                        elif issue == "stack_foreign": applied["applied_owners"][0]["applied_stack_points"].append(
                            {"address": "0x140001080", "delta": -8})
                        elif issue == "stack_duplicate": applied["applied_owners"][0]["applied_stack_points"].append(
                            dict(applied["applied_owners"][0]["applied_stack_points"][0]))
                        elif issue == "missing_helper": supplement["shared_tail_helpers"] = []
                        elif issue == "duplicate_helper": supplement["shared_tail_helpers"] *= 2
                        elif issue == "modified_base": applied["base_database_modified"] = True
                        elif issue == "database": database.write_bytes(b"changed retained IDB")
                        elif issue == "retained_source": (result / "functions/1040.json").write_bytes(b"changed source evidence")
                        elif issue in {"helper_bytes", "helper_xrefs", "helper_flags", "helper_pseudocode"}:
                            if issue == "helper_bytes": helper["assembly"].pop()
                            elif issue == "helper_xrefs": helper["assembly"][0]["xrefs_to"] = []
                            elif issue == "helper_flags": helper["flags"] ^= 0x40000
                            else: helper["pseudocode"] = {"lines": [{"text": "invented"}], "locals": []}
                            identity = write(output / "shared-tail-helpers/1040.json", helper)
                            supplement["shared_tail_helpers"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                        elif issue in {"parent_failed", "parent_instruction", "parent_xrefs", "parent_flags"}:
                            if issue == "parent_failed": parent["pseudocode"] = None
                            elif issue == "parent_instruction": parent["assembly"].pop()
                            elif issue == "parent_xrefs": parent["assembly"][3]["xrefs_to"] = []
                            else: parent["flags"] ^= 0x40000
                            identity = write(output / "functions/1000.json", parent)
                            supplement["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                        write(output / "capture.json", supplement)
                        resolved = analysis.native_reanalysis_resolution(result)
                        self.assertEqual(bool(resolved["verification_failures"]), issue is not None)
                        self.assertEqual(len(resolved["resolved_failures"]), 0 if issue else 1)
                        self.assertEqual(set(resolved["complete_functions"]), set() if issue else {"0x1000"})
                        self.assertEqual(resolved["remaining_failures"], capture["failures"] if issue else [capture["failures"][1]])
                        self.assertEqual((result / "capture.json").read_bytes(), before)

    def import_fixture(self):
        raw = self.fixture()
        raw[0x300:0x400] = bytes(0x100)
        struct.pack_into("<I", raw, 0x98 + 108, 16)
        struct.pack_into("<II", raw, 0x98 + 120, 0x2000, 40)
        struct.pack_into("<IIIII", raw, 0x300, 0x2030, 0, 0, 0x2050, 0x2040)
        struct.pack_into("<QQ", raw, 0x330, 0x2070, 0)
        struct.pack_into("<QQ", raw, 0x340, 0x2070, 0)
        raw[0x350:0x360] = b"WppRecorder.sys\0"
        raw[0x370:0x382] = b"\0\0WppAutoLogTrace\0"
        return bytes(raw)

    def test_import_recovery_binds_name_slot_bytes_and_reviewed_variadic_declaration(self):
        raw = self.import_fixture()
        declaration = "long (__cdecl *WppAutoLogTrace)(void *, unsigned char, unsigned long, void *, unsigned short, ...);"
        result = analysis.native_import_type_recoveries(raw, ["0x2040=" + declaration], "Exact caller registers and imported official variadic ABI")
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["library"], "WppRecorder.sys")
        self.assertEqual(result[0]["name"], "WppAutoLogTrace")
        self.assertEqual(result[0]["raw_hex"], raw[0x340:0x348].hex())
        self.assertEqual(result[0]["bytes"], 8)
        capture = {"functions": [{"rva": "0x1000", "address": "0x140001000"}]}
        selected = analysis.native_function_selection(raw, capture, [0x1000], type_note="Reviewed ABI",
            import_specs=["0x2040=" + declaration])
        self.assertEqual(selected["import_type_recoveries"][0]["declaration"], declaration)

    def test_import_recovery_rejects_non_iat_duplicate_ordinal_and_unreviewed_declarations(self):
        raw = self.import_fixture()
        specification = "0x2040=long (__cdecl *WppAutoLogTrace)(void *, ...);"
        for specifications, note in [([specification], None), ([specification] * 2, "reviewed"),
                ([specification.replace("0x2040", "0x2030")], "reviewed"),
                ([specification.replace("0x2040", "0x2048")], "reviewed"),
                (["0x2040="], "reviewed")]:
            with self.subTest(specifications=specifications), self.assertRaises(ValueError):
                analysis.native_import_type_recoveries(raw, specifications, note)
        ordinal = bytearray(raw)
        struct.pack_into("<Q", ordinal, 0x330, (1 << 63) | 7)
        with self.assertRaises(ValueError):
            analysis.native_import_type_recoveries(bytes(ordinal), [specification], "reviewed")

    def test_import_binding_keeps_loaded_bytes_distinct_but_requires_exact_symbol_slot_and_width(self):
        raw = self.import_fixture()
        request = analysis.native_import_type_recoveries(raw,
            ["0x2040=long (__cdecl *WppAutoLogTrace)(void *, ...);"], "reviewed ABI")[0]
        binding = {"rva": "0x2040", "name": "WppAutoLogTrace", "module": "WppRecorder",
                   "ordinal": 0, "loaded_slot_raw_hex": "ffffffffffffffff"}
        self.assertNotEqual(binding["loaded_slot_raw_hex"], request["raw_hex"])
        self.assertTrue(analysis.native_import_binding_matches(request, binding))
        for field, wrong in [("rva", "0x2048"), ("name", "WrongFunction"), ("name", None),
                ("module", "WrongRecorder.sys"), ("module", None),
                ("loaded_slot_raw_hex", "ffffffff"), ("loaded_slot_raw_hex", "not-hex")]:
            with self.subTest(field=field, wrong=wrong):
                self.assertFalse(analysis.native_import_binding_matches(request, {**binding, field: wrong}))

    def import_thunk_fixture(self, root, architecture="x64"):
        raw = bytearray(self.import_fixture())
        base = 0x140000000
        if architecture == "x86":
            sections = raw[0x188:0x1d8]
            struct.pack_into("<H", raw, 0x84, 0x14c)
            struct.pack_into("<H", raw, 0x94, 0xe0)
            struct.pack_into("<H", raw, 0x98, 0x10b)
            struct.pack_into("<II", raw, 0x98 + 24, 0x2000, 0x400000)
            struct.pack_into("<I", raw, 0x98 + 92, 16)
            struct.pack_into("<II", raw, 0x98 + 104, 0x2000, 40)
            raw[0x98 + 120:0x98 + 128] = bytes(8)
            raw[0x178:0x1c8] = sections
            struct.pack_into("<II", raw, 0x330, 0x2070, 0)
            struct.pack_into("<II", raw, 0x340, 0x2070, 0)
            base = 0x400000
        for rva in (0x1000, 0x1010):
            operand = 0x2040 - rva - 6 if architecture == "x64" else base + 0x2040
            raw[0x200 + rva - 0x1000:0x206 + rva - 0x1000] = b"\xff\x25" + struct.pack("<I", operand)
        raw = bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        result = root / ".analysis/results/native" / digest

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            data = json.dumps(value).encode()
            path.write_bytes(data)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest()}

        functions, bodies = [], []
        for rva in (0x1000, 0x1010):
            at = 0x200 + rva - 0x1000
            code = raw[at:at + 6].hex()
            body = {"address": hex(base + rva), "rva": hex(rva), "end": hex(base + rva + 6),
                    "chunks": [{"start": hex(base + rva), "end": hex(base + rva + 6), "raw_hex": code,
                                "sha256": hashlib.sha256(raw[at:at + 6]).hexdigest()}],
                    "assembly": [{"address": hex(base + rva), "rva": hex(rva), "bytes": code,
                                  "text": "jmp cs:__imp_WppAutoLogTrace"}],
                    "xrefs_to_entry": [], "pseudocode": None}
            reference = write(result / "functions" / (format(rva, "x") + ".json"), body)
            reference.update(path="functions/" + format(rva, "x") + ".json", rva=hex(rva), address=hex(base + rva))
            functions.append(reference)
            bodies.append(body)
        original = root / ".analysis/inputs/native/fixture.dll"
        original.parent.mkdir(parents=True)
        original.write_bytes(raw)
        capture = {"input": original.relative_to(root).as_posix(), "input_bytes": len(raw),
                   "input_sha256": digest, "functions": functions,
                   "failures": [{"stage": "1000:hexrays", "error": "call analysis -12"}]}
        return raw, result, capture, bodies, write

    def test_import_thunks_bind_both_pe_architectures_and_every_exact_same_slot_entry(self):
        for architecture, expected in [("x64", ["ff253a100000", "ff252a100000"]),
                                       ("x86", ["ff2540204000", "ff2540204000"])]:
            with self.subTest(architecture=architecture), tempfile.TemporaryDirectory() as directory:
                with patch.object(analysis, "ROOT", Path(directory)):
                    raw, result, capture, _, _ = self.import_thunk_fixture(Path(directory), architecture)
                    selected = analysis.native_function_selection(raw, capture, [0x1000], type_note="reviewed ABI",
                        import_specs=["0x2040=long (__cdecl *WppAutoLogTrace)(void *, ...);"])
                    self.assertEqual([row["raw_hex"] for row in selected["import_thunks"]], expected)
                    self.assertEqual([row["rva"] for row in selected["import_thunks"]], ["0x1000", "0x1010"])
                    for row in selected["import_thunks"]:
                        self.assertEqual((row["slot_rva"], row["library"], row["name"], row["bytes"]),
                                         ("0x2040", "WppRecorder.sys", "WppAutoLogTrace", 6))
                        data = (Path(directory) / row["source_function"]["path"]).read_bytes()
                        self.assertEqual(row["source_function"]["bytes"], len(data))
                        self.assertEqual(row["source_function"]["sha256"], hashlib.sha256(data).hexdigest())
                    (result / capture["functions"][0]["path"]).write_bytes(b"changed evidence")
                    plain = analysis.native_function_selection(raw, capture, [0x1000])
                    self.assertNotIn("import_thunks", plain)
                    self.assertNotIn("import_type_recoveries", plain)

    def test_import_thunks_reject_changed_slot_boundary_chunks_instruction_and_source_identity(self):
        for architecture in ("x64", "x86"):
            for issue in ("slot", "end", "chunks", "instructions", "chunk_hash", "source_hash", "entry"):
                with self.subTest(architecture=architecture, issue=issue), tempfile.TemporaryDirectory() as directory:
                    with patch.object(analysis, "ROOT", Path(directory)):
                        raw, result, capture, bodies, write = self.import_thunk_fixture(Path(directory), architecture)
                        body = bodies[0]
                        if issue == "slot":
                            body["chunks"][0]["raw_hex"] = "ff2548204000"
                            body["assembly"][0]["bytes"] = "ff2548204000"
                        elif issue == "end":
                            body["end"] = hex(int(body["end"], 16) + 1)
                        elif issue == "chunks":
                            body["chunks"].append(dict(body["chunks"][0]))
                        elif issue == "instructions":
                            body["assembly"].append(dict(body["assembly"][0]))
                        elif issue == "chunk_hash":
                            body["chunks"][0]["sha256"] = "0" * 64
                        elif issue == "entry":
                            body["rva"] = "0x1001"
                        reference = write(result / capture["functions"][0]["path"], body)
                        if issue != "source_hash":
                            capture["functions"][0]["sha256"] = reference["sha256"]
                            capture["functions"][0]["bytes"] = reference["bytes"]
                        else:
                            capture["functions"][0]["sha256"] = "0" * 64
                        with self.assertRaises(ValueError):
                            analysis.native_function_selection(raw, capture, [0x1000], type_note="reviewed ABI",
                                import_specs=["0x2040=long (__cdecl *WppAutoLogTrace)(void *, ...);"])

    def merge_fixture(self, root):
        raw = self.fixture()
        raw[0x200:0x206] = bytes.fromhex("55908bec5dc3")
        raw = bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        result = root / ".analysis/results/native" / digest

        def write(path, data):
            path.parent.mkdir(parents=True, exist_ok=True)
            payload = json.dumps(data).encode()
            path.write_bytes(payload)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(payload),
                    "sha256": hashlib.sha256(payload).hexdigest()}

        source = root / ".analysis/inputs/native/merge.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        capture = {"input": source.relative_to(root).as_posix(), "input_sha256": digest,
                   "input_bytes": len(raw), "image_base": "0x140000000", "capture_complete": True,
                   "functions": [], "failures": [{"stage": "1000:hexrays", "error": "synthetic owner"},
                                                    {"stage": "1002:hexrays", "error": "synthetic member"}]}
        for rva, code, instructions in [(0x1000, "5590", [(0, "55")]),
                                        (0x1002, "8bec", [(0, "8bec")]),
                                        (0x1004, "5dc3", [(0, "5d"), (1, "c3")])]:
            start = 0x140000000 + rva
            body = {"address": hex(start), "rva": hex(rva), "end": hex(start + 2),
                    "chunks": [{"start": hex(start), "end": hex(start + 2), "raw_hex": code,
                                "sha256": hashlib.sha256(bytes.fromhex(code)).hexdigest()}],
                    "assembly": [{"address": hex(start + offset), "rva": hex(rva + offset), "bytes": value}
                                 for offset, value in instructions],
                    "xrefs_to_entry": [{"from": "0x140001080", "to": hex(start), "code": True, "type": 17}]
                    if rva == 0x1000 else [], "pseudocode": None}
            record = write(result / "functions" / (hex(rva)[2:] + ".json"), body)
            record.update(path="functions/" + hex(rva)[2:] + ".json", rva=hex(rva), address=hex(start),
                          assembly_captured=True, pseudocode_captured=False)
            capture["functions"].append(record)
        write(result / "capture.json", capture)
        return raw, result, capture, write

    def partition_fixture(self, root):
        raw = self.fixture()
        # The outer jump skips a real local XOR/RET helper. Its one padding byte
        # is retained in the outer chunk although no instruction enumerates it.
        raw[0x200:0x20c] = bytes.fromhex("eb0431c0c390e8f7ffffffc3")
        raw = bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        result = root / ".analysis/results/native" / digest

        def write(path, data):
            path.parent.mkdir(parents=True, exist_ok=True)
            payload = json.dumps(data).encode()
            path.write_bytes(payload)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(payload),
                    "sha256": hashlib.sha256(payload).hexdigest()}

        source = root / ".analysis/inputs/native/partition.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        base = 0x140000000

        def xref(first, target, kind):
            return {"from": hex(base + first), "to": hex(base + target), "type": kind,
                    "code": True, "user": False}

        rows = [(0x1000, "eb04", "jmp     loc_140001006", [xref(0x1000, 0x1006, 19)], []),
                (0x1002, "31c0", "xor     eax, eax", [xref(0x1002, 0x1004, 21)], [xref(0x1006, 0x1002, 17)]),
                (0x1004, "c3", "retn", [], [xref(0x1002, 0x1004, 21)]),
                (0x1006, "e8f7ffffff", "call    loc_140001002", [xref(0x1006, 0x1002, 17), xref(0x1006, 0x100b, 21)], [xref(0x1000, 0x1006, 19)]),
                (0x100b, "c3", "retn", [], [xref(0x1006, 0x100b, 21)])]
        body = {"address": hex(base + 0x1000), "rva": "0x1000", "end": hex(base + 0x100c), "flags": 0,
            "chunks": [{"start": hex(base + 0x1000), "end": hex(base + 0x100c),
                        "raw_hex": raw[0x200:0x20c].hex(), "sha256": hashlib.sha256(raw[0x200:0x20c]).hexdigest()}],
            "assembly": [{"address": hex(base + at), "rva": hex(at), "bytes": code, "text": text,
                          "xrefs_from": outgoing, "xrefs_to": incoming} for at, code, text, outgoing, incoming in rows],
            "xrefs_to_entry": [], "pseudocode": None}
        record = write(result / "functions/1000.json", body)
        record.update(path="functions/1000.json", rva="0x1000", address=body["address"],
                      assembly_captured=True, pseudocode_captured=False)
        capture = {"input": source.relative_to(root).as_posix(), "input_sha256": digest, "input_bytes": len(raw),
                   "capture_complete": True, "image_base": hex(base), "functions": [record],
                   "failures": [{"stage": "1000:hexrays", "error": "synthetic positive-SP"}]}
        write(result / "capture.json", capture)
        return raw, result, capture, body, write

    def test_partition_selection_preserves_padding_call_edges_and_exact_complete_byte_union(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            raw, result, capture, body, _ = self.partition_fixture(Path(directory))
            before = (result / "capture.json").read_bytes()
            selection = analysis.native_function_selection(raw, capture, [0x1000], type_note="Complete local CALL/RET review",
                partition_specs=["0x1000=0x1002:0x1005"])
            partition = selection["function_partitions"][0]
            pieces = sorted(partition["owner_chunks"] + partition["helpers"][0]["expected_chunks"], key=lambda c: c["start"])
            self.assertEqual("".join(c["raw_hex"] for c in pieces), body["chunks"][0]["raw_hex"])
            self.assertEqual(partition["owner_chunks"][1]["raw_hex"], "90e8f7ffffffc3")
            self.assertEqual(partition["helpers"][0]["call_edges"], [{"rva": "0x1006", "target_rva": "0x1002", "bytes": "e8f7ffffff"}])
            baselines = analysis.native_function_partition_bodies(partition)
            self.assertEqual(baselines["owner"]["end"], "0x140001002")
            self.assertEqual([row["rva"] for row in baselines["owner"]["assembly"]], ["0x1000", "0x1006", "0x100b"])
            self.assertEqual([row["rva"] for row in baselines["helpers"]["0x1002"]["assembly"]], ["0x1002", "0x1004"])
            self.assertEqual((result / "capture.json").read_bytes(), before)

    def test_partition_selection_rejects_changed_code_bad_boundaries_and_unclosed_control_flow(self):
        issues = ("source_hash", "chunk", "instruction", "interior", "overlap", "duplicate_owner", "unselected",
                  "no_note", "missing_refs", "nonlocal", "interior_entry", "call_bytes", "terminal")
        for issue in issues:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    raw, result, capture, body, write = self.partition_fixture(root)
                    specs, selected, note = ["0x1000=0x1002:0x1005"], [0x1000], "Explicit complete-byte review"
                    if issue == "interior": specs = ["0x1000=0x1003:0x1005"]
                    elif issue == "overlap": specs = ["0x1000=0x1002:0x1005,0x1002:0x1005"]
                    elif issue == "duplicate_owner": specs *= 2
                    elif issue == "unselected": selected = [0x1001]
                    elif issue == "no_note": note = None
                    else:
                        if issue == "chunk": body["chunks"][0]["raw_hex"] = "90" * 12
                        elif issue == "instruction": body["assembly"][1]["bytes"] = "9090"
                        elif issue == "missing_refs": del body["assembly"][1]["xrefs_to"]
                        elif issue == "nonlocal": body["assembly"][1]["xrefs_from"][0]["to"] = "0x140001100"
                        elif issue == "interior_entry": body["assembly"][2]["xrefs_to"].append(body["assembly"][1]["xrefs_to"][0])
                        elif issue == "call_bytes": body["assembly"][1]["xrefs_to"][0]["from"] = "0x140001000"
                        elif issue == "terminal": body["assembly"][1]["xrefs_from"] = []
                        identity = write(result / "functions/1000.json", body)
                        if issue != "source_hash": capture["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                        else: capture["functions"][0]["sha256"] = "0" * 64
                    with self.assertRaises(ValueError):
                        analysis.native_function_selection(raw, capture, selected, type_note=note, partition_specs=specs)

    def partition_reanalysis_fixture(self, root):
        raw, result, capture, _, write = self.partition_fixture(root)
        selection = analysis.native_function_selection(raw, capture, [0x1000], type_note="Complete reviewed local CALL/RET partition",
            partition_specs=["0x1000=0x1002:0x1005"])
        partition = selection["function_partitions"][0]
        baselines = analysis.native_function_partition_bodies(partition)
        body = baselines["owner"]
        body["pseudocode"] = {"lines": [{"text": "void owner() { /* recovered original operations */ }"}], "locals": []}
        self.typed_fixture(body)
        supplemental = result / "supplements/partitioned"
        record = write(supplemental / "functions/1000.json", body)
        record.update(path="functions/1000.json", rva="0x1000", address=body["address"], assembly_captured=True, pseudocode_captured=True)
        helper = baselines["helpers"]["0x1002"]
        helper["flags"] = 0x40000
        helper_record = write(supplemental / "partition-helpers/1002.json", helper)
        helper_record.update(path="partition-helpers/1002.json", rva="0x1002", address=helper["address"],
                             assembly_captured=True, pseudocode_captured=False)
        capture_ref = {"path": analysis.rel(result / "capture.json"), "sha256": analysis.sha(result / "capture.json")}
        supplement = {"input_sha256": capture["input_sha256"], "base_capture": capture_ref,
            "capture_complete": True, "all_pseudocode_captured": True, "failures": [], "database_integrity": {"verified": True},
            "base_capture_unchanged": True, "base_database_unchanged": True, "target_execution": False, "capture_tool": {},
            "function_selection": selection, "functions": [record], "type_recoveries": [],
            "partition_helpers": [helper_record], "function_partition_recoveries": [{**partition,
                "base_database_modified": False, "applied_owner_chunks": partition["owner_chunks"], "applied_helpers": [{
                    "rva": "0x1002", "chunks": partition["helpers"][0]["expected_chunks"], "previous_flags": 0,
                    "applied_flags": 0x40000, "outline_flag": 0x40000, "base_database_modified": False}]}],
            "ida_version": "synthetic", "hexrays_version": "synthetic"}
        write(supplemental / "capture.json", supplement)
        write(supplemental / "session.json", {"passed": True, "exit_code": 0, "capture_tool": {}})
        write(supplemental / "plan.json", {"function_selection": selection, "base_capture": capture_ref})
        return result, supplemental, capture, supplement, body, helper, write

    def test_partition_resolution_rejects_omitted_helpers_union_changes_flags_and_pseudocode_claims(self):
        issues = (None, "missing_partition", "base", "owner_chunks", "helper_chunks", "flags", "missing_helper",
                  "duplicate_helper", "helper_bytes", "helper_xrefs", "helper_pseudocode", "owner_bytes", "source")
        for issue in issues:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    result, output, capture, supplement, owner, helper, write = self.partition_reanalysis_fixture(root)
                    applied = supplement["function_partition_recoveries"][0]
                    if issue == "missing_partition": supplement["function_partition_recoveries"] = []
                    elif issue == "base": applied["base_database_modified"] = True
                    elif issue == "owner_chunks": applied["applied_owner_chunks"] = []
                    elif issue == "helper_chunks": applied["applied_helpers"][0]["chunks"] = []
                    elif issue == "flags": applied["applied_helpers"][0]["applied_flags"] = 0
                    elif issue == "missing_helper": supplement["partition_helpers"] = []
                    elif issue == "duplicate_helper": supplement["partition_helpers"] *= 2
                    elif issue == "source": (result / "functions/1000.json").write_bytes(b"changed retained original")
                    elif issue in {"helper_bytes", "helper_xrefs", "helper_pseudocode"}:
                        if issue == "helper_bytes": helper["assembly"][0]["bytes"] = "9090"
                        elif issue == "helper_xrefs": helper["xrefs_to_entry"] = []
                        else: helper["pseudocode"] = {"lines": [{"text": "invented"}], "locals": []}
                        identity = write(output / "partition-helpers/1002.json", helper)
                        supplement["partition_helpers"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                    elif issue == "owner_bytes":
                        owner["assembly"].pop()
                        identity = write(output / "functions/1000.json", owner)
                        supplement["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                    write(output / "capture.json", supplement)
                    resolved = analysis.native_reanalysis_resolution(result)
                    self.assertEqual(bool(resolved["verification_failures"]), bool(issue))
                    self.assertEqual(len(resolved["resolved_failures"]), 0 if issue else 1)
                    self.assertEqual(set(resolved["complete_functions"]), set() if issue else {"0x1000"})

    def outline_fixture(self, root):
        raw, result, capture, write = self.merge_fixture(root)
        for row in capture["functions"]:
            path = result / row["path"]
            body = json.loads(path.read_bytes())
            body["flags"] = 0
            record = write(path, body)
            row.update(sha256=record["sha256"], bytes=record["bytes"])
        write(result / "capture.json", capture)
        return raw, result, capture, write

    def fresh_inline_fixture(self, root):
        raw = bytearray(self.fixture())
        raw[0x200:0x206] = bytes.fromhex("554889e5c9c3")
        raw[0x240:0x24c] = bytes.fromhex("e8bbffffff90e8b5ffffffc3")
        raw, base = bytes(raw), 0x140000000
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        source = root / ".analysis/inputs/native/fresh.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            data = json.dumps(value).encode()
            path.write_bytes(data)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest()}

        def edge(at):
            return {"from": hex(base + at), "to": hex(base + 0x1000), "type": 17, "code": True, "user": False}

        functions = []
        for rva, instructions in ((0x1000, [(0, "55"), (1, "4889e5"), (4, "c9"), (5, "c3")]),
                                  (0x1040, [(0, "e8bbffffff"), (5, "90"), (6, "e8b5ffffff"), (11, "c3")])):
            length = 6 if rva == 0x1000 else 12
            code = raw[0x200 + rva - 0x1000:0x200 + rva - 0x1000 + length]
            assembly = [{"address": hex(base + rva + at), "rva": hex(rva + at), "bytes": value,
                         "text": "retained instruction", "xrefs_from": [edge(rva + at)] if value.startswith("e8") else [],
                         "xrefs_to": [edge(0x1040), edge(0x1046)] if rva == 0x1000 and at == 0 else []}
                        for at, value in instructions]
            body = {"address": hex(base + rva), "rva": hex(rva), "end": hex(base + rva + length), "flags": 0,
                    "chunks": [{"start": hex(base + rva), "end": hex(base + rva + length), "raw_hex": code.hex(),
                                "sha256": hashlib.sha256(code).hexdigest()}],
                    "assembly": assembly, "xrefs_to_entry": assembly[0]["xrefs_to"], "pseudocode": None}
            ref = write(result / "functions" / (hex(rva)[2:] + ".json"), body)
            functions.append({**ref, "path": "functions/" + hex(rva)[2:] + ".json", "rva": hex(rva),
                              "address": body["address"], "assembly_captured": True, "pseudocode_captured": False})
        capture = {"input": source.relative_to(root).as_posix(), "input_sha256": hashlib.sha256(raw).hexdigest(),
                   "input_bytes": len(raw), "image_base": hex(base), "capture_complete": True, "functions": functions,
                   "failures": [{"stage": "1040:hexrays", "error": "fixture"}]}
        write(result / "capture.json", capture)
        return raw, result, capture, write

    def test_fresh_inline_selection_requires_original_direct_calls_and_reviewed_complete_helpers(self):
        for issue in (None, "unoutlined", "duplicate", "missing_parent", "parent_bytes", "helper_edge"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                raw, result, capture, write = self.fresh_inline_fixture(Path(directory))
                selected, outlines, fresh = [0x1040], [0x1000], [0x1000]
                if issue == "unoutlined": outlines = []
                elif issue == "duplicate": fresh *= 2
                elif issue == "missing_parent": selected = [0x1000]
                elif issue in {"parent_bytes", "helper_edge"}:
                    record = capture["functions"][1 if issue == "parent_bytes" else 0]
                    body = json.loads((result / record["path"]).read_bytes())
                    if issue == "parent_bytes": body["assembly"][0]["bytes"] = "e8baffffff"
                    else: body["xrefs_to_entry"].pop()
                    identity = write(result / record["path"], body)
                    record.update(bytes=identity["bytes"], sha256=identity["sha256"])
                if issue:
                    with self.assertRaises(ValueError):
                        analysis.native_function_selection(raw, capture, selected, type_note="reviewed own frame and clone cache",
                                                          outline_rvas=outlines, fresh_inline_rvas=fresh)
                else:
                    before = (result / "capture.json").read_bytes()
                    selection = analysis.native_function_selection(raw, capture, selected, type_note="reviewed own frame and clone cache",
                                                                  outline_rvas=outlines, fresh_inline_rvas=fresh)
                    helper = selection["function_fresh_inlines"][0]
                    self.assertEqual(helper["flag_names"], ["INLINE_EXTFRAME", "INLINE_DONTCOPY"])
                    self.assertEqual([c["callsite_rva"] for c in helper["callsites"]], ["0x1040", "0x1046"])
                    self.assertEqual([c["raw_hex"] for c in helper["callsites"]], ["e8bbffffff", "e8b5ffffff"])
                    self.assertEqual((result / "capture.json").read_bytes(), before)

    def test_nested_register_leaf_selection_binds_each_original_outer_and_inner_call(self):
        for issue in (None, "stack_frame", "nested_call", "unoutlined_leaf", "missing_outer",
                      "leaf_edge", "wrong_flags", "duplicate_leaf"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                raw, result, capture, write = self.fresh_inline_fixture(root)
                data = bytearray(raw)
                data[0x200:0x206] = bytes.fromhex("e87b000000c3")
                leaf_code = bytes.fromhex("31c0c3")
                if issue == "stack_frame": leaf_code = bytes.fromhex("554889e5c9c3")
                elif issue == "nested_call": leaf_code = bytes.fromhex("e87bffffffc3")
                data[0x280:0x280 + len(leaf_code)] = leaf_code
                raw = bytes(data)
                destination = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
                destination.mkdir(parents=True)
                base = 0x140000000
                inner_edge = {"from": hex(base + 0x1000), "to": hex(base + 0x1080),
                              "type": 17, "code": True, "user": False}
                for record in capture["functions"]:
                    body = json.loads((result / record["path"]).read_bytes())
                    if record["rva"] == "0x1000":
                        body["chunks"][0].update(raw_hex="e87b000000c3",
                            sha256=hashlib.sha256(bytes.fromhex("e87b000000c3")).hexdigest())
                        body["assembly"] = [{"address": hex(base + 0x1000), "rva": "0x1000",
                            "bytes": "e87b000000", "text": "call nested_leaf", "xrefs_from": [inner_edge],
                            "xrefs_to": body["xrefs_to_entry"]}, {"address": hex(base + 0x1005),
                            "rva": "0x1005", "bytes": "c3", "text": "retn", "xrefs_from": [], "xrefs_to": []}]
                    identity = write(destination / record["path"], body)
                    record.update(bytes=identity["bytes"], sha256=identity["sha256"])
                texts = (["xor eax, eax", "retn"] if issue not in {"stack_frame", "nested_call"}
                         else ["push rbp", "mov rbp, rsp", "leave", "retn"] if issue == "stack_frame"
                         else ["call another", "retn"])
                codes = (["31c0", "c3"] if issue not in {"stack_frame", "nested_call"}
                         else ["55", "4889e5", "c9", "c3"] if issue == "stack_frame"
                         else ["e87bffffff", "c3"])
                at, instructions = base + 0x1080, []
                for text, code in zip(texts, codes):
                    instructions.append({"address": hex(at), "rva": hex(at - base), "bytes": code,
                        "text": text, "xrefs_from": [], "xrefs_to": []})
                    at += len(bytes.fromhex(code))
                body = {"address": hex(base + 0x1080), "rva": "0x1080", "end": hex(at), "flags": 0,
                    "chunks": [{"start": hex(base + 0x1080), "end": hex(at), "raw_hex": leaf_code.hex(),
                                "sha256": hashlib.sha256(leaf_code).hexdigest()}], "assembly": instructions,
                    "xrefs_to_entry": [] if issue == "leaf_edge" else [inner_edge], "pseudocode": None}
                identity = write(destination / "functions/1080.json", body)
                capture["functions"].append({**identity, "path": "functions/1080.json", "rva": "0x1080",
                    "address": body["address"], "assembly_captured": True, "pseudocode_captured": False})
                capture.update(input_sha256=hashlib.sha256(raw).hexdigest(), input_bytes=len(raw))
                (root / capture["input"]).write_bytes(raw)
                write(destination / "capture.json", capture)
                leaf = {"rva": "0x1080", "inline_scope": "reviewed_nested_register_leaf",
                        "flag_names": ["INLINE_DONTCOPY"]}
                if issue == "wrong_flags": leaf["flag_names"] = ["INLINE_EXTFRAME", "INLINE_DONTCOPY"]
                specs = ([] if issue == "missing_outer" else [0x1000]) + [leaf]
                if issue == "duplicate_leaf": specs.append(dict(leaf))
                outlines = [0x1000] + ([] if issue == "unoutlined_leaf" else [0x1080])
                pe = analysis.native_function_selection(raw, capture, [0x1040], type_note="complete register leaf review",
                                                        outline_rvas=outlines)
                from host import inspect_pe
                if issue:
                    with self.assertRaises(ValueError):
                        analysis.native_function_fresh_inlines(raw, capture, pe, specs,
                                                              "complete register leaf review", inspect_pe(raw))
                else:
                    plans = analysis.native_function_fresh_inlines(raw, capture, pe, specs,
                                                                  "complete register leaf review", inspect_pe(raw))
                    self.assertEqual(len(plans), 2)
                    self.assertEqual(plans[1]["flag_names"], ["INLINE_DONTCOPY"])
                    self.assertEqual([row["owner_rva"] for row in plans[1]["callsites"]], ["0x1040"] * 2)
                    self.assertEqual([row["call_path"][0]["callsite_rva"] for row in plans[1]["callsites"]],
                                     ["0x1040", "0x1046"])
                    self.assertEqual([row["call_path"][1]["callsite_rva"] for row in plans[1]["callsites"]],
                                     ["0x1000"] * 2)

    def test_fresh_inline_resolution_rejects_incomplete_sdk_calls_and_cleanup(self):
        for issue in (None, "missing_audit", "cleanup", "flags", "omitted_call", "wrong_call", "sdk_error", "pseudocode"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                raw, result, capture, write = self.fresh_inline_fixture(Path(directory))
                selection = analysis.native_function_selection(raw, capture, [0x1040], type_note="reviewed own frame and clone cache",
                                                              outline_rvas=[0x1000], fresh_inline_rvas=[0x1000])
                baseline = write(result / "capture.json", capture)
                capture_ref = {key: baseline[key] for key in ("path", "sha256")}
                output = result / "supplements/fresh"
                body = json.loads((result / "functions/1040.json").read_bytes())
                body["pseudocode"] = {"lines": [{"text": "return;"}], "locals": []}
                self.typed_fixture(body)
                audit = {"hook_registered": True, "hook_unregistered": True, "complete": True, "errors": [],
                         "dontcopy_flag": 2, "external_frame_flag": 1, "target_execution": False, "base_database_modified": False,
                         "calls": [{**call, "result_code": 0, "status": "inlined", "inline_flags": 3,
                                    "flag_names": ["INLINE_EXTFRAME", "INLINE_DONTCOPY"]}
                                   for call in selection["function_fresh_inlines"][0]["callsites"]]}
                body["fresh_inline_audit"] = audit
                if issue == "missing_audit": body.pop("fresh_inline_audit")
                elif issue == "cleanup": audit["hook_unregistered"] = False
                elif issue == "flags": audit["calls"][1]["inline_flags"] = 1
                elif issue == "omitted_call": audit["calls"].pop()
                elif issue == "wrong_call": audit["calls"][1]["callsite_rva"] = "0x1040"
                elif issue == "sdk_error": audit["calls"][1]["result_code"] = -12
                elif issue == "pseudocode": body["pseudocode"] = None
                record = write(output / "functions/1040.json", body)
                record.update(path="functions/1040.json", rva="0x1040", address=body["address"],
                              assembly_captured=True, pseudocode_captured=True)
                helper = selection["function_outlines"][0]
                supplement = {"input_sha256": capture["input_sha256"], "base_capture": capture_ref,
                    "capture_complete": True, "all_pseudocode_captured": True, "failures": [],
                    "database_integrity": {"verified": True}, "base_capture_unchanged": True,
                    "base_database_unchanged": True, "target_execution": False, "capture_tool": {},
                    "function_selection": selection, "functions": [record], "type_recoveries": [],
                    "ida_version": "fixture", "hexrays_version": "fixture", "function_outline_recoveries": [
                        {**helper, "previous_flags": 0, "applied_flags": 0x40000, "outline_flag": 0x40000,
                         "applied_chunks": helper["expected_chunks"], "base_database_modified": False}]}
                write(output / "capture.json", supplement)
                write(output / "session.json", {"passed": True, "exit_code": 0, "capture_tool": {}})
                write(output / "plan.json", {"function_selection": selection, "base_capture": capture_ref})
                resolved = analysis.native_reanalysis_resolution(result)
                self.assertEqual(bool(resolved["verification_failures"]), bool(issue))
                self.assertEqual(len(resolved["resolved_failures"]), 0 if issue else 1)

    def test_outline_selection_keeps_complete_helper_code_and_rejects_unreviewed_boundaries(self):
        for issue in (None, "selected", "duplicate", "missing", "note", "code", "chunk", "identity"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    raw, result, capture, write = self.outline_fixture(root)
                    outlines, note = [0x1002], "reviewed shared frame and multiple live outputs"
                    if issue == "selected": outlines = [0x1000]
                    elif issue == "duplicate": outlines *= 2
                    elif issue == "missing": outlines = [0x1090]
                    elif issue == "note": note = None
                    elif issue in {"code", "chunk", "identity"}:
                        row = capture["functions"][1]
                        path = result / row["path"]
                        body = json.loads(path.read_bytes())
                        if issue == "code": body["assembly"][0]["bytes"] = "9090"
                        elif issue == "chunk": body["chunks"][0]["raw_hex"] = "9090"
                        else: body["flags"] = 1
                        record = write(path, body)
                        if issue != "identity": row.update(sha256=record["sha256"], bytes=record["bytes"])
                    if issue:
                        with self.assertRaises(ValueError):
                            analysis.native_function_selection(raw, capture, [0x1000], type_note=note, outline_rvas=outlines)
                    else:
                        before = (result / "capture.json").read_bytes()
                        selected = analysis.native_function_selection(raw, capture, [0x1000], type_note=note, outline_rvas=outlines)
                        helper = selected["function_outlines"][0]
                        self.assertEqual(helper["rva"], "0x1002")
                        self.assertEqual(helper["set_flag"], "FUNC_OUTLINE")
                        self.assertEqual(helper["expected_chunks"][0]["raw_hex"], "8bec")
                        self.assertEqual((result / "capture.json").read_bytes(), before)

    def test_outline_resolution_rejects_missing_changed_flags_chunks_and_helper_identities(self):
        for issue in (None, "missing", "flags", "base", "chunks", "helper", "flag_value"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    raw, result, capture, write = self.outline_fixture(root)
                    selection = analysis.native_function_selection(raw, capture, [0x1000],
                        type_note="reviewed outlined helper", outline_rvas=[0x1002])
                    baseline = write(result / "capture.json", capture)
                    capture_ref = {key: baseline[key] for key in ("path", "sha256")}
                    output = result / "supplements/outline"
                    body = json.loads((result / capture["functions"][0]["path"]).read_bytes())
                    body["pseudocode"] = {"lines": [{"line": 0, "text": "void caller() {}"}], "locals": []}
                    self.typed_fixture(body)
                    record = write(output / "functions/1000.json", body)
                    record.update(path="functions/1000.json", rva="0x1000", address=body["address"],
                                  assembly_captured=True, pseudocode_captured=True)
                    helper = selection["function_outlines"][0]
                    applied = {**helper, "previous_flags": 0, "applied_flags": 0x40000, "outline_flag": 0x40000,
                               "applied_chunks": helper["expected_chunks"], "base_database_modified": False}
                    supplement = {"input_sha256": capture["input_sha256"], "base_capture": capture_ref,
                        "capture_complete": True, "all_pseudocode_captured": True, "failures": [],
                        "database_integrity": {"verified": True}, "base_capture_unchanged": True,
                        "base_database_unchanged": True, "target_execution": False, "capture_tool": {},
                        "function_selection": selection, "functions": [record], "type_recoveries": [],
                        "ida_version": "synthetic", "hexrays_version": "synthetic",
                        "function_outline_recoveries": [applied]}
                    if issue == "missing": supplement["function_outline_recoveries"] = []
                    elif issue == "flags": applied["applied_flags"] = 0
                    elif issue == "base": applied["base_database_modified"] = True
                    elif issue == "chunks": applied["applied_chunks"] = []
                    elif issue == "helper":
                        applied["source_function"] = {**helper["source_function"], "sha256": "0" * 64}
                    elif issue == "flag_value": applied["outline_flag"] = 3
                    write(output / "capture.json", supplement)
                    write(output / "plan.json", {"function_selection": selection, "base_capture": capture_ref})
                    write(output / "session.json", {"passed": True, "exit_code": 0, "capture_tool": {}})
                    checked = analysis.native_reanalysis_resolution(result)
                    self.assertEqual(len(checked["resolved_failures"]), 0 if issue else 1)
                    self.assertEqual(bool(checked["verification_failures"]), bool(issue))
                    self.assertEqual(json.loads((result / "capture.json").read_bytes()), capture)

    def test_function_merge_binds_complete_pe_union_and_preserves_unassembled_nop(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            raw, result, capture, _ = self.merge_fixture(Path(directory))
            before = (result / "capture.json").read_bytes()
            selected = analysis.native_function_selection(raw, capture, [0x1000],
                type_note="Explicit shared frame proven in retained callers", merge_specs=["0x1000=0x1000,0x1002,0x1004"])
            merge = selected["function_merges"][0]
            self.assertEqual(merge["owner_rva"], "0x1000")
            self.assertEqual(merge["member_rvas"], ["0x1000", "0x1002", "0x1004"])
            self.assertEqual(merge["expected_start"], "0x140001000")
            self.assertEqual(merge["expected_end"], "0x140001006")
            self.assertEqual(merge["expected_chunks"], [{"start": "0x140001000", "end": "0x140001006",
                "raw_hex": "55908bec5dc3", "sha256": hashlib.sha256(bytes.fromhex("55908bec5dc3")).hexdigest()}])
            body = analysis.native_function_merge_body(merge)
            self.assertEqual([row["bytes"] for row in body["assembly"]], ["55", "8bec", "5d", "c3"])
            self.assertEqual(body["xrefs_to_entry"][0]["from"], "0x140001080")
            self.assertEqual(body["end"], merge["expected_end"])
            self.assertEqual(body["chunks"], merge["expected_chunks"])
            self.assertEqual((result / "capture.json").read_bytes(), before)
            self.assertEqual((Path(directory) / capture["input"]).read_bytes(), raw)

    def test_function_merge_rejects_unreviewed_unselected_duplicate_and_foreign_members(self):
        cases = [(None, [0x1000], ["0x1000=0x1000,0x1002"]),
                 ("reviewed", [0x1000], ["0x1002=0x1000,0x1002"]),
                 ("reviewed", [0x1000], ["0x1002=0x1002,0x1004"]),
                 ("reviewed", [0x1000, 0x1002], ["0x1000=0x1000,0x1002"]),
                 ("reviewed", [0x1000], ["0x1000=0x1000,0x1000"]),
                 ("reviewed", [0x1000], ["0x1000=0x1000,0x1080"]),
                 ("reviewed", [0x1000], ["0x1000=0x1000,0x1002"] * 2),
                 ("reviewed", [0x1000], ["0x1000=0x1000"]),
                 ("reviewed", [0x1000], ["0x1000=invalid"])]
        for note, targets, specifications in cases:
            with self.subTest(specifications=specifications, note=note), tempfile.TemporaryDirectory() as directory:
                with patch.object(analysis, "ROOT", Path(directory)):
                    raw, _, capture, _ = self.merge_fixture(Path(directory))
                    with self.assertRaises(ValueError):
                        analysis.native_function_selection(raw, capture, targets,
                            type_note=note, merge_specs=specifications)

    def test_function_merge_rejects_changed_input_member_chunks_paths_and_assembly(self):
        issues = ("input_hash", "input_size", "incomplete_capture", "image_base", "member_hash", "member_size",
                  "path", "overlap", "hole", "chunk_hash", "chunk_bytes", "multiple_chunks", "assembly",
                  "assembly_order", "assembly_outside", "entry_call", "incomplete_assembly")
        for issue in issues:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                with patch.object(analysis, "ROOT", Path(directory)):
                    raw, result, capture, write = self.merge_fixture(Path(directory))
                    record = capture["functions"][0 if issue in {"overlap", "hole"} else 1]
                    path = result / record["path"]
                    body = json.loads(path.read_bytes())
                    if issue == "input_hash": capture["input_sha256"] = "0" * 64
                    elif issue == "input_size": capture["input_bytes"] += 1
                    elif issue == "incomplete_capture": capture["capture_complete"] = False
                    elif issue == "image_base": capture["image_base"] = "0x140000001"
                    elif issue == "member_hash": record["sha256"] = "0" * 64
                    elif issue == "member_size": record["bytes"] += 1
                    elif issue == "path": record["path"] = "../outside.json"
                    elif issue == "incomplete_assembly": record["assembly_captured"] = False
                    else:
                        if issue in {"overlap", "hole"}:
                            size = 3 if issue == "overlap" else 1
                            code = raw[0x200:0x200 + size]
                            body["end"] = hex(int(body["address"], 16) + size)
                            body["chunks"][0].update(end=body["end"], raw_hex=code.hex(),
                                                      sha256=hashlib.sha256(code).hexdigest())
                        elif issue == "chunk_hash": body["chunks"][0]["sha256"] = "0" * 64
                        elif issue == "chunk_bytes": body["chunks"][0]["raw_hex"] = "9090"
                        elif issue == "multiple_chunks": body["chunks"] *= 2
                        elif issue == "assembly": body["assembly"][0]["bytes"] = "9090"
                        elif issue == "assembly_order": body["assembly"] *= 2
                        elif issue == "assembly_outside": body["assembly"][0]["address"] = "0x140001080"
                        elif issue == "entry_call":
                            body["xrefs_to_entry"] = [{"from": "0x140001080", "to": body["address"], "code": True, "type": 17}]
                        identity = write(path, body)
                        record.update(bytes=identity["bytes"], sha256=identity["sha256"])
                    with self.assertRaises(ValueError):
                        analysis.native_function_selection(raw, capture, [0x1000], type_note="Explicit review",
                            merge_specs=["0x1000=0x1000,0x1002,0x1004"])

    def merge_reanalysis_fixture(self, root):
        raw, result, capture, write = self.merge_fixture(root)
        selected = analysis.native_function_selection(raw, capture, [0x1000], type_note="Explicit shared-frame evidence",
                                                      merge_specs=["0x1000=0x1000,0x1002,0x1004"])
        merge = selected["function_merges"][0]
        body = analysis.native_function_merge_body(merge)
        body["pseudocode"] = {"lines": [{"text": "void recovered() {}"}], "locals": []}
        self.typed_fixture(body)
        supplemental = result / "supplements/merged"
        recovered = write(supplemental / "functions/1000.json", body)
        recovered.update(path="functions/1000.json", rva="0x1000", address="0x140001000",
                         assembly_captured=True, pseudocode_captured=True)
        capture_ref = {"path": analysis.rel(result / "capture.json"), "sha256": analysis.sha(result / "capture.json")}
        source_database = {"path": ".work/private.i64", "sha256": "a" * 64}
        tool = {"path": "tools/ida/supplement.py", "sha256": "b" * 64}
        supplement = {"input_sha256": capture["input_sha256"], "base_capture": capture_ref,
                      "capture_complete": True, "all_pseudocode_captured": True, "failures": [],
                      "database_integrity": {"verified": True}, "base_capture_unchanged": True,
                      "base_database_unchanged": True, "target_execution": False, "capture_tool": tool,
                      "function_selection": selected, "functions": [recovered], "type_recoveries": [],
                      "function_merge_recoveries": [{**merge, "base_database_modified": False,
                                                       "applied_chunks": merge["expected_chunks"]}],
                      "source_database": source_database, "ida_version": "synthetic", "hexrays_version": "synthetic"}
        write(supplemental / "capture.json", supplement)
        write(supplemental / "session.json", {"passed": True, "exit_code": 0, "capture_tool": tool})
        write(supplemental / "plan.json", {"function_selection": selected, "base_capture": capture_ref,
                                          "source_database": source_database})
        return result, supplemental, capture, supplement, body, write

    def test_reanalysis_merge_accepts_only_exact_union_and_keeps_member_failure(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            result, _, capture, _, body, _ = self.merge_reanalysis_fixture(Path(directory))
            before = (result / "capture.json").read_bytes()
            resolved = analysis.native_reanalysis_resolution(result)
            self.assertFalse(resolved["verification_failures"])
            self.assertEqual(resolved["remaining_failures"], [capture["failures"][1]])
            self.assertEqual([row["original_failure"] for row in resolved["resolved_failures"]], [capture["failures"][0]])
            self.assertEqual(set(resolved["complete_functions"]), {"0x1000"})
            self.assertEqual(resolved["complete_functions"]["0x1000"]["assembly"], body["assembly"])
            self.assertEqual((result / "capture.json").read_bytes(), before)

    def test_reanalysis_merge_rejects_manifest_selection_and_complete_union_differences(self):
        issues = ("missing_merge", "modified_base", "applied_chunks", "member_identity", "selection_identity",
                  "end", "chunks", "assembly_missing", "assembly_order", "assembly_bytes", "xrefs", "changed_member")
        for issue in issues:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                with patch.object(analysis, "ROOT", Path(directory)):
                    result, supplemental, capture, supplement, body, write = self.merge_reanalysis_fixture(Path(directory))
                    before = (result / "capture.json").read_bytes()
                    if issue == "missing_merge": supplement["function_merge_recoveries"] = []
                    elif issue == "modified_base": supplement["function_merge_recoveries"][0]["base_database_modified"] = True
                    elif issue == "applied_chunks": supplement["function_merge_recoveries"][0]["applied_chunks"] = []
                    elif issue == "member_identity":
                        supplement["function_merge_recoveries"] = json.loads(json.dumps(supplement["function_merge_recoveries"]))
                        supplement["function_merge_recoveries"][0]["members"][1]["sha256"] = "0" * 64
                    elif issue == "selection_identity":
                        supplement["function_selection"]["function_merges"][0]["members"][1]["sha256"] = "0" * 64
                        plan = json.loads((supplemental / "plan.json").read_bytes())
                        plan["function_selection"] = supplement["function_selection"]
                        write(supplemental / "plan.json", plan)
                    elif issue == "changed_member":
                        (result / "functions/1002.json").write_bytes(b"changed original member")
                    else:
                        if issue == "end": body["end"] = "0x140001005"
                        elif issue == "chunks": body["chunks"][0]["raw_hex"] = "55908bec5d90"
                        elif issue == "assembly_missing": body["assembly"].pop()
                        elif issue == "assembly_order": body["assembly"].reverse()
                        elif issue == "assembly_bytes": body["assembly"][0]["bytes"] = "90"
                        elif issue == "xrefs": body["xrefs_to_entry"] = []
                        identity = write(supplemental / "functions/1000.json", body)
                        supplement["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                    write(supplemental / "capture.json", supplement)
                    rejected = analysis.native_reanalysis_resolution(result)
                    self.assertTrue(rejected["verification_failures"])
                    self.assertEqual(rejected["remaining_failures"], capture["failures"])
                    self.assertFalse(rejected["resolved_failures"])
                    self.assertFalse(rejected["complete_functions"])
                    self.assertEqual((result / "capture.json").read_bytes(), before)

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
        self.typed_fixture(body)
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

    def test_typed_serializer_uses_retained_source_after_the_live_tool_changes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            result, output, _, manifest, _, write = self.reanalysis_fixture(root)
            source = root / "tools/ida/capture_values.py"
            source.parent.mkdir(parents=True)
            raw = b"# inert typed-location serializer fixture\n"
            source.write_bytes(raw)
            manifest["typed_value_serializer"] = {"path": source.relative_to(root).as_posix(),
                "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
            write(output / "capture.json", manifest)
            resolved = analysis.native_reanalysis_resolution(result)
            self.assertFalse(resolved["verification_failures"])
            source.write_bytes(b"# next independent source version\n")
            resolved = analysis.native_reanalysis_resolution(result)
            self.assertFalse(resolved["verification_failures"])
            snapshot = root / ".analysis/results/reverse-pending/tool-sources" / (hashlib.sha256(raw).hexdigest() + ".py")
            self.assertEqual(snapshot.read_bytes(), raw)
            snapshot.write_bytes(b"changed retained serializer")
            resolved = analysis.native_reanalysis_resolution(result)
            self.assertTrue(resolved["remaining_failures"])
            self.assertIn("serializer source bytes changed", resolved["verification_failures"][0]["error"])

    def test_typed_serializer_rejects_missing_identity_or_a_different_source(self):
        for mutation in ({"bytes": True}, {"bytes": 0}, {"path": "tools/ida/other.py"},
                         {"sha256": "0" * 64}, {"extra": "unreviewed"}):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                result, output, _, manifest, _, write = self.reanalysis_fixture(root)
                source = root / "tools/ida/capture_values.py"
                source.parent.mkdir(parents=True)
                raw = b"# inert source fixture\n"
                source.write_bytes(raw)
                manifest["typed_value_serializer"] = {"path": source.relative_to(root).as_posix(),
                    "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), **mutation}
                write(output / "capture.json", manifest)
                resolved = analysis.native_reanalysis_resolution(result)
                self.assertTrue(resolved["verification_failures"])
                self.assertTrue(resolved["remaining_failures"])

    def conflicting_reanalysis_fixture(self, root, conflict_first=True, location_only=False):
        result, first, capture, first_manifest, first_body, write = self.reanalysis_fixture(root)
        second_base = {"address": "0x140001010", "rva": "0x1010", "end": "0x140001012",
                       "chunks": [{"start": "0x140001010", "end": "0x140001012", "raw_hex": "0000"}],
                       "assembly": [{"address": "0x140001010", "bytes": "0000"}],
                       "xrefs_to_entry": [], "pseudocode": None}
        base_record = write(result / "functions/1010.json", second_base)
        capture["functions"].append({**base_record, "path": "functions/1010.json", "rva": "0x1010",
                                     "address": second_base["address"]})
        capture["failures"].append({"stage": "1010:hexrays", "error": "independent original failure"})
        capture_ref = write(result / "capture.json", capture)
        if location_only:
            first_body["pseudocode"]["locals"] = [{"name": "value", "type": "int", "argument": False,
                "location": "<ida_hexrays.vdloc_t; proxy of <Swig Object of type 'vdloc_t *' at 0x001> >"}]
            record = write(first / "functions/1000.json", first_body)
            first_manifest["functions"][0]["sha256"] = record["sha256"]
        first_manifest["base_capture"] = capture_ref
        write(first / "capture.json", first_manifest)
        first_plan = json.loads((first / "plan.json").read_bytes())
        first_plan["base_capture"] = capture_ref
        write(first / "plan.json", first_plan)

        output = result / "supplements/z-independent"
        duplicate = json.loads(json.dumps(first_body))
        if location_only:
            duplicate["pseudocode"]["locals"][0]["location"] = (
                "<ida_hexrays.vdloc_t; proxy of <Swig Object of type 'vdloc_t *' at 0x002> >")
        else:
            duplicate["pseudocode"]["lines"][0]["text"] = "void recovered() { different_effect(); }"
        independent = {**second_base, "pseudocode": {"lines": [{"line": 0, "text": "void independent() {}"}], "locals": []}}
        self.typed_fixture(independent)
        functions = []
        for body in ([duplicate, independent] if conflict_first else [independent, duplicate]):
            record = write(output / ("functions/" + body["rva"][2:] + ".json"), body)
            functions.append({**record, "path": "functions/" + body["rva"][2:] + ".json", "rva": body["rva"],
                              "address": body["address"], "assembly_captured": True, "pseudocode_captured": True})
        selected = analysis.native_function_selection(bytes(self.fixture()), capture,
            [int(row["rva"], 16) for row in functions])
        manifest = {**first_manifest, "function_selection": selected, "functions": functions}
        write(output / "capture.json", manifest)
        write(output / "session.json", json.loads((first / "session.json").read_bytes()))
        write(output / "plan.json", {"function_selection": selected, "base_capture": capture_ref,
                                      "source_database": manifest["source_database"]})
        return result, first, output, capture, manifest, duplicate, independent, write

    def test_reanalysis_conflicting_full_artifact_does_not_discard_an_independent_verified_function(self):
        for conflict_first in (True, False):
            for location_only in (True, False):
                with self.subTest(conflict_first=conflict_first, location_only=location_only), \
                        tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                    result, first, output, capture, _, _, _, _ = self.conflicting_reanalysis_fixture(
                        Path(directory), conflict_first, location_only)
                    before = {path: path.read_bytes() for path in (
                        result / "capture.json", first / "functions/1000.json", output / "functions/1000.json")}
                    resolved = analysis.native_reanalysis_resolution(result)
                    self.assertEqual(set(resolved["complete_functions"]), {"0x1010"})
                    self.assertEqual(resolved["conflicting_function_rvas"], ["0x1000"])
                    self.assertEqual([row["function_rva"] for row in resolved["resolved_failures"]], ["0x1010"])
                    self.assertEqual(resolved["remaining_failures"], [capture["failures"][0]])
                    conflict = resolved["conflicting_functions"]["0x1000"]
                    self.assertEqual(conflict["retained_function"]["sha256"], analysis.sha(first / "functions/1000.json"))
                    self.assertEqual(conflict["alternatives"][0]["complete_function"]["sha256"],
                                     analysis.sha(output / "functions/1000.json"))
                    self.assertEqual(conflict["semantic_equivalence"], "unverified")
                    self.assertEqual(resolved["verification_failures"][0]["function_rva"], "0x1000")
                    self.assertEqual(len(resolved["supplements"]), 2)
                    self.assertEqual({path: path.read_bytes() for path in before}, before)

    def test_reanalysis_validates_every_output_before_recording_conflicts_or_independent_recoveries(self):
        for issue in ("digest", "chunks", "pseudocode", "missing_output", "session", "plan"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                result, _, output, capture, manifest, _, independent, write = self.conflicting_reanalysis_fixture(Path(directory))
                if issue == "digest":
                    manifest["functions"][1]["sha256"] = "0" * 64
                elif issue in ("chunks", "pseudocode"):
                    if issue == "chunks":
                        independent["chunks"][0]["raw_hex"] = "1111"
                    else:
                        independent["pseudocode"] = None
                    record = write(output / "functions/1010.json", independent)
                    manifest["functions"][1]["sha256"] = record["sha256"]
                elif issue == "missing_output":
                    manifest["functions"].pop()
                elif issue == "session":
                    write(output / "session.json", {"passed": False, "exit_code": 1,
                                                     "capture_tool": manifest["capture_tool"]})
                else:
                    plan = json.loads((output / "plan.json").read_bytes())
                    plan["function_selection"] = {"targets": []}
                    write(output / "plan.json", plan)
                write(output / "capture.json", manifest)
                resolved = analysis.native_reanalysis_resolution(result)
                self.assertEqual(set(resolved["complete_functions"]), {"0x1000"})
                self.assertEqual(resolved["conflicting_function_rvas"], [])
                self.assertEqual(resolved["conflicting_functions"], {})
                self.assertEqual(resolved["remaining_failures"], [capture["failures"][1]])
                self.assertEqual(len(resolved["verification_failures"]), 1)
                self.assertEqual(len(resolved["supplements"]), 1)

    def test_reanalysis_exact_duplicate_reuses_verified_bytes_and_new_function_without_a_conflict(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            result, first, output, _, manifest, _, _, write = self.conflicting_reanalysis_fixture(Path(directory))
            source = first / "functions/1000.json"
            destination = output / "functions/1000.json"
            destination.write_bytes(source.read_bytes())
            manifest["functions"][0]["sha256"] = analysis.sha(destination)
            write(output / "capture.json", manifest)
            resolved = analysis.native_reanalysis_resolution(result)
            self.assertEqual(set(resolved["complete_functions"]), {"0x1000", "0x1010"})
            self.assertFalse(resolved["conflicting_function_rvas"])
            self.assertFalse(resolved["conflicting_functions"])
            self.assertFalse(resolved["verification_failures"])
            self.assertFalse(resolved["remaining_failures"])
            self.assertEqual(len(resolved["resolved_failures"]), 2)

    def test_reanalysis_later_matching_capture_keeps_a_conflicting_function_unresolved(self):
        for matching_source in ("retained", "alternative"):
            with self.subTest(matching_source=matching_source), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                result, first, output, capture, _, _, _, write = self.conflicting_reanalysis_fixture(Path(directory))
                later = result / "supplements/zz-repeated"
                repeated = first if matching_source == "retained" else output
                source_manifest = json.loads((repeated / "capture.json").read_bytes())
                for record in source_manifest["functions"]:
                    source = repeated / record["path"]
                    destination = later / record["path"]
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(source.read_bytes())
                write(later / "capture.json", source_manifest)
                write(later / "session.json", json.loads((repeated / "session.json").read_bytes()))
                write(later / "plan.json", json.loads((repeated / "plan.json").read_bytes()))
                resolved = analysis.native_reanalysis_resolution(result)
                self.assertEqual(set(resolved["complete_functions"]), {"0x1010"})
                self.assertEqual(resolved["conflicting_function_rvas"], ["0x1000"])
                self.assertEqual(resolved["remaining_failures"], [capture["failures"][0]])
                self.assertEqual([row["function_rva"] for row in resolved["resolved_failures"]], ["0x1010"])
                self.assertEqual(len(resolved["supplements"]), 3)
                conflict = resolved["conflicting_functions"]["0x1000"]
                self.assertEqual(conflict["retained_function"]["sha256"], analysis.sha(first / "functions/1000.json"))
                self.assertEqual(conflict["semantic_equivalence"], "unverified")

    def test_observation_resolution_requires_exact_review_source_and_executed_observer(self):
        for issue in (None, "observer", "review"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                result, output, capture, supplement, _, write = self.reanalysis_fixture(root)
                observer = root / ".analysis/results/tool-sources/observer.py"
                observer.parent.mkdir(parents=True)
                observer.write_bytes(b"inert observer source\n")
                source = {"path": observer.relative_to(root).as_posix(), "bytes": observer.stat().st_size,
                          "sha256": hashlib.sha256(observer.read_bytes()).hexdigest()}
                review = root / ".analysis/results/frozen-review.json"
                write(review, {"observation": {"reason": "New static callsite observations.", "observer_source": source}})
                specification = ".analysis/results/frozen-review.json#/observation"
                selection = analysis.native_function_selection(bytes(self.fixture()), capture, [0x1000],
                                                               observation_evidence=specification)
                supplement["function_selection"] = selection
                supplement["capture_tool"] = {"path": "tools/ida/supplement.py", "sha256": source["sha256"]}
                if issue == "observer": supplement["capture_tool"]["sha256"] = "0" * 64
                if issue == "review":
                    write(review, {"observation": {"reason": "Changed review.", "observer_source": source}})
                write(output / "capture.json", supplement)
                write(output / "session.json", {"passed": True, "exit_code": 0,
                                                "capture_tool": supplement["capture_tool"]})
                write(output / "plan.json", {"function_selection": selection, "base_capture": supplement["base_capture"],
                                             "source_database": supplement["source_database"]})
                resolution = analysis.native_reanalysis_resolution(result)
                if issue is None:
                    self.assertFalse(resolution["verification_failures"])
                    self.assertFalse(resolution["remaining_failures"])
                else:
                    self.assertTrue(resolution["verification_failures"])
                    self.assertEqual(resolution["remaining_failures"], capture["failures"])

    def test_reanalysis_requires_exact_applied_import_thunk_manifest_and_original_selection(self):
        issues = (None, "missing", "slot", "source_hash", "empty_type", "non_string_type",
                  "modified_base", "duplicate", "selection_slot", "selection_missing")
        for architecture in ("x64", "x86"):
            for issue in issues:
                with self.subTest(architecture=architecture, issue=issue), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    with patch.object(analysis, "ROOT", root):
                        raw, result, capture, bodies, write = self.import_thunk_fixture(root, architecture)
                        baseline = write(result / "capture.json", capture)
                        capture_ref = {key: baseline[key] for key in ("path", "sha256")}
                        selection = analysis.native_function_selection(raw, capture, [0x1000], type_note="reviewed ABI",
                            import_specs=["0x2040=long (__cdecl *WppAutoLogTrace)(void *, ...);"])
                        supplemental = result / "supplements/selected"
                        body = bodies[0]
                        body["pseudocode"] = {"lines": [{"line": 0, "text": "return imported();"}], "locals": []}
                        self.typed_fixture(body)
                        recovered = write(supplemental / "functions/1000.json", body)
                        recovered.update(path="functions/1000.json", rva="0x1000", address=body["address"],
                                         assembly_captured=True, pseudocode_captured=True)
                        source_database = {"path": ".work/private.i64", "sha256": "a" * 64}
                        tool = {"path": "tools/ida/supplement.py", "sha256": "b" * 64}
                        applied_imports = [{**requested, "applied_type": requested["declaration"],
                            "base_database_modified": False, "database_import_binding": {
                                "rva": requested["rva"], "name": requested["name"], "module": "WppRecorder",
                                "ordinal": 0, "loaded_slot_raw_hex": "ff" * requested["bytes"]}}
                            for requested in selection["import_type_recoveries"]]
                        applied_thunks = json.loads(json.dumps([{**requested,
                            "applied_type": "long __cdecl imported(void *, ...)", "base_database_modified": False}
                            for requested in selection["import_thunks"]]))
                        if issue == "missing":
                            applied_thunks = []
                        elif issue == "slot":
                            applied_thunks[0]["slot_rva"] = "0x2048"
                        elif issue == "source_hash":
                            applied_thunks[0]["source_function"]["sha256"] = "0" * 64
                        elif issue == "empty_type":
                            applied_thunks[0]["applied_type"] = "  "
                        elif issue == "non_string_type":
                            applied_thunks[0]["applied_type"] = True
                        elif issue == "modified_base":
                            applied_thunks[0]["base_database_modified"] = True
                        elif issue == "duplicate":
                            applied_thunks.append(dict(applied_thunks[0]))
                        elif issue == "selection_slot":
                            selection["import_thunks"][0]["slot_rva"] = "0x2048"
                        elif issue == "selection_missing":
                            selection["import_thunks"] = []
                            applied_thunks = []
                        supplement = {"input_sha256": capture["input_sha256"], "base_capture": capture_ref,
                            "capture_complete": True, "all_pseudocode_captured": True, "failures": [],
                            "database_integrity": {"verified": True}, "base_capture_unchanged": True,
                            "base_database_unchanged": True, "target_execution": False,
                            "capture_tool": tool, "function_selection": selection, "functions": [recovered],
                            "type_recoveries": [], "import_type_recoveries": applied_imports,
                            "import_thunk_type_recoveries": applied_thunks, "source_database": source_database,
                            "ida_version": "synthetic", "hexrays_version": "synthetic"}
                        write(supplemental / "capture.json", supplement)
                        write(supplemental / "session.json", {"passed": True, "exit_code": 0, "capture_tool": tool})
                        write(supplemental / "plan.json", {"function_selection": selection,
                            "base_capture": capture_ref, "source_database": source_database})
                        resolved = analysis.native_reanalysis_resolution(result)
                        self.assertEqual(bool(resolved["verification_failures"]), issue is not None)
                        if issue is None:
                            self.assertFalse(resolved["remaining_failures"])
                            self.assertEqual(len(resolved["resolved_failures"]), 1)
                        else:
                            self.assertEqual(resolved["remaining_failures"], capture["failures"])
                            self.assertFalse(resolved["resolved_failures"])
                        self.assertEqual(json.loads((result / "capture.json").read_bytes()), capture)

    def test_exact_applied_size_limit_resolves_without_bypassing_original_byte_integrity(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            result, supplemental, capture, supplement, body, write = self.reanalysis_fixture(root)
            original = root / capture["input"]
            original_bytes = original.read_bytes()
            baseline_bytes = (result / "capture.json").read_bytes()
            selection = analysis.native_function_selection(original_bytes, capture, [0x1000],
                                                            max_function_kib=1024)
            supplement["function_selection"] = selection
            supplement["applied_decompiler_limits"] = {"max_function_kib": 1024}
            plan = json.loads((supplemental / "plan.json").read_bytes())
            plan["function_selection"] = selection
            write(supplemental / "plan.json", plan)
            write(supplemental / "capture.json", supplement)

            resolved = analysis.native_reanalysis_resolution(result)
            self.assertFalse(resolved["verification_failures"])
            self.assertFalse(resolved["remaining_failures"])
            self.assertEqual(len(resolved["resolved_failures"]), 1)
            self.assertEqual(resolved["resolved_failures"][0]["original_failure"], capture["failures"][0])
            self.assertEqual(set(resolved["complete_functions"]), {"0x1000"})
            self.assertEqual(resolved["complete_functions"]["0x1000"]["assembly"], body["assembly"])
            self.assertEqual(original.read_bytes(), original_bytes)
            self.assertEqual((result / "capture.json").read_bytes(), baseline_bytes)

            for issue in ("changed_original", "changed_instruction"):
                with self.subTest(issue=issue):
                    if issue == "changed_original":
                        changed = bytearray(original_bytes)
                        changed[0x200] ^= 1
                        original.write_bytes(changed)
                    else:
                        original.write_bytes(original_bytes)
                        body["assembly"][0]["bytes"] = "90"
                        ref = write(supplemental / "functions/1000.json", body)
                        supplement["functions"][0]["sha256"] = ref["sha256"]
                        write(supplemental / "capture.json", supplement)
                    rejected = analysis.native_reanalysis_resolution(result)
                    self.assertTrue(rejected["verification_failures"])
                    expected_error = ("original bytes changed" if issue == "changed_original"
                                      else "differs from original complete assembly")
                    self.assertIn(expected_error, rejected["verification_failures"][0]["error"])
                    self.assertEqual(rejected["remaining_failures"], capture["failures"])
                    self.assertFalse(rejected["resolved_failures"])
                    self.assertFalse(rejected["complete_functions"])
                    self.assertEqual((result / "capture.json").read_bytes(), baseline_bytes)

    def test_unadjudicated_recovery_requires_complete_live_types_and_original_instruction_snapshot(self):
        cases = (None, "metadata_missing", "metadata_incomplete", "mba_entry", "location_proxy",
                 "local_incomplete", "location_incomplete", "namespace", "conversion",
                 "stack_roundtrip", "missing_type", "type_before", "assembly_snapshot")
        for issue in cases:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                result, output, capture, supplement, body, write = self.reanalysis_fixture(Path(directory))
                body["pseudocode"]["locals"] = [{"complete": True, "width": 4,
                    "location": {"complete": True, "status": "observed", "kind": "stack",
                                 "register_namespace": "hexrays_microregisters"},
                    "ida_location_conversion": {"complete": True, "roundtrip_matches": True},
                    "stack_coordinate_conversion": {"complete": True, "roundtrip_matches": True}}]
                local = body["pseudocode"]["locals"][0]
                if issue == "metadata_missing": del body["pseudocode"]["locals_capture"]
                elif issue == "metadata_incomplete": body["pseudocode"]["locals_capture"]["complete"] = False
                elif issue == "mba_entry": body["pseudocode"]["locals_capture"]["mba_entry_ea"] = "0x140001001"
                elif issue == "location_proxy": local["location"] = "<vdloc_t proxy>"
                elif issue == "local_incomplete": local["complete"] = False
                elif issue == "location_incomplete": local["location"]["complete"] = False
                elif issue == "namespace": local["location"]["register_namespace"] = "processor_registers"
                elif issue == "conversion": local["ida_location_conversion"]["roundtrip_matches"] = False
                elif issue == "stack_roundtrip": local["stack_coordinate_conversion"]["roundtrip_matches"] = False
                elif issue == "missing_type": del body["type_after_decompile"]
                elif issue == "type_before": body["type_before_decompile"] = "int invented(void)"
                elif issue == "assembly_snapshot": body["assembly_after_decompile"] = []
                record = write(output / "functions/1000.json", body)
                supplement["functions"][0]["sha256"] = record["sha256"]
                write(output / "capture.json", supplement)
                baseline_before = (result / "capture.json").read_bytes()
                function_before = (output / "functions/1000.json").read_bytes()
                resolved = analysis.native_reanalysis_resolution(result)
                self.assertEqual(bool(resolved["complete_functions"]), issue is None)
                self.assertEqual(bool(resolved["resolved_failures"]), issue is None)
                self.assertEqual(bool(resolved["verification_failures"]), issue is not None)
                if issue:
                    self.assertEqual(resolved["remaining_failures"], capture["failures"])
                    self.assertEqual(resolved["verification_failures"][0]["function_rva"], "0x1000")
                self.assertEqual((result / "capture.json").read_bytes(), baseline_before)
                self.assertEqual((output / "functions/1000.json").read_bytes(), function_before)

    def test_reanalysis_changed_or_incomplete_evidence_does_not_hide_original_failure(self):
        for issue in ["changed_original", "changed_base", "changed_body", "incomplete_locals", "changed_instruction",
                      "failed_session", "unverified_database", "unselected_boundary", "unapplied_type",
                      "unapplied_size_limit", "different_size_limit"]:
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
                    elif issue in {"unapplied_size_limit", "different_size_limit"}:
                        supplement["function_selection"]["decompiler_limits"] = {"max_function_kib": 1024}
                        plan = json.loads((supplemental / "plan.json").read_bytes())
                        plan["function_selection"] = supplement["function_selection"]
                        write(supplemental / "plan.json", plan)
                        if issue == "different_size_limit":
                            supplement["applied_decompiler_limits"] = {"max_function_kib": 64}
                        write(supplemental / "capture.json", supplement)
                    else:
                        supplement["type_recoveries"] = [{"rva": "0x1000", "applied_type": "invented"}]
                        write(supplemental / "capture.json", supplement)
                    resolved = analysis.native_reanalysis_resolution(result)
                    self.assertTrue(resolved["verification_failures"])
                    self.assertEqual(resolved["remaining_failures"], capture["failures"])
                    self.assertFalse(resolved["resolved_failures"])


class FrontendAcquisitionTests(unittest.TestCase):
    def reuse_fixture(self, root, *, namespace="old", url="https://apps.razer.com/app/main.12345678.js", body=b"/* inert source */"):
        output = root / ".analysis/inputs/frontend" / namespace
        original = analysis.raw_path(output, url)
        original.parent.mkdir(parents=True, exist_ok=True)
        original.write_bytes(body)
        row = {"url": url, "path": analysis.rel(original), "bytes": len(body),
               "sha256": hashlib.sha256(body).hexdigest(), "http_status": 200,
               "content_type": "application/javascript", "acquired_at_utc": "retained-source"}
        receipt = output / "acquisition.json"
        receipt.write_text(json.dumps({"source_current_during_acquisition": True, "files": [row]}), encoding="utf-8")
        return analysis.rel(receipt), row

    def test_reuse_references_exact_immutable_original_without_copying_or_mutating_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            with patch.object(analysis, "ROOT", root):
                receipt, source = self.reuse_fixture(root)
                before = (root / receipt).read_bytes()
                fallback = MagicMock()
                reuse = analysis.FrontendArtifactReuse([receipt, receipt], fallback)
                destination = root / ".analysis/inputs/frontend/new"
                actual = reuse.download(source["url"], destination)
                self.assertEqual(actual["path"], source["path"])
                self.assertEqual(actual["reused_from_acquisition"], reuse.receipts[0])
                self.assertEqual(len(reuse.receipts), 1)
                self.assertFalse(destination.exists())
                reuse.verify()
                self.assertEqual((root / receipt).read_bytes(), before)
                fallback.assert_not_called()

    def test_reuse_never_matches_another_url_query_hashless_root_or_failed_attempt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            with patch.object(analysis, "ROOT", root):
                receipt, source = self.reuse_fixture(root)
                root_receipt, root_source = self.reuse_fixture(root, namespace="root", url="https://apps.razer.com/app/main.js")
                document = json.loads((root / receipt).read_bytes())
                failed_url = "https://apps.razer.com/app/missing.12345678.js.map"
                document["files"].append({"url": failed_url, "http_status": 404, "error": "HTTP Error 404"})
                (root / receipt).write_text(json.dumps(document), encoding="utf-8")
                failed = {"url": failed_url, "error": "fresh HTTP failure"}
                fallback = MagicMock(return_value=failed)
                reuse = analysis.FrontendArtifactReuse([receipt, root_receipt], fallback)
                urls = [source["url"].replace("/app/", "/other/"), source["url"] + "?different=1", root_source["url"], failed_url]
                output = root / ".analysis/inputs/frontend/new"
                for url in urls:
                    self.assertIs(reuse.download(url, output), failed)
                self.assertEqual([call.args[0] for call in fallback.call_args_list], urls)

    def test_reuse_rejects_changed_or_missing_original_without_network_fallback(self):
        for mutation in ("changed", "missing", "length", "escape"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                with patch.object(analysis, "ROOT", root):
                    receipt, source = self.reuse_fixture(root)
                    if mutation == "changed":
                        (root / source["path"]).write_bytes(b"X" * source["bytes"])
                    elif mutation == "missing":
                        (root / source["path"]).unlink()
                    else:
                        document = json.loads((root / receipt).read_bytes())
                        if mutation == "length":
                            document["files"][0]["bytes"] += 1
                        else:
                            document["files"][0]["path"] = ".analysis/inputs/../../outside.js"
                        (root / receipt).write_text(json.dumps(document), encoding="utf-8")
                    fallback = MagicMock()
                    reuse = analysis.FrontendArtifactReuse([receipt], fallback)
                    with self.assertRaises((ValueError, OSError)):
                        reuse.download(source["url"], root / ".analysis/inputs/frontend/new")
                    fallback.assert_not_called()

    def test_reuse_rejects_conflicting_immutable_identities_and_receipt_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            with patch.object(analysis, "ROOT", root):
                first, source = self.reuse_fixture(root)
                second, _ = self.reuse_fixture(root, namespace="conflict", body=b"different retained source")
                with self.assertRaisesRegex(ValueError, "conflict"):
                    analysis.FrontendArtifactReuse([first, second], MagicMock())
                reuse = analysis.FrontendArtifactReuse([first], MagicMock())
                reuse.download(source["url"], root / ".analysis/inputs/frontend/new")
                (root / first).write_bytes(b"changed receipt")
                with self.assertRaisesRegex(ValueError, "identity differs"):
                    reuse.verify()

    def test_reuse_propagates_fetch_failure_and_cancellation_and_preserves_old_evidence(self):
        for failure in (RuntimeError("fetch failed"), KeyboardInterrupt()):
            with self.subTest(failure=type(failure).__name__), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                with patch.object(analysis, "ROOT", root):
                    receipt, source = self.reuse_fixture(root)
                    before = {name: (root / name).read_bytes() for name in (receipt, source["path"])}
                    reuse = analysis.FrontendArtifactReuse([receipt], MagicMock(side_effect=failure))
                    output = root / ".analysis/inputs/frontend/new"
                    with self.assertRaises(type(failure)):
                        reuse.download("https://apps.razer.com/app/index.html", output)
                    self.assertFalse(output.exists())
                    self.assertEqual({name: (root / name).read_bytes() for name in before}, before)

    def test_fetch_reuses_immutable_paths_but_fetches_roots_and_indexes_only_new_inputs(self):
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()):
            root = Path(directory).resolve()
            with patch.object(analysis, "ROOT", root):
                receipt, source = self.reuse_fixture(root)
                entry, manifest = "https://apps.razer.com/app/index.html", "https://apps.razer.com/app/asset-manifest.json"
                bodies = {entry: b'<script src="main.12345678.js"></script>', manifest: b'{"files":{}}'}
                calls, indexed = [], []
                def downloader(url, output):
                    calls.append(url)
                    raw = bodies[url]
                    path = analysis.raw_path(output, url)
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(raw)
                    return {"url": url, "path": analysis.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                            "http_status": 200, "content_type": "text/html" if url == entry else "application/json", "acquired_at_utc": "fresh"}
                def urlopen(request, **kwargs):
                    response = MagicMock()
                    response.__enter__.return_value = response
                    response.read.return_value = bodies[request.full_url]
                    return response
                tool = root / "static-analyzer.exe"
                tool.write_bytes(b"inert analyzer identity")
                args = argparse.Namespace(url=entry, output=".analysis/inputs/frontend/new", reuse_acquisition=[receipt],
                    download_artifact=downloader, index_artifacts=indexed.extend, no_refresh_gaps=True)
                with patch.object(analysis, "wakaru", return_value=tool), \
                     patch.object(analysis.subprocess, "run", return_value=argparse.Namespace(returncode=0, stdout=b'{"enumeration":{}}', stderr=b"")), \
                     patch.object(analysis.urllib.request, "urlopen", side_effect=urlopen):
                    analysis.fetch_frontend(args)
                self.assertEqual(set(calls), set(bodies))
                self.assertEqual(len(indexed), 3)
                self.assertNotIn(source["path"], {row["path"] for row in indexed})
                result = json.loads((root / args.output / "acquisition.json").read_bytes())
                retained = next(row for row in result["files"] if row["url"] == source["url"])
                self.assertEqual(retained["path"], source["path"])
                self.assertTrue(result["source_current_during_acquisition"])
                self.assertFalse(analysis.raw_path(root / args.output, source["url"]).exists())
                frozen = (root / args.output / "acquisition.json").read_bytes()
                with patch.object(analysis, "wakaru", return_value=tool), \
                     self.assertRaisesRegex(ValueError, "new output directory"):
                    analysis.fetch_frontend(args)
                self.assertEqual((root / args.output / "acquisition.json").read_bytes(), frozen)
                self.assertEqual(len(calls), 2)

    def test_reuse_cli_is_repeatable_and_incompatible_with_repair(self):
        parser = argparse.ArgumentParser()
        analysis.add_commands(parser.add_subparsers(dest="command"))
        args = parser.parse_args(["fetch-frontend", "--url", "https://apps.razer.com/app/", "--output", ".analysis/inputs/frontend/new",
                                  "--reuse-acquisition", ".analysis/inputs/frontend/old/acquisition.json",
                                  "--reuse-acquisition", ".analysis/inputs/frontend/another/acquisition.json", "--repair"])
        self.assertEqual(len(args.reuse_acquisition), 2)
        with self.assertRaisesRegex(ValueError, "--repair"):
            analysis.fetch_frontend(args)

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

    def test_chunk_capture_borrows_verified_outputs_without_mutating_provider(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source, tool = root / "source.js", root / "analyzer.exe"
            old, new = root / "old-capture", root / "new-capture"
            source.write_bytes(b"/* inert source */")
            tool.write_bytes(b"static analyzer fixture")
            result = argparse.Namespace(returncode=0, stdout=b'{"enumeration":null}', stderr=b"")
            with patch.object(analysis, "ROOT", root), patch.object(analysis.subprocess, "run", return_value=result) as run:
                first = analysis.enumerate_frontend_chunks(source, tool, old, "source")
                retained = {path: path.read_bytes() for path in old.iterdir()}
                borrowed = analysis.enumerate_frontend_chunks(source, tool, new, "source", reuse_directories=[old])
                self.assertEqual(borrowed.capture, first.capture)
                self.assertEqual(run.call_count, 1)
                self.assertFalse(new.exists())
                self.assertEqual({path: path.read_bytes() for path in old.iterdir()}, retained)
                (old / "source.json").write_bytes(b"corrupt output")
                rebuilt = analysis.enumerate_frontend_chunks(source, tool, new, "source", reuse_directories=[old])
                self.assertEqual(run.call_count, 2)
                self.assertNotEqual(rebuilt.capture["path"], first.capture["path"])
                self.assertEqual((old / "source.json").read_bytes(), b"corrupt output")

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

    def reviewed_pe_fixture(self, root, *, count=1, aliases=False):
        def put(name, raw):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

        def pe(machine, data, *, signed=False, entry=True):
            raw_size = (len(data) + 0x1ff) & ~0x1ff
            body = bytearray(0x200 + raw_size)
            body[:2] = b"MZ"
            struct.pack_into("<I", body, 0x3c, 0x80)
            body[0x80:0x84] = b"PE\0\0"
            wide = machine != 0x14c
            optional_size = 240 if wide else 224
            struct.pack_into("<HHIIIHH", body, 0x84, machine, 1, 0, 0, 0, optional_size, 0x2102)
            optional = 0x98
            struct.pack_into("<H", body, optional, 0x20b if wide else 0x10b)
            struct.pack_into("<I", body, optional + 16, 0x1000 if entry else 0)
            struct.pack_into("<Q" if wide else "<I", body, optional + (24 if wide else 28), 0x10000)
            struct.pack_into("<II", body, optional + 32, 0x1000, 0x200)
            struct.pack_into("<II", body, optional + 56, (0x1000 + raw_size + 0xfff) & ~0xfff, 0x200)
            struct.pack_into("<H", body, optional + 68, 1)
            directories = optional + (112 if wide else 96)
            struct.pack_into("<I", body, directories - 4, 16)
            table = optional + optional_size
            struct.pack_into("<8sIIII", body, table, b".text\0\0\0" if entry else b".data\0\0\0",
                             len(data), 0x1000, raw_size, 0x200)
            struct.pack_into("<I", body, table + 36, 0x60000020 if entry else 0xc0000040)
            body[0x200:0x200 + len(data)] = data
            if signed:
                struct.pack_into("<II", body, directories + 4 * 8, len(body), 16)
                body.extend(struct.pack("<IHH", 16, 0x200, 2) + b"fixture!")
            return bytes(body)

        children = [pe(0x14c if i == 0 else 0x8664, bytes([0xb8, i, 0, 0, 0, 0xc3]), signed=True)
                    for i in range(count)]
        offsets, end = [], 0x300
        for body in children:
            offsets.append(end)
            end = (end + len(body) + 0x1ff) & ~0x1ff
        data = bytearray(end - 0x200)
        data[:0x100] = b"p" * 0x100
        for offset, body in zip(offsets, children):
            data[offset - 0x200:offset - 0x200 + len(body)] = body
        parent_raw = pe(0x8664, data, entry=False)
        parent = {**put(".analysis/inputs/official-native-current/blobs/parent.dll", parent_raw),
                  "kind": "official_device_resource", "url": "https://app-assets.razer.com/files/fixture/parent.dll",
                  "verified_against_manifest": True, "container_extraction_complete": True}
        parents = [parent]
        if aliases:
            alias = {**put(".analysis/inputs/official-native-current/resources/alias.dll", parent_raw),
                     "verified_against_manifest": True, "container_extraction_complete": True}
            partial = {**put(".analysis/inputs/official-native-current/resources/partial.dll", parent_raw),
                       "source_bytes_verified": False}
            parents.extend([alias, partial])
        put("tools/analysis.py", b"synthetic static analyzer identity")
        evidence_row = put(".analysis/results/native/fixture/functions/1000.json",
                           b'{"rva":"0x1000","review":"complete pointer and byte-length owner"}')
        base_capture = put(".analysis/results/native/fixture/capture.json",
                           b'{"capture_complete":true,"failures":[{"stage":"2000:hexrays","error":"outer context"}]}')
        ranges = [{"name": "driver-" + str(i) + ".sys", "offset": offset, "bytes": len(body),
                   "sha256": hashlib.sha256(body).hexdigest(), "machine": "0x14c" if i == 0 else "0x8664",
                   "evidence": [dict(evidence_row)], "basis": "Complete fixture owner provides pointer and length"}
                  for i, (offset, body) in enumerate(zip(offsets, children))]
        group = {"schema_version": 1, "source_input": {key: parent[key] for key in ("path", "bytes", "sha256")},
                 "ranges": ranges}
        review_value = {"pe/groups~": [group]}
        review_name = ".analysis/results/native-packages/review.json"
        put(review_name, json.dumps(review_value, ensure_ascii=False).encode())
        index_path = root / ".analysis/inputs.json"
        index_path.write_text(json.dumps({"inputs": parents}), encoding="utf-8")
        report_name = ".analysis/results/native-packages/embedded-inputs.json"
        old_report = b'{"schema_version":2,"inputs":[],"failures":[{"stage":"old","error":"retain history"}]}'
        put(report_name, old_report)
        args = argparse.Namespace(source_prefix=".analysis/inputs/official-native-current/", no_refresh_gaps=True,
                                  pe_review=[review_name + "#/pe~1groups~0/0"])
        return {"root": root, "put": put, "parent": parent, "parent_raw": parent_raw,
                "parents": parents, "children": children, "group": group, "review_value": review_value,
                "review_name": review_name, "evidence": evidence_row, "base_capture": base_capture,
                "index_path": index_path, "report_name": report_name, "old_report": old_report, "args": args}

    def write_pe_review(self, fixture, value=None):
        value = fixture["review_value"] if value is None else value
        return fixture["put"](fixture["review_name"], json.dumps(value, ensure_ascii=False).encode())

    def run_pe_review(self, fixture):
        root = fixture["root"]
        with patch.object(analysis, "ROOT", root), patch.object(analysis, "__file__", str(root / "tools/analysis.py")), \
                patch.object(analysis.subprocess, "run", side_effect=AssertionError("Static PE extraction must not execute")), \
                patch.object(analysis, "refresh_gaps") as refresh, redirect_stdout(io.StringIO()):
            result = analysis.extract_embedded_inputs(fixture["args"])
        refresh.assert_not_called()
        return result

    def assert_pe_review_rejected(self, fixture):
        root = fixture["root"]
        index_before = fixture["index_path"].read_bytes()
        report_path = root / fixture["report_name"]
        report_before = report_path.read_bytes()
        embedded = root / ".analysis/inputs/official-native-current/embedded"
        children_before = {path.relative_to(embedded).as_posix(): path.read_bytes()
                           for path in embedded.rglob("*") if path.is_file()} if embedded.exists() else {}
        with self.assertRaises(ValueError):
            self.run_pe_review(fixture)
        self.assertEqual(fixture["index_path"].read_bytes(), index_before)
        self.assertEqual(report_path.read_bytes(), report_before)
        children_after = {path.relative_to(embedded).as_posix(): path.read_bytes()
                          for path in embedded.rglob("*") if path.is_file()} if embedded.exists() else {}
        self.assertEqual(children_after, children_before)

    def test_reviewed_pe_extraction_preserves_originals_index_and_frozen_complete_provenance(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = self.reviewed_pe_fixture(Path(directory).resolve(), count=2, aliases=True)
            root = fixture["root"]
            review_raw = (root / fixture["review_name"]).read_bytes()
            unchanged = {row["path"]: (root / row["path"]).read_bytes()
                         for row in [*fixture["parents"], fixture["evidence"], fixture["base_capture"]]}
            result = self.run_pe_review(fixture)
            self.assertEqual(result["schema_version"], 3)
            self.assertEqual(result["summary"]["reviewed_portable_executables"], 2)
            self.assertEqual(result["summary"]["newly_indexed_inputs"], 2)
            self.assertFalse(result["target_execution"])
            self.assertFalse(result["failures"])
            frozen_review = {"path": ".analysis/results/native-packages/review-sources/"
                             + hashlib.sha256(review_raw).hexdigest() + ".json", "bytes": len(review_raw),
                             "sha256": hashlib.sha256(review_raw).hexdigest()}
            self.assertEqual((root / frozen_review["path"]).read_bytes(), review_raw)
            self.assertEqual(result["pe_reviews"][0]["review_source"], frozen_review)
            self.assertEqual(result["pe_reviews"][0]["source_input"], fixture["group"]["source_input"])
            expected_aliases = {row["path"] for row in fixture["parents"][:2]}
            indexed = {row["path"]: row for row in json.loads(fixture["index_path"].read_bytes())["inputs"]}
            for i, artifact in enumerate(result["inputs"]):
                spec = fixture["group"]["ranges"][i]
                self.assertEqual((root / artifact["path"]).read_bytes(), fixture["children"][i])
                self.assertEqual(artifact["sha256"], spec["sha256"])
                self.assertEqual(artifact["container_offset"], spec["offset"])
                self.assertTrue(artifact["byte_range_verified"])
                self.assertTrue(artifact["verified_against_manifest"])
                self.assertEqual(artifact["container_sha256"], fixture["parent"]["sha256"])
                self.assertEqual({row["path"] for row in artifact["container_aliases"]}, expected_aliases)
                self.assertTrue(all(row["byte_identity_verified"] for row in artifact["container_aliases"]))
                extraction = artifact["extraction"]
                self.assertEqual(extraction["kind"], "reviewed_embedded_pe_range")
                self.assertEqual(extraction["pe"]["machine"], spec["machine"])
                self.assertEqual(extraction["layout"]["security_directory"]["bytes"], 16)
                self.assertEqual(extraction["layout"]["complete_file_bytes"], 0x410)
                self.assertEqual(extraction["review"], spec)
                self.assertEqual(extraction["review_source"], frozen_review)
                self.assertEqual(extraction["review_pointer"], "/pe~1groups~0/0")
                self.assertEqual(indexed[artifact["path"]], artifact)
            for name, original in unchanged.items():
                self.assertEqual((root / name).read_bytes(), original)
            previous = result["previous_report"]
            self.assertEqual((root / previous["path"]).read_bytes(), fixture["old_report"])
            self.assertEqual(previous["sha256"], hashlib.sha256(fixture["old_report"]).hexdigest())
            self.assertEqual((root / fixture["review_name"]).read_bytes(), review_raw)
            self.assertEqual(json.loads((root / fixture["report_name"]).read_bytes()), result)

    def test_reviewed_pe_pointer_accepts_group_array_or_root_group(self):
        for mode in ("array", "root"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve())
                if mode == "array":
                    fixture["args"].pe_review = [fixture["review_name"] + "#/pe~1groups~0"]
                else:
                    self.write_pe_review(fixture, fixture["group"])
                    fixture["args"].pe_review = [fixture["review_name"] + "#"]
                self.assertEqual(self.run_pe_review(fixture)["summary"]["reviewed_portable_executables"], 1)

    def test_reviewed_pe_cli_preserves_repeated_review_references(self):
        parser = argparse.ArgumentParser()
        analysis.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["extract-embedded-inputs", "--pe-review", ".analysis/results/a.json#/one",
                                  "--pe-review", ".analysis/results/b.json#/two", "--no-refresh-gaps"])
        self.assertEqual(args.pe_review, [".analysis/results/a.json#/one", ".analysis/results/b.json#/two"])

    def test_reviewed_pe_changed_or_missing_parent_and_evidence_do_not_publish(self):
        for issue in ("parent", "evidence", "missing_parent", "missing_evidence", "missing_review"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve())
                name = (fixture["parent"]["path"] if "parent" in issue else fixture["evidence"]["path"])
                if issue == "missing_review":
                    name = fixture["review_name"]
                path = fixture["root"] / name
                if issue.startswith("missing"):
                    path.unlink()
                else:
                    path.write_bytes(path.read_bytes() + b"changed")
                self.assert_pe_review_rejected(fixture)

    def test_reviewed_pe_changes_during_preflight_are_detected_before_publication(self):
        inspect_ranges = host.inspect_embedded_pe_ranges
        for issue in ("parent", "evidence", "review"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve())
                name = {"parent": fixture["parent"]["path"], "evidence": fixture["evidence"]["path"],
                        "review": fixture["review_name"]}[issue]

                def inspect_then_change(raw, ranges):
                    result = inspect_ranges(raw, ranges)
                    path = fixture["root"] / name
                    path.write_bytes(path.read_bytes() + b" ")
                    return result

                with patch.object(host, "inspect_embedded_pe_ranges", side_effect=inspect_then_change):
                    self.assert_pe_review_rejected(fixture)

    def test_reviewed_pe_group_schema_current_index_and_scope_are_strict(self):
        issues = ("extra_group", "missing_ranges", "bool_version", "wrong_version", "empty_ranges",
                  "extra_source", "unindexed", "index_hash", "scope", "partial_parent", "invalid_json")
        for issue in issues:
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve())
                group = fixture["group"]
                if issue == "extra_group": group["extra"] = True
                elif issue == "missing_ranges": group.pop("ranges")
                elif issue == "bool_version": group["schema_version"] = True
                elif issue == "wrong_version": group["schema_version"] = 2
                elif issue == "empty_ranges": group["ranges"] = []
                elif issue == "extra_source": group["source_input"]["kind"] = "invented"
                elif issue == "unindexed": fixture["index_path"].write_text('{"inputs":[]}', encoding="utf-8")
                elif issue == "index_hash":
                    fixture["parent"]["sha256"] = "0" * 64
                    fixture["index_path"].write_text(json.dumps({"inputs": fixture["parents"]}), encoding="utf-8")
                elif issue == "scope": fixture["args"].source_prefix = ".analysis/inputs/official-native-current/other/"
                elif issue == "partial_parent":
                    fixture["parent"]["source_bytes_verified"] = False
                    fixture["index_path"].write_text(json.dumps({"inputs": fixture["parents"]}), encoding="utf-8")
                self.write_pe_review(fixture)
                if issue == "invalid_json":
                    (fixture["root"] / fixture["review_name"]).write_bytes(b"{invalid")
                self.assert_pe_review_rejected(fixture)

    def test_reviewed_pe_invalid_rfc6901_pointer_is_not_silently_reinterpreted(self):
        for pointer in ("missing/slash", "/pe~2groups~0", "/pe~1groups~0/00", "/pe~1groups~0/-1",
                        "/pe~1groups~0/2", "/absent"):
            with self.subTest(pointer=pointer), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve())
                fixture["args"].pe_review = [fixture["review_name"] + "#" + pointer]
                self.assert_pe_review_rejected(fixture)

    def test_reviewed_pe_second_range_failure_cannot_publish_the_valid_first_range(self):
        for issue in ("hash", "bounds", "overlap", "truncated_certificate"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve(), count=2)
                spec = fixture["group"]["ranges"][1]
                if issue == "hash": spec["sha256"] = "0" * 64
                elif issue == "bounds": spec["offset"] = len(fixture["parent_raw"]) - 8
                elif issue == "overlap": spec["offset"] = fixture["group"]["ranges"][0]["offset"] + 8
                else:
                    spec["bytes"] -= 8
                    spec["sha256"] = hashlib.sha256(fixture["parent_raw"][spec["offset"]:
                                                                          spec["offset"] + spec["bytes"]]).hexdigest()
                self.write_pe_review(fixture)
                self.assert_pe_review_rejected(fixture)

    def test_reviewed_pe_all_groups_and_destinations_are_preflighted_together(self):
        for issue in ("second_group_hash", "duplicate_destination", "conflicting_child"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve(), count=2)
                group = fixture["group"]
                first = {**group, "ranges": [group["ranges"][0]]}
                second = {**group, "ranges": [dict(group["ranges"][1])]}
                if issue == "second_group_hash": second["ranges"][0]["sha256"] = "0" * 64
                elif issue == "duplicate_destination": second["ranges"] = first["ranges"]
                else:
                    destination = ".analysis/inputs/official-native-current/embedded/" + fixture["parent"]["sha256"] + "/driver-1.sys"
                    fixture["put"](destination, b"conflicting immutable child")
                self.write_pe_review(fixture, [first, second])
                fixture["args"].pe_review = [fixture["review_name"]]
                self.assert_pe_review_rejected(fixture)

    def test_reviewed_pe_frozen_review_or_previous_report_conflict_cannot_publish_children(self):
        for source in ("review", "previous_report"):
            with self.subTest(source=source), tempfile.TemporaryDirectory() as directory:
                fixture = self.reviewed_pe_fixture(Path(directory).resolve())
                raw = ((fixture["root"] / fixture["review_name"]).read_bytes()
                       if source == "review" else fixture["old_report"])
                frozen = ".analysis/results/native-packages/review-sources/" + hashlib.sha256(raw).hexdigest() + ".json"
                fixture["put"](frozen, b"conflicting immutable review source")
                self.assert_pe_review_rejected(fixture)

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

    def test_inline_component_words_and_regional_names_are_not_export_namespace_routes(self):
        source = self.source() + (";const region={en:'en-US','zh-CN':'zh-CN'};"
            "const monitor={en:{SENSOR:'Temperature'},'zh-CN':{SENSOR:'温度'}};").encode()
        result = evidence.locale_export_dictionaries(source, ["en", "zh-CN"])
        self.assertEqual(result["values"]["en"]["SHOW_GUIDE"], "Show Guide")
        self.assertNotIn("SENSOR", result["values"]["en"])

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


class LocaleFrozenDictionaryTests(unittest.TestCase):
    def source(self):
        return ("(()=>{const chinese='zh-CN',route={en:()=>Promise.resolve().then(function(){return en}),"
                "[chinese]:()=>Promise.resolve().then(()=>zh)};"
                "var en=Object.freeze({__proto__:null,TITLE:'Keyboard Shortcuts',CLOSE:'Close'}),"
                "zh=Object.freeze({__proto__:null,TITLE:'键盘快捷键',CLOSE:'关闭'});return route})()").encode()

    def test_frozen_lazy_namespaces_keep_language_identity_and_full_owners(self):
        raw = self.source()
        selected = evidence.locale_export_dictionaries(raw, ['en', 'zh-CN'])
        self.assertEqual(selected['values'], {
            'en': {'TITLE': 'Keyboard Shortcuts', 'CLOSE': 'Close'},
            'zh-CN': {'TITLE': '键盘快捷键', 'CLOSE': '关闭'}})
        self.assertEqual(selected['locale_order'], ['en', 'zh-CN'])
        self.assertFalse(selected['javascript_executed'])
        for row in selected['dictionaries']:
            self.assertIn('Promise.resolve()', row['loader']['body'])
            body = row['export_object']
            self.assertEqual(raw[body['start_byte']:body['end_byte']].decode(), body['body'])

    def test_dynamic_loaders_shared_namespaces_and_replaced_globals_are_rejected(self):
        raw = self.source()
        for changed in (
                raw.replace(b'Promise.resolve()', b'Promise.resolve(load())'),
                raw.replace(b'return en', b'sideEffect();return en'),
                raw.replace(b'()=>zh', b'()=>en'),
                raw.replace(b'()=>zh', b'()=>({TITLE:"wrong"})'),
                raw.replace(b'Object.freeze', b'factory.freeze'),
                raw.replace(b"CLOSE:'Close'", b"CLOSE:'Close',CLOSE:'duplicate'"),
                raw.replace(b"TITLE:'Keyboard Shortcuts'", b"TITLE:window.words"),
                raw.replace(b'const chinese', b'const Promise=dynamic;const chinese'),
                raw.replace(b'const chinese', b'const Object=dynamic;const chinese')):
            with self.subTest(raw=changed), self.assertRaises(ValueError):
                evidence.locale_export_dictionaries(changed, ['en', 'zh-CN'])


class LocaleChunkExportTests(unittest.TestCase):
    def source(self):
        return ("(self.webpackChunk=self.webpackChunk||[]).push([[12],{7927:(module,exports,require)=>{"
                "require.d(exports,{TITLE:()=>title,SECS:()=>seconds});"
                "const title='Keyboard Shortcuts',seconds='secs';}}]);").encode()

    def test_exports_resolve_in_actual_factory_and_keep_complete_capture(self):
        selected = evidence.locale_chunk_exports(self.source(), ['SECS'])
        self.assertEqual(selected['values'], {'SECS': 'secs'})
        self.assertIn("TITLE:()=>title", selected['complete_factory']['body'])
        self.assertEqual(len(selected['resolved_exports']), 1)
        self.assertFalse(selected['javascript_executed'])
        self.assertEqual(evidence.locale_chunk_exports(self.source())['values'],
                         {'TITLE': 'Keyboard Shortcuts', 'SECS': 'secs'})

    def test_decoy_exports_shadowed_values_and_dynamic_getters_are_rejected(self):
        raw = self.source()
        for changed in (
                raw.replace(b'require.d(exports', b'other.d(exports'),
                raw.replace(b'require.d(exports', b'require.d(other'),
                raw.replace(b"seconds='secs'", b'seconds=read()'),
                raw.replace(b'SECS:()=>seconds', b'SECS:(input)=>seconds'),
                raw.replace(b'SECS:()=>seconds', b'SECS:()=>outside'),
                raw.replace(b'SECS:()=>seconds', b'SECS:()=>seconds,SECS:()=>seconds'),
                raw + raw):
            with self.subTest(raw=changed), self.assertRaises(ValueError):
                evidence.locale_chunk_exports(changed, ['SECS'])

    def test_declared_absence_is_verified_without_fabricating_a_translation(self):
        raw = self.source()
        selected = evidence.locale_chunk_exports(raw, ['TITLE', 'MISSING'], absent_keys=['MISSING'])
        self.assertEqual(selected['values'], {'TITLE': 'Keyboard Shortcuts'})
        self.assertEqual(selected['absent_keys'], ['MISSING'])
        self.assertEqual(evidence.locale_chunk_exports(raw, ['MISSING'], absent_keys=['MISSING'])['values'], {})
        for keys, absent in ((['SECS'], ['SECS']), (['TITLE'], ['MISSING']),
                             (['MISSING'], ['MISSING', 'MISSING']), (None, ['MISSING'])):
            with self.subTest(keys=keys, absent=absent), self.assertRaises(ValueError):
                evidence.locale_chunk_exports(raw, keys, absent_keys=absent)


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

    def test_iteration_lexical_bindings_shadow_factory_require_only_inside_the_loop(self):
        for declaration in (b"const", b"let"):
            for operator in (b"in", b"of"):
                raw = (b'(m,e,o)=>{for(' + declaration + b' o ' + operator
                       + b' values)o(1);outside(o)}')
                with self.subTest(declaration=declaration, operator=operator):
                    found = self.binding(raw, "o", b'o(1)')
                    self.assertEqual(found["resolution"], "iteration_requires_value_analysis")
                    self.assertEqual(found["scope"]["syntax"], "for_in_statement")
                    self.assertEqual(found["body"].encode(), raw[found["start_byte"]:found["end_byte"]])
                    self.assertEqual(self.binding(raw, "o", b'outside(o)')["resolution"],
                                     "parameter_requires_caller_analysis")

    def test_iteration_var_binding_belongs_to_the_enclosing_function(self):
        raw = b'const value="outer";function f(){for(var value of items){inside(value)}outside(value)}after(value)'
        for marker in (b'inside(value)', b'outside(value)'):
            found = self.binding(raw, "value", marker)
            self.assertEqual(found["resolution"], "iteration_requires_value_analysis")
            self.assertEqual(found["scope"]["syntax"], "function_declaration")
        self.assertEqual(self.binding(raw, "value", b'after(value)')["body"], 'value="outer"')

    def test_iteration_assignment_does_not_declare_a_lexical_binding(self):
        raw = b'const value="outer";function f(){for(value of items){inside(value)}}'
        self.assertEqual(self.binding(raw, "value", b'inside(value)')["body"], 'value="outer"')

    def test_iteration_destructuring_indexes_only_bound_names(self):
        raw = (b'const key="outer-key",fallback="outer-fallback";'
               b'function f(){for(const {[key]:o,nested:[x,...rest],val=fallback} of items)'
               b'{inside(o,x,rest,val,key,fallback)}}')
        for name in ("o", "x", "rest", "val"):
            self.assertEqual(self.binding(raw, name, b'inside(')["resolution"],
                             "iteration_requires_value_analysis")
        for name in ("key", "fallback"):
            self.assertEqual(self.binding(raw, name, b'inside(')["resolution"], "declaration")

    def test_await_iteration_and_nested_closures_keep_the_iteration_owner(self):
        raw = b'async function f(o){for await(const o of items){const closure=()=>inside(o)}outside(o)}'
        self.assertEqual(self.binding(raw, "o", b'inside(o)')["resolution"],
                         "iteration_requires_value_analysis")
        self.assertEqual(self.binding(raw, "o", b'outside(o)')["resolution"],
                         "parameter_requires_caller_analysis")


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

    def test_standalone_script_entry_keeps_importer_semantics_separate(self):
        path = self.source(".analysis/inputs/frontend/shared/runtime", b"const runtime={};")
        original = families.identity(path)
        url = "https://apps.razer.com/synapse/assets/js/runtime.js"
        result = evidence.frontend_deployment_sources({"entry_url": url,
            "files": [{"url": url, "content_type": "text/javascript", **original}],
            "source_current_during_acquisition": True,
            "stability": [{"url": url, "unchanged": True,
                           "before_sha256": original["sha256"], "after_sha256": original["sha256"]}]})
        self.assertFalse(result["entry_is_html"])
        self.assertEqual(result["failures"], [])
        self.assertEqual(result["sources"][0]["deployment_role"], "standalone_entry_requires_importer_audit")
        self.assertTrue(result["sources"][0]["identity_verified"])
        self.assertFalse(result["source_semantics_complete"])

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
                patch.object(workspace, "rust_inputs", return_value=[]), \
                patch.object(workspace, "rust_test_scope", return_value=None), redirect_stdout(io.StringIO()):
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


class PlatformContractPublicationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        root_patch = patch.object(analysis, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        self.directory = self.root / ".analysis/inputs/platform-abi-current/wdf"
        self.directory.mkdir(parents=True)
        self.receipt = self.directory / "acquisition.json"
        self.repository = "https://github.com/microsoft/Windows-Driver-Frameworks"
        self.commit = "b" * 40
        self.document = {"schema_version": 1, "scope": "OS queue ownership and KMDF ABI only",
            "version": "KMDF 1.15", "target_execution": False,
            "repositories": [{"repository": self.repository, "commit": self.commit,
                "license_paths": [(self.directory / "LICENSE").relative_to(self.root).as_posix()]}],
            "files": []}
        for name, repository_path, raw, claims in [
            ("wdfrequest.h", "src/publicinc/wdf/kmdf/1.15/wdfrequest.h",
             b"WdfRequestForwardToIoQueue\nWdfObjectDelete\n", [
                 {"symbol": "WdfRequestForwardToIoQueue", "line_start": 1, "line_end": 1,
                  "claim": "The declared request forwarding API"}]),
            ("LICENSE", "LICENSE", b"Retained official license fixture\n", []),
        ]:
            path = self.directory / name
            path.write_bytes(raw)
            url = "https://raw.githubusercontent.com/microsoft/Windows-Driver-Frameworks/" + self.commit + "/" + repository_path
            self.document["files"].append({"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest(), "repository": self.repository, "commit": self.commit,
                "repository_path": repository_path, "url": url, "final_url": url, "http_status": 200,
                "version": "KMDF 1.15", "declarations": claims})
        self.index = self.root / ".analysis/inputs.json"
        self.unrelated = {"path": ".analysis/inputs/unrelated", "bytes": 1, "sha256": "d" * 64}
        analysis.save(self.index, {"schema_version": 1, "inputs": [self.unrelated]})

    def publish(self, document=None):
        analysis.save(self.receipt, self.document if document is None else document)
        return analysis.publish_platform_contract(self.receipt)

    def test_pinned_sources_declaration_lines_and_license_publish_once(self):
        result = self.publish()
        self.assertEqual((result["files"], result["declarations"]), (2, 1))
        document = json.loads(self.index.read_bytes())
        self.assertIn(self.unrelated, document["inputs"])
        self.assertEqual(len(document["inputs"]), 4)
        verification = document["source_publications"][0]["verification"]
        self.assertFalse(verification["network_requests"])
        self.assertIn("not Razer protocol evidence", verification["scope"])
        previous = self.index.read_bytes()
        self.publish()
        self.assertEqual(self.index.read_bytes(), previous)

    def test_other_origin_floating_commit_redirect_and_wrong_abi_fail(self):
        for field, value in [("url", "https://example.com/source.h"),
                             ("url", self.document["files"][0]["url"].replace(self.commit, "main")),
                             ("final_url", "https://example.com/redirect.h"),
                             ("commit", "c" * 40), ("repository_path", "src/kmdf/1.17/wdfrequest.h")]:
            candidate = json.loads(json.dumps(self.document))
            candidate["files"][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.publish(candidate)

    def test_claim_text_bounds_and_actual_source_bytes_are_checked(self):
        for field, value in [("symbol", "InventedFunction"), ("line_start", 2),
                             ("line_end", 30), ("line_start", True), ("claim", ""), ("sha256", "0" * 64)]:
            candidate = json.loads(json.dumps(self.document))
            candidate["files"][0]["declarations"][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.publish(candidate)
        source = self.directory / "wdfrequest.h"
        source.write_bytes(source.read_bytes().replace(b"Forward", b"Missing"))
        with self.assertRaisesRegex(ValueError, "original bytes"):
            self.publish()

    def test_missing_license_duplicate_member_and_unknown_repository_fail(self):
        missing = json.loads(json.dumps(self.document))
        missing["files"].pop()
        repeated = json.loads(json.dumps(self.document))
        repeated["files"].append(repeated["files"][0])
        unknown = json.loads(json.dumps(self.document))
        unknown["repositories"][0]["repository"] = "https://github.com/example/Windows-Driver-Frameworks"
        false_license = json.loads(json.dumps(self.document))
        false_license["repositories"][0]["license_paths"] = [false_license["files"][0]["path"]]
        for candidate in (missing, repeated, unknown, false_license):
            with self.subTest(candidate=candidate), self.assertRaises(ValueError):
                self.publish(candidate)

    def test_outside_acquisition_directory_and_execution_promotion_fail(self):
        outside = self.directory.parent / "outside.h"
        outside.write_bytes((self.directory / "wdfrequest.h").read_bytes())
        candidate = json.loads(json.dumps(self.document))
        candidate["files"][0]["path"] = outside.relative_to(self.root).as_posix()
        with self.assertRaises(ValueError):
            self.publish(candidate)
        candidate = json.loads(json.dumps(self.document))
        candidate["target_execution"] = True
        with self.assertRaises(ValueError):
            self.publish(candidate)

    def test_unified_reconcile_routes_explicit_platform_receipt(self):
        analysis.save(self.receipt, self.document)
        arguments = SimpleNamespace(frontend=None, publication=None, instruction_manual=None,
            platform_contract=[self.receipt.relative_to(self.root).as_posix()])
        with redirect_stdout(io.StringIO()) as output:
            analysis.reconcile_source_index(arguments)
        report = json.loads(output.getvalue())
        self.assertEqual(report["platform_contracts"], arguments.platform_contract)
        self.assertTrue(report["passed"])
        self.assertFalse(report["target_execution"])


class SelectedGapRefreshTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.patch = patch.object(analysis, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.path = self.root / ".analysis/results/gaps.json"
        self.owner = ".analysis/results/frontend/mouse-mw-audit.json"
        self.host = ".analysis/results/host-current/local-audit.json"
        self.native = ".analysis/results/native/"+"a"*64+"/capture.json"
        self.other = {"rows": [{"kind": "preserved", "finding": "unrelated failure"}]}
        analysis.save(self.path, {"owners": {"unrelated": self.other, self.owner: [{"kind": "old"}]},
                                 "derived_owners": ["unrelated", self.owner], "manual": {"keep": True}})

    def test_batch_refresh_preserves_unrelated_owners_and_all_failure_boundaries(self):
        analysis.save(self.root / self.owner, {"queries": {"power": {"product_id": 182,
            "failures": [{"stage": "cafe:hexrays", "error": "retained failure"}],
            "unresolved_chains": ["unconfirmed real transport"]}}})
        analysis.save(self.root / self.host, {"window/lifecycle": {"source_chain_complete": False,
            "failures": ["actual query failure"]}})
        analysis.save(self.root / self.native, {"capture_complete": False})
        resolution = {"remaining_failures": [{"rva": "0x1000", "error": "retained decompile failure"}],
                      "verification_failures": [{"reason": "unresolved conflict"}]}
        with patch.object(analysis, "native_reanalysis_resolution", return_value=resolution):
            ledger = analysis.refresh_gaps([self.owner, self.host, self.native])
        self.assertEqual(ledger["owners"]["unrelated"], self.other)
        self.assertEqual(ledger["manual"], {"keep": True})
        self.assertEqual([row["kind"] for row in ledger["owners"][self.owner]],
                         ["analyzer_query_failure", "semantic_query_chain_unresolved", "page_implementation_acceptance_pending"])
        self.assertEqual(ledger["owners"][self.owner][0]["product_id"], 182)
        self.assertEqual(ledger["owners"][self.host][1]["pointer"], "/window~1lifecycle")
        self.assertEqual([row["kind"] for row in ledger["owners"][self.native]],
                         ["native_capture_failure", "native_supplement_verification_failure", "native_capture_incomplete",
                          "semantic_audit_pending", "implementation_acceptance_pending"])
        self.assertEqual({row["path"] for row in ledger["selected_refresh"]["sources"]},
                         {self.owner, self.host, self.native})
        self.assertFalse(ledger["selected_refresh"]["target_execution"])
        self.assertEqual(set(ledger["derived_owners"]), {"unrelated", self.owner, self.host, self.native})

    def test_changed_source_refuses_publication_without_touching_existing_ledger(self):
        analysis.save(self.root / self.owner, {"queries": {"power": {"failures": []}}})
        previous = self.path.read_bytes()
        derive = analysis.semantic_result_gaps
        def change(data, name):
            rows = derive(data, name)
            analysis.save(self.root / name, {"queries": {"power": {"failures": ["late failure"]}}})
            return rows
        with patch.object(analysis, "semantic_result_gaps", side_effect=change):
            with self.assertRaisesRegex(ValueError, "changed during"):
                analysis.refresh_gaps([self.owner])
        self.assertEqual(self.path.read_bytes(), previous)

    def test_invalid_ambiguous_and_outside_owner_paths_never_change_the_ledger(self):
        previous = self.path.read_bytes()
        for owners in ([], [self.owner, self.owner], ["../outside.json"],
                       [".analysis/inputs/acquisition.json"],
                       [".analysis/results/frontend/mouse-shared-page-layout-audit.json"],
                       [".analysis/results/frontend/../frontend/mouse-mw-audit.json"]):
            with self.subTest(owners=owners), self.assertRaises(ValueError):
                analysis.refresh_gaps(owners)
            self.assertEqual(self.path.read_bytes(), previous)

    def test_malformed_or_missing_owner_never_publishes_partial_batch(self):
        analysis.save(self.root / self.owner, [])
        analysis.save(self.root / self.host, {"window": {"failures": []}})
        previous = self.path.read_bytes()
        with self.assertRaisesRegex(ValueError, "result object"):
            analysis.refresh_gaps([self.host, self.owner])
        self.assertEqual(self.path.read_bytes(), previous)
        (self.root / self.owner).unlink()
        with self.assertRaises(FileNotFoundError):
            analysis.refresh_gaps([self.host, self.owner])
        self.assertEqual(self.path.read_bytes(), previous)

    def test_cli_routes_all_explicit_owners_to_single_shared_refresh(self):
        parser = argparse.ArgumentParser()
        analysis.add_commands(parser.add_subparsers(dest="command"))
        args = parser.parse_args(["refresh-gaps", "--owner", self.owner, "--owner", self.host])
        with patch.object(analysis, "refresh_gaps", return_value={"owners": {}}) as refresh, \
             redirect_stdout(io.StringIO()):
            analysis.run(args)
        refresh.assert_called_once_with([self.owner, self.host])


VERIFICATION_INPUT_NAMES = ("tools/evidence.py", "tools/test_evidence.py", "tools/analysis.py", "tools/frontend.py",
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
                                      "tools/ida/capture.py", "tools/ida/capture_io.py", "tools/ida/capture_values.py",
                                      "tools/reverse_pending.py", "tools/test_reverse_pending.py",
                                      "tools/test_ida_capture_values.py", "tools/analysis_storage.py",
                                      "tools/test_analysis_storage.py", "tools/native_adjudication.py",
                                      "tools/test_native_adjudication.py", "tools/native_callsites.py",
                                       "tools/test_native_callsites.py", "tools/test_ida_capture.py",
                                       "tools/native_attempt_history.py", "tools/test_native_attempt_history.py",
                                       "tools/native_boundaries.py", "tools/test_native_boundaries.py",
                                       "tools/native_embedded.py", "tools/test_native_embedded.py",
                                       "tools/native_ia64_contracts.py", "tools/test_native_ia64_contracts.py",
                                       "tools/native_import_forwarders.py", "tools/test_native_import_forwarders.py",
                                       "tools/native_source_correspondence.py", "tools/test_native_source_correspondence.py",
                                       "tools/native_static_reviews.py", "tools/test_native_static_reviews.py",
                                       "tools/test_frontend.py")


def verification_input_snapshot():
    rows = []
    for name in VERIFICATION_INPUT_NAMES:
        try:
            raw = (analysis.ROOT / name).read_bytes()
        except OSError as error:
            rows.append({"path": name, "sha256": None, "error": str(error)})
        else:
            rows.append({"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
    return rows


def run_verified_suite(suite, inputs, *, runner=None, selected_tests=None):
    result, interrupted = None, None
    try:
        result = (runner or unittest.TextTestRunner(verbosity=2)).run(suite)
    except BaseException as error:
        interrupted = {"type": type(error).__name__, "message": str(error)}
        raise
    finally:
        final_inputs = verification_input_snapshot()
        before, after = ({row["path"]: row for row in snapshot} for snapshot in (inputs, final_inputs))
        changed = [{"path": name, "before": before.get(name), "after": after.get(name)}
                   for name in sorted(before.keys() | after.keys()) if before.get(name) != after.get(name)]
        complete = bool(inputs) and all(row.get("sha256") for row in inputs+final_inputs)
        executed = result.testsRun-len(result.skipped) if result is not None else 0
        passed = bool(result is not None and result.wasSuccessful() and executed > 0 and complete and not changed)
        report = {"schema_version": 1, "recorded_at_utc": analysis.stamp(),
                  "tests_run": result.testsRun if result is not None else 0, "executed_tests": executed,
                  "failures": len(result.failures) if result is not None else 0,
                  "errors": len(result.errors) if result is not None else 0,
                  "skipped": len(result.skipped) if result is not None else 0,
                  "failure_details": [{"test": test.id(), "traceback": detail} for test, detail in result.failures] if result is not None else [],
                  "error_details": [{"test": test.id(), "traceback": detail} for test, detail in result.errors] if result is not None else [],
                  "test_selection": selected_tests,
                  "passed": passed, "inputs": inputs, "final_inputs": final_inputs,
                  "input_changes": changed, "snapshot_stable": complete and not changed,
                  "interrupted": interrupted, "target_execution": False, "real_device_operations": False}
        import workspace
        destination = analysis.ROOT / ".analysis/results/tool-verification.json"
        with analysis.index_lock(destination.with_suffix(".lock")):
            if destination.exists():
                previous = destination.read_bytes()
                if not isinstance(json.loads(previous), dict):
                    raise ValueError("Previous tool verification receipt is not an object")
                report["previous_receipt"] = workspace._publish_immutable_evidence(
                    previous, directory=".analysis/verification", suffix=".json")
            analysis.save(destination, report)
    return 0 if passed else 1


class ToolVerificationSnapshotTests(unittest.TestCase):
    def verify(self, action=None, *, empty=False):
        import workspace
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "tools/owned.py"
            source.parent.mkdir()
            source.write_bytes(b"original source bytes")
            with patch.object(analysis, "ROOT", root), \
                 patch.object(workspace, "ROOT", root), \
                 patch.object(sys.modules[__name__], "VERIFICATION_INPUT_NAMES", ("tools/owned.py",)):
                inputs = verification_input_snapshot()
                suite = unittest.TestSuite([] if empty else [unittest.FunctionTestCase(lambda: action(source) if action else None)])
                status = run_verified_suite(suite, inputs, runner=unittest.TextTestRunner(stream=io.StringIO()))
                report = json.loads((root / ".analysis/results/tool-verification.json").read_bytes())
            return status, report

    def test_stable_executed_test_preserves_both_exact_input_snapshots(self):
        status, report = self.verify()
        self.assertEqual(status, 0)
        self.assertTrue(report["passed"])
        self.assertTrue(report["snapshot_stable"])
        self.assertEqual(report["tests_run"], 1)
        self.assertEqual(report["executed_tests"], 1)
        self.assertEqual(report["inputs"], report["final_inputs"])
        self.assertEqual(report["input_changes"], [])
        self.assertIsNone(report["interrupted"])
        self.assertFalse(report["target_execution"])

    def test_passing_test_cannot_publish_changed_source_as_tested_input(self):
        status, report = self.verify(lambda source: source.write_bytes(b"changed source bytes"))
        self.assertEqual(status, 1)
        self.assertFalse(report["passed"])
        self.assertFalse(report["snapshot_stable"])
        self.assertEqual(report["failures"], 0)
        self.assertEqual(report["input_changes"][0]["before"], report["inputs"][0])
        self.assertEqual(report["input_changes"][0]["after"], report["final_inputs"][0])
        self.assertNotEqual(report["inputs"][0]["sha256"], report["final_inputs"][0]["sha256"])

    def test_missing_final_input_retains_failure_instead_of_old_success(self):
        status, report = self.verify(lambda source: source.unlink())
        self.assertEqual(status, 1)
        self.assertFalse(report["passed"])
        self.assertFalse(report["snapshot_stable"])
        self.assertIsNone(report["final_inputs"][0]["sha256"])
        self.assertTrue(report["final_inputs"][0]["error"])

    def test_failed_tests_and_zero_executed_tests_never_pass(self):
        def fail(source):
            raise AssertionError("independent failure")

        def skip(source):
            raise unittest.SkipTest("explicit execution boundary")

        for action, empty in ((fail, False), (skip, False), (None, True)):
            with self.subTest(action=action, empty=empty):
                status, report = self.verify(action, empty=empty)
                self.assertEqual(status, 1)
                self.assertFalse(report["passed"])
                self.assertTrue(report["snapshot_stable"])
                self.assertEqual(report["failures"], 1 if action is fail else 0)
                self.assertEqual(report["executed_tests"], 1 if action is fail else 0)

    def test_interruption_replaces_previous_success_and_propagates(self):
        import workspace
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "owned.py"
            source.write_bytes(b"stable")
            with patch.object(analysis, "ROOT", root), \
                 patch.object(workspace, "ROOT", root), \
                 patch.object(sys.modules[__name__], "VERIFICATION_INPUT_NAMES", ("owned.py",)):
                path = root / ".analysis/results/tool-verification.json"
                analysis.save(path, {"passed": True})
                runner = MagicMock()
                runner.run.side_effect = KeyboardInterrupt("cancelled")
                with self.assertRaises(KeyboardInterrupt):
                    run_verified_suite(unittest.TestSuite(), verification_input_snapshot(), runner=runner)
                report = json.loads(path.read_bytes())
            self.assertFalse(report["passed"])
            self.assertEqual(report["interrupted"]["type"], "KeyboardInterrupt")
            self.assertEqual(report["tests_run"], 0)

    def test_selected_retry_retains_exact_failed_receipt_and_traceback(self):
        import workspace
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "owned.py"
            source.write_bytes(b"stable")
            destination = root / ".analysis/results/tool-verification.json"
            with patch.object(analysis, "ROOT", root), patch.object(workspace, "ROOT", root), \
                 patch.object(sys.modules[__name__], "VERIFICATION_INPUT_NAMES", ("owned.py",)):
                def fail():
                    raise AssertionError("retained independent failure")
                runner = unittest.TextTestRunner(stream=io.StringIO())
                self.assertEqual(run_verified_suite(unittest.TestSuite([unittest.FunctionTestCase(fail)]),
                    verification_input_snapshot(), runner=runner, selected_tests=["test_evidence.synthetic"]), 1)
                failed_bytes = destination.read_bytes()
                failed = json.loads(failed_bytes)
                self.assertIn("retained independent failure", failed["failure_details"][0]["traceback"])
                self.assertEqual(failed["test_selection"], ["test_evidence.synthetic"])
                self.assertEqual(run_verified_suite(unittest.TestSuite([unittest.FunctionTestCase(lambda: None)]),
                    verification_input_snapshot(), runner=runner, selected_tests=["test_evidence.retry"]), 0)
                passed = json.loads(destination.read_bytes())
                retained = root / passed["previous_receipt"]["path"]
                self.assertEqual(retained.read_bytes(), failed_bytes)
                self.assertEqual(passed["previous_receipt"]["sha256"], hashlib.sha256(failed_bytes).hexdigest())
                self.assertFalse(json.loads(retained.read_bytes())["passed"])
                self.assertEqual(passed["test_selection"], ["test_evidence.retry"])

    def test_selected_suite_rejects_unknown_duplicate_or_empty_scopes_before_loading(self):
        with patch.object(unittest.defaultTestLoader, "loadTestsFromName") as loader:
            for names in [[], ["os"], ["test_evidence.__invalid__()"], ["test_host", "test_host"]]:
                with self.subTest(names=names), self.assertRaises(ValueError):
                    run(names)
            loader.assert_not_called()

    def test_cli_preserves_all_explicit_selected_scopes(self):
        parser = argparse.ArgumentParser()
        evidence.add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["verify-analysis-tools", "--test", "test_host", "--test", "test_evidence.ToolVerificationSnapshotTests"])
        self.assertEqual(args.test, ["test_host", "test_evidence.ToolVerificationSnapshotTests"])


class KeyboardNavigationSourceTests(unittest.TestCase):
    LOADER = (
        'function(){if(window.RzAccessibility)return Promise.resolve(window.RzAccessibility);'
        'if(pending)return pending;const e=window.__RZ_A11Y_SRC__||'
        '"/synapse/assets/js/keyboard-nav/rz-accessibility.min.js";'
        'return pending=new Promise((t,o)=>{const n=document.createElement("script");'
        'n.src=e,n.async=!0,n.setAttribute("data-rz-accessibility","");'
        'const i=setTimeout(()=>r("timed out after 10000ms"),1e4),'
        'r=t=>{clearTimeout(i),pending=null,n.remove(),o(new Error("[rz-accessibility] "+t+": "+e))};'
        'n.addEventListener("load",()=>{clearTimeout(i),window.RzAccessibility?t(window.RzAccessibility):'
        'r("loaded but window.RzAccessibility is not defined")}),'
        'n.addEventListener("error",()=>r("failed to load")),document.head.appendChild(n)}),pending}'
    )

    def setUp(self):
        self.audit = host.HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)

    def source(self, body, name="main.js"):
        raw = body.encode("utf-8")
        path = self.root / ".analysis/inputs/frontend/keyboard-fixture" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}

    def body(self):
        return ('(()=>{let pending=null;const Wrapper=({children:t})=>((0,React.useEffect)(()=>{'
                'let e=!1;return ' + self.LOADER + '().then(t=>{e||t&&t.init(document.getElementById("root"))})'
                '.catch(e=>{console.warn("[rz-accessibility] failed to load keyboard-navigation runtime:",e)}),'
                '()=>{e=!0}},[]),(0,jsx.jsx)(jsx.Fragment,{children:t}));'
                'ReactDOM.createRoot(document.getElementById("root")).render((0,jsx.jsx)(Wrapper,{children:null}));})();')

    def loaders(self, transform=lambda value: value):
        source = self.source(transform(self.body()))
        return evidence.keyboard_navigation_loaders(self.audit, [source], source)

    def test_complete_loader_effect_cache_and_root_are_retained(self):
        row, = self.loaders()
        self.assertTrue(row["late_completion_guard"])
        self.assertTrue(row["failure_resets_cache_and_removes_script"])
        self.assertEqual(row["timeout_ms"], 10000)
        self.assertFalse(row["source_unmount_calls_runtime_destroy"])
        self.assertEqual(len(row["roots"]), 1)
        self.assertEqual(len(row["roots"][0]["startup_invocations"]), 1)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_renamed_retained_promise_and_component_keep_the_same_behavior(self):
        row, = self.loaders(lambda value: value.replace("pending", "cachedRuntime").replace("Wrapper", "RootBoundary"))
        self.assertEqual(self.audit.owners[row["cache"]]["body"], "cachedRuntime=null")

    def test_timeout_failure_cleanup_and_real_promise_are_exact(self):
        for before, after in ((",1e4)", ",5e3)"), ("pending=null,n.remove()", "pending=null"),
                              ("new Promise", "new OtherPromise")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "loader differs"):
                self.loaders(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = host.HostLocalAudit()

    def test_late_completion_and_unmount_guard_cannot_be_dropped(self):
        for before, after in (("e||t&&t.init", "t&&t.init"), ("()=>{e=!0}", "()=>{e=!1}")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "effect changed"):
                self.loaders(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = host.HostLocalAudit()

    def test_hook_and_dependency_list_are_checked(self):
        for before, after in (("React.useEffect", "React.useMemo"), ("},[])", "},[value])")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "hook or dependency"):
                self.loaders(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = host.HostLocalAudit()

    def test_cache_must_have_the_audited_initial_value(self):
        with self.assertRaisesRegex(ValueError, "initial null"):
            self.loaders(lambda value: value.replace("let pending=null", "let pending=existing"))

    def test_declared_but_unmounted_wrapper_is_not_a_runtime_path(self):
        with self.assertRaisesRegex(ValueError, "no statically resolved mounted JSX"):
            self.loaders(lambda value: value.replace('(0,jsx.jsx)(Wrapper,{children:null})', "null"))

    def test_root_inside_an_uncalled_function_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "uninvoked function"):
            self.loaders(lambda value: value.replace("ReactDOM.createRoot", "function notStarted(){ReactDOM.createRoot")
                .replace("{children:null}));})();", "{children:null}));}})();"))

    def test_root_element_must_match_effect_initialization_target(self):
        with self.assertRaisesRegex(ValueError, "DOM target"):
            self.loaders(lambda value: value.replace('ReactDOM.createRoot(document.getElementById("root"))',
                                                     'ReactDOM.createRoot(document.getElementById("other"))'))

    def test_root_element_can_use_its_actual_lexical_binding(self):
        row, = self.loaders(lambda value: value.replace("ReactDOM.createRoot(document",
            'const target=document.getElementById("root");ReactDOM.createRoot(document', 1)
            .replace('ReactDOM.createRoot(document.getElementById("root"))', "ReactDOM.createRoot(target)"))
        self.assertEqual(row["roots"][0]["dom_target"]["syntax"], "call_expression")

    def test_legacy_render_traces_actual_interop_and_reactdom_external(self):
        prefix = ('const modules={5206:e=>{"use strict";e.exports=ReactDOM}},cache={};'
                  'function require(id){var found=cache[id];if(void 0!==found)return found.exports;'
                  'var entry=cache[id]={id:id,loaded:!1,exports:{}};'
                  'return modules[id](entry,entry.exports,require),entry.loaded=!0,entry.exports}'
                  'require.n=e=>{var t=e&&e.__esModule?()=>e.default:()=>e;return require.d(t,{a:t}),t};'
                  'const client=require(5206),getter=require.n(client);')
        row, = self.loaders(lambda value: value.replace("(()=>{", "(()=>{" + prefix, 1)
            .replace('ReactDOM.createRoot(document.getElementById("root")).render((0,jsx.jsx)(Wrapper,{children:null}))',
                     'getter().render((0,jsx.jsx)(Wrapper,{children:null}),document.getElementById("root"))'))
        self.assertEqual(len(row["roots"][0]["reactdom_legacy_aliases"]), 3)

    def test_textual_override_name_is_not_a_loader(self):
        source = self.source('const unrelated="__RZ_A11Y_SRC__";')
        self.assertEqual(evidence.keyboard_navigation_loaders(self.audit, [source], source), [])

    def test_actual_create_root_commonjs_alias_is_resolved(self):
        prefix = ('const modules={260:(e,t,n)=>{"use strict";var o=n(5206);t.H=o.createRoot,o.hydrateRoot},'
                  '5206:e=>{"use strict";e.exports=ReactDOM}},cache={};'
                  'function require(id){var found=cache[id];if(void 0!==found)return found.exports;'
                  'var entry=cache[id]={id:id,loaded:!1,exports:{}};'
                  'return modules[id].call(entry.exports,entry,entry.exports,require),entry.loaded=!0,entry.exports}'
                  'const client=require(260);')
        row, = self.loaders(lambda value: value.replace("(()=>{", "(()=>{" + prefix, 1)
                           .replace("ReactDOM.createRoot(document", "(0,client.H)(document"))
        self.assertEqual(len(row["roots"][0]["create_root_aliases"]), 2)

    def shared(self, transform=lambda value: value):
        body = ('var RzAccessibility=function(api){"use strict";'
                'function init(root){}function open(){}function close(){}function isOpen(){}'
                'function isReady(){}const runtime={};return api.init=init,api.openGuide=open,api.closeGuide=close,'
                'api.isGuideOpen=isOpen,api.isInitialized=isReady,api.default=runtime,api}({});')
        return evidence.keyboard_navigation_shared_runtime(self.audit, self.source(transform(body), "runtime.js"))

    def test_shared_iife_retains_the_actual_exported_implementations(self):
        row = self.shared()
        self.assertEqual(set(row["exports"]), {"init", "openGuide", "closeGuide", "isGuideOpen", "isInitialized", "default"})
        self.assertTrue(self.audit.owners[row["iife"]]["body"].startswith("RzAccessibility=function"))
        self.assertFalse(row["target_execution"])

    def test_shared_runtime_cannot_borrow_unknown_repeated_or_dynamic_exports(self):
        for before, after in (("api.init=init", "api.init=unknown"),
                              ("api.openGuide=open", "api.openGuide=open,api.openGuide=open"),
                              ("api.openGuide=open", "api.openGuide=makeOpen()"), ("}({})", "}({initial:true})")):
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.shared(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = host.HostLocalAudit()

    def translation(self, transform=lambda value: value):
        getter = (
            'p=(e,t={})=>{var s;const n={...t};let a=n._lang;if(null===l)return"";'
            'a?delete n._lang:a=h(),l[a]||(a=i.Nm);'
            'let o=null!==(s=l[a][e])&&void 0!==s?s:l[i.Nm][e];'
            'if(o){let e;for(const[t,s]of Object.entries(n))e=new RegExp(`{{${t}}}`,"g"),o=o.replace(e,s)}'
            'else console.debug("[getTextItem][%s]",e);const r=o||e;'
            'return t.a&&t.aEnd&&"string"===typeof r&&!r.includes("<a")?`${t.a}${r}${t.aEnd}`:r}'
        )
        updater = (
            'd=(e,t,s=()=>{})=>a=>{const o=c(e);null===l&&a({type:n.Ztp,payload:"unknown"});'
            'new Promise(e=>{t?t().then(({default:t})=>{r=t,e(r)}):e(r)})'
            '.then(e=>{const t=[e[i.Nm]()];o!==i.Nm&&t.push(e[o]()),Promise.all(t).then(([e,r])=>{'
            'l={[i.Nm]:e},t.length>1&&(l[o]=r),a({type:n.Ztp,payload:o}),s()})})}'
        )
        body = (
            'const modules={1:(e,t,n)=>{const keys=n(2),H=n(3);function Panel(){return jsx(Block,'
            '{title:keys.r6A,children:[jsx(H.A,{text:keys.o1z}),jsx(H.A,{text:keys.IFV}),jsx(H.A,{text:keys.njo})]})}},'
            '2:(e,t,n)=>{n.d(t,{r6A:()=>title,o1z:()=>action,IFV:()=>chroma,njo:()=>synapse});'
            'const title="KEYBOARD_NAVIGATION",action="VIEW_ALL_SHORTCUTS",chroma="KEYBOARD_NAVIGATION_CHROMA_MSG",'
            'synapse="KEYBOARD_NAVIGATION_SYNAPSE_MSG"},'
            '3:(e,t,n)=>{n.d(t,{A:()=>Connected});const i=n(4),Translator=({text:e,lang:s})=>(0,i.getTextItem)(e,{_lang:s});'
            'const Connected=connect(state,actions)(Translator)},'
            '4:(e,t,s)=>{s.d(t,{getTextItem:()=>p,updateLanguage:()=>d});'
            'const i=s(5),n={Ztp:"language"};let r,l=null,other=null;const c=e=>e,h=()=>"en";const ' + getter + ';const ' + updater + '},'
            '5:(e,t,n)=>{n.d(t,{Nm:()=>english});const english="en"}};'
        )
        source = self.source(transform(body))
        component = next(node for node in host._help_nodes(self.audit.tree(source).root_node)
                         if node.type == "function_declaration" and node.child_by_field_name("name").text == b"Panel")
        return evidence.keyboard_navigation_translation_fallback(self.audit, [source], source, component)

    def test_chroma_translation_keys_reader_publisher_and_english_fallback_are_connected(self):
        result = self.translation()
        self.assertEqual(result["default_language"], "en")
        self.assertEqual(result["missing_key"], "english_then_source_key")
        self.assertEqual(set(result["constants"]), {"KEYBOARD_NAVIGATION", "VIEW_ALL_SHORTCUTS",
                                                   "KEYBOARD_NAVIGATION_CHROMA_MSG", "KEYBOARD_NAVIGATION_SYNAPSE_MSG"})
        self.assertEqual(self.audit.owners[result["dictionary"]]["body"], "l=null")

    def test_translation_changed_fallback_language_key_or_loading_order_is_rejected(self):
        for before, after in (('english="en"', 'english="de"'), ('title="KEYBOARD_NAVIGATION"', 'title="OTHER"'),
                              ("const t=[e[i.Nm]()]", "const t=[]"), ("s:l[i.Nm][e]", "s:e")):
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.translation(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = host.HostLocalAudit()

    def test_translation_cannot_claim_a_different_dictionary_publisher(self):
        def change(value):
            before, updater = value.split("const d=", 1)
            return before + "const d=" + updater.replace("null===l", "null===other").replace("l={[", "other={[").replace("l[o]", "other[o]")
        with self.assertRaisesRegex(ValueError, "different dictionaries"):
            self.translation(change)


VERIFICATION_TEST_MODULES = ("test_evidence", "test_rust_data", "test_typed_tables", "test_windows_installer", "test_workspace",
                   "test_resource_scope", "test_asset_release", "test_host", "test_native_runtime", "test_families",
                   "test_reverse_pending", "test_ida_capture_values", "test_analysis_storage",
                    "test_native_adjudication", "test_native_callsites", "test_ida_capture",
                    "test_native_attempt_history", "test_native_boundaries", "test_native_embedded",
                    "test_native_ia64_contracts", "test_native_import_forwarders",
                    "test_native_source_correspondence", "test_native_static_reviews", "test_frontend")


def run(selected_tests=None):
    if selected_tests is not None and (not isinstance(selected_tests, (tuple, list)) or not selected_tests
            or any(not isinstance(name, str) or name.split(".")[0] not in VERIFICATION_TEST_MODULES
                   or not all(part.isidentifier() for part in name.split(".")) for name in selected_tests)
            or len(set(selected_tests)) != len(selected_tests)):
        raise ValueError("Select distinct maintained tool test modules, classes or methods")
    inputs = verification_input_snapshot()
    suite = unittest.TestSuite()
    for name in selected_tests or VERIFICATION_TEST_MODULES:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromName(name))
    return run_verified_suite(suite, inputs, selected_tests=list(selected_tests) if selected_tests is not None else None)
