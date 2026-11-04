"""Pure static PE and source-contract tests; no target is loaded or executed."""
from __future__ import annotations

import argparse
import base64
import concurrent.futures
import copy
import hashlib
import json
import re
import struct
import subprocess
import tempfile
import threading
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from host import (HostLocalAudit, add_commands, audit_host_local, audit_product_chroma_observation, publish_host_static_query,
                  acquire_host, acquire_host_release, host_release_metadata, verified_host_release_receipt,
                  extract_asar, _host_release_declaration, _publish_host_bytes,
                  collect_profiles_static, publish_profiles_static,
                  verify_profiles_owner, verify_profiles_static_receipt,
                  help_system_mount_layout, help_system_native_declaration, help_system_report_constants,
                  inspect_host, inspect_pe, inspect_embedded_pe_ranges, pe_cursor_groups, pe_resource_entries,
                  setup_factory_static_extraction, native_ascii_pointer_table,
                  receiver_hardware_dispatch_proof, native_pseudocode_failures, lighting_catalog_projection,
                  native_exception_region_coverage, collect_keypad_mapping_static, keypad_mapping_constants,
                  collect_mapping_descriptors_static, mapping_descriptor_proof,
                  MAPPING_DESCRIPTOR_LIFECYCLE_FUNCTIONS, mapping_descriptor_lifecycle_proof,
                  MAPPING_DESCRIPTOR_PARSER_FUNCTIONS, mapping_descriptor_parser_proof,
                  collect_mapping_descriptor_gamemode,
                  collect_product_chroma_publisher, product_chroma_reducer,
                  collect_chroma_dashboard_launch, verify_chroma_dashboard_launch,
                  collect_lighting_product_input_map)


class ManagedInputSelectionTests(unittest.TestCase):
    def setUp(self):
        import analysis
        import host
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve()
        self.capture_path = self.root / ".analysis/results/host-current/managed/capture.json"
        self.tool = self.root / "tools/analyzers/fixture"
        self.tool.mkdir(parents=True)
        (self.tool / "acquisition.json").write_bytes(b"{}")
        self.enterContext(patch.object(analysis, "ROOT", self.root))
        self.enterContext(patch.object(host, "prepare_managed_analyzer", return_value=(self.tool, {}, [])))
        self.inspect = self.enterContext(patch.object(host, "inspect_pe", return_value={"clr": {}, "version_resources": []}))
        self.run = self.enterContext(patch.object(host.subprocess, "run", side_effect=self.decompile))
        self.enterContext(patch("builtins.print"))
        self.phase_exit = 0

    def source(self, name, raw=b"inert CLR fixture; never executed"):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    def index(self, rows):
        path = self.root / ".analysis/inputs.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"inputs": rows}), encoding="utf-8")

    def previous(self, rows, **extra):
        value = {"assemblies": rows, "pending_dependencies": [], "classification_failures": [],
                 "tool": {"acquisition_sha256": hashlib.sha256(b"{}").hexdigest()},
                 "full_managed_corpus_captured": True, **extra}
        self.capture_path.parent.mkdir(parents=True, exist_ok=True)
        self.capture_path.write_text(json.dumps(value), encoding="utf-8")

    def decompile(self, command, **kwargs):
        kwargs["stdout"].write(b"inert captured output")
        if "-o" in command:
            source = Path(command[command.index("-o") + 1]) / "Fixture.cs"
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("// Could not resolve fixture dependency\n", encoding="utf-8")
        return argparse.Namespace(returncode=self.phase_exit if "--ilcode" in command else 0)

    def analyze(self, names, **kwargs):
        import host
        host.analyze_host(argparse.Namespace(managed_input=names, managed_scope="vendor", workers=2, **kwargs))
        return json.loads(self.capture_path.read_bytes())

    def test_cli_accepts_repeated_exact_managed_inputs(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["analyze-host", "--managed-input", "first.dll", "--managed-input", "second.dll"])
        self.assertEqual(args.managed_input, ["first.dll", "second.dll"])

    def test_exact_mode_rejects_other_modes_before_analysis(self):
        for options in ({"javascript_only": True}, {"extract_asar_only": True},
                        {"resolve_managed_dependencies": True}, {"input": "archive.asar"}):
            with self.subTest(options=options), self.assertRaisesRegex(ValueError, "requires managed capture mode"):
                self.analyze(["exact.dll"], **options)
        self.run.assert_not_called()

    def test_incremental_capture_retains_unselected_failures_and_classifies_only_selection(self):
        selected = self.source(".analysis/inputs/host-current/current.dll")
        unselected = {"path": ".analysis/inputs/host-current/missing.dll", "sha256": "f" * 64, "bytes": 99}
        self.index([unselected, selected])
        old = {"input": unselected["path"], "input_sha256": unselected["sha256"], "capture_complete": False,
               "commands": [{"exit_code": 7}], "diagnostics": [{"text": "original failure"}]}
        failure = {"input": unselected["path"], "error": "retained classification failure"}
        self.previous([old], classification_failures=[failure], preserved_field={"evidence": "keep"})
        result = self.analyze([selected["path"]])
        self.inspect.assert_called_once_with((self.root / selected["path"]).read_bytes())
        self.assertEqual(self.run.call_count, 2)
        self.assertEqual(result["assemblies"][0], old)
        self.assertEqual(result["classification_failures"], [failure])
        self.assertEqual(result["preserved_field"], {"evidence": "keep"})
        self.assertEqual(result["selection"]["retained_assemblies"], 1)
        self.assertEqual(result["selection"]["assembly_sha256"], [selected["sha256"]])
        self.assertTrue(result["selected_capture_complete"])
        self.assertFalse(result["assemblies"][1]["all_csharp_resolved"])
        self.assertFalse(result["full_managed_corpus_captured"])

    def test_reuse_verifies_outputs_and_preserves_aliases_and_unresolved_diagnostics(self):
        selected = self.source(".analysis/inputs/host-current/current.dll")
        self.index([selected])
        artifact = self.source(".analysis/results/host-current/managed/fixture.cs", b"retained source")
        old = {"input": selected["path"], "input_sha256": selected["sha256"], "capture_complete": True,
               "all_csharp_resolved": False, "outputs": [artifact], "commands": [{"exit_code": 0}],
               "aliases": [".analysis/inputs/host-current/prior-alias.dll"],
               "diagnostics": [{"text": "unresolved prior dependency"}]}
        self.previous([old], pending_dependencies=[{"input": selected["path"], "sha256": selected["sha256"]},
                                                  {"input": "other", "sha256": "e" * 64}])
        result = self.analyze([selected["path"]])
        self.run.assert_not_called()
        self.assertEqual(len(result["assemblies"]), 1)
        reused = result["assemblies"][0]
        self.assertEqual(reused["diagnostics"], old["diagnostics"])
        self.assertEqual(reused["aliases"], [*old["aliases"], selected["path"]])
        self.assertFalse(reused["all_csharp_resolved"])
        self.assertEqual(result["pending_dependencies"], [{"input": "other", "sha256": "e" * 64}])
        (self.root / artifact["path"]).write_bytes(b"changed source")
        before = self.capture_path.read_bytes()
        with self.assertRaisesRegex(ValueError, "Retained managed output changed"):
            self.analyze([selected["path"]])
        self.assertEqual(self.capture_path.read_bytes(), before)
        self.run.assert_not_called()

    def test_failed_il_stage_is_retained_without_claiming_completion(self):
        selected = self.source(".analysis/inputs/host-current/current.dll")
        self.index([selected])
        self.phase_exit = 5
        result = self.analyze([selected["path"]])
        row = result["assemblies"][0]
        self.assertEqual([stage["exit_code"] for stage in row["commands"]], [0, 5])
        self.assertTrue(row["diagnostics"])
        self.assertFalse(row["capture_complete"])
        self.assertFalse(row["all_csharp_resolved"])
        self.assertFalse(result["selected_capture_complete"])
        self.assertFalse(result["full_managed_corpus_captured"])
        for output in row["outputs"]:
            raw = (self.root / output["path"]).read_bytes()
            self.assertEqual((len(raw), hashlib.sha256(raw).hexdigest()), (output["bytes"], output["sha256"]))

    def test_exact_input_rejects_unsafe_missing_duplicate_partial_and_changed_evidence(self):
        selected = self.source(".analysis/inputs/host-current/current.dll")
        for name, rows in (("../current.dll", [selected]),
                           (".analysis/inputs/host-current/../current.dll", [selected]),
                           (".analysis/inputs/host-current/unknown.dll", [selected]),
                           (selected["path"], [selected, selected]),
                           (selected["path"], [{**selected, "container_extraction_complete": False}]),
                           (selected["path"], [{**selected, "source_bytes_verified": False}]),
                           (selected["path"], [{**selected, "sha256": "0" * 64}])):
            self.index(rows)
            with self.subTest(name=name, rows=rows), self.assertRaises(ValueError):
                self.analyze([name])
        self.index([selected])
        with self.assertRaisesRegex(ValueError, "distinct indexed paths"):
            self.analyze([selected["path"], selected["path"]])
        self.run.assert_not_called()
        self.assertFalse(self.capture_path.exists())

    def test_exact_input_rejects_non_clr_and_pe_classification_failure(self):
        selected = self.source(".analysis/inputs/host-current/native.dll")
        self.index([selected])
        self.inspect.return_value = {"version_resources": []}
        with self.assertRaisesRegex(ValueError, "not a CLR assembly"):
            self.analyze([selected["path"]])
        self.inspect.side_effect = ValueError("bad PE")
        with self.assertRaisesRegex(ValueError, "cannot be classified"):
            self.analyze([selected["path"]])
        self.run.assert_not_called()
        self.assertFalse(self.capture_path.exists())


class InterhapticsServiceSourceTests(unittest.TestCase):
    def test_unicode_strings_keep_variables_and_code_unit_offsets(self):
        from host import nsis_unicode_string
        raw = "xx\0\x03\u80ab\\服务.exe\0".encode("utf-16le")
        self.assertEqual(nsis_unicode_string(raw, 3), [
            {"kind": "variable", "index": 43, "encoded": 0x80ab},
            {"kind": "literal", "value": "\\服务.exe"}])

    def test_unicode_escape_preserves_literal_control_and_surrogate_pair(self):
        from host import nsis_unicode_string
        self.assertEqual(nsis_unicode_string("\x04\x03😀\0".encode("utf-16le"), 0),
                         [{"kind": "literal", "value": "\x03😀"}])

    def test_unicode_rejects_invalid_bounds_and_truncated_tokens(self):
        from host import nsis_unicode_string
        for raw, offset in ((b"x", 0), (b"\0\0", True), (b"\0\0", -1),
                            (b"\0\0", 1), (b"\x03\0", 0), (b"\x03\0\0\0", 0),
                            ("unterminated".encode("utf-16le"), 0), (b"\0\xd8\0\0", 0)):
            with self.subTest(raw=raw, offset=offset), self.assertRaises(ValueError):
                nsis_unicode_string(raw, offset)

    def fixture(self, export="StopService"):
        words, offsets = bytearray(), {}
        for name, value in (("module", "\x03\u809a\\SimpleSC.dll"), ("export", export),
                            ("service", "\x03\u80ab"), ("flag", "1"), ("timeout", "30")):
            offsets[name] = len(words) // 2
            words.extend((value + "\0").encode("utf-16le"))
        push = lambda value: [31, offsets[value], 0, 0, 0, 0, 0]
        entries = [push("timeout"), push("flag"), push("service"),
                   [44, offsets["module"], offsets["export"], 0, 0, 1, -37],
                   [31, 0, 1, 0, 0, 0, 0]]
        if export == "GetServiceStatus":
            entries.append([31, 1, 1, 0, 0, 0, 0])
        return {"entries": entries}, bytes(words)

    def test_stop_uses_reverse_stack_order_and_one_result(self):
        from host import interhaptics_service_calls
        header, strings = self.fixture()
        result, = interhaptics_service_calls(header, strings)
        self.assertEqual(result["export"], "StopService")
        self.assertEqual([row["entry"] for row in result["arguments_in_pop_order"]], [2, 1, 0])
        self.assertEqual(result["arguments_in_pop_order"][0]["tokens"][0]["index"], 43)
        self.assertEqual(result["arguments_in_pop_order"][2]["tokens"], [{"kind": "literal", "value": "30"}])
        self.assertEqual([row["variable_index"] for row in result["result_consumers_in_pop_order"]], [0])

    def test_status_retains_error_then_state_consumers_and_only_one_input(self):
        from host import interhaptics_service_calls
        header, strings = self.fixture("GetServiceStatus")
        result, = interhaptics_service_calls(header, strings)
        self.assertEqual(len(result["arguments_in_pop_order"]), 1)
        self.assertEqual([row["variable_index"] for row in result["result_consumers_in_pop_order"]], [0, 1])
        header["entries"].pop()
        with self.assertRaisesRegex(ValueError, "result stack"):
            interhaptics_service_calls(header, strings)

    def test_unknown_export_and_noncontiguous_arguments_fail(self):
        from host import interhaptics_service_calls
        header, strings = self.fixture("InstallService")
        with self.assertRaisesRegex(ValueError, "Unreviewed"):
            interhaptics_service_calls(header, strings)
        header, strings = self.fixture()
        header["entries"][1][0] = 7
        with self.assertRaisesRegex(ValueError, "contiguous"):
            interhaptics_service_calls(header, strings)

    def test_incremental_queries_dispatch_without_starting_the_full_host_audit(self):
        import host
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        for query, name in (("interhaptics-services", "audit_interhaptics_services"),
                            ("hdk-refresh", "audit_hdk_refresh")):
            with self.subTest(query=query), patch.object(host, name) as collect, \
                    patch.object(host, "HostLocalAudit") as full:
                args = parser.parse_args(["audit-host-local", "--query", query])
                audit_host_local(args)
                collect.assert_called_once_with(args)
                full.assert_not_called()


class InterhapticsFilePathTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        audit = HostLocalAudit()
        inspection = audit.a.ROOT / (".analysis/results/native-packages/"
            "33954697ea3c99d8a7e1ffd99255834c29a01e4d0712b9e8fbc83f5db346d83f/inspection.json")
        original, = [row for row in json.loads(inspection.read_text(encoding="utf-8"))["files"]
                     if Path(row["path"]).name == "SimpleSC.dll"]
        cls.retained = audit.native_slice(original, (), depth=0,
            rvas=(0x203e4, 0x2ac84, 0xa8a0, 0xa9f8, 0xa730, 0xe8f38, 0x2034c, 0xe9028),
            regions=({"role": "binary_path_directory_separators", "rva": 0x20494, "bytes": 6},
                     {"role": "binary_path_argument_separator", "rva": 0xe9024, "bytes": 4}))
        if cls.retained["failures"]:
            raise AssertionError(cls.retained["failures"])

    def test_reviewed_assembly_proof_preserves_warning_and_false_gate(self):
        from host import interhaptics_file_path_proof
        proof = interhaptics_file_path_proof(self.retained)
        self.assertEqual(len(proof["reviewed_owners"]), 8)
        self.assertTrue(proof["static_path_semantics_complete"])
        self.assertFalse(proof["official_execution_verified"])
        owner, = [row["capture"] for row in self.retained["complete_functions"]
                  if row["capture"]["rva"] == "0x203e4"]
        self.assertTrue(any("bad sp value" in row["text"] for row in owner["pseudocode"]["lines"]))
        self.assertIn("without attempting", proof["existence_gate"]["false_result"])

    def test_unreviewed_bytes_missing_owner_and_changed_separator_fail(self):
        from host import interhaptics_file_path_proof
        for change in ("bytes", "owner", "separator"):
            retained = copy.deepcopy(self.retained)
            if change == "bytes":
                retained["complete_functions"][0]["capture"]["chunks"][0]["raw_hex"] += "90"
            elif change == "owner":
                retained["complete_functions"].pop()
            else:
                retained["verified_data_regions"][0]["raw_hex"] = "2f003a000000"
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "InterHaptics reviewed path"):
                interhaptics_file_path_proof(retained)

    def test_assembly_cases_keep_quotes_environment_and_late_argument_separator(self):
        from host import interhaptics_file_path_projection
        # Expected candidates follow 203e4/e8f38 and Copy's index-zero clamp;
        # these are static string cases, not official DLL execution results.
        cases = (
            (r'C:\Program Files\svc.exe --quiet', r'C:\Program Files\svc.exe'),
            (r'"C:\Program Files\svc.exe" --quiet', r'"C:\Program Files\svc.exe"'),
            (r'C:\dir\service name.exe --quiet', r'C:\dir\service'),
            (r'%SystemRoot%\System32\svc.exe --quiet', r'%SystemRoot%\System32\svc.exe'),
            (r'\\server\share\svc.exe --quiet', r'\\server\share\svc.exe'),
            (r'C:\svc.exe --log D:\logs\file name.txt', r'C:\svc.exe --log D:\logs\file'),
            ('C:/Program Files/svc.exe --quiet', 'C:/Program'),
            ('svc.exe\t--quiet', 'svc.exe\t--quiet'),
            ('svc.exe --quiet', 'svc.exe'),
            (' --quiet', ''),
            ('', ''),
        )
        for binary_path, expected in cases:
            with self.subTest(binary_path=binary_path):
                self.assertEqual(interhaptics_file_path_projection(binary_path), expected)
        with self.assertRaises(ValueError):
            interhaptics_file_path_projection(None)


class HostCurrentSourceRouteTests(unittest.TestCase):
    def setUp(self):
        import analysis
        self.audit = HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)

    def test_reviewed_routes_share_selector_and_preserve_missing_capture_and_failures(self):
        import frontend
        for name, route, snapshot in (
            ("synapse-lighting-engine-current", "https://apps.razer.com/synapse/lighting-engine/", "synapse-lighting-engine-snapshot-new"),
            ("gamepad-mw-4115-current", "https://apps.razer.com/synapse/products/4115/mw/", "gamepad-mw-4115-snapshot-new-closure"),
            ("gamepad-4115-current", "https://apps.razer.com/synapse/products/4115/ui/", "gamepad-4115-current-refresh-new"),
            ("mouse-226-current", "https://apps.razer.com/synapse/products/226/ui/", "mouse-226-snapshot-new"),
            ("keyboard-756-current", "https://apps.razer.com/synapse/products/756/ui/", "keyboard-756-current-refresh-new"),
            ("mouse-162-current", "https://apps.razer.com/synapse/products/162/ui/", "mouse-162-snapshot-new"),
        ):
            with self.subTest(route=route):
                receipt = self.root / ".analysis/inputs/frontend" / snapshot / "acquisition.json"
                receipt.parent.mkdir(parents=True, exist_ok=True)
                receipt.write_text("{}", encoding="utf-8")
                selected = {"name": snapshot, "canonical_entry_url": route, "acquisition": {"path": receipt.as_posix()},
                    "deployment": {"failures": []}, "capture": None,
                    "capture_failures": ["complete_capture_unavailable"], "unresolved_acquisition": ["map"],
                    "failed_acquisition": [{"http_status": 404}], "originals": [{"path": "main.js"}],
                    "candidate_decisions": [{"name": name, "disposition": "superseded"}]}
                with patch.object(frontend, "current_source_selection", return_value=selected) as select:
                    result, originals = self.audit.current_source_set(name)
                self.assertEqual(select.call_args.args, (route, self.audit))
                self.assertIn(receipt, select.call_args.kwargs["candidates"])
                self.assertEqual(originals, selected["originals"])
                self.assertEqual(result["failed_acquisition"], selected["failed_acquisition"])
                self.assertEqual(result["capture_failures"], ["complete_capture_unavailable"])
                self.assertIsNone(result["capture"])
                self.assertEqual(result["current_source_selection"]["candidate_decisions"], selected["candidate_decisions"])

    def test_selector_failure_cannot_fall_back_to_the_legacy_directory(self):
        with patch("frontend.current_source_selection", side_effect=ValueError("newest bytes differ")), \
                patch.object(self.audit, "source_set") as legacy:
            with self.assertRaisesRegex(ValueError, "newest bytes differ"):
                self.audit.current_source_set("synapse-lighting-engine-current")
        legacy.assert_not_called()

    def test_corrupt_newest_snapshot_bytes_cannot_select_the_valid_legacy_source(self):
        entry = "https://apps.razer.com/synapse/lighting-engine/"
        for name, date in (("synapse-lighting-engine-current", "2026-10-01T00:00:00Z"),
                           ("synapse-lighting-engine-snapshot-new", "2026-10-30T00:00:00Z")):
            folder = self.root / ".analysis/inputs/frontend" / name
            folder.mkdir(parents=True)
            rows = []
            for filename, raw in (("index.html", b'<script src="main.js"></script>'), ("main.js", b"run();")):
                path = folder / filename
                path.write_bytes(raw)
                rows.append({"path": path.relative_to(self.root).as_posix(), "url": entry + filename,
                    "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
            rows[0]["url"] = entry
            acquired = {"entry_url": entry, "recorded_at_utc": date, "files": rows, "edges": [],
                "source_current_during_acquisition": True, "target_execution": False,
                "stability": [{"url": entry, "unchanged": True,
                    "before_sha256": rows[0]["sha256"], "after_sha256": rows[0]["sha256"]}]}
            (folder / "acquisition.json").write_text(json.dumps(acquired), encoding="utf-8")
        (folder / "main.js").write_bytes(b"changed();")
        with self.assertRaisesRegex(ValueError, "Newest frontend deployment failed"):
            self.audit.current_source_set("synapse-lighting-engine-current")

    def test_unreviewed_route_cannot_borrow_a_known_product(self):
        with self.assertRaisesRegex(ValueError, "no reviewed route"):
            self.audit.current_source_set("gamepad-mw-4133-current")


class ProductChromaPublisherTests(unittest.TestCase):
    # Reduced webpack owners are parsed only; none of this JavaScript executes.
    SOURCE = '''const modules={
    44:(m,x,load)=>{load.d(x,{Key:()=>key});const key="lighting-engine-chroma-devices"},
    55:(m,x,load)=>{load.d(x,{Api:()=>api});
        class Bridge{constructor(){this.getMemoryStorageItem=e=>window.getMemoryStorageItem(e);
        this.setMemoryStorageItem=(k,v)=>window.setMemoryStorageItem(k,v)}}const api=new Bridge},
    66:(m,x,load)=>{load.d(x,{Common:()=>common});const common=new class{
        generateSetChromaEffectCommonData(e){return e}async setChromaEffectCommon(e){await e.rzDevice.setChromaEffectCommon(e)}}},
    77:(m,x,load)=>{const keys=load(44),memory=load(55),effects=load(66);
        class Manager{constructor(){
            this.updateChromaDevice=async e=>{await this.updateChromaStudio(e)};
            this.handleRefreshDevices=async e=>{await this.updateChromaStudio(e)};
            this.destroyDeviceLightingEngine=async e=>{await e.destroy()};
            this.applyQuickEffect=async(e={})=>{setTimeout(async()=>{await effects.Common.setChromaEffectCommon(e)},50)};
            this.applyRegionQuickEffect=async e=>{await this.applyQuickEffect(e)};
            this.syncHWEffect=async e=>{await this.applyQuickEffect(e)};
            this.applyCustomQuickEffect=async e=>{await e.render()};
            this.onStorageChange_device=async e=>{await this.applyRegionQuickEffect(e)};
            this.onExit=async e=>{await this.applyQuickEffect(e)}
        }
        async updateChromaStudio({productId,instanceId,action}){
            const storageKey=keys.Key,payload=this.generateUpdateChromaStudioPayload({productId,instanceId,action});
            if(void 0!==payload){try{await memory.Api.setMemoryStorageItem(storageKey,JSON.stringify(payload))}catch(error){console.warn(error)}}
        }
        generateUpdateChromaStudioPayload({action}){const devices=[];
            if("add"===action&&0===devices.length)return;return{action,payload:{devices}}}
        async addChromaDevice(e,t){await this.updateChromaStudio(e)}
        async removeDeviceChromaEngine(e){await this.destroyDeviceLightingEngine(e);await this.updateChromaStudio(e)}
        }
        class Positions{constructor(){this.getDevicePositions=async e=>this.manager.generateUpdateChromaStudioPayload(e)}}
    }};'''

    def setUp(self):
        import analysis
        self.audit = HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)

    def source(self, body):
        raw = body.encode("utf-8")
        path = self.root / ".analysis/inputs/frontend/chroma-fixture/main.js"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}

    def collect(self, body=None):
        return collect_product_chroma_publisher(self.audit, [self.source(self.SOURCE if body is None else body)])

    def test_destructured_publisher_reaches_actual_key_bridge_and_quick_writer(self):
        report = self.collect()
        self.assertEqual([(row["module_id"], row["export"]) for row in report["exports"]],
                         [(44, "Key"), (55, "Api"), (66, "Common")])
        self.assertEqual(report["memory_key"], "lighting-engine-chroma-devices")
        for role, body in (
            ("engine_actual_host_memory_publisher_bridge", 'this.setMemoryStorageItem=(k,v)=>window.setMemoryStorageItem(k,v)'),
            ("engine_quick_common_effect_writer", 'async setChromaEffectCommon(e){await e.rzDevice.setChromaEffectCommon(e)}'),
            ("engine_remove_caller", 'async removeDeviceChromaEngine(e){await this.destroyDeviceLightingEngine(e);await this.updateChromaStudio(e)}'),
        ):
            owner, = report["selected"][role]
            self.assertEqual(self.audit.owners[owner]["body"], body)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_aliases_exports_and_module_ids_use_the_actual_lexical_import(self):
        source = self.SOURCE
        for old, new in (("keys", "constants"), ("memory", "hostBridge"), ("effects", "deviceApi"),
                         ("load", "requireModule"), ("Key", "ObservedKey"), ("Api", "Host"),
                         ("44", "91"), ("55", "92"), ("66", "93")):
            source = re.sub(r"\b" + re.escape(old) + r"\b", new, source)
        report = self.collect(source)
        self.assertEqual([(row["module_id"], row["export"]) for row in report["exports"]],
                         [(91, "ObservedKey"), (92, "Host"), (93, "Common")])

    def test_correct_key_in_an_unrelated_factory_cannot_repair_the_imported_wrong_key(self):
        body = self.SOURCE.replace('const modules={', 'const modules={99:(m,x,n)=>{n.d(x,{Key:()=>key});const key="lighting-engine-chroma-devices"},')
        body = body.replace('44:(m,x,load)=>{load.d(x,{Key:()=>key});const key="lighting-engine-chroma-devices"}',
                            '44:(m,x,load)=>{load.d(x,{Key:()=>key});const key="unrelated-storage"}')
        with self.assertRaisesRegex(ValueError, "memory key changed"):
            self.collect(body)

    def test_dynamic_missing_or_shadowed_require_cannot_borrow_another_module(self):
        for body in (self.SOURCE.replace("keys=load(44)", "keys=load(runtimeId)"),
                     self.SOURCE.replace("keys=load(44)", "keys=load(404)"),
                     self.SOURCE.replace("keys=load(44)", "keys=((load)=>load(44))(other)")):
            with self.subTest(body=body[:90]), self.assertRaises(ValueError):
                # A fresh store keeps each failure tied to its own source bytes.
                self.audit = HostLocalAudit()
                self.collect(body)

    def test_unawaited_or_ambiguous_memory_write_is_not_the_reviewed_publisher(self):
        store = "await memory.Api.setMemoryStorageItem(storageKey,JSON.stringify(payload))"
        for replacement in (store.removeprefix("await "), store + ";" + store):
            with self.subTest(replacement=replacement), self.assertRaisesRegex(ValueError, "unique awaited memory write"):
                self.audit = HostLocalAudit()
                self.collect(self.SOURCE.replace(store, replacement))

    def test_unrelated_same_named_method_cannot_replace_the_exported_api_member(self):
        body = self.SOURCE.replace("this.getMemoryStorageItem=e=>window.getMemoryStorageItem(e)",
                                   "this.unrelated=e=>window.getMemoryStorageItem(e)")
        body += "class Decoy{getMemoryStorageItem(e){return e}}"
        with self.assertRaisesRegex(ValueError, "engine_actual_host_memory_bridge"):
            self.collect(body)

    def test_query_preserves_newest_source_without_borrowing_legacy_capture(self):
        source_set = {"name": "synapse-lighting-engine-snapshot-latest", "capture": None,
                      "capture_failures": ["complete_capture_unavailable"]}
        with patch("host.HostLocalAudit", return_value=self.audit), \
                patch.object(self.audit, "artifact", return_value=b""), \
                patch.object(self.audit, "current_source_set", side_effect=[(source_set, []), ValueError("next independent product unavailable")]) as current, \
                patch.object(self.audit, "source_set") as fixed, \
                patch.object(self.audit, "frontend_capture") as capture, \
                patch("host.publish_host_static_query") as publish, patch("builtins.print"):
            with self.assertRaisesRegex(ValueError, "query remains unresolved"):
                audit_product_chroma_observation(argparse.Namespace(query="product-chroma-observation"))
        self.assertEqual([call.args for call in current.call_args_list],
                         [("synapse-lighting-engine-current",), ("mouse-226-current",)])
        fixed.assert_not_called()
        capture.assert_not_called()
        report = publish.call_args.args[0]
        self.assertIsNone(report["source_sets"][0]["verified_complete_capture"])
        self.assertEqual(report["source_sets"][0]["capture_failures"], ["complete_capture_unavailable"])
        self.assertEqual(report["failures"], ["next independent product unavailable"])
        self.assertFalse(report["source_chain_complete"])

    def test_reducer_initial_reference_ignores_same_named_shadowed_state(self):
        reducer = 'reducer=function(state,action){if(void 0===state)return initial;switch(action.type){default:return state}}'
        source = self.source('const initial={selectedEffectId:3,effectSettings:[]};const ' + reducer + ';'
            'function unrelated(){const initial={};const decoy=function(state,action){if(void 0===state)return initial;switch(action.type){default:return state}}}')
        initial, = self.audit.retain("initial", source, "{selectedEffectId:3,effectSettings:[]", {"variable_declarator"})
        owner = product_chroma_reducer(self.audit, source, initial, "profiles_reducer")
        self.assertEqual(self.audit.owners[owner]["body"], reducer)

    def test_multiple_real_reducers_for_one_initial_state_require_further_analysis(self):
        source = self.source('const initial={selectedEffectId:3,effectSettings:[]};'
            'const first=function(s,a){if(void 0===s)return initial;switch(a.type){default:return s}},'
            'second=function(s,a){if(void 0===s)return initial;switch(a.type){default:return s}};')
        initial, = self.audit.retain("initial", source, "{selectedEffectId:3,effectSettings:[]", {"variable_declarator"})
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            product_chroma_reducer(self.audit, source, initial, "profiles_reducer")


class HelpRuntimeVersionsTests(unittest.TestCase):
    # Reduced static owners from the current 3594 Help component and helpers.
    # The fixture is parsed as text; no vendor JavaScript is executed.
    SOURCE = r'''
const normalize=e=>(null!==e&&void 0!==e?e:"").replace(/[{}]/g,"").toLowerCase(),
select=(e,{containerId:E,productIds:_})=>{var a;if(!Array.isArray(e))return;
const t=normalize(E),i=_.filter(e=>void 0!==e&&null!==e).map(String),
o=t&&e.find(e=>(e=>{try{return normalize(new URL(e).searchParams.get("containerId"))}
catch(E){return""}})(null===e||void 0===e?void 0:e.windowName)===t),
n=e.find(e=>i.includes((e=>{try{const E=new URL(e).pathname.split("/").filter(Boolean),
_=E.findIndex(e=>"products"===e.toLowerCase()),a=_>=0?E[_+1]:"";
return/^\d+$/.test(a)?a:""}catch(E){return""}})
(null===e||void 0===e?void 0:e.windowName)));
return null===(a=o||n)||void 0===a?void 0:a.value},key="mwVersion";
class Help {
constructor(e){this.state={serialNumber:e.serialNumber,uiVersion:void 0,mwVersion:void 0,synapseVersion:void 0}}
componentDidMount(){this.getDeviceUIVersionFromUIElement(),this.getDeviceMWVersionFromWindowsStorage(),this.getAppVersion()}
componentDidUpdate(e,E){}
getDeviceMWVersionFromWindowsStorage(){var e,E;null===(e=api.getWindowStorageItem(key))||void 0===e||null===(E=e.then(e=>{if(!e)return;const E=select(JSON.parse(e),{containerId:new URLSearchParams(window.location.search).get("containerId"),productIds:[device.DeviceInfo.productId,device.DeviceInfo.dongleId]});E&&this.setState({mwVersion:E})}))||void 0===E||E.catch(e=>{console.error("Failed to read mwVersion from window storage:",e)})}
getAppVersion(){try{const e=localStorage.getItem("apps");if(e){const E=JSON.parse(e).find(e=>"synapse"===e.name);if(E&&E.version){let e="4.0.0";e=["4",...E.version.split(".").slice(1)].join("."),console.log("Found synapse version:",e),this.setState({synapseVersion:e})}}}catch(e){console.error("Error reading version from local storage:",e)}}
getDeviceUIVersionFromUIElement(){const e=document.querySelector("noscript#version");if(console.log("device UI buildInfo:",e),e)try{const E=e.innerHTML;if(console.log("device UI html:",E),E.indexOf("VERSION_PLACEHOLDER")>=0)return;const _=JSON.parse(E);console.log("device UI:",_);let a=_.version,t=_.buildVersion;console.log("device UI version:",a),console.log("device UI buildVersion:",t),this.setState({uiVersion:`${a}.${t}`})}catch(E){console.error("Failed to parse device ui version data from noscript:",E)}}
render(){return null}
}
const Connected=connect(state,actions)(Help);
'''

    def setUp(self):
        import analysis
        self.audit = HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)

    def source(self, body, name="main.js"):
        raw = body.encode("utf-8")
        path = self.root / ".analysis/inputs/frontend/help-fixture" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}

    def analyze(self, transform=lambda value: value, mount=True):
        from host import help_runtime_components
        body = self.SOURCE + ('const root=(0,react.jsx)(Connected,{hasCamoStudio:!0,resetObm:this.resetDevice});' if mount else '')
        return help_runtime_components(self.audit, [self.source(transform(body))])

    def test_complete_getters_scoped_helpers_and_actual_props_are_retained(self):
        result, = self.analyze()
        self.assertEqual(result["middleware"]["mode"], "scoped")
        self.assertEqual(result["middleware"]["storage_key"], "mwVersion")
        self.assertTrue(result["retains_serial"])
        self.assertEqual(result["mounts"][0]["props"], {"hasCamoStudio": "!0", "resetObm": "this.resetDevice"})
        self.assertEqual(set(result["methods"]), {"constructor", "componentDidMount", "componentDidUpdate",
            "getDeviceUIVersionFromUIElement", "getDeviceMWVersionFromWindowsStorage", "getAppVersion", "render"})
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_alias_names_can_change_without_borrowing_other_lexical_bindings(self):
        result, = self.analyze(lambda value: value.replace("normalize", "canonicalContainer")
            .replace("select(", "pickVersion(").replace("select=", "pickVersion=")
            .replace("Connected", "ActualHelp").replace("Help {", "CurrentHelp {").replace("(Help)", "(CurrentHelp)"))
        self.assertEqual(result["middleware"]["product_fields"], ["productId", "dongleId"])

    def test_unmounted_class_cannot_prove_a_runtime_consumer(self):
        with self.assertRaisesRegex(ValueError, "no statically resolved mounted JSX"):
            self.analyze(mount=False)

    def test_shadowed_same_name_mount_cannot_be_borrowed(self):
        with self.assertRaisesRegex(ValueError, "no statically resolved mounted JSX"):
            self.analyze(lambda value: value.replace("const root=", "function unrelated(Connected){const root=") + "}")

    def test_cross_chunk_export_and_namespace_import_reach_actual_mount(self):
        from host import help_runtime_components
        component = self.source('const modules={17:(e,t,n)=>{n.d(t,{A:()=>Connected});' + self.SOURCE + '}};', "help.chunk.js")
        caller = self.source('const modules={99:(e,t,n)=>{const help=n(17);'
            'const root=(0,react.jsx)(help.A,{resetObm:this.resetDevice});}};', "main.js")
        result, = help_runtime_components(self.audit, [caller, component])
        self.assertEqual(result["mounts"][0]["original"]["path"], caller["path"])
        self.assertEqual(result["mounts"][0]["props"], {"resetObm": "this.resetDevice"})
        self.assertEqual(len(result["binding_links"]), 2)

    def lazy_sources(self, change_caller=lambda body: body, change_wrapper=lambda body: body):
        component = self.source('self.chunks.push([[50],{17:(e,t,n)=>{n.d(t,{default:()=>Connected});' +
                                self.SOURCE + '}}]);', "help.chunk.js")
        wrapper = ('const modules={31:(e,t,n)=>{n.d(t,{kg:()=>E});'
                   'const E=({lazy:e,LoadingComponent:t,ErrorComponent:n,...o})=>{'
                   'const[r,E]=(0,i.useState)(null),[_,s]=(0,i.useState)(null);'
                   'return(0,i.useEffect)(()=>((async()=>{try{const t=await e();E(()=>t.default)}'
                   'catch(t){console.error("Asynchronous component loading failed",t),'
                   's(t instanceof Error?t:new Error("AsyncComponent loading failed."))}})(),'
                   '()=>{E(null),s(null)}),[e]),_?n||null:r?(0,a.jsx)(r,{...o}):t||null}}};')
        caller = ('const modules={99:(e,t,n)=>{const loader=n(31);'
                  'const root=(0,react.jsx)(loader.kg,{lazy:()=>Promise.all([n.e(7),n.e(50)]).then(n.bind(n,17)),resetObm:this.resetDevice});}};')
        return [self.source(change_wrapper(wrapper), "wrapper.js"),
                self.source(change_caller(caller), "main.js"), component]

    def test_lazy_component_follows_actual_chunk_default_export_and_forwarded_props(self):
        from host import help_runtime_components
        row, = help_runtime_components(self.audit, self.lazy_sources())
        mounted, = row["mounts"]
        self.assertEqual(mounted["props"], {"resetObm": "this.resetDevice"})
        self.assertEqual(mounted["lazy_import"]["chunk_ids"], [7, 50])
        self.assertEqual(mounted["lazy_import"]["registered_chunk_ids"], [50])
        self.assertFalse(mounted["lazy_import"]["cleanup_cancels_import"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_lazy_renderer_body_destructure_preserves_actual_props_and_cleanup(self):
        from host import help_runtime_components
        old = 'E=({lazy:e,LoadingComponent:t,ErrorComponent:n,...o})=>{'
        new = 'E=props=>{let{lazy:e,LoadingComponent:t,ErrorComponent:n,...o}=props;'
        row, = help_runtime_components(self.audit, self.lazy_sources(
            change_wrapper=lambda body: body.replace(old, new)))
        mounted, = row["mounts"]
        self.assertEqual(mounted["props"], {"resetObm": "this.resetDevice"})
        self.assertEqual(mounted["lazy_import"]["chunk_ids"], [7, 50])
        self.assertFalse(mounted["lazy_import"]["cleanup_cancels_import"])
        wrapper = self.audit.owners[mounted["lazy_import"]["wrapper"]]["body"]
        self.assertIn('...o}=props;', wrapper)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_lazy_body_destructure_cannot_borrow_props_drop_errors_or_add_cancellation(self):
        from host import help_runtime_components
        old = 'E=({lazy:e,LoadingComponent:t,ErrorComponent:n,...o})=>{'
        new = 'E=props=>{let{lazy:e,LoadingComponent:t,ErrorComponent:n,...o}=props;'
        for before, after in (('...o}=props;', '...o}=unrelated;'),
                ('t.default', 't.other'), ('(r,{...o})', '(r,{})'),
                ('s(t instanceof Error?t:new Error("AsyncComponent loading failed."))', 's(null)'),
                ('()=>{E(null),s(null)}', '()=>{cancel(),E(null),s(null)}')):
            with self.subTest(change=after), self.assertRaisesRegex(ValueError, "complete default-export"):
                help_runtime_components(self.audit, self.lazy_sources(change_wrapper=lambda body:
                    body.replace(old, new).replace(before, after)))
            self.audit = HostLocalAudit()

    def test_lazy_target_requires_its_published_chunk_and_actual_require(self):
        from host import help_runtime_components
        for old, new, message in (
                ('n.e(50)', 'n.e(51)', "registered chunk"),
                ('n.bind(n,17)', 'n.bind(other,17)', "different require"),
                ('n.e(50)', 'other.e(50)', "chunk request"),
                ('n.e(7),n.e(50)', 'n.e(50),n.e(50)', "duplicate chunk"),
                ('lazy:()=>', 'loader:()=>', "no statically resolved mounted JSX")):
            with self.subTest(replacement=new), self.assertRaisesRegex(ValueError, message):
                help_runtime_components(self.audit, self.lazy_sources(lambda body: body.replace(old, new)))
            self.audit = HostLocalAudit()

    def test_lazy_renderer_cannot_drop_props_change_export_or_fabricate_cancellation(self):
        from host import help_runtime_components
        for old, new in (('t.default', 't.other'), ('(r,{...o})', '(r,{})'),
                         ('()=>{E(null),s(null)}', '()=>{cancel(),E(null),s(null)}')):
            with self.subTest(replacement=new), self.assertRaisesRegex(ValueError, "complete default-export"):
                help_runtime_components(self.audit, self.lazy_sources(change_wrapper=lambda body: body.replace(old, new)))
            self.audit = HostLocalAudit()

    def test_export_must_target_actual_factory_exports(self):
        from host import help_runtime_components
        component = self.source('const modules={17:(e,t,n)=>{n.d(other,{A:()=>Connected});' + self.SOURCE + '}};', "help.chunk.js")
        caller = self.source('const modules={99:(e,t,n)=>{const help=n(17);'
            'const root=(0,react.jsx)(help.A,{});}};', "main.js")
        with self.assertRaisesRegex(ValueError, "no statically resolved mounted JSX"):
            help_runtime_components(self.audit, [caller, component])

    def test_shadowed_require_cannot_claim_cross_chunk_mount(self):
        from host import help_runtime_components
        component = self.source('const modules={17:(e,t,n)=>{n.d(t,{A:()=>Connected});' + self.SOURCE + '}};', "help.chunk.js")
        caller = self.source('const modules={99:(e,t,n)=>{function unrelated(n){const help=n(17);'
            'const root=(0,react.jsx)(help.A,{});}}};', "main.js")
        with self.assertRaisesRegex(ValueError, "no statically resolved mounted JSX"):
            help_runtime_components(self.audit, [caller, component])

    def test_storage_key_follows_real_exported_object(self):
        from host import help_runtime_components
        component = self.source('const modules={17:(e,t,n)=>{const storage=n(23);' +
            self.SOURCE.replace("api.getWindowStorageItem(key)", "api.getWindowStorageItem(storage.K.MW_VERSION)") +
            '(0,react.jsx)(Connected,{});}};', "help.chunk.js")
        constants = self.source('const modules={23:(e,t,n)=>{n.d(t,{K:()=>keys});'
            'const keys={MW_VERSION:"mwVersion"};}};', "constants.chunk.js")
        result, = help_runtime_components(self.audit, [component, constants])
        self.assertEqual(result["middleware"]["storage_key_source"]["path"], constants["path"])

    def test_storage_key_can_follow_the_actual_bootstrap_require(self):
        from host import help_runtime_components
        prefix = ('const modules={23:(e,t,n)=>{n.d(t,{K:()=>keys});const keys={MW_VERSION:"mwVersion"};}},cache={};'
                  'function require(id){var found=cache[id];if(void 0!==found)return found.exports;'
                  'var entry=cache[id]={id:id,loaded:!1,exports:{}};'
                  'return modules[id](entry,entry.exports,require),entry.loaded=!0,entry.exports}'
                  'const storage=require(23);')
        source = self.source(prefix + self.SOURCE.replace(
            "api.getWindowStorageItem(key)", "api.getWindowStorageItem(storage.K.MW_VERSION)") +
            '(0,react.jsx)(Connected,{});')
        row, = help_runtime_components(self.audit, [source])
        self.assertEqual(row["middleware"]["storage_key"], "mwVersion")
        self.assertTrue(any("source_bootstrap_require" in owner["roles"] for owner in self.audit.owners.values()))

    def test_direct_component_alias_preserves_its_lexical_mount(self):
        row, = self.analyze(lambda value: value.replace("const root=", "const Alias=Connected;const root=")
                           .replace("(Connected,{hasCamoStudio", "(Alias,{hasCamoStudio"))
        self.assertEqual(row["mounts"][0]["props"]["hasCamoStudio"], "!0")

    def test_changed_selector_output_or_order_is_rejected(self):
        for before, after in (("a=o||n", "a=n||o"), ("void 0:a.value", "void 0:a.version"),
                              ('get("containerId")', 'get("other")'), ("DeviceInfo.dongleId", "DeviceInfo.productId")):
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.analyze(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = HostLocalAudit()

    def test_missing_normalizer_or_changed_storage_key_is_rejected(self):
        for before, after in (("const normalize=", "const missing="), ('key="mwVersion"', 'key="version"')):
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.analyze(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = HostLocalAudit()

    def test_current_direct_promise_variant_preserves_scoped_selection(self):
        result, = self.analyze(lambda value: value.replace("var e,E;null===(e=", "var e;null===(e=")
            .replace("||null===(E=e.then(", "||e.then(").replace("}))||void 0===E||E.catch", "}).catch"))
        self.assertEqual(result["middleware"]["mode"], "scoped")
        self.assertEqual(result["middleware"]["promise_variant"], "direct_then_result")

    def test_scoped_selector_with_lowered_parameter_destructuring_is_complete(self):
        result, = self.analyze(lambda value: value.replace(
            "select=(e,{containerId:E,productIds:_})=>{var a;",
            "select=(e,options)=>{var a;let{containerId:E,productIds:_}=options;"))
        self.assertEqual(result["middleware"]["mode"], "scoped")

    def test_current_first_record_variant_is_not_misclassified_as_scoped(self):
        first = ('getDeviceMWVersionFromWindowsStorage(){api.getWindowStorageItem(key).then(e=>{'
                 'var t;const s=JSON.parse(e);0!==s.length&&this.setState('
                 '{mwVersion:null===(t=s[0])||void 0===t?void 0:t.value})})}')
        def replace(value):
            start = value.index("getDeviceMWVersionFromWindowsStorage(){")
            end = value.index("\ngetAppVersion(){", start)
            return value[:start] + first + value[end:]
        result, = self.analyze(replace)
        self.assertEqual(result["middleware"]["mode"], "first")
        self.assertFalse(result["middleware"]["promise_failure_handler"])

    def test_getter_content_globals_and_missing_value_paths_are_checked(self):
        for before, after in (('.slice(1)', '.slice(0)'), ('localStorage.getItem', 'otherStorage.getItem'),
                              ('indexOf("VERSION_PLACEHOLDER")>=0', 'indexOf("VERSION_PLACEHOLDER")>0'),
                              ("E&&this.setState({mwVersion:E})", "this.setState({mwVersion:E})"),
                              ("synapseVersion:void 0", 'synapseVersion:"4.0.0"')):
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.analyze(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = HostLocalAudit()

    def test_getters_must_run_directly_in_mount_order(self):
        for before, after in (("this.getAppVersion()", "()=>this.getAppVersion()"),
                              ("this.getDeviceUIVersionFromUIElement(),this.getDeviceMWVersionFromWindowsStorage()",
                               "this.getDeviceMWVersionFromWindowsStorage(),this.getDeviceUIVersionFromUIElement()")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "mount order"):
                self.analyze(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = HostLocalAudit()

    def test_dynamic_props_and_nonprop_serial_initializer_are_rejected(self):
        for before, after in (("{hasCamoStudio:!0,", "{...other,hasCamoStudio:!0,"),
                              ("serialNumber:e.serialNumber", "serialNumber:cached.serialNumber")):
            with self.subTest(before=before), self.assertRaises(ValueError):
                self.analyze(lambda value, before=before, after=after: value.replace(before, after))
            self.audit = HostLocalAudit()

    def test_ui_build_integer_string_missing_and_placeholder_are_distinct(self):
        from host import help_runtime_ui_version
        for build in (2610141058, "2610141058"):
            raw = ('<noscript id="version">' + json.dumps({"version": "1.0.0", "buildVersion": build}) + '</noscript>').encode()
            self.assertEqual(help_runtime_ui_version(raw), {"version": "1.0.0", "build_version": build})
        self.assertIsNone(help_runtime_ui_version(b"<html></html>"))
        self.assertIsNone(help_runtime_ui_version(b'<noscript id="version">VERSION_PLACEHOLDER</noscript>'))

    def test_ui_build_ambiguity_nested_markup_and_invalid_fields_fail(self):
        from host import help_runtime_ui_version
        valid = b'<noscript id="version">{"version":"1","buildVersion":2}</noscript>'
        for raw in (valid + valid, valid[:-11], valid.replace(b'{"version"', b'<b>{"version"'),
                    valid.replace(b':2', b':true'), valid.replace(b':2', b':-2'),
                    valid.replace(b':2', b':""'), valid.replace(b'{"version"', b'{&quot;version&quot;')):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                help_runtime_ui_version(raw)

    def query(self, *, fail=False):
        import analysis
        import frontend
        import host
        import resource_database
        catalog = self.root / ".analysis/inputs/help-catalog.json"
        catalog.parent.mkdir(parents=True, exist_ok=True)
        catalog.write_text(json.dumps([{"product_id": pid, "pages": [{"view_more": True, "offset": pid}]}
                                       for pid in (3594, 3595)]), encoding="utf-8")
        for name in ("host.py", "frontend.py", "evidence.py"):
            path = self.root / "tools" / name
            path.parent.mkdir(exist_ok=True)
            path.write_text("# query fixture\n", encoding="utf-8")
        entry = self.source('<noscript id="version">{"version":"1","buildVersion":2}</noscript>', "index.html")
        body = self.SOURCE + '(0,react.jsx)(Connected,{});'
        main = self.source(body.replace(".slice(1)", ".slice(0)") if fail else body)
        main["url"] = "https://apps.razer.com/synapse/products/3594/ui/main.js"
        source = {"audit": self.audit, "status": "verified", "entry": entry, "acquisition": {"path": "fixture"},
                  "acquired": {}, "selection": {"originals": [main]}, "freshness": [], "files": {main["url"]: main}}
        args = argparse.Namespace(product=[3594], query="help-runtime-versions")
        with patch.object(frontend, "current_acquisition_routes", return_value={}), \
                patch.object(frontend, "latest_deployment_observations", return_value={}), \
                patch.object(frontend, "current_product_source", return_value=source), \
                patch.object(frontend, "verify_latest_deployment_observations",
                             side_effect=lambda acquired, rows, audit, **kwargs: [{"url": row["url"]} for row in rows]), \
                patch.object(resource_database, "PRODUCT_HELP", catalog.relative_to(self.root).as_posix()), \
                patch.object(host, "__file__", str(self.root / "tools/host.py")), patch("builtins.print"):
            if fail:
                with self.assertRaisesRegex(ValueError, "remains unresolved"):
                    host.audit_host_help_runtime_versions(args)
            else:
                host.audit_host_help_runtime_versions(args)
        return json.loads((analysis.ROOT / ".analysis/results/host-current/local-audit.json").read_bytes())["help_runtime_versions"]

    def test_partial_query_does_not_claim_full_coverage_or_publisher_chain(self):
        report = self.query()
        self.assertEqual(report["requested_products"], [3594])
        self.assertEqual(report["all_local_help_products"], [3594, 3595])
        self.assertTrue(report["mounted_consumers_complete"])
        self.assertFalse(report["coverage_complete"])
        self.assertFalse(report["source_chain_complete"])
        self.assertEqual(report["runtime_acceptance"], "unverified")

    def test_failed_query_retains_selected_originals_complete_methods_and_failure(self):
        report = self.query(fail=True)
        self.assertFalse(report["mounted_consumers_complete"])
        self.assertFalse(report["coverage_complete"])
        self.assertTrue(report["complete_source_owners"])
        self.assertTrue(report["verified_artifacts"])
        self.assertEqual(report["products"][0]["acquisition"], {"path": "fixture"})
        self.assertEqual(report["products"][0]["ui_version"], {"version": "1", "build_version": 2})
        self.assertIn("getAppVersion", report["products"][0]["failures"][0])


class LightingFrontendOwnerTests(unittest.TestCase):
    def setUp(self):
        import analysis
        self.audit = HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)

    def source(self, text, name="main.fixture.js"):
        raw = text.encode("utf-8")
        path = self.root / ".analysis/inputs/frontend/fixture/raw" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                "sha256": hashlib.sha256(raw).hexdigest()}

    def test_class_identity_comes_from_its_own_constructor(self):
        source = self.source('class Other{constructor(){this.callback=()=>{this.name="Protocol"}}}'
                             'class Actual{constructor(){this.name="Protocol"}send(){return 1}}')
        node, owner = self.audit.named_class("protocol", source, self.audit.tree(source).root_node, "Protocol")
        self.assertTrue(node.text.startswith(b"class Actual"))
        self.assertEqual(self.audit.owners[owner]["syntax"], "class_declaration")

    def test_two_real_constructor_identities_are_ambiguous(self):
        source = self.source('class A{constructor(){this.name="Protocol"}}class B{constructor(){this.name="Protocol"}}')
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            self.audit.named_class("protocol", source, self.audit.tree(source).root_node, "Protocol")

    def test_dynamic_identity_write_cannot_be_ignored(self):
        source = self.source('class A{constructor(){this.name="Protocol";this.name=readName()}}')
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            self.audit.named_class("protocol", source, self.audit.tree(source).root_node, "Protocol")

    def test_repeated_factories_remain_separate_and_export_needs_scope(self):
        text = 'const modules={7:(e,t,n)=>{n.d(t,{A:()=>C});class C{constructor(){this.name="Protocol"}}}};'
        left, right = self.source(text, "left.js"), self.source(text, "right.js")
        matches = self.audit.webpack_factories([left, right], 7)
        self.assertEqual(len(matches), 2)
        for source, _, factory in matches:
            self.audit.named_class("protocol", source, factory, "Protocol")
        with self.assertRaisesRegex(ValueError, "ambiguous: 2"):
            self.audit.webpack_export("protocol", [left, right], 7, "A")

    def abi(self, mutate=lambda value: value):
        from host import lighting_frontend_abi
        source = self.source(mutate('const modules={2198:(e,t,n)=>{const c=n(4407);class Engine{async init(){'
            'const a={"4.0.2.0":{RzLightingApi:["pointer",["string"]],FreeMalloc:["void",["pointer"]]},'
            '"4.0.3.0":{PollEvents:["pointer",[]]}};'
            'return this.usingIpcLighting?await(0,c.Qq)(this.dllName,this.channelName,a):'
            '(0,c.Sl)(this.dllName,this.channelName,a)}}},'
            '4407:(e,t,n)=>{n.d(t,{Qq:()=>q,Sl:()=>s});const q=(d,c,a)=>a,s=(d,c,a)=>a}};'))
        return lighting_frontend_abi(self.audit, {"name": "fixture", "acquisition": {"path": "fixture"}}, [source])

    def test_abi_reads_literal_table_and_both_actual_configuration_calls(self):
        result = self.abi()
        self.assertEqual(result["api_versions"]["4.0.2.0"]["RzLightingApi"], ["pointer", ["string"]])
        self.assertEqual([(row["export"], row["awaited"]) for row in result["configuration_calls"]],
                         [("Qq", True), ("Sl", False)])
        self.assertEqual(len({row["api_declaration_owner"] for row in result["configuration_calls"]}), 1)
        self.assertFalse(result["target_execution"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_abi_rejects_wrong_table_argument(self):
        with self.assertRaisesRegex(ValueError, "does not consume"):
            self.abi(lambda text: text.replace("this.channelName,a", "this.channelName,other", 1))

    def test_abi_rejects_shadowed_table(self):
        with self.assertRaisesRegex(ValueError, "shadowed"):
            self.abi(lambda text: text.replace("return this.usingIpcLighting?", "return ((a)=>this.usingIpcLighting?")
                     .replace("this.channelName,a)}}}", "this.channelName,a))({})}}}"))

    def test_abi_rejects_wrong_bridge_import(self):
        with self.assertRaisesRegex(ValueError, "4407 import"):
            self.abi(lambda text: text.replace("n(4407)", "n(4408)"))

    def test_abi_rejects_same_number_from_an_unrelated_function(self):
        with self.assertRaisesRegex(ValueError, "actual webpack require"):
            self.abi(lambda text: text.replace("n(4407)", "unrelated(4407)"))

    def test_abi_rejects_missing_await_on_ipc_configuration(self):
        with self.assertRaisesRegex(ValueError, "two actual configuration"):
            self.abi(lambda text: text.replace("await(0,c.Qq)", "(0,c.Qq)"))

    def test_abi_rejects_dynamic_signature(self):
        with self.assertRaisesRegex(ValueError, "literal string"):
            self.abi(lambda text: text.replace('["pointer",["string"]]', '[returnType,["string"]]'))


class CurrentHostArchiveSelectionTests(unittest.TestCase):
    def test_official_package_closure_excludes_unrelated_installed_asar_members(self):
        from host import current_host_asar_members
        package = {"path": "package", "sha256": "package-sha", "manifest_identity_verified": True}
        official = {"path": ".analysis/inputs/host-current/extracted/current/app.asar", "sha256": "official-sha",
                    "bytes": 10, "container": "package", "container_sha256": "package-sha"}
        installed = {"path": ".analysis/inputs/installed-current/app.asar", "sha256": "installed-sha", "bytes": 11}
        member = {"path": ".analysis/inputs/host-current/asar/current/electron/main.js", "container": official["path"]}
        unrelated = {"path": ".analysis/inputs/host-current/asar/installed/electron/main.js", "container": installed["path"]}
        result = current_host_asar_members(None, {"inputs": [official, installed, member, unrelated]}, {"files": [package]})
        self.assertEqual(result, [member])
        package["manifest_identity_verified"] = False
        self.assertEqual(current_host_asar_members(None, {"inputs": [official, member]}, {"files": [package]}), [])

    def native_fixture(self, *, conflict=False, corrupt=False):
        import analysis
        audit = HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", root)
        patched.start()
        self.addCleanup(patched.stop)

        def original(name, raw, **fields):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), **fields}

        package = original(".analysis/inputs/host-current/blobs/current/package.exe", b"package",
                           manifest_identity_verified=True)
        current = original(".analysis/inputs/host-current/extracted/current/CommonDLL/library.dll", b"current",
                           container=package["path"], container_sha256=package["sha256"])
        old = original(".analysis/inputs/host-current/extracted/old/CommonDLL/library.dll", b"old",
                       container="old-package", container_sha256="old-sha")
        rows = [package, current, old]
        if conflict:
            rows.append(original(".analysis/inputs/host-current/extracted/also-current/CommonDLL/library.dll", b"other",
                                 container=package["path"], container_sha256=package["sha256"]))
        original(".analysis/inputs/host-current/acquisition.json", json.dumps({"files": [package]}).encode())
        original(".analysis/inputs.json", json.dumps({"inputs": rows}).encode())
        if corrupt:
            (root / current["path"]).write_bytes(b"changed-current")
        return audit, current

    def test_native_selection_ignores_same_name_outside_current_official_package(self):
        from host import verified_host_native
        audit, expected = self.native_fixture()
        actual, provenance = verified_host_native(audit, "CommonDLL/library.dll")
        self.assertEqual(actual, expected)
        self.assertEqual(provenance["official_package"]["path"], expected["container"])
        self.assertEqual(audit.final_snapshot(), [])

    def test_two_current_native_candidates_remain_an_error(self):
        from host import verified_host_native
        audit, _ = self.native_fixture(conflict=True)
        with self.assertRaisesRegex(ValueError, "absent or ambiguous"):
            verified_host_native(audit, "CommonDLL/library.dll")

    def test_corrupt_current_native_cannot_fall_back_to_old_package(self):
        from host import verified_host_native
        audit, _ = self.native_fixture(corrupt=True)
        with self.assertRaisesRegex(ValueError, "identity differs"):
            verified_host_native(audit, "CommonDLL/library.dll")


class KeypadMappingStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        if not (analysis.ROOT / ".analysis/inputs/host-current/acquisition.json").is_file():
            raise unittest.SkipTest("Current official host inputs are unavailable")
        audit = HostLocalAudit()
        cls.report = collect_keypad_mapping_static(audit)
        cls.raw = audit.read(cls.report["provenance"]["original"])

    def test_current_complete_consumers_and_dispatch_are_byte_verified(self):
        constants = self.report["verified_constants"]
        self.assertEqual(constants["axis_names"], ["left", "right", "up", "down", "invalid"])
        self.assertEqual(constants["joystick_ids"], ["button", "axes", "raxes", "pov"])
        self.assertGreaterEqual(constants["functions_verified"], 78)
        self.assertGreater(constants["instructions_verified"], 9000)
        self.assertFalse(self.report["target_execution"])
        self.assertFalse(self.report["source_chain_complete"])
        self.assertEqual(self.report["actual_dll_comparison"], "unverified")

    def test_missing_deferred_release_owner_cannot_pass(self):
        retained = dict(self.report["native"])
        retained["complete_functions"] = [row for row in retained["complete_functions"]
            if row["capture"]["rva"] != "0x100ed0"]
        with self.assertRaisesRegex(ValueError, "virtual dispatch"):
            keypad_mapping_constants(self.raw, retained)

    def test_instruction_edit_cannot_reuse_original_machine_code(self):
        retained = copy.deepcopy(self.report["native"])
        function = next(row["capture"] for row in retained["complete_functions"]
            if row["capture"]["rva"] == "0x7eb92")
        next(row for row in function["assembly"] if row["rva"] == "0x7ebe5")["bytes"] = "b8214e0000"
        with self.assertRaisesRegex(ValueError, "instruction differs"):
            keypad_mapping_constants(self.raw, retained)

    def test_changed_virtual_pointer_is_rejected_even_with_rehashed_region(self):
        retained = copy.deepcopy(self.report["native"])
        region = next(row for row in retained["verified_data_regions"] if row["role"] == "analog_stick_input")
        body = bytearray.fromhex(region["raw_hex"])
        body[0] ^= 1
        region["raw_hex"] = body.hex()
        region["sha256"] = hashlib.sha256(body).hexdigest()
        with self.assertRaisesRegex(ValueError, "data region differs"):
            keypad_mapping_constants(self.raw, retained)

    def test_shared_mapping_query_reaches_the_maintained_cli(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "keypad-mapping"])
        with patch("host.audit_host_keypad_mapping", return_value="receipt") as collect:
            self.assertEqual(audit_host_local(args), "receipt")
            collect.assert_called_once_with(args)


class MappingDescriptorStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        if not (analysis.ROOT / ".analysis/inputs/host-current/acquisition.json").is_file():
            raise unittest.SkipTest("Current official host inputs are unavailable")
        audit = HostLocalAudit()
        cls.report = collect_mapping_descriptors_static(audit)
        cls.raw = audit.read(cls.report["provenance"]["original"])
        cls.lifecycle = audit.native_slice(cls.report["provenance"]["original"], (), depth=0,
            rvas=MAPPING_DESCRIPTOR_LIFECYCLE_FUNCTIONS,
            regions=({"role": "input_base_lifetime_vtable", "rva": 0x2e5a18, "bytes": 80},))
        cls.parser_reuse = audit.native_slice(cls.report["provenance"]["original"], (), depth=0,
            rvas=MAPPING_DESCRIPTOR_PARSER_FUNCTIONS,
            regions=({"role": "mapping_descriptor_md5_initial_state", "rva": 0x2f3740, "bytes": 16},))

    def test_actual_primary_observers_and_report_classes_are_verified(self):
        proof = self.report["verified_contract"]
        tables = {row["role"]: row for row in proof["dispatch"]}
        for role in ("input_base", "usb", "ble", "external", "os_hook"):
            self.assertEqual([row["rva"] for row in tables[role + "_storage_vtable"]["targets"]][1:],
                ["0x65cc0", "0x65cc0", "0x662b0", "0x66590"])
        self.assertEqual([row["class"] for row in proof["report_names"]], list(range(9)))
        self.assertEqual(proof["report_names"][8]["name"], "hardwareEventReportID")
        self.assertEqual(proof["default_report_class"], 8)
        self.assertEqual(proof["report_id_key_bits"], 8)
        self.assertEqual(len(self.report["host_owners"]), 3)
        self.assertFalse(self.report["target_execution"])
        self.assertFalse(self.report["source_chain_complete"])
        self.assertEqual(self.report["actual_dll_comparison"], "unverified")

    def test_missing_publication_owner_cannot_pass_from_storage_success(self):
        retained = dict(self.report["native"])
        retained["complete_functions"] = [row for row in retained["complete_functions"]
            if row["capture"]["rva"] != "0xe3c90"]
        with self.assertRaisesRegex(ValueError, "virtual dispatch"):
            mapping_descriptor_proof(self.raw, retained)

    def test_omitted_change_gate_cannot_claim_a_complete_publication(self):
        retained = copy.deepcopy(self.report["native"])
        function = next(row["capture"] for row in retained["complete_functions"]
            if row["capture"]["rva"] == "0xe3c90")
        function["assembly"] = [row for row in function["assembly"] if row["rva"] != "0xe3cfc"]
        with self.assertRaisesRegex(ValueError, "ready_parse_changed_flag_then_publish"):
            mapping_descriptor_proof(self.raw, retained)

    def test_changed_low_byte_conversion_cannot_reuse_official_bytes(self):
        retained = copy.deepcopy(self.report["native"])
        function = next(row["capture"] for row in retained["complete_functions"]
            if row["capture"]["rva"] == "0xe7080")
        next(row for row in function["assembly"] if row["rva"] == "0xe71d4")["bytes"] = "8b44246c"
        with self.assertRaisesRegex(ValueError, "instruction differs"):
            mapping_descriptor_proof(self.raw, retained)

    def test_rehashed_observer_table_cannot_replace_primary_this_dispatch(self):
        retained = copy.deepcopy(self.report["native"])
        region = next(row for row in retained["verified_data_regions"] if row["role"] == "usb_storage_vtable")
        body = bytearray.fromhex(region["raw_hex"])
        body[8] ^= 1
        region["raw_hex"], region["sha256"] = body.hex(), hashlib.sha256(body).hexdigest()
        with self.assertRaisesRegex(ValueError, "data region differs"):
            mapping_descriptor_proof(self.raw, retained)

    def test_primary_constructor_cannot_omit_this_assignment(self):
        retained = copy.deepcopy(self.report["native"])
        function = next(row["capture"] for row in retained["complete_functions"]
            if row["capture"]["rva"] == "0xa89f0")
        function["assembly"] = [row for row in function["assembly"] if row["rva"] != "0xa8a4c"]
        with self.assertRaisesRegex(ValueError, "storage observer this or constructor"):
            mapping_descriptor_proof(self.raw, retained)

    def test_mapping_descriptor_query_reaches_the_maintained_cli(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "mapping-descriptors"])
        with patch("host.audit_host_mapping_descriptors", return_value="receipt") as collect:
            self.assertEqual(audit_host_local(args), "receipt")
            collect.assert_called_once_with(args)

    def test_destructor_queued_base_slot_is_a_noop_not_device_confirmation(self):
        proof = mapping_descriptor_lifecycle_proof(self.raw, self.lifecycle)
        self.assertEqual(proof["functions_verified"], 19)
        self.assertEqual(proof["input_base_slot72_target_rva"], "0x77f00")

    def test_lifecycle_cannot_omit_owned_thread_join(self):
        retained = dict(self.lifecycle)
        retained["complete_functions"] = [row for row in retained["complete_functions"]
            if row["capture"]["rva"] != "0x1a9710"]
        with self.assertRaisesRegex(ValueError, "complete owners differ"):
            mapping_descriptor_lifecycle_proof(self.raw, retained)

    def test_lifecycle_cannot_remove_unregister_tail(self):
        retained = copy.deepcopy(self.lifecycle)
        function = next(row["capture"] for row in retained["complete_functions"]
            if row["capture"]["rva"] == "0x547e0")
        function["assembly"] = [row for row in function["assembly"] if row["rva"] != "0x54894"]
        with self.assertRaisesRegex(ValueError, "reviewed instruction differs"):
            mapping_descriptor_lifecycle_proof(self.raw, retained)

    def test_equal_profile_hash_retains_the_shared_gamemode_mutation(self):
        proof = mapping_descriptor_parser_proof(self.raw, self.report["native"], self.parser_reuse)
        self.assertTrue(proof["same_hash_profile_shared_mutation"])
        self.assertFalse(proof["same_hash_rechecks_mapping_payload"])
        self.assertEqual(proof["additional_functions_verified"], 18)

    def test_equal_hash_branch_cannot_omit_the_shared_object_write(self):
        baseline = copy.deepcopy(self.report["native"])
        function = next(row["capture"] for row in baseline["complete_functions"]
            if row["capture"]["rva"] == "0xe75a0")
        function["assembly"] = [row for row in function["assembly"] if row["rva"] != "0xe7e86"]
        with self.assertRaisesRegex(ValueError, "same_hash_mutates_shared_gamemode"):
            mapping_descriptor_parser_proof(self.raw, baseline, self.parser_reuse)

    def test_profile_reuse_cannot_be_treated_as_validated_new_payload(self):
        baseline = copy.deepcopy(self.report["native"])
        function = next(row["capture"] for row in baseline["complete_functions"]
            if row["capture"]["rva"] == "0xe75a0")
        function["assembly"] = [row for row in function["assembly"] if row["rva"] != "0xe7ef5"]
        with self.assertRaisesRegex(ValueError, "same_hash_mutates_shared_gamemode"):
            mapping_descriptor_parser_proof(self.raw, baseline, self.parser_reuse)

    def test_rehashed_md5_state_must_still_match_the_actual_pe(self):
        retained = copy.deepcopy(self.parser_reuse)
        region = retained["verified_data_regions"][0]
        body = bytearray.fromhex(region["raw_hex"])
        body[0] ^= 1
        region["raw_hex"], region["sha256"] = body.hex(), hashlib.sha256(body).hexdigest()
        with self.assertRaisesRegex(ValueError, "data region differs"):
            mapping_descriptor_parser_proof(self.raw, self.report["native"], retained)

    def gamemode_fixture(self):
        audit = HostLocalAudit()
        directory = audit.a.ROOT / ".analysis/results/reverse-pending/review-sources"
        baseline = audit.families.identity(directory / "21fe84d134926df8d0aecdcd5bbc215e298bd2a4709a9a964d7e344624b27960.json")
        parser = audit.families.identity(directory / "4b228a4f2121c88570c573c8e45db32823696fe18c67b86f387df4308038b5c1.json")
        retained = audit.native_slice(self.report["provenance"]["original"], (), depth=0,
            rvas=(0xe5a36, 0x8eb70, 0xc438a, 0xc45b0, 0xc0e76))
        return audit, baseline, parser, retained

    def test_effective_gamemode_consumer_keeps_all_service_owners(self):
        audit, baseline, parser, retained = self.gamemode_fixture()
        with patch.object(audit, "native_slice", return_value=retained):
            report = collect_mapping_descriptor_gamemode(audit, baseline, parser)
        self.assertEqual(report["verified_contract"]["additional_functions_verified"], 5)
        self.assertFalse(report["target_execution"])
        self.assertFalse(report["source_chain_complete"])

    def test_mode_service_omission_cannot_pass_from_the_cached_getter(self):
        audit, baseline, parser, retained = self.gamemode_fixture()
        retained["complete_functions"] = [row for row in retained["complete_functions"]
            if row["capture"]["rva"] != "0xc45b0"]
        with patch.object(audit, "native_slice", return_value=retained):
            with self.assertRaisesRegex(ValueError, "complete owners differ"):
                collect_mapping_descriptor_gamemode(audit, baseline, parser)

    def test_effective_gamemode_cannot_drop_the_published_cache_read(self):
        audit, baseline, parser, retained = self.gamemode_fixture()
        function = next(row["capture"] for row in retained["complete_functions"]
            if row["capture"]["rva"] == "0xe5a36")
        function["assembly"] = [row for row in function["assembly"] if row["rva"] != "0xe5a54"]
        with patch.object(audit, "native_slice", return_value=retained):
            with self.assertRaisesRegex(ValueError, "gamemode consumer instruction differs"):
                collect_mapping_descriptor_gamemode(audit, baseline, parser)


class FrontendCaptureModuleTests(unittest.TestCase):
    def fixture(self):
        import analysis
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        audit = HostLocalAudit()
        replacement = patch.object(analysis, "ROOT", root)
        replacement.start()
        self.addCleanup(replacement.stop)

        def write(name, body):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(body)
            return {"path": name, "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest()}

        name = "module-fixture-current"
        originals = [write(".analysis/inputs/frontend/" + name + "/" + file, body)
            for file, body in (("main.js", b"const main=1;"), ("manifest.mjs", b"export default {};"),
                              ("shared.MJS", b"export const shared=2;"), ("main.css", b"body{}"))]
        output = write(".analysis/results/frontend/" + name + "/modules.json", b"[]")
        tool = write("tools/analyzers/wakaru-1.0.0/wakaru.exe", b"inert fixture; never executed")
        capture = {"input": ".analysis/inputs/frontend/" + name, "original_inputs": originals,
            "original_js_inputs": [row["path"] for row in originals[:3]], "outputs": [output],
            "tool": {"version": "1.0.0", "sha256": tool["sha256"]}, "commands": []}
        manifest = root / ".analysis/results/frontend" / name / "analysis.json"
        manifest.write_text(json.dumps(capture), encoding="utf-8")
        return audit, name, originals, capture, manifest

    def test_es_modules_are_part_of_the_exact_capture_input_set(self):
        audit, name, originals, _, _ = self.fixture()
        receipt = audit.frontend_capture(name, originals)
        self.assertTrue(receipt["successful_full_capture"])
        self.assertEqual(receipt["originals_verified"], 4)

    def test_omitted_es_module_remains_a_capture_failure(self):
        audit, name, originals, capture, manifest = self.fixture()
        capture["original_js_inputs"].pop()
        manifest.write_text(json.dumps(capture), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "JavaScript input list differs"):
            audit.frontend_capture(name, originals)

    def test_repeated_es_module_or_stylesheet_cannot_satisfy_the_input_set(self):
        for replacement in ("duplicate", "stylesheet"):
            with self.subTest(replacement=replacement):
                audit, name, originals, capture, manifest = self.fixture()
                capture["original_js_inputs"].append(originals[1 if replacement == "duplicate" else 3]["path"])
                manifest.write_text(json.dumps(capture), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "JavaScript input list differs"):
                    audit.frontend_capture(name, originals)

    def acquired_blob_fixture(self):
        audit, name, originals, capture, manifest = self.fixture()
        body = b"export default {};"
        digest = hashlib.sha256(body).hexdigest()
        blob = audit.a.ROOT / ".analysis/inputs/frontend-revalidation/blobs" / digest
        blob.parent.mkdir(parents=True, exist_ok=True)
        blob.write_bytes(body)
        originals[1] = {"path": audit.a.rel(blob), "bytes": len(body), "sha256": digest}
        capture["original_js_inputs"][1] = originals[1]["path"]
        acquisition = {"entry_url": "https://apps.razer.com/synapse/fixture/",
            "source_current_during_acquisition": True, "target_execution": False,
            "files": [{"url": "https://apps.razer.com/synapse/fixture/" + filename, **row}
                for row, filename in zip(originals, ("main.js", "manifest.js", "shared.MJS", "main.css"))]}
        acquired_path = audit.a.ROOT / ".analysis/inputs/frontend" / name / "acquisition.json"
        acquired_path.write_text(json.dumps(acquisition), encoding="utf-8")
        manifest.write_text(json.dumps(capture), encoding="utf-8")
        return audit, name, originals, capture, manifest, acquisition, acquired_path

    def test_acquired_javascript_url_identifies_an_exact_hash_addressed_blob(self):
        audit, name, originals, _, _, _, _ = self.acquired_blob_fixture()
        receipt = audit.frontend_capture(name, originals)
        self.assertTrue(receipt["successful_full_capture"])
        self.assertEqual(receipt["originals_verified"], 4)
        self.assertEqual(audit.sources[originals[1]["path"]], originals[1])

    def test_unregistered_blob_or_non_javascript_url_cannot_supply_a_script(self):
        for change in ("unregistered", "non_javascript_url", "identity_mismatch"):
            with self.subTest(change=change):
                audit, name, originals, _, _, acquisition, acquired_path = self.acquired_blob_fixture()
                if change == "unregistered":
                    acquisition["files"].pop(1)
                elif change == "non_javascript_url":
                    acquisition["files"][1]["url"] = "https://apps.razer.com/synapse/fixture/manifest.json"
                else:
                    acquisition["files"][1]["sha256"] = "0" * 64
                acquired_path.write_text(json.dumps(acquisition), encoding="utf-8")
                with self.assertRaises(ValueError):
                    audit.frontend_capture(name, originals)

    def test_javascript_blob_still_requires_its_exact_capture_declaration(self):
        audit, name, originals, capture, manifest, _, _ = self.acquired_blob_fixture()
        capture["original_js_inputs"].remove(originals[1]["path"])
        manifest.write_text(json.dumps(capture), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "JavaScript input list differs"):
            audit.frontend_capture(name, originals)

    def shared_recipe_fixture(self):
        audit, name, originals, capture, manifest = self.fixture()
        body = b"retained static recovery recipe; never executed"
        digest = hashlib.sha256(body).hexdigest()
        target = audit.a.ROOT / ".analysis/results/frontend/recovery-tools" / (digest + ".py")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(body)
        output = {"path": audit.a.rel(target), "bytes": len(body), "sha256": digest}
        recipe = {"path": output["path"], "sha256": digest, "parser_versions": {"tree-sitter": "fixture"}}
        capture["module_recovery"] = {"recipe": recipe}
        capture["outputs"].append(output)
        manifest.write_text(json.dumps(capture), encoding="utf-8")
        return audit, name, originals, capture, manifest

    def test_exact_hash_addressed_recovery_recipe_is_a_verified_shared_output(self):
        audit, name, originals, _, _ = self.shared_recipe_fixture()
        self.assertEqual(audit.frontend_capture(name, originals)["outputs_verified"], 2)

    def test_shared_recipe_cannot_authorize_an_unrelated_output(self):
        audit, name, originals, capture, manifest = self.shared_recipe_fixture()
        capture["outputs"][-1]["path"] = ".analysis/results/frontend/recovery-tools/unrelated.py"
        manifest.write_text(json.dumps(capture), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "output boundary"):
            audit.frontend_capture(name, originals)

    def test_shared_recipe_must_bind_the_output_hash_and_actual_bytes(self):
        for change in ("declared_hash", "actual_bytes", "missing_output", "missing_parser"):
            with self.subTest(change=change):
                audit, name, originals, capture, manifest = self.shared_recipe_fixture()
                if change == "declared_hash":
                    capture["outputs"][-1]["sha256"] = "0" * 64
                elif change == "actual_bytes":
                    (audit.a.ROOT / capture["outputs"][-1]["path"]).write_bytes(b"changed recipe")
                elif change == "missing_output":
                    capture["outputs"].pop()
                else:
                    capture["module_recovery"]["recipe"]["parser_versions"] = {}
                manifest.write_text(json.dumps(capture), encoding="utf-8")
                with self.assertRaises(ValueError):
                    audit.frontend_capture(name, originals)


class ProfilesOwnerReceiptTests(unittest.TestCase):
    def fixture(self):
        import analysis
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        audit = HostLocalAudit()
        replacement = patch.object(analysis, "ROOT", root)
        replacement.start()
        self.addCleanup(replacement.stop)
        path = ".analysis/inputs/frontend/profiles-fixture-current/main.js"
        raw = b"const commit=()=>{return 7};\n"
        target = root / path
        target.parent.mkdir(parents=True)
        target.write_bytes(raw)
        original = {"path": path, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        node = audit.tree(original).root_node.named_children[0].named_children[0]
        body = raw[node.start_byte:node.end_byte]
        owner = {"original": original, "roles": ["fixture_commit"], "syntax": node.type,
            "byte_start": node.start_byte, "byte_end": node.end_byte,
            "body": body.decode(), "body_sha256": hashlib.sha256(body).hexdigest(),
            "id": original["sha256"] + ":" + str(node.start_byte) + ":" + str(node.end_byte)}
        return audit, owner, target

    def test_complete_owner_is_checked_against_original_bytes(self):
        audit, owner, _ = self.fixture()
        self.assertEqual(verify_profiles_owner(audit, owner).type, "variable_declarator")

    def test_changed_body_is_rejected_even_after_attacker_rehashes_it(self):
        audit, owner, _ = self.fixture()
        owner["body"] = owner["body"].replace("return 7", "return 8")
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(audit, owner)

    def test_byte_correct_excerpt_cannot_claim_a_complete_owner(self):
        audit, owner, _ = self.fixture()
        owner["byte_end"] -= 1
        owner["body"] = owner["body"][:-1]
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        owner["id"] = owner["original"]["sha256"] + ":" + str(owner["byte_start"]) + ":" + str(owner["byte_end"])
        with self.assertRaisesRegex(ValueError, "complete original AST"):
            verify_profiles_owner(audit, owner)

    def test_modified_actual_source_is_rejected_by_a_fresh_verifier(self):
        _, owner, target = self.fixture()
        target.write_bytes(target.read_bytes().replace(b"return 7", b"return 8"))
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            verify_profiles_owner(HostLocalAudit(), owner)


class ProfilesPublicationTests(unittest.TestCase):
    def test_query_and_independent_verify_are_explicit_cli_options(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "profiles", "--verify-only"])
        self.assertEqual(args.query, "profiles")
        self.assertTrue(args.verify_only)

    def test_unrelated_report_fields_keep_canonical_identity(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "local-audit.json"
            old = {"schema_version": 1, "lighting_frame_lifecycle": {"branches": [2, 1], "state": "observed"},
                "window_lifecycle": {"target_execution": False}}
            destination.write_text(json.dumps(old), encoding="utf-8")
            result = {"query": "profiles"}
            preserved = publish_profiles_static(result, destination, analysis)
            after = json.loads(destination.read_bytes())
            self.assertEqual({key: after[key] for key in old}, old)
            self.assertEqual(set(preserved), set(old))
            expected = hashlib.sha256(b'{"branches":[2,1],"state":"observed"}').hexdigest()
            self.assertEqual(preserved["lighting_frame_lifecycle"], expected)
            self.assertEqual(after["profiles"]["preserved_top_level_canonical_sha256"], preserved)

    def test_publication_reads_back_the_saved_unrelated_fields(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "local-audit.json"
            destination.write_text('{"schema_version":1,"lighting_frame_lifecycle":{"x":1}}')
            def corrupt(path, value):
                value["lighting_frame_lifecycle"]["x"] = 2
                path.write_text(json.dumps(value), encoding="utf-8")
            with patch.object(analysis, "save", side_effect=corrupt):
                with self.assertRaisesRegex(ValueError, "unrelated top-level"):
                    publish_profiles_static({"query": "profiles"}, destination, analysis)


class HostStaticQueryFailureHistoryTests(unittest.TestCase):
    def fixture(self, root):
        destination = root / ".analysis/results/host-current/local-audit.json"
        destination.parent.mkdir(parents=True)
        prior = {"failures": ["actual source identity differs"], "target_execution": False,
                 "complete_source_owners": [{"body": "complete retained failure evidence"}]}
        document = {"schema_version": 1, "dynamic_lighting": prior,
                    "window_lifecycle": {"state": [2, 1]}}
        destination.write_text(json.dumps(document), encoding="utf-8")
        return destination, document

    def test_success_preserves_complete_failed_query_and_keeps_its_history(self):
        import analysis
        import reverse_pending
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            destination, before = self.fixture(Path(directory))
            result = {"failures": [], "target_execution": False}
            publish_host_static_query(result, destination, analysis, "dynamic_lighting")
            reference = result["previous_failed_query"]
            self.assertTrue(reverse_pending.valid(reference))
            snapshot = json.loads((Path(directory) / reference["path"]).read_bytes())
            self.assertEqual(snapshot["report"], before["dynamic_lighting"])
            self.assertEqual(snapshot["query"], "dynamic_lighting")
            self.assertFalse(snapshot["target_execution"])
            after = json.loads(destination.read_bytes())
            self.assertEqual(after["window_lifecycle"], before["window_lifecycle"])
            next_result = {"failures": [], "target_execution": False}
            publish_host_static_query(next_result, destination, analysis, "dynamic_lighting")
            self.assertEqual(next_result["previous_failed_query"], reference)
            self.assertEqual(len(list((Path(directory) / Path(reference["path"]).parent).glob("*.json"))), 1)

    def test_failure_snapshot_publication_error_preserves_current_receipt(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            destination, _ = self.fixture(Path(directory))
            before = destination.read_bytes()
            with patch("reverse_pending.retain_review_json", side_effect=OSError("cannot retain failure")):
                with self.assertRaisesRegex(OSError, "cannot retain"):
                    publish_host_static_query({"failures": []}, destination, analysis, "dynamic_lighting")
            self.assertEqual(destination.read_bytes(), before)

    def test_changed_failure_reference_blocks_successful_republication(self):
        import analysis
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            destination, _ = self.fixture(Path(directory))
            result = {"failures": [], "target_execution": False}
            publish_host_static_query(result, destination, analysis, "dynamic_lighting")
            snapshot = Path(directory) / result["previous_failed_query"]["path"]
            snapshot.write_bytes(snapshot.read_bytes() + b" ")
            before = destination.read_bytes()
            with self.assertRaisesRegex(ValueError, "lost its retained"):
                publish_host_static_query({"failures": []}, destination, analysis, "dynamic_lighting")
            self.assertEqual(destination.read_bytes(), before)

    def test_another_query_failure_cannot_be_adopted_as_this_query_history(self):
        import analysis
        import reverse_pending
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            destination, document = self.fixture(Path(directory))
            reference = reverse_pending.retain_review_json({"kind": "failed_host_static_query",
                "query": "profiles", "target_execution": False,
                "report": document["dynamic_lighting"]})
            document["dynamic_lighting"] = {"failures": [], "previous_failed_query": reference}
            destination.write_text(json.dumps(document), encoding="utf-8")
            before = destination.read_bytes()
            with self.assertRaisesRegex(ValueError, "another query or scope"):
                publish_host_static_query({"failures": []}, destination, analysis, "dynamic_lighting")
            self.assertEqual(destination.read_bytes(), before)


class ProfilesCurrentStaticReceiptTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        if not (analysis.ROOT / ".analysis/inputs/frontend/synapse-profiles-current/acquisition.json").is_file():
            raise unittest.SkipTest("Current official static inputs are unavailable")
        cls.receipt = collect_profiles_static(HostLocalAudit())

    def test_independent_verifier_does_not_call_the_collector(self):
        with patch("host.collect_profiles_static", side_effect=AssertionError("collector must not run")):
            result = verify_profiles_static_receipt(self.receipt)
        self.assertEqual(result["selected_owners"], 32)
        self.assertFalse(result["target_execution"])
        self.assertEqual(result["runtime_acceptance"], "unverified")

    def test_failure_and_cancellation_cannot_be_relabelled_as_success(self):
        for group, key, value in (("installed_apps", "empty_or_failure_clears_refresh", True),
                ("installed_apps", "inflight_cancel_on_unmount", True),
                ("browse", "dispatch", "joined_successful_batch"),
                ("add", "publication_error_rethrown", True),
                ("add", "close_proves_service_success", True),
                ("drop", "overlay_drop_cleanup", "removes_listener"),
                ("scan", "stop_action_in_page", True),
                ("scan", "finished_requires_success_flag", True),
                ("relocate", "selection_dispatches_add", True),
                ("relocate", "clear_and_select_requires_link", False),
                ("host", "async_error_is_reliable_rejection", True)):
            with self.subTest(group=group, key=key):
                changed = copy.deepcopy(self.receipt)
                changed["boundaries"][group][key] = value
                with self.assertRaisesRegex(ValueError, "boundary was changed"):
                    verify_profiles_static_receipt(changed)

    def test_static_source_cannot_be_promoted_to_runtime_or_implementation_acceptance(self):
        for key, value in (("source_chain_complete", True), ("target_execution", True),
                ("real_device_operations", True), ("actual_dll_comparison", "passed"),
                ("rust_independent_implementation", "complete"), ("ui_backend_wiring", "complete"),
                ("runtime_acceptance", "passed"), ("snapshot_stable", False)):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.receipt)
                changed[key] = value
                with self.assertRaisesRegex(ValueError, "promotes static evidence"):
                    verify_profiles_static_receipt(changed)

    def test_forged_complete_function_is_rejected_even_with_a_matching_recorded_hash(self):
        changed = copy.deepcopy(self.receipt)
        owner = next(row for row in changed["complete_source_owners"]
            if row["id"] == changed["selected"]["add_action"])
        owner["body"] = owner["body"].replace("catch(o){console.log", "catch(o){throw o;console.log")
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_static_receipt(changed)

    def test_missing_source_export_graph_is_rejected(self):
        changed = copy.deepcopy(self.receipt)
        del changed["cross_chunk_exports"]["scan_action"]
        with self.assertRaisesRegex(ValueError, "export set differs"):
            verify_profiles_static_receipt(changed)

    def test_import_cannot_be_rebound_to_a_similarly_named_module(self):
        changed = copy.deepcopy(self.receipt)
        changed["bindings"]["games_drag_import"]["body"] = "H=s(399)"
        with self.assertRaisesRegex(ValueError, "producer import binding differs"):
            verify_profiles_static_receipt(changed)

    def test_capture_failure_cannot_disappear_from_current_receipt(self):
        changed = copy.deepcopy(self.receipt)
        self.assertTrue(changed["source_sets"][0]["capture_failures"])
        changed["source_sets"][0]["capture_failures"] = []
        with self.assertRaisesRegex(ValueError, "deployment receipt differs: capture_failures"):
            verify_profiles_static_receipt(changed)

    def test_changed_acquisition_cannot_be_silently_transferred_to_old_capture(self):
        changed = copy.deepcopy(self.receipt)
        relation = changed["source_sets"][0]["capture_acquisition_relation"]
        self.assertNotEqual(relation["capture_recorded_acquisition_sha256"], relation["current_acquisition_sha256"])
        relation["capture_recorded_acquisition_sha256"] = relation["current_acquisition_sha256"]
        with self.assertRaisesRegex(ValueError, "acquisition change was hidden"):
            verify_profiles_static_receipt(changed)


class ProfileLocalApplicationsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        import host
        capture = analysis.ROOT / ".analysis/results/native/bc0387a0913c882a9cd82aa1a8e147d6408fc7bb7f77519f4f8860f16c5b17c8/capture.json"
        if not capture.is_file():
            raise unittest.SkipTest("Current official SysUtils static inputs are unavailable")
        cls.receipt = host.collect_profile_local_applications(HostLocalAudit())

    def verify(self, receipt):
        from host import _verify_profile_local_applications_claims
        return _verify_profile_local_applications_claims(receipt)

    def test_current_release_and_actual_ui_native_consumers_are_selected(self):
        result = self.verify(self.receipt)
        self.assertEqual(self.receipt["host_release"]["version"], "4.0.896")
        self.assertEqual(self.receipt["host_release"]["native_file_versions"], ["1.2.3.0"])
        self.assertEqual(self.receipt["source_set"]["name"], "synapse-profiles-current-refresh-ec3db4b29c5a")
        self.assertEqual((result["selected_source_owners"], result["selected_native_functions"]), (49, 40))
        self.assertEqual(len(self.receipt["native"]["verified_call_edges"]), 35)
        self.assertFalse(self.receipt["profiles_dependency"]["previous_frontend_owner_bytes_reused"])
        self.assertFalse(self.receipt["profiles_dependency"]["previous_host_evidence_reused"])

    def test_strict_sorted_json_roundtrip_keeps_complete_review(self):
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(saved), self.receipt["verification"])
        self.assertEqual(saved["native"]["complete_selected_functions"],
                         self.receipt["native"]["complete_selected_functions"])

    def test_complete_owners_match_actual_ast_bytes(self):
        audit = HostLocalAudit()
        for owner in self.receipt["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])

    def test_failures_cancellation_and_release_cannot_be_promoted(self):
        for group, name, value in (
                ("registry", "empty_result", "JSON empty array string"),
                ("registry", "uwp_or_shortcut_enumeration", True),
                ("icons", "native_cancel_parameter", True),
                ("icons", "cache_missing_file_can_hit", False),
                ("icons", "batch_prunes_other_keys", False),
                ("icons", "cache_atomic_publish", True),
                ("icons", "temporary_files_always_deleted", True),
                ("icons", "short_read_has_zero_progress_exit", True),
                ("icons", "all_bitmap_failure_paths_release", True),
                ("host", "async_callback_error_is_reliable_rejection", True),
                ("host", "free_after_readCString_exception", True),
                ("host", "FreeFFI_waits_for_inflight_or_confirms_unload", True),
                ("page", "empty_or_failure_clears_previous_list", True),
                ("page", "unmount_cancels_native_query", True),
                ("page", "stale_generation_guard", True),
                ("page", "list_update_proves_game_or_device_write", True)):
            with self.subTest(group=group, name=name):
                changed = copy.deepcopy(self.receipt)
                changed["boundaries"][group][name] = value
                with self.assertRaisesRegex(ValueError, "failure/release boundary differs"):
                    self.verify(changed)

    def test_static_scope_does_not_claim_whole_profiles_or_runtime_success(self):
        for name, value in (("whole_profiles_complete", True), ("target_execution", True),
                ("real_device_operations", True), ("actual_dll_comparison", "passed"),
                ("rust_independent_implementation", "complete"), ("ui_backend_wiring", "complete"),
                ("runtime_acceptance", "passed"), ("product_code_modified", True),
                ("snapshot_stable", False), ("changed_during_query", ["changed-source"])):
            with self.subTest(name=name):
                changed = copy.deepcopy(self.receipt)
                changed[name] = value
                with self.assertRaisesRegex(ValueError, "static review was promoted"):
                    self.verify(changed)

    def test_ffi_signature_or_release_export_cannot_be_changed(self):
        changed = copy.deepcopy(self.receipt)
        changed["ffi_declarations"]["getApplicationIcons"]["signature"][1][0] = "pointer"
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)
        changed = copy.deepcopy(self.receipt)
        changed["native"]["verified_exports"][0]["rva"] = "0x6f080"
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_forged_owner_stays_invalid_after_hash_recalculation(self):
        changed = copy.deepcopy(self.receipt)
        owner = next(row for row in changed["complete_source_owners"]
                     if row["id"] == changed["selected"]["componentWillUnmount"])
        owner["body"] = owner["body"].replace("this.timeout&&", "this.cancelNativeQuery(),this.timeout&&")
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(HostLocalAudit(), owner)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_current_exports_bindings_call_edges_and_pe_roots_are_not_optional(self):
        for mutate in (lambda r: r["cross_chunk_exports"].pop("list_action_export"),
                lambda r: r["bindings"]["ui_api_import"].update(body="u=a(7226)"),
                lambda r: r["native"]["verified_call_edges"].pop(),
                lambda r: r["native"]["verified_data_regions"][0].update(decoded=["HKCU", "HKLM"])):
            changed = copy.deepcopy(self.receipt)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_global_hexrays_failure_remains_separate_from_selected_success(self):
        self.assertEqual(self.receipt["native"]["selected_pseudocode_failures"], [])
        self.assertEqual(len(self.receipt["native"]["failures"]), 1)
        changed = copy.deepcopy(self.receipt)
        changed["native"]["failures"] = []
        with self.assertRaisesRegex(ValueError, "native capture failure was hidden"):
            self.verify(changed)


class ProfileScannerVersionDifferencesTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.receipt = host.collect_profile_scanner_version_differences(cls.audit)
        cls.current = next(row for row in cls.receipt["assemblies"] if row["version"] == "3.15.0.1200"
                           and row["original"]["path"].endswith("/GameManagerServiceLib.dll"))
        metadata = host._profile_gog_clr(cls.audit.read(cls.current["original"]))
        cls.heap = host._profile_scanner_user_heap(metadata)

    def verify(self, receipt):
        from host import _verify_profile_scanner_version_differences
        return _verify_profile_scanner_version_differences(receipt)

    def selected(self, name):
        return next(row for row in self.current["complete_methods"] if len(row["class_ancestry"]) == 1 and row["name"] == name)

    def test_current_package_actual_registrations_and_gog_reuse(self):
        receipt = self.receipt
        self.assertEqual(receipt["host_release"]["version"], "4.0.896")
        self.assertEqual(receipt["host_release"]["official_gms_package"]["declaration"]["resourceVersion"], "3.15.0.1200")
        self.assertEqual(len(receipt["registration"]), 7)
        self.assertTrue(all(row["resolved_method"]["name"] == "AddScoped" for row in receipt["registration"]))
        self.assertTrue(receipt["assemblies"][-1]["service_methods_reused_from_gog"])
        self.assertFalse(any("GogRegistrySearch" in method["class_ancestry"][0]
                            for assembly in receipt["assemblies"] for method in assembly["complete_methods"]))

    def test_equivalence_requires_original_code_calls_constants_and_complete_il(self):
        same = [row for row in self.receipt["comparisons"] if row["classification"] == "same_selected_method_after_metadata_relocation"]
        self.assertTrue(same)
        self.assertTrue(any(not row["original_code_identical"] for row in same))
        for row in same:
            self.assertTrue(row["non_token_bytes_identical"])
            self.assertTrue(row["original_user_strings_identical"])
            self.assertTrue(row["resolved_call_targets_identical"])
            self.assertTrue(row["complete_il_except_rva_identical"])
        sources = {row["class"]: row for row in self.receipt["source_comparisons"]}
        for name in ("ArcRegistrySearch", "BethesdaRegistry", "EARegistrySearch", "MihoyoPlatformSearch"):
            self.assertTrue(sources["GameScannerStrategy" + name]["complete_source_bytes_equal"])
        for name in ("RiotPlatformSearch", "UplayConfigurationSearch"):
            self.assertFalse(sources["GameScannerStrategy" + name]["complete_source_bytes_equal"])

    def test_riot_pattern_and_uplay_ranks_come_from_original_bytes(self):
        changes = self.receipt["reviewed_changes"]
        self.assertIn("U([0-9a-fA-F]{8})", changes["riot"]["regex_from_original_user_heap"]["value"])
        self.assertTrue(changes["riot"]["decoder_exception_sections"])
        self.assertEqual(changes["uplay"]["original_rank_returns"], [0, 1, 2, 3, 4])
        self.assertEqual(changes["uplay"]["matching_passes"], 5)
        self.assertEqual([row["value"] for row in changes["uplay"]["display_literals"]], ["?"] * 4)
        self.assertEqual(len(changes["verified_changed_call_edges"]), 28)

    def test_token_masks_cannot_hide_non_token_constant_change(self):
        from host import _profile_scanner_compare, _profile_scanner_token_ranges
        original = self.selected("GetFallbackLaunchExeRank")
        changed = copy.deepcopy(original)
        instruction = next(row for row in changed["instructions"] if row["opcode"] == "ldc.i4.0")
        raw = bytearray.fromhex(changed["raw_code_hex"])
        raw[instruction["offset"]] = 0x17
        changed["raw_code_hex"], changed["raw_code_sha256"] = raw.hex(), hashlib.sha256(raw).hexdigest()
        changed.update(_profile_scanner_token_ranges(changed, self.heap))
        result = _profile_scanner_compare(original, changed)
        self.assertFalse(result["non_token_bytes_identical"])
        self.assertEqual(result["classification"], "changed_selected_method")

    def test_original_heap_string_change_is_not_metadata_relocation(self):
        from host import _profile_scanner_compare, _profile_scanner_token_ranges
        raw = bytes.fromhex("72010000702a")
        old = {"class_ancestry": ["Fixture"], "complete_il": '.method static string F() { // Method begins at RVA 0x1\n ldstr "A"\n ret\n}',
            "token": "0x6000001", "rva": "0x1", "code_size": len(raw), "raw_code_hex": raw.hex(),
            "raw_code_sha256": hashlib.sha256(raw).hexdigest(), "exception_sections": [], "calls": [],
            "instructions": [{"offset": 0, "opcode": "ldstr", "operand": '"A"'}, {"offset": 5, "opcode": "ret", "operand": ""}]}
        new = copy.deepcopy(old)
        old.update(_profile_scanner_token_ranges(old, bytes.fromhex("0003410000")))
        new.update(_profile_scanner_token_ranges(new, bytes.fromhex("0003420000")))
        result = _profile_scanner_compare(old, new)
        self.assertTrue(result["non_token_bytes_identical"])
        self.assertFalse(result["original_user_strings_identical"])
        self.assertEqual(result["classification"], "changed_selected_method")

    def test_token_mask_requires_actual_opcode_and_full_operand_extent(self):
        from host import _profile_scanner_token_ranges
        original = self.selected("GetUplayDisplayName")
        entry = next(row for row in original["token_operands"] if row["opcode"] == "ldstr")
        for change in ("opcode", "extent"):
            changed = copy.deepcopy(original)
            if change == "opcode":
                raw = bytearray.fromhex(changed["raw_code_hex"])
                raw[entry["offset"]] = 0x00
                changed["raw_code_hex"], changed["raw_code_sha256"] = raw.hex(), hashlib.sha256(raw).hexdigest()
            else:
                index = next(index for index, row in enumerate(changed["instructions"]) if row["offset"] == entry["offset"])
                changed["instructions"][index + 1]["offset"] += 1
            with self.assertRaisesRegex(ValueError, "token operand opcode or extent differs"):
                _profile_scanner_token_ranges(changed, self.heap)

    def test_same_code_and_rva_do_not_hide_changed_local_declarations(self):
        from host import _profile_scanner_compare
        original = self.selected("GetFallbackLaunchExeRank")
        changed = copy.deepcopy(original)
        changed["complete_il"] = re.sub(r"\.maxstack \d+", ".maxstack 99", changed["complete_il"])
        self.assertNotEqual(original["complete_il"], changed["complete_il"])
        result = _profile_scanner_compare(original, changed)
        self.assertTrue(result["original_code_identical"])
        self.assertFalse(result["complete_il_except_rva_identical"])
        self.assertEqual(result["classification"], "changed_selected_method")

    def test_changed_chain_cannot_drop_match_pass_or_forge_ranking_constant(self):
        from host import _profile_scanner_review_changes
        for mode in ("match", "rank"):
            changed = copy.deepcopy(self.receipt["assemblies"])
            current = next(row for row in changed if row["version"] == "3.15.0.1200"
                           and row["original"]["path"].endswith("/GameManagerServiceLib.dll"))
            if mode == "match":
                method = next(row for row in current["complete_methods"] if row["name"] == "FindMatchingUplayData")
                next(row for row in method["calls"] if row["resolved_method"] and row["resolved_method"]["name"] == "FirstOrDefault")["resolved_method"]["name"] = "Unrelated"
                expected = "matching priority"
            else:
                method = next(row for row in current["complete_methods"] if row["name"] == "GetFallbackLaunchExeRank")
                instruction = next(row for row in method["instructions"] if row["opcode"] == "ldc.i4.0")
                raw = bytearray.fromhex(method["raw_code_hex"])
                raw[instruction["offset"]] = 0x17
                method["raw_code_hex"] = raw.hex()
                expected = "ranking return"
            with self.assertRaisesRegex(ValueError, expected):
                _profile_scanner_review_changes(changed)

    def test_failures_stop_release_and_equivalence_limits_cannot_be_promoted(self):
        for group, name, value in (("equivalence", "whole_dll_or_product_equivalence", True),
                ("equivalence", "whole_IL_independently_decoded", True), ("equivalence", "changed_strings_masked_as_equivalent", True),
                ("uplay", "fallback_file_query_has_stop_token", True), ("uplay", "rollback_or_atomic_mapping_publish_added", True),
                ("uplay", "display_name_removed_marks", ["™", "®"]), ("riot", "argument_or_overflow", "empty string"),
                ("dependencies", "service_all_csharp_resolved", True)):
            changed = copy.deepcopy(self.receipt)
            changed["boundaries"][group][name] = value
            with self.assertRaisesRegex(ValueError, "failure, equivalence or lifecycle boundary differs"):
                self.verify(changed)
        self.assertEqual(len(self.receipt["assemblies"][-1]["capture_input"]["diagnostics"]), 10)

    def test_forged_review_stays_invalid_after_hash_recalculation(self):
        from host import _profile_scanner_projection
        changed = copy.deepcopy(self.receipt)
        changed["reviewed_changes"]["uplay"]["matching_passes"] = 1
        changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_profile_scanner_projection(changed),
            ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)
        changed = copy.deepcopy(self.receipt)
        changed["source_chain_complete"] = True
        with self.assertRaisesRegex(ValueError, "static evidence was promoted"):
            self.verify(changed)

    def test_complete_evidence_snapshot_and_strict_json_roundtrip(self):
        self.assertEqual(self.audit.final_snapshot(), [])
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(saved, self.receipt)
        self.assertEqual(self.verify(saved), self.receipt["verification"])


class ProfileStoreScannerVersionsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.receipt = host.collect_profile_store_scanner_versions(cls.audit)
        cls.current = next(row for row in cls.receipt["assemblies"] if row["version"] == "3.15.0.1200"
                           and row["original"]["path"].endswith("/GameManagerServiceLib.dll"))

    def verify(self, receipt):
        from host import _verify_profile_store_scanner_versions
        return _verify_profile_store_scanner_versions(receipt)

    def method(self, root, name, assemblies=None):
        rows = [row for assembly in assemblies or [self.current] if assembly["version"] == "3.15.0.1200"
                for row in assembly["complete_methods"] if row["class_ancestry"] == ["Razer.GameManagerServiceLib." + root]
                and row["name"] == name]
        self.assertEqual(len(rows), 1)
        return rows[0]

    def test_actual_registration_inventory_keeps_remaining_eleven_unreviewed(self):
        receipt = self.receipt
        self.assertEqual(receipt["host_release"]["version"], "4.0.896")
        self.assertEqual(receipt["host_release"]["official_gms_package"]["declaration"]["resourceVersion"], "3.15.0.1200")
        rows = receipt["registration"]["registrations"]
        self.assertEqual(len(rows), 22)
        self.assertEqual([row["strategy"] for row in rows if row["review_scope"] == "this_store_group"],
            ["GameScannerStrategyBattleNetSearchV2", "GameScannerStrategyEpicLauncherInstalledList",
             "GameScannerStrategySteamLibrarySearch", "GameScannerStrategySteamShortcutsSearch"])
        self.assertEqual(sum(row["review_scope"] == "previous_seven_strategy_group" for row in rows), 7)
        self.assertEqual(sum(row["review_scope"] == "remaining_current_strategy" for row in rows), 11)
        self.assertEqual([row["strategy"] for row in rows if row["condition"] != "unconditional"], ["GameScannerStrategyUwpSearch"])
        self.assertTrue(all(row["resolved_method"]["name"] == "AddScoped" for row in rows))
        self.assertFalse(receipt["registration"]["capture_input"]["all_csharp_resolved"])
        self.assertEqual(len(receipt["registration"]["capture_input"]["diagnostics"]), 10)

    def test_selected_methods_and_raw_classifications_do_not_count_feature_deletions(self):
        receipt = self.receipt
        self.assertEqual(sum(len(row["complete_methods"]) for row in receipt["assemblies"]), 613)
        self.assertEqual(len(receipt["comparisons"]), 318)
        self.assertEqual(receipt["verification"]["comparison_classes"], {
            "same_selected_method_after_metadata_relocation": 286, "changed_selected_method": 9,
            "added_selected_method": 20, "removed_selected_method": 3})
        self.assertEqual(receipt["reviewed_changes"]["steam"]["generated_owner_rename"]["current"], "<ReadAsync>d__14")
        self.assertFalse(receipt["boundaries"]["steam"]["old_readasync_state_machine_rename_is_feature_removal"])
        self.assertFalse(any("GogRegistrySearch" in method["class_ancestry"][0]
            for assembly in receipt["assemblies"] for method in assembly["complete_methods"]))
        self.assertFalse(any(method["name"].startswith("ExtractIcon")
            for assembly in receipt["assemblies"] for method in assembly["complete_methods"]))

    def test_unchanged_readers_still_require_bytes_declarations_calls_and_strings(self):
        same = [row for row in self.receipt["comparisons"] if row["classification"] == "same_selected_method_after_metadata_relocation"]
        for row in same:
            for key in ("instructions_identical", "exception_ranges_identical", "original_user_strings_identical",
                        "resolved_call_targets_identical", "non_token_bytes_identical", "complete_il_except_rva_identical"):
                self.assertTrue(row[key], (row["key"], key))
        sources = {row["class"]: row for row in self.receipt["source_comparisons"]}
        for name in ("GameScannerStrategySteamLibrarySearch", "GameScannerStrategySteamShortcutsSearch",
                "BattleNetAgentLogParser", "EpicLauncherInstalledGamesReader", "EpicGameManifest", "SteamHelper"):
            self.assertTrue(sources[name]["complete_source_bytes_equal"])
        self.assertFalse(sources["SteamAppInfoData"]["complete_source_bytes_equal"])
        self.assertFalse(sources["GameScannerStrategyEpicLauncherInstalledList"]["complete_source_bytes_equal"])

    def test_steam_recursive_original_strings_and_real_exception_boundary(self):
        changes = self.receipt["reviewed_changes"]["steam"]
        self.assertTrue(changes["old_get_launch_exception_sections"])
        self.assertEqual(changes["current_get_launch_exception_sections"], [])
        self.assertEqual([clause["flags"] for section in changes["recursive_exception_sections"] for clause in section["clauses"]], [2])
        method = self.method("SteamAppInfoData", "FindLaunchExe")
        self.assertEqual([row["original_user_string"]["value"] for row in method["token_operands"] if "original_user_string" in row],
            ["executable", ".exe"])
        self.assertEqual(bytes.fromhex(method["raw_code_hex"])[0x21], 0x1b)
        self.assertFalse(self.receipt["boundaries"]["steam"]["numeric_slot_limit"])

    def test_epic_null_synthetic_launch_property_is_rejected_before_fallback(self):
        changes = self.receipt["reviewed_changes"]["epic"]
        self.assertNotIn("set_LaunchExecutable", changes["converter_setters"])
        self.assertEqual(changes["empty_launch_branch"], {"offset": 0x22f, "bytes": "3ab9080000", "target": "IL_0aed"})
        self.assertEqual(changes["matching_passes"], 3)
        self.assertEqual(changes["original_rank_returns"], [0, 1, 2, 3])
        self.assertEqual(len(changes["invariant_identity_methods"]), 4)
        getter = self.method("EpicGameManifest", "get_LaunchExecutable")
        self.assertEqual([row["opcode"] for row in getter["instructions"]], ["ldarg.0", "ldfld", "ret"])
        self.assertEqual(self.method("EpicGameManifest", ".ctor")["calls"][0]["resolved_method"]["declaring_type"], "System.Object")
        self.assertEqual(changes["fallback_exception_sections"], [])

    def test_battle_net_frozen_set_retains_original_constants_and_comparer(self):
        changes = self.receipt["reviewed_changes"]["battle_net"]
        self.assertEqual(changes["blacklist_literals"], ["bna", "agent", "clnt"])
        self.assertEqual(changes["frozen_set_call"]["resolved_method"]["name"], "ToFrozenSet")
        self.assertEqual(self.receipt["boundaries"]["battle_net"]["blacklist_comparer"], "CurrentCultureIgnoreCase")
        self.assertFalse(self.receipt["boundaries"]["battle_net"]["constructor_allocation_or_framework_success_proven"])

    def test_forged_epic_producer_and_gate_fail_before_completion(self):
        from host import _profile_store_review_changes
        for mode in ("producer", "getter", "branch"):
            assemblies = copy.deepcopy(self.receipt["assemblies"])
            if mode == "producer":
                method = self.method("GameScannerStrategyEpicLauncherInstalledList", "CreateEpicManifestFromInstalledGame", assemblies)
                next(row for row in method["calls"] if row["resolved_method"]["name"] == "set_AppName")["resolved_method"]["name"] = "set_LaunchExecutable"
                message = "producer differs"
            elif mode == "getter":
                self.method("EpicGameManifest", "get_LaunchExecutable", assemblies)["instructions"][1]["opcode"] = "ldstr"
                message = "producer differs"
            else:
                method = next(row for assembly in assemblies if assembly["version"] == "3.15.0.1200"
                    for row in assembly["complete_methods"] if row["class_ancestry"][0].endswith(".GameScannerStrategyEpicLauncherInstalledList")
                    and row["name"] == "MoveNext")
                code = bytearray.fromhex(method["raw_code_hex"])
                code[0x22f] = 0x39
                method["raw_code_hex"] = code.hex()
                message = "original branch differs"
            with self.assertRaisesRegex(ValueError, message):
                _profile_store_review_changes(assemblies)

    def test_steam_catch_matching_pass_and_rank_mutations_are_rejected(self):
        from host import _profile_store_review_changes
        for mode in ("catch", "matching", "rank"):
            assemblies = copy.deepcopy(self.receipt["assemblies"])
            if mode == "catch":
                self.method("SteamAppInfoData", "GetLaunchExe", assemblies)["exception_sections"] = [{"clauses": []}]
                message = "exception boundary differs"
            elif mode == "matching":
                method = self.method("GameScannerStrategyEpicLauncherInstalledList", "FindMatchingInstalledGame", assemblies)
                next(row for row in method["calls"] if row["resolved_method"]["name"] == "FirstOrDefault")["resolved_method"]["name"] = "Unrelated"
                message = "actual changed call differs"
            else:
                method = self.method("GameScannerStrategyEpicLauncherInstalledList", "GetEpicFallbackLaunchExeRank", assemblies)
                code = bytearray.fromhex(method["raw_code_hex"])
                code[0x38] = 0x17
                method["raw_code_hex"] = code.hex()
                message = "ranking return differs"
            with self.assertRaisesRegex(ValueError, message):
                _profile_store_review_changes(assemblies)

    def test_registration_cancellation_and_device_success_cannot_be_promoted(self):
        for group, field, value in (("registration", "remaining_promoted", True), ("registration", "this_group", 22),
                ("steam", "get_launch_catch_removed", False), ("epic", "launch_executable_backfilled", True),
                ("epic", "launch_nonempty_gate_precedes_mgl_and_fallback", False), ("epic", "fallback_catch_or_cancel_added", True),
                ("epic", "launch_command_consumed", True), ("dependencies", "service_diagnostics_retained", 0)):
            changed = copy.deepcopy(self.receipt)
            changed["boundaries"][group][field] = value
            with self.assertRaisesRegex(ValueError, "registration or lifecycle boundary differs"):
                self.verify(changed)
        changed = copy.deepcopy(self.receipt)
        changed["source_chain_complete"] = True
        with self.assertRaisesRegex(ValueError, "static evidence was promoted"):
            self.verify(changed)

    def test_recalculated_digest_cannot_hide_dropped_registration_or_field_chain(self):
        from host import _profile_store_projection
        for field in ("registration", "producer"):
            changed = copy.deepcopy(self.receipt)
            if field == "registration":
                changed["registration"]["registrations"].pop()
            else:
                changed["reviewed_changes"]["epic"]["converter_setters"].append("set_LaunchExecutable")
            changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_profile_store_projection(changed),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_current_snapshot_json_roundtrip_and_no_temporary_resources(self):
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["analysis_sessions_created"], [])
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(saved), self.receipt["verification"])


class ProfileLocalScannerVersionsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.receipt = host.collect_profile_local_scanner_versions(cls.audit)

    def verify(self, receipt):
        from host import _verify_profile_local_scanner_versions
        return _verify_profile_local_scanner_versions(receipt)

    def method(self, root, name, assemblies=None, version="3.15.0.1200"):
        rows = [row for assembly in assemblies or self.receipt["assemblies"] if assembly["version"] == version
            for row in assembly["complete_methods"] if row["class_ancestry"][0].split(".")[-1] == root and row["name"] == name
            and (len(row["class_ancestry"]) == 1 if name != "MoveNext" else any("<ScanAsync>" in owner for owner in row["class_ancestry"][1:]))]
        self.assertEqual(len(rows), 1)
        return rows[0]

    def test_actual_registration_keeps_four_independent_readers_unreviewed(self):
        rows = self.receipt["registration"]["registrations"]
        self.assertEqual(self.receipt["host_release"]["version"], "4.0.896")
        self.assertEqual(len(rows), 22)
        self.assertEqual(sum(row["review_scope"] == "previously_reviewed_group" for row in rows), 11)
        self.assertEqual(sum(row["review_scope"] == "this_local_group" for row in rows), 7)
        self.assertEqual([row["strategy"] for row in rows if row["review_scope"] == "remaining_current_strategy"],
            ["GameScannerStrategyFeverPlatformSearch", "GameScannerStrategyItchIoSearch", "GameScannerStrategyTwitchSearch", "GameScannerStrategyUwpSearch"])
        self.assertEqual([row["strategy"] for row in rows if row["condition"] != "unconditional"], ["GameScannerStrategyUwpSearch"])
        self.assertEqual(len(self.receipt["registration"]["capture_input"]["diagnostics"]), 10)
        self.assertFalse(self.receipt["registration"]["capture_input"]["all_csharp_resolved"])

    def test_complete_selected_methods_include_gameux_model_and_only_one_change(self):
        receipt = self.receipt
        self.assertEqual(sum(len(row["complete_methods"]) for row in receipt["assemblies"]), 248)
        self.assertEqual(len(receipt["comparisons"]), 124)
        self.assertEqual(receipt["verification"]["comparison_classes"], {
            "same_selected_method_after_metadata_relocation": 123, "changed_selected_method": 1})
        changed, = [row for row in receipt["comparisons"] if row["classification"] == "changed_selected_method"]
        self.assertIn("RegistryInspector|", changed["key"])
        self.assertIn("SearchForRegistryKey", changed["key"])
        model = [method for assembly in receipt["assemblies"] if assembly["version"] == "3.15.0.1200"
            for method in assembly["complete_methods"] if len(method["class_ancestry"]) == 2
            and method["class_ancestry"][1] == "GameUXInfo"]
        self.assertEqual({row["name"] for row in model}, {".ctor"} | {prefix + name for name in (
            "AppExePath", "ConfigGdfBinaryPath", "EntryGuid", "GameGuid", "Manufacturer", "RegistryKey", "RegistryPathKeyName", "Title")
            for prefix in ("get_", "set_")})
        self.assertFalse(any(method["name"] == "GetAllPossibleFilenames" for assembly in receipt["assemblies"]
            for method in assembly["complete_methods"]))

    def test_unchanged_strategy_text_does_not_replace_complete_il_and_original_checks(self):
        sources = {row["class"]: row for row in self.receipt["source_comparisons"]}
        for name in ("DefaultExeSearch", "DefaultRegistryKey", "GameUXInfoSearch", "GameUXRegistrySearch",
                "RegistrySearch", "UninstallSearch", "UserFolderExeSearch"):
            self.assertTrue(sources["GameScannerStrategy" + name]["complete_source_bytes_equal"])
        self.assertFalse(sources["RegistryInspector"]["complete_source_bytes_equal"])
        self.assertFalse(sources["GameSignatureHelper"]["complete_source_bytes_equal"])
        for row in self.receipt["comparisons"]:
            if row["classification"] == "same_selected_method_after_metadata_relocation":
                self.assertTrue(all(row[key] for key in ("instructions_identical", "exception_ranges_identical",
                    "original_user_strings_identical", "resolved_call_targets_identical", "non_token_bytes_identical",
                    "complete_il_except_rva_identical")))

    def test_identical_raw_contains_tokens_have_different_actual_targets(self):
        changes = self.receipt["reviewed_changes"]["registry_contains"]
        self.assertEqual([row["offset"] for row in changes["old"]["calls"]], [0x4c, 0x7c])
        self.assertEqual([row["offset"] for row in changes["current"]["calls"]], [0x51, 0x86])
        for side, owner in (("old", "System.Linq.Enumerable"), ("current", "System.MemoryExtensions")):
            self.assertEqual(changes[side]["exception_sections"], [])
            for call in changes[side]["calls"]:
                self.assertEqual(call["bytes"], "281c00002b")
                self.assertEqual(call["resolved_method"]["declaring_type"], owner)
                self.assertEqual(call["resolved_method"]["generic_signature_hex"], "0a010e")
        self.assertEqual([row["offset"] for row in changes["current"]["span_conversions"]], [0x46, 0x7b])
        self.assertFalse(self.receipt["boundaries"]["registry_inspector"]["framework_runtime_equivalence_proven"])
        self.assertFalse(self.receipt["boundaries"]["registry_inspector"]["span_conversion_typespec_independently_decoded"])

    def test_original_discarded_key_candidate_property_and_preload_order_are_preserved(self):
        changes = self.receipt["reviewed_changes"]
        self.assertEqual(changes["explicit_registry32"]["raw_pop_and_reload"], "260208110d")
        self.assertEqual(changes["explicit_registry32"]["reused_default_key_local"], 13)
        self.assertEqual([row["candidate_property"] for row in changes["registry_default_exe64_branches"]], ["DefaultExe32"] * 2)
        self.assertEqual(changes["gameux_preloads"]["offsets"], [0x40, 0x47, 0x4f, 0x57])
        self.assertEqual(changes["gameux_preloads"]["first_scan_loop_stop_offset"], 0x7a)
        self.assertEqual(self.receipt["boundaries"]["paths"]["default_drive"], "C:\\")
        self.assertFalse(self.receipt["boundaries"]["paths"]["new_cancel_or_stale_generation_guard"])

    def test_wrong_target_raw_call_string_argument_and_exception_are_rejected(self):
        from host import _profile_local_review_changes
        for mode in ("owner", "raw", "string", "catch"):
            assemblies = copy.deepcopy(self.receipt["assemblies"])
            method = self.method("RegistryInspector", "SearchForRegistryKey", assemblies)
            call = next(row for row in method["calls"] if row["offset"] == 0x51)
            if mode == "owner":
                call["resolved_method"]["declaring_type"] = "System.Linq.Enumerable"
                message = "actual call differs"
            elif mode == "raw":
                raw = bytearray.fromhex(method["raw_code_hex"])
                raw[0x51] = 0x6f
                method["raw_code_hex"] = raw.hex()
                message = "original call bytes differ"
            elif mode == "string":
                call["resolved_method"]["generic_signature_hex"] = "0a0108"
                message = "string argument or exception boundary differs"
            else:
                method["exception_sections"] = [{"clauses": []}]
                message = "string argument or exception boundary differs"
            with self.assertRaisesRegex(ValueError, message):
                _profile_local_review_changes(assemblies)

    def test_replacing_discard_or_claiming_exe64_candidate_is_rejected(self):
        from host import _profile_local_review_changes
        for mode in ("key", "candidate"):
            assemblies = copy.deepcopy(self.receipt["assemblies"])
            if mode == "key":
                method = self.method("RegistryInspector", "SearchRegistryKeyPath", assemblies)
                raw = bytearray.fromhex(method["raw_code_hex"])
                raw[0x180] = 0x0a
                method["raw_code_hex"] = raw.hex()
                message = "Registry32 result or wrapper catch differs"
            else:
                method = self.method("GameScannerStrategyDefaultRegistryKey", "MoveNext", assemblies)
                next(row for row in method["calls"] if row["offset"] == 0x30d)["resolved_method"]["name"] = "get_DefaultExe64"
                message = "actual call differs"
            with self.assertRaisesRegex(ValueError, message):
                _profile_local_review_changes(assemblies)

    def test_missing_readers_and_static_results_cannot_be_promoted(self):
        for group, field, value in (("registration", "remaining_promoted", True), ("registration", "remaining_current_group", 0),
                ("registry_inspector", "framework_runtime_equivalence_proven", True),
                ("registry_inspector", "registry_key_release_added", True), ("gameux", "clear_on_normal_tail_only", False),
                ("paths", "new_cancel_or_stale_generation_guard", True), ("dependencies", "service_diagnostics_retained", 0)):
            changed = copy.deepcopy(self.receipt)
            changed["boundaries"][group][field] = value
            with self.assertRaisesRegex(ValueError, "failure or lifecycle boundary differs"):
                self.verify(changed)
        changed = copy.deepcopy(self.receipt)
        changed["source_chain_complete"] = True
        with self.assertRaisesRegex(ValueError, "static evidence was promoted"):
            self.verify(changed)

    def test_recomputed_digest_cannot_hide_removed_registration_or_actual_target(self):
        from host import _profile_local_projection
        for mode in ("registration", "target"):
            changed = copy.deepcopy(self.receipt)
            if mode == "registration":
                changed["registration"]["registrations"].pop()
            else:
                changed["reviewed_changes"]["registry_contains"]["current"]["calls"][0]["resolved_method"]["declaring_type"] = "System.Linq.Enumerable"
            changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_profile_local_projection(changed),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_current_snapshot_json_roundtrip_and_no_temporary_resources(self):
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["analysis_sessions_created"], [])
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(saved), self.receipt["verification"])


class ProfileSharedHelperConsumersTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.host = host
        cls.audit = HostLocalAudit()
        cls.receipt = host.collect_profile_shared_helper_consumers(cls.audit)

    def verify(self, receipt):
        return self.host._verify_profile_shared_helper_consumers(receipt)

    def method(self, root, name, *, version="3.15.0.1200", assemblies=None):
        rows = [row for assembly in assemblies or self.receipt["assemblies"] if assembly["version"] == version
            for row in assembly["complete_methods"] if row["class_ancestry"][0].split(".")[-1] == root and row["name"] == name
            and len(row["class_ancestry"]) == 1]
        self.assertTrue(rows)
        return max(rows, key=lambda row: row["code_size"])

    def source(self, name):
        row, = [row for assembly in self.receipt["assemblies"] if assembly["version"] == "3.15.0.1200"
                for row in assembly["complete_source_owners"] if row["class"] == name]
        return row["complete_source"].replace("\r\n", "\n")

    def test_current_package_and_complete_helpers_do_not_reanalyse_registered_strategies(self):
        receipt = self.receipt
        self.assertEqual(receipt["host_release"]["version"], "4.0.896")
        self.assertEqual(receipt["verification"]["complete_managed_methods"], 214)
        self.assertEqual(receipt["verification"]["comparison_classes"],
            {"same_selected_method_after_metadata_relocation": 106, "changed_selected_method": 1})
        self.assertFalse(receipt["verification"]["registered_strategy_reanalysis"])
        self.assertEqual([(row["version"], len(row["complete_methods"])) for row in receipt["assemblies"]],
            [("3.14.0.1109", 82), ("3.14.0.1109", 25), ("3.15.0.1200", 82), ("3.15.0.1200", 25)])
        self.assertFalse(any("GameScannerStrategy" in row["declaring_type"]
            for assembly in receipt["assemblies"] for row in assembly["complete_methods"]))

    def test_only_filename_membership_changed_and_original_tokens_are_not_semantic_identity(self):
        changed, = [row for row in self.receipt["comparisons"] if row["classification"] == "changed_selected_method"]
        self.assertIn("GetAllPossibleFilenames", changed["key"])
        proof = self.receipt["reviewed_chains"]["filenames"]
        self.assertEqual((proof["old_method"]["code_size"], proof["code_size"]), (577, 582))
        self.assertEqual(proof["old_contains"]["resolved_method"]["declaring_type"], "System.Linq.Enumerable")
        self.assertEqual(proof["contains"]["resolved_method"]["declaring_type"], "System.MemoryExtensions")
        self.assertEqual(proof["old_contains"]["bytes"], proof["contains"]["bytes"])
        self.assertEqual(proof["contains"]["resolved_method"]["generic_signature_hex"], "0a010e")
        self.assertLess(proof["span_conversion"]["offset"], proof["contains"]["offset"])
        self.assertFalse(changed["exception_ranges_identical"])
        self.assertFalse(self.receipt["boundaries"]["filenames"]["framework_behavior_equivalence_executed"])

    def test_uwp_package_preload_classification_and_partial_result_are_kept_separate(self):
        proof = self.receipt["reviewed_chains"]["installed"]
        self.assertEqual(proof["preload_calls"], {"FindPackagesForUser": [12], "GetCategoryCache": [24],
            "GetEntitlementsCache": [39, 109], "ReadManifest": [196], "FileExists": [270], "Exists": [540]})
        self.assertEqual([c["flags"] for s in proof["exception_sections"] for c in s["clauses"]], [2, 0, 0, 2, 0])
        boundary = self.receipt["boundaries"]["uwp_list"]
        for key in ("only_games_returned", "package_dedup_or_architecture_filter", "stop_or_generation_check",
                "impersonator_dispose_proves_handle_close"):
            self.assertFalse(boundary[key])
        source = self.source("UwpHelper")
        self.assertIn("using (new Impersonator(-1))", source)
        self.assertIn('uwpAppInfo.ProductKind = "Game";', source)
        for json_name in ("displayName", "exeFileName", "familyName", "fullName", "installedLocation", "logoPath",
                "name", "productKind", "publisherDisplayName", "publisherId"):
            self.assertIn('[JsonPropertyName("' + json_name + '")]', self.source("UwpAppInfo"))

    def test_full_name_uses_os_architecture_then_neutral_and_retains_failed_previous_state(self):
        rows = [row for assembly in self.receipt["assemblies"] if assembly["version"] == "3.15.0.1200"
            for row in assembly["complete_methods"] if row["class_ancestry"][0].split(".")[-1] == "UwpHelper"
            and "<GetUwpFullName>" in row["name"]]
        self.assertEqual(len(rows), 3)
        self.assertEqual([bytes.fromhex(row["raw_code_hex"])[11:-1].hex() for row in rows], ["1f09fe01", "16fe01", "1f0bfe01"])
        method = self.method("UwpHelper", "GetUwpFullName")
        calls = [row["resolved_method"]["name"] for row in method["calls"] if row["resolved_method"]]
        self.assertIn("FindPackages", calls)
        self.assertNotIn("FindPackagesForUser", calls)
        self.assertEqual(calls.count("set_ProcessName"), 2)
        self.assertIn("null signature can throw again", self.receipt["boundaries"]["uwp_full_name"]["no_match_or_error"])
        self.assertEqual(self.method("UwpHelperUnsupported", "GetUwpFullName")["raw_code_hex"], "2a")
        self.assertFalse(self.receipt["boundaries"]["unsupported"]["product_support_claim"])

    def test_filename_field_order_exclusion_and_final_comparer_are_not_conflated(self):
        proof = self.receipt["reviewed_chains"]["filenames"]
        self.assertEqual(proof["ordered_getters"], ["DefaultExe", "DefaultExe32", "DefaultExe64", "AlternativeExe",
            "DefaultFolderExe", "DefaultFolderExe32", "LaunchHelper", "LaunchHelperExcludables", "RunningExe"])
        boundary = self.receipt["boundaries"]["filenames"]
        self.assertFalse(boundary["launch_exe_directly_included"])
        self.assertFalse(boundary["file_exists_required"])
        source = self.source("GameSignatureHelper")
        self.assertIn("return exe.Split('|');", source)
        self.assertIn("((ReadOnlySpan<string>)components3).Contains(text4)", source)
        self.assertIn("return list.Distinct<string>(StringComparer.CurrentCultureIgnoreCase).ToList();", source)

    def test_every_retained_interface_call_has_an_actual_complete_consumer(self):
        calls = self.receipt["actual_helper_calls"]
        self.assertEqual(len(calls), 18)
        for version in ("3.14.0.1109", "3.15.0.1200"):
            selected = [row for row in calls if row["version"] == version]
            self.assertEqual([row["resolved_method"]["name"] for row in selected].count("GetAllPossibleFilenames"), 5)
            self.assertEqual([row["resolved_method"]["name"] for row in selected].count("GetInstalledUwpApps"), 2)
            self.assertEqual([row["resolved_method"]["name"] for row in selected].count("GetUwpFullName"), 2)
        assembly = next(row for row in self.receipt["assemblies"] if row["version"] == "3.15.0.1200"
            and row["original"]["path"].endswith("/GameManagerServiceLib.dll"))
        il = self.audit.artifact(self.audit.a.ROOT / assembly["il_artifact"]["path"], assembly["il_artifact"]).decode("utf-8-sig")
        methods = [row for row in assembly["complete_methods"] if row["name"] != "GetRunningProcesses"]
        with self.assertRaisesRegex(ValueError, "direct caller is absent or ambiguous"):
            self.host._profile_shared_helper_call_inventory("3.15.0.1200", assembly["original"], il, methods)

    def test_both_add_variants_fill_after_settings_and_do_not_convert_lookup_into_success(self):
        rows = self.receipt["reviewed_chains"]["manual_add_order"]
        self.assertEqual([(row["set_settings_offset"], row["full_name_offset"], row["notify_offset"]) for row in rows],
            [(3511, 3556, 4218), (3539, 3584, 4297)])
        boundary = self.receipt["boundaries"]["consumers"]
        self.assertFalse(boundary["full_name_failure_prevents_add"])
        self.assertFalse(boundary["serialization_failure_rolls_back_mutations"])
        self.assertTrue(boundary["serialization_failure_can_still_notify"])
        self.assertFalse(boundary["outer_add_catch_always_changes_initial_success"])

    def test_null_client_yield_does_not_terminate_or_cancel_the_actual_hub_task(self):
        source = self.source("GameManagerServiceHub").split("public async IAsyncEnumerable<List<UwpAppInfo>> GetUwpApps()", 1)[1].split("public string GetWorkingDirectory", 1)[0]
        self.assertEqual(source.count("yield return"), 2)
        self.assertNotIn("yield break", source)
        self.assertIn("Task.Run(() => gameListAndSettingsManager.GetUwpApps())", source)
        proof = self.receipt["reviewed_chains"]["hub"]
        self.assertEqual((proof["code_size"], proof["dispose_async"]["code_size"]), (398, 86))
        self.assertFalse(self.receipt["boundaries"]["consumers"]["hub_task_has_cancel_token"])

    def test_cache_blacklist_and_running_process_lifetimes_keep_partial_mutations(self):
        source = self.source("GameProcessManager")
        self.assertIn("filenameCache = new ConcurrentDictionary<string, HashSet<Guid>>(StringComparer.CurrentCultureIgnoreCase)", source)
        self.assertIn('blackListedExes.ContainsKey(allPossibleFilename + ".exe")', source)
        self.assertIn("value.Add(value3.EntryGUID);\n\t\t\t\t\t\t\tcontinue;", source)
        cache = self.method("GameProcessManager", "CacheFileNames")
        calls = [row["resolved_method"]["name"] for row in cache["calls"] if row["resolved_method"]]
        self.assertNotIn("Clear", calls)
        self.assertEqual([c["flags"] for s in cache["exception_sections"] for c in s["clauses"]], [2, 2, 2, 0, 2])
        self.assertFalse(self.receipt["boundaries"]["consumers"]["process_objects_explicitly_disposed"])
        self.assertFalse(self.receipt["boundaries"]["consumers"]["partial_cache_rollback_on_error"])

    def test_raw_calls_missing_finally_and_invented_cache_clear_are_rejected(self):
        for mode in ("raw", "finally", "cache"):
            with self.subTest(mode=mode):
                assemblies = copy.deepcopy(self.receipt["assemblies"])
                if mode == "raw":
                    method = self.method("UwpHelper", "GetInstalledUwpApps", assemblies=assemblies)
                    call = next(row for row in method["calls"] if (row["resolved_method"] or {}).get("name") == "FindPackagesForUser")
                    raw = bytearray.fromhex(method["raw_code_hex"])
                    raw[call["offset"]] = 0x28
                    method["raw_code_hex"] = raw.hex()
                    expected = "original call bytes differ"
                elif mode == "finally":
                    self.method("UwpHelper", "GetInstalledUwpApps", assemblies=assemblies)["exception_sections"] = []
                    expected = "catch/finally boundary differs"
                else:
                    self.method("GameProcessManager", "CacheFileNames", assemblies=assemblies)["calls"].append(
                        {"resolved_method": {"name": "Clear"}})
                    expected = "additive cache was changed"
                with self.assertRaisesRegex(ValueError, expected):
                    self.host._profile_shared_helper_review(assemblies)

    def test_forged_semantics_stage_promotion_and_method_changes_fail_but_strict_roundtrip_passes(self):
        for mutate, expected in (
                (lambda r: r["boundaries"]["consumers"].update(null_client_terminates_enumeration=True), "consumer, failure or lifecycle"),
                (lambda r: r.update(actual_dll_comparison="passed"), "static evidence was promoted"),
                (lambda r: r["actual_helper_calls"].pop(), "selected bytes require complete semantic review"),
                (lambda r: r["assemblies"][0]["complete_methods"][0].update(raw_code_hex="2a"), "selected bytes require complete semantic review")):
            changed = copy.deepcopy(self.receipt)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, expected):
                self.verify(changed)
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["analysis_sessions_created"], [])
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(saved), self.receipt["verification"])


class ProfileRemainingScannerVersionsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.receipt = host.collect_profile_remaining_scanner_versions(cls.audit)

    def verify(self, receipt):
        from host import _verify_profile_remaining_scanner_versions
        return _verify_profile_remaining_scanner_versions(receipt)

    def method(self, root, name, assemblies=None):
        rows = [row for assembly in assemblies or self.receipt["assemblies"] if assembly["version"] == "3.15.0.1200"
            for row in assembly["complete_methods"] if row["class_ancestry"][0].split(".")[-1] == root and row["name"] == name
            and (len(row["class_ancestry"]) == 1 if name != "MoveNext" else any("<ScanAsync>" in owner for owner in row["class_ancestry"][1:]))]
        self.assertEqual(len(rows), 1)
        return rows[0]

    def test_actual_registration_reviews_four_and_preserves_both_os_gates(self):
        receipt = self.receipt
        self.assertEqual(receipt["host_release"]["version"], "4.0.896")
        rows = receipt["registration"]["registrations"]
        self.assertEqual(len(rows), 22)
        self.assertEqual(sum(row["review_scope"] == "previously_reviewed_group" for row in rows), 18)
        self.assertEqual([row["strategy"] for row in rows if row["review_scope"] == "this_remaining_group"],
            ["GameScannerStrategyFeverPlatformSearch", "GameScannerStrategyItchIoSearch", "GameScannerStrategyTwitchSearch", "GameScannerStrategyUwpSearch"])
        self.assertEqual([row["original_bytes"] for row in receipt["registration"]["os_major_gates"]], ["1c3209", "1c3207"])
        bindings = receipt["registration"]["actual_bindings"]
        self.assertEqual([row["offset"] for row in bindings], [0x1b0, 0x1b7, 0x208, 0x211, 0x2cd])
        self.assertEqual(bindings[3]["resolved_method"]["generic_type_arguments"][-1], "Razer.GameManagerServiceLib.UwpHelperUnsupported")
        self.assertEqual(len(receipt["registration"]["capture_input"]["diagnostics"]), 10)
        self.assertFalse(receipt["source_chain_complete"])
        self.assertFalse(receipt["boundaries"]["registration"]["whole_scanner_semantics_complete"])

    def test_complete_methods_and_declared_imports_are_compared_without_execution(self):
        receipt = self.receipt
        self.assertEqual(receipt["verification"]["complete_managed_methods"], 288)
        self.assertEqual(receipt["verification"]["comparison_classes"], {"same_selected_method_after_metadata_relocation": 144})
        self.assertEqual(receipt["verification"]["selected_declarations"], 20)
        self.assertEqual(len(receipt["declaration_comparisons"]), 10)
        self.assertTrue(all(row["complete_il_equal"] and not row["runtime_abi_verified"] for row in receipt["declaration_comparisons"]))
        self.assertEqual(len(receipt["source_comparisons"]), 24)
        self.assertTrue(all(row["complete_source_bytes_equal"] for row in receipt["source_comparisons"]))
        for row in receipt["comparisons"]:
            self.assertTrue(all(row[key] for key in ("instructions_identical", "exception_ranges_identical",
                "original_user_strings_identical", "resolved_call_targets_identical", "non_token_bytes_identical", "complete_il_except_rva_identical")))
        self.assertFalse(any(method["name"] in ("GetInstalledUwpApps", "GetUwpFullName", "GetAllPossibleFilenames")
            for assembly in receipt["assemblies"] for method in assembly["complete_methods"]))

    def test_fever_constructs_but_has_no_publication_stop_or_registry_release(self):
        fever = self.receipt["reviewed_chains"]["fever"]
        self.assertEqual(fever["code_size"], 446)
        self.assertNotIn("AddFoundGame", fever["actual_call_names"])
        self.assertNotIn("GetStopScanFlag", fever["actual_call_names"])
        self.assertNotIn("Dispose", fever["actual_call_names"])
        self.assertEqual(fever["actual_call_names"].count("GetValue"), 3)
        self.assertFalse(self.receipt["boundaries"]["fever"]["constructed_signature_published"])
        self.assertFalse(self.receipt["boundaries"]["fever"]["startup_path_consumed"])

    def test_database_preload_identity_side_effects_and_real_finally_are_preserved(self):
        for reader in self.receipt["reviewed_chains"]["sqlite_readers"]:
            self.assertEqual(reader["get_offset"], 0x34)
            self.assertEqual(reader["stop_offset"], 0x51)
            self.assertEqual(reader["mapping_offset"], 0xc4)
            self.assertEqual(reader["guid_parse_offset"], 0xdd)
            self.assertLess(reader["guid_parse_offset"], reader["publication_offset"])
            flags = [c["flags"] for s in reader["reader_exception_sections"] for c in s["clauses"]]
            self.assertGreaterEqual(flags.count(2), 3)
        self.assertFalse(self.receipt["boundaries"]["sqlite"]["reader_explicit_dispose"])
        self.assertFalse(self.receipt["boundaries"]["sqlite"]["normal_tail_clear_is_finally"])
        self.assertEqual(self.receipt["boundaries"]["itch"]["signature_id"], "caves.id; not games.id or caves.game_id")

    def test_compiler_path_helpers_have_complete_il_while_csharp_output_is_empty(self):
        owners = [row for assembly in self.receipt["assemblies"] for row in assembly["complete_source_owners"]
                  if row["class"] == "<PrivateImplementationDetails>"]
        self.assertEqual(len(owners), 2)
        self.assertTrue(all(row["artifact"]["bytes"] == 0 and row["complete_source"] == "" for row in owners))
        for name in ("InlineArrayElementRef", "InlineArrayAsReadOnlySpan"):
            method = self.method("<PrivateImplementationDetails>", name)
            self.assertTrue(method["complete_il"])
            self.assertGreater(method["code_size"], 0)
        calls = self.method("TwitchGamesDataReader", "GetInstalledGames")["calls"]
        self.assertEqual(sum((row["resolved_method"] or {}).get("name") == "InlineArrayElementRef" for row in calls), 5)

    def test_uwp_classification_precedes_stop_and_resource_claims_remain_separate(self):
        uwp = self.receipt["reviewed_chains"]["uwp"]
        self.assertEqual(uwp["preload_offsets"], [0xda, 0x111, 0x126, 0x133])
        self.assertEqual(uwp["stop_offsets"], [0x30d, 0x63e])
        self.assertEqual(uwp["publication_offsets"], [0x57b, 0x9b9])
        self.assertEqual(uwp["xbox_local_exception_sections"], [])
        boundary = self.receipt["boundaries"]["uwp"]
        self.assertTrue(boundary["known_game_requires_default_folder_exe_wildcard"])
        self.assertTrue(boundary["process_name_set_after_add_found_game"])
        self.assertFalse(boundary["installed_location_null_allows_installedpath_fallback"])
        self.assertFalse(boundary["config_executable_requires_exists"])
        self.assertFalse(boundary["classification_has_stop_check"])

    def test_impersonator_revert_result_is_discarded_without_close_handle(self):
        method = self.method("Impersonator", "Dispose")
        self.assertEqual([row["opcode"] for row in method["instructions"]], ["call", "pop", "ret"])
        self.assertEqual(bytes.fromhex(method["raw_code_hex"])[-2:], bytes.fromhex("262a"))
        self.assertFalse(self.receipt["reviewed_chains"]["impersonator"]["close_handle_called"])
        self.assertFalse(self.receipt["boundaries"]["impersonation"]["system_pinvoke_execution_or_abi_proven"])

    def test_forged_fever_publication_raw_call_and_sqlite_finally_are_rejected(self):
        from host import _profile_remaining_review
        for mode in ("fever", "raw", "finally"):
            assemblies = copy.deepcopy(self.receipt["assemblies"])
            if mode == "fever":
                method = self.method("GameScannerStrategyFeverPlatformSearch", "MoveNext", assemblies)
                method["calls"].append({"resolved_method": {"name": "AddFoundGame"}})
                message = "Fever publication, stop or registry release claim differs"
            elif mode == "raw":
                method = self.method("ItchIoGamesDataReader", "GetInstalledGames", assemblies)
                call = next(row for row in method["calls"] if (row["resolved_method"] or {}).get("name") == "Open")
                raw = bytearray.fromhex(method["raw_code_hex"])
                raw[call["offset"]] = 0x28
                method["raw_code_hex"] = raw.hex()
                message = "original call bytes differ"
            else:
                self.method("TwitchGamesDataReader", "GetInstalledGames", assemblies)["exception_sections"] = []
                message = "database catch/finally boundary differs"
            with self.assertRaisesRegex(ValueError, message):
                _profile_remaining_review(assemblies)

    def test_original_os_constant_and_revert_discard_cannot_be_rewritten(self):
        from host import _profile_remaining_registration, _profile_remaining_review
        changed = copy.deepcopy(self.receipt)
        method = changed["registration"]["complete_register_method"]
        next(row for row in method["instructions"] if row["offset"] == 0x204)["opcode"] = "ldc.i4.5"
        with self.assertRaisesRegex(ValueError, "original UWP OS gate differs"):
            _profile_remaining_registration(HostLocalAudit(), changed)
        assemblies = copy.deepcopy(self.receipt["assemblies"])
        self.method("Impersonator", "Dispose", assemblies)["instructions"][1]["opcode"] = "ret"
        with self.assertRaisesRegex(ValueError, "impersonation result or token release differs"):
            _profile_remaining_review(assemblies)

    def test_recomputed_digest_cannot_hide_dropped_strategy_or_import_declaration(self):
        from host import _profile_remaining_projection
        for mode in ("strategy", "import"):
            changed = copy.deepcopy(self.receipt)
            if mode == "strategy":
                changed["registration"]["registrations"].pop()
            else:
                next(row for row in changed["assemblies"] if row["selected_declarations"])["selected_declarations"].pop()
            changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_profile_remaining_projection(changed),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_no_static_version_review_promotes_full_semantics_or_runtime(self):
        for group, field, value in (("registration", "whole_scanner_semantics_complete", True),
                ("fever", "constructed_signature_published", True), ("sqlite", "normal_tail_clear_is_finally", True),
                ("uwp", "classification_has_stop_check", True), ("impersonation", "token_close", True)):
            changed = copy.deepcopy(self.receipt)
            changed["boundaries"][group][field] = value
            with self.assertRaisesRegex(ValueError, "failure or lifecycle boundary differs"):
                self.verify(changed)
        changed = copy.deepcopy(self.receipt)
        changed["actual_dll_comparison"] = "passed"
        with self.assertRaisesRegex(ValueError, "static evidence was promoted"):
            self.verify(changed)

    def test_current_snapshot_json_roundtrip_and_no_temporary_resources(self):
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.receipt["maintenance_lifecycle"]["analysis_sessions_created"], [])
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(saved), self.receipt["verification"])


class ProfileGogScannerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.receipt = host.collect_profile_gog_scanner(cls.audit)
        cls.common = next(row for row in cls.receipt["managed"]["assemblies"]
                          if row["original"]["path"].endswith("/GameScannerCommon.dll"))
        cls.common_raw = cls.audit.read(cls.common["original"])
        cls.common_il = (cls.audit.a.ROOT / cls.common["il_artifact"]["path"]).read_text(encoding="utf-8-sig")

    def verify(self, receipt):
        from host import _verify_profile_gog_claims
        return _verify_profile_gog_claims(receipt)

    def test_current_package_and_real_strategy_registration(self):
        receipt = self.receipt
        self.assertEqual(receipt["host_release"]["version"], "4.0.896")
        self.assertEqual(receipt["host_release"]["official_gms_package"]["declaration"]["resourceVersion"], "3.15.0.1200")
        self.assertEqual([row["file_versions"] for row in receipt["managed"]["assemblies"]], [["3.15.0.1200"]] * 3)
        registration = receipt["managed"]["gog_scoped_registration"]["target"]
        self.assertEqual(registration["name"], "AddScoped")
        self.assertEqual(registration["generic_type_arguments"], ["Razer.GameManagerServiceLib.IGameScannerStrategy",
                         "Razer.GameManagerServiceLib.GameScannerStrategyGogRegistrySearch"])
        self.assertFalse(receipt["managed"]["old_gms_3_14_used_as_current"])
        self.assertEqual(len(receipt["managed"]["verified_call_edges"]), 41)

    def test_registry_order_guid_and_partial_failure_boundaries(self):
        registry = self.receipt["boundaries"]["registry"]
        self.assertEqual((registry["hive"], registry["view"], registry["values"]),
                         ("LocalMachine", "Registry32", ["exe", "launchParam", "gameName"]))
        self.assertTrue(registry["read_before_stop_check"])
        self.assertFalse(registry["explicit_key_disposal"])
        self.assertEqual(registry["wrapper_exception"], "return_completed_prefix")
        self.assertEqual(self.receipt["boundaries"]["strategy"]["priority_value"], 4)
        self.assertFalse(self.receipt["boundaries"]["strategy"]["scan_item_catch"])

    def test_actual_frontend_routes_and_consumers_exclude_ble_methods(self):
        owners = {row["id"]: row for row in self.receipt["complete_source_owners"]}
        selected = self.receipt["selected"]
        self.assertEqual(owners[selected["scan_action_name"]]["body"], 'l="scanGame"')
        self.assertEqual(owners[selected["sdk_startScan"]]["body"],
                         'startScan(){return this._client.invoke(a.GmsServerMethods.StartScan)}')
        self.assertIn("gms-proxy", owners[selected["proxy_startScan"]]["original"]["path"])
        self.assertEqual(self.receipt["bindings"]["page_scan_actions"]["body"], "D=n(38301)")
        self.assertEqual(len(self.receipt["verified_calls"]), 16)
        self.assertIn("this.props.updateGameList", {row["callee"] for row in self.receipt["verified_calls"]})

    def test_strings_comments_and_other_owners_cannot_supply_scan_calls(self):
        from host import _profile_gog_calls
        body = '''class Ble { startScan(){return ble.startScan();} }
            class Games { refresh(){const decoy="this.props.initiateGameScan()";
                // this.props.initiateGameScan()
                return this.props.initiateGameScan();} }
        '''
        audit = HostLocalAudit()
        raw = body.encode()
        source = {"path": ".analysis/inputs/fixture/gog-calls.js", "bytes": len(raw),
                  "sha256": hashlib.sha256(raw).hexdigest()}
        audit.raw[source["path"]], audit.sources[source["path"]] = raw, source
        node, _ = audit.callable_member("refresh", source, audit.tree(source).root_node, "refresh")
        self.assertEqual(len(_profile_gog_calls(audit, source, node, "this.props.initiateGameScan")), 1)
        ble, _ = audit.callable_member("ble", source, audit.tree(source).root_node, "startScan")
        with self.assertRaisesRegex(ValueError, "actual frontend call differs"):
            _profile_gog_calls(audit, source, ble, "this.props.initiateGameScan")

    def test_complete_ast_owners_and_input_snapshot_are_stable(self):
        audit = HostLocalAudit()
        for owner in self.receipt["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_full_il_methods_preserve_original_code_and_exception_sections(self):
        methods = [method for assembly in self.receipt["managed"]["assemblies"] for method in assembly["complete_methods"]]
        self.assertEqual(len(methods), 174)
        for method in methods:
            raw = bytes.fromhex(method["raw_code_hex"])
            self.assertEqual(len(raw), method["code_size"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), method["raw_code_sha256"])
            self.assertEqual(hashlib.sha256(method["complete_il"].encode()).hexdigest(), method["il_sha256"])
            self.assertFalse(method["whole_il_independently_decoded"])
        self.assertTrue(any(method["exception_sections"] for method in methods))
        self.assertEqual({row["target"] for row in self.receipt["managed"]["discarded_return_evidence"]},
                         {"WaitAll", "SaveLocalGameList"})

    def test_il_target_rva_and_original_call_bytes_are_independently_checked(self):
        from host import _profile_gog_clr, _profile_gog_il_methods
        metadata = _profile_gog_clr(self.common_raw)
        selection = {"RegistryWrapper": ("GetRegistryGroup",)}
        method, = _profile_gog_il_methods(self.common["original"], self.common_il, metadata, selection)
        changed = self.common_il.replace("RegistryKey::OpenBaseKey", "RegistryKey::ReadUnrelatedKey")
        with self.assertRaisesRegex(ValueError, "call target differs"):
            _profile_gog_il_methods(self.common["original"], changed, metadata, selection)
        changed = self.common_il.replace("Method begins at RVA " + method["rva"], "Method begins at RVA 0x1")
        with self.assertRaisesRegex(ValueError, "MethodDef identity"):
            _profile_gog_il_methods(self.common["original"], changed, metadata, selection)
        call = next(row for row in method["calls"] if row["resolved_method"]
                    and row["resolved_method"]["name"] == "OpenBaseKey")
        rva = int(method["rva"], 16) + method["header_size"] + call["offset"]
        section, = [row for row in metadata["pe"]["sections"] if row["rva"] <= rva < row["rva"] + row["bytes"]]
        at = section["file_offset"] + rva - section["rva"]
        for corrupt in (b"\x27" + bytes.fromhex(call["bytes"])[1:], b"\x28" + (0x0600033f).to_bytes(4, "little")):
            raw = bytearray(self.common_raw)
            raw[at:at + 5] = corrupt
            with self.assertRaisesRegex(ValueError, "original IL (call opcode|local call target) differs"):
                _profile_gog_il_methods(self.common["original"], self.common_il, _profile_gog_clr(bytes(raw)), selection)

    def test_actual_edges_ignored_returns_and_service_failures_cannot_be_hidden(self):
        from host import _profile_gog_managed_edges
        changed = copy.deepcopy(self.receipt["managed"]["assemblies"])
        method = next(row for row in changed[0]["complete_methods"] if row["name"] == "MoveNext"
                      and any("<OnStartScan>d" in value for value in row["class_ancestry"]))
        wait = next(row for row in method["calls"] if row["resolved_method"] and row["resolved_method"]["name"] == "WaitAll")
        pop = next(row for row in method["instructions"] if row["offset"] == wait["offset"] + 5)
        pop["opcode"] = "stloc.0"
        with self.assertRaisesRegex(ValueError, "ignored-return original pop differs"):
            _profile_gog_managed_edges(changed)
        service = self.receipt["managed"]["assemblies"][-1]
        self.assertFalse(service["capture_input"]["all_csharp_resolved"])
        self.assertEqual(len(service["capture_input"]["diagnostics"]), 10)
        self.assertEqual(len(self.receipt["managed"]["selected_service_csharp_diagnostics"]), 2)
        changed = copy.deepcopy(self.receipt)
        changed["managed"]["assemblies"][-1]["capture_input"]["diagnostics"] = []
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_failure_stop_dedup_and_publish_boundaries_cannot_be_promoted(self):
        for group, name in (("registry", "explicit_key_disposal"), ("strategy", "clear_in_finally"),
                ("controller", "wait_timeout_bool_checked"), ("controller", "stop_joins_strategies"),
                ("controller", "save_local_result_checked"), ("controller", "stale_generation_guard"),
                ("events", "broadcaster_async_void_awaited_by_publisher"), ("post_scan", "atomic_or_rollback"),
                ("frontend", "finished_checks_scanCompleted"), ("frontend", "failed_start_clears_isScanning"),
                ("frontend", "queued_memory_timers_cancelled"), ("frontend", "list_update_proves_file_or_device_write")):
            with self.subTest(group=group, name=name):
                changed = copy.deepcopy(self.receipt)
                changed["boundaries"][group][name] = True
                with self.assertRaisesRegex(ValueError, "failure, cancellation or release boundary differs"):
                    self.verify(changed)

    def test_static_evidence_never_promotes_runtime_or_whole_project(self):
        for name, value in (("source_chain_complete", True), ("whole_profiles_complete", True),
                ("target_execution", True), ("product_code_modified", True), ("actual_dll_comparison", "passed"),
                ("rust_independent_implementation", "complete"), ("ui_backend_wiring", "complete"),
                ("runtime_acceptance", "passed"), ("snapshot_stable", False)):
            changed = copy.deepcopy(self.receipt)
            changed[name] = value
            with self.assertRaisesRegex(ValueError, "static review was promoted"):
                self.verify(changed)

    def test_forged_selection_stays_invalid_after_recalculating_hash(self):
        from host import _profile_gog_projection
        changed = copy.deepcopy(self.receipt)
        changed["managed"]["gog_scoped_registration"]["target"]["generic_type_arguments"][-1] = "OtherStrategy"
        changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_profile_gog_projection(changed),
            ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_strict_json_roundtrip_preserves_complete_receipt(self):
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(saved, self.receipt)
        self.assertEqual(self.verify(saved), self.receipt["verification"])


class ProfileMacSysutilsNativeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        import host
        if not (analysis.ROOT / ".analysis/inputs/host-current/releases/4.0.896/acquisition.json").is_file():
            raise unittest.SkipTest("Retained current Host Mac SysUtils JS sources are unavailable")
        cls.audit = HostLocalAudit()
        cls.host = host
        cls.receipt = host.collect_profile_mac_sysutils_native(cls.audit)

    def changed_source(self, role, before, after):
        owners = {key: dict(self.audit.owners[value]) for key, value in self.receipt["selected"].items()}
        self.assertIn(before, owners[role]["body"])
        owners[role]["body"] = owners[role]["body"].replace(before, after, 1)
        owners[role]["body_sha256"] = hashlib.sha256(owners[role]["body"].encode()).hexdigest()
        return self.host._profile_mac_sysutils_source_contract(owners, self.receipt["ffi_declarations"])

    def test_current_asar_member_is_proven_without_promoting_windows_package_to_mac_native(self):
        result = self.receipt
        self.assertEqual(result["kind"], "current_profile_mac_sysutils_native")
        self.assertEqual(result["host_release"]["version"], "4.0.896")
        self.assertTrue(result["host_release"]["current_js_from_windows_package"])
        self.assertEqual(result["host_release"]["mac_native_version"], "unobserved")
        member = next(row["member"] for row in result["host_release"]["host_members"]
            if row["member"]["path"].endswith("/sysutil/mac/index.js"))
        self.assertEqual(member["sha256"], "b739d78a2253d67de0e2aae4f3e2fc6f4bfc049bd26f0d55a8c5fdee861bc299")
        self.assertEqual((member["bytes"], member["offset"]), (4963, 3739509))
        self.assertTrue(all(row["member_offset_verified"] for row in result["host_release"]["host_members"]))
        self.assertEqual(result["native"]["status"], "not_acquired")
        self.assertIsNone(result["native"]["original"])
        self.assertEqual(result["native"]["complete_selected_functions"], [])
        self.assertFalse(result["native"]["windows_native_used_as_substitute"])

    def test_acquisition_limit_records_real_windows_metadata_and_specific_required_mac_input(self):
        proof = self.receipt["acquisition"]
        self.assertEqual(proof["status"], "source_not_located_in_selected_current_evidence")
        self.assertEqual(proof["selected_metadata_scope"]["os"], "WINDOWS")
        self.assertEqual(len(proof["verified_metadata"]), 8)
        self.assertEqual(proof["indexed_native_candidates"], [])
        self.assertEqual(proof["selected_metadata_native_locator_matches"], [])
        self.assertIsNone(proof["download_url_from_selected_loader"])
        self.assertFalse(proof["absence_of_mac_functionality_inferred"])
        self.assertFalse(proof["guessed_urls_or_platform_parameters"])
        self.assertIn("Mac release/package locator", proof["required_input"])
        self.assertIn("Mach-O architecture/exports", proof["required_input"])
        self.assertEqual(proof["network_requests_attempted"], [])

    def test_new_native_input_cannot_silently_keep_the_old_missing_source_conclusion(self):
        sources, provenance, indexed, release = self.host._profile_mac_sysutils_inputs(self.audit)
        changed = dict(indexed)
        changed["inputs"] = [*indexed["inputs"], {"path": ".analysis/inputs/native/libSysUtilsNative.dylib",
            "bytes": 2, "sha256": "00" * 32}]
        with self.assertRaisesRegex(ValueError, "acquisition inputs changed and need native/source review"):
            self.host._profile_mac_sysutils_acquisition(self.audit, changed, release)

    def test_local_path_and_backend_bindings_cannot_be_replaced_with_windows_or_preload_behavior(self):
        self.assertEqual(self.receipt["source_contract"]["backend"], "ffiMain; not FFIPreloadMain")
        self.assertEqual(self.receipt["bindings"]["host_ffi"]["body"], 'Ye=require("./modules/ffi/ffiMain")')
        self.assertEqual(self.receipt["bindings"]["ffi_callback_map"]["body"], "t=new Map")
        for role, before, after in (
                ("platform_selector", 'require("./mac")', 'require("./win")'),
                ("host_common_directory", '"CommonDylib"', '"CommonDLL"'),
                ("host_loader", 'n="libSysUtilsNative.dylib"', 'n="SysUtilsNative.dll"')):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source(role, before, after)

    def test_initialize_declaration_and_truthy_configuration_do_not_prove_native_initialization(self):
        source = self.receipt["source_contract"]["initialization"]
        self.assertTrue(source["initialize_declared"])
        self.assertFalse(source["initialize_called_by_initDll"])
        self.assertFalse(source["configuration_checks_result_field"])
        self.assertFalse(source["is_init_is_native_ready_confirmation"])
        for before, after in (("this.isInit=!0", 'this.isInit=!0,this.ffi.callDLLMain(s,{action:"Initialize"})'),
                ("if(this.ffi.callDLLMain(s,e))", "if(this.ffi.callDLLMain(s,e)?.result)"),
                ("useNewCallback:!0", "useNewCallback:!1")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source("mac_initDll", before, after)

    def test_keyboard_subscription_bug_and_absent_exports_are_retained_as_js_facts(self):
        self.assertEqual(len(self.receipt["ffi_declarations"]), 32)
        self.assertNotIn("StartMonitorKeyboardLayout", self.receipt["ffi_declarations"])
        self.assertNotIn("StopMonitorKeyboardLayout", self.receipt["ffi_declarations"])
        self.assertFalse(self.receipt["source_contract"]["subscriptions"]["keyboard_start_stop_declared"])
        for before, after in (
                ("this.keyboardLayoutChangeEventList=this.foregroundEventList.filter", "this.keyboardLayoutChangeEventList=this.keyboardLayoutChangeEventList.filter"),
                ("this.foregroundEventList.length>0", "this.keyboardLayoutChangeEventList.length>0"),
                ("this.keyboardLayoutChangeEventList.push(t)", "this.foregroundEventList.push(t)")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source("mac_callDLL", before, after)
        owners = {key: self.audit.owners[value] for key, value in self.receipt["selected"].items()}
        declaration = copy.deepcopy(self.receipt["ffi_declarations"])
        declaration["StartMonitorKeyboardLayout"] = ["bool", []]
        with self.assertRaisesRegex(ValueError, "declared ConfigureFFI table changed"):
            self.host._profile_mac_sysutils_source_contract(owners, declaration)

    def test_foreground_url_and_network_short_circuit_cannot_be_promoted_to_native_observation(self):
        for before, after in (
                ("this.foregroundEventList.includes(t)", "this.foregroundEventList.includes(e.id)"),
                ("await(this.ffi?.callDLLMainAsync(s,t,e))", "this.ffi?.callDLLMain(s,t,e)"),
                ("return!this.isInit||", "return this.isInit&&")):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source("mac_callDLL", before, after)
        self.assertIn("not observed connectivity", self.receipt["source_contract"]["events"]["not_initialized_network_query"])

    def test_callback_json_events_and_respective_url_lists_are_bound_to_actual_owner(self):
        for before, after in (
                ("JSON.parse(e)", "JSON.parse(e||'{}')"),
                ("this.foregroundEventList.includes(e.getURL())", "this.keyboardLayoutChangeEventList.includes(e.getURL())"),
                ("this.keyboardLayoutChangeEventList.includes(e.getURL())", "this.foregroundEventList.includes(e.getURL())"),
                ('n.events.emit("networkStatus",s.isOnline)', 'n.events.emit("networkStatus",!0)'),
                ('this.emit("systempoweroff")', 'n.events.emit("systempoweroff")')):
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source("mac_handleFFIEvent", before, after)

    def test_library_map_release_and_native_callback_clear_have_distinct_lifetimes(self):
        ffi = self.receipt["source_contract"]["ffi"]
        self.assertTrue(ffi["callback_false_result_still_retained"])
        self.assertIn("independent", ffi["callback_retained_by"])
        self.assertIn("library Map entry only", ffi["free_ffi"])
        self.assertFalse(ffi["async_callback_anne"])
        for role, before, after in (
                ("ffi_sync", "f=l.SetNodeFFIEvent(a),t.set(o,a)", "f=l.SetNodeFFIEvent(a),f&&t.set(o,a)"),
                ("ffi_sync", "o.delete(l)", "o.delete(l),t.delete(l)"),
                ("ffi_async", "o.delete(i)", "o.delete(i),t.delete(i)")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source(role, before, after)

    def test_three_second_shutdown_race_does_not_cancel_or_acknowledge_native_release(self):
        shutdown = self.receipt["source_contract"]["shutdown"]
        self.assertEqual(shutdown["timeout_ms"], 3000)
        for field in ("timeout_cancels_call", "main_clears_url_lists", "main_unregisters_callback",
                "main_calls_free_ffi", "native_release_or_callback_quiescence_proven"):
            self.assertFalse(shutdown[field])
        for role, before, after in (("host_shutdown", "yo(3e3,po.callDLLAsync", "yo(0,po.callDLLAsync"),
                ("timeout", "Promise.race([t,o])", "Promise.all([t,o])")):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source boundary changed"):
                self.changed_source(role, before, after)

    def test_complete_source_roundtrip_and_delivery_stages_remain_separate(self):
        for row in self.audit.owners.values():
            raw = self.audit.read(row["original"])[row["byte_start"]:row["byte_end"]]
            self.assertEqual(raw.decode(), row["body"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), row["body_sha256"])
        for key in ("source_chain_complete", "whole_profiles_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(self.receipt[key])
        for key in ("actual_dll_comparison", "rust_independent_implementation", "ui_backend_wiring", "runtime_acceptance"):
            self.assertEqual(self.receipt[key], "unverified")
        self.assertEqual(self.receipt["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class ProfileAlternateIconsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_profile_alternate_icons
        cls.audit = HostLocalAudit()
        cls.receipt = collect_profile_alternate_icons(cls.audit)

    def verify(self, receipt):
        from host import _verify_profile_alternate_icons_claims
        return _verify_profile_alternate_icons_claims(receipt)

    def fixture(self, body):
        audit = HostLocalAudit()
        raw = body.encode("utf-8")
        source = {"path": ".analysis/inputs/fixture/profile-alt.js", "bytes": len(raw),
                  "sha256": hashlib.sha256(raw).hexdigest()}
        audit.raw[source["path"]], audit.sources[source["path"]] = raw, source
        consumers = {}
        for name, size in (("gameIcons", [80, 80]), ("customCover", [300, 150])):
            _, owner = audit.callable_member(name, source, audit.tree(source).root_node, name)
            consumers[name] = (audit.owners[owner], size)
        return audit, source, consumers

    def test_current_actual_consumers_and_native_platform_boundaries(self):
        receipt = self.receipt
        self.assertEqual(receipt["host_release"]["version"], "4.0.896")
        self.assertEqual(receipt["host_release"]["native_file_versions"], ["1.2.3.0"])
        self.assertEqual(self.verify(receipt)["actual_alt_consumers"], 2)
        self.assertEqual({row["consumer"]: row["size"] for row in receipt["actual_alt_calls"]},
                         {"getApplicationsIcon": [80, 80], "setCustomGameCover": [300, 150]})
        self.assertEqual(len(receipt["native"]["complete_selected_functions"]), 17)
        self.assertEqual(len(receipt["native"]["verified_call_edges"]), 15)
        self.assertNotIn("getApplicationIconsAlt", receipt["ffi_declarations"]["macos"])
        self.assertEqual(receipt["macos_native"]["retained_candidates"], [])

    def test_strict_json_roundtrip_preserves_all_reviewed_evidence(self):
        saved = json.loads(json.dumps(self.receipt, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(saved), self.receipt["verification"])
        self.assertEqual(saved, self.receipt)

    def test_every_complete_owner_matches_retained_original_and_snapshot(self):
        audit = HostLocalAudit()
        for owner in self.receipt["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_named_member_and_subscript_calls_exclude_string_and_comment_decoys(self):
        from host import _profile_alternate_icon_calls
        audit, source, consumers = self.fixture('''class Bridge {
            gameIcons(){ return api.getApplicationIconsAlt(data,80,80); }
            customCover(){ return api["getApplicationIconsAlt"](data,300,150); }
        } const text="api.getApplicationIconsAlt(data,1,1)";
        // api.getApplicationIconsAlt(data,1,1)
        ''')
        calls = _profile_alternate_icon_calls(audit, [source], consumers)
        self.assertEqual([row["consumer"] for row in calls], ["gameIcons", "customCover"])

    def test_extra_missing_duplicate_or_dynamic_sized_calls_require_new_review(self):
        from host import _profile_alternate_icon_calls
        prefix = "class Bridge {gameIcons(){"
        suffix = '}customCover(){return api.getApplicationIconsAlt(data,300,150);}}'
        for body in (
                prefix + "return api.getApplicationIconsAlt(data,width,80);" + suffix,
                prefix + "return api.getApplicationIconsAlt(data,128,128);" + suffix,
                prefix + "return ordinaryIcons(data,80,80);" + suffix,
                prefix + "api.getApplicationIconsAlt(data,80,80);return api.getApplicationIconsAlt(data,80,80);" + suffix,
                prefix + "return api.getApplicationIconsAlt(data,80,80);" + suffix
                    + "api.getApplicationIconsAlt(data,80,80);",
                prefix + "return api.getApplicationIconsAlt(data,80,80,extra);" + suffix):
            with self.subTest(body=body):
                audit, source, consumers = self.fixture(body)
                with self.assertRaisesRegex(ValueError, "Profile Alt"):
                    _profile_alternate_icon_calls(audit, [source], consumers)

    def test_real_empty_image_fallback_cache_and_release_limits_cannot_be_promoted(self):
        for group, name, value in (
                ("custom_cover", "empty_image", "retain current cover"),
                ("platform", "mac_declares_alt", True),
                ("platform", "non_nullish_empty_string_falls_back", True),
                ("platform", "non_electron_direct_alt_bridge", True),
                ("native", "alternate_uses_exe_icon_dispatcher", True),
                ("native", "returned_path_becomes_altPath", True),
                ("native", "cache_tracks_successful_source_path", False),
                ("native", "missing_file_can_hit_cache", False),
                ("native", "batch_prunes_other_keys", False),
                ("native", "cache_atomic_publish", True),
                ("native", "native_cancel_parameter", True),
                ("lifecycle", "older_native_results_can_mutate_intermediate_lists", False),
                ("lifecycle", "bridge_has_unmount", True),
                ("lifecycle", "native_query_cancelled_on_unmount", True),
                ("lifecycle", "async_callback_error_is_reliable_rejection", True),
                ("lifecycle", "free_after_readCString_exception", True),
                ("lifecycle", "FreeFFI_waits_for_inflight_or_confirms_unload", True),
                ("lifecycle", "list_update_proves_service_or_device_write", True)):
            with self.subTest(group=group, name=name):
                changed = copy.deepcopy(self.receipt)
                changed["boundaries"][group][name] = value
                with self.assertRaisesRegex(ValueError, "failure/platform boundary differs"):
                    self.verify(changed)

    def test_mac_missing_original_and_static_status_never_imply_completion(self):
        for name, value in (("source_chain_complete", True), ("whole_profiles_complete", True),
                ("target_execution", True), ("actual_dll_comparison", "passed"),
                ("rust_independent_implementation", "complete"), ("ui_backend_wiring", "complete"),
                ("runtime_acceptance", "passed"), ("real_device_operations", True)):
            changed = copy.deepcopy(self.receipt)
            changed[name] = value
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "static review was promoted"):
                self.verify(changed)
        changed = copy.deepcopy(self.receipt)
        changed["macos_native"].update(status="complete", absence_of_feature_inferred=True)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_changed_consumers_exports_bindings_abi_and_native_edges_require_review(self):
        for mutate in (
                lambda r: r["cross_chunk_exports"].pop("update_game_list"),
                lambda r: r["bindings"]["api_import"].update(body="g=n(12345)"),
                lambda r: r["ffi_declarations"]["windows"]["getApplicationIconsAlt"]["signature"][1].__setitem__(0, "pointer"),
                lambda r: r["native"]["verified_call_edges"].pop(),
                lambda r: r["actual_alt_calls"][0].update(size=[128, 128]),
                lambda r: r["native"]["complete_selected_functions"][0].update(pseudocode_text_sha256="0" * 64)):
            changed = copy.deepcopy(self.receipt)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_forged_full_consumer_remains_invalid_after_hash_recalculation(self):
        changed = copy.deepcopy(self.receipt)
        owner = next(row for row in changed["complete_source_owners"]
                     if row["id"] == changed["selected"]["setCustomGameCover"])
        owner["body"] = owner["body"].replace(",300,150)", ",128,128)")
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(HostLocalAudit(), owner)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_global_hexrays_failure_stays_separate_from_selected_success(self):
        self.assertEqual(self.receipt["native"]["selected_pseudocode_failures"], [])
        self.assertEqual(len(self.receipt["native"]["failures"]), 1)
        changed = copy.deepcopy(self.receipt)
        changed["native"]["failures"] = []
        with self.assertRaisesRegex(ValueError, "native capture failure was hidden"):
            self.verify(changed)


class NativePseudocodeStatusTests(unittest.TestCase):
    @staticmethod
    def owner(rva, pseudocode):
        return {"artifact": {"path": "functions/" + format(rva, "x") + ".json"},
                "capture": {"address": hex(0x140000000 + rva), "rva": hex(rva),
                            "pseudocode": pseudocode}}

    def test_stage_bound_failure_is_retained_without_a_top_level_address(self):
        failure = {"stage": "1544c0:hexrays", "error": "call analysis failed at another address"}
        selected = [self.owner(0x1544c0, None), self.owner(0x14fe20, {"lines": []})]
        result = native_pseudocode_failures(selected, [failure,
            {"stage": "154500:hexrays", "error": "unselected owner"}])
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["address"], "0x1401544c0")
        self.assertEqual(result[0]["capture_failures"], [failure])
        self.assertFalse(result[0]["pseudocode_captured"])
        self.assertEqual(result[0]["artifact"], selected[0]["artifact"])

    def test_missing_failure_metadata_cannot_turn_an_absent_body_into_success(self):
        result = native_pseudocode_failures([self.owner(0x154500, None)], [])
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["capture_failures"], [])
        self.assertFalse(result[0]["pseudocode_captured"])

    def test_supplemental_rva_bound_failure_keeps_the_original_error(self):
        failure = {"stage": "hexrays", "rva": "0x30888", "error": "call analysis failed"}
        result = native_pseudocode_failures([self.owner(0x30888, None)], [failure])
        self.assertEqual(result[0]["capture_failures"], [failure])
        self.assertEqual(result[0]["rva"], "0x30888")

    def test_recovered_body_does_not_inherit_its_original_failure(self):
        recovered = self.owner(0x1544c0, {"lines": [{"text": "complete recovered body"}]})
        self.assertEqual(native_pseudocode_failures([recovered],
            [{"stage": "1544c0:hexrays", "error": "original failure"}]), [])


class BoundedSourceObservationTests(unittest.TestCase):
    def fixture(self, mutate=None):
        import analysis
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", root)
        patched.start()
        self.addCleanup(patched.stop)
        directory = ".analysis/inputs/frontend/observation-current/"
        path = directory + "raw/apps.razer.com/catalog.json"
        original = {"path": path, "bytes": 2, "sha256": hashlib.sha256(b"[]").hexdigest()}
        target = root / path
        target.parent.mkdir(parents=True)
        target.write_bytes(b"[]")
        url = "https://apps.razer.com/catalog.json"
        http = {**original, "url": url, "final_url": url, "redirects": [], "http_status": 200,
                "acquired_at_utc": "2026-10-22T11:00:00Z"}
        receipt = {"entry_url": url, "scope": "bounded catalog observation", "target_execution": False,
            "source_current_during_acquisition": True, "files": [http],
            "http_observations": [{**http, "phase": "before"}, {**http, "phase": "after"}],
            "stability": [{"url": url, "before_sha256": original["sha256"],
                           "after_sha256": original["sha256"], "unchanged": True}]}
        if mutate:
            mutate(receipt)
        receipt_path = directory + "acquisition.json"
        raw = json.dumps(receipt).encode()
        (root / receipt_path).write_bytes(raw)
        (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [original,
            {"path": receipt_path, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}]}))
        audit = HostLocalAudit.__new__(HostLocalAudit)
        audit.a, audit.artifacts, audit.sources, audit.raw = analysis, {}, {}, {}
        return audit, url, target

    def test_verified_rounds_remain_bounded_without_a_dependency_closure(self):
        audit, url, _ = self.fixture()
        result, originals = audit.observed_sources("observation-current", url)
        self.assertEqual(result["http_observations_verified"], 2)
        self.assertEqual(len(originals), 1)
        self.assertFalse(result["full_dependency_closure"])
        self.assertFalse(result["target_execution"])
        self.assertEqual(audit.final_snapshot(), [])

    def test_claimed_stability_cannot_hide_a_changed_http_round(self):
        audit, url, _ = self.fixture(lambda d: d["http_observations"][1].update(sha256="0" * 64))
        with self.assertRaisesRegex(ValueError, "HTTP identity changed"):
            audit.observed_sources("observation-current", url)

    def test_intermediate_entry_check_is_verified_without_replacing_either_round(self):
        def change(d):
            item = {**d["http_observations"][0], "phase": "entry_midcheck"}
            item["finished_at_utc"] = item.pop("acquired_at_utc")
            d["http_observations"].insert(1, item)
            for request in d["http_observations"]:
                request["finished_at_utc"] = request.pop("acquired_at_utc", item["finished_at_utc"])
        audit, url, _ = self.fixture(change)
        result, _ = audit.observed_sources("observation-current", url)
        self.assertEqual(result["http_observations_verified"], 3)
        self.assertEqual(result["observations"][0]["additional_entry_observations"], 1)
        audit, url, _ = self.fixture(lambda d: d["http_observations"].insert(1,
            {**d["http_observations"][0], "phase": "entry_midcheck", "bytes": 3}))
        with self.assertRaisesRegex(ValueError, "HTTP identity changed"):
            audit.observed_sources("observation-current", url)

    def test_duplicate_missing_or_unrelated_observations_are_rejected(self):
        for change in (lambda d: d["files"].append(copy.deepcopy(d["files"][0])),
                       lambda d: d["http_observations"].pop(),
                       lambda d: d["http_observations"].append({"url": "https://apps.razer.com/other"}),
                       lambda d: d["http_observations"][1].update(final_url="https://apps.razer.com/other")):
            with self.subTest(change=change):
                audit, url, _ = self.fixture(change)
                with self.assertRaises(ValueError):
                    audit.observed_sources("observation-current", url)

    def test_current_flag_and_retained_bytes_are_verified_independently(self):
        audit, url, _ = self.fixture(lambda d: d.update(source_current_during_acquisition=False))
        with self.assertRaisesRegex(ValueError, "unverified identity"):
            audit.observed_sources("observation-current", url)
        audit, url, target = self.fixture()
        target.write_bytes(b"{}")
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            audit.observed_sources("observation-current", url)

    def extended_fixture(self, mutate=None, *, stale_index=False, multiple_edges=False):
        import evidence
        audit, url, original = self.fixture()
        root = audit.a.ROOT
        receipt = original.parents[2] / "acquisition.json"
        previous = json.loads(receipt.read_bytes())
        dependency_url = "https://apps.razer.com/dependency.js"
        if multiple_edges:
            raw = b'<link rel="preload" href="/dependency.js"><script src="/dependency.js"></script>'
            original.write_bytes(raw)
            claim = {"bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), "content_type": "text/html"}
            previous["files"][0].update(claim)
            for row in previous["http_observations"]:
                row.update(claim)
            previous["stability"][0].update(before_sha256=claim["sha256"], after_sha256=claim["sha256"])
        previous["edges"] = [{"from": url, "to": dependency_url, "kind": "script", "literal": "dependency.js"}]
        previous_raw = json.dumps(previous).encode()

        def retain(path, value):
            raw = json.dumps(value).encode()
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(raw)
            return {"path": path, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

        digest = hashlib.sha256(previous_raw).hexdigest()
        predecessor = {"path": ".analysis/results/reverse-pending/review-sources/" + digest + ".json",
                       "bytes": len(previous_raw), "sha256": digest}
        predecessor_path = root / predecessor["path"]
        predecessor_path.parent.mkdir(parents=True)
        predecessor_path.write_bytes(previous_raw)
        addition = {**previous["files"][0], "url": dependency_url,
                    "final_url": dependency_url,
                    "path": ".analysis/inputs/frontend/provider-current/raw/dependency.js",
                    "bytes": 2, "sha256": hashlib.sha256(b"[]").hexdigest()}
        target = root / addition["path"]
        target.parent.mkdir(parents=True)
        target.write_bytes(b"[]")
        provider = retain(".analysis/inputs/frontend/provider-current/acquisition.json", {
            "entry_url": "https://apps.razer.com/provider/", "target_execution": False,
            "source_current_during_acquisition": True, "files": [addition]})
        capture = retain(".analysis/results/frontend/provider-current/analysis.json", {
            "input": ".analysis/inputs/frontend/provider-current", "input_bytes_unchanged": True,
            "target_execution": False, "original_inputs": [addition],
            "commands": [{"stage": "validate", "exit_code": 1}]})
        current = copy.deepcopy(previous)
        current["files"].append({**addition, "retained_dependency": {"acquisition": provider, "capture": capture}})
        prior_deployment = evidence.frontend_deployment_sources(previous)
        current["edges"].extend({**reference, "to": source["url"], "retained_provider": provider}
                                for source in prior_deployment["sources"] if source["url"] == addition["url"]
                                for reference in source["references"])
        current["dependency_repairs"] = [{"previous_acquisition": predecessor,
            "provider_acquisition": provider, "provider_capture": capture,
            "target_execution": False, "copied_originals": 0, "repeated_factory_capture": False,
            "added_originals": [{key: addition[key] for key in ("url", "path", "bytes", "sha256")}],
            "previous_deployment_failures": evidence.frontend_deployment_sources(previous)["failures"],
            "deployment_failures_after_repair": evidence.frontend_deployment_sources(current)["failures"]}]
        if mutate:
            mutate(current)
        identity = retain(receipt.relative_to(root).as_posix(), current)
        index_path = root / ".analysis/inputs.json"
        indexed = json.loads(index_path.read_bytes())
        indexed["inputs"][0] = {key: previous["files"][0][key] for key in ("path", "bytes", "sha256")}
        if not stale_index:
            indexed["inputs"][-1] = identity
        indexed["inputs"].append({key: addition[key] for key in ("path", "bytes", "sha256")})
        index_path.write_text(json.dumps(indexed))
        return audit, url, receipt, current, target, retain

    def test_provider_extension_keeps_the_exact_bounded_http_observation(self):
        audit, url, _, _, _, _ = self.extended_fixture()
        result, originals = audit.observed_sources("observation-current", url)
        self.assertEqual(result["http_observations_verified"], 2)
        self.assertEqual(len(originals), 1)
        self.assertEqual(len(result["observations"]), 1)
        self.assertNotEqual(result["bounded_http_acquisition"], result["acquisition"])
        self.assertEqual(result["retained_dependency_extensions"][0]["additional_originals_verified"], 1)
        self.assertEqual(result["retained_dependency_extensions"][0]["additional_http_rounds_claimed"], 0)
        self.assertFalse(result["full_dependency_closure"])
        self.assertEqual(audit.final_snapshot(), [])

    def test_extension_never_accepts_a_stale_index_or_changed_provider_bytes(self):
        audit, url, _, _, _, _ = self.extended_fixture(stale_index=True)
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            audit.observed_sources("observation-current", url)
        audit, url, _, _, target, _ = self.extended_fixture()
        target.write_bytes(b"{}")
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            audit.observed_sources("observation-current", url)

    def test_one_original_preserves_both_actual_preload_and_script_references(self):
        audit, url, _, current, _, _ = self.extended_fixture(multiple_edges=True)
        self.assertEqual([edge["role"] for edge in current["edges"][1:]],
                         ["html_preload_candidate", "html_script_include"])
        result, originals = audit.observed_sources("observation-current", url)
        self.assertEqual(result["retained_dependency_extensions"][0]["additional_originals_verified"], 1)
        self.assertEqual(len(originals), 1)
        self.assertEqual(audit.final_snapshot(), [])
        for change in (lambda d: d["edges"].pop(),
                       lambda d: d["edges"].reverse(),
                       lambda d: d["edges"][-1].update(role="html_preload_candidate")):
            with self.subTest(change=change):
                audit, url, _, _, _, _ = self.extended_fixture(change, multiple_edges=True)
                with self.assertRaisesRegex(ValueError, "edges differ|original owners"):
                    audit.observed_sources("observation-current", url)
    def test_repair_cannot_rewrite_observation_or_substitute_provider_ownership(self):
        changes = (
            lambda d: d["http_observations"][0].update(bytes=3),
            lambda d: d["files"][0].update(http_status=404),
            lambda d: d["files"][1]["retained_dependency"]["capture"].update(sha256="0" * 64),
            lambda d: d["dependency_repairs"][0]["added_originals"].clear(),
            lambda d: d["dependency_repairs"][0].update(target_execution=True),
            lambda d: d["dependency_repairs"][0].update(copied_originals=1),
            lambda d: d["edges"][-1].update(to="https://apps.razer.com/unrelated.js"),
            lambda d: d["edges"][-1].update(role="html_script_include"),
            lambda d: d["edges"][-1].update(literal="unrelated.js"),
            lambda d: d["edges"].append(copy.deepcopy(d["edges"][-1])),
            lambda d: d["dependency_repairs"][0]["previous_deployment_failures"].clear(),
            lambda d: d["files"][1].update(url=d["files"][0]["url"]),
        )
        for change in changes:
            with self.subTest(change=change):
                audit, url, _, _, _, _ = self.extended_fixture(change)
                with self.assertRaises(ValueError):
                    audit.observed_sources("observation-current", url)


class ReviewedAcquisitionIdentityTests(unittest.TestCase):
    def fixture(self):
        fixtures = BoundedSourceObservationTests("test_provider_extension_keeps_the_exact_bounded_http_observation")
        self.addCleanup(fixtures.doCleanups)
        audit, _, receipt, current, _, retain = fixtures.extended_fixture()
        indexed = {row["path"]: row for row in json.loads(
            (audit.a.ROOT / ".analysis/inputs.json").read_bytes())["inputs"]}
        reference = {**current["dependency_repairs"][0]["previous_acquisition"],
                     "path": receipt.relative_to(audit.a.ROOT).as_posix()}
        return audit, receipt, current, indexed, reference, retain

    def test_reviewed_scope_remains_its_exact_immutable_predecessor(self):
        from host import verify_reviewed_frontend_acquisition
        audit, receipt, _, indexed, reference, _ = self.fixture()
        original = copy.deepcopy(reference)
        raw = receipt.read_bytes()
        result = verify_reviewed_frontend_acquisition(audit, reference, indexed)
        self.assertEqual(result["relation"], "verified_immutable_predecessor")
        self.assertTrue(result["historical_scope_preserved"])
        self.assertEqual(result["reviewed_acquisition"], original)
        self.assertNotEqual(result["retained_acquisition"]["path"], reference["path"])
        self.assertEqual(result["retained_acquisition"]["sha256"], reference["sha256"])
        self.assertEqual(receipt.read_bytes(), raw)
        self.assertEqual(reference, original)
        self.assertEqual(audit.final_snapshot(), [])

    def test_unchanged_review_requires_the_current_index_and_bytes(self):
        from host import verify_reviewed_frontend_acquisition
        audit, receipt, _, indexed, reference, _ = self.fixture()
        current = indexed[reference["path"]]
        result = verify_reviewed_frontend_acquisition(audit, current, indexed)
        self.assertEqual(result["relation"], "unchanged")
        receipt.write_bytes(receipt.read_bytes() + b" ")
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            verify_reviewed_frontend_acquisition(audit, current, indexed)

    def test_valid_but_unrelated_snapshot_cannot_become_the_reviewed_scope(self):
        from host import verify_reviewed_frontend_acquisition
        audit, _, _, indexed, reference, retain = self.fixture()
        raw = b'{"entry_url":"https://apps.razer.com/unrelated"}'
        digest = hashlib.sha256(raw).hexdigest()
        unrelated = retain(".analysis/results/reverse-pending/review-sources/" + digest + ".json", json.loads(raw))
        with self.assertRaisesRegex(ValueError, "not a verified immutable predecessor"):
            verify_reviewed_frontend_acquisition(audit, {**unrelated, "path": reference["path"]}, indexed)

    def test_missing_or_stale_index_and_changed_predecessor_are_rejected(self):
        from host import verify_reviewed_frontend_acquisition
        for mutation in ("missing", "stale", "changed_predecessor"):
            with self.subTest(mutation=mutation):
                audit, _, current, indexed, reference, _ = self.fixture()
                if mutation == "missing":
                    indexed.pop(reference["path"])
                elif mutation == "stale":
                    indexed[reference["path"]] = reference
                else:
                    path = audit.a.ROOT / current["dependency_repairs"][0]["previous_acquisition"]["path"]
                    path.write_bytes(path.read_bytes() + b" ")
                with self.assertRaises(ValueError):
                    verify_reviewed_frontend_acquisition(audit, reference, indexed)

    def test_changed_repair_and_non_acquisition_owners_are_rejected(self):
        from host import verify_reviewed_frontend_acquisition
        audit, receipt, current, indexed, reference, retain = self.fixture()
        current["dependency_repairs"][0]["copied_originals"] = 1
        indexed[reference["path"]] = retain(reference["path"], current)
        with self.assertRaisesRegex(ValueError, "original owners"):
            verify_reviewed_frontend_acquisition(audit, reference, indexed)
        for changed in ({**reference, "path": "tools/host.py"},
                        {**reference, "bytes": True}, {**reference, "sha256": "not-a-digest"}):
            with self.subTest(changed=changed), self.assertRaisesRegex(ValueError, "exact maintained identity"):
                verify_reviewed_frontend_acquisition(audit, changed, indexed)


class SingleRequestDependencyRepairTests(unittest.TestCase):
    def fixture(self, mutate=None, *, stale_index=False):
        import analysis
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", root)
        patched.start()
        self.addCleanup(patched.stop)
        directory = ".analysis/inputs/frontend/single-request-current/"
        url = "https://apps.razer.com/catalog.json"

        def write(path, raw):
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(raw)
            return {"path": path, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

        original = write(directory + "raw/apps.razer.com/catalog.json", b"[]")
        http = {**original, "url": url, "final_url": url, "redirects": [], "http_status": 200,
                "acquired_at_utc": "2026-10-22T11:00:00Z"}
        previous = {"entry_url": url, "scope": "bounded observation", "target_execution": False,
            "source_current_during_acquisition": True, "finished_at_utc": "2026-10-22T11:01:00Z",
            "files": [http], "http_observations": [{**http, "phase": "before"}, {**http, "phase": "after"}],
            "stability": [{"url": url, "before_sha256": original["sha256"],
                           "after_sha256": original["sha256"], "unchanged": True}]}
        raw = json.dumps(previous).encode()
        digest = hashlib.sha256(raw).hexdigest()
        predecessor = write(".analysis/results/reverse-pending/review-sources/" + digest + ".json", raw)
        addition = {**write(directory + "raw/apps.razer.com/dependency.js", b"const value=7;"),
                    "url": "https://apps.razer.com/dependency.js", "final_url": "https://apps.razer.com/dependency.js",
                    "http_status": 200, "content_type": "text/javascript",
                    "headers": {"Content-Type": "text/javascript", "Content-Length": "14"},
                    "acquired_at_utc": "2026-10-26T11:02:49.500000Z"}
        current = copy.deepcopy(previous)
        current["files"].append(addition)
        current["dependency_repairs"] = [{"previous_acquisition": predecessor,
            "recorded_at_utc": "2026-10-26T11:02:49.900000Z", "target_execution": False,
            "requested_urls": [addition["url"]],
            "successful_originals": [{key: addition[key] for key in ("url", "path", "bytes", "sha256")}]}]
        if mutate:
            mutate(current)
        identity = write(directory + "acquisition.json", json.dumps(current).encode())
        indexed = [original, {key: addition[key] for key in ("path", "bytes", "sha256")},
                   {**(predecessor if stale_index else identity), "path": identity["path"]}]
        write(".analysis/inputs.json", json.dumps({"inputs": indexed}).encode())
        audit = HostLocalAudit.__new__(HostLocalAudit)
        audit.a, audit.artifacts, audit.sources, audit.raw = analysis, {}, {}, {}
        return audit, url, root / identity["path"], current, root / addition["path"]

    def test_single_http_request_does_not_expand_the_two_round_observation(self):
        audit, url, _, _, _ = self.fixture()
        result, originals = audit.observed_sources("single-request-current", url)
        self.assertEqual(len(originals), 1)
        self.assertEqual(result["http_observations_verified"], 2)
        extension = result["retained_dependency_extensions"][0]
        self.assertEqual(extension["dependency_extension_kind"], "single_http_request")
        self.assertEqual(extension["additional_http_requests_recorded"], 1)
        self.assertEqual(extension["additional_http_rounds_claimed"], 0)
        self.assertFalse(extension["dependency_stability_verified"])
        self.assertFalse(extension["full_dependency_closure"])
        self.assertFalse(extension["http_requests"][0]["redirect_chain_recorded"])
        self.assertEqual(audit.final_snapshot(), [])

    def test_stale_index_and_changed_new_bytes_are_still_rejected(self):
        audit, url, _, _, _ = self.fixture(stale_index=True)
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            audit.observed_sources("single-request-current", url)
        audit, url, _, _, target = self.fixture()
        target.write_bytes(b"const value=8;")
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            audit.observed_sources("single-request-current", url)

    def test_successful_list_cannot_change_original_http_or_repair_scope(self):
        changes = (
            lambda d: d["files"][0].update(http_status=404),
            lambda d: d["http_observations"][0].update(phase="single"),
            lambda d: d["dependency_repairs"][0]["successful_originals"].clear(),
            lambda d: d["dependency_repairs"][0]["requested_urls"].append(d["files"][1]["url"]),
            lambda d: d["dependency_repairs"][0].update(provider_capture={}),
            lambda d: d["dependency_repairs"][0].update(target_execution=True),
            lambda d: d.update(edges=[{"to": d["files"][1]["url"]}]),
            lambda d: d["dependency_repairs"][0]["previous_acquisition"].update(bytes=1),
        )
        for change in changes:
            with self.subTest(change=change):
                audit, url, _, _, _ = self.fixture(change)
                with self.assertRaises(ValueError):
                    audit.observed_sources("single-request-current", url)

    def test_additional_http_identity_requires_official_url_exact_path_length_and_time(self):
        changes = (
            lambda d: d["files"][1].update(url="http://apps.razer.com/dependency.js"),
            lambda d: d["files"][1].update(final_url="https://apps.razer.com/other.js"),
            lambda d: d["files"][1].update(path=d["files"][0]["path"]),
            lambda d: d["files"][1].update(http_status=404),
            lambda d: d["files"][1].update(redirects=[{"status": 302}]),
            lambda d: d["files"][1]["headers"].update({"Content-Length": "13"}),
            lambda d: d["files"][1]["headers"].pop("Content-Length"),
            lambda d: d["files"][1]["headers"].update({"Content-Type": "text/html"}),
            lambda d: d["files"][1].update(acquired_at_utc="2026-10-26T11:02:49.500000"),
            lambda d: d["files"][1].update(acquired_at_utc="2026-10-26T11:02:50Z"),
            lambda d: d["dependency_repairs"][0].update(recorded_at_utc="2026-10-22T10:00:00Z"),
            lambda d: d["files"][1].update(retained_dependency={}),
        )
        for change in changes:
            with self.subTest(change=change):
                audit, url, _, _, _ = self.fixture(change)
                with self.assertRaises(ValueError):
                    audit.observed_sources("single-request-current", url)


class LightingCatalogProjectionTests(unittest.TestCase):
    def test_row_order_precedes_later_primary_and_preserves_duplicate_rows(self):
        result = lighting_catalog_projection([
            {"productId": 20, "dongleId": 10}, {"productId": 10}, {"productId": 20}])
        self.assertEqual(result["rows"], 3)
        self.assertEqual(result["distinct_primary_pids"], 2)
        self.assertEqual(result["ordered_resolutions"], [
            {"input_pid": 10, "primary": 20, "index": 0, "matched": "dongleId"},
            {"input_pid": 20, "primary": 20, "index": 0, "matched": "productId"}])

    def test_array_and_other_aliases_extend_the_domain_without_becoming_matches(self):
        result = lighting_catalog_projection([
            {"productId": 20, "dongleId": [10, 11], "wiredId": [12], "xBoxId": 13, "bleId": 14}])
        self.assertEqual(result["all_declared_numeric_id_count"], 6)
        self.assertEqual(result["engine_scalar_input_count"], 2)
        self.assertEqual(result["engine_changed_input_count"], 1)
        self.assertEqual([row["primary"] for row in result["ordered_resolutions"]], [10, 11, 12, 13, 20, 20])

    def test_invalid_or_new_field_shapes_require_an_explicit_audit(self):
        for rows in ([], [{"productId": True}], [{"productId": 0}], [{"productId": 65536}],
                     [{"productId": 20, "bleId": [10]}], [{"productId": 20, "dongleId": [False]}],
                     [{"productId": 20, "isNotChromaDevice": 1}], [{"productId": 20, "newAlias": 10}]):
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                lighting_catalog_projection(rows)


class ReceiverHardwareDispatchTests(unittest.TestCase):
    def capture(self):
        base = 0x180000000
        result = {"image_base": hex(base), "complete_functions": [], "verified_data_regions": []}
        for role, owner, lea, table, target in (
            ("input_base", 0x64e8c, 0x64ef0, 0x2e5ac8, 0x2a4ac0),
            ("usb", 0xa89f0, 0xa8a5a, 0x2e7e38, 0xa8de0),
            ("ble", 0x52158, 0x521c2, 0x2e3fd8, 0x53c70),
            ("external", 0x6319c, 0x63206, 0x2e5668, 0x64a60),
            ("os_hook", 0xa6b74, 0xa6bde, 0x2e7958, 0x549d0),
        ):
            result["complete_functions"].append({"capture": {"rva": hex(owner), "assembly": [
                {"rva": hex(lea), "bytes": (b"\x48\x8d\x05" + struct.pack("<i", table - lea - 7)).hex()},
                {"rva": hex(lea + 7), "bytes": "48898680000000"}]}})
            result["verified_data_regions"].append({"role": role + "_hardware_vtable",
                "raw_hex": struct.pack("<QQQQ", base, base, base + 0x6a330, base + target).hex()})
        for owner, rows in ((0x9d7d0, [(0x9d7f4, "488b4620"), (0x9d7f8, "488b4e28"),
                                     (0x9d7fc, "4883e980")]),
                            (0x6a470, [(0x6a470, "488b01"), (0x6a473, "488b4018")])):
            result["complete_functions"].append({"capture": {"rva": hex(owner), "assembly": [
                {"rva": hex(rva), "bytes": raw} for rva, raw in rows]}})
        return result

    def test_adjusted_subobject_resolves_each_concrete_device_override(self):
        result = receiver_hardware_dispatch_proof(self.capture())
        self.assertEqual(result["bound_object_adjustment"], 128)
        self.assertEqual({row["device_type"]: row["queued_target_rva"] for row in result["dispatch"]},
            {"input_base": "0x2a4ac0", "usb": "0xa8de0", "ble": "0x53c70",
             "external": "0x64a60", "os_hook": "0x549d0"})

    def test_wrong_adjustment_constructor_slot_or_base_vtable_cannot_pass(self):
        cases = []
        wrong_adjustment = self.capture()
        wrong_adjustment["complete_functions"][-2]["capture"]["assembly"][-1]["bytes"] = "4883e988"
        cases.append(wrong_adjustment)
        wrong_owner = self.capture()
        wrong_owner["complete_functions"][1]["capture"]["assembly"][-1]["bytes"] = "48898688000000"
        cases.append(wrong_owner)
        wrong_slot = self.capture()
        wrong_slot["complete_functions"][-1]["capture"]["assembly"][-1]["bytes"] = "488b4010"
        cases.append(wrong_slot)
        wrong_table = self.capture()
        wrong_table["verified_data_regions"][1]["raw_hex"] = struct.pack(
            "<QQQQ", 0x180000000, 0x180000000, 0x18006a330, 0x1800662b0).hex()
        cases.append(wrong_table)
        truncated = self.capture()
        truncated["verified_data_regions"][1]["raw_hex"] = "00" * 24
        cases.append(truncated)
        for index, case in enumerate(cases):
            with self.subTest(case=index), self.assertRaises(ValueError):
                receiver_hardware_dispatch_proof(case)


class NativeStringPointerTests(unittest.TestCase):
    def test_pointer_values_select_actual_strings_and_preserve_enum_order(self):
        values = native_ascii_pointer_table(struct.pack("<QQ", 0x180001004, 0x180001000),
            b"one\0two\0", image_base=0x180000000, strings_rva=0x1000, count=2)
        self.assertEqual([row["name"] for row in values], ["two", "one"])
        self.assertEqual([row["index"] for row in values], [0, 1])
        self.assertEqual(values[0]["string_rva"], "0x1004")

    def test_invalid_pointer_length_encoding_termination_or_duplicates_are_rejected(self):
        for table, strings, count in (
            (b"\0" * 7, b"one\0", 1),
            (struct.pack("<Q", 0x180000fff), b"one\0", 1),
            (struct.pack("<Q", 0x180001004), b"one\0", 1),
            (struct.pack("<Q", 0x180001000), b"one", 1),
            (struct.pack("<Q", 0x180001000), b"\0", 1),
            (struct.pack("<Q", 0x180001000), b"\xff\0", 1),
            (struct.pack("<Q", 0x180001000), b"x" * 256 + b"\0", 1),
            (struct.pack("<QQ", 0x180001000, 0x180001004), b"one\0one\0", 2)):
            with self.subTest(table=table, strings=strings), self.assertRaises(ValueError):
                native_ascii_pointer_table(table, strings,
                    image_base=0x180000000, strings_rva=0x1000, count=count)


def pe_fixture(bits=64):
    raw = bytearray(0x1000)
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3c, 0x80)
    raw[0x80:0x84] = b"PE\0\0"
    size = 240 if bits == 64 else 224
    struct.pack_into("<HHIIIHH", raw, 0x84, 0x8664 if bits == 64 else 0x14c, 1, 0, 0, 0, size, 0x2102)
    optional = 0x98
    struct.pack_into("<H", raw, optional, 0x20b if bits == 64 else 0x10b)
    struct.pack_into("<Q" if bits == 64 else "<I", raw, optional + (24 if bits == 64 else 28), 0x180000000 if bits == 64 else 0x10000000)
    struct.pack_into("<I", raw, optional + 60, 0x200)
    directory = optional + (112 if bits == 64 else 96)
    struct.pack_into("<I", raw, directory - 4, 16)
    struct.pack_into("<8sIIII", raw, optional + size, b".rdata\0\0", 0xe00, 0x1000, 0xe00, 0x200)
    return raw, directory


def cursor_fixture():
    raw, directory = pe_fixture()
    struct.pack_into("<II", raw, directory + 16, 0x1200, 0x400)

    def node(offset, entries):
        base = 0x400 + offset
        struct.pack_into("<IIHHHH", raw, base, 0, 0, 0, 0, 0, len(entries))
        for index, entry in enumerate(entries):
            struct.pack_into("<II", raw, base + 16 + index * 8, *entry)

    node(0, [(1, 0x80000030), (12, 0x80000060)])
    node(0x30, [(7, 0x80000090)])
    node(0x60, [(102, 0x800000b0)])
    node(0x90, [(1033, 0xd0)])
    node(0xb0, [(1033, 0xe0)])
    dib = struct.pack("<IiiHHIIiiII", 40, 2, 4, 1, 32, 0, 0, 0, 0, 0, 0) + bytes(range(16)) + b"\x80\0\0\0" * 2
    cursor = struct.pack("<HH", 1, 0) + dib
    group = struct.pack("<HHHHHHHIH", 0, 2, 1, 2, 4, 1, 32, len(cursor), 7)
    struct.pack_into("<IIII", raw, 0x4d0, 0x1400, len(cursor), 0, 0)
    struct.pack_into("<IIII", raw, 0x4e0, 0x1500, len(group), 0, 0)
    raw[0x600:0x600 + len(cursor)] = cursor
    raw[0x700:0x700 + len(group)] = group
    return raw, dib


class StaticPeTests(unittest.TestCase):
    def test_architecture_and_native_classification_come_from_headers(self):
        for bits, machine in ((32, "x86"), (64, "x64")):
            with self.subTest(bits=bits):
                raw, _ = pe_fixture(bits)
                result = inspect_pe(raw)
                self.assertEqual(result["kind"], "native")
                self.assertEqual(result["architecture"], machine)
                self.assertEqual(result["unresolved"], [])

    def test_ia64_machine_is_distinct_from_x64(self):
        raw, _ = pe_fixture()
        struct.pack_into("<H", raw, 0x84, 0x200)
        result = inspect_pe(raw)
        self.assertEqual((result["machine"], result["architecture"]), ("0x200", "ia64"))

    def test_import_name_and_ordinal_are_decoded_with_pointer_width(self):
        for bits in (32, 64):
            with self.subTest(bits=bits):
                raw, directory = pe_fixture(bits)
                struct.pack_into("<II", raw, directory + 8, 0x1100, 40)
                struct.pack_into("<IIIII", raw, 0x300, 0x1180, 0, 0, 0x1160, 0x1180)
                raw[0x360:0x36d] = b"KERNEL32.dll\0"
                struct.pack_into("<QQQ" if bits == 64 else "<III", raw, 0x380, 0x11c0, (1 << (bits - 1)) | 9, 0)
                raw[0x3c2:0x3ca] = b"WinExec\0"
                result = inspect_pe(raw)
                self.assertEqual(result["imports"], [{"library": "KERNEL32.dll", "symbols": [{"name": "WinExec"}, {"ordinal": 9}]}])

    def test_exports_distinguish_code_rva_and_forwarder(self):
        raw, directory = pe_fixture()
        struct.pack_into("<II", raw, directory, 0x1200, 0x100)
        struct.pack_into("<IIHHIIIIIII", raw, 0x400, 0, 0, 0, 0, 0, 1, 2, 1, 0x1250, 0x1260, 0x1270)
        struct.pack_into("<II", raw, 0x450, 0x1700, 0x1280)
        struct.pack_into("<I", raw, 0x460, 0x1290)
        struct.pack_into("<H", raw, 0x470, 0)
        raw[0x480:0x48d] = b"OTHER.Target\0"
        raw[0x490:0x49a] = b"DoRestart\0"
        exports = inspect_pe(raw)["exports"]
        self.assertEqual(exports[0], {"ordinal": 1, "names": ["DoRestart"], "rva": "0x1700"})
        self.assertEqual(exports[1]["forwarder"], "OTHER.Target")

    def test_clr_and_ready_to_run_do_not_enter_pure_native_queue(self):
        for flags, ready, expected in ((1, False, "managed"), (0, False, "mixed_managed_native"), (0xc, True, "managed")):
            with self.subTest(flags=flags, ready=ready):
                raw, directory = pe_fixture()
                struct.pack_into("<II", raw, directory + 14 * 8, 0x1400, 72)
                struct.pack_into("<IHHIIII", raw, 0x600, 72, 2, 5, 0x1460, 0x40, flags, 0)
                raw[0x660:0x664] = b"BSJB"
                if ready:
                    struct.pack_into("<II", raw, 0x640, 0x1500, 16)
                    struct.pack_into("<IHHII", raw, 0x700, 0x00525452, 9, 1, 0x20, 5)
                result = inspect_pe(raw)
                self.assertEqual(result["kind"], expected)
                self.assertEqual("ready_to_run" in result["clr"], ready)

    def test_bad_clr_metadata_is_rejected_instead_of_routed_as_native(self):
        raw, directory = pe_fixture()
        struct.pack_into("<II", raw, directory + 14 * 8, 0x1400, 72)
        struct.pack_into("<IHHIIII", raw, 0x600, 72, 2, 5, 0x1460, 0x40, 1, 0)
        with self.assertRaisesRegex(ValueError, "valid metadata"):
            inspect_pe(raw)

    def test_invalid_import_rva_remains_explicit(self):
        raw, directory = pe_fixture()
        struct.pack_into("<II", raw, directory + 8, 0xffff0000, 40)
        result = inspect_pe(raw)
        self.assertEqual(result["unresolved"][0]["stage"], "imports")
        self.assertEqual(result["imports"], [])

    def test_truncated_and_non_pe_inputs(self):
        self.assertEqual(inspect_pe(b"\x7fELF"), {"format": "elf", "kind": "other"})
        with self.assertRaisesRegex(ValueError, "exceeds input"):
            inspect_pe(b"MZ")


class Ia64ExceptionRegionTests(unittest.TestCase):
    @staticmethod
    def fixture():
        raw, directory = pe_fixture()
        struct.pack_into("<H", raw, 0x84, 0x200)
        struct.pack_into("<I", raw, 0x98 + 240 + 36, 0x60000020)
        struct.pack_into("<II", raw, directory + 3 * 8, 0x1400, 24)
        struct.pack_into("<IIIIII", raw, 0x600, 0x1100, 0x1180, 0x1500, 0x1200, 0x1240, 0x1508)
        raw[0x300:0x380] = bytes(range(128))
        raw[0x400:0x440] = bytes(range(64))
        raw[0x700:0x710] = bytes.fromhex("03000000010000000300000002000000")
        return raw, directory

    def test_ia64_records_use_three_dword_rvas_and_preserve_original_bytes(self):
        raw, _ = self.fixture()
        result = inspect_pe(raw)
        self.assertEqual(result["unresolved"], [])
        table = result["exception_directory"]
        self.assertTrue(table["validated"])
        self.assertEqual((table["rva"], table["bytes"], table["record_bytes"]), ("0x1400", 24, 12))
        self.assertEqual(table["sha256"], hashlib.sha256(raw[0x600:0x618]).hexdigest())
        self.assertEqual([(x["begin_rva"], x["end_rva"], x["file_offset"], x["unwind_rva"])
                          for x in table["regions"]],
                         [("0x1100", "0x1180", 0x300, "0x1500"), ("0x1200", "0x1240", 0x400, "0x1508")])
        first = table["regions"][0]
        self.assertEqual(first["entry_hex"], "001100008011000000150000")
        self.assertEqual(first["sha256"], hashlib.sha256(bytes(range(128))).hexdigest())
        self.assertEqual(first["unwind_header_hex"], "0300000001000000")
        self.assertFalse(first["unwind_semantics_verified"])

    def test_other_architectures_do_not_use_the_ia64_parser(self):
        raw, _ = self.fixture()
        struct.pack_into("<H", raw, 0x84, 0x8664)
        self.assertNotIn("exception_directory", inspect_pe(raw))

    def test_absent_directory_is_distinct_from_incomplete_or_partial_records(self):
        for address, size, valid in ((0, 0, True), (0x1400, 0, False), (0, 24, False), (0x1400, 23, False),
                                     (0x1df8, 24, False)):
            with self.subTest(address=address, size=size):
                raw, directory = self.fixture()
                struct.pack_into("<II", raw, directory + 3 * 8, address, size)
                result = inspect_pe(raw)
                self.assertEqual(result["exception_directory"]["validated"], valid)
                self.assertEqual(result["exception_directory"]["regions"], [])
                self.assertEqual(bool(result["unresolved"]), not valid)

    def test_bad_later_record_cannot_publish_partial_region_coverage(self):
        for begin, end, unwind in ((0x1201, 0x1240, 0x1508), (0x1200, 0x1241, 0x1508),
                (0x1200, 0x1200, 0x1508), (0x1100, 0x1140, 0x1508), (0x1170, 0x1240, 0x1508),
                (0x2000, 0x2040, 0x1508), (0x1200, 0x1240, 0), (0x1200, 0x1240, 0x1509),
                (0x1200, 0x1240, 0x1e00)):
            with self.subTest(begin=begin, end=end, unwind=unwind):
                raw, _ = self.fixture()
                struct.pack_into("<III", raw, 0x60c, begin, end, unwind)
                result = inspect_pe(raw)
                self.assertFalse(result["exception_directory"]["validated"])
                self.assertEqual(result["exception_directory"]["regions"], [])
                self.assertEqual(result["unresolved"][0]["stage"], "ia64_exception_directory")

    def test_non_executable_or_unbacked_code_is_rejected(self):
        raw, _ = self.fixture()
        struct.pack_into("<I", raw, 0x98 + 240 + 36, 0x40000040)
        result = inspect_pe(raw)
        self.assertFalse(result["exception_directory"]["validated"])
        self.assertIn("executable", result["unresolved"][0]["error"])

    def test_exception_code_table_and_unwind_must_have_unique_file_backing(self):
        for rva, size in ((0x1100, 0x80), (0x1170, 0x30), (0x1400, 24), (0x1500, 8)):
            with self.subTest(rva=hex(rva), size=size):
                raw, _ = self.fixture()
                struct.pack_into("<H", raw, 0x86, 2)
                second = 0x98 + 240 + 40
                struct.pack_into("<8sIIII", raw, second, b".overlap", size, rva, size, 0x900)
                struct.pack_into("<I", raw, second + 36, 0x40000040)
                result = inspect_pe(raw)
                self.assertFalse(result["exception_directory"]["validated"])
                self.assertEqual(result["exception_directory"]["regions"], [])
                self.assertIn("uniquely", result["unresolved"][0]["error"])

    def test_chunk_coverage_keeps_fragment_identity_separate_and_merges_shared_bytes(self):
        raw, _ = self.fixture()
        table = inspect_pe(raw)["exception_directory"]
        base = 0x180000000
        def function(rva, chunks):
            return {"rva": hex(rva), "address": hex(base + rva),
                    "chunks": [{"start": hex(base + a), "end": hex(base + b)} for a, b in chunks]}
        functions = [function(0x1000, [(0x1000, 0x1010), (0x1100, 0x1120), (0x1140, 0x1180)]),
                     function(0x1050, [(0x1050, 0x1060), (0x1140, 0x1180)]),
                     function(0x1200, [(0x1200, 0x1240)])]
        result = native_exception_region_coverage(table, functions, hex(base))
        self.assertEqual((result["region_count"], result["uncovered_region_count"], result["uncovered_bytes"]), (2, 1, 32))
        self.assertEqual(result["regions"][0]["captured_chunk_owners"], ["0x1000", "0x1050"])
        self.assertFalse(result["regions"][0]["matches_captured_entry"])
        self.assertEqual(result["regions"][0]["uncovered"], [{"begin_rva": "0x1120", "end_rva": "0x1140", "bytes": 32}])
        self.assertEqual(result["regions"][0]["covered_bytes"], 96)
        self.assertTrue(result["regions"][1]["matches_captured_entry"])
        self.assertTrue(result["regions"][1]["all_bytes_in_captured_chunks"])
        self.assertFalse(result["function_discovery_complete"])
        functions[0]["chunks"].append({"start": hex(base + 0x1120), "end": hex(base + 0x1140)})
        complete = native_exception_region_coverage(table, functions, base)
        self.assertTrue(complete["all_regions_in_captured_chunks"])
        self.assertFalse(complete["function_discovery_complete"])

    def test_unvalidated_directory_and_wrong_chunk_coordinates_are_rejected(self):
        raw, _ = self.fixture()
        table = inspect_pe(raw)["exception_directory"]
        for changed in (dict(table, validated=False), dict(table, validated=1)):
            with self.assertRaisesRegex(ValueError, "validated"):
                native_exception_region_coverage(changed, [], "0x180000000")
        for function in ({"address": "0x180001000", "rva": "0x1001", "chunks": []},
                         {"address": "0x180001000", "rva": "0x1000", "chunks": [{"start": "0x180001020", "end": "0x180001000"}]}):
            with self.assertRaises(ValueError):
                native_exception_region_coverage(table, [function], "0x180000000")


class EmbeddedPeRangeTests(unittest.TestCase):
    def fixture(self, bits=64, machine=None, signed=False):
        raw, directory = pe_fixture(bits)
        optional = 0x98
        table = optional + (240 if bits == 64 else 224)
        if machine is not None:
            struct.pack_into("<H", raw, 0x84, machine)
        struct.pack_into("<I", raw, optional + 16, 0x1000)
        struct.pack_into("<II", raw, optional + 32, 0x1000, 0x200)
        struct.pack_into("<I", raw, optional + 56, 0x2000)
        struct.pack_into("<I", raw, table + 36, 0x60000020)
        struct.pack_into("<H", raw, optional + 68, 1)
        if signed:
            certificates = (struct.pack("<IHH", 13, 0x200, 2) + b"abcde" + b"\0" * 3
                            + struct.pack("<IHH", 12, 0x200, 2) + b"wxyz" + b"\0" * 4)
            struct.pack_into("<II", raw, directory + 4 * 8, 0x1000, 32)
            raw.extend(certificates)
        return raw, directory, table

    def spec(self, raw, **changes):
        evidence = {"path": ".analysis/results/native/fixture/functions/1000.json", "bytes": 3,
                    "sha256": hashlib.sha256(b"abc").hexdigest()}
        return {"name": "driver.sys", "offset": 0x80, "bytes": len(raw),
            "sha256": hashlib.sha256(raw).hexdigest(), "machine": hex(struct.unpack_from("<H", raw, 0x84)[0]),
            "evidence": [evidence], "basis": "Complete fixture owner declares this exact pointer and length", **changes}

    def inspect(self, raw, **changes):
        return inspect_embedded_pe_ranges(b"p" * 0x80 + raw + b"s" * 0x80, [self.spec(raw, **changes)])[0]

    def test_explicit_length_retains_both_certificates_instead_of_size_of_image(self):
        for bits, machine, architecture in ((32, 0x14c, "x86"), (64, 0x8664, "x64"), (64, 0x200, "ia64")):
            with self.subTest(machine=machine):
                raw, _, _ = self.fixture(bits, machine, signed=True)
                result = self.inspect(raw)
                self.assertEqual((result["offset"], result["bytes"]), (0x80, 0x1020))
                self.assertEqual(result["sha256"], hashlib.sha256(raw).hexdigest())
                self.assertEqual(result["pe"]["architecture"], architecture)
                layout = result["layout"]
                self.assertEqual((layout["image_bytes"], layout["complete_file_bytes"]), (0x2000, 0x1020))
                self.assertEqual((layout["sections"][0]["rva"], layout["sections"][0]["file_offset"]), (0x1000, 0x200))
                certificates = layout["security_directory"]
                self.assertEqual((certificates["offset"], certificates["bytes"]), (0x1000, 32))
                self.assertEqual(certificates["sha256"], hashlib.sha256(raw[0x1000:]).hexdigest())
                self.assertEqual([(c["offset"], c["bytes"], c["aligned_bytes"]) for c in certificates["certificates"]],
                                 [(0x1000, 13, 16), (0x1010, 12, 16)])
                self.assertFalse(certificates["cryptographic_signature_verified"])

    def test_ia64_descriptor_and_global_pointer_do_not_use_x64_entry_rules(self):
        raw, directory, table = self.fixture(machine=0x200)
        struct.pack_into("<I", raw, table + 36, 0x40000040)
        struct.pack_into("<II", raw, directory + 8 * 8, 0x20c000, 0)
        result = self.inspect(raw)
        self.assertEqual(result["layout"]["entrypoint_kind"], "ia64_function_descriptor")
        self.assertEqual(result["layout"]["directories"][8],
                         {"index": 8, "address": 0x20c000, "bytes": 0, "address_kind": "global_pointer"})
        struct.pack_into("<I", raw, directory + 8 * 8 + 4, 4)
        with self.assertRaisesRegex(ValueError, "global-pointer"):
            self.inspect(raw)

    def test_no_review_does_not_discover_mz_or_execute_or_write_anything(self):
        raw, _, _ = self.fixture()
        with patch("subprocess.run", side_effect=AssertionError("No analyzer or target process")), \
                patch.object(Path, "write_bytes", side_effect=AssertionError("Pure parser must not publish")):
            self.assertEqual(inspect_embedded_pe_ranges(b"p" * 0x80 + raw, []), [])
            self.assertEqual(self.inspect(raw)["bytes"], 0x1000)

    def test_hash_machine_and_header_width_must_all_agree(self):
        raw, _, _ = self.fixture()
        for changes in ({"sha256": "0" * 64}, {"machine": "0x14c"}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.inspect(raw, **changes)
        struct.pack_into("<H", raw, 0x84, 0x14c)
        with self.assertRaisesRegex(ValueError, "header width"):
            self.inspect(raw)

    def test_review_schema_and_provenance_cannot_be_omitted_or_escape_workspace(self):
        raw, _, _ = self.fixture()
        cases = [{"basis": " "}, {"evidence": []}, {"offset": True}, {"offset": 0},
                 {"offset": -1}, {"bytes": len(raw) + 0x81}, {"machine": "0X8664"}]
        for path in ("C:/private/review.json", ".work/review.json", ".analysis/../private.json", ".analysis\\review.json"):
            original = dict(self.spec(raw)["evidence"][0], path=path)
            cases.append({"evidence": [original]})
        for changes in cases:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.inspect(raw, **changes)
        parent = b"p" * 0x80 + raw
        missing = self.spec(raw)
        missing.pop("basis")
        with self.assertRaisesRegex(ValueError, "schema"):
            inspect_embedded_pe_ranges(parent, [missing])

    def test_duplicate_names_and_overlapping_reviewed_slices_are_rejected_before_parsing(self):
        raw, _, _ = self.fixture()
        first = self.spec(raw)
        parent = b"p" * 0x80 + raw + b"s" * 0x80
        for other in (dict(first, name="DRIVER.SYS"), dict(first, name="other.sys", offset=0x88)):
            with self.subTest(other=other), self.assertRaisesRegex(ValueError, "collide|overlap"):
                inspect_embedded_pe_ranges(parent, [first, other])
        for name in ("../driver.sys", "C:driver.sys", "dir\\driver.sys", "driver.sys.", "CON.sys", "x\n.sys"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "basename"):
                self.inspect(raw, name=name)

    def test_adjacent_disjoint_reviews_preserve_declared_order_and_copy_provenance(self):
        first, _, _ = self.fixture(32)
        second, _, _ = self.fixture()
        parent = b"p" * 0x80 + first + second
        review = [self.spec(second, name="second.sys", offset=0x1080), self.spec(first, name="first.sys")]
        result = inspect_embedded_pe_ranges(parent, review)
        self.assertEqual([r["name"] for r in result], ["second.sys", "first.sys"])
        review[0]["evidence"][0]["path"] = "changed"
        self.assertEqual(result[0]["review"]["evidence"][0]["path"], ".analysis/results/native/fixture/functions/1000.json")

    def test_dos_pe_header_and_alignment_bounds_are_checked(self):
        original, _, _ = self.fixture()
        cases = []
        for offset, fmt, value in ((0x3c, "<I", len(original) - 4), (0x98 + 60, "<I", 0x100),
                (0x98 + 32, "<I", 0x1800), (0x98 + 36, "<I", 0),
                (0x98 + 56, "<I", 0x1000), (0x98 + 108, "<I", 17)):
            raw = bytearray(original)
            struct.pack_into(fmt, raw, offset, value)
            cases.append(raw)
        raw = bytearray(original)
        raw[0x80:0x84] = b"LE\0\0"
        cases.append(raw)
        for raw in cases:
            with self.subTest(header=raw[0x80:0x100]), self.assertRaises(ValueError):
                self.inspect(raw)

    def test_section_file_and_virtual_ranges_cannot_overlap_or_exceed_boundaries(self):
        original, _, table = self.fixture()
        cases = []
        for offset in (0, 0x1000):
            raw = bytearray(original)
            struct.pack_into("<I", raw, table + 20, offset)
            cases.append(raw)
        for virtual_overlap in (False, True):
            raw = bytearray(original)
            raw.extend(b"\0" * 0x200)
            struct.pack_into("<H", raw, 0x86, 2)
            struct.pack_into("<I", raw, 0x98 + 56, 0x3000)
            struct.pack_into("<8sIIII", raw, table + 40, b".extra\0\0", 0x200,
                             0x1000 if virtual_overlap else 0x2000, 0x200, 0x1000 if virtual_overlap else 0x200)
            cases.append(raw)
        for raw in cases:
            with self.subTest(section=raw[table:table + 80]), self.assertRaisesRegex(ValueError, "overlap|bounds"):
                self.inspect(raw)

    def test_entrypoint_and_directories_cannot_claim_unbacked_or_non_executable_bytes(self):
        raw, directory, table = self.fixture()
        struct.pack_into("<I", raw, table + 36, 0x40000040)
        with self.assertRaisesRegex(ValueError, "entrypoint"):
            self.inspect(raw)
        raw, directory, table = self.fixture()
        struct.pack_into("<I", raw, table + 8, 0x1100)
        struct.pack_into("<I", raw, 0x98 + 56, 0x3000)
        struct.pack_into("<I", raw, 0x98 + 16, 0x1f00)
        with self.assertRaisesRegex(ValueError, "byte backing"):
            self.inspect(raw)
        for address, size in ((0x3000, 40), (0x1100, 0), (0, 40)):
            raw, directory, _ = self.fixture()
            struct.pack_into("<II", raw, directory + 8, address, size)
            with self.subTest(address=address, size=size), self.assertRaises(ValueError):
                self.inspect(raw)

    def test_import_semantic_errors_are_not_promoted_to_empty_valid_metadata(self):
        raw, directory, _ = self.fixture()
        struct.pack_into("<II", raw, directory + 8, 0x1100, 40)
        struct.pack_into("<IIIII", raw, 0x300, 0xffff0000, 0, 0, 0x1160, 0x1180)
        raw[0x360:0x36d] = b"KERNEL32.dll\0"
        with self.assertRaisesRegex(ValueError, "unresolved"):
            self.inspect(raw)

    def test_certificate_overlay_is_mandatory_and_unaccounted_tail_is_not_trimmed(self):
        raw, directory, _ = self.fixture(signed=True)
        for changed in (raw[:0x1000], raw + b"extra"):
            with self.subTest(length=len(changed)), self.assertRaisesRegex(ValueError, "security directory|complete declared"):
                self.inspect(changed)
        for address, size in ((0x1100, 32), (0x1001, 31), (0, 32), (0x1000, 0)):
            changed = bytearray(raw)
            struct.pack_into("<II", changed, directory + 4 * 8, address, size)
            with self.subTest(address=address, size=size), self.assertRaisesRegex(ValueError, "security directory"):
                self.inspect(changed)

    def test_invalid_or_overlapping_certificate_records_are_rejected(self):
        original, directory, _ = self.fixture(signed=True)
        for field, fmt, value in ((0, "<I", 7), (0, "<I", 33), (4, "<H", 0), (6, "<H", 0)):
            raw = bytearray(original)
            struct.pack_into(fmt, raw, 0x1000 + field, value)
            with self.subTest(field=field, value=value), self.assertRaisesRegex(ValueError, "WIN_CERTIFICATE"):
                self.inspect(raw)
        raw, directory, _ = self.fixture()
        struct.pack_into("<II", raw, directory + 4 * 8, 0x200, 16)
        raw[0x200:0x210] = struct.pack("<IHH", 16, 0x200, 2) + b"abcdefgh"
        with self.assertRaisesRegex(ValueError, "security/image.*overlap"):
            self.inspect(raw)

    def test_debug_raw_overlay_is_bounded_and_included_in_complete_extent(self):
        raw, directory, _ = self.fixture()
        raw.extend(b"debug-record")
        struct.pack_into("<II", raw, directory + 6 * 8, 0x1200, 28)
        struct.pack_into("<IIHHIIII", raw, 0x400, 0, 0, 0, 0, 2, 12, 0, 0x1000)
        result = self.inspect(raw)
        self.assertEqual(result["layout"]["complete_file_bytes"], 0x100c)
        self.assertIn({"kind": "debug_data", "type": 2, "offset": 0x1000, "bytes": 12}, result["layout"]["raw_ranges"])
        struct.pack_into("<I", raw, 0x400 + 20, 0x1400)
        with self.assertRaisesRegex(ValueError, "debug RVA"):
            self.inspect(raw)

    def test_coff_symbol_overlay_has_a_bounded_string_table(self):
        raw, _, _ = self.fixture()
        struct.pack_into("<II", raw, 0x80 + 12, 0x1000, 1)
        raw.extend(b"\0" * 18 + struct.pack("<I", 6) + b"x\0")
        result = self.inspect(raw)
        self.assertEqual(result["layout"]["complete_file_bytes"], 0x1018)
        struct.pack_into("<I", raw, 0x1012, 0xffffffff)
        with self.assertRaisesRegex(ValueError, "COFF symbols/string table"):
            self.inspect(raw)


class StaticCursorTests(unittest.TestCase):
    def test_reconstruction_preserves_dib_mask_hotspot_and_resource_identity(self):
        raw, dib = cursor_fixture()
        leaves = pe_resource_entries(raw)
        self.assertEqual([(row["type"], row["name"], row["language"]) for row in leaves], [(1, 7, 1033), (12, 102, 1033)])
        result, = pe_cursor_groups(raw, 102)
        self.assertEqual(result["data"][:22], struct.pack("<HHHBBBBHHII", 0, 2, 1, 2, 2, 0, 0, 1, 0, len(dib), 22))
        self.assertEqual(result["data"][22:], dib)
        self.assertEqual(result["frames"][0]["group_height"], 4)
        self.assertEqual(result["frames"][0]["original"], leaves[0])

    def test_missing_or_foreign_language_frame_is_rejected(self):
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x4a0, 1031)
        with self.assertRaisesRegex(ValueError, "language/identifier"):
            pe_cursor_groups(raw, 102)

    def test_hotspot_and_truncated_and_mask_are_rejected(self):
        raw, _ = cursor_fixture()
        struct.pack_into("<H", raw, 0x600, 2)
        with self.assertRaisesRegex(ValueError, "hotspot"):
            pe_cursor_groups(raw, 102)
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x4d4, 64)
        struct.pack_into("<I", raw, 0x70e, 64)
        with self.assertRaisesRegex(ValueError, "mask length"):
            pe_cursor_groups(raw, 102)

    def test_cyclic_and_out_of_directory_references_are_rejected(self):
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x414, 0x80000000)
        with self.assertRaisesRegex(ValueError, "cyclic"):
            pe_resource_entries(raw)
        raw, _ = cursor_fixture()
        struct.pack_into("<I", raw, 0x4a4, 0x10000)
        with self.assertRaisesRegex(ValueError, "declared bounds"):
            pe_resource_entries(raw)


class HelpSystemConstantEvidenceTests(unittest.TestCase):
    def fixture(self):
        raw, _ = pe_fixture()
        property_guid = uuid.UUID("a45c254e-df1c-4efd-8020-67d146a850e0").bytes_le
        values = [struct.pack("<d", 0.0009765625),
                  property_guid + struct.pack("<I", 3),
                  property_guid + struct.pack("<I", 11),
                  property_guid + struct.pack("<I", 13),
                  uuid.UUID("5b45201d-f2f2-4f3b-85bb-30ff1f953599").bytes_le,
                  uuid.UUID("e6f07b5f-ee97-4a90-b076-33f57bf4eaa7").bytes_le]
        assembly = []
        for index, value in enumerate(values):
            offset = 0x300 + index * 32
            raw[offset:offset + len(value)] = value
            assembly.append({"address": hex(0x180002000 + index * 7),
                             "xrefs_from": [{"to": hex(0x180001100 + index * 32), "code": False}]})
        native = {"complete_functions": [{"artifact": "functions/2000.json",
                                          "capture": {"assembly": assembly}}]}
        return raw, native

    def test_constants_bind_actual_bytes_to_retained_instructions(self):
        raw, native = self.fixture()
        result = help_system_report_constants(raw, native)
        self.assertEqual(len(result), 6)
        scale, = result["memory_scale"]
        self.assertEqual(scale["rva"], "0x1100")
        self.assertEqual(scale["raw_hex"], "000000000000503f")
        self.assertEqual(scale["sha256"], hashlib.sha256(raw[0x300:0x308]).hexdigest())
        self.assertEqual(scale["function"], "functions/2000.json")
        self.assertEqual(scale["instruction"], native["complete_functions"][0]["capture"]["assembly"][0])

    def test_present_bytes_without_data_reference_do_not_prove_usage(self):
        for references in ([], [{"to": "0x180001100", "code": True}]):
            raw, native = self.fixture()
            native["complete_functions"][0]["capture"]["assembly"][0]["xrefs_from"] = references
            with self.subTest(references=references), self.assertRaisesRegex(ValueError, "memory_scale"):
                help_system_report_constants(raw, native)

    def test_changed_data_or_reference_outside_file_backed_section_is_rejected(self):
        raw, native = self.fixture()
        raw[0x300] ^= 1
        with self.assertRaisesRegex(ValueError, "memory_scale"):
            help_system_report_constants(raw, native)
        for address in ("0x180000fff", "0x180001dff", "0x180100000"):
            raw, native = self.fixture()
            native["complete_functions"][0]["capture"]["assembly"][0]["xrefs_from"][0]["to"] = address
            with self.subTest(address=address), self.assertRaisesRegex(ValueError, "memory_scale"):
                help_system_report_constants(raw, native)


class HelpSystemSourceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parser = HostLocalAudit().parser

    def props(self, text):
        tree = self.parser.parse(("const props=" + text + ";").encode())
        self.assertFalse(tree.root_node.has_error)
        return tree.root_node.named_children[0].named_children[0].child_by_field_name("value")

    def test_mounted_capabilities_select_the_actual_basic_or_image_branch(self):
        for text, layout in (("{resetObm:reset,hasSystemInfo:!0}", "basic"),
                             ("{hasSystemInfo:true,hasSystemInfoImage:false}", "basic"),
                             ("{hasSystemInfo:true,hasSystemInfoImage:!0}", "image")):
            with self.subTest(props=text):
                self.assertEqual(help_system_mount_layout(self.props(text)), layout)

    def test_unmounted_dynamic_duplicate_or_spread_capabilities_remain_unresolved(self):
        for text in ("{hasSystemInfo:false}", "{hasSystemInfoImage:true}",
                     "{hasSystemInfo:true,hasSystemInfoImage:runtimeFlag}",
                     "{hasSystemInfo:true,hasSystemInfo:false}",
                     "{hasSystemInfo:true,...other}"):
            with self.subTest(props=text), self.assertRaises(ValueError):
                help_system_mount_layout(self.props(text))

    def declaration(self):
        original = {"url": "https://app-assets.razer.com/files/blade.dll",
                    "bytes": 4096, "sha256": "ab" * 32}
        entry = {"resourceName": "BladeDLL", "resourceVersion": "1.0.50.0",
                 "url": original["url"], "size": 4096, "sha256": "AB" * 32}
        return {"resourceManifest": {"resources": [entry]}}, original

    def test_manifest_selection_binds_actual_payload_identity_and_version(self):
        manifest, original = self.declaration()
        selected = help_system_native_declaration(manifest, original)
        self.assertIs(selected, manifest["resourceManifest"]["resources"][0])
        self.assertEqual(selected["resourceVersion"], "1.0.50.0")

    def test_changed_length_digest_url_or_version_cannot_reuse_native_evidence(self):
        manifest, original = self.declaration()
        for field, value in (("size", 4095), ("size", True), ("sha256", "cd" * 32),
                             ("sha256", "ab"), ("url", "https://invalid.example/other.dll"),
                             ("resourceVersion", ""), ("resourceVersion", None)):
            candidate = copy.deepcopy(manifest)
            candidate["resourceManifest"]["resources"][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                help_system_native_declaration(candidate, original)

    def test_absent_or_ambiguous_native_declaration_is_not_success(self):
        manifest, original = self.declaration()
        entry = manifest["resourceManifest"]["resources"][0]
        for entries in ([], [entry, copy.deepcopy(entry)]):
            with self.subTest(entries=len(entries)), self.assertRaises(ValueError):
                help_system_native_declaration({"resourceManifest": {"resources": entries}}, original)

    def test_help_audit_is_reachable_through_the_maintained_cli_dispatch(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "help-system-info"])
        with patch("host.audit_host_help_system_info", return_value="static receipt") as audit:
            self.assertEqual(audit_host_local(args), "static receipt")
            audit.assert_called_once_with(args)

    def test_product_chroma_audit_is_reachable_through_the_maintained_cli_dispatch(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "product-chroma-observation"])
        with patch("host.audit_product_chroma_observation", return_value="static receipt") as audit:
            self.assertEqual(audit_host_local(args), "static receipt")
            audit.assert_called_once_with(args)


class ChromaDashboardLaunchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        path = analysis.ROOT / ".analysis/results/host-current/local-audit.json"
        if not path.is_file():
            raise unittest.SkipTest("Current host lifecycle evidence is unavailable")
        retained = json.loads(path.read_bytes())["window_lifecycle"]
        originals = {row["original"]["path"].split("/asar/b2ce8c54dc5c/", 1)[1]: row["original"]
            for row in retained["host_sources"]}
        cls.audit = HostLocalAudit()
        cls.report = collect_chroma_dashboard_launch(cls.audit, originals)

    def test_current_mount_reaches_page_restore_and_real_host_result_owners(self):
        result = verify_chroma_dashboard_launch(self.audit, self.report)
        self.assertEqual(result["selected_owners"], 57)
        self.assertFalse(result["target_execution"])
        self.assertEqual(result["runtime_acceptance"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_restore_and_hidden_start_cannot_be_rebound_to_another_complete_owner(self):
        for role in ("handleOnUserRestoreWindow", "handleAppHiddenStart", "host_storage", "entry_mount",
                     "user_gate_class", "installation_wrapper", "lazy_component", "content_page"):
            with self.subTest(role=role):
                changed = copy.deepcopy(self.report)
                changed["selected"][role] = changed["selected"]["render"]
                with self.assertRaisesRegex(ValueError, "complete owner identity differs"):
                    verify_chroma_dashboard_launch(self.audit, changed)

    def test_export_rebinding_does_not_inherit_the_correct_page_body(self):
        changed = copy.deepcopy(self.report)
        changed["exports"]["entry_export"]["binding"]["owner_id"] = changed["selected"]["render"]
        with self.assertRaisesRegex(ValueError, "export was rebound"):
            verify_chroma_dashboard_launch(self.audit, changed)

    def test_missing_host_result_and_cleanup_owners_are_rejected(self):
        for role in ("host_storage", "preload_invoke", "preload_pagehide", "getTabInfo",
                     "user_initial", "root_reducers", "installer_events", "content_window_listener"):
            with self.subTest(role=role):
                changed = copy.deepcopy(self.report)
                del changed["selected"][role]
                with self.assertRaisesRegex(ValueError, "omits a required"):
                    verify_chroma_dashboard_launch(self.audit, changed)

    def test_byte_excerpt_cannot_replace_complete_restore_function(self):
        audit = HostLocalAudit()
        audit.owners = copy.deepcopy(self.audit.owners)
        owner = audit.owners[self.report["selected"]["handleOnUserRestoreWindow"]]
        owner["byte_end"] -= 1
        owner["body"] = owner["body"][:-1]
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        owner["id"] = owner["original"]["sha256"] + ":" + str(owner["byte_start"]) + ":" + str(owner["byte_end"])
        changed = copy.deepcopy(self.report)
        changed["selected"]["handleOnUserRestoreWindow"] = owner["id"]
        audit.owners[owner["id"]] = owner
        with self.assertRaisesRegex(ValueError, "complete original AST"):
            verify_chroma_dashboard_launch(audit, changed)

    def test_changed_publication_body_is_rejected_even_with_recomputed_hash(self):
        audit = HostLocalAudit()
        audit.owners = copy.deepcopy(self.audit.owners)
        owner = audit.owners[self.report["selected"]["handleAppHiddenStart"]]
        owner["body"] = owner["body"].replace('status:"hidden"', 'status:"running"')
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_chroma_dashboard_launch(audit, self.report)

    def test_page_restore_and_memory_publication_do_not_prove_device_or_runtime_success(self):
        for name, value in (("target_execution", True), ("source_chain_complete", True),
                ("actual_dll_comparison", "passed"), ("rust_independent_implementation", "complete"),
                ("ui_backend_wiring", "complete"), ("runtime_acceptance", "passed")):
            with self.subTest(name=name):
                changed = copy.deepcopy(self.report)
                changed[name] = value
                with self.assertRaisesRegex(ValueError, "promotes static evidence"):
                    verify_chroma_dashboard_launch(self.audit, changed)

    def test_outer_gate_distinguishes_first_visible_mount_from_an_existing_hidden_instance(self):
        owners = {role: self.audit.owners[owner]["body"] for role, owner in self.report["selected"].items()}
        self.assertEqual(owners["user_initial"], "X={item:{},isLoggedIn:!1}")
        self.assertIn("user:e.user.item", owners["user_gate_connect"])
        self.assertIn('render(){return this.props.user.id?(0,v.jsx)("span",{children:this.props.renderView()}):null}',
                      owners["user_gate_class"])
        self.assertIn("componentDidUpdate(e){!1===e.shouldLogin&&this.props.shouldLogin!==e.shouldLogin&&",
                      owners["user_gate_class"])
        self.assertNotIn("key:", owners["render"])
        self.assertIn("if(t)return", owners["install_gate"])
        self.assertIn('n.e(4461).then(n.bind(n,34461))', owners["install_gate"])
        self.assertIn("},[])", owners["install_gate"])
        self.assertFalse(self.report["mount_boundaries"]["restore_existing_instance_restarts_install_effect"])

    def test_lexical_aliases_cannot_rebind_the_gate_installer_or_actual_page(self):
        for role, name in (("user_gate_class", "ge"), ("installation_ui", "Y"), ("content_page", "Pa")):
            with self.subTest(role=role):
                changed = copy.deepcopy(self.report)
                changed["bindings"][role]["name"] = name
                with self.assertRaisesRegex(ValueError, "lexical binding was rebound"):
                    verify_chroma_dashboard_launch(self.audit, changed)
        changed = copy.deepcopy(self.report)
        changed["exports"]["installation_wrapper"]["module_id"] = 23322
        with self.assertRaisesRegex(ValueError, "export relation differs"):
            verify_chroma_dashboard_launch(self.audit, changed)

    def test_hidden_key_effect_dependencies_children_and_release_claims_cannot_be_promoted(self):
        for field, value in (
                ("initial_user_has_id", True), ("hidden_state_rekeys_gate", True),
                ("install_effect_dependencies", ["isStartHidden"]),
                ("restore_existing_instance_restarts_install_effect", True),
                ("installation_children", "always"), ("initial_completed_statuses", ["restart-required", "skipped", "completed"]),
                ("installer_removed_events_ignored", False), ("installer_initial_read_canceled_on_cleanup", True),
                ("lazy_effect_dependencies", []), ("lazy_cleanup_cancels_inflight", True),
                ("content_retains_exit_response_disposer", True), ("window_list_hook_returns_cleanup", True),
                ("account_and_online_orchestration_in_product", True), ("runtime_behavior_executed", True),
                ("runtime_behavior_executed", 0)):
            with self.subTest(field=field, value=value):
                changed = copy.deepcopy(self.report)
                changed["mount_boundaries"][field] = value
                with self.assertRaisesRegex(ValueError, "conditional mount or cleanup boundary differs"):
                    verify_chroma_dashboard_launch(self.audit, changed)

    def test_installer_state_and_actual_children_keep_their_separate_consumers(self):
        owners = {role: self.audit.owners[owner]["body"] for role, owner in self.report["selected"].items()}
        self.assertIn("[o,i]=(0,s.useState)(!0)", owners["installation_wrapper"])
        self.assertTrue(owners["installation_wrapper"].endswith(":e.children})"))
        self.assertIn('s({type:i.TM,payload:"completed"})', owners["installer_events"])
        self.assertIn('["restart-required","skipped","completed"].includes(n)', owners["installer_events"])
        self.assertIn('("skipped"!==o&&"completed"!==o||(o="completed")', owners["installer_events"])
        self.assertIn('if("removed"===l)return', owners["installer_events"])
        self.assertIn('()=>{l.A.off("windowstorageevent",n)}', owners["installer_events"])
        self.assertIn("(ja(ka))", owners["content_export"])
        self.assertIn("return Pa(),(0,d.jsx)(e,{...t})", owners["content_wrapper"])

    def test_lazy_cleanup_does_not_replace_actual_content_or_window_subscription_cleanup(self):
        owners = {role: self.audit.owners[owner]["body"] for role, owner in self.report["selected"].items()}
        self.assertIn("()=>{c(null),d(null)}),[t])", owners["lazy_component"])
        self.assertIn('componentWillUnmount(){ee.A.removeWindowStorageItem("dependent-app-status"),'
                      'window.removeEventListener("storage",this.checkStorageChanges)}', owners["content_page"])
        self.assertIn('response("exitApp",', owners["content_page"])
        self.assertIn('response("window-list-change",', owners["content_window_listener"])
        self.assertNotIn("return()=>", owners["content_window_listener"])
        self.assertFalse(self.report["mount_boundaries"]["content_removes_anonymous_dom_listeners"])

    def test_another_retained_duplicate_installer_module_is_not_the_requested_source_owner(self):
        source = next(row for name, row in self.audit.sources.items() if name.endswith("/4234.c5514adb.chunk.js"))
        changed = copy.deepcopy(self.report)
        changed["exports"]["installer_events"] = self.audit.webpack_export(
            "different_installer_factory", [source], 20682, "A")
        with self.assertRaisesRegex(ValueError, "export was rebound"):
            verify_chroma_dashboard_launch(self.audit, changed)


class HostReleaseAcquisitionTests(unittest.TestCase):
    def setUp(self):
        import analysis
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.root_patch = patch.object(analysis, "ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)
        self.index = {"inputs": []}
        self.responses, self.calls = {}, []
        self.calls_lock = threading.Lock()
        self.version = "4.0.896"
        self.release_name = ".analysis/inputs/host-current/releases/4.0.896/acquisition.json"
        self.original_acquisition = b'{"files": [], "retained": "historical host evidence"}\n'
        self.original_inspection = b'{"containers": [], "retained": "historical static evidence"}\n'
        baseline = self.write(".analysis/inputs/host-current/acquisition.json", self.original_acquisition,
                              kind="acquisition_receipt")
        legacy_result = self.root / ".analysis/results/host-current/inspection.json"
        legacy_result.parent.mkdir(parents=True)
        legacy_result.write_bytes(self.original_inspection)
        self.package_bytes = {name: b"MZ current package " + name.encode() for name in
                              ("Razer App Engine", "Razer Synapse", "Razer Chroma")}
        self.resource_bytes = {"Shared": b"current shared resource", "Launcher": b"current launcher resource"}
        self.declarations = {}
        modules = {}
        for name, raw in self.package_bytes.items():
            url = "https://manifest-assets.razersynapse.com/" + name.replace(" ", "") + "-4.0.896.exe"
            declaration = {"name": name, "version": self.version, "download_url": url,
                           "file_size": len(raw), "download_file_checksum": hashlib.md5(raw).hexdigest()}
            modules[name] = declaration
            self.responses[url] = raw
            self.declarations[url] = declaration
        products = [{"name": "Anne", "version": self.version,
                     "modules": [modules["Razer App Engine"], modules["Razer Synapse"]]},
                    {"name": "Chroma", "version": self.version,
                     "modules": [modules["Razer App Engine"], modules["Razer Chroma"]]}]
        documents = []
        for location, resource in (("background-manager", "Shared"), ("synapse/dashboard", "Launcher")):
            url = "https://app-assets.razer.com/" + resource + ".dll"
            raw = self.resource_bytes[resource]
            digest = hashlib.sha256(raw).hexdigest()
            declaration = {"resourceName": resource, "resourceVersion": "1.0.0.0", "url": url,
                           "size": len(raw), "sha256": digest.upper(),
                           "integrity": "sha256-" + base64.b64encode(bytes.fromhex(digest)).decode()}
            self.responses[url] = raw
            self.declarations[url] = declaration
            documents.extend([
                ("https://apps.razer.com/" + location + "/installer-manifest.json",
                 "official_host_release_manifest", {"baseURL": location + "/manifests",
                     "latest": {"version": "1.0.139", "url": "manifest-1.0.139.json"}}),
                ("https://apps.razer.com/" + location + "/manifests/manifest-1.0.139.json",
                 "official_host_resource_manifest", {"resources": [declaration]})])
        discovery = "https://discovery3.razerapi.com/api/v1/endpoints"
        release_url = ("https://manifest3.razerapi.com/api/v1/releases/NewProd/tags/prod/products?"
                       "os=WINDOWS&osver=11&arch=64&mfr=Generic-MFR&model=Generic-MDL0&sku=Generic-SKU&l=en-US")
        documents.extend([(discovery, "official_host_discovery", {"data": {"items": [{"name": "prod", "hash": "NewProd"}]}}),
                          (release_url, "official_host_product_release", {"data": {"items": products}}),
                          ("https://apps.razer.com/background-manager/manifests/manifest-1.0.136.json",
                           "official_host_resource_manifest", {"resources": [{"retained": "previous manifest"}]}),
                          (release_url.replace("NewProd", "OldProd"), "official_host_product_release",
                           {"data": {"items": products}})])
        files, stability = [], []
        for url, kind, document in documents:
            raw = json.dumps(document).encode()
            self.responses[url] = raw
            row = self.blob(url, raw, kind)
            row.update(final_url=url, http_status=200)
            files.extend([{**row, "acquired_at_utc": "2026-10-29T17:00:00+00:00"},
                          {**row, "acquired_at_utc": "2026-10-29T17:00:01+00:00"}])
            stability.append({"url": url, "before_sha256": row["sha256"],
                              "after_sha256": row["sha256"], "unchanged": True})
        self.metadata = {"schema_version": 1, "metadata_only": True, "target_execution": False,
                         "real_device_operations": False, "deployment_metadata_stable": True, "failures": [],
                         "finished_at_utc": "2026-10-29T17:00:02+00:00", "files": files, "stability": stability,
                         "baseline": {key: baseline[key] for key in ("path", "sha256")}, "stable_products": products}
        self.save_metadata()
        self.blob("https://old.razer.invalid/previous-app-engine.exe", self.package_bytes["Razer App Engine"],
                  "official_stable_host_package")
        self.blob("https://app-assets.razer.com/old-shared-url.dll", self.resource_bytes["Shared"], "official_host_artifact")
        self.save_index()

    def write(self, name, raw, **extra):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        row = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), **extra}
        self.index["inputs"] = [item for item in self.index["inputs"] if item["path"] != name] + [row]
        return row

    def blob(self, url, raw, kind):
        digest = hashlib.sha256(raw).hexdigest()
        filename = url.split("?", 1)[0].rsplit("/", 1)[-1]
        return self.write(".analysis/inputs/host-current/blobs/" + digest + "/" + filename,
                          raw, kind=kind, url=url)

    def save_index(self):
        path = self.root / ".analysis/inputs.json"
        path.write_text(json.dumps(self.index), encoding="utf-8")

    def save_metadata(self):
        self.write(".analysis/inputs/host-current/metadata-recheck.json", json.dumps(self.metadata).encode(),
                   kind="deployment_metadata_recheck")
        self.save_index()

    def fetch(self, url, expected=None, kind="official_host_artifact", *, release_declaration=None, cancel_event=None):
        with self.calls_lock:
            self.calls.append(url)
        if cancel_event is not None and cancel_event.is_set():
            raise concurrent.futures.CancelledError()
        raw = self.responses[url]
        if release_declaration is not None:
            _host_release_declaration(raw, release_declaration["declaration"], release_declaration["algorithm"])
        row = self.blob(url, raw, kind)
        row.update(final_url=url, http_status=200)
        return raw, row

    def acquire(self):
        result = acquire_host_release(self.fetch, workers=2)
        self.index = json.loads((self.root / ".analysis/inputs.json").read_bytes())
        return result

    def test_current_metadata_requires_every_actual_sample_and_derives_current_closure(self):
        result = host_release_metadata()
        self.assertEqual(result["version"], "4.0.896")
        self.assertEqual(len(result["metadata_files"]), 8)
        self.assertEqual(len(result["active_metadata_files"]), 6)
        self.assertEqual(len(result["declarations"]), 5)
        self.metadata["files"].pop()
        self.save_metadata()
        with self.assertRaisesRegex(ValueError, "two actual"):
            host_release_metadata()

    def test_source_sample_cannot_be_forged_by_a_stability_flag(self):
        self.metadata["files"][0]["sha256"] = "0" * 64
        self.save_metadata()
        with self.assertRaisesRegex(ValueError, "indexed identity"):
            host_release_metadata()

    def test_shared_package_and_resource_reuse_by_verified_bytes_and_complete_release_is_idempotent(self):
        result = self.acquire()
        self.assertTrue(result["complete"])
        self.assertTrue(result["source_current_during_acquisition"])
        self.assertFalse(result["host_package_closure_complete"])
        self.assertEqual(sum(row["acquisition"] == "downloaded" for row in result["files"]), 3)
        self.assertEqual(sum(row["acquisition"] == "reused_verified_bytes" for row in result["files"]), 2)
        self.assertEqual(len(self.calls), 9)
        self.assertEqual((self.root / ".analysis/inputs/host-current/acquisition.json").read_bytes(), self.original_acquisition)
        self.assertEqual((self.root / ".analysis/results/host-current/inspection.json").read_bytes(), self.original_inspection)
        verified = verified_host_release_receipt()
        self.assertEqual(verified["receipt"]["path"], self.release_name)
        self.assertIn(verified["metadata_snapshot"], verified["verified_dependencies"])
        frozen = (self.root / self.release_name).read_bytes()
        with patch("host.concurrent.futures.ThreadPoolExecutor", side_effect=AssertionError("No new work for complete bytes")):
            self.acquire()
        self.assertEqual(len(self.calls), 9)
        self.assertEqual((self.root / self.release_name).read_bytes(), frozen)

    def test_reuse_cannot_accept_same_url_with_a_changed_checksum(self):
        url = next(url for url, row in self.declarations.items() if row.get("name") == "Razer Synapse")
        old = b"MZ previous Synapse".ljust(len(self.responses[url]), b"x")
        self.blob(url, old, "official_stable_host_package")
        self.save_index()
        result = self.acquire()
        artifact = next(row for row in result["files"] if row["url"] == url)
        self.assertEqual(artifact["acquisition"], "downloaded")
        self.assertEqual(artifact["md5"], hashlib.md5(self.responses[url]).hexdigest())

    def test_corrupt_retained_sha_input_fails_without_downloading_a_replacement(self):
        row = next(row for row in self.index["inputs"] if row.get("url") == "https://app-assets.razer.com/old-shared-url.dll")
        (self.root / row["path"]).write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "byte identity"):
            self.acquire()
        report = json.loads((self.root / self.release_name).read_bytes())
        self.assertFalse(report["complete"])
        self.assertEqual(self.calls, [])

    def test_download_error_keeps_failure_and_rejects_partial_current_release(self):
        url = next(url for url, row in self.declarations.items() if row.get("name") == "Razer Synapse")
        self.responses[url] = b"MZ wrong bytes"
        with self.assertRaises(SystemExit):
            self.acquire()
        report = json.loads((self.root / self.release_name).read_bytes())
        self.assertFalse(report["complete"])
        self.assertTrue(any(row.get("url") == url for row in report["failures"]))
        with self.assertRaisesRegex(ValueError, "did not complete"):
            verified_host_release_receipt()

    def test_deployment_change_during_download_never_publishes_current_complete(self):
        url = "https://discovery3.razerapi.com/api/v1/endpoints"
        self.responses[url] = b'{"data":{"items":[{"name":"prod","hash":"Different"}]}}'
        with self.assertRaises(SystemExit):
            self.acquire()
        report = json.loads((self.root / self.release_name).read_bytes())
        self.assertFalse(report["complete"])
        self.assertTrue(any(row["stage"] == "deployment_revalidation" for row in report["failures"]))

    def test_cancel_records_failure_and_joins_owned_workers(self):
        original = self.fetch

        def interrupt(url, **kwargs):
            if kwargs.get("release_declaration"):
                raise KeyboardInterrupt()
            return original(url, **kwargs)

        with self.assertRaises(KeyboardInterrupt):
            acquire_host_release(interrupt, workers=2)
        report = json.loads((self.root / self.release_name).read_bytes())
        self.assertTrue(report["cancelled"])
        self.assertFalse(report["complete"])
        self.assertTrue(report["failures"])
        self.assertFalse(list(self.root.rglob("*.tmp")))

    def test_missing_snapshot_is_a_failure_after_a_release_receipt_exists(self):
        self.acquire()
        snapshot = self.root / ".analysis/inputs/host-current/releases/4.0.896/metadata-recheck.json"
        snapshot.unlink()
        with self.assertRaisesRegex(ValueError, "snapshot is missing"):
            verified_host_release_receipt()

    def test_current_declarations_cannot_be_replaced_by_an_older_version_receipt(self):
        self.acquire()
        with self.assertRaisesRegex(ValueError, "current stable version"):
            verified_host_release_receipt(".analysis/inputs/host-current/releases/4.0.827/acquisition.json")

    def test_owned_atomic_publication_releases_staging_after_failure_and_cancel(self):
        target = self.root / ".analysis/inputs/host-current/blobs/test/payload.dll"
        with patch("host.os.link", side_effect=OSError("publish failure")):
            with self.assertRaisesRegex(OSError, "publish failure"):
                _publish_host_bytes(target, b"original bytes")
        self.assertFalse(target.exists())
        self.assertFalse(list(target.parent.glob("*.tmp")))
        cancelled = threading.Event()
        cancelled.set()
        with self.assertRaises(concurrent.futures.CancelledError):
            _publish_host_bytes(target, b"original bytes", cancelled)
        self.assertFalse(target.exists())
        self.assertFalse(list(target.parent.glob("*.tmp")))

    def test_release_inspection_preserves_legacy_proofs_and_indexes_immutable_container_receipts(self):
        self.acquire()
        release = verified_host_release_receipt()
        source = next(row for row in release["files"] if row["declaration"].get("name") == "Razer App Engine")
        analyzer = self.root / "7z.exe"
        analyzer.write_bytes(b"static analyzer")
        analyzer.with_name("7z.dll").write_bytes(b"static analyzer library")
        args = argparse.Namespace(acquisition=self.release_name, input=source["path"], seven_zip=str(analyzer), extract=True)

        def run(argv, **_):
            self.assertIn(argv[1], ("i", "l"))
            return subprocess.CompletedProcess(argv, 0 if argv[1] == "i" else 2,
                b"7-Zip fixture\n" if argv[1] == "i" else b"Type = PE\n", b"")

        with patch("host.subprocess.run", side_effect=run), \
                patch("host.setup_factory_static_extraction", return_value=SetupFactoryExtractionTests.decoded()):
            inspect_host(args)
            index = json.loads((self.root / ".analysis/inputs.json").read_bytes())
            outputs = [row for row in index["inputs"] if row.get("container_sha256") == source["sha256"]]
            self.assertEqual(len(outputs), 4)
            proof = outputs[0]["extraction_receipt"]
            self.assertTrue(all(row["extraction_receipt"] == proof for row in outputs))
            self.assertIn("/releases/4.0.896/", proof["path"])
            frozen = (self.root / proof["path"]).read_bytes()
            self.assertEqual(hashlib.sha256(frozen).hexdigest(), proof["sha256"])
            inspect_host(args)
            self.assertEqual((self.root / proof["path"]).read_bytes(), frozen)
            nested = next(row for row in outputs if row["path"].endswith("engine/irsetup.exe"))
            inspect_host(argparse.Namespace(**{**vars(args), "input": nested["path"]}))
            foreign = self.write(".analysis/inputs/host-current/foreign.exe", b"MZ unrelated package",
                                 kind="official_stable_host_package")
            self.index = json.loads((self.root / ".analysis/inputs.json").read_bytes())
            self.index["inputs"].append(foreign)
            self.save_index()
            with self.assertRaisesRegex(ValueError, "does not belong"):
                inspect_host(argparse.Namespace(**{**vars(args), "input": foreign["path"]}))
            (self.root / outputs[0]["path"]).write_bytes(b"corrupt output")
            with self.assertRaisesRegex(ValueError, "byte identity changed"):
                inspect_host(args)
        self.assertEqual((self.root / ".analysis/results/host-current/inspection.json").read_bytes(), self.original_inspection)
        self.assertEqual((self.root / ".analysis/inputs/host-current/acquisition.json").read_bytes(), self.original_acquisition)

    def test_exact_asar_extraction_reuses_its_capture_and_never_touches_other_archives(self):
        payload = b"static original host module"
        digest = hashlib.sha256(payload).hexdigest()
        header = json.dumps({"files": {"entry.js": {"size": len(payload), "offset": "0",
            "integrity": {"algorithm": "SHA256", "hash": digest, "blockSize": len(payload), "blocks": [digest]}}}}).encode()
        raw = struct.pack("<IIII", 4, len(header) + 8, len(header) + 4, len(header)) + header + payload
        sha = hashlib.sha256(raw).hexdigest()
        name = ".analysis/inputs/host-current/extracted/" + sha + "/resources/app.asar"
        self.write(name, raw, kind="static_extracted_host_input")
        self.save_index()
        unrelated = self.root / ".analysis/results/host-current/asar/retained/capture.json"
        unrelated.parent.mkdir(parents=True)
        unrelated.write_bytes(b"previous exact analysis receipt")
        extract_asar(name)
        capture = self.root / ".analysis/results/host-current/asar" / sha / "capture.json"
        frozen = capture.read_bytes()
        extract_asar(name)
        self.assertEqual(capture.read_bytes(), frozen)
        self.assertEqual(unrelated.read_bytes(), b"previous exact analysis receipt")
        self.assertEqual((self.root / ".analysis/inputs/host-current/asar" / sha[:12] / "entry.js").read_bytes(), payload)
        with self.assertRaisesRegex(ValueError, "exact indexed"):
            extract_asar(".analysis/inputs/host-current/extracted/missing.asar")

    def test_release_inspection_reuses_a_complete_legacy_owner_without_reextracting_or_rewriting_it(self):
        self.acquire()
        source = next(row for row in verified_host_release_receipt()["files"]
                      if row["declaration"].get("name") == "Razer App Engine")
        analyzer = self.root / "7z.exe"
        analyzer.write_bytes(b"static analyzer")
        analyzer.with_name("7z.dll").write_bytes(b"static analyzer library")

        def run(argv, **_):
            return subprocess.CompletedProcess(argv, 0 if argv[1] == "i" else 2,
                b"7-Zip fixture\n" if argv[1] == "i" else b"Type = PE\n", b"")

        with patch("host.subprocess.run", side_effect=run), \
                patch("host.setup_factory_static_extraction", return_value=SetupFactoryExtractionTests.decoded()):
            inspect_host(argparse.Namespace(input=source["path"], seven_zip=str(analyzer), extract=True))
        legacy = self.root / ".analysis/results/host-current/inspection.json"
        frozen = legacy.read_bytes()
        release_args = argparse.Namespace(acquisition=self.release_name, input=source["path"], seven_zip=str(analyzer), extract=True)
        with patch("host.subprocess.run", side_effect=lambda argv, **_: subprocess.CompletedProcess(argv, 0, b"7-Zip fixture\n", b"")) as process:
            inspect_host(release_args)
        self.assertEqual([call.args[0][1] for call in process.call_args_list], ["i"])
        self.assertEqual(legacy.read_bytes(), frozen)
        proof = self.root / ".analysis/results/host-current/releases/4.0.896" / source["sha256"] / "inspection.json"
        record = json.loads(proof.read_bytes())["containers"][0]
        self.assertEqual(record["reused_extraction_receipt"]["sha256"], hashlib.sha256(frozen).hexdigest())
        summary = json.loads((proof.parents[1] / "inspection.json").read_bytes())
        self.assertIn(record["reused_extraction_receipt"], summary["verified_dependencies"])
        (self.root / record["files"][0]["path"]).write_bytes(b"corrupt retained bytes")
        with self.assertRaisesRegex(ValueError, "byte identity changed"), \
                patch("host.subprocess.run", return_value=subprocess.CompletedProcess([], 0, b"7-Zip fixture\n", b"")):
            inspect_host(release_args)
        self.assertEqual(legacy.read_bytes(), frozen)

    def test_batch_inspection_keeps_successful_leaf_index_and_partial_failure_owners(self):
        self.acquire()
        release = verified_host_release_receipt()
        failed = next(row for row in release["files"] if row["declaration"].get("name") == "Razer Synapse")
        analyzer = self.root / "7z.exe"
        analyzer.write_bytes(b"static analyzer")
        analyzer.with_name("7z.dll").write_bytes(b"static analyzer library")

        def run(argv, **_):
            return subprocess.CompletedProcess(argv, 0 if argv[1] == "i" else 2,
                b"7-Zip fixture\n" if argv[1] == "i" else b"Type = PE\n", b"")

        def decode(raw):
            if raw == self.package_bytes["Razer Synapse"]:
                target = self.root / ".analysis/inputs/host-current/extracted" / failed["sha256"] / "partial.bin"
                target.parent.mkdir(parents=True)
                target.write_bytes(b"unverified partial payload")
                raise ValueError("synthetic complete decode failure")
            return SetupFactoryExtractionTests.decoded()

        args = argparse.Namespace(acquisition=self.release_name, seven_zip=str(analyzer), extract=True)
        with patch("host.subprocess.run", side_effect=run), patch("host.setup_factory_static_extraction", side_effect=decode):
            with self.assertRaisesRegex(ValueError, "synthetic complete decode failure"):
                inspect_host(args)
        base = self.root / ".analysis/results/host-current/releases/4.0.896"
        summary = json.loads((base / "inspection.json").read_bytes())
        self.assertFalse(summary["complete"])
        self.assertTrue(summary["index_publication_complete"])
        self.assertEqual(len(summary["containers"]), 2)
        self.assertEqual(len(summary["failures"]), 1)
        self.assertEqual(summary["failures"][0]["partial_outputs"][0]["sha256"], hashlib.sha256(b"unverified partial payload").hexdigest())
        index = json.loads((self.root / ".analysis/inputs.json").read_bytes())
        extracted = [row for row in index["inputs"] if row.get("kind") == "static_extracted_host_input"]
        self.assertEqual(len(extracted), 8)
        self.assertFalse(any(row.get("container_sha256") == failed["sha256"] for row in extracted))
        snapshots = {path.as_posix(): path.read_bytes() for path in base.glob("*/inspection.json")}
        with patch("host.subprocess.run", side_effect=run), \
                patch("host.setup_factory_static_extraction", side_effect=AssertionError("Completed leaves must be reused")):
            for artifact in release["files"]:
                if artifact["declaration"].get("name") in {"Razer App Engine", "Razer Chroma"}:
                    inspect_host(argparse.Namespace(**{**vars(args), "input": artifact["path"]}))
        for name, frozen in snapshots.items():
            self.assertEqual(Path(name).read_bytes(), frozen)
        self.assertEqual((self.root / ".analysis/results/host-current/inspection.json").read_bytes(), self.original_inspection)

    def test_batch_cancel_joins_success_and_records_every_requested_row(self):
        self.acquire()
        analyzer = self.root / "7z.exe"
        analyzer.write_bytes(b"static analyzer")
        analyzer.with_name("7z.dll").write_bytes(b"static analyzer library")
        def run(argv, **_):
            return subprocess.CompletedProcess(argv, 0 if argv[1] == "i" else 2,
                b"7-Zip fixture\n" if argv[1] == "i" else b"Type = PE\n", b"")

        def decode(raw):
            if raw == self.package_bytes["Razer Synapse"]:
                raise KeyboardInterrupt()
            return SetupFactoryExtractionTests.decoded()

        with patch("host.subprocess.run", side_effect=run), patch("host.setup_factory_static_extraction", side_effect=decode):
            with self.assertRaises(KeyboardInterrupt):
                inspect_host(argparse.Namespace(acquisition=self.release_name, seven_zip=str(analyzer), extract=True))
        summary = json.loads((self.root / ".analysis/results/host-current/releases/4.0.896/inspection.json").read_bytes())
        self.assertTrue(summary["cancelled"])
        self.assertFalse(summary["complete"])
        self.assertTrue(summary["index_publication_complete"])
        attempted = {row["sha256"] for row in summary["requested_inputs"]}
        observed = set(summary["observed_inputs"])
        rejected = {row["input"]["sha256"] for row in summary["failures"] if "input" in row}
        self.assertEqual(attempted, observed | rejected)
        for proof in (self.root / ".analysis/results/host-current/releases/4.0.896").glob("*/inspection.json"):
            for artifact in json.loads(proof.read_bytes())["containers"][0]["files"]:
                current = json.loads((self.root / ".analysis/inputs.json").read_bytes())
                self.assertTrue(any(row["path"] == artifact["path"] for row in current["inputs"]))

    def test_legacy_dependency_change_before_final_publication_rejects_its_successful_leaf(self):
        self.acquire()
        source = next(row for row in verified_host_release_receipt()["files"]
                      if row["declaration"].get("name") == "Razer App Engine")
        name = ".analysis/inputs/host-current/extracted/" + source["sha256"] + "/one.bin"
        artifact = self.write(name, b"retained complete payload", kind="static_extracted_host_input",
                              container=source["path"], container_sha256=source["sha256"])
        self.save_index()
        listing = self.root / ".analysis/results/host-current" / source["sha256"] / "listing.txt"
        listing.parent.mkdir(parents=True)
        listing.write_bytes(b"exact static listing")
        output = {"path": listing.relative_to(self.root).as_posix(), "bytes": listing.stat().st_size,
                  "sha256": hashlib.sha256(listing.read_bytes()).hexdigest()}
        record = {"input": source["path"], "sha256": source["sha256"], "input_record": source,
                  "extracted": True, "input_bytes_unchanged": True, "extraction_exit_code": 0,
                  "list_exit_code": 0, "listing": output["path"], "files": [artifact]}
        legacy = self.root / ".analysis/results/host-current/inspection.json"
        legacy_raw = json.dumps({"containers": [record], "outputs": [output]}).encode()
        legacy.write_bytes(legacy_raw)
        analyzer = self.root / "7z.exe"
        analyzer.write_bytes(b"static analyzer")
        analyzer.with_name("7z.dll").write_bytes(b"static analyzer library")

        def publish(path, raw, cancel_event=None):
            _publish_host_bytes(path, raw, cancel_event)
            if path.name == "inspection.json":
                legacy.write_bytes(legacy_raw + b" ")

        with patch("host.subprocess.run", return_value=subprocess.CompletedProcess([], 0, b"7-Zip fixture\n", b"")), \
                patch("host._publish_host_bytes", side_effect=publish):
            with self.assertRaisesRegex(ValueError, "byte identity changed"):
                inspect_host(argparse.Namespace(acquisition=self.release_name, input=source["path"],
                                                seven_zip=str(analyzer), extract=True))
        summary = json.loads((self.root / ".analysis/results/host-current/releases/4.0.896/inspection.json").read_bytes())
        self.assertFalse(summary["complete"])
        self.assertEqual(summary["observed_inputs"], [])
        self.assertEqual(len(summary["failures"]), 1)
        index = json.loads((self.root / ".analysis/inputs.json").read_bytes())
        retained = next(row for row in index["inputs"] if row["path"] == artifact["path"])
        self.assertNotIn("extraction_receipt", retained)

    def test_legacy_plain_pe_observation_is_not_a_complete_extraction_and_partial_outputs_remain_rejected(self):
        self.acquire()
        source = next(row for row in verified_host_release_receipt()["files"]
                      if row["declaration"].get("name") == "Razer App Engine")
        legacy = self.root / ".analysis/results/host-current/inspection.json"
        old = {"input": source["path"], "sha256": source["sha256"], "input_record": source,
               "extracted": False, "list_exit_code": 0, "formats": ["PE"],
               "extraction_status": "no supported embedded archive detected; PE sections alone do not prove payload extraction"}
        frozen = json.dumps({"containers": [old], "outputs": []}).encode()
        legacy.write_bytes(frozen)
        analyzer = self.root / "7z.exe"
        analyzer.write_bytes(b"static analyzer")
        analyzer.with_name("7z.dll").write_bytes(b"static analyzer library")
        args = argparse.Namespace(acquisition=self.release_name, input=source["path"], seven_zip=str(analyzer), extract=True)

        def run(argv, **_):
            self.assertIn(argv[1], ("i", "l"))
            return subprocess.CompletedProcess(argv, 0, b"7-Zip fixture\n" if argv[1] == "i" else b"Type = PE\n", b"")

        summary_path = self.root / ".analysis/results/host-current/releases/4.0.896/inspection.json"
        summary_path.parent.mkdir(parents=True, exist_ok=True)
        original_failures = [{"input": {key: source[key] for key in ("path", "bytes", "sha256")},
                              "error_kind": "ValueError", "error": "retained first complete-extraction rejection"}]
        summary_path.write_text(json.dumps({"containers": [], "failures": original_failures}), encoding="utf-8")
        with patch("host.subprocess.run", side_effect=run) as process, \
                patch("host.setup_factory_static_extraction", return_value=None):
            inspect_host(args)
            summary = json.loads(summary_path.read_bytes())
            self.assertEqual(summary["failures"], [])
            self.assertEqual(summary["previous_failures"], original_failures)
            record = summary["containers"][0]
            self.assertFalse(record["extracted"])
            self.assertIn("no supported embedded archive", record["extraction_status"])
            self.assertNotIn("reused_extraction_receipt", record)
            self.assertEqual([call.args[0][1] for call in process.call_args_list], ["i", "l"])
        destination = self.root / ".analysis/inputs/host-current/extracted" / source["sha256"]
        destination.mkdir(parents=True)
        partial = destination / "unverified.bin"
        partial.write_bytes(b"preexisting unverified partial bytes")
        with patch("host.subprocess.run", side_effect=run) as process:
            with self.assertRaisesRegex(ValueError, "no successful receipt"):
                inspect_host(args)
            self.assertEqual([call.args[0][1] for call in process.call_args_list], ["i"])
        rejected = json.loads(summary_path.read_bytes())
        self.assertFalse(rejected["complete"])
        self.assertEqual(len(rejected["failures"]), 1)
        self.assertEqual(rejected["previous_failures"], original_failures)
        self.assertEqual(rejected["failures"][0]["partial_outputs"][0]["sha256"],
                         hashlib.sha256(partial.read_bytes()).hexdigest())
        self.assertEqual(legacy.read_bytes(), frozen)
        index = json.loads((self.root / ".analysis/inputs.json").read_bytes())
        self.assertFalse(any(row["path"] == partial.relative_to(self.root).as_posix() for row in index["inputs"]))

    def test_cli_selects_separate_acquisition_and_exact_asar(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["acquire-host", "--release-only", "--workers", "2"])
        self.assertTrue(args.release_only)
        args = parser.parse_args(["inspect-host", "--acquisition", self.release_name, "--input", "fixture.exe", "--extract"])
        self.assertEqual(args.acquisition, self.release_name)
        args = parser.parse_args(["analyze-host", "--extract-asar-only", "--input", "fixture.asar"])
        self.assertEqual(args.input, "fixture.asar")
        with self.assertRaisesRegex(ValueError, "separate indexed metadata"):
            acquire_host(argparse.Namespace(release_only=True, release=True))


class SetupFactoryExtractionTests(unittest.TestCase):
    @staticmethod
    def decoded():
        files = []
        for name, data in (("engine/irsetup.exe", bytes(pe_fixture()[0])),
                           ("engine/lua5.3.dll", bytes(pe_fixture()[0])),
                           ("auxiliary/setup.dat", b"local installation script"),
                           ("payload/0000.bin", b"device payload")):
            files.append({"name": name, "data": data, "bytes": len(data),
                          "sha256": hashlib.sha256(data).hexdigest()})
        return {"format": "Setup Factory", "files": files, "complete": True,
                "target_execution": False}

    def test_unrecognized_container_is_not_claimed_as_setup_factory(self):
        self.assertIsNone(setup_factory_static_extraction(bytes(pe_fixture()[0])))
        marker = bytes(value for value in range(0xe0, 0xe8) for _ in range(2))
        with self.assertRaisesRegex(ValueError, "header exceeds input"):
            setup_factory_static_extraction(bytes(pe_fixture()[0]) + marker)

    def test_inspection_extracts_without_invoking_an_unsupported_unpacker_and_verifies_reuse(self):
        import analysis
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / ".analysis/inputs/host-current/setup.exe"
            source.parent.mkdir(parents=True)
            raw = bytes(pe_fixture()[0])
            source.write_bytes(raw)
            row = {"path": source.relative_to(root).as_posix(), "bytes": len(raw),
                   "sha256": hashlib.sha256(raw).hexdigest(), "kind": "official_stable_host_package"}
            (source.parent / "acquisition.json").write_text(json.dumps({"files": [row]}))
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [row]}))
            result = root / ".analysis/results/host-current"
            result.mkdir(parents=True)
            analyzer = root / "7z.exe"
            analyzer.write_bytes(b"static analyzer identity")
            analyzer.with_name("7z.dll").write_bytes(b"static analyzer library identity")
            args = argparse.Namespace(input=row["path"], seven_zip=str(analyzer), extract=True)

            def run(argv, **_):
                self.assertIn(argv[1], ("i", "l"), "Setup Factory must use its source-derived decoder")
                return subprocess.CompletedProcess(argv, 0 if argv[1] == "i" else 2,
                    b"7-Zip fixture\n" if argv[1] == "i" else b"Type = PE\n", b"")

            with patch.object(analysis, "ROOT", root), patch("host.subprocess.run", side_effect=run), \
                    patch("host.setup_factory_static_extraction", return_value=self.decoded()), \
                    patch.object(analysis, "index_files"):
                inspect_host(args)
                report = json.loads((result / "inspection.json").read_text())
                container = report["containers"][0]
                self.assertTrue(container["extracted"])
                self.assertTrue(container["input_bytes_unchanged"])
                self.assertEqual(len(container["files"]), 4)
                inspect_host(args)
                retained = json.loads((result / "inspection.json").read_text())
                self.assertTrue(retained["containers"][0]["retained_outputs_verified"])
                artifact = next(entry for entry in retained["containers"][0]["files"]
                                if entry["path"].endswith("0000.bin"))
                replacement = b"wrong payload"
                (root / artifact["path"]).write_bytes(replacement)
                artifact.update(bytes=len(replacement), sha256=hashlib.sha256(replacement).hexdigest())
                (result / "inspection.json").write_text(json.dumps(retained))
                with self.assertRaisesRegex(ValueError, "differ from decoded source"):
                    inspect_host(args)


class StudioEditingConsumersTests(unittest.TestCase):
    def setUp(self):
        import analysis
        self.audit = HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)
        self.fixture_count = 0

    def javascript(self, text):
        self.fixture_count += 1
        path = self.root / ".analysis/inputs/frontend/fixture" / (str(self.fixture_count) + ".js")
        path.parent.mkdir(parents=True, exist_ok=True)
        raw = text.encode("utf-8")
        path.write_bytes(raw)
        source = {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                  "sha256": hashlib.sha256(raw).hexdigest()}
        return source, self.audit.tree(source).root_node

    def test_mapper_records_only_named_ui_effects_without_adding_engine_only_ids(self):
        from host import studio_editing_effect_catalog
        source, node = self.javascript('function kind(name){let value;switch(name){'
            'case"static":value=6;break;case"wave":value=2;break;case"chroma-generate":value=18;break}return value}')
        catalog = studio_editing_effect_catalog(self.audit, source, node)
        self.assertEqual({name: row["effect_id"] for name, row in catalog.items()},
                         {"static": 6, "wave": 2, "chroma-generate": 18})
        self.assertNotIn(15, [row["effect_id"] for row in catalog.values()])
        self.assertNotIn(17, [row["effect_id"] for row in catalog.values()])
        for row in catalog.values():
            self.assertEqual(self.audit.owners[row["owner"]]["syntax"], "switch_case")

    def test_mapper_rejects_duplicate_ids_dynamic_values_and_default_branches(self):
        from host import studio_editing_effect_catalog
        for cases in ('case"one":v=2;break;case"two":v=2;break;',
                      'case"one":v=getEffect();break;', 'case"one":v=2;break;default:v=99;'):
            with self.subTest(cases=cases):
                source, node = self.javascript('function map(x){let v;switch(x){' + cases + '}return v}')
                with self.assertRaisesRegex(ValueError, "duplicate|nonliteral|dynamic/default"):
                    studio_editing_effect_catalog(self.audit, source, node)

    def test_lazy_routes_resolve_actual_single_and_shared_chunk_loaders(self):
        from host import studio_editing_lazy_routes
        source, node = self.javascript('function sidebar(){return {'
            'static:jsx(Lazy,{lazy:()=>loader.e(435).then(loader.bind(loader,3181))}),'
            'wave:jsx(Lazy,{lazy:()=>Promise.all([loader.e(606),loader.e(285)]).then(loader.bind(loader,1591))})}}')
        routes = studio_editing_lazy_routes(self.audit, source, node, {"static", "wave"})
        self.assertEqual(routes["static"]["module_id"], 3181)
        self.assertEqual(routes["static"]["chunk_ids"], [435])
        self.assertEqual(routes["wave"]["module_id"], 1591)
        self.assertEqual(routes["wave"]["chunk_ids"], [606, 285])

    def test_lazy_routes_do_not_fill_a_missing_panel_from_the_engine_catalog(self):
        from host import studio_editing_lazy_routes
        source, node = self.javascript('function sidebar(){return {static:jsx(Lazy,'
            '{lazy:()=>loader.e(435).then(loader.bind(loader,3181))})}}')
        with self.assertRaisesRegex(ValueError, "do not match"):
            studio_editing_lazy_routes(self.audit, source, node, {"static", "radiate"})

    def test_lazy_routes_reject_duplicate_names(self):
        from host import studio_editing_lazy_routes
        source, node = self.javascript('function sidebar(){return {static:one,static:two}}')
        with self.assertRaisesRegex(ValueError, "duplicate names"):
            studio_editing_lazy_routes(self.audit, source, node, {"static"})

    def test_lazy_routes_reject_a_dynamic_module_or_chunk(self):
        from host import studio_editing_lazy_routes
        for loader in ("loader.e(435).then(loader.bind(loader,target))",
                       "loader.e(chunk).then(loader.bind(loader,3181))"):
            with self.subTest(loader=loader):
                source, node = self.javascript('function sidebar(){return {static:jsx(Lazy,{lazy:()=>'
                                               + loader + '})}}')
                with self.assertRaisesRegex(ValueError, "is dynamic"):
                    studio_editing_lazy_routes(self.audit, source, node, {"static"})

    def test_collector_uses_the_current_route_and_does_not_fall_back_to_old_sources(self):
        from host import collect_studio_editing_consumers
        current = {"name": "latest", "canonical_entry_url": "current", "acquisition": {},
            "deployment": {}, "capture": None, "capture_failures": ["complete_capture_unavailable"],
            "unresolved_acquisition": ["source_map"], "failed_acquisition": [], "candidate_decisions": [], "originals": []}
        with patch("frontend.current_source_selection", return_value=current) as selection, \
                patch.object(self.audit, "source_set") as legacy:
            with self.assertRaisesRegex(ValueError, "Current Studio source is absent"):
                collect_studio_editing_consumers(self.audit)
        self.assertEqual(selection.call_args.args[0], "https://apps.razer.com/synapse/chroma-studio/")
        legacy.assert_not_called()


class HdkRefreshTests(unittest.TestCase):
    def setUp(self):
        import analysis
        self.audit = HostLocalAudit()
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        patched = patch.object(analysis, "ROOT", self.root)
        patched.start()
        self.addCleanup(patched.stop)

    def manifest_fixture(self, *, product_id=3849, stable=True):
        folder = self.root / ".analysis/inputs/frontend/hdk-3849-manifest-current"
        folder.mkdir(parents=True)
        original = folder / "DeviceManifest_3849_0.json"
        manifest = {"productId": product_id, "wiredProductId": 3849, "vendorId": 5426,
            "hasLedStrips": True, "isChromaStudioSupported": True,
            "chromaStudioRegionIdList": list(range(2147483649, 2147483653)),
            "quickEffectOnExit": {"effectId": 3, "params": {}},
            "sendLightingFrameToStorage": False, "isSVGFromMemoryStorage": False,
            "rzDeviceConfig": {"rzDeviceName": "rzDevice25"}, "svgExtraParams": {"hasRefresh": True}}
        original.write_text(json.dumps(manifest), encoding="utf-8")
        def identity(path):
            raw = path.read_bytes()
            return {"path": path.relative_to(self.root).as_posix(), "bytes": len(raw),
                    "sha256": hashlib.sha256(raw).hexdigest()}
        primary = "https://apps.razer.com/synapse/products/3849/mw/lighting-manifests/DeviceManifest_3849_0.json"
        fallback = "https://apps.razer.com/synapse/lighting-engine/manifest/DeviceManifest_3849_0.json"
        source = {**identity(original), "url": fallback, "http_status": 200}
        acquisition = folder / "acquisition.json"
        acquisition.write_text(json.dumps({"entry_url": primary, "source_current_during_acquisition": False,
            "files": [source, {"url": primary, "error": "HTTP Error 404: Not Found"}],
            "stability": [{"url": fallback, "unchanged": stable,
                "before_sha256": source["sha256"], "after_sha256": source["sha256"]}]}), encoding="utf-8")
        (self.root / ".analysis/inputs.json").write_text(json.dumps(
            {"inputs": [identity(acquisition), source]}), encoding="utf-8")
        return original

    def test_manifest_preserves_primary_failure_and_individual_fallback_stability(self):
        from host import hdk_refresh_manifest
        self.manifest_fixture()
        report = hdk_refresh_manifest(self.audit)
        self.assertFalse(report["source_current_during_acquisition"])
        self.assertEqual(report["primary_request"]["error"], "HTTP Error 404: Not Found")
        self.assertTrue(report["fallback_stability"]["unchanged"])
        self.assertTrue(report["manifest"]["svgExtraParams"]["hasRefresh"])
        self.assertEqual(report["manifest"]["chromaStudioRegionIdList"],
                         [2147483649, 2147483650, 2147483651, 2147483652])

    def test_manifest_changed_bytes_cannot_reuse_the_acquisition_identity(self):
        from host import hdk_refresh_manifest
        original = self.manifest_fixture()
        original.write_bytes(original.read_bytes() + b" ")
        with self.assertRaisesRegex(ValueError, "artifact identity differs"):
            hdk_refresh_manifest(self.audit)

    def test_manifest_other_product_cannot_supply_hdk_applicability(self):
        from host import hdk_refresh_manifest
        self.manifest_fixture(product_id=3884)
        with self.assertRaisesRegex(ValueError, "applicability requires review"):
            hdk_refresh_manifest(self.audit)

    def test_unstable_fallback_cannot_be_promoted_by_a_successful_get(self):
        from host import hdk_refresh_manifest
        self.manifest_fixture(stable=False)
        with self.assertRaisesRegex(ValueError, "stability is unverified"):
            hdk_refresh_manifest(self.audit)

    def test_derived_features_include_conditional_writes_instead_of_inventing_an_argb_bridge(self):
        from host import hdk_derived_feature_names
        raw = b'const derive=e=>{const r={};return e.obm&&(r.isObm={yes:{}}),r.useMappingEngine={config:{}},r};'
        declaration = self.audit.parser.parse(raw).root_node.named_children[0].named_children[0]
        self.assertEqual(hdk_derived_feature_names(declaration), ["isObm", "useMappingEngine"])
        changed = raw.replace(b"r.isObm", b"r.isMultiPortDevice")
        declaration = self.audit.parser.parse(changed).root_node.named_children[0].named_children[0]
        self.assertIn("isMultiPortDevice", hdk_derived_feature_names(declaration))

    def test_derived_feature_helper_rejects_indirect_mutation(self):
        from host import hdk_derived_feature_names
        raw = b'const derive=e=>{const r={};return populate(r,e),r.useMappingEngine={},r};'
        declaration = self.audit.parser.parse(raw).root_node.named_children[0].named_children[0]
        with self.assertRaisesRegex(ValueError, "unreviewed mutating call"):
            hdk_derived_feature_names(declaration)

    def test_current_routes_preserve_missing_capture_without_a_legacy_fallback(self):
        from host import collect_hdk_refresh
        current = {"name": "latest", "canonical_entry_url": "current", "acquisition": {},
            "deployment": {}, "capture": None, "capture_failures": ["complete_capture_unavailable"],
            "unresolved_acquisition": ["source_map"], "failed_acquisition": [], "candidate_decisions": [],
            "originals": []}
        with patch.object(self.audit, "current_source_set", return_value=({"name": "engine-latest", "capture": None}, [])) as engine, \
                patch("frontend.current_source_selection", return_value=current) as selection, \
                patch.object(self.audit, "source_set") as legacy:
            with self.assertRaisesRegex(ValueError, "HDK source is absent"):
                collect_hdk_refresh(self.audit)
        engine.assert_called_once_with("synapse-lighting-engine-current")
        self.assertEqual([call.args[0] for call in selection.call_args_list], [
            "https://apps.razer.com/synapse/chroma-studio/", "https://apps.razer.com/synapse/products/3849/mw/"])
        legacy.assert_not_called()

    def publish(self, *, error=None, changed=()):
        from host import audit_hdk_refresh
        observation = {"source_sets": [{"name": "latest", "capture": None,
            "capture_failures": ["complete_capture_unavailable"]}], "selected": {}, "exports": [],
            "remaining_chains": ["Runtime catalog/cache state and DLL/device comparison are unverified"],
            "source_chain_complete": False, "studio_to_hdk_refresh_complete": False,
            "studio_refresh_routing_semantics_complete": True, "manifest_static_applicability_complete": True}
        with patch("host.HostLocalAudit", return_value=self.audit), \
                patch.object(self.audit, "artifact", return_value=b""), \
                patch.object(self.audit, "final_snapshot", return_value=list(changed)), \
                patch("host.collect_hdk_refresh", side_effect=error, return_value=observation), \
                patch("host.publish_host_static_query") as publish, patch("builtins.print"):
            if error or changed:
                with self.assertRaisesRegex(ValueError, "query remains unresolved"):
                    audit_hdk_refresh(argparse.Namespace(query="hdk-refresh"))
            else:
                audit_hdk_refresh(argparse.Namespace(query="hdk-refresh"))
        self.assertEqual(publish.call_args.args[3], "hdk_refresh")
        return publish.call_args.args[0]

    def test_bounded_observation_does_not_claim_the_unresolved_product_chain(self):
        report = self.publish()
        self.assertTrue(report["bounded_observation_complete"])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["studio_to_hdk_refresh_complete"])
        self.assertTrue(report["studio_refresh_routing_semantics_complete"])
        self.assertTrue(report["manifest_static_applicability_complete"])
        self.assertFalse(report["target_execution"])
        self.assertFalse(report["real_device_operations"])
        self.assertEqual(report["runtime_acceptance"], "unverified")
        self.assertEqual(len(report["remaining_chains"]), 1)
        self.assertIsNone(report["source_sets"][0]["capture"])
        self.assertEqual(report["source_sets"][0]["capture_failures"], ["complete_capture_unavailable"])

    def test_collector_failure_is_published_as_a_failed_observation(self):
        report = self.publish(error=ValueError("current source changed"))
        self.assertFalse(report["bounded_observation_complete"])
        self.assertEqual(report["failures"], ["current source changed"])

    def test_source_change_during_query_cannot_publish_a_complete_observation(self):
        report = self.publish(changed=(".analysis/inputs/frontend/latest/main.js",))
        self.assertFalse(report["snapshot_stable"])
        self.assertFalse(report["bounded_observation_complete"])
        self.assertFalse(report["source_chain_complete"])
        self.assertIn("Audit artifact changed during query", report["failures"][0])


class LightingStaticLayerTests(unittest.TestCase):
    def test_current_cleanup_has_distinct_timer_device_and_downstream_boundaries(self):
        from host import collect_lighting_static_layer
        audit = HostLocalAudit()
        report = collect_lighting_static_layer(audit)
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        boundary = report["device_cleanup_boundary"]
        self.assertFalse(boundary["engine_record_owns_physical_device"])
        self.assertTrue(boundary["engine_destroy_waits_for_engine_timer_callbacks"])
        self.assertFalse(boundary["engine_destroy_waits_for_device_work"])
        self.assertTrue(boundary["device_destroy_waits_for_submitted_device_work"])
        self.assertTrue(boundary["device_work_wait_cancels_pending_callbacks"])
        self.assertFalse(boundary["downstream_frame_driver_hid_completion_barrier"])
        slots = {slot["role"]: slot for slot in report["native"]["verified_indirect_slots"]}
        self.assertEqual(slots["device_detach_engine"]["vtable_rva"], "0x1a83c8")
        self.assertEqual(slots["device_detach_engine"]["target_rva"], "0x47830")
        self.assertEqual(slots["device_worker_destructor"]["target_rva"], "0x49100")
        self.assertEqual(slots["engine_remove_device"]["target_rva"], "0x4bbc0")
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(audit.final_snapshot(), [])

    def test_changed_native_wait_semantics_cannot_reuse_reviewed_cleanup(self):
        from host import collect_lighting_static_layer
        audit = HostLocalAudit()
        verify = audit.families.verify_native_functions

        def changed_wait(*args, **kwargs):
            native = verify(*args, **kwargs)
            owner = next(row for row in native["complete_selected_functions"]
                         if row["role"] == "device_work_wait_and_close")
            lines = owner["capture"]["pseudocode"]["lines"]
            wait = next(row for row in lines if "WaitForThreadpoolWorkCallbacks(v2, 1)" in row["text"])
            wait["text"] = wait["text"].replace("(v2, 1)", "(v2, 0)")
            return native

        with patch.object(audit.families, "verify_native_functions", side_effect=changed_wait):
            with self.assertRaisesRegex(ValueError, "requires semantic review: device_work_wait_and_close"):
                collect_lighting_static_layer(audit)

    def test_explicit_query_is_registered(self):
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "lighting-static-layer"])
        self.assertEqual(args.query, "lighting-static-layer")

    def test_missing_or_ambiguous_current_main_is_rejected(self):
        from host import collect_lighting_static_layer
        audit = unittest.mock.Mock()
        for sources in ([], [{"path": "a/main.one.js"}, {"path": "b/main.two.js"}]):
            with self.subTest(sources=sources):
                audit.current_source_set.return_value = ({"name": "current"}, sources)
                with self.assertRaisesRegex(ValueError, "main source is absent or ambiguous"):
                    collect_lighting_static_layer(audit)

    def test_current_deployment_with_unreviewed_owner_cannot_reuse_semantics(self):
        from host import collect_lighting_static_layer
        audit = unittest.mock.Mock()
        audit.current_source_set.return_value = ({"name": "current"}, [{"path": "main.current.js"}])
        audit.webpack_factories.return_value = [(None, None, object())]
        audit.callable_member.return_value = (None, "changed")
        audit.owners = {"changed": {"body_sha256": hashlib.sha256(b"changed source semantics").hexdigest()}}
        with self.assertRaisesRegex(ValueError, "requires semantic review: 5954:initLightingState"):
            collect_lighting_static_layer(audit)
        audit.families.verify_native_functions.assert_not_called()

    def test_changed_snapshot_publishes_failure_without_promoting_observation(self):
        import host
        audit = unittest.mock.Mock()
        audit.a.ROOT = Path("lighting-query-fixture")
        audit.sources, audit.owners, audit.artifacts = {}, {}, {}
        audit.final_snapshot.return_value = ["current-main.js"]
        with patch.object(host, "HostLocalAudit", return_value=audit), \
                patch.object(host, "collect_lighting_static_layer", return_value={}), \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            with self.assertRaisesRegex(ValueError, "query remains unresolved"):
                audit_host_local(argparse.Namespace(query="lighting-static-layer", verify_only=False))
        retained = publish.call_args.args[0]
        self.assertFalse(retained["bounded_observation_complete"])
        self.assertEqual(retained["changed_during_query"], ["current-main.js"])
        self.assertIn("input changed during query", retained["failures"][0])

    def test_publication_preserves_frame_and_other_queries(self):
        import analysis
        import host
        audit = HostLocalAudit()
        with tempfile.TemporaryDirectory() as temporary, patch.object(analysis, "ROOT", Path(temporary)):
            destination = Path(temporary) / ".analysis/results/host-current/local-audit.json"
            destination.parent.mkdir(parents=True)
            previous = {"schema_version": 1, "lighting_frame_lifecycle": {"old_failure": [2, 1]},
                        "dynamic_lighting": {"source_chain_complete": False}}
            destination.write_text(json.dumps(previous), encoding="utf-8")
            for name in ("host.py", "families.py", "analysis.py", "workspace.py"):
                path = Path(temporary) / "tools" / name
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(b"fixture maintenance tool")
            result = {"native": {"complete_selected_functions": []}, "source_chain_complete": False,
                      "target_execution": False, "runtime_acceptance": "unverified", "failures": []}
            with patch.object(host, "HostLocalAudit", return_value=audit), \
                    patch.object(host, "collect_lighting_static_layer", return_value=result), \
                    patch.object(audit.families, "identity", return_value={"path": "tools/host.py"}), \
                    patch("builtins.print"):
                audit_host_local(argparse.Namespace(query="lighting-static-layer", verify_only=False))
            published = json.loads(destination.read_bytes())
            self.assertEqual({key: published[key] for key in previous}, previous)
            self.assertFalse(published["lighting_static_layer"]["source_chain_complete"])
            self.assertEqual(published["lighting_static_layer"]["runtime_acceptance"], "unverified")

    def test_collector_failure_is_retained_before_the_query_fails(self):
        import host
        audit = unittest.mock.Mock()
        audit.a.ROOT = Path("lighting-query-fixture")
        audit.sources, audit.owners, audit.artifacts = {}, {}, {}
        audit.final_snapshot.return_value = []
        with patch.object(host, "HostLocalAudit", return_value=audit), \
                patch.object(host, "collect_lighting_static_layer", side_effect=ValueError("native bytes differ")), \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            with self.assertRaisesRegex(ValueError, "query remains unresolved"):
                audit_host_local(argparse.Namespace(query="lighting-static-layer", verify_only=False))
        retained = publish.call_args.args[0]
        self.assertEqual(retained["failures"], ["native bytes differ"])
        self.assertFalse(retained["bounded_observation_complete"])
        self.assertFalse(retained["source_chain_complete"])


class LightingTimedEffectsTests(unittest.TestCase):
    def test_complete_current_query_preserves_all_values_through_json_publication(self):
        from host import collect_lighting_timed_effects
        audit = HostLocalAudit()
        report = collect_lighting_timed_effects(audit)
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(set(report["frontend_effect_cases"]),
                         {"1", "2", "3", "4", "5", "6", "7", "8", "11", "12", "13", "15", "17", "18", "19"})
        self.assertFalse(report["source_chain_complete"])
        self.assertEqual(audit.final_snapshot(), [])

    def factory_fixture(self, body):
        audit = HostLocalAudit()
        raw = ("class Factory { constructor(){this.addEffect=async()=>{" + body + "};}}").encode()
        source = {"path": ".analysis/inputs/fixture/main.js", "bytes": len(raw),
                  "sha256": hashlib.sha256(raw).hexdigest()}
        audit.raw[source["path"]] = raw
        audit.sources[source["path"]] = source
        member, _ = audit.callable_member("fixture_factory", source, audit.tree(source).root_node, "addEffect")
        return audit, source, member

    def test_direct_effect_cases_exclude_nested_color_switches(self):
        from host import _lighting_timed_effect_cases
        audit, source, member = self.factory_fixture(
            "switch(effectId){case 2:switch(random){case 1:break;case 3:break;}break;case 7:break;}")
        cases = _lighting_timed_effect_cases(audit, source, member)
        self.assertEqual(set(cases), {2, 7})
        self.assertEqual(audit.owners[cases[7]]["body"], "case 7:break;")
        self.assertIn("switch(random)", audit.owners[cases[2]]["body"])

    def test_duplicate_dynamic_and_default_effect_branches_are_rejected(self):
        from host import _lighting_timed_effect_cases
        for body, expected in (
                ("switch(id){case 2:break;case 2:break;}", "duplicate effect"),
                ("switch(id){case selected:break;}", "dynamic effect"),
                ("switch(id){case 2:break;default:break;}", "unreviewed default")):
            with self.subTest(body=body):
                audit, source, member = self.factory_fixture(body)
                with self.assertRaisesRegex(ValueError, expected):
                    _lighting_timed_effect_cases(audit, source, member)

    def test_missing_or_multiple_direct_switches_are_rejected(self):
        from host import _lighting_timed_effect_cases
        for body in ("return;", "switch(id){case 2:break;}switch(other){case 7:break;}"):
            with self.subTest(body=body):
                audit, source, member = self.factory_fixture(body)
                with self.assertRaisesRegex(ValueError, "unique direct effect switch"):
                    _lighting_timed_effect_cases(audit, source, member)

    def constant_fixture(self):
        value = bytes.fromhex("00000000210000004200000064000000")
        raw = b"\0" * 16 + value
        pe = {"sections": [{"rva": 0x1000, "file_offset": 16, "bytes": 16}]}
        return raw, pe, hashlib.sha256(value).hexdigest()

    def test_constants_use_actual_file_offsets_and_reviewed_logical_bytes(self):
        from host import _lighting_timed_constant
        raw, pe, sha = self.constant_fixture()
        result = _lighting_timed_constant(raw, pe, "spectrum_stops", 0x1000, 16, sha)
        self.assertEqual(result["file_offset"], 16)
        self.assertEqual(struct.unpack("<4I", bytes.fromhex(result["raw_hex"])), (0, 33, 66, 100))
        self.assertTrue(result["actual_pe_bytes_verified"])

    def test_changed_or_truncated_constants_cannot_reuse_reviewed_semantics(self):
        from host import _lighting_timed_constant
        raw, pe, sha = self.constant_fixture()
        for changed in (raw[:-1], raw[:17] + b"\x01" + raw[18:]):
            with self.subTest(raw=changed), self.assertRaisesRegex(ValueError, "differs from reviewed"):
                _lighting_timed_constant(changed, pe, "stops", 0x1000, 16, sha)

    def test_constant_spans_cannot_cross_or_alias_pe_sections(self):
        from host import _lighting_timed_constant
        raw, pe, sha = self.constant_fixture()
        for rva, size, sections in ((0x1000, 0, pe["sections"]), (0x1001, 16, pe["sections"]),
                                    (0x1000, 16, pe["sections"] * 2)):
            with self.subTest(rva=rva, size=size, sections=sections), \
                    self.assertRaisesRegex(ValueError, "one file-backed PE section"):
                _lighting_timed_constant(raw, {"sections": sections}, "stops", rva, size, sha)

    def test_missing_or_ambiguous_current_main_is_rejected(self):
        from host import collect_lighting_timed_effects
        audit = unittest.mock.Mock()
        for sources in ([], [{"path": "a/main.one.js"}, {"path": "b/main.two.js"}]):
            with self.subTest(sources=sources):
                audit.current_source_set.return_value = ({"name": "current"}, sources)
                with self.assertRaisesRegex(ValueError, "main source is absent or ambiguous"):
                    collect_lighting_timed_effects(audit)

    def test_new_mapper_bytes_need_review_before_using_old_effect_semantics(self):
        from host import collect_lighting_timed_effects
        audit = unittest.mock.Mock()
        audit.current_source_set.return_value = ({"name": "current"}, [{"path": "main.current.js"}])
        audit.webpack_factories.return_value = [(None, None, object())]
        audit.callable_member.return_value = (None, "changed")
        audit.owners = {"changed": {"body_sha256": hashlib.sha256(b"new mapper semantics").hexdigest()}}
        with self.assertRaisesRegex(ValueError, "requires semantic review: 2578:addEffect"):
            collect_lighting_timed_effects(audit)
        audit.families.verify_native_functions.assert_not_called()


class LightingTypeMediaChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_type_media_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_type_media_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_type_media_claims
        return _verify_lighting_type_media_claims(report)

    def test_actual_media_mount_and_quick_type_conversion_join_both_current_dlls(self):
        report = self.report
        self.assertEqual(report["current_native_release"]["version"], "4.0.896")
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(report["current_ai_release"]["declaration"]["resourceVersion"], "1.0.1.0")
        self.assertEqual(self.verify(report)["actual_mounted_effect_ids"], [18])
        self.assertEqual(self.verify(report)["actual_quick_type_effect_id"], 15)
        self.assertIn("destiny2:80", report["exports"]["quick_effect_enum"]["binding"]["body"])
        self.assertIn("SpecialEdition:15", report["exports"]["native_effect_enum"]["binding"]["body"])
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 70)
        self.assertEqual(len(report["ai_native"]["complete_selected_functions"]), 11)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 7)
        self.assertEqual(len(report["native"]["verified_call_edges"]), 42)
        self.assertEqual(len(report["ai_native"]["verified_call_edges"]), 8)

    def test_type15_fallthrough_is_anchored_in_actual_pe_instructions(self):
        proof = self.report["instruction_evidence"]["type15_fallthrough_and_pointer_overwrite"]["instructions"]
        rows = {row["rva"]: row for row in proof}
        self.assertEqual(rows["0x513c1"]["bytes"], "7516")
        self.assertEqual(rows["0x513c3"]["bytes"], "b980010000")
        self.assertEqual(rows["0x513d4"]["bytes"], "e8d77a0100")
        self.assertEqual(rows["0x513d9"]["bytes"], "b918130000")
        self.assertIn("var_40", rows["0x513f6"]["text"])
        self.assertEqual(int(rows["0x513d4"]["rva"], 16) + len(bytes.fromhex(rows["0x513d4"]["bytes"])), 0x513d9)
        self.assertEqual(rows["0x51402"]["bytes"], "488d05df841500")
        type_boundary = self.report["boundaries"]["type15"]
        self.assertEqual(type_boundary["actual_result"], "E_INVALIDARG")
        self.assertTrue(type_boundary["destiny_pointer_overwritten"])
        self.assertFalse(type_boundary["destiny_object_released_after_overwrite"])
        self.assertFalse(type_boundary["failure_publishes_effect_handle"])

    def test_all_complete_ast_owners_and_snapshot_match_retained_originals(self):
        audit = HostLocalAudit()
        for owner in self.report["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.report["changed_during_query"], [])

    def test_strict_json_preserves_both_global_failure_lists(self):
        report = json.loads(json.dumps(self.report, allow_nan=False, ensure_ascii=False, sort_keys=True))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), report["verification"])
        for name, count in (("native", 10), ("ai_native", 1)):
            native = report[name]
            capture = json.loads(self.audit.artifact(self.audit.a.ROOT / native["capture_receipt"]["path"]))
            self.assertEqual(native["failures"], capture["failures"])
            self.assertEqual(len(native["failures"]), count)
            self.assertEqual(native["selected_pseudocode_failures"], [])
            self.assertTrue(all(row["pseudocode_captured"] and row["input_chunks_verified"] and not row["failures"]
                                for row in native["complete_selected_functions"]))

    def test_missing_mount_or_new_type15_consumer_requires_semantic_review(self):
        from host import _lighting_type_media_scope
        catalog = {"chroma-generate": {"effect_id": 18}}
        routes = {"chroma-generate": {"module_id": 2474, "chunk_ids": [474]}}
        for changed_catalog, changed_routes in (({}, routes), (catalog, {}),
                ({"chroma-generate": {"effect_id": 15}}, routes),
                (catalog, {"chroma-generate": {"module_id": 2474, "chunk_ids": [1474]}})):
            with self.subTest(catalog=changed_catalog, routes=changed_routes), \
                    self.assertRaisesRegex(ValueError, "mounted route requires semantic review"):
                _lighting_type_media_scope(changed_catalog, changed_routes)
        for changed_catalog, changed_routes in (({**catalog, "alias": {"effect_id": 15}}, routes),
                (catalog, {**routes, "destiny2": {"module_id": 111, "chunk_ids": [111]}})):
            with self.assertRaisesRegex(ValueError, "Studio Type15 consumer requires semantic review"):
                _lighting_type_media_scope(changed_catalog, changed_routes)

    def test_file_history_failure_and_remove_semantics_cannot_be_reinterpreted(self):
        for name, value in (("history_trim_condition", "length>=5"), ("pattern_one_probability", 1 / 9),
                ("file_reader_abort_on_switch_or_unmount", True), ("file_reader_stale_generation_guard", True),
                ("file_reader_error_handler", True), ("video_pause_on_remove", True),
                ("native_input_value", "dataURL"), ("removal_action_deletes_file", True),
                ("save_persists_generation_history", True)):
            changed = copy.deepcopy(self.report)
            changed["boundaries"]["controls"][name] = value
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)

    def test_native_parameter_media_and_cleanup_limits_remain_explicit(self):
        for group, name, value in (("type15", "case_sensitive", False),
                ("type15", "destiny_renderer_reachable_through_factory15", True),
                ("parameters", "pattern_required", True), ("parameters", "pattern_range_checked", True),
                ("parameters", "path_copy_result_checked", True), ("parameters", "sampling_rate_positive_guard", True),
                ("parameters", "initialization_calls_shared_base_initialize", True),
                ("parameters", "media_load_result_reaches_initialization_hresult", True),
                ("media", "image_resize_result_checked", True), ("media", "frame_sample_hresult_returned", True),
                ("media", "hardware_copy_checks_row_pitch", True), ("media", "hardware_copy_unmaps_staging_resource", True),
                ("media", "reset_releases_media", True), ("cleanup", "effect_destructor_releases_global_color_generator", True),
                ("cleanup", "global_generator_loader_retains_module_handle_for_free", True)):
            changed = copy.deepcopy(self.report)
            changed["boundaries"][group][name] = value
            with self.subTest(group=group, name=name), self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)

    def test_actual_pe_tables_exports_and_model_resource_are_preserved(self):
        native = self.report["native"]
        self.assertEqual([row["value"] for row in native["actual_blend_modes"]], list(range(15)))
        self.assertEqual([row["name"] for row in native["actual_media_extensions"] if row["value"] == 1],
                         [".mp4", ".mkv", ".avi"])
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in native["actual_blend_modes"]))
        ai = self.report["ai_native"]
        self.assertEqual([row["rva"] for row in ai["actual_exports"]], ["0x2350", "0x2490", "0x24c0"])
        directory, model = ai["verified_data_regions"]
        data = bytes.fromhex(directory["raw_hex"])
        self.assertEqual(struct.unpack_from("<II", data, 16), (10, 0x80000028))
        self.assertEqual(struct.unpack_from("<II", data, 0x38), (100, 0x80000070))
        self.assertEqual(struct.unpack_from("<II", data, 0x80), (1033, 184))
        self.assertEqual(struct.unpack_from("<II", data, 184), (0x556438, 59336))
        self.assertEqual(bytes.fromhex(model["raw_hex"])[4:8], b"TFL3")
        self.assertEqual(model["sha256"], "98f063f9d8a3e6263523afccfbb1e60e0611656f1c5401a3aa88864fd8e042dc")

    def test_model_and_exact_pattern_dependences_cannot_be_promoted_to_complete(self):
        for name in ("model_operator_semantics_complete", "all_pattern_formula_semantics_complete",
                "ai_generate_outer_success_means_inference_succeeded"):
            changed = copy.deepcopy(self.report)
            changed["boundaries"]["generation"][name] = True
            with self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)
        self.assertEqual(len(self.report["remaining_dependencies"]), 3)
        self.assertIn("RVA556438", self.report["remaining_dependencies"][0]["input"])
        self.assertFalse(self.report["verification"]["model_and_all_pattern_semantics_complete"])

    def test_changed_bindings_native_edges_or_model_identity_require_review(self):
        for mutate in (lambda r: r["exports"].pop("encode_path"),
                lambda r: r["bindings"]["media_actions_import"].update(body="f=n(18480)"),
                lambda r: r["native"]["verified_indirect_slots"][1].update(target_rva="0x68ee0"),
                lambda r: r["native"]["verified_call_edges"].pop(),
                lambda r: r["instruction_evidence"]["type15_fallthrough_and_pointer_overwrite"]["instructions"].pop(),
                lambda r: r["current_ai_release"]["declaration"].update(resourceVersion="1.0.0.0"),
                lambda r: r["ai_native"]["actual_exports"][0].update(rva="0x1234"),
                lambda r: r["remaining_dependencies"].clear()):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_rehashed_owner_and_selection_still_require_original_semantic_review(self):
        from host import _lighting_type_media_projection
        changed = copy.deepcopy(self.report)
        owner = next(row for row in changed["complete_source_owners"] if row["id"] == changed["selected"]["media_control"])
        previous = owner["body"]
        owner["body"] = previous.replace('accept:"image/*, .mp4"', 'accept:"*/*"')
        self.assertNotEqual(owner["body"], previous)
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_type_media_projection(changed),
            ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(HostLocalAudit(), owner)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_static_slice_cannot_promote_execution_implementation_or_hide_capture_failures(self):
        for name, value in (("source_chain_complete", True), ("target_execution", True),
                ("product_code_modified", True), ("real_device_operations", True),
                ("actual_dll_comparison", "passed"), ("runtime_acceptance", "passed"),
                ("ui_backend_wiring", "complete"), ("rust_independent_implementation", "complete"),
                ("snapshot_stable", False), ("changed_during_query", ["changed-input"])):
            changed = copy.deepcopy(self.report)
            changed[name] = value
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "static review was promoted"):
                self.verify(changed)
        for name in ("native", "ai_native"):
            for mutate in (lambda r: r[name]["failures"].clear(),
                    lambda r: r[name]["selected_pseudocode_failures"].append({"stage": "selected:hexrays"}),
                    lambda r: r[name].update(all_pseudocode_captured=True)):
                changed = copy.deepcopy(self.report)
                mutate(changed)
                with self.assertRaisesRegex(ValueError, "native capture failure was hidden"):
                    self.verify(changed)


class LightingMediaCalculationsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_media_calculations
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_media_calculations(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_media_calculations
        return _verify_lighting_media_calculations(report)

    def test_current_media_arithmetic_uses_existing_complete_capture_and_real_constants(self):
        report = self.report
        self.assertEqual(report["kind"], "current_lighting_media_calculations")
        self.assertEqual(report["current_native_release"]["version"], "4.0.896")
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 19)
        self.assertEqual(len(report["native"]["verified_call_edges"]), 18)
        constants = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in report["native"]["verified_data_regions"]}
        self.assertEqual(len(constants), 15)
        self.assertEqual(struct.unpack("<d", constants["pi"]), (3.141592653589793,))
        self.assertEqual(struct.unpack("<17I", constants["resize_layout_index"])[4], 5)
        self.assertEqual(constants["resize_layout_channels"][5], 4)
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])

    def test_all_nine_scalar_branches_match_independent_fixed_vectors(self):
        from host import _lighting_media_pattern_value
        vector = self.report["fixed_vectors"]["pattern_uv_quarter_half"]
        actual = [_lighting_media_pattern_value(i, vector["u"], vector["v"]) for i in range(1, 10)]
        self.assertEqual(actual, vector["expected"])
        self.assertEqual(_lighting_media_pattern_value(0, 0.25, 0.5), 0.25)
        self.assertEqual(_lighting_media_pattern_value(99, 0.25, 0.5), 0.25)
        self.assertEqual(_lighting_media_pattern_value(3, 0, 0), -2.0)
        self.assertEqual(_lighting_media_pattern_value(6, 0, 0), 0.5)
        self.assertEqual(_lighting_media_pattern_value(8, 0, 0), 0.5)

    def test_uint64_negative_wrap_speed_sign_and_elapsed_are_not_a_signed_modulo(self):
        from host import _lighting_media_palette_position
        for name in ("phase_negative_one", "phase_positive_speed_reverses", "phase_elapsed_90"):
            vector = self.report["fixed_vectors"][name]
            actual = _lighting_media_palette_position(vector["scalar"], vector["count"], vector["spread"],
                vector["speed"], vector["elapsed"])
            self.assertEqual(actual["remainder"], vector["remainder"])
            if "unsigned_phase" in vector:
                self.assertEqual(actual["unsigned_phase"], vector["unsigned_phase"])
        self.assertEqual(_lighting_media_palette_position(0.2, 5, 1, -1, 0)["remainder"], 1)
        last = _lighting_media_palette_position(19.8, 5, 1, 0, 0)
        self.assertEqual((last["index"], last["next_index"], last["fraction"]), (4, 0, 0.0))
        with self.assertRaisesRegex(ValueError, "finite conversion range"):
            _lighting_media_palette_position(float("inf"), 5, 1, 0, 0)

    def test_sampled_lightness_drives_two_three_stripes_and_point_two_blends(self):
        from host import _lighting_media_blend_choice
        self.assertEqual(_lighting_media_blend_choice(6, 0.1), {"source": "palette", "index": 1, "black_lerp": False})
        self.assertEqual(_lighting_media_blend_choice(7, 0.2), {"source": "palette", "index": 2, "black_lerp": False})
        self.assertEqual(_lighting_media_blend_choice(13, 0.2), {"source": "palette", "index": 2, "black_lerp": True})
        for mode in (10, 12):
            self.assertEqual(_lighting_media_blend_choice(mode, 0.2), {"source": "pattern", "black_lerp": True})
        self.assertEqual(_lighting_media_blend_choice(10, 0.1999), {"source": "pattern", "black_lerp": False})
        for mode in (11, 12):
            self.assertEqual(_lighting_media_blend_choice(mode, 0.1999), {"source": "palette", "index": 0, "black_lerp": False})
        self.assertEqual(_lighting_media_blend_choice(11, 0.2), {"source": "pattern", "black_lerp": False})
        self.assertEqual(_lighting_media_blend_choice(14, 0.3), {"source": "palette", "index": 0, "black_lerp": False})
        self.assertEqual(_lighting_media_blend_choice(14, 0.4), {"source": "pattern", "black_lerp": True})
        for mode in (0, 8, 99):
            self.assertEqual(_lighting_media_blend_choice(mode, 0.4), {"source": "pattern", "black_lerp": False})

    def test_fallback_has_exact_eight_entries_repeated_base_and_actual_plus_one(self):
        from host import _lighting_media_fallback_lightness
        expected = self.report["fixed_vectors"]["fallback_half"]["expected_lightness"]
        actual = _lighting_media_fallback_lightness(0.5)
        for left, right in zip(actual, expected, strict=True):
            self.assertAlmostEqual(left, right, places=7)
        dark, light = _lighting_media_fallback_lightness(0), _lighting_media_fallback_lightness(1)
        self.assertEqual((dark[0], dark[4]), (0, 0))
        self.assertEqual((light[0], light[4]), (1, 1))
        self.assertAlmostEqual(dark[6], 0.95, places=7)
        self.assertAlmostEqual(light[6], 0.95, places=7)
        self.assertAlmostEqual(dark[5], 0.05, places=7)
        self.assertAlmostEqual(light[1], 0.8, places=7)

    def test_true_register_arguments_override_hidden_or_missing_decompiler_parameters(self):
        proof = self.report["instruction_evidence"]
        rows = {row["rva"]: row for row in proof["power_float_and_integer_arguments"]["instructions"]}
        self.assertEqual(rows["0x6f400"]["bytes"], "f20f2aca")
        self.assertEqual(rows["0x6f404"]["bytes"], "f30f5ac0")
        rows = {row["rva"]: row for row in proof["all_nine_pattern_equations"]["instructions"]}
        self.assertEqual(rows["0x6f23f"]["bytes"], "ba02000000")
        rows = {row["rva"]: row for row in proof["sampled_lightness_fifth_argument"]["instructions"]}
        self.assertEqual(rows["0x6f3cd"]["bytes"], "0f28de")
        rows = {row["rva"]: row for row in proof["uint64_phase_and_modulo100"]["instructions"]}
        self.assertEqual(rows["0x6f2ec"]["bytes"], "4c89d0")
        self.assertEqual(rows["0x6f2ef"]["bytes"], "48c1e802")

    def test_procedural_means_and_resize_ownership_cannot_be_promoted(self):
        for group, key, value in (("procedural", "mean_weighted_by_pixel_frequency", True),
                ("procedural", "empty_group_division_guard", True),
                ("procedural", "stable_order_for_all_equal_frequency_groups", True),
                ("image", "resize_allocates_output_in_current_path", True),
                ("image", "resize_caller_checks_result", True),
                ("image", "resize_failure_frees_caller_buffer", True),
                ("image", "resampler_and_codec_internal_semantics_complete", True),
                ("scope", "ai_model_operators_reviewed", True)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][key] = value
            with self.subTest(group=group, key=key), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_selection_bytes_claims_and_remaining_dependencies_require_review(self):
        from host import _lighting_media_calculation_projection
        for mutate in (lambda r: r["native"]["complete_selected_functions"][0].update(pseudocode_text_sha256="0" * 64),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00"),
                lambda r: r["instruction_evidence"]["all_nine_pattern_equations"]["instructions"].clear(),
                lambda r: r["remaining_dependencies"].clear(),
                lambda r: r["fixed_vectors"]["phase_negative_one"].update(remainder=99)):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_media_calculation_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_capture_failures_snapshot_and_runtime_limits_remain_separate(self):
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.report["changed_during_query"], [])
        self.assertEqual(len(self.report["native"]["failures"]), 10)
        self.assertEqual(self.report["native"]["selected_pseudocode_failures"], [])
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("ui_backend_wiring", "verified"),
                ("runtime_acceptance", "verified")):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)

    def test_strict_json_keeps_full_static_selection_and_does_not_claim_codec_closure(self):
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), report["verification"])
        self.assertEqual(report["verification"]["pattern_ids_reviewed"], list(range(1, 10)))
        self.assertFalse(report["verification"]["codec_resampler_complete"])


class LightingMediaResamplerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_media_resampler
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_media_resampler(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_media_resampler
        return _verify_lighting_media_resampler(report)

    def test_current_caller_retains_complete_owners_real_slots_and_constants(self):
        report = self.report
        self.assertEqual(report["kind"], "current_lighting_media_resampler")
        self.assertEqual(report["current_native_release"]["version"], "4.0.896")
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 107)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 61)
        self.assertEqual(len(report["native"]["verified_call_edges"]), 62)
        self.assertEqual(len(report["instruction_evidence"]), 21)
        constants = {row["role"]: bytes.fromhex(row["raw_hex"])
            for row in report["native"]["verified_data_regions"]}
        self.assertEqual(len(constants), 24)
        self.assertEqual(struct.unpack("<6i", constants["working_channels"]), (7, 7, 7, 7, 3, 3))
        self.assertEqual(struct.unpack("<17i", constants["layout_mapping"])[4], 5)
        self.assertEqual(constants["layout_channels"][5], 4)
        self.assertEqual(struct.unpack("<f", constants["linear_input_scale"][:4]), (0.003921568859368563,))
        self.assertEqual(struct.unpack("<8f", constants["output_clamp_and_half"]),
            (255, 255, 255, 255, 0.5, 0.5, 0.5, 0.5))

    def test_auto_kernel_and_extreme_vertical_shrink_modes_follow_actual_source_size(self):
        from host import _lighting_media_resampler_axis
        for row in self.report["fixed_vectors"]["axis_cases"]:
            with self.subTest(row=row):
                axis = _lighting_media_resampler_axis(row["source"], vertical=row["vertical"])
                self.assertEqual((axis["filter"], axis["mode"], axis["maximum_footprint"]),
                    (row["filter"], row["mode"], row["footprint"]))
        self.assertEqual(_lighting_media_resampler_axis(257)["mode"], 2)
        self.assertEqual(_lighting_media_resampler_axis(4096, vertical=True)["mode"], 0)
        for source in (0, -1, True, 32.0):
            with self.subTest(source=source), self.assertRaises(ValueError):
                _lighting_media_resampler_axis(source)

    def test_current_cubic_polynomials_and_box_support_are_not_interchangeable(self):
        from host import _lighting_media_resampler_kernel
        vectors = self.report["fixed_vectors"]
        for filter_id, expected in ((4, vectors["catmull_rom_expected"]), (5, vectors["mitchell_expected"])):
            self.assertEqual([_lighting_media_resampler_kernel(filter_id, x)
                for x in vectors["kernel_coordinates"]], expected)
            self.assertEqual([_lighting_media_resampler_kernel(filter_id, -x)
                for x in vectors["kernel_coordinates"]], expected)
        self.assertEqual(_lighting_media_resampler_kernel(6, 100), 1)
        with self.assertRaisesRegex(ValueError, "outside the current media caller"):
            _lighting_media_resampler_kernel(3, 0.5)

    def test_coefficients_normalize_before_clamp_merging_and_preserve_negative_lobes(self):
        from host import _lighting_media_resampler_clamp_coefficients
        row = self.report["fixed_vectors"]["clamp_after_normalization"]
        self.assertEqual(_lighting_media_resampler_clamp_coefficients(
            row["first"], row["coefficients"], row["source_count"]), row["expected"])
        self.assertNotEqual(row["expected"], [2 / 3, 1 / 3])
        self.assertEqual(_lighting_media_resampler_clamp_coefficients(
            0, [1.125, -0.125], 2), [1.125, -0.125])
        self.assertEqual(_lighting_media_resampler_clamp_coefficients(
            -1, [0.5, 1, 0.5], 1), [1])
        self.assertEqual(_lighting_media_resampler_clamp_coefficients(0, [1e-40, -1e-40], 2), [0, 0])

    def test_exact4_and_nonexact7_use_different_real_float_ranges(self):
        from host import _lighting_media_resampler_decode
        row = self.report["fixed_vectors"]["transparent_color"]
        exact = _lighting_media_resampler_decode(row["rgba"], exact=True)
        ordinary = _lighting_media_resampler_decode(row["rgba"])
        self.assertEqual(exact, row["exact_expected"])
        self.assertEqual(len(ordinary), 7)
        self.assertEqual(ordinary[4:], row["nonexact_appended"])
        self.assertAlmostEqual(ordinary[0], 64 / 255, places=7)
        self.assertGreater(ordinary[0], 0.25)
        self.assertEqual(ordinary[2], 1)
        self.assertNotEqual(exact[:3], ordinary[:3])

    def test_post_alpha_keeps_transparent_rgb_and_uses_premultiplied_half_mix(self):
        from host import _lighting_media_resampler_decode, _lighting_media_resampler_encode
        row = self.report["fixed_vectors"]["transparent_color"]
        self.assertEqual(_lighting_media_resampler_encode(
            _lighting_media_resampler_decode(row["rgba"])), row["encoded"])
        self.assertEqual(_lighting_media_resampler_encode(
            _lighting_media_resampler_decode(row["rgba"], exact=True), exact=True), row["encoded"])
        row = self.report["fixed_vectors"]["premultiplied_half_mix"]
        self.assertEqual(_lighting_media_resampler_encode(row["working"]), row["encoded"])
        self.assertEqual(_lighting_media_resampler_encode([0.25, 0.5, 1, -0.25, 0, 0, 0]),
            [64, 128, 255, 0])
        self.assertEqual(_lighting_media_resampler_encode([-1, 12.49, 12.5, 300], exact=True),
            [0, 12, 13, 255])

    def test_actual_converter_and_kernel_slots_bind_current_gate_branches(self):
        slots = {row["role"]: row for row in self.report["native"]["verified_indirect_slots"]}
        expected = {"nonexact_input": ("0x1ae700", 0, "0x9ffa0"),
            "nonexact_output": ("0x1ae900", 0, "0xa6ad0"),
            "exact_input": ("0x1ae6e0", 8, "0xa0120"), "exact_output": ("0x1ae8e0", 8, "0xa6c40"),
            "current_pre_alpha": ("0x1af020", 0, "0xa9fc0"), "current_post_alpha": ("0x1af050", 0, "0xaa120"),
            "current_clamp_edge0": ("0x1af160", 0, "0xac250"),
            "automatic_filter4": ("0x1aef60", 32, "0xa9b80"),
            "automatic_filter5": ("0x1aef60", 40, "0xa9c00"), "automatic_filter6": ("0x1aef60", 48, "0xa9c90")}
        for name, identity in expected.items():
            with self.subTest(name=name):
                self.assertEqual(tuple(slots[name][key] for key in
                    ("vtable_rva", "slot_byte_offset", "target_rva")), identity)
                self.assertTrue(slots[name]["actual_pe_bytes_verified"])
        proof = self.report["instruction_evidence"]
        self.assertIn("double_box_disables_alpha_expansion", proof)
        self.assertIn("current_nonexact_seven_channel_selection", proof)

    def test_both_scan_paths_keep_all_generated_slots_and_minus8_indirect_dispatch(self):
        slots = self.report["native"]["verified_indirect_slots"]
        for table in ("0x1af180", "0x1af1c0", "0x1af200", "0x1af240"):
            self.assertEqual([row["slot_byte_offset"] for row in slots if row["vtable_rva"] == table],
                list(range(0, 64, 8)))
        self.assertEqual(len([row for row in slots if row["vtable_rva"] == "0x1f5d00"]), 4)
        self.assertEqual(len([row for row in slots if row["vtable_rva"] == "0x1f5ea0"]), 12)
        proof = self.report["instruction_evidence"]
        gather = {row["rva"]: row for row in proof["gather_table_minus8_dispatch"]["instructions"]}
        scatter = {row["rva"]: row for row in proof["scatter_table_minus8_dispatch"]["instructions"]}
        self.assertEqual(gather["0xb1589"]["bytes"], "ff54f0f8")
        self.assertEqual(scatter["0xb4a22"]["bytes"], "ff54f8f8")
        scan = {row["rva"]: row for row in proof["worker_count_r8_preserved_and_scan_dispatch"]["instructions"]}
        self.assertEqual(scan["0x8a3b4"]["bytes"], "0f84166a0200")
        self.assertEqual(scan["0x8a3ba"]["bytes"], "e9b1680200")
        self.assertFalse(any(row["caller_rva"] == "0xb1460" and row["target_rva"] == "0xb1720"
            for row in self.report["native"]["verified_call_edges"]))

    def test_hidden_float_arguments_original_loops_and_free_pointer_are_preserved(self):
        proof = self.report["instruction_evidence"]
        expected = {
            "single_worker_hidden_parameter": {"0x8a2e0": "ba01000000"},
            "gather_actual_kernel_float_arguments": {"0xab594": "0f28cf", "0xab59f": "ffd6"},
            "scatter_actual_kernel_float_arguments": {"0xabff1": "0f28cf", "0xac000": "f30f59c7"},
            "original_prepend_backward_loops": {"0xac188": "ebf6", "0xac1ce": "ebf0"},
            "free_original_pointer_not_aligned_plan": {"0x89fe3": "488b8998010000"}}
        for role, rows in expected.items():
            instructions = {row["rva"]: row for row in proof[role]["instructions"]}
            for rva, expected_bytes in rows.items():
                with self.subTest(role=role, rva=rva):
                    self.assertEqual(instructions[rva]["bytes"], expected_bytes)

    def test_actual_pe_checks_reject_wrong_slot_and_changed_constant_bytes(self):
        from host import _lighting_timed_constant
        original = self.report["native"]["input"]
        with self.assertRaisesRegex(ValueError, "slot target differs"):
            self.audit.families.verify_native_functions(original, {0xa0120: "exact_converter"},
                [{"role": "wrong_exact_slot", "vtable_rva": "0x1ae6e0",
                    "slot_byte_offset": 0, "target_rva": "0xa0120"}])
        raw = self.audit.read(original)
        pe = inspect_pe(raw)
        constant = next(row for row in self.report["native"]["verified_data_regions"]
            if row["role"] == "linear_input_scale")
        changed = bytearray(raw)
        changed[constant["file_offset"]] ^= 1
        with self.assertRaisesRegex(ValueError, "differs from reviewed original bytes"):
            _lighting_timed_constant(bytes(changed), pe, constant["role"],
                int(constant["rva"], 16), constant["bytes"], constant["sha256"])

    def test_current_math_scope_and_ownership_boundaries_cannot_be_broadened(self):
        for group, key, value in (("caller", "variable_decoded_source_dimensions", False),
                ("axes", "extreme_shrink_mode_removed_for_fixed_output", True),
                ("coefficients", "discard_edge_before_normalization", True),
                ("pixels", "double_box_alpha_expansion", True),
                ("pixels", "all_paths_normalize_to_unit_range", True),
                ("pixels", "transparent_rgb_forced_black", True),
                ("scanlines", "float_sum_reassociation_bitwise_equivalent", True),
                ("ownership", "current_output_buffer_is_caller_owned", False),
                ("scope", "all_generic_layouts_types_edges_or_custom_kernels_reviewed", True)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][key] = value
            with self.subTest(group=group, key=key), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_function_chunk_slot_constant_instruction_and_semantic_mutations_fail(self):
        from host import _lighting_media_resampler_projection
        for mutate in (
                lambda r: r["native"]["complete_selected_functions"][0]["verified_original_chunks"][0].update(sha256="0" * 64),
                lambda r: r["native"]["complete_selected_functions"][0].update(pseudocode_text_sha256="0" * 64),
                lambda r: r["native"]["verified_indirect_slots"][0].update(target_rva="0x0"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00"),
                lambda r: r["instruction_evidence"]["gather_table_minus8_dispatch"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(scan_and_pixels="same range for all paths"),
                lambda r: r["fixed_vectors"]["clamp_after_normalization"].update(expected=[2 / 3, 1 / 3]),
                lambda r: r["remaining_dependencies"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_media_resampler_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_failed_capture_snapshot_and_actual_execution_cannot_be_promoted(self):
        self.assertEqual(len(self.report["native"]["failures"]), 10)
        self.assertEqual(self.report["native"]["selected_pseudocode_failures"], [])
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("runtime_acceptance", "verified"),
                ("ui_backend_wiring", "verified"), ("changed_during_query", [{"path": "changed"}])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)

    def test_strict_json_snapshot_and_temporary_lifecycle_remain_honest(self):
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), report["verification"])
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(report["changed_during_query"], [])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(report["maintenance_lifecycle"]["analysis_sessions_created"], [])
        self.assertFalse(report["verification"]["generic_resampler_codec_ai_complete"])
        self.assertFalse(report["fixed_vectors"]["actual_dll_results"])


class LightingPngDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_png_decoder_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_png_decoder_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_png_decoder_chain
        return _verify_lighting_png_decoder_chain(report)

    def test_current_png_entry_complete_owners_bytes_and_real_edges(self):
        report = self.report
        native = report["native"]
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 48)
        self.assertEqual(len(native["verified_call_edges"]), 70)
        functions = {row["rva"]: row for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8b880", "0x8fa10", "0x8fb40", "0x89010", "0x9c8d0",
            "0x9d340", "0x926e0", "0x91d40", "0x16b1c0"} <= functions.keys())
        self.assertNotIn("0x8b910", functions)
        self.assertNotIn("0x98c80", functions)
        self.assertEqual(report["boundaries"]["entry"]["selected_path"], ["0x8b880", "0x8fa10", "0x8fb40"])
        self.assertEqual(report["boundaries"]["entry"]["selected_scan_mode"], 0)
        self.assertEqual(native["selected_pseudocode_failures"], [])

    def test_pe_tables_bind_adam7_filters_huffman_and_packed_scaling(self):
        regions = {row["role"]: bytes.fromhex(row["raw_hex"])
            for row in self.report["native"]["verified_data_regions"]}
        self.assertEqual(len(regions), 13)
        self.assertEqual(regions["first_row_filter_map"], bytes([0, 1, 0, 5, 1]))
        self.assertEqual(regions["packed_gray_scale"], bytes([0, 255, 85, 0, 17, 0, 0, 0, 1]))
        self.assertEqual(regions["code_length_permutation"], bytes([16, 17, 18, 0, 8, 7, 9, 6, 10,
            5, 11, 4, 12, 3, 13, 2, 14, 1, 15]))
        self.assertEqual(regions["fixed_literal_length_sizes"],
            bytes([8] * 144 + [9] * 112 + [7] * 24 + [8] * 8))
        self.assertEqual(regions["fixed_distance_sizes"], bytes([5] * 32))
        self.assertEqual(struct.unpack("<29I", regions["length_base"])[-1], 258)
        self.assertEqual(struct.unpack("<30I", regions["distance_base"])[-1], 24577)
        self.assertEqual(struct.unpack("<30I", regions["distance_extra_bits"])[-1], 13)
        tables = struct.unpack("<31I", regions["adam7_origins_steps_and_padding"])
        self.assertEqual(tables[0:7], (0, 4, 0, 2, 0, 1, 0))
        self.assertEqual(tables[8:15], (0, 0, 4, 0, 2, 0, 1))
        self.assertEqual(tables[16:23], (8, 8, 4, 4, 2, 2, 1))
        self.assertEqual(tables[24:31], (8, 8, 8, 4, 4, 2, 2))

    def test_scanline_vectors_keep_paeth_packed_gray_palette_endian_and_adam7(self):
        from host import _lighting_png_decoder_reconstruct
        for row in self.report["fixed_vectors"]["scanlines"]:
            with self.subTest(name=row["name"]):
                kwargs = {key: row[key] for key in ("width", "height", "channels", "depth",
                    "color_type", "output_channels", "interlace")}
                self.assertEqual(_lighting_png_decoder_reconstruct(bytes.fromhex(row["input_hex"]),
                    **kwargs).hex(), row["expected_hex"])
        self.assertEqual(_lighting_png_decoder_reconstruct(bytes([0, 0xa8]), 5, 1, 1, 1),
            bytes([255, 0, 255, 0, 255]))
        self.assertEqual(_lighting_png_decoder_reconstruct(bytes([0, 0x0f, 0x80]), 3, 1, 1, 4),
            bytes([0, 255, 136]))
        self.assertEqual(_lighting_png_decoder_reconstruct(bytes([0, 0x6c]), 1, 1, 3, 2,
            color_type=2, output_channels=4), bytes([1, 2, 3, 255]))

    def test_all_filters_first_row_mapping_multichannel_stride_and_modulo(self):
        from host import _lighting_png_decoder_reconstruct
        cases = [
            (bytes([0, 10, 20, 30]), 3, 1, 1, bytes([10, 20, 30])),
            (bytes([1, 1, 2, 3, 4, 9, 18, 27, 36]), 2, 1, 4,
                bytes([1, 2, 3, 4, 10, 20, 30, 40])),
            (bytes([2, 10, 20, 30, 2, 1, 2, 3]), 3, 2, 1, bytes([10, 20, 30, 11, 22, 33])),
            (bytes([3, 10, 10, 10]), 3, 1, 1, bytes([10, 15, 17])),
            (bytes([0, 10, 20, 30, 3, 15, 20, 25]), 3, 2, 1, bytes([10, 20, 30, 20, 40, 60])),
            (bytes([4, 10, 10, 10]), 3, 1, 1, bytes([10, 20, 30])),
            (bytes([1, 250, 20]), 2, 1, 1, bytes([250, 14]))]
        for data, width, height, channels, expected in cases:
            with self.subTest(data=data):
                self.assertEqual(_lighting_png_decoder_reconstruct(data, width, height, channels, 8), expected)
        self.assertEqual(_lighting_png_decoder_reconstruct(bytes([0, 9, 99, 88]), 1, 1, 1, 8), b"\x09")
        with self.assertRaisesRegex(ValueError, "not enough pixels"):
            _lighting_png_decoder_reconstruct(bytes([0, 1]), 2, 1, 1, 8)
        with self.assertRaisesRegex(ValueError, "invalid filter"):
            _lighting_png_decoder_reconstruct(bytes([5, 1]), 1, 1, 1, 8)

    def test_transparency_exact_words_and_defined_palette_alpha(self):
        from host import _lighting_png_decoder_finish
        self.assertEqual(_lighting_png_decoder_finish(bytes.fromhex("112233ff112234ff"), 4,
            transparent=[17, 34, 51]).hex(), "11223300112234ff")
        self.assertEqual(_lighting_png_decoder_finish(bytes.fromhex("3412ffff3512ffff"), 2, depth=16,
            transparent=[0x1234]).hex(), "341200003512ffff")
        palette = [[17, 34, 51, 0], [68, 85, 102, 127], [119, 136, 153, 255]]
        self.assertEqual(_lighting_png_decoder_finish(bytes([2, 0, 1]), 1, palette=palette).hex(),
            "778899ff112233004455667f")
        with self.assertRaisesRegex(ValueError, "uninitialized entries"):
            _lighting_png_decoder_finish(bytes([3]), 1, palette=palette)
        self.assertFalse(self.report["boundaries"]["pixels"]["palette_actual_index_checked"])

    def test_cgbi_alpha0_rounding_byte_truncation_and_unforced_selectors(self):
        from host import _lighting_png_decoder_finish
        self.assertEqual(_lighting_png_decoder_finish(bytes.fromhex("20406080"), 4,
            cgbi_convert=True, cgbi_unpremultiply=True).hex(), "bf804080")
        self.assertEqual(_lighting_png_decoder_finish(bytes.fromhex("12345600"), 4,
            cgbi_convert=True, cgbi_unpremultiply=True).hex(), "56341200")
        self.assertEqual(_lighting_png_decoder_finish(bytes.fromhex("02020201"), 4,
            cgbi_convert=True, cgbi_unpremultiply=True).hex(), "fefefe01")
        self.assertEqual(_lighting_png_decoder_finish(bytes.fromhex("12345678"), 4).hex(), "12345678")
        native = self.report["native"]
        region, = native["verified_zero_fill_regions"]
        self.assertEqual(region["loader_initial_bytes_hex"], "0000000000000000")
        self.assertFalse(region["current_runtime_values_observed"])
        self.assertEqual(native["global_selector_setter_entry_references"], {"0x89300": [], "0x89310": []})
        self.assertFalse(self.report["boundaries"]["pixels"]["cgbi_depth16_guard_present"])

    def test_deflate_routes_malformed_limits_and_hidden_growth_arguments(self):
        boundaries = self.report["boundaries"]["inflate"]
        self.assertEqual(set(boundaries["block_types"]), {"0", "1", "2", "3"})
        self.assertEqual(boundaries["dynamic_repeat_codes"], [16, 17, 18])
        self.assertTrue(boundaries["stored_len_nlen_checked"])
        self.assertTrue(boundaries["backreference_cannot_precede_output"])
        self.assertTrue(boundaries["overlapping_copy_preserved"])
        self.assertFalse(boundaries["adler32_verified"])
        self.assertFalse(boundaries["zlib_cinfo_range_checked"])
        evidence = self.report["instruction_evidence"]
        literal = {row["rva"]: row for row in evidence["literal_growth_requests_one_byte"]["instructions"]}
        self.assertEqual(literal["0x9d404"]["bytes"], "41b801000000")
        stored = {row["rva"]: row for row in evidence["stored_growth_preserves_length_in_r8"]["instructions"]}
        self.assertEqual(stored["0x9cc4f"]["text"], "movzx   r8d, word ptr [rsp+48h+var_1C]")
        self.assertTrue(stored["0x9ccb7"]["text"].endswith("sub_18009D7F0"))
        self.assertEqual([row["target_rva"] for row in self.report["native"]["verified_indirect_slots"]],
            ["0x8b3e0", "0x8b400"])

    def test_real_ownership_early_returns_and_prior_reducer_failure_remain_distinct(self):
        owner = self.report["boundaries"]["ownership"]
        for field in ("inflate_failure_frees_current_output", "invalid_filter_releases_scratch",
                "reconstruction_failure_leaves_pixels_for_outer_cleanup",
                "adam7_failure_releases_full_destination",
                "palette_allocation_failure_leaves_indices_for_outer_cleanup",
                "channel_conversion_failure_returns_before_common_exit",
                "channel_conversion_failure_has_already_consumed_input"):
            self.assertTrue(owner[field])
        self.assertFalse(owner["invalid_depth_reachable_after_successful_header"])
        self.assertFalse(owner["separate_async_cancellation_api_observed"])
        self.assertFalse(owner["file_close_and_later_rgba8_reducer_revalidated"])
        self.assertIn("later16to8 failure leak", self.report["reviewed_semantics"]["ownership_and_failure"])

    def test_unclosed_other_formats_implementation_and_product_chains_reach_ledger(self):
        from analysis import semantic_query_gaps
        evidence = ".analysis/results/host-current/local-audit.json"
        findings = [row for row in semantic_query_gaps(self.report, evidence)
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual(len(findings), 3)
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual([row["pointer"] for row in findings], ["/remaining_chains"] * 3)
        self.assertNotIn("remaining_dependencies", self.report)
        self.assertNotIn("PNG8FB40", self.report["remaining_chains"][0]["owner"])

    def test_wrong_format_pixels_cleanup_or_integrity_assumptions_are_rejected(self):
        for group, field, value in (("entry", "selected_path", ["0x8b880", "0x8b910"]),
                ("chunks", "crc_verified", True), ("inflate", "adler32_verified", True),
                ("pixels", "depth16_input_order", "little endian"),
                ("pixels", "palette_actual_index_checked", True),
                ("pixels", "cgbi_depth16_guard_present", True),
                ("ownership", "inflate_failure_frees_current_output", False),
                ("ownership", "channel_conversion_failure_returns_before_common_exit", False),
                ("ownership", "separate_async_cancellation_api_observed", True)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_truncated_owner_edge_table_instruction_or_missing_scope_fails(self):
        from host import _lighting_png_decoder_projection
        for mutate in (lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["complete_selected_functions"][0].update(pseudocode_text_sha256="0" * 64),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x8b910"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="000000000000000000"),
                lambda r: r["instruction_evidence"]["outer_transfer_and_common_cleanup"]["instructions"].clear(),
                lambda r: r["fixed_vectors"]["scanlines"][0].update(expected_hex="00"),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_png_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_static_snapshot_and_strict_json_do_not_claim_native_execution(self):
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("runtime_acceptance", "verified"),
                ("ui_backend_wiring", "verified"), ("changed_during_query", [{"path": "changed"}])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["fixed_vectors"]["actual_dll_results"])
        self.assertFalse(report["verification"]["all_format_decoders_complete"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingPnmDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_pnm_decoder_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_pnm_decoder_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_pnm_decoder_chain
        return _verify_lighting_pnm_decoder_chain(report)

    def test_current_pnm_complete_functions_pe_bytes_and_real_edges(self):
        report = self.report
        native = report["native"]
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 19)
        self.assertEqual(len(native["verified_call_edges"]), 22)
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8e010", "0x9bbc0", "0x9be70", "0x9bfc0", "0x9c090", "0x9c0c0",
            "0x9bdd0", "0x9be30"} <= functions)
        self.assertFalse({"0x8b480", "0x8fb40", "0x8b910", "0x8eda0"} & functions)
        self.assertEqual(native["selected_pseudocode_failures"], [])
        regions = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in native["verified_data_regions"]}
        self.assertEqual(len(regions), 2)
        self.assertEqual(struct.unpack("<3Q", regions["file_callback_table"]),
            (0x18008b3e0, 0x18008b400, 0x18008b440))
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]],
            ["0x8b3e0", "0x8b440"])

    def test_fixed_vectors_preserve_source_quirks_without_native_execution(self):
        from host import _lighting_pnm_decoder_synthetic
        for vector in self.report["fixed_vectors"]["resident_inputs"]:
            actual = _lighting_pnm_decoder_synthetic(bytes.fromhex(vector["input_hex"]))
            for key, expected in vector["expected"].items():
                with self.subTest(name=vector["name"], field=key):
                    self.assertEqual(actual[key], expected)
        self.assertFalse(self.report["fixed_vectors"]["actual_dll_results"])

    def test_p5_p6_header_comments_whitespace_and_magic_without_separator(self):
        from host import _lighting_pnm_decoder_synthetic
        data = b"P6# before width\r\n1\t# before height\n1\v# before maximum\r255\n\x10\x20\x30"
        actual = _lighting_pnm_decoder_synthetic(data)
        self.assertTrue(actual["success"])
        self.assertEqual((actual["width"], actual["height"], actual["channels"]), (1, 1, 3))
        self.assertEqual(actual["rgba8_hex"], "102030ff")
        self.assertEqual(_lighting_pnm_decoder_synthetic(b"P51 1 255\n\x7f")["rgba8_hex"], "7f7f7fff")
        for magic in (b"P2", b"P4", b"P7", b"p5"):
            actual = _lighting_pnm_decoder_synthetic(magic + b"\n1 1\n255\n\x7f")
            self.assertFalse(actual["success"])
            self.assertEqual(actual["raster_offset"], 0)
            self.assertEqual(actual["error_writes"], [])

    def test_maxval_is_depth_threshold_without_rescaling_or_sample_validation(self):
        from host import _lighting_pnm_decoder_synthetic
        for maximum in (b"0", b"x", b"15", b"255"):
            actual = _lighting_pnm_decoder_synthetic(b"P5\n1 1\n" + maximum + b"\n\xff")
            self.assertTrue(actual["success"])
            self.assertEqual(actual["bits"], 8)
            self.assertEqual(actual["raster_hex"], "ff" if maximum != b"x" else "0a")
        actual = _lighting_pnm_decoder_synthetic(b"P5\n1 1\n256\n\x00\x01")
        self.assertEqual((actual["bits"], actual["raster_hex"], actual["rgba8_hex"]),
            (16, "0001", "010101ff"))
        actual = _lighting_pnm_decoder_synthetic(b"P5\n1 1\n65536\n\x00\x01")
        self.assertFalse(actual["success"])
        self.assertEqual(actual["error_writes"], ["max value > 65535"])

    def test_only_one_postmaxval_lookahead_is_discarded(self):
        from host import _lighting_pnm_decoder_synthetic
        for suffix, expected in ((b"\r\n\x7f", "0a"), (b"  \x7f", "20"),
                (b" #comment\n\x7f", "23"), (b"X\x7f", "7f")):
            actual = _lighting_pnm_decoder_synthetic(b"P5\n1 1\n255" + suffix)
            self.assertTrue(actual["success"])
            self.assertEqual(actual["raster_hex"], expected)
        actual = _lighting_pnm_decoder_synthetic(b"P5\n1 1\n255\n\x7f\x42")
        self.assertEqual(actual["raster_hex"], "7f")

    def test_sixteen_bit_raster_remains_raw_before_native_word_high_byte(self):
        from host import _lighting_pnm_decoder_synthetic
        actual = _lighting_pnm_decoder_synthetic(b"P5\n2 1\n65535\n\x12\x34\x56\x78")
        self.assertEqual(actual["raster_hex"], "12345678")
        self.assertEqual(actual["rgba8_hex"], "343434ff787878ff")
        self.assertNotEqual(actual["rgba8_hex"], "121212ff565656ff")
        instructions = self.report["instruction_evidence"]["raw_raster_read_without_swap_or_scale"]["instructions"]
        calls = [row["text"] for row in instructions if row["text"].startswith("call")]
        self.assertEqual(calls, ["call    sub_180091C90"])

    def test_postdigit_overflow_zero_dimensions_and_checked_allocation_limits(self):
        from host import _lighting_pnm_decoder_synthetic
        actual = _lighting_pnm_decoder_synthetic(b"P5\n214748365 1\n255\n\x7f")
        self.assertFalse(actual["success"])
        self.assertEqual(actual["error_writes"], ["integer parse overflow", "invalid width"])
        actual = _lighting_pnm_decoder_synthetic(b"P5\n1 1\n214748364\n\x7f")
        self.assertEqual(actual["error_writes"], ["max value > 65535"])
        for dimensions, error in ((b"0 1", "invalid width"), (b"1 0", "invalid width"),
                (b"16777217 1", "too large"), (b"16777216 16777216", "too large")):
            actual = _lighting_pnm_decoder_synthetic(b"P5\n" + dimensions + b"\n255\n\x7f")
            self.assertFalse(actual["success"])
            self.assertEqual(actual["error_writes"], [error])
        self.assertFalse(self.report["boundaries"]["ownership"]["size_failure_allocates_pixels"])

    def test_short_read_frees_and_real_converter_arguments_keep_shared_ownership(self):
        evidence = self.report["instruction_evidence"]
        rows = {row["rva"]: row for row in evidence["short_raster_read_frees_before_error"]["instructions"]}
        self.assertTrue(rows["0x8e142"]["text"].endswith("sub_18014FBA0"))
        self.assertIn("bad PNM", rows["0x8e147"]["text"])
        word = {row["rva"]: row for row in evidence["word_converter_actual_width_height_channels"]["instructions"]}
        self.assertEqual(word["0x8e101"]["text"], "mov     r9d, [rsi]")
        self.assertEqual(word["0x8e104"]["text"], "mov     ecx, [rsi+4]")
        self.assertEqual(word["0x8e109"]["bytes"], "894c2420")
        self.assertTrue(word["0x8e110"]["text"].endswith("sub_1800910A0"))
        owner = self.report["boundaries"]["ownership"]
        self.assertTrue(owner["conversion_takes_source_ownership"])
        self.assertTrue(owner["conversion_frees_source_on_failure"])
        self.assertFalse(owner["separate_async_cancellation_api_observed"])

    def test_wrong_format_normalization_fallback_or_cleanup_assumptions_fail(self):
        for group, key, value in (("entry", "selected_decoder_rva", "0x8eda0"),
                ("header", "maxval_positive_required", True),
                ("header", "maxval_parse_failure_error_can_survive_success", False),
                ("header", "skip_after_maxval", True), ("pixels", "maxval_rescaling", True),
                ("pixels", "sixteen_bit_input_byte_swap", True),
                ("ownership", "short_read_frees_pixels_before_error", False),
                ("ownership", "conversion_frees_source_on_failure", False)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_owner_byte_edge_semantics_or_dependency_mutations_fail(self):
        from host import _lighting_pnm_decoder_projection
        for mutate in (lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00" * 32),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x8b910"),
                lambda r: r["instruction_evidence"]["numeric_postdigit_overflow_and_consumed_lookahead"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(raster_and_parameters="big endian samples normalized by maxval"),
                lambda r: r["dependencies"][0].update(section="unreviewed_converter"),
                lambda r: r["fixed_vectors"]["resident_inputs"][1]["expected"].update(rgba8_hex="12569aff"),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_pnm_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_remaining_chains_and_static_snapshot_keep_actual_execution_unverified(self):
        from analysis import semantic_query_gaps
        findings = [row for row in semantic_query_gaps(self.report, ".analysis/results/host-current/local-audit.json")
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual(len(findings), 3)
        self.assertEqual([row["pointer"] for row in findings], ["/remaining_chains"] * 3)
        self.assertNotIn("PNM", self.report["remaining_chains"][0]["owner"])
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("ui_backend_wiring", "verified"),
                ("changed_during_query", [{"path": "changed"}])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["verification"]["all_format_decoders_complete"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingBmpTgaDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_bmp_tga_decoder_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_bmp_tga_decoder_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_bmp_tga_decoder_chain
        return _verify_lighting_bmp_tga_decoder_chain(report)

    def test_current_original_complete_functions_edges_slots_and_pe_tables(self):
        native = self.report["native"]
        self.assertEqual(self.report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 25)
        self.assertEqual(len(native["verified_call_edges"]), 41)
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8b910", "0x93fa0", "0x94300", "0x94370", "0x943c0", "0x94400",
            "0x8eda0", "0x9c380", "0x9c3e0"} <= functions)
        self.assertFalse({"0x8b480", "0x8fb40", "0x8e010", "0x98c80"} & functions)
        regions = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in native["verified_data_regions"]}
        self.assertEqual(len(regions), 4)
        self.assertEqual(struct.unpack("<9I", regions["bmp_channel_multipliers"]), (0, 255, 85, 73, 17, 33, 65, 129, 1))
        self.assertEqual(struct.unpack("<9I", regions["bmp_channel_shifts"]), (0, 0, 0, 1, 0, 2, 4, 6, 0))
        self.assertEqual(struct.unpack("<4I", regions["bmp32_default_masks"]),
            (0xff0000, 0xff00, 0xff, 0xff000000))
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]], ["0x8b3e0", "0x8b400"])

    def test_independent_fixed_vectors_keep_bmp_tga_pixel_differences(self):
        from host import _lighting_bmp_tga_decoder_bmp_pixels, _lighting_bmp_tga_decoder_tga_pixels
        for kind, helper in (("bmp_pixels", _lighting_bmp_tga_decoder_bmp_pixels),
                ("tga_pixels", _lighting_bmp_tga_decoder_tga_pixels)):
            for row in self.report["fixed_vectors"][kind]:
                kwargs = {key: value for key, value in row.items()
                    if key not in ("name", "input_hex", "expected_rgba8_hex")}
                with self.subTest(name=row["name"]):
                    self.assertEqual(helper(bytes.fromhex(row["input_hex"]), **kwargs).hex(), row["expected_rgba8_hex"])
        self.assertFalse(self.report["fixed_vectors"]["actual_dll_results"])

    def test_bmp_packed_indices_padding_direction_and_uninitialized_palette_boundary(self):
        from host import _lighting_bmp_tga_decoder_bmp_pixels
        palette = [(255, 0, 0), (0, 255, 0), (0, 0, 255)]
        self.assertEqual(_lighting_bmp_tga_decoder_bmp_pixels(bytes.fromhex("40000000"), 3, -1, 1,
            palette=palette).hex(), "ff0000ff00ff00ffff0000ff")
        self.assertEqual(_lighting_bmp_tga_decoder_bmp_pixels(bytes.fromhex("12000000"), 3, -1, 4,
            palette=palette).hex(), "00ff00ff0000ffffff0000ff")
        self.assertEqual(_lighting_bmp_tga_decoder_bmp_pixels(bytes.fromhex("00010200"), 3, -1, 8,
            palette=palette).hex(), "ff0000ff00ff00ff0000ffff")
        self.assertEqual(_lighting_bmp_tga_decoder_bmp_pixels(bytes.fromhex("010000000200000003000000"),
            1, 3, 24).hex(), "000003ff000002ff000001ff")
        with self.assertRaisesRegex(ValueError, "uninitialized entries"):
            _lighting_bmp_tga_decoder_bmp_pixels(b"\x03\x00\x00\x00", 1, -1, 8, palette=palette)

    def test_bmp_mask_table_contiguity_and_alpha_heuristic_are_not_repaired(self):
        from host import _lighting_bmp_tga_decoder_mask, _lighting_bmp_tga_decoder_bmp_pixels
        self.assertEqual(_lighting_bmp_tga_decoder_mask(16 << 10, 0x7c00), 132)
        self.assertEqual(_lighting_bmp_tga_decoder_mask(5, 5), 170)
        self.assertFalse(self.report["boundaries"]["bmp"]["mask_contiguity_required"])
        data = bytes.fromhex("1020300040506001")
        self.assertEqual(_lighting_bmp_tga_decoder_bmp_pixels(data, 2, -1, 32,
            alpha_accumulator_zero=True).hex(), "3020100060504001")
        self.assertEqual(_lighting_bmp_tga_decoder_bmp_pixels(data[:4], 1, -1, 32).hex(), "30201000")
        self.assertEqual(_lighting_bmp_tga_decoder_bmp_pixels(bytes.fromhex("00800000"), 1, -1, 16).hex(), "000000ff")
        with self.assertRaisesRegex(ValueError, "bad masks"):
            _lighting_bmp_tga_decoder_bmp_pixels(b"\x00" * 4, 1, -1, 16, masks=(0, 1, 2, 0))

    def test_tga_raw_and_rle_origin_horizontal_bit_gray_alpha_and_rgb555(self):
        from host import _lighting_bmp_tga_decoder_tga_pixels
        data = bytes.fromhex("010203040506")
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(data, 2, 1, 24,
            descriptor=48).hex(), "030201ff060504ff")
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(data, 1, 2, 24,
            descriptor=0).hex(), "060504ff030201ff")
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(bytes.fromhex("ffff"), 1, 1, 16).hex(), "ffffffff")
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(bytes.fromhex("8033"), 1, 1, 16,
            image_type=3, descriptor=32).hex(), "80808033")
        self.assertFalse(self.report["boundaries"]["tga"]["descriptor_horizontal_bit16_applied"])

    def test_tga_packets_clip_at_raster_and_scalar_eof_differs_from_raw_short_read(self):
        from host import _lighting_bmp_tga_decoder_tga_pixels
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(bytes.fromhex("ff010203"), 1, 1, 24,
            image_type=10).hex(), "030201ff")
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(b"", 1, 1, 24,
            image_type=10).hex(), "000000ff")
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(b"", 1, 1, 16).hex(), "000000ff")
        with self.assertRaisesRegex(ValueError, "unobserved"):
            _lighting_bmp_tga_decoder_tga_pixels(b"", 1, 1, 24)
        evidence = self.report["instruction_evidence"]
        for key, call, following in (("tga_raw_bottom_row_result_unchecked", "0x8f0fd", "0x8f102"),
                ("tga_raw_top_row_result_unchecked", "0x8f60d", "0x8f612")):
            rows = {row["rva"]: row for row in evidence[key]["instructions"]}
            self.assertTrue(rows[call]["text"].endswith("sub_180091C90"))
            self.assertNotIn("eax", rows[following]["text"])

    def test_tga_palette_out_of_range_selects_zero_and_origin_skip_is_unscaled(self):
        from host import _lighting_bmp_tga_decoder_tga_pixels
        palette = [(16, 32, 48), (64, 80, 96)]
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(bytes([1, 2]), 2, 1, 8,
            image_type=1, palette=palette).hex(), "605040ff302010ff")
        self.assertEqual(_lighting_bmp_tga_decoder_tga_pixels(bytes.fromhex("0100"), 1, 1, 16,
            image_type=1, palette=palette).hex(), "605040ff")
        rows = {row["rva"]: row for row in self.report["instruction_evidence"]["tga_palette_origin_raw_byte_skip"]["instructions"]}
        self.assertEqual(rows["0x8f18b"]["text"], "mov     edx, r15d")
        self.assertTrue(rows["0x8f18e"]["text"].endswith("sub_180091B50"))

    def test_actual_cleanup_both_palette_buffers_and_shared_conversion_ownership(self):
        evidence = self.report["instruction_evidence"]
        rows = {row["rva"]: row for row in evidence["tga_palette_short_read_both_frees"]["instructions"]}
        self.assertTrue(rows["0x8f7a1"]["text"].endswith("sub_18014FBA0"))
        self.assertEqual(rows["0x8f7a6"]["text"], "mov     rcx, r15")
        failure = {row["rva"]: row for row in evidence["tga_palette_failure_release_and_errors"]["instructions"]}
        self.assertTrue(failure["0x8f740"]["text"].endswith("sub_18014FBA0"))
        self.assertTrue(failure["0x8f751"]["text"].endswith("sub_18014FBA0"))
        bmp = {row["rva"]: row for row in evidence["bmp_bad_masks_release"]["instructions"]}
        self.assertTrue(bmp["0x8c12c"]["text"].endswith("sub_18014FBA0"))
        self.assertTrue(self.report["boundaries"]["shared"]["conversion_failure_frees_source"])
        self.assertFalse(self.report["boundaries"]["shared"]["decoder_async_cancel_observed"])

    def test_incorrect_format_compression_mask_palette_direction_and_cleanup_fail(self):
        for group, field, value in (("bmp", "rle1_rle2_rejected", False),
                ("bmp", "palette_index_actual_count_checked", True),
                ("bmp", "explicit_alpha_masks_force_all_zero_opaque", True),
                ("bmp", "negative_height_is_top_down", False),
                ("tga", "raw_nonpalette_non_rgb16_exact_row_return_checked", True),
                ("tga", "out_of_range_palette_index_selects_zero", False),
                ("tga", "palette_short_read_releases_palette_and_destination", False),
                ("tga", "color_map_first_index_subtracts_from_pixel_index", True),
                ("shared", "conversion_failure_frees_source", False)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_sources_functions_bytes_edges_semantics_and_dependencies_fail(self):
        from host import _lighting_bmp_tga_decoder_projection
        for mutate in (lambda r: r["current_native_release"].update(version="old"),
                lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x8fb40"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00" * 36),
                lambda r: r["instruction_evidence"]["tga_raw_top_row_result_unchecked"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(bmp_pixels="generic division like TGA"),
                lambda r: r["dependencies"][0].update(section="unknown"),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_bmp_tga_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_remaining_formats_static_snapshot_and_actual_execution_are_separate(self):
        from analysis import semantic_query_gaps
        findings = [row for row in semantic_query_gaps(self.report, ".analysis/results/host-current/local-audit.json")
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual(len(findings), 3)
        self.assertEqual([row["pointer"] for row in findings], ["/remaining_chains"] * 3)
        self.assertFalse(any(name in self.report["remaining_chains"][0]["owner"] for name in ("BMP", "TGA", "PNG", "PNM")))
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("runtime_acceptance", "verified"),
                ("changed_during_query", [{"path": "changed"}])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["verification"]["all_format_decoders_complete"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingPicDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_pic_decoder_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_pic_decoder_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_pic_decoder_chain
        return _verify_lighting_pic_decoder_chain(report)

    def decode(self, data, width=1, height=1):
        from host import _lighting_pic_decoder_synthetic
        return _lighting_pic_decoder_synthetic(bytes.fromhex(data), width, height)

    def test_current_original_complete_functions_edges_callbacks_and_pe_bytes(self):
        native = self.report["native"]
        self.assertEqual(self.report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 16)
        self.assertEqual(len(native["verified_call_edges"]), 22)
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8dc40", "0x95910", "0x95ec0", "0x960e0", "0x90680"} <= functions)
        self.assertFalse({"0x8b480", "0x8fb40", "0x93fa0", "0x98c80", "0x8e920"} & functions)
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]], ["0x8b3e0", "0x8b440"])
        regions = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in native["verified_data_regions"]}
        self.assertEqual(len(regions), 2)
        self.assertEqual(len(regions["read_callback_leaf_and_padding"]), 32)
        self.assertEqual(struct.unpack("<3Q", regions["file_callback_table"]),
            (0x18008b3e0, 0x18008b400, 0x18008b440))
        self.assertEqual(len(self.report["instruction_evidence"]), 12)

    def test_independent_fixed_packet_vectors(self):
        for row in self.report["fixed_vectors"]["packet_streams"]:
            result = self.decode(row["input_hex"], row["width"], row["height"])
            with self.subTest(name=row["name"]):
                for field, expected in row["expected"].items():
                    self.assertEqual(result[field], expected)
        self.assertFalse(self.report["fixed_vectors"]["actual_dll_results"])

    def test_channel_masks_packet_order_rows_and_trailing_input(self):
        result = self.decode("010800800008004010203040", 2, 1)
        self.assertEqual(result["rgba8_hex"], "1030ffff2040ffff")
        result = self.decode("010800800008004010203040", 1, 2)
        self.assertEqual(result["rgba8_hex"], "1020ffff3040ffff")
        result = self.decode("0008000faabb", 2, 1)
        self.assertEqual(result["rgba8_hex"], "ffffffffffffffff")
        self.assertEqual(result["bytes_consumed"], 4)
        self.assertEqual(result["selected_channels"], 3)
        result = self.decode("0008001055", 1, 1)
        self.assertEqual(result["rgba8_hex"], "ffffff55")
        self.assertEqual(result["selected_channels"], 4)

    def test_pure_repeat_clips_count_and_zero_count_still_consumes_sample(self):
        result = self.decode("0008018000aa0533", 2, 1)
        self.assertEqual(result["rgba8_hex"], "33ffffff33ffffff")
        self.assertEqual(result["bytes_consumed"], 8)
        result = self.decode("0008018002110222", 2, 2)
        self.assertEqual(result["rgba8_hex"], "11ffffff11ffffff22ffffff22ffffff")
        self.assertEqual(result["bytes_consumed"], 8)

    def test_mixed_literal_short_extended_and_zero_repeat_counts(self):
        self.assertEqual(self.decode("000802808155", 2, 1)["rgba8_hex"], "55ffffff55ffffff")
        self.assertEqual(self.decode("0008028080000244", 2, 1)["rgba8_hex"], "44ffffff44ffffff")
        result = self.decode("00080280800000aa0033", 1, 1)
        self.assertEqual(result["rgba8_hex"], "33ffffff")
        self.assertEqual(result["bytes_consumed"], 10)
        for data in ("00080280011122", "000802808111", "0008028080000211"):
            with self.subTest(data=data):
                result = self.decode(data, 1, 2)
                self.assertFalse(result["success"])
                self.assertEqual(result["error"], "bad file")
        self.assertEqual(self.decode("0008028000110022", 1, 2)["rgba8_hex"], "11ffffff22ffffff")

    def test_descriptors_limits_unknown_compression_and_zero_dimensions(self):
        self.assertEqual(self.decode("0007008011")["error"], "bad format")
        ten = "01080000" * 9 + "00080000"
        self.assertEqual(self.decode(ten)["packet_count"], 10)
        eleven = "01080000" * 10 + "00080000"
        result = self.decode(eleven)
        self.assertEqual(result["error"], "bad format")
        self.assertEqual(result["bytes_consumed"], 40)
        self.assertEqual(self.decode("00080300")["error"], "bad format")
        self.assertTrue(self.decode("00080300", 1, 0)["success"])
        self.assertTrue(self.decode("00080200", 0, 1)["success"])
        self.assertEqual(self.decode("00080300", 0, 1)["error"], "bad format")

    def test_short_synthetic_payload_does_not_claim_native_callback_eof(self):
        for data in ("", "000800", "000800e01020", "0008018000", "000802808000"):
            with self.subTest(data=data):
                self.assertEqual(self.decode(data)["error"], "bad file")
        from host import _lighting_pic_decoder_synthetic
        for data, width, height in ((b"", 65536, 1), (b"", 1025, 1025), (b"x" * 1048577, 1, 1)):
            with self.assertRaisesRegex(ValueError, "outside the reviewed model"):
                _lighting_pic_decoder_synthetic(data, width, height)
        owner = self.report["boundaries"]["failure_and_ownership"]
        self.assertTrue(owner["callback_refill_can_still_supply_zero_sample"])
        self.assertFalse(owner["global_exact_raster_read_gate"])
        self.assertFalse(self.report["boundaries"]["scope"]["synthetic_model_is_product_rust_or_callback_execution"])

    def test_full_width_optional_pointer_required4_arguments_and_failure_release(self):
        evidence = self.report["instruction_evidence"]
        pointer = {row["rva"]: row for row in evidence["full_width_component_pointer_and_dimensions"]["instructions"]}
        self.assertEqual(pointer["0x8dcb7"]["text"], "cmovnz  r15, r12")
        self.assertEqual(pointer["0x8dcb7"]["bytes"], "4d0f45fc")
        convert = {row["rva"]: row for row in evidence["current_converter_actual_source4_width_height"]["instructions"]}
        self.assertEqual(convert["0x8dd9c"]["text"], "mov     edx, 4")
        self.assertEqual(convert["0x8dda1"]["text"], "mov     r8d, esi")
        self.assertEqual(convert["0x8dda4"]["text"], "mov     r9d, ebp")
        self.assertTrue(convert["0x8dda7"]["text"].endswith("sub_180090680"))
        failure = {row["rva"]: row for row in evidence["packet_failure_free_then_common_converter"]["instructions"]}
        self.assertTrue(failure["0x8de0c"]["text"].endswith("sub_18014FBA0"))
        self.assertEqual(failure["0x8de11"]["text"], "xor     r13d, r13d")
        self.assertTrue(failure["0x8de1b"]["text"].endswith("loc_18008DD94"))
        self.assertTrue(failure["0x8de24"]["text"].endswith("loc_18008DD94"))
        owner = self.report["boundaries"]["failure_and_ownership"]
        self.assertEqual(owner["current_converter_source_and_required_components"], [4, 4])
        self.assertTrue(owner["current_null_converter_result_is_direct_null_return"])
        self.assertTrue(owner["decode_failure_updates_width_height_outputs"])

    def test_wrong_packet_channel_compression_eof_and_cleanup_assumptions_fail(self):
        for group, field, value in (("entry", "working_pixel_stride", 3),
                ("packets", "maximum_descriptors", 11), ("packets", "at_least_one_selected_channel_required", True),
                ("packets", "compression1_zero_count_rejected", True),
                ("packets", "compression2_run_must_fit_remaining_row", False),
                ("packets", "runs_cross_scanline_boundaries", True),
                ("failure_and_ownership", "global_exact_raster_read_gate", True),
                ("failure_and_ownership", "decode_failure_still_reaches_channel_converter", False),
                ("failure_and_ownership", "decode_failure_frees_rgba_buffer", False)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_sources_bytes_edges_semantics_vectors_dependencies_and_remaining_fail(self):
        from host import _lighting_pic_decoder_projection
        for mutate in (lambda r: r["current_native_release"].update(version="old"),
                lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x98c80"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00" * 32),
                lambda r: r["instruction_evidence"]["packet_failure_free_then_common_converter"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(compression="all runs clip at the image boundary"),
                lambda r: r["fixed_vectors"]["packet_streams"][0]["expected"].update(rgba8_hex="10203000"),
                lambda r: r["dependencies"][0].update(section="unreviewed"),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_pic_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_remaining_chains_snapshot_and_actual_execution_stay_separate(self):
        from analysis import semantic_query_gaps
        findings = [row for row in semantic_query_gaps(self.report, ".analysis/results/host-current/local-audit.json")
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual(len(findings), 3)
        self.assertEqual([row["pointer"] for row in findings], ["/remaining_chains"] * 3)
        self.assertFalse(any(name in self.report["remaining_chains"][0]["owner"]
            for name in ("PIC", "PNG", "PNM", "BMP", "TGA")))
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("ui_backend_wiring", "verified"),
                ("runtime_acceptance", "verified"), ("changed_during_query", [{"path": "changed"}])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["verification"]["all_format_decoders_complete"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingGifDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_lighting_gif_decoder_chain(cls.audit)
        cls.previous = json.loads((analysis.ROOT / ".analysis/results/host-current/local-audit.json").read_bytes())

    def verify(self, report):
        from host import _lighting_gif_decoder_verify
        return _lighting_gif_decoder_verify(report)

    def decode(self, value):
        from host import _lighting_gif_decoder_synthetic
        return _lighting_gif_decoder_synthetic(bytes.fromhex(value) if isinstance(value, str) else value)

    def image(self, raster="0202440100", *, width=1, height=1, rect=None, background=0,
              global_table=True, image_flags=0, local="", extensions="", palette="112233445566"):
        rect = (0, 0, width, height) if rect is None else rect
        header = b"GIF89a" + struct.pack("<HHBBB", width, height, 128 if global_table else 0, background, 0)
        colors = bytes.fromhex(palette) if global_table else b""
        return header + colors + bytes.fromhex(extensions) + b"," + struct.pack("<HHHHB", *rect, image_flags) + bytes.fromhex(local + raster) + b";"

    def test_current_original_selected_owners_actual_edges_slots_and_pe_bytes(self):
        native = self.report["native"]
        self.assertEqual(self.report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"], "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 18)
        self.assertEqual(len(native["verified_call_edges"]), 25)
        self.assertEqual(len(self.report["instruction_evidence"]), 17)
        self.assertEqual(len(native["verified_data_regions"]), 2)
        self.assertTrue(all(row["pseudocode_captured"] and row["input_chunks_verified"] and not row["failures"]
            for row in native["complete_selected_functions"]))
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]], ["0x8b3e0", "0x8b400", "0x8b440"])
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8cc40", "0x945c0", "0x94c30", "0x94f50", "0x95090", "0x95450"} <= functions)
        self.assertFalse({"0x8b480", "0x8cd70", "0x98c80", "0x8e160"} & functions)

    def test_hand_packed_fixed_first_images_and_independent_expected_colors(self):
        for row in self.report["fixed_vectors"]["first_images"]:
            result = self.decode(row["input_hex"])
            with self.subTest(name=row["name"]):
                for field, expected in row["expected"].items():
                    self.assertEqual(result[field], expected)
        self.assertFalse(self.report["fixed_vectors"]["actual_dll_results"])

    def test_current_caller_stops_after_first_image_without_using_delay_or_tail(self):
        first = self.image(extensions="21f9040c34120000")
        result = self.decode(first + b"malformed later frames are never parsed")
        self.assertEqual(result["pixels_rgba8_hex"], "112233ff")
        self.assertEqual(result["delay_ms"], 46600)
        self.assertEqual(result["bytes_consumed"], len(first) - 1)
        self.assertTrue(result["first_image_only"])
        instructions = self.report["instruction_evidence"]["one_zero_state_and_previous_null_call"]["instructions"]
        by_rva = {row["rva"]: row for row in instructions}
        self.assertEqual(by_rva["0x8cc77"]["text"], "mov     r8d, 8870h")
        self.assertEqual(by_rva["0x8cc90"]["text"], "xor     r9d, r9d")
        self.assertEqual(sum(row["text"].endswith("sub_1800945C0") for row in instructions), 1)
        caller = self.report["boundaries"]["caller"]
        self.assertFalse(caller["multi_frame_disposal_branch_reached"])
        self.assertFalse(caller["delay_drives_current_playback"])

    def test_internal_bgra_normal_rgba_background_raw_copy_and_index0(self):
        result = self.decode(self.image(width=2, rect=(0, 0, 1, 1), background=1))
        self.assertEqual(result["pixels_rgba8_hex"], "112233ff665544ff")
        self.assertEqual(self.decode(self.image(width=2, rect=(0, 0, 1, 1)))["pixels_rgba8_hex"], "112233ff00000000")
        result = self.decode(self.image(width=2, rect=(0, 0, 1, 1), background=1, extensions="21f9040100000000"))
        self.assertEqual(result["pixels_rgba8_hex"], "00000000665544ff")
        self.assertEqual(result["history_hex"], "0100")
        emitted = {row["rva"]: row for row in self.report["instruction_evidence"]["history_precedes_transparency_and_rgba_emit"]["instructions"]}
        self.assertEqual(emitted["0x954a4"]["text"], "mov     byte ptr [r8+rdx], 1")
        self.assertEqual(emitted["0x954b9"]["text"], "cmp     byte ptr [rdx+r8*4+3], 81h")
        background = {row["rva"]: row for row in self.report["instruction_evidence"]["background_nonzero_untouched_raw_bgra_copy"]["instructions"]}
        self.assertEqual(background["0x94c20"]["text"], "mov     [rdx+rcx*4], r8d")

    def test_local_palette_precedence_and_unchecked_undeclared_palette_index(self):
        result = self.decode(self.image(image_flags=128, local="a1b2c3d4e5f6"))
        self.assertEqual((result["palette"], result["pixels_rgba8_hex"]), ("local", "a1b2c3ff"))
        result = self.decode(self.image(global_table=False, image_flags=128, local="a1b2c3d4e5f6"))
        self.assertEqual(result["pixels_rgba8_hex"], "a1b2c3ff")
        self.assertEqual(self.decode(self.image(global_table=False))["error"], "missing color table")
        result = self.decode(self.image("02025c0100"))
        self.assertTrue(result["success"])
        self.assertEqual((result["pixels_rgba8_hex"], result["history_hex"]), ("00000000", "01"))
        restored = self.image("02025c0100", extensions="21f904010000030021f9040000000000")
        self.assertEqual(self.decode(restored)["pixels_rgba8_hex"], "000000ff")

    def test_interlaced_rectangle_order_is_bound_to_actual_passes(self):
        data = bytes.fromhex("47494638396101000800820000" +
            "000000010000020000030000040000050000060000070000" +
            "2c0000000001000800400306082143652701003b")
        result = self.decode(data)
        self.assertEqual(result["pixels_rgba8_hex"], "000000ff040000ff020000ff050000ff010000ff060000ff030000ff070000ff")
        self.assertEqual(result["history_hex"], "01" * 8)
        result = self.decode(self.image(width=2, height=2, rect=(1, 1, 1, 1)))
        self.assertEqual(result["pixels_rgba8_hex"], "00000000" * 3 + "112233ff")

    def test_subblock_boundaries_early_termination_and_excess_symbols(self):
        self.assertEqual(self.decode(self.image("020144010100"))["pixels_rgba8_hex"], "112233ff")
        result = self.decode(self.image("0200"))
        self.assertEqual((result["stop"], result["history_hex"]), ("zero_subblock", "00"))
        result = self.decode(self.image("02010500"))
        self.assertEqual((result["stop"], result["history_hex"]), ("end_code", "00"))
        result = self.decode(self.image("02034401ff02aabb00"))
        self.assertEqual(result["pixels_rgba8_hex"], "112233ff")
        self.assertEqual(result["bytes_consumed"], len(self.image("02034401ff02aabb00")) - 1)
        result = self.decode(self.image("0202840b00"))
        self.assertEqual((result["decoded_symbols"], result["clipped_symbols"]), (1, 2))
        result = self.decode(self.image(rect=(0, 0, 0, 1)))
        self.assertTrue(result["success"])
        self.assertEqual(result["history_hex"], "00")

    def test_actual_lzw_minimum_range_clear_errors_and_dictionary_limit(self):
        self.assertEqual(self.decode(self.image("00011100", width=2))["pixels_rgba8_hex"], "112233ff112233ff")
        self.assertEqual(self.decode(self.image("01013600"))["pixels_rgba8_hex"], "445566ff")
        result = self.decode(self.image("0c05003020044000"))
        self.assertEqual(result["pixels_rgba8_hex"], "445566ff")
        self.assertEqual((result["stop"], result["decoded_symbols"], result["clipped_symbols"]), ("end_code", 1, 0))
        for raster, error in (("02012800", "no clear code"), ("02010700", "illegal code in raster"),
                ("02013400", "illegal code in raster")):
            with self.subTest(raster=raster):
                self.assertEqual(self.decode(self.image(raster))["error"], error)
        result = self.decode(self.image("0d"))
        self.assertEqual((result["success"], result["error"], result["stop"]), (False, None, "minimum_code_size_above12"))
        codes = [4096] + [0] * 4096
        packed = sum(code << (13 * index) for index, code in enumerate(codes)).to_bytes((13 * len(codes) + 7) // 8, "little")
        subblocks = b"".join(bytes((len(packed[offset:offset + 255]),)) + packed[offset:offset + 255]
            for offset in range(0, len(packed), 255))
        data = self.image()[:-6] + b"\x0c" + subblocks + b"\0;"
        self.assertEqual(self.decode(data)["error"], "too many codes")

    def test_header_extensions_descriptor_and_no_image_failures_remain_distinct(self):
        self.assertEqual(self.decode(b"BADHDR" + self.image()[6:])["error"], "not GIF")
        self.assertEqual(self.decode(self.image(rect=(1, 0, 1, 1)))["error"], "bad Image Descriptor")
        self.assertEqual(self.decode(self.image()[:19] + b"x")["error"], "unknown code")
        self.assertEqual(self.decode(self.image()[:19] + b";")["stop"], "trailer_before_image")
        extensions = "21fe02616201630021ff036162630021f902aabb00"
        self.assertEqual(self.decode(self.image(extensions=extensions))["pixels_rgba8_hex"], "112233ff")
        data = b"GIF89a" + struct.pack("<HHBBB", 65535, 65535, 0, 0, 0)
        self.assertEqual(self.decode(data)["error"], "too large")
        for data, message in ((self.image()[:10], "unobserved callback-dependent"),
                (self.image()[:-4], "unobserved callback-dependent"),
                (b"GIF89a" + struct.pack("<HHBBB", 0, 1, 0, 0, 0), "unobserved allocator-dependent")):
            with self.subTest(data=data), self.assertRaisesRegex(ValueError, message):
                self.decode(data)

    def test_allocation_cleanup_actual_converter_parameters_and_commit_order(self):
        evidence = self.report["instruction_evidence"]
        allocation = evidence["three_allocations_before_test_then_zero"]["instructions"]
        calls = [row["rva"] for row in allocation if row["text"].endswith("sub_180088FC0")]
        self.assertEqual(calls, ["0x94692", "0x946a1", "0x946b3"])
        self.assertEqual([row["rva"] for row in evidence["unconditional_history_and_background_release"]["instructions"]
            if row["text"].endswith("sub_18014FBA0")], ["0x8ccf6", "0x8cd00"])
        convert = {row["rva"]: row for row in evidence["actual_converter_parameters_and_dimension_commit"]["instructions"]}
        self.assertEqual(convert["0x8cccb"]["text"], "mov     [rbx], r9d")
        self.assertEqual(convert["0x8cce4"]["text"], "mov     edx, 4")
        self.assertEqual(convert["0x8ccdd"]["text"], "mov     [rsp+88C8h+var_88A8], eax")
        self.assertFalse(self.report["boundaries"]["ownership"]["converter_failure_reverts_dimensions"])
        self.assertFalse(self.report["boundaries"]["ownership"]["universal_truncation_failure"])

    def test_wrong_frame_layout_transparency_lzw_and_release_assumptions_fail(self):
        for group, field, value in (("caller", "decoder_calls_per_wrapper", 2),
                ("caller", "delay_drives_current_playback", True), ("header", "palette_internal_layout", "R,G,B,A"),
                ("image", "transparent_pixels_mark_history_before_skipping_write", False),
                ("image", "background_index0_filled", True), ("image", "first_background_fill_bytes", "R,G,B,A"),
                ("lzw", "accepted_minimum_code_sizes", [2, 8]), ("lzw", "dictionary_entries", 4096),
                ("lzw", "end_code_requires_full_raster", True), ("ownership", "universal_truncation_failure", True)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_native_bytes_edges_vectors_semantics_and_remaining_cannot_be_forged(self):
        from host import _lighting_gif_decoder_projection
        for mutate in (lambda r: r["current_native_release"].update(version="old"),
                lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x98c80"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00"),
                lambda r: r["instruction_evidence"]["history_precedes_transparency_and_rgba_emit"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(raster="background always uses RGBA"),
                lambda r: r["fixed_vectors"]["first_images"][1]["expected"].update(pixels_rgba8_hex="112233ff445566ff"),
                lambda r: r["dependencies"][0].update(pointer="/unreviewed"),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_gif_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_exact_upstream_pointers_reject_changes_and_allow_unrelated_refresh(self):
        from host import _lighting_gif_decoder_dependencies
        previous = copy.deepcopy(self.previous)
        previous["independent_refresh"] = True
        self.assertEqual(_lighting_gif_decoder_dependencies(previous), self.report["dependencies"])
        for mode in ("missing", "changed", "unresolved"):
            previous = copy.deepcopy(self.previous)
            if mode == "missing":
                previous["lighting_psd_decoder_chain"]["remaining_chains"].pop(0)
            elif mode == "changed":
                previous["lighting_media_codec_boundaries"]["boundaries"]["caller"]["required_channels"] = 3
            else:
                previous["lighting_psd_decoder_chain"]["bounded_observation_complete"] = False
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "upstream pointer requires review"):
                _lighting_gif_decoder_dependencies(previous)

    def test_remaining_failures_snapshot_and_real_execution_stay_separate(self):
        from analysis import semantic_query_gaps
        report = self.report
        findings = [row for row in semantic_query_gaps(report, ".analysis/results/host-current/local-audit.json")
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual([row["finding"] for row in findings], report["remaining_chains"])
        self.assertEqual(len(findings), 3)
        self.assertEqual([row["pointer"] for row in findings], ["/remaining_chains"] * 3)
        self.assertEqual(report["verification"]["global_native_failures_retained"], 10)
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("ui_backend_wiring", "verified"),
                ("runtime_acceptance", "verified"), ("changed_during_query", [{"path": "changed"}])):
            changed = copy.deepcopy(report)
            changed[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(changed)
        changed = copy.deepcopy(report)
        changed["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(changed)
        self.assertEqual(self.verify(json.loads(json.dumps(report, ensure_ascii=False))), report["verification"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(report["maintenance_lifecycle"]["analysis_sessions_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingPsdDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_psd_decoder_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_psd_decoder_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_psd_decoder_chain
        return _verify_lighting_psd_decoder_chain(report)

    def pixels(self, data, width=1, height=1, channels=3, depth=8, compression=0, requested_bpc=8):
        from host import _lighting_psd_decoder_pixels
        return _lighting_psd_decoder_pixels(bytes.fromhex(data), width, height, channels, depth, compression, requested_bpc)

    def test_current_original_complete_functions_edges_slots_and_pe_constants(self):
        native = self.report["native"]
        self.assertEqual(self.report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 16)
        self.assertEqual(len(native["verified_call_edges"]), 19)
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8cd70", "0x95570", "0x90680", "0x910a0"} <= functions)
        self.assertFalse({"0x8b480", "0x8fb40", "0x95910", "0x98c80", "0x8e920"} & functions)
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]],
            ["0x8b3e0", "0x8b400", "0x8b440"])
        regions = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in native["verified_data_regions"]}
        self.assertEqual(len(regions), 5)
        self.assertEqual(struct.unpack("<f", regions["alpha_byte_maximum"]), (255.0,))
        self.assertEqual(struct.unpack("<f", regions["alpha_word_maximum"]), (65535.0,))
        self.assertEqual(struct.unpack("<f", regions["alpha_inverse_one"]), (1.0,))
        self.assertEqual(len(self.report["instruction_evidence"]), 16)

    def test_header_actual_reserved_skip_signed_dimensions_channels_mode_and_sections(self):
        evidence = self.report["instruction_evidence"]
        header = {row["rva"]: row for row in evidence["header_signature_version_reserved_channels"]["instructions"]}
        self.assertEqual(header["0x8cd96"]["text"], "cmp     eax, 38425053h")
        self.assertEqual(header["0x8cdad"]["text"], "mov     edx, 6")
        self.assertTrue(header["0x8cdb2"]["text"].endswith("sub_180091B50"))
        self.assertEqual(header["0x8cdbf"]["text"], "cmp     eax, 11h")
        self.assertTrue(header["0x8cdc2"]["text"].startswith("jb "))
        fields = {row["rva"]: row for row in evidence["dimensions_depth_rgb_and_sections"]["instructions"]}
        self.assertTrue(fields["0x8cdfd"]["text"].startswith("jge "))
        self.assertTrue(fields["0x8ce04"]["text"].startswith("jl "))
        self.assertEqual(fields["0x8ce4f"]["text"], "cmp     eax, 3")
        for site in ("0x8ce5f", "0x8ce71", "0x8ce83"):
            self.assertEqual(fields[site]["text"], "mov     edx, eax")
        entry = self.report["boundaries"]["entry"]
        self.assertEqual(entry["channel_count_range"], [0, 16])
        self.assertFalse(entry["zero_dimensions_explicitly_rejected"])
        self.assertFalse(entry["reserved_bytes_must_be_zero"])
        self.assertFalse(entry["skip_return_checked"])

    def test_independent_fixed_raw_rle_depth_and_matte_vectors(self):
        for row in self.report["fixed_vectors"]["pixel_payloads"]:
            kwargs = {key: value for key, value in row.items() if key not in ("name", "input_hex", "expected")}
            result = self.pixels(row["input_hex"], **kwargs)
            with self.subTest(name=row["name"]):
                for field, expected in row["expected"].items():
                    self.assertEqual(result[field], expected)
        self.assertFalse(self.report["fixed_vectors"]["actual_dll_results"])

    def test_missing_channels_zero_rgb_opaque_alpha_and_extra_planes_unread(self):
        self.assertEqual(self.pixels("", channels=0)["pixels_le_hex"], "000000ff")
        self.assertEqual(self.pixels("7f", channels=1)["pixels_le_hex"], "7f0000ff")
        self.assertEqual(self.pixels("1020", channels=2)["pixels_le_hex"], "102000ff")
        self.assertEqual(self.pixels("", channels=0, depth=16, requested_bpc=16)["pixels_le_hex"], "000000000000ffff")
        result = self.pixels("102030ff7788", channels=5)
        self.assertEqual(result["pixels_le_hex"], "102030ff")
        self.assertEqual(result["bytes_consumed"], 4)
        result = self.pixels("00100020003000ff7788", channels=16, compression=1)
        self.assertEqual(result["pixels_le_hex"], "102030ff")
        self.assertEqual(result["bytes_consumed"], 8)

    def test_raw_big_endian_high_byte_and_only_uncompressed_retains16(self):
        for requested in (8, 16):
            with self.subTest(requested=requested):
                result = self.pixels("00ff7fff8000", depth=16, requested_bpc=requested)
                self.assertEqual(result["pixels_le_hex"], "007f80ff" if requested == 8 else "ff00ff7f0080ffff")
                self.assertEqual(result["bits_per_channel"], requested)
        result = self.pixels("00120056009a", depth=16, compression=1, requested_bpc=16)
        self.assertEqual(result["pixels_le_hex"], "12569aff")
        self.assertEqual(result["bits_per_channel"], 8)
        evidence = self.report["instruction_evidence"]
        high = {row["rva"]: row for row in evidence["raw16_current_high_byte"]["instructions"]}
        self.assertEqual(high["0x8d2d4"]["text"], "mov     [rbp+rcx*4+0], ah")
        storage = self.report["boundaries"]["storage"]
        self.assertTrue(storage["retained16_sets_result_bpc_before_allocation_success_check"])
        self.assertEqual(storage["all_other_paths_output_bpc"], 8)

    def test_packbits_noop_literal_repeat_plane_bounds_and_row_crossing(self):
        result = self.pixels("80011234", width=1, height=2, channels=1, compression=1)
        self.assertEqual(result["pixels_le_hex"], "120000ff340000ff")
        self.assertEqual(result["bytes_consumed"], 4)
        self.assertEqual(self.pixels("ff55", width=2, channels=1, compression=1)["pixels_le_hex"], "550000ff550000ff")
        result = self.pixels("8155", width=128, channels=1, compression=1)
        self.assertEqual(result["pixels_le_hex"], "550000ff" * 128)
        literal = "7f" + bytes(range(128)).hex()
        result = self.pixels(literal, width=128, channels=1, compression=1)
        self.assertEqual(result["pixels_le_hex"], b"".join(bytes((i, 0, 0, 255)) for i in range(128)).hex())
        for data in ("011122", "fe33"):
            with self.subTest(data=data):
                result = self.pixels(data, width=1, channels=1, compression=1)
                self.assertFalse(result["success"])
                self.assertEqual(result["error"], "corrupt")
                self.assertEqual(result["bytes_consumed"], 1)
        table = {row["rva"]: row for row in self.report["instruction_evidence"]["rle_table_skip_and_full_plane_size"]["instructions"]}
        self.assertEqual(table["0x8cf53"]["text"], "imul    edx, eax")
        self.assertEqual(table["0x8cf56"]["text"], "add     edx, edx")
        self.assertEqual(table["0x8cf7a"]["text"], "mov     r8d, r12d")

    def test_alpha_endpoints_partial_float32_and_low_byte_word_storage(self):
        for alpha in ("00", "ff"):
            self.assertEqual(self.pixels("102030" + alpha, channels=4)["pixels_le_hex"], "102030" + alpha)
        self.assertEqual(self.pixels("0080ff80", channels=4)["pixels_le_hex"], "0301ff80")
        result = self.pixels("aaaa0000ffff5555", channels=4, depth=16, requested_bpc=16)
        self.assertEqual(result["pixels_le_hex"], "00000200ffff5555")
        alpha = self.report["boundaries"]["alpha"]
        self.assertFalse(alpha["clamp_before_storage"])
        self.assertFalse(alpha["alpha_modified"])
        rows = {row["rva"]: row for row in self.report["instruction_evidence"]["alpha8_float_and_byte_truncation"]["instructions"]}
        self.assertEqual(rows["0x8da7b"]["text"], "cvttss2si edx, xmm4")
        self.assertEqual(rows["0x8da7f"]["text"], "mov     [rbp+rcx*4+0], dl")

    def test_short_synthetic_payload_remains_unobserved_not_native_failure(self):
        for data, depth, compression in (("1020", 8, 0), ("12345678", 16, 0), ("00", 8, 1), ("80", 8, 1)):
            with self.subTest(data=data), self.assertRaisesRegex(ValueError, "unobserved callback-dependent"):
                self.pixels(data, depth=depth, compression=compression)
        from host import _lighting_psd_decoder_pixels
        with self.assertRaisesRegex(ValueError, "outside the reviewed model"):
            _lighting_psd_decoder_pixels(b"", 1025, 1025, 3, 8, 0)
        with self.assertRaisesRegex(ValueError, "outside the reviewed model"):
            _lighting_psd_decoder_pixels(b"", 1, 1, 17, 8, 0)
        owner = self.report["boundaries"]["ownership"]
        self.assertFalse(owner["raw_short_read_always_fails"])
        self.assertFalse(owner["rle_short_read_always_fails"])
        self.assertTrue(self.report["boundaries"]["rle"]["exhausted_scalar_reads_can_supply_zero"])
        self.assertFalse(self.report["boundaries"]["scope"]["synthetic_model_is_product_rust_or_callback_execution"])

    def test_failure_free_precedes_error_current4_bypass_and_output_commit(self):
        evidence = self.report["instruction_evidence"]
        failure = {row["rva"]: row for row in evidence["compressed_failure_release_before_error"]["instructions"]}
        self.assertTrue(failure["0x8d29a"]["text"].endswith("sub_18014FBA0"))
        self.assertIn('"corrupt"', failure["0x8d29f"]["text"])
        convert = {row["rva"]: row for row in evidence["converter_actual4_and_current_bypass"]["instructions"]}
        self.assertEqual(convert["0x8dae3"]["text"], "test    r8d, 0FFFFFFFBh")
        self.assertTrue(convert["0x8daea"]["text"].endswith("loc_18008DB30"))
        self.assertEqual(convert["0x8db01"]["text"], "mov     edx, 4")
        self.assertEqual(convert["0x8db15"]["text"], "mov     edx, 4")
        self.assertEqual(convert["0x8daf9"]["bytes"], "44897c2420")
        self.assertTrue(convert["0x8db2a"]["text"].endswith("loc_18008CE12"))
        output = {row["rva"]: row for row in evidence["success_output_metadata"]["instructions"]}
        self.assertEqual(output["0x8db35"]["text"], "mov     dword ptr [rbx], 4")
        self.assertEqual(output["0x8db3b"]["text"], "mov     [rdi], r15d")
        self.assertEqual(output["0x8db3e"]["text"], "mov     [rsi], r9d")

    def test_wrong_header_depth_rle_alpha_and_ownership_assumptions_fail(self):
        for group, field, value in (("entry", "reserved_bytes_must_be_zero", True),
                ("entry", "channel_count_range", [1, 4]), ("entry", "dimension_upper_gate_signed", False),
                ("storage", "compressed16_uses_same_byte_plane_decoder", False),
                ("storage", "extra_channel_pixels_consumed_or_skipped", True),
                ("rle", "row_length_entries_used_to_bound_runs", True),
                ("rle", "runs_can_cross_rows", False), ("alpha", "clamp_before_storage", True),
                ("ownership", "raw_short_read_always_fails", True),
                ("ownership", "compressed_overflow_frees_pixels_before_error", False)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_sources_bytes_edges_semantics_vectors_dependencies_and_remaining_fail(self):
        from host import _lighting_psd_decoder_projection
        for mutate in (lambda r: r["current_native_release"].update(version="old"),
                lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x95910"),
                lambda r: r["native"]["verified_data_regions"][2].update(raw_hex="00000000"),
                lambda r: r["instruction_evidence"]["rle_table_skip_and_full_plane_size"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(packbits="decode16bit samples and reset every row"),
                lambda r: r["fixed_vectors"]["pixel_payloads"][1]["expected"].update(pixels_le_hex="3478bcff"),
                lambda r: r["dependencies"][0].update(section="unreviewed"),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_psd_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_remaining_chains_snapshot_and_real_execution_stay_separate(self):
        from analysis import semantic_query_gaps
        findings = [row for row in semantic_query_gaps(self.report, ".analysis/results/host-current/local-audit.json")
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual(len(findings), 3)
        self.assertEqual([row["pointer"] for row in findings], ["/remaining_chains"] * 3)
        self.assertFalse(any(name in self.report["remaining_chains"][0]["owner"]
            for name in ("PSD", "PIC", "PNG", "PNM", "BMP", "TGA")))
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("ui_backend_wiring", "verified"),
                ("runtime_acceptance", "verified"), ("changed_during_query", [{"path": "changed"}])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["verification"]["all_format_decoders_complete"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingJpegDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_jpeg_decoder_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_jpeg_decoder_chain(cls.audit)

    def verify(self, report):
        from host import _lighting_jpeg_decoder_verify
        return _lighting_jpeg_decoder_verify(report)

    def instructions(self, role):
        return {row["rva"]: row for row in self.report["instruction_evidence"][role]["instructions"]}

    def test_current_complete_native_selection_edges_callbacks_and_failure_boundary(self):
        native = self.report["native"]
        self.assertEqual(self.report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 51)
        self.assertEqual(len(native["verified_call_edges"]), 74)
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]],
            ["0x8b3e0", "0x8b400", "0x8b440"])
        for row in native["complete_selected_functions"]:
            self.assertTrue(row["assembly_captured"] and row["pseudocode_captured"] and row["input_chunks_verified"])
            self.assertEqual(row["failures"], 0)
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8deb0", "0x98c80", "0x9a9e0", "0x9b020", "0x9b320", "0x9baa0",
            "0x962c0", "0x96ae0", "0x96820", "0x97550", "0x98be0"} <= functions)
        self.assertFalse({"0x8b480", "0x945c0", "0x8e160", "0x95570", "0x95910"} & functions)
        self.assertFalse(native["all_pseudocode_captured"])
        self.assertEqual(native["selected_pseudocode_failures"], [])
        self.assertEqual(len(native["failures"]), 10)
        self.assertEqual(len(self.report["instruction_evidence"]), 18)

    def test_exact_pe_zigzag_padding_entropy_masks_and_simd_constants(self):
        regions = {row["role"]: bytes.fromhex(row["raw_hex"])
            for row in self.report["native"]["verified_data_regions"]}
        self.assertEqual(len(regions), 6)
        zigzag = regions["zigzag_padding_and_RGB_ids"]
        self.assertEqual(list(zigzag[:16]), [0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5])
        self.assertEqual(sorted(zigzag[:64]), list(range(64)))
        self.assertEqual(zigzag[64:79], bytes([63] * 15))
        self.assertEqual(zigzag[79:], b"RGB")
        masks = struct.unpack("<17I", regions["entropy_masks"])
        self.assertEqual(masks, tuple((1 << i) - 1 for i in range(17)))
        constants = regions["jpeg_simd_coefficients"]
        self.assertEqual(struct.unpack_from("<8h", constants, 16), (2217, -5350) * 4)
        self.assertEqual(struct.unpack_from("<4i", constants, 144), (512,) * 4)
        self.assertEqual(struct.unpack_from("<4i", constants, 160), (16842752,) * 4)
        self.assertEqual(struct.unpack_from("<8h", constants, 192), (5743,) * 8)
        self.assertEqual(struct.unpack_from("<8h", constants, 208), (-1410,) * 8)
        self.assertEqual(struct.unpack_from("<8h", constants, 224), (7258,) * 8)
        self.assertEqual(struct.unpack_from("<8h", constants, 240), (-2925,) * 8)

    def test_header_actual_precision_nonzero_dimensions_components_sampling_and_alignment(self):
        rows = self.instructions("frame_header_sampling_and_checked_storage")
        self.assertEqual(rows["0x983aa"]["text"], "cmp     byte ptr [rcx], 8")
        self.assertEqual(rows["0x983bc"]["text"], "test    eax, eax")
        self.assertEqual(rows["0x983ca"]["text"], "test    eax, eax")
        self.assertEqual(rows["0x98431"]["text"], "cmp     edx, 4")
        self.assertEqual(rows["0x98775"]["text"], "idiv    dword ptr [r8-4]")
        self.assertEqual(rows["0x98784"]["text"], "idiv    dword ptr [r8]")
        self.assertTrue(rows["0x98898"]["text"].endswith("sub_180093BA0"))
        self.assertTrue(rows["0x988eb"]["text"].endswith("sub_180093AD0"))
        self.assertEqual(rows["0x988ba"]["text"], "and     rax, 0FFFFFFFFFFFFFFF0h")
        self.assertEqual(rows["0x988fe"]["text"], "and     rax, 0FFFFFFFFFFFFFFF0h")
        entry = self.report["boundaries"]["entry"]
        self.assertEqual(entry["sof_markers"], [192, 193, 194])
        self.assertEqual(entry["component_counts"], [1, 3, 4])
        self.assertEqual(entry["sampling_factors"], [1, 4])
        self.assertFalse(entry["dnl_creates_missing_height"])

    def test_sos_baseline_overwrites_end_and_progressive_has_no_history_guarantee(self):
        rows = self.instructions("sos_scan_parameter_validation")
        self.assertEqual(rows["0x99f72"]["text"], "cmp     edx, 3Fh ; '?'")
        self.assertEqual(rows["0x99f7e"]["text"], "cmp     r8d, 40h ; '@'")
        self.assertEqual(rows["0x99f91"]["text"], "cmp     al, 0E0h")
        self.assertEqual(rows["0x99f96"]["text"], "cmp     ecx, 0Eh")
        self.assertEqual(rows["0x99fc0"]["text"], "mov     dword ptr [rsi+4838h], 3Fh ; '?'")
        scans = self.report["boundaries"]["scans"]
        self.assertTrue(scans["baseline_spectral_end_overwritten_to63"])
        self.assertFalse(scans["duplicate_scan_ids_explicitly_rejected"])
        self.assertFalse(scans["scan_history_or_high_low_relation_validated"])
        self.assertEqual(scans["progressive_successive_high_low_max"], 13)
        self.assertIn("EOB count persists", scans["progressive"])

    def test_marker_error_and_missing_restart_preserve_actual_early_success(self):
        rows = self.instructions("body_marker_early_success_and_eoi_finish")
        self.assertTrue(rows["0x99696"]["text"].endswith("sub_1800979E0"))
        self.assertTrue(rows["0x9969f"]["text"].endswith("loc_1800996F4"))
        self.assertEqual(rows["0x996f4"]["text"], "mov     esi, 1")
        self.assertTrue(rows["0x996ed"]["text"].endswith("sub_18009A830"))
        scan = self.instructions("baseline_actual_idct_callback")
        self.assertEqual(scan["0x9a2e2"]["text"], "and     al, 0F8h")
        self.assertEqual(scan["0x9a2e4"]["text"], "cmp     al, 0D0h")
        self.assertTrue(scan["0x9a2e6"]["text"].endswith("loc_18009A48D"))
        success = self.instructions("restart_missing_marker_returns_success")
        self.assertEqual(success["0x9a48d"]["text"], "mov     r14d, 1")
        self.assertFalse(self.report["boundaries"]["scans"]["eoi_required_for_all_success"])
        self.assertFalse(self.report["boundaries"]["scans"]["missing_restart_always_fails"])
        self.assertTrue(self.report["boundaries"]["tables_and_markers"]["exhausted_scalar_reads_may_supply_zero"])
        self.assertTrue(self.report["boundaries"]["ownership"]["marker_error_can_leave_uninitialized_component_or_coefficient_data"])

    def test_actual_scalar_sse2_slots_and_baseline_progressive_indirect_call_sites(self):
        rows = self.instructions("scalar_sse_callback_assignment")
        self.assertEqual(rows["0x9613f"]["text"], "mov     [rcx+4870h], rax")
        self.assertEqual(rows["0x96180"]["text"], "mov     [rsi+4878h], rax")
        self.assertEqual(rows["0x9618e"]["text"], "mov     [rsi+4880h], rax")
        cpu = self.instructions("cpuid_sse2_selection")
        self.assertEqual(cpu["0x96ac9"]["text"], "shr     eax, 1Ah")
        dispatch = self.report["native"]["verified_dynamic_dispatch"]
        self.assertEqual([row["byte_offset"] for row in dispatch["context_slots"]], [18544, 18552, 18560])
        self.assertEqual([row["sse2_target"] for row in dispatch["context_slots"]], ["0x96ae0", "0x972f0", "0x97550"])
        self.assertEqual([row["instruction"]["rva"] for row in dispatch["call_sites"]],
            ["0x9a2ac", "0x9a63d", "0x9a928", "0x98f15", "0x9943b", "0x98fd3"])
        self.assertTrue(all(row["instruction"]["text"].startswith("call    ") for row in dispatch["call_sites"]))
        finish = self.instructions("progressive_actual_finish_callback")
        self.assertTrue(finish["0x9a908"]["text"].endswith("sub_18009BAA0"))
        self.assertEqual(finish["0x9a928"]["text"], "call    qword ptr [r15+4870h]")

    def test_fixed_scalar_idct_dc_both_ac_axes_and_clamp_vectors(self):
        from host import _lighting_jpeg_decoder_block
        for row in self.report["fixed_vectors"]["scalar_blocks"]:
            with self.subTest(name=row["name"]):
                self.assertEqual(_lighting_jpeg_decoder_block(row["coefficients"]).hex(), row["expected_pixels_hex"])
        self.assertEqual(len(self.instructions("scalar_complete_idct")), 273)
        self.assertEqual(len(self.instructions("sse2_complete_idct")), 405)
        for coefficients in ([0] * 63, [32768] + [0] * 63, [-32769] + [0] * 63, [False] + [0] * 63):
            with self.assertRaisesRegex(ValueError, "requires 64 signed16"):
                _lighting_jpeg_decoder_block(coefficients)
        self.assertFalse(self.report["boundaries"]["scope"]["synthetic_model_executes_jpeg_stream_or_simd"])

    def test_fixed_color_rows_simd_groups_tail_and_opaque_k_conversion(self):
        from host import _lighting_jpeg_decoder_row
        for row in self.report["fixed_vectors"]["rows"]:
            for simd in (False, True):
                with self.subTest(name=row["name"], simd=simd):
                    actual = _lighting_jpeg_decoder_row([bytes.fromhex(value) for value in row["planes_hex"]], row["color"], simd=simd)
                    self.assertEqual(actual.hex(), row["expected_rgba_hex"])
                    self.assertEqual(actual[3::4], b"\xff" * (len(actual) // 4))
        self.assertEqual(_lighting_jpeg_decoder_row([b"\xff", b"\xff", b"\xff", b"\x00"], "cmyk"), b"\x00\x00\x00\xff")
        self.assertEqual(self.instructions("current_rgba_row_and_opaque_alpha")["0x990fc"]["text"], "mov     byte ptr [rbx], 0FFh")
        for planes, color in (([], "rgb"), ([b""], "gray"), ([b"a", b"b"], "rgb"), ([b"a", b"bb", b"c"], "ycbcr"), ([b"a"], "unknown")):
            with self.assertRaisesRegex(ValueError, "complete equal-width"):
                _lighting_jpeg_decoder_row(planes, color)

    def test_resampling_original_horizontal_tail_vertical_combination_and_generic(self):
        from host import _lighting_jpeg_decoder_resample
        for row in self.report["fixed_vectors"]["resampling"]:
            with self.subTest(name=row["name"]):
                self.assertEqual(_lighting_jpeg_decoder_resample(bytes.fromhex(row["near_hex"]), bytes.fromhex(row["far_hex"]),
                    row["horizontal"], row["vertical"]).hex(), row["expected_hex"])
        self.assertEqual(_lighting_jpeg_decoder_resample(b"\x0a", b"\x32", 2, 1), b"\x0a\x0a")
        self.assertEqual(_lighting_jpeg_decoder_resample(b"\x0a", b"\x32", 2, 2), b"\x14\x14")
        tail = self.instructions("horizontal2_original_last_weight")
        self.assertEqual(tail["0x99b25"]["text"], "movzx   r9d, byte ptr [rdx+r9-2]")
        self.assertEqual(tail["0x99b2b"]["text"], "lea     r9d, [r9+r9*2]")
        for near, far, horizontal, vertical in ((b"", b"", 1, 1), (b"a", b"bb", 1, 2), (b"a", b"b", 5, 1), (b"a", b"b", 2, 0)):
            with self.assertRaisesRegex(ValueError, "outside the reviewed sampling"):
                _lighting_jpeg_decoder_resample(near, far, horizontal, vertical)

    def test_cleanup_partial_components_context_output_metadata_and_unproven_unwind(self):
        wrapper = self.instructions("context_allocation_and_release")
        self.assertEqual(wrapper["0x8df3f"]["text"], "retn")
        cleanup = self.instructions("owned_plane_cleanup")
        frees = [row for row in cleanup.values() if row["text"].endswith("sub_18014FBA0")]
        self.assertEqual([row["rva"] for row in frees], ["0x98c23", "0x98c37", "0x98c57"])
        self.assertEqual(cleanup["0x98c3c"]["text"], "mov     qword ptr [rsi+rbx-10h], 0")
        self.assertEqual(cleanup["0x98c5c"]["text"], "mov     qword ptr [rsi+rbx-8], 0")
        output = self.instructions("output_commit_and_component_cleanup")
        self.assertTrue(output["0x9954f"]["text"].endswith("sub_180099720"))
        self.assertEqual(output["0x99561"]["text"], "mov     [rax], ecx")
        self.assertEqual(output["0x99571"]["text"], "mov     [rax], ecx")
        self.assertEqual(output["0x9958c"]["text"], "lea     ecx, ds:1[rdx*2]")
        ownership = self.report["boundaries"]["ownership"]
        self.assertTrue(ownership["component_allocation_failure_frees_partial_components"])
        self.assertTrue(ownership["success_transfers_output_allocation"])
        self.assertFalse(ownership["cpp_exception_unwind_or_heap_fault_recovery_established"])
        self.assertFalse(ownership["separate_async_cancel_observed"])

    def test_wrong_header_scan_eof_pixel_and_release_assumptions_are_rejected(self):
        for group, field, value in (("entry", "component_counts", [1, 2, 3, 4]),
                ("entry", "sample_bits", 16), ("entry", "dnl_creates_missing_height", True),
                ("tables_and_markers", "body_marker_error_runs_progressive_finish", True),
                ("scans", "missing_restart_always_fails", True), ("scans", "eoi_required_for_all_success", True),
                ("scans", "duplicate_scan_ids_explicitly_rejected", True),
                ("scans", "scan_history_or_high_low_relation_validated", True),
                ("pixels", "horizontal2_last_inner_uses_previous_weight3", False),
                ("ownership", "strict_short_read_or_eoi_guarantee", True),
                ("ownership", "marker_error_can_leave_uninitialized_component_or_coefficient_data", False)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_sources_bytes_edges_dispatch_vectors_dependencies_still_require_review(self):
        from host import _lighting_jpeg_decoder_projection
        for mutate in (lambda r: r["current_native_release"].update(version="old"),
                lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x945c0"),
                lambda r: r["native"]["verified_dynamic_dispatch"]["context_slots"][0].update(sse2_target="0x962c0"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00"),
                lambda r: r["instruction_evidence"]["scalar_complete_idct"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(entropy="all malformed scans fail"),
                lambda r: r["fixed_vectors"]["scalar_blocks"][1].update(expected_pixels_hex="80" * 64),
                lambda r: r["fixed_vectors"]["resampling"][2].update(expected_hex="0a214e64"),
                lambda r: r["dependencies"][0].update(pointer="/unreviewed"),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_jpeg_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_exact_upstream_pointers_reuse_semantic_validation_and_reject_change(self):
        from host import _lighting_jpeg_decoder_dependencies
        sections = {"lighting_psd_decoder_chain", "lighting_media_codec_boundaries", "lighting_media_resampler"}
        previous = {key: value for key, value in json.loads((self.audit.a.ROOT / ".analysis/results/host-current/local-audit.json").read_bytes()).items()
            if key in sections}
        self.assertEqual(_lighting_jpeg_decoder_dependencies(previous), self.report["dependencies"])
        self.assertEqual([row["pointer"] for row in self.report["dependencies"]],
            ["/lighting_psd_decoder_chain/remaining_chains/0", "/lighting_media_codec_boundaries/reviewed_selection_sha256",
                "/lighting_media_resampler/reviewed_selection_sha256"])
        for section in ("lighting_psd_decoder_chain", "lighting_media_codec_boundaries", "lighting_media_resampler"):
            changed = copy.deepcopy(previous)
            changed[section]["boundaries"]["unreviewed"] = True
            with self.subTest(section=section), self.assertRaisesRegex(ValueError, "boundary|bytes|selection"):
                _lighting_jpeg_decoder_dependencies(changed)
        changed = copy.deepcopy(previous)
        changed["lighting_psd_decoder_chain"]["remaining_chains"][0]["owner"] = "JPEG only"
        with patch("host._verify_lighting_psd_decoder_chain", return_value={}):
            with self.assertRaisesRegex(ValueError, "exact upstream pointer changed"):
                _lighting_jpeg_decoder_dependencies(changed)

    def test_snapshot_remaining_chains_roundtrip_and_execution_stages_stay_separate(self):
        from analysis import semantic_query_gaps
        findings = [row for row in semantic_query_gaps(self.report, ".analysis/results/host-current/local-audit.json")
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual(len(findings), 3)
        self.assertEqual(self.report["remaining_chains"][0]["owner"], "HDR8E160")
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("real_device_operations", True), ("product_code_modified", True),
                ("rust_independent_implementation", "verified"), ("actual_dll_comparison", "verified"),
                ("ui_backend_wiring", "verified"), ("runtime_acceptance", "verified"),
                ("snapshot_stable", False), ("changed_during_query", ["changed"])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["verification"]["all_format_decoders_complete"])
        self.assertFalse(report["fixed_vectors"]["actual_dll_results"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(report["maintenance_lifecycle"]["analysis_sessions_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingAiFlatbufferTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import _lighting_type_media_color_source, _lighting_ai_flatbuffer_resource
        audit = HostLocalAudit()
        original, _ = _lighting_type_media_color_source(audit)
        cls.raw = audit.read(original)
        cls.report = _lighting_ai_flatbuffer_resource(cls.raw)
        resource = cls.report["resource"]
        cls.model = cls.raw[resource["file_offset"]:resource["file_offset"] + resource["bytes"]]

    def slot(self, offset, field):
        vtable = offset - struct.unpack_from("<i", self.model, offset)[0]
        relative = struct.unpack_from("<H", self.model, vtable + 4 + field * 2)[0]
        self.assertNotEqual(relative, 0)
        return offset + relative

    def changed(self, offset, fmt, value):
        result = bytearray(self.model)
        struct.pack_into(fmt, result, offset, value)
        return bytes(result)

    def decode(self, data):
        from host import _lighting_ai_flatbuffer_graph
        return _lighting_ai_flatbuffer_graph(data)

    def test_current_resource_graph_and_exact_constant_spans(self):
        report = self.report
        self.assertEqual((report["identifier"], report["version"], report["root_table_offset"]), ("TFL3", 3, 28))
        self.assertEqual((report["model_bytes"], report["model_sha256"]), (59336,
            "98f063f9d8a3e6263523afccfbb1e60e0611656f1c5401a3aa88864fd8e042dc"))
        self.assertEqual(report["resource"]["rva"], "0x556438")
        self.assertNotEqual(report["resource"]["file_offset"], int(report["resource"]["rva"], 16))
        self.assertEqual([(row["name"], row["version"]) for row in report["operator_codes"]], [
            ("CONV_2D", 1), ("MAX_POOL_2D", 1), ("RESHAPE", 1), ("FULLY_CONNECTED", 1), ("SOFTMAX", 1)])
        graph, = report["subgraphs"]
        self.assertEqual((graph["name"], graph["inputs"], graph["outputs"]), ("main", [0], [20]))
        self.assertEqual([(row["opcode_index"], row["inputs"], row["outputs"]) for row in graph["operators"]], [
            (0, [0, 5, 1], [12]), (0, [12, 6, 2], [13]), (1, [13], [14]),
            (0, [14, 7, 3], [15]), (0, [15, 8, 4], [16]), (1, [16], [17]),
            (2, [17, 10], [18]), (3, [18, 11, 9], [19]), (4, [19], [20])])
        tensors = graph["tensors"]
        self.assertEqual(len(tensors), 21)
        self.assertEqual(tensors[0]["shape"], [1, 32, 32, 3])
        self.assertEqual(tensors[0]["shape_signature"], [-1, 32, 32, 3])
        self.assertEqual(tensors[20]["shape"], [1, 16])
        self.assertEqual(tensors[10]["int32_constant"], [-1, 400])
        self.assertTrue(all(row["quantization_fields"] == [] and row["has_rank"] for row in tensors))
        buffers = report["buffers"]
        self.assertEqual(len(buffers), 24)
        self.assertEqual((buffers[12]["data_offset"], buffers[12]["bytes"], buffers[12]["sha256"]),
            (536, 25600, "feebae6e213cd6e8c90ebbb400cc5d2b7ac97084961d00be0cd49593b8d15c85"))
        self.assertIsNone(buffers[1]["data_offset"])
        for row in buffers:
            value = b"" if row["data_offset"] is None else self.model[row["data_offset"]:row["data_offset"] + row["bytes"]]
            self.assertEqual(hashlib.sha256(value).hexdigest(), row["sha256"])

    def test_options_distinguish_serialized_values_from_format_defaults(self):
        ops = self.report["subgraphs"][0]["operators"]
        conv = ops[0]["options"]
        self.assertEqual((conv["padding"], conv["stride_w"], conv["stride_h"], conv["fused_activation"]), (1, 1, 1, 1))
        self.assertEqual(conv["defaulted_fields"], ["dilation_w_factor", "dilation_h_factor"])
        pool = ops[2]["options"]
        self.assertEqual((pool["stride_w"], pool["stride_h"], pool["filter_width"], pool["filter_height"]), (2, 2, 2, 2))
        self.assertEqual(pool["defaulted_fields"], ["fused_activation"])
        self.assertIsNone(ops[6]["options"])
        dense = ops[7]["options"]
        self.assertEqual(dense["defaulted_fields"], ["fused_activation", "weights_format", "keep_num_dims", "asymmetric_quantize_inputs"])
        self.assertEqual(ops[8]["options"]["beta"], 1.0)
        self.assertEqual(ops[8]["options"]["defaulted_fields"], [])
        self.assertEqual(self.report["metadata"][0]["nul_terminated_ascii"], "1.5.0")
        signature, = self.report["signature_defs"]
        self.assertEqual((signature["name"], signature["inputs"], signature["outputs"]),
            ("serving_default", [{"name": "conv2d_input", "tensor_index": 0}], [{"name": "dense", "tensor_index": 20}]))

    def test_shared_vtable_after_table_is_accepted(self):
        tensors = self.report["subgraphs"][0]["tensors"]
        self.assertTrue(any(struct.unpack_from("<i", self.model, row["table_offset"])[0] < 0 for row in tensors))
        self.assertEqual(self.decode(self.model)["model_sha256"], self.report["model_sha256"])

    def test_truncation_invalid_offsets_and_oversized_vectors_are_rejected(self):
        root = self.report["root_table_offset"]
        opcode_vector_slot = self.slot(root, 1)
        opcode_vector = opcode_vector_slot + struct.unpack_from("<I", self.model, opcode_vector_slot)[0]
        for changed in (self.model[:7], self.model[:-1], self.changed(0, "<I", len(self.model)),
                self.changed(root, "<i", 0x7fffffff), self.changed(opcode_vector_slot, "<I", 0),
                self.changed(opcode_vector, "<I", 0xffffffff)):
            with self.subTest(bytes=len(changed)), self.assertRaises(ValueError):
                self.decode(changed)

    def test_bad_indices_and_union_discriminator_are_rejected(self):
        graph = self.report["subgraphs"][0]
        tensor = graph["tensors"][0]
        op = graph["operators"][2]
        input_slot = self.slot(op["table_offset"], 1)
        input_vector = input_slot + struct.unpack_from("<I", self.model, input_slot)[0]
        for changed, error in ((self.changed(self.slot(tensor["table_offset"], 2), "<I", 24), "buffer index"),
                (self.changed(self.slot(op["table_offset"], 0), "<I", 5), "opcode index"),
                (self.changed(input_vector + 4, "<i", -1), "tensor index"),
                (self.changed(self.slot(op["table_offset"], 3), "<B", 1), "options type")):
            with self.subTest(error=error), self.assertRaisesRegex(ValueError, error):
                self.decode(changed)

    def test_string_table_and_constant_size_boundaries_are_enforced(self):
        root = self.report["root_table_offset"]
        vtable = root - struct.unpack_from("<i", self.model, root)[0]
        description_slot = self.slot(root, 3)
        description = description_slot + struct.unpack_from("<I", self.model, description_slot)[0]
        description_end = description + 4 + struct.unpack_from("<I", self.model, description)[0]
        shape_tensor = self.report["subgraphs"][0]["tensors"][10]
        shape_slot = self.slot(shape_tensor["table_offset"], 0)
        shape_vector = shape_slot + struct.unpack_from("<I", self.model, shape_slot)[0]
        for changed, error in ((self.changed(description_end, "<B", 1), "NUL terminated"),
                (self.changed(vtable, "<H", 3), "table header"),
                (self.changed(vtable + 2, "<H", 4), "table field"),
                (self.changed(shape_vector + 4, "<i", 3), "constant bytes")):
            with self.subTest(error=error), self.assertRaisesRegex(ValueError, error):
                self.decode(changed)

    def test_resource_identity_absence_and_ambiguity_are_rejected(self):
        from host import _lighting_ai_flatbuffer_resource
        resource = self.report["resource"]
        changed = bytearray(self.raw)
        changed[resource["file_offset"] + 536] ^= 1
        with self.assertRaisesRegex(ValueError, "identity requires review"):
            _lighting_ai_flatbuffer_resource(bytes(changed))
        for rows in ([], [resource, resource]):
            with self.subTest(rows=len(rows)), patch("host.pe_resource_entries", return_value=rows), \
                    self.assertRaisesRegex(ValueError, "absent or ambiguous"):
                _lighting_ai_flatbuffer_resource(self.raw)


class LightingAiModelChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_ai_model_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_ai_model_chain(cls.audit)

    def verify(self, report):
        from host import _lighting_ai_model_verify
        return _lighting_ai_model_verify(report)

    def test_current_source_release_model_graph_and_capture_failures(self):
        report, native = self.report, self.report["native"]
        self.assertEqual(report["current_ai_release"]["version"], "4.0.896")
        self.assertEqual(report["current_ai_release"]["declaration"]["resourceVersion"], "1.0.1.0")
        self.assertEqual(native["input"]["sha256"], "20604ff3a2193710a76147931da4528ae1092ad89149e517f7b95c8996d3e191")
        self.assertEqual((len(native["complete_selected_functions"]), len(native["verified_call_edges"])), (109, 83))
        self.assertEqual((native["functions_total"], native["functions_captured"]), (14934, 14934))
        self.assertEqual(native["selected_pseudocode_failures"], [])
        self.assertEqual(len(native["failures"]), 1)
        self.assertEqual(native["failures"][0]["stage"], "2c83d0:hexrays")
        self.assertFalse(native["all_pseudocode_captured"])
        model = report["model"]
        self.assertEqual((model["version"], model["model_bytes"], model["description"]), (3, 59336, "MLIR Converted."))
        graph, = model["subgraphs"]
        self.assertEqual((len(graph["tensors"]), len(graph["operators"]), len(model["buffers"])), (21, 9, 24))
        self.assertEqual((graph["inputs"], graph["outputs"]), ([0], [20]))
        self.assertEqual(graph["tensors"][0]["shape"], [1, 32, 32, 3])
        self.assertEqual(graph["tensors"][20]["shape"], [1, 16])
        self.assertEqual([graph["tensors"][i]["shape"] for i in (12, 13, 14, 15, 16, 17, 18)],
            [[1, 30, 30, 16], [1, 28, 28, 16], [1, 14, 14, 16], [1, 12, 12, 16],
                [1, 10, 10, 16], [1, 5, 5, 16], [1, 400]])

    def test_actual_five_builtin_version_ranges_and_pe_callback_tables(self):
        report = self.report
        bindings = {row["builtin_code"]: row for row in report["operator_bindings"]}
        self.assertEqual({code: row["registered_versions_inclusive"] for code, row in bindings.items()},
            {3: [1, 8], 17: [1, 3], 22: [1, 1], 9: [1, 11], 25: [1, 3]})
        self.assertEqual(bindings[3]["callbacks"], {"init": "0x4bcf0", "free": "0x4bd60", "prepare": "0x4db30", "invoke": "0x4db40"})
        image_base = int(report["native"]["image_base"], 16)
        regions = {row["rva"]: row for row in report["native"]["verified_data_regions"]}
        for op in report["model"]["operator_codes"]:
            binding = bindings[op["effective_builtin_code"]]
            self.assertEqual((op["version"], binding["model_version"]), (1, 1))
            callbacks = struct.unpack("<4Q", bytes.fromhex(regions[binding["registration_rva"]]["raw_hex"])[:32])
            self.assertEqual(callbacks, tuple(image_base + int(binding["callbacks"][name], 16) for name in ("init", "free", "prepare", "invoke")))
        self.assertEqual(len(regions), 7)
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in regions.values()))

    def test_actual_mounted_callers_startup_gates_and_sixteen_primary_colors(self):
        report = self.report
        self.assertEqual(set(report["selected"]), {"initialize_ai_generator", "initialize_ai_action", "media_parameters", "generator_choices", "panel", "startup"})
        owners = {row["id"]: row for row in report["complete_source_owners"]}
        startup = owners[report["selected"]["startup"]]
        self.assertEqual(startup["body_sha256"], "9b12cfae75b3684098993950ebc969cfba05240fc74d4713a9ed9d493139e25c")
        self.assertIn('!(0,u.I4)())return', startup["body"])
        self.assertIn('"RzChromaAIColorGenDLL"===e.name', startup["body"])
        self.assertIn('await t.A.initChromaAiColorGen(n,[127,65535,802917,2460111,3753048,4894788,8008747,8323327,8388352,10517857,13421772,16553138,16711680,16744192,16776960,16777215])', startup["body"])
        self.assertEqual(report["boundaries"]["caller"]["media_input"], [32, 32, 4])
        self.assertEqual(report["boundaries"]["caller"]["mounted_effect"], 18)
        self.assertFalse(report["boundaries"]["caller"]["quick_all_products_reviewed"])

    def test_normalization_float32_bytes_and_alpha_ignored_without_inference(self):
        from host import _lighting_ai_model_normalize
        vector = self.report["fixed_vectors"]["normalization_rgba"]
        raw = bytes.fromhex(vector["input_hex"])
        actual = _lighting_ai_model_normalize(raw)
        self.assertEqual(b"".join(struct.pack("<f", value) for value in actual).hex(),
            "00000000fffefe3e0000803f8180803bfffe7e3f8180003f")
        self.assertEqual(vector["expected_float32_hex"], "00000000fffefe3e0000803f8180803bfffe7e3f8180003f")
        changed_alpha = raw[:3] + b"\xff" + raw[4:7] + b"\x00"
        self.assertEqual(_lighting_ai_model_normalize(changed_alpha), actual)
        self.assertEqual(_lighting_ai_model_normalize(b"\x00\x00\x00\xff\xff\xff\xff\x00"), [0.0] * 3 + [1.0] * 3)
        with self.assertRaises(ValueError):
            _lighting_ai_model_normalize(b"\x00\x01\x02")
        self.assertEqual(self.report["fixed_vectors"]["actual_inference_results"], "not_executed")

    def test_ranking_descends_ties_stay_stable_and_color_mapping_is_not_scores(self):
        from host import _lighting_ai_model_rank
        self.assertEqual(_lighting_ai_model_rank(list(range(16))), self.report["fixed_vectors"]["rank_scores_ascending"])
        self.assertEqual(_lighting_ai_model_rank([0.0] * 16), self.report["fixed_vectors"]["rank_all_equal_primary_order"])
        scores = [0.0] * 16
        scores[2] = scores[5] = 1.0
        self.assertEqual(_lighting_ai_model_rank(scores, list(range(16))), [2, 5, 0, 1, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
        scores[5] += 2.0 ** -25
        self.assertEqual(_lighting_ai_model_rank(scores, list(range(16)))[:2], [2, 5])
        self.assertEqual(_lighting_ai_model_rank(list(range(16)), [123] * 16), [123] * 16)
        for scores in ([0.0] * 15, [float("nan")] * 16, [float("inf")] * 16, [1e39] * 16):
            with self.subTest(scores=scores[0]), self.assertRaises(ValueError):
                _lighting_ai_model_rank(scores)

    def test_real_input_and_invoke_status_boundaries_are_preserved(self):
        boundary = self.report["boundaries"]
        self.assertTrue(boundary["input"]["checks_input_tensor_float_type"])
        self.assertFalse(boundary["input"]["checks_typed_input_pointer_before_pixel_writes"])
        self.assertFalse(boundary["input"]["requires_exact_model_width_height"])
        self.assertFalse(boundary["input"]["input_buffer_size_or_tensor_shape_checked"])
        rows = self.report["instruction_evidence"]["input_guards_normalization_unchecked_invoke_and_ranking"]["instructions"]
        self.assertEqual(sum(row["text"].startswith("divss") for row in rows), 3)
        at = next(i for i, row in enumerate(rows) if row["rva"] == "0x12a7")
        self.assertEqual(rows[at]["text"], "call    sub_18001AB20")
        self.assertEqual([row["text"].split()[0] for row in rows[at + 1:at + 10]],
            ["mov", "mov", "mov", "mov", "mov", "mov", "imul", "mov", "movss"])
        self.assertFalse(boundary["output"]["invoke_status_checked_by_rank_caller"])
        self.assertFalse(boundary["output"]["zero_output_is_inference_success"])
        self.assertEqual(self.report["fixed_vectors"]["structural_failure"], {"output_uint32": [0] * 16, "inference_executed": False})

    def test_initialization_normal_cleanup_and_real_failure_leak_are_distinguished(self):
        boundary = self.report["boundaries"]
        self.assertTrue(boundary["initialization"]["allocate_tensors_status_checked"])
        self.assertTrue(boundary["initialization"]["allocate_failure_deletes_interpreter"])
        self.assertTrue(boundary["initialization"]["builder_missing_interpreter_clears_model_without_release"])
        self.assertFalse(boundary["initialization"]["outer_create_requires_valid_interpreter"])
        self.assertFalse(boundary["initialization"]["native_checks_complete_flatbuffer_bounds"])
        rows = {row["rva"]: row["text"] for row in self.report["instruction_evidence"]["model_build_allocate_and_failure_ownership"]["instructions"]}
        self.assertEqual(rows["0x167e"], "call    sub_18001A6C0")
        self.assertEqual(rows["0x1683"], "test    eax, eax")
        self.assertEqual(rows["0x16a0"], "call    sub_180019D70")
        self.assertEqual(rows["0x16b6"], "mov     qword ptr [rax], 0")
        self.assertEqual(boundary["lifetime"]["destroy_order"],
            ["interpreter_and_subgraphs", "model_memory_allocation_wrapper", "generator", "export_wrapper"])
        self.assertTrue(boundary["lifetime"]["native_cancel_callback_supported_in_generic_interpreter"])
        self.assertFalse(boundary["lifetime"]["generator_exports_expose_cancel_or_install_callback"])
        self.assertFalse(boundary["lifetime"]["generator_export_inflight_drain"])

    def test_default_delegate_is_real_and_host_partition_selection_remains_unverified(self):
        native, execution = self.report["native"], self.report["boundaries"]["execution"]
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]],
            ["0x4ab0", "0x3024c0", "0x304800", "0x304990", "0x3049a0"])
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in native["verified_indirect_slots"]))
        self.assertTrue(execution["default_delegate_creator_registered"])
        self.assertTrue(execution["delegate_can_replace_execution_plan"])
        self.assertTrue(execution["delegate_apply_failure_can_restore_original_plan"])
        self.assertFalse(execution["delegate_enabled_unconditionally_on_every_host"])
        self.assertFalse(execution["all_optimized_microkernel_numeric_semantics_reviewed"])
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x2ef970", "0x2efdb0", "0x2f1b90", "0x304800", "0x3049a0", "0x2ef6a0", "0x289610"} <= functions)

    def test_exact_upstream_pointer_change_rejected_without_whole_mutable_receipt_dependency(self):
        import analysis as a
        import host
        previous = json.loads((a.ROOT / ".analysis/results/host-current/local-audit.json").read_bytes())
        dependencies = host._lighting_ai_model_dependencies(previous)
        self.assertEqual(dependencies, self.report["dependencies"])
        self.assertTrue(all(row["pointer"].startswith("/lighting_") for row in dependencies))
        changed = copy.deepcopy(previous)
        changed["lighting_hdr_decoder_chain"]["remaining_chains"][1]["status"] += " changed"
        with patch.object(host, "_lighting_hdr_decoder_verify", return_value={}), self.assertRaisesRegex(ValueError, "exact upstream pointer changed"):
            host._lighting_ai_model_dependencies(changed)
        previous["unrelated_parallel_publication"] = {"status": "changed"}
        self.assertEqual(host._lighting_ai_model_dependencies(previous), dependencies)
        self.assertNotIn(".analysis/results/host-current/local-audit.json", self.audit.artifacts)

    def test_source_owner_graph_pseudocode_table_and_instruction_changes_rejected(self):
        import host
        changed = copy.deepcopy(self.report)
        changed["complete_source_owners"][0]["body"] += " "
        with self.assertRaisesRegex(ValueError, "complete source owner differs"):
            self.verify(changed)
        mutations = (
            lambda row: row["model"]["subgraphs"][0]["operators"][8]["options"].update(beta=2.0),
            lambda row: row["native"]["complete_selected_functions"][0].update(pseudocode_text_sha256="0" * 64),
            lambda row: row["native"]["verified_data_regions"][0].update(raw_hex="00" * 88),
            lambda row: row["instruction_evidence"]["actual_reshape_version"]["instructions"][0].update(text="nop"))
        for mutate in mutations:
            changed = copy.deepcopy(self.report)
            mutate(changed)
            changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(host._lighting_ai_model_projection(changed),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(changed)

    def test_static_stages_json_snapshot_and_no_temporary_sessions(self):
        report = json.loads(json.dumps(self.report, ensure_ascii=False, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        for stage in ("source_chain_complete", "target_execution", "product_code_modified", "real_device_operations"):
            self.assertFalse(report[stage])
        for stage in ("actual_dll_comparison", "runtime_acceptance", "ui_backend_wiring"):
            self.assertEqual(report[stage], "unverified")
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(report["maintenance_lifecycle"]["analysis_sessions_created"], [])
        self.assertEqual(report["changed_during_query"], [])
        self.assertEqual(self.audit.final_snapshot(), [])
        changed = copy.deepcopy(report)
        changed["actual_dll_comparison"] = "passed"
        with self.assertRaisesRegex(ValueError, "static boundary was promoted"):
            self.verify(changed)


class LightingHdrDecoderChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_hdr_decoder_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_hdr_decoder_chain(cls.audit)

    def verify(self, report):
        from host import _lighting_hdr_decoder_verify
        return _lighting_hdr_decoder_verify(report)

    def image(self, payload, width=1, height=1, signature=b"#?RADIANCE", format_line=b"FORMAT=32-bit_rle_rgbe", layout=None):
        if layout is None:
            layout = f"-Y {height} +X {width}".encode()
        return signature + b"\n" + format_line + b"\n\n" + layout + b"\n" + payload

    def decode(self, data, requested=4):
        from host import _lighting_hdr_decoder_pixels
        return _lighting_hdr_decoder_pixels(data, requested)

    def instructions(self, role):
        return {row["rva"]: row for row in self.report["instruction_evidence"][role]["instructions"]}

    def test_current_complete_selection_actual_calls_constants_and_global_failures(self):
        native = self.report["native"]
        self.assertEqual(self.report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"], "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 21)
        self.assertEqual(len(native["verified_call_edges"]), 29)
        functions = {row["rva"] for row in native["complete_selected_functions"]}
        self.assertTrue({"0x8e160", "0x9c280", "0x9c0d0", "0x8e920", "0x1530f0", "0x1593f0", "0x15a860"} <= functions)
        self.assertFalse({"0x8b480", "0x98c80", "0x945c0", "0x154300"} & functions)
        edge = next(row for row in native["verified_call_edges"] if row["target_rva"] == "0x154300")
        self.assertEqual(edge["instruction_addresses"], ["0x18008ea15"])
        self.assertEqual([row["target_rva"] for row in native["verified_indirect_slots"]], ["0x8b3e0", "0x8b400", "0x8b440"])
        self.assertEqual(len(native["verified_data_regions"]), 9)
        regions = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in native["verified_data_regions"]}
        self.assertEqual(regions["format_exact_overlapping_strings"][:16], b"32-bit_rle_rgbe\x00")
        self.assertEqual(struct.unpack("<d", regions["double_one"]), (1.0,))
        self.assertEqual(struct.unpack("<2f", regions["default_inverse_gamma_and_scale"]), (0.45454543828964233, 1.0))
        self.assertEqual(len(self.report["instruction_evidence"]), 11)
        self.assertEqual(native["selected_pseudocode_failures"], [])
        self.assertFalse(native["all_pseudocode_captured"])
        self.assertEqual(len(native["failures"]), 10)

    def test_header_exact_lines_direction_signed_dimensions_and_metadata_before_failure(self):
        payload = bytes.fromhex("80402081")
        self.assertTrue(self.decode(self.image(payload, signature=b"#?RGBE"))["success"])
        for changed, error in ((self.image(payload, signature=b"#?OTHER"), "not HDR"),
                (self.image(payload, format_line=b"FORMAT=32-bit_rle_rgbe "), "unsupported format"),
                (self.image(payload, format_line=b"FORMAT=32-bit_rle_rgbe\r"), "unsupported format"),
                (self.image(payload, layout=b"+Y 1 +X 1"), "unsupported data layout"),
                (self.image(payload, layout=b"-Y 1 -X 1"), "unsupported data layout"),
                (self.image(payload, layout=b"-Y 1\t+X 1"), "unsupported data layout")):
            with self.subTest(error=error):
                self.assertEqual(self.decode(changed)["error"], error)
        result = self.decode(self.image(payload, layout=b"-Y +1   +X +1trailing"))
        self.assertTrue(result["success"])
        self.assertEqual((result["width"], result["height"], result["source_components"]), (1, 1, 3))
        result = self.decode(self.image(b"", width=-1))
        self.assertEqual(result["error"], "too large")
        self.assertEqual((result["width"], result["height"], result["source_components"]), (-1, 1, 3))
        result = self.decode(self.image(b"", width=16777217))
        self.assertIsNone(result["width"])
        self.assertIsNone(result["source_components"])
        rows = self.instructions("header_format_layout_metadata_and_alloc")
        self.assertEqual(rows["0x8e344"]["text"], "mov     [r12], ebx")
        self.assertEqual(rows["0x8e350"]["text"], "mov     dword ptr [r14], 3")
        self.assertTrue(rows["0x8e370"]["text"].endswith("sub_18009BDD0"))
        self.assertFalse(self.report["boundaries"]["entry"]["explicit_nonzero_dimension_check"])

    def test_overlong_comment_is_drained_and_cr_is_not_trimmed(self):
        source = b"#?RADIANCE\n" + b"x" * 1100 + b"\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 1\n" + bytes.fromhex("80402081")
        result = self.decode(source)
        self.assertTrue(result["success"])
        self.assertEqual(result["bytes_consumed"], len(source))
        self.assertEqual(self.decode(self.image(b"", signature=b"#?RADIANCE\r"))["error"], "not HDR")
        rows = self.instructions("bounded_line_reader")
        self.assertEqual(rows["0x9c1a8"]["text"], "cmp     rbx, 3FEh")
        self.assertEqual(rows["0x9c265"]["text"], "mov     ebx, 3FFh")
        self.assertEqual(rows["0x9c26a"]["text"], "mov     byte ptr [rsi+rbx], 0")

    def test_fixed_rgbe_zero_subnormal_maximum_and_gray_alpha_arithmetic(self):
        from host import _lighting_hdr_decoder_rgbe
        for row in self.report["fixed_vectors"]["rgbe"]:
            actual = _lighting_hdr_decoder_rgbe(bytes.fromhex(row["input_hex"]), row["requested"])
            with self.subTest(input=row["input_hex"], requested=row["requested"]):
                self.assertEqual(b"".join(struct.pack("<f", value) for value in actual).hex(), row["expected_float_le_hex"])
        self.assertEqual(_lighting_hdr_decoder_rgbe(b"\x01\x01\x01\x81", 1), [0.0078125])
        rows = self.instructions("complete_rgbe_conversion")
        self.assertEqual(rows["0x9c298"]["text"], "add     edx, 0FFFFFF78h")
        self.assertEqual(rows["0x9c2ab"]["text"], "cvtsd2ss xmm0, xmm0")
        self.assertEqual(rows["0x9c34d"]["text"], "mov     dword ptr [rsi+0Ch], 3F800000h")
        for pixel, requested in ((b"", 4), (b"1234", 0), (b"1234", 5)):
            with self.assertRaisesRegex(ValueError, "four bytes"):
                _lighting_hdr_decoder_rgbe(pixel, requested)

    def test_flat_pixels_do_not_expand_legacy_repeat_marker(self):
        source = self.image(bytes.fromhex("8040208101010181"), width=2)
        result = self.decode(source)
        self.assertTrue(result["success"])
        self.assertEqual(result["paths"], ["flat"])
        self.assertEqual(result["float_le_hex"], "0000803f0000003f0000803e0000803f" + "0000003c" * 3 + "0000803f")
        self.assertEqual(result["bytes_consumed"], len(source))
        self.assertFalse(self.report["boundaries"]["encoding"]["legacy_111_repeat_marker_expanded"])

    def test_new_rle_repeat_and_literal_planes_both_return_rgba_float(self):
        pixel_hex = "0000003e0000803e0000c03e0000803f"
        repeat = bytes.fromhex("020200088810882088308881")
        literal = bytes.fromhex("02020008") + b"\x08\x10" + b"\x10" * 7 + b"\x08\x20" + b"\x20" * 7 + b"\x08\x30" + b"\x30" * 7 + b"\x08\x81" + b"\x81" * 7
        for payload in (repeat, literal):
            source = self.image(payload, width=8)
            result = self.decode(source)
            with self.subTest(payload=payload.hex()):
                self.assertTrue(result["success"])
                self.assertEqual(result["paths"], ["rle"])
                self.assertEqual(result["float_le_hex"], pixel_hex * 8)
                self.assertEqual(result["bytes_consumed"], len(source))
        payload = bytes.fromhex("02020080") + b"\x80" + b"\x10" * 128 + b"\x80" + b"\x20" * 128 + b"\x80" + b"\x30" * 128 + b"\x80" + b"\x81" * 128
        self.assertEqual(self.decode(self.image(payload, width=128))["float_le_hex"], pixel_hex * 128)

    def test_second_row_nonnew_header_resets_entire_output_and_overwrites_old_rows(self):
        prior = bytes.fromhex("020200088810882088308881")
        fallback = bytes.fromhex("80402081") * 16
        source = self.image(prior + fallback, width=8, height=2)
        result = self.decode(source)
        self.assertTrue(result["success"])
        self.assertEqual(result["paths"], ["rle", "flat"])
        self.assertEqual(result["fallback_rows"], [1])
        self.assertEqual(result["float_le_hex"], "0000803f0000003f0000803e0000803f" * 16)
        self.assertEqual(result["bytes_consumed"], len(source))
        rows = self.instructions("rle_errors_fallback_release_and_ignored_raw_read")
        self.assertEqual(rows["0x8e84e"]["text"], "mov     rcx, [rsp+4F8h+var_4D8]")
        self.assertEqual(rows["0x8e867"]["text"], "xor     r14d, r14d")
        self.assertEqual(rows["0x8e86a"]["text"], "mov     r12d, 1")

    def test_new_rle_width_zero_and_overrun_fail_but_missing_values_can_be_zero(self):
        for payload, error in ((bytes.fromhex("02020009"), "invalid decoded scanline length"),
                (bytes.fromhex("0202000800"), "corrupt"),
                (bytes.fromhex("0202000889aa"), "corrupt"),
                (bytes.fromhex("0202000809"), "corrupt")):
            result = self.decode(self.image(payload, width=8))
            with self.subTest(payload=payload.hex()):
                self.assertFalse(result["success"])
                self.assertEqual(result["error"], error)
                self.assertEqual((result["width"], result["height"], result["source_components"]), (8, 1, 3))
        result = self.decode(self.image(bytes.fromhex("0202000888108820883088"), width=8))
        self.assertTrue(result["success"])
        self.assertEqual(result["float_le_hex"], "0000000000000000000000000000803f" * 8)
        encoding = self.report["boundaries"]["encoding"]
        self.assertEqual(encoding["new_rle_width_inclusive_range"], [8, 32767])
        self.assertEqual(encoding["literal_counts"], [1, 128])
        self.assertFalse(encoding["all_truncated_payloads_rejected"])

    def test_raw_short_read_zero_size_and_unterminated_header_remain_unobserved(self):
        for source, reason in ((self.image(b"\x80\x40"), "raw short-read"),
                (self.image(b"", width=0), "zero-size"), (b"#?RADIANCE", "unterminated header")):
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, reason):
                self.decode(source)
        rows = self.instructions("rle_errors_fallback_release_and_ignored_raw_read")
        self.assertTrue(rows["0x8e8e0"]["text"].endswith("sub_180091C90"))
        self.assertEqual(rows["0x8e8e5"]["text"], "mov     eax, ebx")
        self.assertTrue(self.report["boundaries"]["encoding"]["truncated_flat_can_use_uninitialized_or_previous_stack_bytes"])

    def test_finite_post_pow_quantization_actual_operands_linear_alpha_and_gray(self):
        from host import _lighting_hdr_decoder_quantize
        for row in self.report["fixed_vectors"]["quantization"]:
            self.assertEqual(_lighting_hdr_decoder_quantize(row["samples"]).hex(), row["expected_hex"])
        for value in (float("inf"), float("-inf"), float("nan")):
            with self.assertRaisesRegex(ValueError, "nonfinite"):
                _lighting_hdr_decoder_quantize([value])
        rows = self.instructions("current_pow_operands_and_opaque_linear_alpha")
        self.assertEqual(rows["0x8ea00"]["text"], "movss   xmm1, cs:dword_1801F5998")
        self.assertEqual(rows["0x8ea0d"]["text"], "mulss   xmm0, cs:dword_1801F599C")
        self.assertTrue(rows["0x8ea15"]["text"].endswith("sub_180154300"))
        self.assertEqual(rows["0x8ea30"]["text"], "cvttss2si eax, xmm0")
        self.assertEqual(rows["0x8ea54"]["text"], "mulss   xmm0, xmm6")
        self.assertFalse(self.report["boundaries"]["pixels"]["byte_alpha_uses_pow"])
        self.assertFalse(self.report["boundaries"]["pixels"]["one_component_converter_uses_gamma"])
        self.assertFalse(self.report["boundaries"]["scope"]["synthetic_quantization_executes_pow"])

    def test_actual_cleanup_for_invalid_rle_width_scratch_oom_and_byte_output(self):
        rows = self.instructions("rle_errors_fallback_release_and_ignored_raw_read")
        for site in ("0x8e7f2", "0x8e7fc", "0x8e862", "0x8e875", "0x8e87d", "0x8e891", "0x8e8a9"):
            self.assertTrue(rows[site]["text"].endswith("sub_18014FBA0"))
        byte_failure = self.instructions("byte_allocation_failure_release")
        self.assertTrue(byte_failure["0x8ea86"]["text"].endswith("sub_18014FBA0"))
        self.assertTrue(self.instructions("byte_normal_release")["0x8eb88"]["text"].endswith("sub_18014FBA0"))
        self.assertFalse(self.report["boundaries"]["ownership"]["strict_raw_short_read_failure_or_initialized_output_guarantee"])
        self.assertFalse(self.report["boundaries"]["ownership"]["cpp_exception_unwind_or_heap_fault_recovery_established"])

    def test_boundary_mutations_and_rehashed_native_vectors_dependencies_are_rejected(self):
        from host import _lighting_hdr_decoder_projection
        for group, field, value in (("entry", "other_orientations_supported", True),
                ("entry", "explicit_nonzero_dimension_check", True), ("entry", "cr_stripped", True),
                ("encoding", "legacy_111_repeat_marker_expanded", True), ("encoding", "flat_exact_read_return_checked", True),
                ("pixels", "rgb_half_unit_bias_added", True), ("pixels", "byte_alpha_uses_pow", True),
                ("ownership", "cpp_exception_unwind_or_heap_fault_recovery_established", True)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)
        for mutate in (lambda r: r["current_native_release"].update(version="old"),
                lambda r: r["native"]["complete_selected_functions"].pop(),
                lambda r: r["native"]["verified_call_edges"][0].update(target_rva="0x945c0"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00"),
                lambda r: r["instruction_evidence"]["complete_rgbe_conversion"]["instructions"].clear(),
                lambda r: r["reviewed_semantics"].update(encoding="legacy repeats expanded"),
                lambda r: r["fixed_vectors"]["rgbe"][0].update(expected_float_le_hex="00" * 16),
                lambda r: r["dependencies"][0].update(pointer="/unreviewed"), lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_hdr_decoder_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_exact_upstream_pointers_snapshot_remaining_chains_and_execution_separation(self):
        from host import _lighting_hdr_decoder_dependencies
        from analysis import semantic_query_gaps
        sections = {"lighting_jpeg_decoder_chain", "lighting_media_codec_boundaries", "lighting_media_resampler"}
        previous = {key: value for key, value in json.loads((self.audit.a.ROOT / ".analysis/results/host-current/local-audit.json").read_bytes()).items() if key in sections}
        self.assertEqual(_lighting_hdr_decoder_dependencies(previous), self.report["dependencies"])
        changed = copy.deepcopy(previous)
        changed["lighting_jpeg_decoder_chain"]["remaining_chains"][0]["owner"] = "changed"
        with patch("host._lighting_jpeg_decoder_verify", return_value={}):
            with self.assertRaisesRegex(ValueError, "exact upstream pointer changed"):
                _lighting_hdr_decoder_dependencies(changed)
        findings = [row for row in semantic_query_gaps(self.report, ".analysis/results/host-current/local-audit.json") if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual(len(findings), 3)
        self.assertEqual(self.report["remaining_chains"][0]["owner"], "HDR setters/compiler pow")
        for field, value in (("source_chain_complete", True), ("target_execution", True), ("real_device_operations", True),
                ("rust_independent_implementation", "verified"), ("actual_dll_comparison", "verified"),
                ("ui_backend_wiring", "verified"), ("runtime_acceptance", "verified"), ("snapshot_stable", False)):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["verification"]["compiler_pow_and_setters_reviewed"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(report["maintenance_lifecycle"]["analysis_sessions_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])


class LightingHdrParametersChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_hdr_parameters_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_hdr_parameters_chain(cls.audit)

    def verify(self, report):
        from host import _lighting_hdr_parameters_verify
        return _lighting_hdr_parameters_verify(report)

    def instructions(self, role):
        return {row["rva"]: row for row in self.report["instruction_evidence"][role]["instructions"]}

    def regions(self):
        return {row["role"]: bytes.fromhex(row["raw_hex"]) for row in self.report["native"]["verified_data_regions"]}

    def test_current_complete_functions_actual_edges_and_original_failures(self):
        native = self.report["native"]
        self.assertEqual(self.report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(native["input"]["sha256"], "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(native["complete_selected_functions"]), 13)
        self.assertEqual(len(native["verified_call_edges"]), 21)
        self.assertEqual(len(native["verified_data_regions"]), 11)
        self.assertEqual(len(self.report["actual_exports"]), 15)
        self.assertEqual(native["selected_pseudocode_failures"], [])
        self.assertFalse(native["all_pseudocode_captured"])
        self.assertEqual(len(native["failures"]), 10)
        self.assertEqual(sum(len(row["instructions"]) for row in self.report["instruction_evidence"].values()), 1106)

    def test_shared_globals_are_distinct_from_tls_and_have_only_captured_reads(self):
        inventory = self.report["reference_inventory"]
        self.assertEqual(inventory["complete_captured_functions"], 8694)
        self.assertEqual(inventory["parameter_storage_section"]["name"], ".data")
        self.assertEqual(inventory["separate_tls_section"]["name"], ".tls")
        self.assertEqual([row["instruction_rva"] for row in inventory["captured_instruction_references"]], ["0x8ea00", "0x8ea0d"])
        self.assertEqual(inventory["absolute_va_file_offsets"], {"0x1f5998": [], "0x1f599c": []})
        self.assertEqual(struct.unpack("<4f", self.regions()["separate_ldr_and_hdr_global_defaults"]),
            (2.200000047683716, 1.0, 0.45454543828964233, 1.0))
        self.assertFalse(self.report["boundaries"]["parameters"]["thread_local"])
        self.assertFalse(self.report["boundaries"]["parameters"]["exported_setter_established"])
        self.assertFalse(self.report["boundaries"]["parameters"]["ui_control_or_mutation_api_inferred"])

    def test_reference_inventory_retains_new_store_and_absolute_pointer_counterexamples(self):
        from host import _lighting_hdr_parameters_references
        assembly = "\n".join((
            "0x18008ea00  f30f100d906f1600  movss   xmm1, cs:dword_1801F5998",
            "0x180001000  90  movss   cs:dword_1801f599c, xmm0",
            "0x180001001  90  movss   xmm1, cs:dword_1801F5990"))
        report = _lighting_hdr_parameters_references(assembly, b"abc" + struct.pack("<Q", 0x1801f599c), 0x180000000)
        self.assertEqual(len(report["captured_instruction_references"]), 2)
        self.assertIn("cs:dword_1801f599c, xmm0", report["captured_instruction_references"][1]["text"])
        self.assertEqual(report["absolute_va_file_offsets"], {"0x1f5998": [], "0x1f599c": [3]})

    def test_float_abi_rgb_before_pow_and_linear_alpha_are_actual_instructions(self):
        rows = self.instructions("hdr_rgb_operands_linear_alpha_and_release")
        self.assertEqual(rows["0x8ea00"]["text"], "movss   xmm1, cs:dword_1801F5998")
        self.assertEqual(rows["0x8ea0d"]["text"], "mulss   xmm0, cs:dword_1801F599C")
        self.assertTrue(rows["0x8ea15"]["text"].endswith("sub_180154300"))
        self.assertEqual(sum("sub_180154300" in row["text"] for row in rows.values()), 1)
        bounds = self.report["boundaries"]
        self.assertEqual(bounds["pow"]["actual_abi"], "float_xmm0_base_float_xmm1_exponent_to_float_xmm0")
        self.assertFalse(bounds["pow"]["decompiler_signature_trusted"])
        self.assertFalse(bounds["consumer"]["alpha_reads_hdr_parameters"])
        self.assertFalse(bounds["consumer"]["pow_error_checked_before_quantization"])

    def test_reverse_converter_has_three_distinct_gamma_then_postscale_calls(self):
        edge = next(row for row in self.report["native"]["verified_call_edges"]
            if row["caller_rva"] == "0x9c5b0" and row["target_rva"] == "0x154300")
        self.assertEqual(edge["instruction_addresses"], ["0x18009c6b8", "0x18009c6e3", "0x18009c72f"])
        rows = self.instructions("reverse_ldr_distinct_gamma_and_scale")
        self.assertEqual(rows["0x9c6a0"]["text"], "movss   xmm1, cs:dword_1801F5990")
        self.assertEqual(rows["0x9c6b4"]["text"], "divss   xmm0, xmm6")
        self.assertEqual(rows["0x9c6bd"]["text"], "mulss   xmm0, cs:dword_1801F5994")
        self.assertFalse(self.report["boundaries"]["consumer"]["reverse_ldr_float_converter_is_current_byte_loader"])

    def test_pow_tables_have_all_257_log_reciprocal_and_64_exp_entries(self):
        regions = self.regions()
        logarithms = struct.unpack("<257d", regions["pow_log257_table"])
        reciprocals = struct.unpack("<257d", regions["pow_reciprocal257_table"])
        exponents = struct.unpack("<64d", regions["pow_exp64_table"])
        self.assertEqual((logarithms[0], logarithms[256]), (0.0, 0.6931471805599453))
        self.assertEqual((reciprocals[0], reciprocals[256]), (2.0, 1.0))
        self.assertEqual(exponents[0], 1.0)
        constants = regions["pow_scalar_and_near_one_constants"]
        self.assertEqual(struct.unpack_from("<2d", constants, 16), (88.72283935546875, -103.2789306640625))
        self.assertEqual(struct.unpack_from("<d", constants, 48), (0.0625,))
        self.assertTrue(all(a < b for a, b in zip(logarithms, logarithms[1:])))
        self.assertTrue(all(a > b for a, b in zip(reciprocals, reciprocals[1:])))

    def test_cpu_flag_selects_sse_fma_but_avx_near_one_rejoins_sse(self):
        sse = self.instructions("pow_entry_sse_table_polynomial_and_return")
        self.assertEqual(sse["0x154310"]["text"], "cmp     cs:dword_1801F9B54, 0")
        self.assertEqual(sse["0x15448e"]["text"], "cvtsd2ss xmm0, xmm4")
        avx = self.instructions("avx_fma_table_polynomial_and_return")
        self.assertTrue(avx["0x154b6b"]["text"].startswith("vfmadd213sd"))
        near = self.instructions("avx_near_one_rejoins_sse_and_special_routes")
        self.assertEqual(near["0x154d1b"]["text"], "jmp     loc_1801543FE")
        cpu = self.instructions("cpu_dispatch_global_writes")
        self.assertEqual(cpu["0x17ca5d"]["text"], "and     ecx, 18001000h")
        self.assertEqual(cpu["0x17ca87"]["text"], "mov     rax, [rsp+18h+arg_0]")
        self.assertEqual(cpu["0x17cac4"]["text"], "test    bl, 20h")
        self.assertEqual(struct.unpack("<Q", self.regions()["cpu_initializer_pointer"]), (0x18017ca40,))

    def test_all_special_error_calls_and_kind_table_are_preserved(self):
        edge = next(row for row in self.report["native"]["verified_call_edges"]
            if row["caller_rva"] == "0x154300" and row["target_rva"] == "0x183890")
        self.assertEqual(len(edge["instruction_addresses"]), 23)
        self.assertEqual(struct.unpack("<8i", self.regions()["pow_error_kind_constants"]), tuple(range(2, 10)))
        self.assertEqual(struct.unpack("<9I", self.regions()["pow_error_switch9_table"]),
            (0x1838bd, 0x1838e7, 0x183916, 0x183916, 0x183916, 0x183928, 0x183954, 0x1839cd, 0x183980))
        self.assertEqual(self.regions()["actual_powf_error_label"], b"powf\0")
        self.assertTrue(self.report["boundaries"]["errors"]["kind_3_4_5_quiet_return"])
        self.assertEqual(self.report["boundaries"]["errors"]["quiet_nan_mask"], "0x400000")

    def test_crt_callback_return_is_ignored_and_underflow_does_not_force_errno34(self):
        rows = self.instructions("pow_special_result_preserved_over_crt_wrapper")
        self.assertEqual(rows["0x18389c"]["text"], "movaps  xmm6, xmm2")
        self.assertEqual(rows["0x1839c8"]["text"], "call    sub_18017C750")
        self.assertTrue(rows["0x1839cd"]["text"].startswith("movaps  xmm0, xmm6"))
        bounds = self.report["boundaries"]["errors"]
        self.assertEqual(bounds["kind_7"], {"matherr_selector": 4, "exception_mask": 18, "errno_argument": 34})
        self.assertEqual(bounds["errno_mapper_only_sets_selectors"], [1, 2, 3])
        self.assertFalse(bounds["kind_7_sets_errno_through_mapper"])
        self.assertFalse(bounds["pow_wrapper_consumes_matherr_return_value"])
        self.assertFalse(bounds["decoder_error_string_same_as_crt_errno"])
        self.assertFalse(bounds["exception_unwind_releases_converter_input_proven"])

    def test_exact_upstream_sections_and_no_temporary_artifacts(self):
        from host import _otfm_macro_encoding_dependency_changes
        self.assertEqual([row["pointer"] for row in self.report["upstream_sections"]],
            ["/lighting_hdr_decoder_chain", "/lighting_hdr_decoder_chain/remaining_chains/0"])
        self.assertEqual(_otfm_macro_encoding_dependency_changes(self.report["upstream_sections"]), [])
        self.assertEqual(self.report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.report["maintenance_lifecycle"]["analysis_sessions_created"], [])
        self.assertEqual(self.report["changed_during_query"], [])

    def test_changed_globals_instructions_upstream_or_numerical_claims_are_rejected(self):
        for mutate in (
                lambda r: r["reference_inventory"]["captured_instruction_references"].append({"instruction_rva": "0x1000", "text": "store"}),
                lambda r: r["native"]["verified_data_regions"][0].__setitem__("raw_hex", "00" * 16),
                lambda r: r["instruction_evidence"]["avx_near_one_rejoins_sse_and_special_routes"]["instructions"][0].__setitem__("text", "ret"),
                lambda r: r["upstream_sections"][0].__setitem__("canonical_sha256", "0" * 64),
                lambda r: r.__setitem__("actual_pow_numerical_results", "verified"),
                lambda r: r.__setitem__("source_chain_complete", True),
                lambda r: r["boundaries"]["parameters"].__setitem__("thread_local", True),
                lambda r: r["native"].__setitem__("failures", [])):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaises(ValueError):
                self.verify(changed)


class LightingMediaCodecBoundariesTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_media_codec_boundaries
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_media_codec_boundaries(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_media_codec_boundaries
        return _verify_lighting_media_codec_boundaries(report)

    def test_unclosed_decoder_mutation_product_and_implementation_chains_reach_ledger(self):
        from analysis import semantic_query_gaps
        evidence = ".analysis/results/host-current/local-audit.json"
        findings = [row for row in semantic_query_gaps(self.report, evidence)
            if row["kind"] == "semantic_query_chain_unresolved"]
        self.assertEqual(len(findings), 4)
        self.assertEqual([row["finding"] for row in findings], self.report["remaining_chains"])
        self.assertEqual([row["evidence"] for row in findings], [evidence] * 4)
        self.assertEqual([row["pointer"] for row in findings], ["/remaining_chains"] * 4)
        self.assertNotIn("remaining_dependencies", self.report)

    def test_current_original_complete_owners_and_real_callback_slots_are_bound(self):
        report = self.report
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(report["native"]["input"]["sha256"],
            "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa")
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 56)
        self.assertEqual(len(report["native"]["verified_call_edges"]), 63)
        self.assertEqual([row["target_rva"] for row in report["native"]["verified_indirect_slots"]],
            ["0x8b3e0", "0x8b400", "0x8b440"])
        regions = {row["role"]: bytes.fromhex(row["raw_hex"])
            for row in report["native"]["verified_data_regions"]}
        self.assertEqual(regions["pic_signature"], bytes.fromhex("5380f634"))
        self.assertEqual(struct.unpack("<2f", regions["hdr_default_inverse_gamma_and_scale"]),
            (0.45454543828964233, 1))
        self.assertEqual(len(report["instruction_evidence"]), 10)

    def test_actual_dispatch_order_keeps_nine_probes_and_selected_decoder_returns(self):
        routes = self.report["format_routes"]
        self.assertEqual([row["format"] for row in routes],
            ["png", "bmp", "gif", "psd", "pic", "jpeg", "pnm", "hdr", "tga"])
        expected = [int(row[key], 16) for row in routes for key in ("probe_rva", "decoder_rva")]
        instructions = self.report["instruction_evidence"]["all_ordered_format_branches"]["instructions"]
        base = int(self.report["native"]["image_base"], 16)
        actual = [int(ref["to"], 16) - base for row in instructions
            if row["text"].split()[0].lower() in ("call", "jmp") for ref in row["xrefs_from"]
            if ref.get("code") and int(ref["to"], 16) - base in expected]
        self.assertEqual(actual, expected)
        self.assertFalse(self.report["boundaries"]["dispatch"]["matched_decoder_failure_tries_next_format"])
        pnm = self.report["instruction_evidence"]["pnm_success_returns_without_rewind"]["instructions"]
        self.assertEqual(next(row["bytes"] for row in pnm if row["rva"] == "0x8dff9"), "c3")
        self.assertEqual(next(row["bytes"] for row in pnm if row["rva"] == "0x8dffd"), "e89e190000")

    def test_rgba8_examples_preserve_gray_alpha_and_high_bytes_without_rounding(self):
        from host import _lighting_media_codec_rgba8
        for row in self.report["fixed_vectors"]["decoded_pixels"]:
            with self.subTest(row=row):
                self.assertEqual(_lighting_media_codec_rgba8(bytes.fromhex(row["input_hex"]),
                    row["width"], row["height"], row["channels"], bits=row["bits"], flip=row["flip"]).hex(),
                    row["expected_hex"])
        cases = [(1, [0x7fff], [127, 127, 127, 255]),
            (2, [0x00ff, 0x8000], [0, 0, 0, 128]),
            (3, [0xff00, 0x7fff, 0x00ff], [255, 127, 0, 255])]
        for channels, values, expected in cases:
            self.assertEqual(_lighting_media_codec_rgba8(struct.pack("<" + "H" * channels, *values),
                1, 1, channels, bits=16), bytes(expected))
        self.assertEqual(_lighting_media_codec_rgba8(bytes.fromhex("10203000"), 1, 1, 4),
            bytes.fromhex("10203000"))

    def test_flip_preserves_wide_whole_rows_odd_middle_and_allocation_scope(self):
        from host import _lighting_media_codec_rgba8
        first, middle, last = bytes([17, 33, 49, 65]) * 513, bytes([81, 97, 113, 129]) * 513, bytes([145, 161, 177, 193]) * 513
        self.assertEqual(_lighting_media_codec_rgba8(first + middle + last, 513, 3, 4, flip=True),
            last + middle + first)
        proof = {row["rva"]: row for row in
            self.report["instruction_evidence"]["flip_full_row_chunk_swap"]["instructions"]}
        self.assertEqual(proof["0x8b7d7"]["bytes"], "41bc00080000")
        self.assertEqual(len([row for row in proof.values() if row["text"].startswith("call")]), 3)
        self.assertEqual(self.report["boundaries"]["pixels"]["flip_scratch_bytes"], 2048)

    def test_real_failure_ownership_and_hdr_alpha_must_not_be_silently_repaired(self):
        boundaries = self.report["boundaries"]
        self.assertFalse(boundaries["pixels"]["sixteen_bit_reducer_frees_input_on_allocation_failure"])
        self.assertFalse(boundaries["pixels"]["sixteen_bit_reducer_null_result_checked_before_optional_flip"])
        self.assertTrue(boundaries["ownership"]["channel_conversion_frees_input_on_allocation_failure"])
        self.assertTrue(boundaries["ownership"]["hdr_conversion_frees_input_on_allocation_failure"])
        self.assertFalse(boundaries["pixels"]["hdr_alpha_uses_gamma"])
        for group, key, value in (("pixels", "sixteen_bit_reducer_frees_input_on_allocation_failure", True),
                ("pixels", "sixteen_bit_reducer_null_result_checked_before_optional_flip", True),
                ("dispatch", "pnm_probe_rewinds_after_success", True),
                ("pixels", "hdr_alpha_uses_gamma", True),
                ("caller", "resize_result_checked", True),
                ("scope", "all_compressed_pixel_arithmetic_reviewed", True)):
            report = copy.deepcopy(self.report)
            report["boundaries"][group][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "boundary requires semantic review"):
                self.verify(report)

    def test_rehashed_route_chunk_constant_instruction_and_remaining_scope_mutations_fail(self):
        from host import _lighting_media_codec_projection
        for mutate in (lambda r: r["format_routes"].reverse(),
                lambda r: r["native"]["complete_selected_functions"][0].update(pseudocode_text_sha256="0" * 64),
                lambda r: r["native"]["verified_indirect_slots"][0].update(target_rva="0x8b400"),
                lambda r: r["native"]["verified_data_regions"][0].update(raw_hex="00000000"),
                lambda r: r["instruction_evidence"]["result8bit_and_real_conversion_parameters"]["instructions"].clear(),
                lambda r: r["remaining_chains"].clear()):
            report = copy.deepcopy(self.report)
            mutate(report)
            report["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_media_codec_projection(report),
                ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
            with self.assertRaisesRegex(ValueError, "selected bytes require semantic review"):
                self.verify(report)

    def test_failed_capture_execution_or_changed_snapshot_cannot_be_promoted(self):
        for field, value in (("source_chain_complete", True), ("target_execution", True),
                ("actual_dll_comparison", "verified"), ("runtime_acceptance", "verified"),
                ("changed_during_query", [{"path": "changed"}])):
            report = copy.deepcopy(self.report)
            report[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "static boundary was promoted"):
                self.verify(report)
        report = copy.deepcopy(self.report)
        report["native"]["failures"].clear()
        with self.assertRaisesRegex(ValueError, "capture failure was hidden"):
            self.verify(report)

    def test_strict_json_and_decoded_example_limits_remain_separate_from_native_results(self):
        from host import _lighting_media_codec_rgba8
        report = json.loads(json.dumps(self.report, ensure_ascii=False, sort_keys=True, allow_nan=False))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), report["verification"])
        self.assertFalse(report["fixed_vectors"]["actual_dll_results"])
        self.assertFalse(report["verification"]["compressed_pixel_arithmetic_complete"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])
        with self.assertRaisesRegex(ValueError, "decoded-buffer length differs"):
            _lighting_media_codec_rgba8(b"\x00", 1, 1, 4)
        with self.assertRaisesRegex(ValueError, "outside the reviewed"):
            _lighting_media_codec_rgba8(b"", 0, 1, 4)


class LightingAudioEffectChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_audio_effect_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_audio_effect_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_audio_effect_claims
        return _verify_lighting_audio_effect_claims(report)

    def test_actual_panel_and_current_release_join_meter_and_notification_tables(self):
        report = self.report
        self.assertEqual(report["current_native_release"]["version"], "4.0.896")
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(report["studio_scope"]["mounted_controls"], ["color_stops", "boost", "auto_boost", "decay"])
        self.assertEqual(self.verify(report)["actual_mounted_effect_ids"], [12])
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 34)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 16)
        self.assertEqual(len(report["native"]["verified_call_edges"]), 16)
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        self.assertTrue(all(row["instruction_addresses"] for row in report["native"]["verified_call_edges"]))
        self.assertFalse(report["studio_scope"]["flow_control_mounted"])
        self.assertFalse(report["studio_scope"]["pcm_capture_inferred"])

    def test_strict_json_keeps_global_failures_distinct_from_selected_bodies(self):
        report = json.loads(json.dumps(self.report, allow_nan=False, ensure_ascii=False, sort_keys=True))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), report["verification"])
        native = report["native"]
        capture = json.loads(self.audit.artifact(self.audit.a.ROOT / native["capture_receipt"]["path"]))
        self.assertEqual(native["failures"], capture["failures"])
        self.assertEqual(len(native["failures"]), 10)
        self.assertEqual(native["selected_pseudocode_failures"], [])
        self.assertTrue(all(row["pseudocode_captured"] and row["input_chunks_verified"] and not row["failures"]
                            for row in native["complete_selected_functions"]))

    def test_all_complete_ast_owners_and_snapshot_match_actual_originals(self):
        audit = HostLocalAudit()
        for owner in self.report["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.report["changed_during_query"], [])

    def test_missing_or_changed_audio_mount_cannot_reuse_the_control_contract(self):
        from host import _lighting_audio_effect_scope
        catalog = {"audio": {"effect_id": 12}}
        routes = {"audio": {"module_id": 9120, "chunk_ids": [606, 412]}}
        for changed_catalog, changed_routes in (({}, routes),
                ({"audio": {"effect_id": 11}}, routes), (catalog, {}),
                (catalog, {"audio": {"module_id": 19120, "chunk_ids": [606, 412]}}),
                (catalog, {"audio": {"module_id": 9120, "chunk_ids": [412]}})):
            with self.subTest(catalog=changed_catalog, routes=changed_routes), \
                    self.assertRaisesRegex(ValueError, "Audio effect mounted route requires semantic review"):
                _lighting_audio_effect_scope(changed_catalog, changed_routes)

    def test_actual_pe_guid_bytes_identify_system_peak_api_without_pcm_capture(self):
        import uuid
        regions = {row["role"]: row for row in self.report["native"]["verified_data_regions"]}
        self.assertEqual(len(regions), 9)
        for role, expected in (("iid_unknown", "00000000-0000-0000-c000-000000000046"),
                ("iid_notification", "7991eec9-7e89-4d85-8390-6c703cec60c0"),
                ("iid_meter", "c02216f6-8c67-4b5b-9d00-d008e73e0064"),
                ("clsid_enumerator", "bcde0395-e52f-467c-8e3d-c4579291692e"),
                ("iid_enumerator", "a95664d2-9614-4f35-a746-de8db63617e6")):
            with self.subTest(role=role):
                row = regions[role]
                self.assertTrue(row["actual_pe_bytes_verified"])
                self.assertEqual(str(uuid.UUID(bytes_le=bytes.fromhex(row["raw_hex"]))), expected)
        self.assertEqual(struct.unpack("<4I", bytes.fromhex(regions["fallback_sentinel"]["raw_hex"])), (100,) * 4)
        self.assertEqual(struct.unpack("<4I", bytes.fromhex(regions["fallback_stops"]["raw_hex"])), (0, 10, 30, 50))
        self.assertEqual(struct.unpack("<4I", bytes.fromhex(regions["fallback_colors"]["raw_hex"])),
                         (16711680, 65280, 65535, 255))
        self.assertEqual(struct.unpack("<2f", bytes.fromhex(regions["gain_constants"]["raw_hex"])), (0.5, 4.0))
        parameters = self.report["boundaries"]["native_parameters"]
        self.assertEqual(parameters["palette_fallback_colors"], [0, 16711680, 65280, 65535, 255, 255])
        self.assertEqual(parameters["palette_fallback_stops"], [0, 10, 30, 50, 80, 100])

    def test_controls_and_packed_native_parameters_cannot_be_silently_reinterpreted(self):
        for group, name, value in (("controls", "default_boost", 0),
                ("controls", "default_auto_boost", True),
                ("controls", "manual_boost_disabled_during_auto", False),
                ("controls", "normalization_clamps_boost_or_decay", True),
                ("controls", "normalization_preserves_flow", True),
                ("controls", "decay_mapper_multiplier", 1),
                ("native_parameters", "json_reads_mode_or_auto", True),
                ("native_parameters", "flow_bits", 32),
                ("native_parameters", "flow_capture_value", 0),
                ("native_parameters", "decay_zero_ms", 0),
                ("native_parameters", "decay_below_one_frame_guarded", True),
                ("native_parameters", "boost_clamped_to_ui_range", True),
                ("native_parameters", "palette_fallback_replaces_all_ten_stops", True)):
            changed = copy.deepcopy(self.report)
            changed["boundaries"][group][name] = value
            with self.subTest(group=group, name=name), \
                    self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)

    def test_meter_failure_auto_gain_and_notification_release_limits_are_preserved(self):
        for group, name, value in (("input", "pcm_or_fft_read", True),
                ("input", "enumerator_creation_hresult_checked_before_deref", True),
                ("input", "notification_registration_hresult_checked", True),
                ("input", "registration_flag_proves_registered", True),
                ("input", "missing_meter", "fail without rendering"),
                ("input", "zero_level_early_return_hides_meter_hresult", False),
                ("input", "meter_peak_range_or_finiteness_checked", True),
                ("input", "default_change_filters_event_flow", True),
                ("input", "default_change_uses_notified_endpoint_id", True),
                ("input", "notification_reactivation_hresult_propagated", True),
                ("calculation", "auto_gain_minute_counter_resets_history", True),
                ("calculation", "auto_gain_history_reset", "every second"),
                ("calculation", "no_level_and_no_peak", "always write current palette zero"),
                ("calculation", "input_or_reset_clears_envelope", True),
                ("cleanup", "unregister_hresult_checked", True),
                ("cleanup", "explicit_inflight_notification_drain", True),
                ("cleanup", "callback_render_meter_swap_locked", True),
                ("cleanup", "reset_releases_meter", True),
                ("scope", "other_style_ui_invented", True),
                ("scope", "all_product_quick_or_audio_visualizer_complete", True)):
            changed = copy.deepcopy(self.report)
            changed["boundaries"][group][name] = value
            with self.subTest(group=group, name=name), \
                    self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)

    def test_changed_defaults_bindings_slots_guid_or_hresult_instructions_require_review(self):
        for mutate in (
                lambda r: r["bindings"]["parameter_writer_import"].update(body="m=n(19286)"),
                lambda r: r["exports"].pop("boost_default"),
                lambda r: r["mounted_routes"]["audio"].update(module_id=1958),
                lambda r: r["native"]["verified_indirect_slots"][0].update(target_rva="0x69250"),
                lambda r: r["native"]["verified_call_edges"].pop(),
                lambda r: r["native"]["verified_data_regions"][4].update(raw_hex="00" * 16),
                lambda r: r["instruction_evidence"]["meter_and_auto_gain"]["instructions"].pop(),
                lambda r: r["reviewed_semantics"].update(stop_and_release="Unregister drains every callback")):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_changed_control_is_invalid_after_owner_and_selection_hash_recalculation(self):
        from host import _lighting_audio_effect_projection
        changed = copy.deepcopy(self.report)
        owner = next(row for row in changed["complete_source_owners"]
                     if row["id"] == changed["selected"]["boost_decay_controls"])
        original_body = owner["body"]
        owner["body"] = original_body.replace("Number(e.value)", "Number(e.value)+1")
        self.assertNotEqual(owner["body"], original_body)
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_audio_effect_projection(changed),
            ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(HostLocalAudit(), owner)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_static_semantics_cannot_promote_rust_ui_dll_or_device_acceptance(self):
        for name, value in (("source_chain_complete", True), ("target_execution", True),
                ("product_code_modified", True), ("real_device_operations", True),
                ("actual_dll_comparison", "passed"), ("runtime_acceptance", "passed"),
                ("ui_backend_wiring", "complete"), ("rust_independent_implementation", "complete"),
                ("snapshot_stable", False), ("changed_during_query", ["changed-input"])):
            changed = copy.deepcopy(self.report)
            changed[name] = value
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "static review was promoted"):
                self.verify(changed)
        self.assertEqual(self.report["rust_independent_implementation"], "partial_existing_not_revalidated")
        self.assertIn("rejects autoBoost!=0", self.report["existing_rust_observation"]["scope"])

    def test_global_and_selected_native_failures_cannot_be_hidden(self):
        for mutate in (lambda r: r["native"]["failures"].clear(),
                lambda r: r["native"]["selected_pseudocode_failures"].append({"stage": "selected:hexrays"}),
                lambda r: r["native"].update(all_pseudocode_captured=True)):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "native capture failure was hidden"):
                self.verify(changed)


class StudioRegionWindowLifecycleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_studio_region_window_lifecycle
        cls.audit = HostLocalAudit()
        cls.report = collect_studio_region_window_lifecycle(cls.audit)

    def verify(self, report):
        from host import _verify_studio_region_window_claims
        return _verify_studio_region_window_claims(report)

    def native_text(self, role):
        row, = [row for row in self.report["native"]["complete_selected_functions"] if row["role"] == role]
        value = json.loads(self.audit.artifact(self.audit.a.ROOT / row["artifact"]["path"], row["artifact"]))
        return "\n".join(line["text"] for line in value["pseudocode"]["lines"])

    def test_current_resource_and_finite_window_selection_are_bound_to_actual_pe(self):
        report = self.report
        native, provenance = report["native"], report["native_provenance"]
        self.assertEqual(provenance["current_declaration"]["resourceVersion"], "1.0.13.1")
        self.assertEqual(provenance["original"]["sha256"],
            "c9a61daec651215975dc9cd4c7f6a8037b6855981b403e6a9449b2e9bd102778")
        self.assertTrue(provenance["native_window_semantics_reviewed_here"])
        self.assertTrue(provenance["latest_manifest_http_observations"]["passed"])
        self.assertEqual(self.verify(report)["selected_native_functions"], 56)
        self.assertEqual((native["functions_captured"], native["functions_total"]), (2031, 2031))
        self.assertEqual((len(report["selected"]), len(report["complete_source_owners"])), (32, 40))
        self.assertEqual((len(native["verified_indirect_slots"]), len(native["verified_call_edges"]),
            len(native["verified_exports"]), len(report["pointer_assignments"])), (28, 44, 8, 19))
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in native["verified_indirect_slots"]))
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"] and not row["failures"]
            and "capture" not in row for row in native["complete_selected_functions"]))
        self.assertEqual(native["verified_data_regions"][0]["raw_hex"], "090a0b0c0d20")

    def test_actual_main_action_uses_synchronous_dispatch_and_freeffi_keeps_callback(self):
        owners = {row["id"]: row for row in self.report["complete_source_owners"]}
        body = lambda role: owners[self.report["selected"][role]]["body"]
        self.assertIn('e.invoke("ffiPreload",t,i)', body("host_preload_main_action"))
        self.assertIn('Ge.callDLL(e,o,n)', body("host_main_ffi_channel"))
        self.assertIn('case"FreeFFI":return!0', body("host_ffi_callDLL"))
        direct = body("host_ffi__handleAction_Default")
        self.assertIn('e[t.action](...t.payload.actionArgs)', direct)
        self.assertNotIn('.async(', direct)
        self.assertLess(direct.index('o=s.readCString()'), direct.index('r=!0'))
        self.assertIn('finally{r&&', direct)
        self.assertIn('this.ffiCallbackMap.set(o,h)', body("host_ffi__handleAction_SetNodeFFIEvent"))
        self.assertEqual(body("host_ffi__handleAction_FreeFFI"), '_handleAction_FreeFFI=async(e,n,a)=>{}')
        self.assertIn('console.error(`doDLLAction deprecated', body("host_preload_legacy_action"))

    def test_selected_and_cancelled_native_callback_orders_remain_distinct(self):
        selected, cancelled = self.native_text("selected_callback"), self.native_text("cancelled_callback")
        self.assertLess(selected.index('qword_18008BD60(v21)'), selected.index('sub_180037920(::Block)'))
        self.assertLess(cancelled.index('sub_180037920(::Block)'), cancelled.index('qword_18008BD60(v5)'))
        self.assertIn('onRegionSelected', selected)
        self.assertIn('onRegionCancelled', cancelled)
        self.assertNotIn('Thrd_join', selected + cancelled)
        stop = self.native_text("stop_selection_export")
        self.assertIn('SendMessageW', stop)
        self.assertIn('Thrd_join', stop)
        self.assertNotIn('sub_180037920', stop)
        terminate = self.native_text("native_terminate")
        self.assertLess(terminate.index('qword_18008BD60 = 0'), terminate.index('StopSelectRegion()'))
        self.assertNotIn('sub_180037920', terminate)

    def test_complete_source_owners_and_snapshots_match_original_bytes(self):
        audit = HostLocalAudit()
        for owner in self.report["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.report["changed_during_query"], [])

    def test_wrong_native_callback_slot_is_rejected_against_original_pe_bytes(self):
        from families import verify_native_functions
        slot = copy.deepcopy(self.report["native"]["verified_indirect_slots"][0])
        slot["target_rva"] = "0x80d0"
        with self.assertRaisesRegex(ValueError, "virtual slot target differs"):
            verify_native_functions(self.report["native_provenance"]["original"], {}, [slot])

    def test_callback_thread_window_pointer_and_call_relations_cannot_be_rebound(self):
        for mutate in (
                lambda r: r["native"]["verified_indirect_slots"][0].update(target_rva="0x80d0"),
                lambda r: r["native"]["verified_call_edges"].pop(),
                lambda r: r["pointer_assignments"][0]["instructions"].pop(),
                lambda r: r["exports"]["native_wrapper"].update(module_id=7760),
                lambda r: r["bindings"]["host_main_ffi_owner"].update(body="Ge=other"),
                lambda r: r["native_provenance"]["current_declaration"].update(resourceVersion="1.0.0"),
                lambda r: r["reviewed_semantics"].update(stop_and_termination="JS terminate joins all workers")):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_source_changes_remain_invalid_after_rehashing_the_selected_summary(self):
        from host import _studio_region_window_projection
        changed = copy.deepcopy(self.report)
        owner = next(row for row in changed["complete_source_owners"]
            if row["id"] == changed["selected"]["host_main_ffi_channel"])
        original = owner["body"]
        owner["body"] = original.replace('Ge.callDLL(e,o,n)', 'Ge.callDLLAsync(e,o,n)')
        self.assertNotEqual(owner["body"], original)
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_studio_region_window_projection(changed),
            ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(HostLocalAudit(), owner)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_original_failure_cancel_and_release_defects_cannot_be_silently_repaired(self):
        for group, key, value in (("frontend", "electron20_subscribes_cancelled", True),
                ("frontend", "main_action_uses_native_async_call", True),
                ("frontend", "host_freeffi_unloads_or_unregisters", True),
                ("frontend", "getscreen_string_freed_after_decode_failure", True),
                ("preview", "replace_closes_previous_thread", True),
                ("preview", "export_waits_for_window_creation", True),
                ("selection", "replace_stops_previous_selection", True),
                ("selection", "selected_callback_thread", "UI thread"),
                ("selection", "selected_closes_cursor_before_event", True),
                ("selection", "cancelled_closes_cursor_before_event", False),
                ("selection", "stop_closes_cursor", True),
                ("selection", "stop_resets_raw_selection_alias", True),
                ("selection", "unhook_result_checked", True),
                ("release", "joinable_replacement_calls_terminate", False),
                ("release", "js_terminate_calls_native_terminate", True),
                ("release", "native_terminate_closes_cursor", True),
                ("scope", "all_screen_picker_products_reviewed", True)):
            changed = copy.deepcopy(self.report)
            changed["boundaries"][group][key] = value
            with self.subTest(group=group, key=key), self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)

    def test_capture_failures_and_unavailable_bodies_cannot_be_hidden(self):
        for mutate in (lambda r: r["native"]["failures"].append({"stage": "hexrays"}),
                lambda r: r["native"]["selected_pseudocode_failures"].append({"stage": "selected:hexrays"}),
                lambda r: r["native"].update(all_pseudocode_captured=False)):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "native failure was hidden"):
                self.verify(changed)

    def test_static_window_review_cannot_promote_rust_ui_dll_or_runtime_acceptance(self):
        for name, value in (("source_chain_complete", True), ("target_execution", True),
                ("product_code_modified", True), ("real_device_operations", True),
                ("actual_dll_comparison", "passed"), ("runtime_acceptance", "passed"),
                ("ui_backend_wiring", "complete"), ("rust_independent_implementation", "complete"),
                ("snapshot_stable", False), ("changed_during_query", ["changed-input"])):
            changed = copy.deepcopy(self.report)
            changed[name] = value
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "static review was promoted"):
                self.verify(changed)

    def test_reference_only_report_is_strict_json_and_retains_selected_scope(self):
        report = json.loads(json.dumps(self.report, ensure_ascii=False, allow_nan=False, sort_keys=True))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), self.report["verification"])
        self.assertEqual(report["native"]["failures"], [])
        self.assertFalse(report["source_chain_complete"])
        self.assertTrue(report["remaining_chains"])
        self.assertEqual(report["maintenance_lifecycle"]["temporary_artifacts_created"], [])
        self.assertEqual(report["maintenance_lifecycle"]["analysis_sessions_created"], [])


class LightingAmbientScreenChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_ambient_screen_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_ambient_screen_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_ambient_screen_claims
        return _verify_lighting_ambient_screen_claims(report)

    def test_current_mounted_panel_joins_current_engine_and_actual_capture_vtables(self):
        report = self.report
        self.assertEqual(report["current_native_release"]["version"], "4.0.896")
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        self.assertEqual(report["studio_scope"]["effect_id"], 11)
        self.assertEqual(report["studio_scope"]["mounted_controls"], ["screen_region", "blur"])
        self.assertEqual(report["studio_scope"]["studio_default_blur"], 5)
        self.assertFalse(report["studio_scope"]["quick_default_blur_inherited"])
        self.assertEqual(self.verify(report)["selected_native_functions"], 43)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 22)
        self.assertEqual(len(report["native"]["verified_call_edges"]), 20)
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"] and not row["failures"]
                            for row in report["native"]["complete_selected_functions"]))

    def test_strict_json_preserves_real_global_failures_and_pending_window_semantics(self):
        report = json.loads(json.dumps(self.report, ensure_ascii=False, allow_nan=False, sort_keys=True))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), self.report["verification"])
        native = report["native"]
        capture = json.loads(self.audit.artifact(self.audit.a.ROOT / native["capture_receipt"]["path"]))
        self.assertEqual(native["failures"], capture["failures"])
        self.assertEqual(len(native["failures"]), 10)
        self.assertEqual(native["selected_pseudocode_failures"], [])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["region_native_provenance"]["native_window_semantics_reviewed_here"])
        self.assertTrue(report["remaining_chains"])

    def test_all_complete_ast_owners_and_source_snapshots_match_actual_bytes(self):
        audit = HostLocalAudit()
        for owner in self.report["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(self.report["changed_during_query"], [])

    def test_missing_or_changed_ambient_mount_cannot_reuse_controls(self):
        from host import _lighting_ambient_screen_scope
        catalog = {"ambient": {"effect_id": 11}}
        routes = {"ambient": {"module_id": 1958, "chunk_ids": [958]}}
        for changed_catalog, changed_routes in (({}, routes),
                ({"ambient": {"effect_id": 12}}, routes), (catalog, {}),
                (catalog, {"ambient": {"module_id": 2958, "chunk_ids": [958]}}),
                (catalog, {"ambient": {"module_id": 1958, "chunk_ids": [958, 12]}})):
            with self.subTest(catalog=changed_catalog, routes=changed_routes), \
                    self.assertRaisesRegex(ValueError, "Ambient mounted route requires semantic review"):
                _lighting_ambient_screen_scope(changed_catalog, changed_routes)

    def test_current_region_native_declaration_comes_from_current_chroma_manifest(self):
        report = self.report["region_native_provenance"]
        declaration, original = report["current_declaration"], report["original"]
        self.assertTrue(report["resource_identity_verified"])
        self.assertEqual(declaration["resourceVersion"], "1.0.13.1")
        self.assertEqual(declaration["sha256"].lower(), original["sha256"])
        self.assertEqual(declaration["size"], 1036424)
        self.assertEqual(report["current_manifest"]["sha256"],
                         "4480ff600b3d7519c5a37bdb565c61cc59d7a620637b0f684fd325b4bbaf9ccb")
        self.assertTrue(all(row["resource_declaration_unchanged"] for row in report["previous_manifests"]))
        observation = report["latest_manifest_http_observations"]
        self.assertTrue(observation["passed"])
        self.assertEqual(observation["comparisons"][0]["status"], "matches_latest_http_bytes")
        self.assertEqual(len(observation["comparisons"][0]["latest_observations"][0]["observation"]["rounds"]), 2)

    def test_changed_or_ambiguous_region_resource_cannot_reuse_current_native(self):
        from host import _lighting_ambient_region_declaration
        original = {"sha256": "a" * 64, "bytes": 123, "url": "https://app-assets.razer.com/region.dll",
            "status": "verified", "declared_sha256": "a" * 64, "declared_bytes": 123,
            "manifests": [{"resource_name": "ChromaStudioNativeDLL", "resource_version": "1.2.3"}]}
        declaration = {"resourceName": "ChromaStudioNativeDLL", "resourceVersion": "1.2.3",
            "sha256": "A" * 64, "size": 123, "url": "https://app-assets.razer.com/region.dll"}
        manifest = {"resourceManifest": {"resources": [declaration]}}
        self.assertEqual(_lighting_ambient_region_declaration(manifest, original), declaration)
        for key, value in (("sha256", "b" * 64), ("size", 124), ("size", True),
                ("url", "https://app-assets.razer.com/other.dll"), ("resourceVersion", "1.2.4")):
            changed = copy.deepcopy(manifest)
            changed["resourceManifest"]["resources"][0][key] = value
            with self.subTest(key=key, value=value), self.assertRaisesRegex(ValueError, "region resource"):
                _lighting_ambient_region_declaration(changed, original)
        for resources in ([], [declaration, declaration]):
            with self.subTest(resources=resources), self.assertRaisesRegex(ValueError, "absent or ambiguous"):
                _lighting_ambient_region_declaration({"resourceManifest": {"resources": resources}}, original)
        with self.assertRaisesRegex(ValueError, "manifest is absent"):
            _lighting_ambient_region_declaration({"resources": [declaration]}, original)

    def test_cache_cancel_release_and_legacy_defects_cannot_be_promoted(self):
        for group, key, value in (("controls", "default_blur", 1),
                ("controls", "custom_preset_restored_from_saved_rect", True),
                ("controls", "native_start_result_checked", True),
                ("controls", "ordinary_unmount_removes_native_listener", True),
                ("controls", "ordinary_unmount_stops_selection", True),
                ("controls", "electron20_subscribes_region_cancelled", True),
                ("controls", "terminate_stops_selection", True),
                ("controls", "legacy_terminate_releases_ffi", True),
                ("engine", "frame_id_committed_before_sample", False),
                ("engine", "sample_failure_retries_same_frame", True),
                ("engine", "initial_black_cache_is_screen_observation", True),
                ("engine", "reset_clears_frame_or_cache", True),
                ("capture", "dx9_failure_falls_back_to_gdi", True),
                ("capture", "duplicate_access_lost_rebuild_attempt_limit", 3),
                ("capture", "duplicate_resource_released_twice_on_success_path", False),
                ("capture", "dx9_reinit_result_propagated", True),
                ("capture", "gdi_checks_sampling_api_results", True),
                ("capture", "gdi_restores_selected_bitmap", True),
                ("capture", "gdi_checks_output_capacity", True),
                ("capture", "loaded_d3d_module_freed_by_capture_destructor", True),
                ("scope", "region_selection_window_internals_reviewed_here", True),
                ("scope", "all_products_or_all_monitors_complete", True)):
            changed = copy.deepcopy(self.report)
            changed["boundaries"][group][key] = value
            with self.subTest(group=group, key=key), \
                    self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)

    def test_changed_bindings_vtables_float_arguments_and_current_provenance_require_review(self):
        for mutate in (
                lambda r: r["bindings"]["parameter_writer_import"].update(body="x=s(19286)"),
                lambda r: r["exports"].pop("blur_default"),
                lambda r: r["mounted_routes"]["ambient"].update(module_id=2958),
                lambda r: r["native"]["verified_indirect_slots"][0].update(target_rva="0x69250"),
                lambda r: r["native"]["verified_call_edges"].pop(),
                lambda r: r["instruction_evidence"]["deriche_sigma_and_axis_calls"]["instructions"].pop(),
                lambda r: r["region_native_provenance"]["current_declaration"].update(resourceVersion="1.0.0"),
                lambda r: r["reviewed_semantics"].update(legacy_capture="Every API failure is checked")):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_changed_control_is_invalid_even_after_hash_recalculation(self):
        from host import _lighting_ambient_screen_projection
        changed = copy.deepcopy(self.report)
        owner = next(row for row in changed["complete_source_owners"]
                     if row["id"] == changed["selected"]["blur_control"])
        original_body = owner["body"]
        owner["body"] = original_body.replace("Number(e.value)", "Number(e.value)+1")
        self.assertNotEqual(owner["body"], original_body)
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        changed["reviewed_selection_sha256"] = hashlib.sha256(json.dumps(_lighting_ambient_screen_projection(changed),
            ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(HostLocalAudit(), owner)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_static_review_cannot_become_rust_ui_dll_or_device_acceptance(self):
        for name, value in (("source_chain_complete", True), ("target_execution", True),
                ("product_code_modified", True), ("real_device_operations", True),
                ("actual_dll_comparison", "passed"), ("runtime_acceptance", "passed"),
                ("ui_backend_wiring", "complete"), ("rust_independent_implementation", "complete"),
                ("snapshot_stable", False), ("changed_during_query", ["changed-input"])):
            changed = copy.deepcopy(self.report)
            changed[name] = value
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "static review was promoted"):
                self.verify(changed)

    def test_global_and_selected_native_failures_remain_distinct(self):
        for mutate in (lambda r: r["native"]["failures"].clear(),
                lambda r: r["native"]["selected_pseudocode_failures"].append({"stage": "selected:hexrays"}),
                lambda r: r["native"].update(all_pseudocode_captured=True)):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "native capture failure was hidden"):
                self.verify(changed)


class LightingReactiveRadiateChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_reactive_radiate_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_reactive_radiate_chain(cls.audit)

    def verify(self, report):
        from host import _verify_lighting_reactive_radiate_claims
        return _verify_lighting_reactive_radiate_claims(report)

    def test_current_release_joins_actual_panel_and_both_native_effects(self):
        report = self.report
        self.assertEqual(report["current_native_release"]["version"], "4.0.896")
        self.assertEqual(report["current_native_release"]["declaration"]["resourceVersion"], "4.0.55.0")
        result = self.verify(report)
        self.assertEqual(result["actual_native_effect_ids"], [3, 17])
        self.assertEqual(result["actual_mounted_effect_ids"], [3])
        self.assertEqual(result["selected_native_functions"], 45)
        self.assertEqual(len(report["native"]["verified_call_edges"]), 38)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 14)
        self.assertFalse(report["studio_scope"]["radiate_absence_outside_reviewed_studio_inferred"])

    def test_strict_json_roundtrip_preserves_native_failures_and_semantic_selection(self):
        report = json.loads(json.dumps(self.report, ensure_ascii=False, allow_nan=False, sort_keys=True))
        self.assertEqual(report, self.report)
        self.assertEqual(self.verify(report), self.report["verification"])
        capture = json.loads(self.audit.artifact(self.audit.a.ROOT / report["native"]["capture_receipt"]["path"]))
        self.assertEqual(report["native"]["failures"], capture["failures"])
        self.assertEqual(len(report["native"]["failures"]), 10)
        self.assertEqual(report["native"]["selected_pseudocode_failures"], [])

    def test_all_complete_ast_owners_match_original_bytes_and_stable_snapshot(self):
        audit = HostLocalAudit()
        for owner in self.report["complete_source_owners"]:
            verify_profiles_owner(audit, owner)
        self.assertEqual(audit.final_snapshot(), [])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_missing_or_changed_reactive_mount_requires_review(self):
        from host import _lighting_reactive_radiate_studio_scope
        catalog = {"reactive": {"effect_id": 3}}
        routes = {"reactive": {"module_id": 7518, "chunk_ids": [606, 42]}}
        for changed_catalog, changed_routes in (({}, routes),
                ({"reactive": {"effect_id": 17}}, routes),
                (catalog, {}), (catalog, {"reactive": {"module_id": 5305, "chunk_ids": [606, 255]}}),
                (catalog, {"reactive": {"module_id": 7518, "chunk_ids": [42]}})):
            with self.subTest(catalog=changed_catalog, routes=changed_routes), \
                    self.assertRaisesRegex(ValueError, "Reactive mounted route requires semantic review"):
                _lighting_reactive_radiate_studio_scope(changed_catalog, changed_routes)

    def test_new_radiate_route_or_alias_cannot_reuse_bounded_absence(self):
        from host import _lighting_reactive_radiate_studio_scope
        catalog = {"reactive": {"effect_id": 3}}
        routes = {"reactive": {"module_id": 7518, "chunk_ids": [606, 42]}}
        expected = _lighting_reactive_radiate_studio_scope(catalog, routes)
        self.assertTrue(expected["radiate_native_effect_exists"])
        for changed_catalog, changed_routes in (
                ({**catalog, "radiate": {"effect_id": 17}}, routes),
                ({**catalog, "new_name": {"effect_id": 17}}, routes),
                (catalog, {**routes, "radiate": {"module_id": 17518, "chunk_ids": [1042]}})):
            with self.subTest(catalog=changed_catalog, routes=changed_routes), \
                    self.assertRaisesRegex(ValueError, "Studio Radiate consumer requires semantic review"):
                _lighting_reactive_radiate_studio_scope(changed_catalog, changed_routes)

    def test_repeated_input_zero_cycles_analog_and_release_limits_cannot_be_promoted(self):
        for group, name, value in (
                ("reactive", "repeated_nonzero_input_restarts_led", False),
                ("reactive", "release_input_removes_led", True),
                ("reactive", "a5_gate_used", True),
                ("reactive", "toggle_bit_stops_existing_led", True),
                ("reactive", "cycles_zero_becomes_infinite", True),
                ("reactive", "random_color_shared_for_one_multi_led_input", False),
                ("reactive", "reset", "clear active sequences"),
                ("radiate", "validates_null_or_empty_leds", True),
                ("radiate", "filters_uint_max_leds", True),
                ("radiate", "digital_inserts_pressed_coordinate", True),
                ("radiate", "digital_release_when_pressed_member", "lower existing wave level to0"),
                ("radiate", "palette_length", "radial distance lookup"),
                ("radiate", "output_alpha", "preserve source alpha"),
                ("radiate", "finite_cycle_or_speed_animation", True),
                ("radiate", "destructor_releases_remaining_payloads", True),
                ("radiate", "failed_child_initialization_releases_allocated_child", True),
                ("shared", "native_reset_cancels_inflight_input", True),
                ("shared", "input_reference_proves_provider_registration", True),
                ("shared", "returned_effect_handle_proves_device_ack", True)):
            changed = copy.deepcopy(self.report)
            changed["boundaries"][group][name] = value
            with self.subTest(group=group, name=name), \
                    self.assertRaisesRegex(ValueError, "failure/lifecycle boundary differs"):
                self.verify(changed)

    def test_static_semantics_never_promote_rust_ui_runtime_or_hardware(self):
        for name, value in (("source_chain_complete", True), ("target_execution", True),
                ("product_code_modified", True), ("real_device_operations", True),
                ("actual_dll_comparison", "passed"), ("runtime_acceptance", "passed"),
                ("ui_backend_wiring", "complete"), ("rust_independent_implementation", "complete"),
                ("snapshot_stable", False), ("changed_during_query", ["changed-native"])):
            changed = copy.deepcopy(self.report)
            changed[name] = value
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "static review was promoted"):
                self.verify(changed)

    def test_changed_mapper_defaults_imports_slots_and_instruction_evidence_require_review(self):
        for mutate in (
                lambda r: r["bindings"]["parameter_writer"].update(body="m=s(12345)"),
                lambda r: r["exports"].pop("duration_default"),
                lambda r: r["mounted_routes"]["reactive"].update(module_id=5305),
                lambda r: r["native"]["verified_indirect_slots"][0].update(target_rva="0x535d0"),
                lambda r: r["native"]["verified_call_edges"].pop(),
                lambda r: r["instruction_evidence"]["radiate_analog_overflow_gate"]["instructions"].pop(),
                lambda r: r["reviewed_semantics"].update(radiate_release="All pending waves released")):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
                self.verify(changed)

    def test_changed_complete_control_stays_invalid_after_hash_recalculation(self):
        changed = copy.deepcopy(self.report)
        owner = next(row for row in changed["complete_source_owners"]
                     if row["id"] == changed["selected"]["reactive_duration_control"])
        owner["body"] = owner["body"].replace("Number(e.value)", "Number(e.value)+1")
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "content differs"):
            verify_profiles_owner(HostLocalAudit(), owner)
        with self.assertRaisesRegex(ValueError, "selected bytes require complete semantic review"):
            self.verify(changed)

    def test_failed_global_capture_and_selected_bodies_remain_distinct(self):
        for mutate in (lambda r: r["native"]["failures"].clear(),
                lambda r: r["native"]["selected_pseudocode_failures"].append({"stage": "selected:hexrays"}),
                lambda r: r["native"].update(all_pseudocode_captured=True)):
            changed = copy.deepcopy(self.report)
            mutate(changed)
            with self.assertRaisesRegex(ValueError, "native capture failure was hidden"):
                self.verify(changed)


class LightingRippleChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_lighting_ripple_chain
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_ripple_chain(cls.audit)

    def test_current_capture_survives_strict_json_and_preserves_real_failures(self):
        report = self.report
        encoded = json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
        self.assertEqual(json.loads(encoded), report)
        capture = json.loads(self.audit.artifact(self.audit.a.ROOT / report["native"]["capture_receipt"]["path"]))
        self.assertEqual(report["native"]["failures"], capture["failures"])
        self.assertTrue(report["native"]["failures"])
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"] and not row["failures"]
                            for row in report["native"]["complete_selected_functions"]))
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])
        self.assertEqual(report["actual_dll_comparison"], "unverified")

    def test_current_native_is_bound_to_version_owned_release_and_original_bytes(self):
        report = self.report
        release = report["current_native_release"]
        self.assertEqual(release["receipt"]["path"],
                         ".analysis/inputs/host-current/releases/" + release["version"] + "/acquisition.json")
        declaration, original = release["declaration"], report["native"]["input"]
        self.assertEqual(declaration["sha256"].lower(), original["sha256"])
        self.assertEqual(declaration["size"], original["bytes"])
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        self.assertTrue(all(row["instruction_addresses"] for row in report["native"]["verified_call_edges"]))

    def test_every_selected_lexical_owner_retains_its_complete_original_span(self):
        for role, owner_id in self.report["selected"].items():
            with self.subTest(role=role):
                owner = self.audit.owners[owner_id]
                raw = self.audit.read(owner["original"])[owner["byte_start"]:owner["byte_end"]]
                self.assertEqual(raw.decode("utf-8"), owner["body"])
                self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["body_sha256"])

    def fixture_owner(self, body):
        audit = HostLocalAudit()
        raw = body.encode("utf-8")
        source = {"path": ".analysis/inputs/fixture/ripple.js", "bytes": len(raw),
                  "sha256": hashlib.sha256(raw).hexdigest()}
        audit.raw[source["path"]], audit.sources[source["path"]] = raw, source
        return audit, source, audit.tree(source).root_node

    def test_changed_control_cannot_reuse_the_previously_reviewed_semantics(self):
        from host import _lighting_ripple_frontend_owner
        old = "function panel(){return setWidth(300);}"
        audit, source, factory = self.fixture_owner("function panel(){return setWidth(400);}")
        with self.assertRaisesRegex(ValueError, "requires semantic review: width"):
            _lighting_ripple_frontend_owner(audit, source, factory, "width", "panel", "function_declaration",
                                           hashlib.sha256(old.encode()).hexdigest())

    def test_missing_or_ambiguous_control_owner_is_rejected(self):
        from host import _lighting_ripple_frontend_owner
        for body in ("function other(){}", "function panel(){} function panel(){}"):
            with self.subTest(body=body):
                audit, source, factory = self.fixture_owner(body)
                with self.assertRaisesRegex(ValueError, "absent or ambiguous: playback"):
                    _lighting_ripple_frontend_owner(audit, source, factory, "playback", "panel",
                                                   "function_declaration", "0" * 64)

    def native_release_fixture(self):
        audit = unittest.mock.Mock()
        audit.a.ROOT = Path("ripple-release-fixture")
        audit.artifact.return_value = b'{"inputs": []}'
        original = {"path": ".analysis/inputs/current/lighting.dll", "bytes": 2153672,
            "sha256": "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa",
            "url": "https://app-assets.razer.com/lighting.dll", "manifest_identity_verified": True,
            "authorities": [{"manifest": "current"}]}
        original["declaration"] = {"resourceName": "LightingEngineDLL", "resourceVersion": "4.0.55.0",
            "size": original["bytes"], "sha256": original["sha256"].upper(), "url": original["url"]}
        release = {"files": [original], "verified_dependencies": [], "version": "4.0.896",
                   "receipt": {"path": ".analysis/inputs/host-current/releases/4.0.896/acquisition.json"}}
        return audit, original, release

    def test_missing_or_duplicate_current_native_declaration_is_rejected_before_use(self):
        from host import _lighting_ripple_native_source
        for duplicate in (False, True):
            audit, original, release = self.native_release_fixture()
            release["files"] = [original, dict(original)] if duplicate else []
            with self.subTest(duplicate=duplicate), \
                    patch("host.verified_host_release_receipt", return_value=release), \
                    self.assertRaisesRegex(ValueError, "declaration is absent or ambiguous"):
                _lighting_ripple_native_source(audit)
            audit.read.assert_not_called()

    def test_native_identity_or_new_algorithm_cannot_reuse_old_capture(self):
        from host import _lighting_ripple_native_source
        for field, value, expected in (("size", 1, "resource declaration differs"),
                ("url", "https://app-assets.razer.com/changed.dll", "resource declaration differs"),
                ("sha256", "0" * 64, "resource declaration differs"),
                ("resourceVersion", "4.0.56.0", "algorithms require semantic review")):
            audit, original, release = self.native_release_fixture()
            original["declaration"][field] = value
            with self.subTest(field=field), patch("host.verified_host_release_receipt", return_value=release), \
                    self.assertRaisesRegex(ValueError, expected):
                _lighting_ripple_native_source(audit)
            audit.read.assert_not_called()

    def test_unverified_manifest_cannot_become_current_native_evidence(self):
        from host import _lighting_ripple_native_source
        audit, original, release = self.native_release_fixture()
        original["manifest_identity_verified"] = False
        with patch("host.verified_host_release_receipt", return_value=release), \
                self.assertRaisesRegex(ValueError, "resource declaration differs"):
            _lighting_ripple_native_source(audit)
        audit.read.assert_not_called()


class LocalApplicationAndRecorderDispatchTests(unittest.TestCase):
    QUERIES = (
        ("profile-local-applications", "collect_profile_local_applications", "profile_local_applications"),
        ("macro-native-recorder", "collect_macro_native_recorder", "macro_native_recorder"))

    def test_local_queries_reach_their_own_collectors_and_receipt_sections(self):
        import host
        for query, collector, section in self.QUERIES:
            parser = argparse.ArgumentParser()
            add_commands(parser.add_subparsers(dest="command", required=True))
            args = parser.parse_args(["audit-host-local", "--query", query])
            with self.subTest(query=query), \
                    patch.object(host, collector, return_value={"failures": []}) as selected, \
                    patch.object(host, "audit_host_profiles") as profiles, \
                    patch.object(host, "audit_host_macro_recorder") as recorder, \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
                audit_host_local(args)
            selected.assert_called_once()
            profiles.assert_not_called()
            recorder.assert_not_called()
            report = publish.call_args.args[0]
            self.assertEqual(publish.call_args.args[3], section)
            self.assertEqual(report["kind"], "current_" + section)
            self.assertTrue(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertFalse(report["target_execution"])
            self.assertEqual(report["actual_dll_comparison"], "unverified")

    def test_missing_current_input_and_changed_snapshot_remain_unresolved(self):
        import host
        for query, collector, _ in self.QUERIES:
            for changed in (False, True):
                with self.subTest(query=query, changed=changed), \
                        patch.object(host, collector, **({"return_value": {"failures": []}} if changed
                            else {"side_effect": ValueError("missing current local owner")})), \
                        patch.object(HostLocalAudit, "final_snapshot", return_value=["current.dll"] if changed else []), \
                        patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                        self.assertRaisesRegex(ValueError, "query remains unresolved"):
                    audit_host_local(argparse.Namespace(query=query, verify_only=False))
                report = publish.call_args.args[0]
                self.assertFalse(report["bounded_observation_complete"])
                self.assertEqual(report["snapshot_stable"], not changed)
                self.assertIn("current.dll" if changed else "missing current local owner", report["failures"][0])


class CurrentReverseChainDispatchTests(unittest.TestCase):
    QUERIES = (
        ("mapping-physical-input-decode", "collect_mapping_physical_input_decode", "mapping_physical_input_decode"),
        ("mapping-external-input-routing", "collect_mapping_external_input_routing", "mapping_external_input_routing"),
        ("mapping-ble-analog-stick-routing", "collect_mapping_ble_analog_stick_routing", "mapping_ble_analog_stick_routing"),
        ("mapping-analog-key-routing", "collect_mapping_analog_key_routing", "mapping_analog_key_routing"),
        ("mapping-input-shutdown-chain", "collect_mapping_input_shutdown_chain", "mapping_input_shutdown_chain"),
        ("mapping-delayed-task-lifecycle", "collect_mapping_delayed_task_lifecycle", "mapping_delayed_task_lifecycle"),
        ("mapping-engine-shutdown-chain", "collect_mapping_engine_shutdown_chain", "mapping_engine_shutdown_chain"),
        ("mapping-input-producer-eligibility", "collect_mapping_input_producer_eligibility", "mapping_input_producer_eligibility"),
        ("mapping-kernel-input-producer", "collect_mapping_kernel_input_producer", "mapping_kernel_input_producer"),
        ("mapping-hardware-notification-chain", "collect_mapping_hardware_notification_chain", "mapping_hardware_notification_chain"),
        ("keypad-mapping-consumers", "collect_keypad_mapping_consumers", "keypad_mapping_consumers"),
        ("keypad-mapping-current-native", "collect_keypad_mapping_current_native", "keypad_mapping_current_native"),
        ("keypad-temporary-keymap-chain", "collect_keypad_temporary_keymap_chain", "keypad_temporary_keymap_chain"),
        ("keypad-two-tap-chain", "collect_keypad_two_tap_chain", "keypad_two_tap_chain"),
        ("common-driver-install-selection", "collect_common_driver_install_selection", "common_driver_install_selection"),
        ("common-driver-legacy-notification", "collect_common_driver_legacy_notification", "common_driver_legacy_notification"),
        ("common-driver-install-consumers", "collect_common_driver_install_consumers", "common_driver_install_consumers"),
        ("keyboard-ble-power-transport", "collect_keyboard_ble_power_transport", "keyboard_ble_power_transport"),
        ("lighting-ripple-chain", "collect_lighting_ripple_chain", "lighting_ripple_chain"),
        ("lighting-reactive-radiate-chain", "collect_lighting_reactive_radiate_chain", "lighting_reactive_radiate_chain"),
        ("lighting-ambient-screen-chain", "collect_lighting_ambient_screen_chain", "lighting_ambient_screen_chain"),
        ("lighting-audio-effect-chain", "collect_lighting_audio_effect_chain", "lighting_audio_effect_chain"),
        ("lighting-type-media-chain", "collect_lighting_type_media_chain", "lighting_type_media_chain"),
        ("lighting-media-calculations", "collect_lighting_media_calculations", "lighting_media_calculations"),
        ("lighting-media-resampler", "collect_lighting_media_resampler", "lighting_media_resampler"),
        ("lighting-media-codec-boundaries", "collect_lighting_media_codec_boundaries", "lighting_media_codec_boundaries"),
        ("lighting-png-decoder-chain", "collect_lighting_png_decoder_chain", "lighting_png_decoder_chain"),
        ("lighting-pnm-decoder-chain", "collect_lighting_pnm_decoder_chain", "lighting_pnm_decoder_chain"),
        ("lighting-bmp-tga-decoder-chain", "collect_lighting_bmp_tga_decoder_chain", "lighting_bmp_tga_decoder_chain"),
        ("lighting-pic-decoder-chain", "collect_lighting_pic_decoder_chain", "lighting_pic_decoder_chain"),
        ("lighting-psd-decoder-chain", "collect_lighting_psd_decoder_chain", "lighting_psd_decoder_chain"),
        ("lighting-gif-decoder-chain", "collect_lighting_gif_decoder_chain", "lighting_gif_decoder_chain"),
        ("lighting-jpeg-decoder-chain", "collect_lighting_jpeg_decoder_chain", "lighting_jpeg_decoder_chain"),
        ("lighting-hdr-decoder-chain", "collect_lighting_hdr_decoder_chain", "lighting_hdr_decoder_chain"),
        ("lighting-hdr-parameters-chain", "collect_lighting_hdr_parameters_chain", "lighting_hdr_parameters_chain"),
        ("lighting-ai-model-chain", "collect_lighting_ai_model_chain", "lighting_ai_model_chain"),
        ("studio-region-window-lifecycle", "collect_studio_region_window_lifecycle", "studio_region_window_lifecycle"),
        ("profile-alternate-icons", "collect_profile_alternate_icons", "profile_alternate_icons"),
        ("profile-gog-scanner", "collect_profile_gog_scanner", "profile_gog_scanner"),
        ("profile-scanner-version-differences", "collect_profile_scanner_version_differences", "profile_scanner_version_differences"),
        ("profile-store-scanner-versions", "collect_profile_store_scanner_versions", "profile_store_scanner_versions"),
        ("profile-local-scanner-versions", "collect_profile_local_scanner_versions", "profile_local_scanner_versions"),
        ("profile-remaining-scanner-versions", "collect_profile_remaining_scanner_versions", "profile_remaining_scanner_versions"),
        ("profile-mac-sysutils-native", "collect_profile_mac_sysutils_native", "profile_mac_sysutils_native"),
        ("profile-shared-helper-consumers", "collect_profile_shared_helper_consumers", "profile_shared_helper_consumers"),
        ("otfm-device-writeback", "collect_otfm_device_writeback", "otfm_device_writeback"),
        ("otfm-macro-encoding-chain", "collect_otfm_macro_encoding_chain", "otfm_macro_encoding_chain"))

    def test_queries_reuse_the_static_publisher_without_replaying_shared_collectors(self):
        import host
        for query, collector, section in self.QUERIES:
            parser = argparse.ArgumentParser()
            add_commands(parser.add_subparsers(dest="command", required=True))
            args = parser.parse_args(["audit-host-local", "--query", query])
            with self.subTest(query=query), \
                    patch.object(host, collector, return_value={"failures": []}) as selected, \
                    patch.object(host, "collect_mapping_input_provider") as provider, \
                    patch.object(host, "collect_lighting_timed_effects") as timed, \
                    patch.object(host, "collect_lighting_input_lifecycle") as lifecycle, \
                    patch.object(host, "collect_studio_editing_consumers") as studio, \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
                audit_host_local(args)
            selected.assert_called_once()
            for shared in (provider, timed, lifecycle, studio):
                shared.assert_not_called()
            report = publish.call_args.args[0]
            self.assertEqual(publish.call_args.args[3], section)
            self.assertEqual(report["kind"], "current_" + section)
            self.assertTrue(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertFalse(report["target_execution"])
            self.assertEqual(report["actual_dll_comparison"], "unverified")

    def test_missing_owner_and_changed_snapshot_publish_the_actual_failure(self):
        import host
        for query, collector, _ in self.QUERIES:
            for mode in ("exception", "returned_failure", "changed"):
                configured = ({"side_effect": ValueError("missing current chain owner")} if mode == "exception" else {
                    "return_value": {"failures": ["missing current chain owner"] if mode == "returned_failure" else []}})
                with self.subTest(query=query, mode=mode), \
                        patch.object(host, collector, **configured), \
                        patch.object(HostLocalAudit, "final_snapshot", return_value=["current-owner"] if mode == "changed" else []), \
                        patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                        self.assertRaisesRegex(ValueError, "query remains unresolved"):
                    audit_host_local(argparse.Namespace(query=query, verify_only=False))
                report = publish.call_args.args[0]
                self.assertFalse(report["bounded_observation_complete"])
                self.assertEqual(report["snapshot_stable"], mode != "changed")
                self.assertIn("current-owner" if mode == "changed" else "missing current chain owner", report["failures"][0])

    def test_exact_upstream_dependency_change_is_published_as_failure_before_returning(self):
        import host
        for query, collector, pointer in (
                ("otfm-macro-encoding-chain", "collect_otfm_macro_encoding_chain", "/otfm_device_writeback"),
                ("mapping-engine-shutdown-chain", "collect_mapping_engine_shutdown_chain", "/mapping_delayed_task_lifecycle"),
                ("lighting-hdr-parameters-chain", "collect_lighting_hdr_parameters_chain", "/lighting_hdr_decoder_chain"),
                ("mapping-hardware-notification-chain", "collect_mapping_hardware_notification_chain", "/mapping_input_provider")):
            dependencies = [{"pointer": pointer}]
            with self.subTest(query=query), patch.object(host, collector, return_value={
                        "failures": [], "upstream_sections": dependencies}), \
                    patch.object(HostLocalAudit, "final_snapshot", return_value=[]), \
                    patch.object(host, "_otfm_macro_encoding_dependency_changes", return_value=[
                        "local-audit.json#" + pointer]) as recheck, \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "query remains unresolved"):
                audit_host_local(argparse.Namespace(query=query, verify_only=False))
            recheck.assert_called_once_with(dependencies)
            report = publish.call_args.args[0]
            self.assertFalse(report["snapshot_stable"])
            self.assertFalse(report["bounded_observation_complete"])
            self.assertIn(pointer, report["failures"][0])

    def test_keypad_dependency_change_is_published_as_failure_before_returning(self):
        import host
        dependencies = [{"pointer": "/keypad_mapping_current_native"}]
        for query, collector in (("keypad-temporary-keymap-chain", "collect_keypad_temporary_keymap_chain"),
                ("keypad-two-tap-chain", "collect_keypad_two_tap_chain")):
            with self.subTest(query=query), patch.object(host, collector, return_value={
                        "failures": [], "upstream_sections": dependencies}), \
                    patch.object(HostLocalAudit, "final_snapshot", return_value=[]), \
                    patch.object(host, "_keypad_temporary_keymap_dependency_changes", return_value=[
                        "local-audit.json#/keypad_mapping_current_native"]) as recheck, \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "query remains unresolved"):
                audit_host_local(argparse.Namespace(query=query, verify_only=False))
            recheck.assert_called_once_with(dependencies)
            report = publish.call_args.args[0]
            self.assertFalse(report["snapshot_stable"])
            self.assertFalse(report["bounded_observation_complete"])
            self.assertIn("/keypad_mapping_current_native", report["failures"][0])

    def test_codec_dependency_change_after_final_snapshot_is_preserved_as_failure(self):
        import host
        dependencies = [{"receipt": "local-audit.json", "pointer": "/selected", "canonical_sha256": "reviewed"}]
        for query, collector, helper in (
                ("lighting-gif-decoder-chain", "collect_lighting_gif_decoder_chain", "_lighting_gif_decoder_dependencies"),
                ("lighting-jpeg-decoder-chain", "collect_lighting_jpeg_decoder_chain", "_lighting_jpeg_decoder_dependencies"),
                ("lighting-hdr-decoder-chain", "collect_lighting_hdr_decoder_chain", "_lighting_hdr_decoder_dependencies"),
                ("lighting-ai-model-chain", "collect_lighting_ai_model_chain", "_lighting_ai_model_dependencies")):
            for mode in ("changed", "exception"):
                configured = ({"side_effect": ValueError("changed selected upstream")} if mode == "exception"
                    else {"return_value": [{**dependencies[0], "canonical_sha256": "changed"}]})
                with self.subTest(query=query, mode=mode), \
                        patch.object(host, collector, return_value={"failures": [], "dependencies": dependencies}) as selected, \
                        patch.object(HostLocalAudit, "final_snapshot", return_value=[]), \
                        patch.object(host, helper, **configured) as recheck, \
                        patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                        self.assertRaisesRegex(ValueError, "query remains unresolved"):
                    audit_host_local(argparse.Namespace(query=query, verify_only=False))
                selected.assert_called_once()
                recheck.assert_called_once_with()
                report = publish.call_args.args[0]
                self.assertFalse(report["snapshot_stable"])
                self.assertFalse(report["bounded_observation_complete"])
                self.assertIn("upstream dependencies", report["failures"][0])

    def test_keypad_recheck_tracks_exact_array_owner_and_rejects_invalid_or_changed_indices(self):
        import analysis
        import host
        from unittest.mock import Mock
        document = {"shared": {"list/owner": [{"rva": "first"}, {"rva": "selected"}]}, "other": 1}
        receipt = ".analysis/results/host-current/local-audit.json"
        dependency = [{"receipt": receipt, "pointer": "/shared/list~1owner/1",
            "canonical_sha256": host._keypad_temporary_keymap_digest(document["shared"]["list/owner"][1])}]
        fake = Mock()
        fake.read_bytes.side_effect = lambda: json.dumps(document).encode()
        with patch.object(analysis, "resolve", return_value=fake):
            self.assertEqual(host._keypad_temporary_keymap_dependency_changes(dependency), [])
            document["other"] = 2
            document["shared"]["list/owner"][0]["rva"] = "unrelated"
            self.assertEqual(host._keypad_temporary_keymap_dependency_changes(dependency), [])
            for index in ("-1", "01", "2", "-", "missing"):
                changed = [{**dependency[0], "pointer": "/shared/list~1owner/" + index}]
                with self.subTest(index=index):
                    self.assertEqual(host._keypad_temporary_keymap_dependency_changes(changed),
                        [receipt + "#" + changed[0]["pointer"]])
            document["shared"]["list/owner"][1]["rva"] = "changed"
            self.assertEqual(host._keypad_temporary_keymap_dependency_changes(dependency),
                [receipt + "#/shared/list~1owner/1"])
            document["shared"]["list/owner"].pop()
            self.assertEqual(host._keypad_temporary_keymap_dependency_changes(dependency),
                [receipt + "#/shared/list~1owner/1"])


class MappingInputProviderDispatchTests(unittest.TestCase):
    def test_mapping_query_reaches_its_collector_and_own_section(self):
        import host
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "mapping-input-provider"])
        with patch.object(host, "collect_mapping_input_provider", return_value={"failures": []}) as provider, \
                patch.object(host, "collect_lighting_input_lifecycle") as lighting, \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            audit_host_local(args)
        provider.assert_called_once()
        lighting.assert_not_called()
        report, _, _, section = publish.call_args.args
        self.assertEqual(section, "mapping_input_provider")
        self.assertTrue(report["bounded_observation_complete"])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])

    def test_missing_provider_and_changed_snapshot_publish_unresolved_facts(self):
        import host
        for changed in (False, True):
            with self.subTest(changed=changed), \
                    patch.object(host, "collect_mapping_input_provider", **({"return_value": {"failures": []}}
                        if changed else {"side_effect": ValueError("missing mapping provider")})), \
                    patch.object(HostLocalAudit, "final_snapshot", return_value=["provider.dll"] if changed else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "Mapping input provider query remains unresolved"):
                audit_host_local(argparse.Namespace(query="mapping-input-provider", verify_only=False))
            report = publish.call_args.args[0]
            self.assertFalse(report["bounded_observation_complete"])
            self.assertEqual(report["snapshot_stable"], not changed)
            self.assertIn("provider.dll" if changed else "missing mapping provider", report["failures"][0])


class LightingConsumerDispatchTests(unittest.TestCase):
    def test_new_queries_publish_to_distinct_sections(self):
        import host
        for query, collector, section in (
        ("lighting-input-lifecycle", "collect_lighting_input_lifecycle", "lighting_input_lifecycle"),
        ("lighting-product-input-map", "collect_lighting_product_input_map", "lighting_product_input_map"),
                ("studio-editing-consumers", "collect_studio_editing_consumers", "studio_editing_consumers")):
            parser = argparse.ArgumentParser()
            add_commands(parser.add_subparsers(dest="command", required=True))
            args = parser.parse_args(["audit-host-local", "--query", query])
            with self.subTest(query=query), \
                    patch.object(host, collector, return_value={"failures": []}) as selected, \
                    patch.object(host, "collect_lighting_static_layer") as static, \
                    patch.object(host, "collect_lighting_timed_effects") as timed, \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
                audit_host_local(args)
            selected.assert_called_once()
            static.assert_not_called()
            timed.assert_not_called()
            report, _, _, destination = publish.call_args.args
            self.assertEqual(destination, section)
            self.assertTrue(report["bounded_observation_complete"])
            self.assertFalse(report["source_chain_complete"])
            self.assertFalse(report["target_execution"])

    def test_new_query_failure_and_changed_snapshot_remain_unresolved(self):
        import host
        for query, collector in (("lighting-input-lifecycle", "collect_lighting_input_lifecycle"),
                                 ("lighting-product-input-map", "collect_lighting_product_input_map"),
                                 ("studio-editing-consumers", "collect_studio_editing_consumers")):
            for changed in (False, True):
                with self.subTest(query=query, changed=changed), \
                        patch.object(host, collector, **({"return_value": {"failures": []}} if changed
                            else {"side_effect": ValueError("missing consumer input")})), \
                        patch.object(HostLocalAudit, "final_snapshot", return_value=["changed.js"] if changed else []), \
                        patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                        self.assertRaisesRegex(ValueError, "query remains unresolved"):
                    audit_host_local(argparse.Namespace(query=query, verify_only=False))
                report = publish.call_args.args[0]
                self.assertFalse(report["bounded_observation_complete"])
                self.assertEqual(report["snapshot_stable"], not changed)
                self.assertIn("changed.js" if changed else "missing consumer input", report["failures"][0])


class LightingInputLifecycleTests(unittest.TestCase):
    def test_current_input_query_preserves_exact_interfaces_and_unverified_boundaries(self):
        from host import collect_lighting_input_lifecycle
        audit = HostLocalAudit()
        report = collect_lighting_input_lifecycle(audit)
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(report["kind"], "current_advanced_input_lifecycle")
        self.assertIn("5954:handleRzInputEvents", report["selected"])
        self.assertIn("host:createCbInputNotification", report["selected"])
        self.assertEqual(set(report["exports"]), {"lG", "eY", "er", "LightingDevice_InputEvent",
                                                "LightingEngine_Event_InputNotify"})
        functions = {row["role"]: row for row in report["native"]["complete_selected_functions"]}
        self.assertEqual(functions["input_ring_enqueue"]["rva"], "0x48b80")
        self.assertEqual(functions["device_input_reference_and_links"]["rva"], "0x48b10")
        self.assertEqual(sum(row["failures"] for row in functions.values()), 0)
        self.assertTrue(all(row["pseudocode_captured"] and "capture" not in row
                            for row in functions.values()))
        self.assertEqual(struct.unpack("<8i", bytes.fromhex(report["native_constants"][0]["raw_hex"])),
                         (0, 1, 0, 0, 0, 0, 0, 1))
        self.assertEqual(struct.unpack("<8i", bytes.fromhex(report["native_constants"][1]["raw_hex"])),
                         (6, 6, 0, 7, 0, 0, 0, 7))
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])
        self.assertFalse(report["real_device_operations"])
        self.assertFalse(report["product_code_modified"])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["runtime_acceptance"], "unverified")
        self.assertEqual(audit.final_snapshot(), [])

    def test_missing_or_ambiguous_current_input_main_cannot_fall_back(self):
        from host import collect_lighting_input_lifecycle
        audit = unittest.mock.Mock()
        for sources in ([], [{"path": "a/main.one.js"}, {"path": "b/main.two.js"}]):
            with self.subTest(sources=sources):
                audit.current_source_set.return_value = ({"name": "current"}, sources)
                with self.assertRaisesRegex(ValueError, "main source is absent or ambiguous"):
                    collect_lighting_input_lifecycle(audit)
        audit.source_set.assert_not_called()

    def test_changed_event_registration_source_requires_review_before_native_analysis(self):
        from host import collect_lighting_input_lifecycle
        audit = unittest.mock.Mock()
        audit.current_source_set.return_value = ({"name": "current"}, [{"path": "main.current.js"}])
        audit.webpack_factories.return_value = [(None, None, object())]
        audit.callable_member.return_value = (None, "changed")
        audit.owners = {"changed": {"body_sha256": hashlib.sha256(b"different event owners").hexdigest()}}
        with self.assertRaisesRegex(ValueError, "requires semantic review: 5954:initEventHandlers"):
            collect_lighting_input_lifecycle(audit)
        audit.families.verify_native_functions.assert_not_called()

    def test_same_method_bytes_cannot_reuse_a_rebound_input_provider(self):
        from host import collect_lighting_input_lifecycle
        audit = HostLocalAudit()
        original = audit.binding

        def changed(role, *args, **kwargs):
            result = original(role, *args, **kwargs)
            if role == "lighting_input_normal_provider":
                return {**result, "body": "K=n(3720)"}
            return result

        with patch.object(audit, "binding", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "input import differs: normal_provider"):
                collect_lighting_input_lifecycle(audit)
        native.assert_not_called()

    def test_native_declaration_must_match_the_current_resource_manifest(self):
        from host import collect_lighting_input_lifecycle
        audit = HostLocalAudit()
        original = audit.artifact

        def changed(path, *args, **kwargs):
            raw = original(path, *args, **kwargs)
            if Path(path).as_posix().endswith("/host-current/acquisition.json"):
                value = json.loads(raw)
                for row in value["files"]:
                    if row.get("declaration", {}).get("resourceName") == "LightingEngineDLL":
                        row["declaration"]["size"] += 1
                return json.dumps(value).encode("utf-8")
            return raw

        with patch.object(audit, "artifact", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "differs from its resource declaration"):
                collect_lighting_input_lifecycle(audit)
        native.assert_not_called()

    def test_incomplete_selected_native_owner_cannot_publish_reviewed_semantics(self):
        from host import collect_lighting_input_lifecycle
        for captured, failures in ((False, 0), (True, 1)):
            with self.subTest(captured=captured, failures=failures):
                audit = HostLocalAudit()
                native = {"complete_selected_functions": [{"role": "selected_key_mask_and_effect_dispatch",
                    "capture": {}, "pseudocode_captured": captured, "failures": failures}]}
                with patch.object(audit.families, "verify_native_functions", return_value=native):
                    with self.assertRaisesRegex(ValueError, "native function remains incomplete"):
                        collect_lighting_input_lifecycle(audit)


class MappingInputProviderTests(unittest.TestCase):
    def test_current_source_preserves_latest_review_and_separate_previous_branch(self):
        from host import collect_mapping_input_provider
        audit = HostLocalAudit()
        report = collect_mapping_input_provider(audit)
        complete = {**report, "complete_originals": list(audit.sources.values()),
                    "complete_source_owners": list(audit.owners.values()),
                    "verified_artifacts": list(audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_input_provider")
        self.assertEqual(len(audit.owners), 22)
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 62)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 32)
        self.assertEqual(len(report["previous_branch"]["native"]["complete_selected_functions"]), 62)
        self.assertEqual(sum(row["failures"] for row in report["native"]["complete_selected_functions"]), 0)
        self.assertTrue(all("capture" not in row for row in report["native"]["complete_selected_functions"]))
        self.assertEqual(report["ffi"]["registerInputNotification"]["signature"], ["void", ["string", "pointer"]])
        proof = report["verified_contract"]
        self.assertEqual({row["subobject_offset"] for row in proof["input_interfaces"]}, {136})
        self.assertEqual((proof["registered_flag_base_offset"], proof["callback_base_offset"]), (712, 720))
        self.assertEqual(proof["event_type"], 1)
        self.assertTrue(proof["timestamp_r9_preserved"])
        self.assertEqual(proof["engine_vtable_rva"], "0x2e0010")
        self.assertEqual(proof["manager_vtable_rva"], "0x2e5a88")
        self.assertEqual(len(proof["verified_producer_release_edges"]), 11)
        branch = report["release_branch"]
        self.assertTrue(branch["latest_source_bytes_equal_reviewed"])
        self.assertFalse(branch["latest_native_bytes_equal_reviewed"])
        self.assertTrue(branch["latest_native_semantics_reviewed"])
        self.assertTrue(report["latest_native_semantics_reviewed"])
        self.assertEqual(branch["reviewed_native_versions"], ["1.3.16.5"])
        self.assertEqual(branch["latest_native_versions"], ["1.3.17.2"])
        self.assertTrue(all(not row["identical"] for row in branch["sections"]))
        self.assertEqual(report["native"]["input"]["sha256"], branch["latest_native"]["sha256"])
        self.assertNotEqual(report["native"]["input"]["sha256"],
                            report["previous_branch"]["native"]["input"]["sha256"])
        functions = {row["role"]: row["rva"] for row in report["native"]["complete_selected_functions"]}
        self.assertEqual(functions["register_input_driver_gate"], "0xaab50")
        self.assertEqual(functions["unregister_input_driver_gate"], "0xab040")
        self.assertEqual(functions["input_json_notification_queue"], "0x802dc")
        for name in ("source_chain_complete", "target_execution",
                     "real_device_operations", "product_code_modified"):
            self.assertFalse(report[name])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["runtime_acceptance"], "unverified")
        self.assertEqual(audit.final_snapshot(), [])

    def test_changed_host_registration_owner_requires_review_before_native_work(self):
        from host import collect_mapping_input_provider
        audit = HostLocalAudit()
        original = audit.callable_member

        def changed(role, *args, **kwargs):
            node, owner = original(role, *args, **kwargs)
            if role == "mapping_input_host_registerInputNotification":
                audit.owners[owner]["body_sha256"] = "0" * 64
            return node, owner

        with patch.object(audit, "callable_member", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "semantic review: registerInputNotification"):
                collect_mapping_input_provider(audit)
        native.assert_not_called()

    def test_same_callback_owner_cannot_reuse_changed_ffi_binding(self):
        from host import collect_mapping_input_provider
        audit = HostLocalAudit()
        original = audit.binding

        def changed(role, *args, **kwargs):
            value = original(role, *args, **kwargs)
            return {**value, "body": 'e=require("other-ffi")'} if role == "mapping_input_host_binding_e" else value

        with patch.object(audit, "binding", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "host binding differs: e"):
                collect_mapping_input_provider(audit)
        native.assert_not_called()

    def test_changed_native_identity_cannot_reuse_reviewed_addresses(self):
        import host
        audit = HostLocalAudit()
        original, provenance = host.verified_host_native(audit, "CommonDLL/mapping_engine.dll")
        with patch.object(host, "verified_host_native", return_value=(
                {**original, "sha256": "f" * 64}, provenance)), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "native provider requires semantic review"):
                host.collect_mapping_input_provider(audit)
        native.assert_not_called()

    def test_missing_native_pseudocode_or_failure_rejects_selected_semantics(self):
        import host
        for captured, failures in ((False, 0), (True, 1)):
            with self.subTest(captured=captured, failures=failures):
                audit = HostLocalAudit()
                native = {"complete_selected_functions": [{"role": "register_input_driver_gate",
                    "capture": {}, "pseudocode_captured": captured, "failures": failures}]}
                with patch.object(host, "_mapping_input_provider_release", return_value={
                        "latest_source_bytes_equal_reviewed": True}), \
                        patch.object(audit.families, "verify_native_functions", return_value=native):
                    with self.assertRaisesRegex(ValueError, "native function remains incomplete"):
                        host.collect_mapping_input_provider(audit)

    def test_hardware_subobject_type0_and_lost_timestamp_cannot_pass_input_proof(self):
        from host import _mapping_input_provider_proof
        base = Path(__file__).resolve().parents[1] / ".analysis/results/native/c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9/functions"
        captures = [json.loads((base / (name + ".json")).read_bytes()) for name in (
            "1060", "19180", "55de4", "64e8c", "a8730", "52158", "6319c", "a68b4",
            "6bc60", "aab50", "ab040", "802dc", "6c320", "51ac0", "51b12", "38b50",
            "6bc70", "6bed0", "6c130", "aa170", "9e1d0", "656ca", "1b8e40")]
        native = {"image_base": "0x180000000", "complete_selected_functions": [{"capture": row} for row in captures]}
        self.assertTrue(_mapping_input_provider_proof(native, current=True)["timestamp_r9_preserved"])
        for address, replacement, expected in (
                ("0xa87af", "48898680000000", "interface adjustment differs: usb"),
                ("0x38b81", "ba00000000", "event ABI differs: 0x38b81"),
                ("0x51b75", "4531c9", "event ABI differs: 0x51b75")):
            with self.subTest(address=address):
                changed = copy.deepcopy(native)
                instruction = next(line for row in changed["complete_selected_functions"]
                                   for line in row["capture"]["assembly"] if line["rva"] == address)
                instruction["bytes"] = replacement
                with self.assertRaisesRegex(ValueError, expected):
                    _mapping_input_provider_proof(changed, current=True)
        changed = copy.deepcopy(native)
        notification = next(row["capture"] for row in changed["complete_selected_functions"]
                            if row["capture"]["rva"] == "0x802dc")
        for line in notification["assembly"]:
            line["xrefs_from"] = [xref for xref in line.get("xrefs_from", [])
                                  if xref["to"] != "0x1801b8720"]
        with self.assertRaisesRegex(ValueError, "producer or release edge differs: 0x802dc -> 0x1b8720"):
            _mapping_input_provider_proof(changed, current=True)

    def test_previous_success_does_not_hide_an_incomplete_latest_native_owner(self):
        import host
        audit = HostLocalAudit()
        verify = audit.families.verify_native_functions

        def incomplete_latest(original, *args, **kwargs):
            if original["sha256"] == "c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9":
                return {"complete_selected_functions": [{"role": "register_input_driver_gate",
                    "capture": {}, "pseudocode_captured": False, "failures": 1}]}
            return verify(original, *args, **kwargs)

        with patch.object(audit.families, "verify_native_functions", side_effect=incomplete_latest):
            with self.assertRaisesRegex(ValueError, "native function remains incomplete: register_input_driver_gate"):
                host.collect_mapping_input_provider(audit)


class MappingInputShutdownChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_input_shutdown_chain(cls.audit)

    def native_captures(self):
        return {"image_base": self.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report["native"]["complete_selected_functions"]]}

    def test_current_shutdown_receipt_separates_subscription_removal_destruction_and_acceptance(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "complete_source_owners": list(self.audit.owners.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_input_shutdown_chain")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"]
            and not row["failures"] and "capture" not in row
            for row in report["native"]["complete_selected_functions"]))
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        proof = report["verified_contract"]
        self.assertEqual((proof["registered_all_offset"], proof["interval_mode_offset"],
            proof["timer_pending_offset"], proof["queue_offset"]), (400, 401, 402, 416))
        self.assertEqual((proof["single_callback_offset"], proof["interval_callback_offset"]), (392, 440))
        self.assertEqual((proof["register_all_mode_word"], proof["register_interval_mode_word"],
            proof["interval_input_multiplier"], proof["forced_flush_minimum_records"]), (1, 257, 1000, 10001))
        self.assertEqual(proof["released_state_offsets"], {"stick_held": 1896, "bypass_retained": 1656,
            "normal_notification": 1616, "pending_mapping": 1512, "key_samples": 1472})
        for key in ("unregister_all_clears_registration_and_id_map", "queued_event_owns_callback_and_payload",
                "manager_erases_and_destroys_before_success_callback", "device_thread_join_precedes_owned_state_release",
                "host_url_removed_before_native_unregister", "host_event_looks_up_identity_at_delivery"):
            self.assertTrue(proof[key])
        for key in ("unregister_all_clears_queue_timer_or_callbacks", "flush_rechecks_registration",
                "delayed_flush_owns_input_device_reference", "cleanup_resets_sample_callbacks",
                "destructor_base_hook_synthesizes_break", "all_future_delayed_tasks_drained_or_disposed",
                "already_posted_main_runner_ffi_js_callbacks_drained", "host_remove_checks_result_before_map_delete",
                "host_event_checks_connection_generation", "host_callback_setup_result_checked_before_set_flag",
                "host_shutdown_clears_device_map_or_all_completion_callbacks", "tick_unit_proven",
                "all_output_restoration_or_snaptap_cancellation_reviewed"):
            self.assertFalse(proof[key])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_native_or_host_identity_cannot_reuse_shutdown_conclusions(self):
        import host
        provenance = self.report["provenance"]
        for target, message in (("original", "shutdown native requires semantic review"),
                                ("host_source", "shutdown Host source requires semantic review")):
            changed = {**provenance[target], "sha256": "0" * 64}
            original = changed if target == "original" else provenance["original"]
            source = changed if target == "host_source" else provenance["host_source"]
            with self.subTest(target=target), patch.object(host, "_mapping_external_input_source",
                    return_value=(original, source, {})), \
                    patch.object(self.audit.families, "verify_native_functions") as verify:
                with self.assertRaisesRegex(ValueError, message):
                    host.collect_mapping_input_shutdown_chain(HostLocalAudit())
                verify.assert_not_called()

    def test_incomplete_flush_is_not_promoted_to_a_cancellation_proof(self):
        import host
        audit = HostLocalAudit()
        native = {"complete_selected_functions": [{"role": "interval_sample_flush_without_registration_gate",
            "capture": {}, "pseudocode_captured": False, "failures": 0}]}
        with patch.object(audit.families, "verify_native_functions", return_value=native):
            with self.assertRaisesRegex(ValueError,
                    "native function remains incomplete: interval_sample_flush_without_registration_gate"):
                host.collect_mapping_input_shutdown_chain(audit)

    def test_changed_registration_timer_callback_or_state_offsets_are_rejected(self):
        import host
        native = self.native_captures()
        for rva, replacement in (("0x72d56", "c6819101000000"), ("0x72a9c", "66c781900100000100"),
                ("0x810be", "4881beb001000010270000"), ("0x8134b", "4883be8801000000"),
                ("0x57bb8", "488b4030"), ("0x658ad", "488d8ec0050000"),
                ("0x6582e", "488d8e78060000"), ("0x65b36", "488d8e88010000")):
            changed = copy.deepcopy(native)
            instruction = next(line for row in changed["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "owner or release differs: " + rva):
                host._mapping_input_shutdown_proof(changed, self.audit.owners, self.report["selected"])

    def test_unregister_flush_callback_cleanup_and_base_hook_require_complete_bodies(self):
        import host
        native = self.native_captures()
        for rva, extra in (("0x72d30", "clear_sample_queue();"), ("0x81300", "if (!registered) return;"),
                ("0x18db0", "if (old_generation) return;"), ("0x786e0", "reset_sample_callback();"),
                ("0x77c40", "synthesize_key_break();")):
            changed = copy.deepcopy(native)
            function = next(row["capture"] for row in changed["complete_selected_functions"]
                            if row["capture"]["rva"] == rva)
            function["pseudocode"]["lines"].append({"text": extra})
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete boundary differs: " + rva):
                host._mapping_input_shutdown_proof(changed, self.audit.owners, self.report["selected"])

    def test_actual_queue_copy_manager_erase_and_join_edges_are_required(self):
        import host
        native = self.native_captures()
        for owner_rva, target in (("0x9e1e2", "0x1801cbc70"), ("0x81300", "0x1800a3f9c"),
                ("0x57e60", "0x18006269e"), ("0x62708", "0x180060dfa"),
                ("0x60dfa", "0x18005f917"), ("0x656ca", "0x1801b8e40")):
            changed = copy.deepcopy(native)
            owner = next(row["capture"] for row in changed["complete_selected_functions"]
                         if row["capture"]["rva"] == owner_rva)
            for line in owner["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", []) if xref["to"] != target]
            with self.subTest(owner=owner_rva), self.assertRaisesRegex(ValueError,
                    "actual ownership edge differs: " + owner_rva):
                host._mapping_input_shutdown_proof(changed, self.audit.owners, self.report["selected"])

    def test_identity_cleanup_destructor_and_base_shutdown_slots_cannot_be_interchanged(self):
        original = self.report["provenance"]["original"]
        for role, table, offset, target in (("ble_cleanup", 0x2e4f28, 48, 0x53090),
                ("ble_destructor", 0x2e4f28, 64, 0x54000),
                ("base_shutdown_hook", 0x2e6a18, 64, 0x77c40)):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
                self.audit.families.verify_native_functions(original, {target: role},
                    [{"role": role, "vtable_rva": table, "slot_byte_offset": offset, "target_rva": target}])

    def test_host_url_order_current_identity_lookup_and_result_consumers_are_frozen(self):
        import host
        native = self.native_captures()
        for role, before, after in (
                ("unregisterAllAnalogKeysEvent", 'filter(e=>e!==a)', 'filter(e=>e===a)'),
                ("createCbAnalogKeyEvent", '6===+t', '1===+t'),
                ("getUSBDeviceInfoFromDeviceMap", 'e===t&&i===s', 'e===t'),
                ("removeDevice", 'this.usbDeviceMap.delete(u),r=c', 'c.result&&this.usbDeviceMap.delete(u),r=c'),
                ("registerAllAnalogKeysEvent", 'g.bSetAnalogKeyEventCallback=!0', 'g.bSetAnalogKeyEventCallback=!1'),
                ("callDLL", 'return this.isInit=!1,!0', 'return this.isInit=!0,!0')):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode("utf-8")).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete Host owner differs: " + role):
                host._mapping_input_shutdown_proof(native, owners, self.report["selected"])


class MappingDelayedTaskLifecycleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_delayed_task_lifecycle(cls.audit)

    def native_captures(self, label="native"):
        return {"image_base": self.report[label]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report[label]["complete_selected_functions"]]}

    def test_current_receipt_separates_delayed_disposal_from_independent_callback_drain(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "complete_source_owners": list(self.audit.owners.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_delayed_task_lifecycle")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual(report["ffi_provenance"]["version"], "1.0.11")
        self.assertTrue(report["ffi_provenance"]["unpacked_native_bytes_verified"])
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 65)
        self.assertEqual(len(report["addon_native"]["complete_selected_functions"]), 18)
        for label in ("native", "addon_native"):
            self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"]
                and not row["failures"] and "capture" not in row
                for row in report[label]["complete_selected_functions"]))
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        proof = report["verified_contract"]
        self.assertEqual((proof["device_main_runner_offset"], proof["device_thread_offset"],
            proof["controller_default_runner_offset"], proof["runner_post_slot_offset"]), (16, 24, 440, 0))
        self.assertEqual((proof["queue_incoming_offset"], proof["queue_delayed_heap_offset"],
            proof["queued_task_bytes"], proof["bind_disposer_slot_offset"]), (40, 192, 168, 16))
        self.assertEqual((proof["addon_queue_offset"], proof["addon_async_handle_offset"],
            proof["closure_environment_offset"], proof["addon_cross_thread_work_bytes"]), (64, 104, 120, 128))
        for key in ("device_thread_exit_destroys_delegate_before_return",
                "sequence_manager_destruction_unregisters_all_owned_queues",
                "queue_shutdown_disposes_unexecuted_future_delayed_tasks", "rejected_post_releases_owned_callback",
                "cross_thread_delayed_wrapper_owns_nested_task", "addon_cross_thread_post_waits_for_js_signal",
                "addon_work_freed_by_waiting_producer", "callback_buffer_finalizer_releases_js_references_and_closure",
                "host_retains_callback_buffer"):
            self.assertTrue(proof[key], key)
        for key in ("dispose_invokes_delayed_flush_body", "device_join_drains_independent_main_runner",
                "device_join_drains_ffi_or_js_queue", "addon_work_owns_argument_or_closure_copies",
                "addon_has_selected_per_device_generation_or_cancel_gate",
                "environment_teardown_proves_pending_producer_drain", "loader_actual_runtime_selection_verified"):
            self.assertFalse(proof[key], key)
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_mapping_and_host_identity_cannot_reuse_lifecycle_conclusions(self):
        import host
        provenance = self.report["provenance"]
        for target, message in (("original", "delayed-task native requires semantic review"),
                                ("host_source", "delayed-task Host source requires semantic review")):
            changed = {**provenance[target], "sha256": "0" * 64}
            original = changed if target == "original" else provenance["original"]
            source = changed if target == "host_source" else provenance["host_source"]
            with self.subTest(target=target), patch.object(host, "_mapping_external_input_source",
                    return_value=(original, source, {})), \
                    patch.object(self.audit.families, "verify_native_functions") as verify:
                with self.assertRaisesRegex(ValueError, message):
                    host.collect_mapping_delayed_task_lifecycle(HostLocalAudit())
                verify.assert_not_called()

    def test_ffi_package_source_and_unpacked_addon_changes_require_review(self):
        import host
        ffi = self.report["ffi_provenance"]
        members = [*ffi["complete_source_originals"], ffi["asar_alias"]]
        for target, message in (("source", "FFI source requires semantic review"),
                                ("addon", "FFI addon requires semantic review")):
            changed = copy.deepcopy(members)
            row = next(row for row in changed if row["path"].endswith(
                "/lib/callback.js" if target == "source" else "/ffi_bindings.node"))
            row["sha256"] = "0" * 64
            with self.subTest(target=target), patch.object(host, "current_host_asar_members", return_value=changed):
                with self.assertRaisesRegex(ValueError, message):
                    host._mapping_delayed_task_sources(HostLocalAudit(), self.report["provenance"])

    def test_incomplete_native_queue_is_not_promoted_to_disposal_proof(self):
        import host
        audit = HostLocalAudit()
        incomplete = {"complete_selected_functions": [{"role": "queue_shutdown_and_dispose_pending_tasks",
            "capture": {}, "pseudocode_captured": False, "failures": 0}]}
        ffi = self.report["ffi_provenance"]
        with patch.object(audit.families, "verify_native_functions", return_value=incomplete), \
                patch.object(host, "_mapping_delayed_task_sources",
                    return_value=(ffi["original"], {}, {}, ffi)):
            with self.assertRaisesRegex(ValueError,
                    "mapping native function remains incomplete: queue_shutdown_and_dispose_pending_tasks"):
                host.collect_mapping_delayed_task_lifecycle(audit)

    def test_runner_queue_heap_disposer_and_addon_owner_offsets_cannot_be_confused(self):
        import host
        for label, rva, replacement in (("native", "0x2068af", "488b4068"),
                ("native", "0x25b409", "488b83b0010000"),
                ("native", "0x26b81f", "488d5630"),
                ("native", "0x2619c8", "4c8da728000000"),
                ("native", "0x1cbd2e", "488b4108"),
                ("addon_native", "0x9bf9", "49894668"),
                ("addon_native", "0xa254", "909090909090"),
                ("addon_native", "0x9483", "909090909090")):
            native, addon = self.native_captures(), self.native_captures("addon_native")
            changed = native if label == "native" else addon
            instruction = next(line for row in changed["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(label=label, rva=rva), self.assertRaisesRegex(ValueError,
                    "owner or disposal differs: " + rva):
                host._mapping_delayed_task_lifecycle_proof(native, addon, self.audit.owners, self.report["selected"])

    def test_queue_destructor_waiting_producer_and_finalizer_require_full_bodies(self):
        import host
        for label, rva, extra in (("native", "0x1b92f0", "return_before_delegate_release();"),
                ("native", "0x26b880", "keep_future_tasks();"),
                ("native", "0x627f0", "flush_device_now();"),
                ("addon_native", "0xa0f0", "skip_condition_wait();"),
                ("addon_native", "0x93e0", "drop_device_generation();"),
                ("addon_native", "0x87d0", "do_not_release_callback();"),
                ("addon_native", "0x7b20", "signal_all_pending_producers();")):
            native, addon = self.native_captures(), self.native_captures("addon_native")
            changed = native if label == "native" else addon
            function = next(row["capture"] for row in changed["complete_selected_functions"]
                            if row["capture"]["rva"] == rva)
            function["pseudocode"]["lines"].append({"text": extra})
            with self.subTest(label=label, rva=rva), self.assertRaisesRegex(ValueError,
                    "complete boundary differs: " + rva):
                host._mapping_delayed_task_lifecycle_proof(native, addon, self.audit.owners, self.report["selected"])

    def test_actual_nested_task_disposal_and_addon_delivery_edges_are_required(self):
        import host
        for label, owner_rva, target in (("native", "0x1b8ea0", "0x1801b9540"),
                ("native", "0x2624b0", "0x180266350"),
                ("native", "0x260fc0", "0x180267f00"),
                ("native", "0x267f00", "0x18026dd10"),
                ("addon_native", "0x94c0", "0x18000a0f0"),
                ("addon_native", "0x93e0", "0x180008c20"),
                ("addon_native", "0xa440", "0x18000a480")):
            native, addon = self.native_captures(), self.native_captures("addon_native")
            changed = native if label == "native" else addon
            owner = next(row["capture"] for row in changed["complete_selected_functions"]
                         if row["capture"]["rva"] == owner_rva)
            for line in owner["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", []) if xref["to"] != target]
            with self.subTest(label=label, rva=owner_rva), self.assertRaisesRegex(ValueError,
                    "actual ownership edge differs: " + owner_rva):
                host._mapping_delayed_task_lifecycle_proof(native, addon, self.audit.owners, self.report["selected"])

    def test_actual_controller_handle_and_post_slots_are_not_interchangeable(self):
        original = self.report["provenance"]["original"]
        for role, table, offset, target in (("controller_get_runner", 0x341160, 104, 0x25b3e0),
                ("handle_destructor", 0x341fd0, 8, 0x26bba0),
                ("runner_post_delayed_task", 0x341450, 8, 0x260150)):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
                self.audit.families.verify_native_functions(original, {target: role},
                    [{"role": role, "vtable_rva": table, "slot_byte_offset": offset, "target_rva": target}])

    def test_anne_arguments_retained_cif_loader_and_host_callback_owners_are_frozen(self):
        import host
        native, addon = self.native_captures(), self.native_captures("addon_native")
        for role, before, after in (
                ("ffi_anne_available", "return true", "return false"),
                ("ffi_callback", "callback._cif = cif", "callback._cif = null"),
                ("ffi_callback", "func.apply(null, args)", "func.apply(null, [])"),
                ("ffi_loader_selection", "runtimeRequire.addon.bind(runtimeRequire)", "null"),
                ("ffi_loader_resolve", "process.env[name + '_PREBUILD']", "null"),
                ("createCbAnalogKeyEvent", "this.cbAnalogKeyEvent=t?e.Callback", "this.cbAnalogKeyEvent=e.Callback")):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode("utf-8")).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source owner differs: " + role):
                host._mapping_delayed_task_lifecycle_proof(native, addon, owners, self.report["selected"])


class MappingEngineShutdownChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_engine_shutdown_chain(cls.audit)
        cls.addon_raw = cls.audit.read(cls.report["ffi_provenance"]["original"])

    def native_captures(self, label="native"):
        return {"image_base": self.report[label]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report[label]["complete_selected_functions"]]}

    def proof(self, native=None, addon=None, owners=None):
        import host
        return host._mapping_engine_shutdown_chain_proof(
            self.native_captures() if native is None else native,
            self.native_captures("addon_native") if addon is None else addon,
            self.audit.owners if owners is None else owners, self.report["selected"], self.addon_raw)

    def test_reviewed_shutdown_order_is_distinct_from_implementation_or_global_drain(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "complete_source_owners": list(self.audit.owners.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_engine_shutdown_chain")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual(report["ffi_provenance"]["version"], "1.0.11")
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 57)
        self.assertEqual(len(report["addon_native"]["complete_selected_functions"]), 10)
        self.assertEqual(len(report["selected"]), 15)
        self.assertEqual(len(report["upstream_sections"]), 2)
        self.assertTrue(all(not row["recaptured"] for row in report["upstream_sections"]))
        for label in ("native", "addon_native"):
            self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"]
                and not row["failures"] and "capture" not in row
                for row in report[label]["complete_selected_functions"]))
        self.assertTrue(report["engine_main_shutdown_order_reviewed"])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified", "global_event_loop_drain_proven"):
            self.assertFalse(report[key], key)
        for key in ("new_native_captures", "new_analysis_sessions", "temporary_files_created"):
            self.assertEqual(report[key], 0)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_nested_construction_runs_in_reverse_with_device_cleanup_first_and_signal_last(self):
        proof = self.report["verified_contract"]
        self.assertEqual(proof["engine_stage_execution_rvas"], ["0x1a620", "0x1a4c0", "0x1a440",
            "0x1a3c0", "0x1a290", "0x1a150", "0x1a010", "0x19ed0", "0x19d30", "0x19cb0", "0x19c30"])
        self.assertEqual(proof["manager_stage_execution_rvas"], ["0x571e0", "0x57090", "0x56f10", "0x56e70"])
        self.assertEqual((proof["engine_initialize_byte_offset"], proof["engine_state_offset"],
            proof["engine_main_thread_offset"], proof["manager_device_count_offset"],
            proof["manager_device_continuation_offset"], proof["manager_executor_count_offset"]), (80, 64, 56, 376, 344, 208))
        self.assertFalse(proof["nested_disposer_executes_next_stage"])
        self.assertTrue(proof["manager_device_erase_and_destructor_precede_next_stage"])

    def test_signal_join_completion_gate_and_final_owner_release_have_distinct_boundaries(self):
        proof = self.report["verified_contract"]
        for key in ("main_stage_signal_precedes_main_thread_join", "main_join_precedes_external_completion",
                "external_completion_precedes_global_sample_gate_clear",
                "external_completion_precedes_at_exit_owner_release", "host_shutdown_uses_synchronous_ffi_call",
                "addon_js_call_precedes_waiter_signal"):
            self.assertTrue(proof[key], key)
        for key in ("shutdown_post_result_checked", "host_shutdown_has_timeout_or_cancel",
                "native_event_wait_has_deadline_or_js_loop_pump", "addon_async_send_result_checked",
                "addon_environment_close_is_engine_shutdown", "environment_queue_destructor_signals_waiting_producers",
                "global_event_loop_drain_proven", "actual_concurrent_teardown_verified"):
            self.assertFalse(proof[key], key)

    def test_changed_native_and_host_sources_cannot_reuse_shutdown_conclusions(self):
        import host
        provenance = self.report["provenance"]
        for target, message in (("original", "shutdown native requires semantic review"),
                                ("host_source", "shutdown Host source requires semantic review")):
            changed = {**provenance[target], "sha256": "0" * 64}
            original = changed if target == "original" else provenance["original"]
            source = changed if target == "host_source" else provenance["host_source"]
            with self.subTest(target=target), patch.object(host, "_mapping_external_input_source",
                    return_value=(original, source, {})), \
                    patch.object(self.audit.families, "verify_native_functions") as verify:
                with self.assertRaisesRegex(ValueError, message):
                    host.collect_mapping_engine_shutdown_chain(HostLocalAudit())
                verify.assert_not_called()

    def test_exact_upstream_sections_ignore_siblings_and_reject_changed_lifetime_owners(self):
        import host
        receipt = self.audit.a.ROOT / ".analysis/results/host-current/local-audit.json"
        original_read = Path.read_bytes
        document = json.loads(original_read(receipt))
        for section in (None, "mapping_delayed_task_lifecycle", "mapping_input_shutdown_chain"):
            changed = copy.deepcopy(document)
            changed["unrelated_new_query"] = {"failures": ["independent"]}
            if section:
                changed[section]["selected_static_semantics_reviewed"] = False
            replacement = json.dumps(changed, ensure_ascii=False).encode()

            def read(path):
                return replacement if path == receipt else original_read(path)

            with self.subTest(section=section), patch.object(Path, "read_bytes", read):
                if section:
                    with self.assertRaisesRegex(ValueError, "upstream section requires review: " + section):
                        host._mapping_engine_shutdown_dependencies(HostLocalAudit())
                else:
                    self.assertEqual(host._mapping_engine_shutdown_dependencies(HostLocalAudit()),
                        self.report["upstream_sections"])
                    self.assertEqual(host._otfm_macro_encoding_dependency_changes(self.report["upstream_sections"]), [])

    def test_changed_ffi_sync_proxy_original_requires_review(self):
        import host
        ffi = self.report["ffi_provenance"]
        members = [*ffi["complete_source_originals"], ffi["asar_alias"]]
        for name in ("library.js", "foreign_function.js", "_foreign_function.js"):
            changed = copy.deepcopy(members)
            row = next(row for row in changed if row["path"].endswith("/lib/" + name))
            row["sha256"] = "0" * 64
            with self.subTest(name=name), patch.object(host, "current_host_asar_members", return_value=changed):
                with self.assertRaisesRegex(ValueError, "shutdown FFI source requires review: " + name):
                    host._mapping_engine_shutdown_sources(HostLocalAudit(), self.report["provenance"])

    def test_incomplete_stage_or_environment_owner_cannot_become_reviewed(self):
        import host
        ffi = self.report["ffi_provenance"]
        for label in ("mapping", "addon"):
            audit = HostLocalAudit()
            incomplete = {"complete_selected_functions": [{"role": "incomplete_shutdown_owner",
                "pseudocode_captured": False, "failures": 0}]}
            valid = {"complete_selected_functions": []}
            reports = [incomplete, valid] if label == "mapping" else [valid, incomplete]
            with self.subTest(label=label), patch.object(audit.families, "verify_native_functions", side_effect=reports), \
                    patch.object(host, "_mapping_engine_shutdown_sources", return_value=(ffi["original"], {}, ffi)):
                with self.assertRaisesRegex(ValueError, "shutdown " + label + " native function remains incomplete"):
                    host.collect_mapping_engine_shutdown_chain(audit)

    def test_actual_engine_and_thread_virtual_slots_are_not_interchangeable(self):
        original = self.report["provenance"]["original"]
        for role, table, offset, target in (("engine_shutdown", 0x2e0010, 8, 0x2090),
                ("thread_deleting_destructor", 0x2ffbe0, 0, 0x1b9510)):
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
                self.audit.families.verify_native_functions(original, {target: role},
                    [{"role": role, "vtable_rva": table, "slot_byte_offset": offset, "target_rva": target}])

    def test_initialized_state_count_and_callback_order_anchors_are_required(self):
        for label, rva in (("native", "0x20dd"), ("native", "0x246f"),
                ("native", "0x2539"), ("native", "0x19c4d"), ("native", "0x1fea8"),
                ("native", "0x57214"), ("native", "0x57ef3"), ("native", "0x5efec"),
                ("native", "0x1b978"), ("addon_native", "0xa254"), ("addon_native", "0x9483")):
            native, addon = self.native_captures(), self.native_captures("addon_native")
            changed = native if label == "native" else addon
            instruction = next(line for row in changed["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = "90" * (len(instruction["bytes"]) // 2)
            with self.subTest(label=label, rva=rva), self.assertRaisesRegex(ValueError, "owner/order anchor differs: " + rva):
                self.proof(native, addon)

    def test_full_stage_wait_completion_disposer_and_environment_bodies_are_required(self):
        for label, rva in (("native", "0x2090"), ("native", "0x2510"), ("native", "0x1fdd0"),
                ("native", "0x1b9c6"), ("native", "0x571e0"), ("native", "0x57e60"),
                ("native", "0x1b8d60"), ("native", "0x1a9ee0"), ("native", "0x1cffd0"),
                ("addon_native", "0xa0f0"), ("addon_native", "0x93e0"), ("addon_native", "0x7b20")):
            native, addon = self.native_captures(), self.native_captures("addon_native")
            changed = native if label == "native" else addon
            capture = next(row["capture"] for row in changed["complete_selected_functions"] if row["capture"]["rva"] == rva)
            capture["pseudocode"]["lines"].append({"text": "pretend_every_queue_drained();"})
            with self.subTest(label=label, rva=rva), self.assertRaisesRegex(ValueError, "complete boundary set differs"):
                self.proof(native, addon)

    def test_actual_staged_manager_join_and_environment_edges_are_required(self):
        for label, owner_rva, target in (("native", "0x1fed0", "0x18001fdd0"),
                ("native", "0x1a620", "0x180056b40"), ("native", "0x571e0", "0x180057e60"),
                ("native", "0x56f10", "0x18005efb0"), ("native", "0x1b8d60", "0x1801a9ee0"),
                ("native", "0x17ac", "0x1801cffd0"), ("addon_native", "0x93e0", "0x180008c20"),
                ("addon_native", "0x2750", "0x180007b20")):
            native, addon = self.native_captures(), self.native_captures("addon_native")
            changed = native if label == "native" else addon
            capture = next(row["capture"] for row in changed["complete_selected_functions"] if row["capture"]["rva"] == owner_rva)
            for line in capture["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", []) if xref["to"] != target]
            with self.subTest(label=label, rva=owner_rva), self.assertRaisesRegex(ValueError, "actual reference edge differs: " + owner_rva):
                self.proof(native, addon)

    def test_host_callback_retention_and_sync_ffi_selection_require_complete_ast(self):
        for role, before, after in (("callDLL", "this.cbMap.set(o,r)", "this.cbMap.clear()"),
                ("callDLL", "this.cbMap.delete(o)", "this.cbMap.clear()"),
                ("initDll", 'mappingEngineShutdown:["void",["pointer"]]',
                    'mappingEngineShutdown:["void",["pointer"],{async:true}]'),
                ("ffi_library", "lib[func] = async ? ff.async : ff", "lib[func] = ff.async"),
                ("ffi_foreign", "return _ForeignFunction(cif, funcPtr, returnType, argTypes)", "return null"),
                ("ffi_proxy", "bindings.ffi_call(cif, funcPtr, result, argsList)", "pump_event_loop()")):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source owner differs: " + role):
                self.proof(owners=owners)

    def test_actual_pe_delay_names_and_instruction_iat_targets_are_both_required(self):
        import host
        bindings = host._mapping_engine_shutdown_delay_imports(self.addon_raw)
        self.assertEqual(len(bindings), 9)
        self.assertTrue(all(row["actual_pe_bytes_verified"] and row["module"] == "node.exe" for row in bindings))
        self.assertEqual({row["symbol"] for row in bindings}, {"uv_async_send", "uv_cond_wait", "uv_cond_signal",
            "uv_close", "napi_get_uv_event_loop", "uv_async_init", "napi_set_instance_data", "napi_make_callback", "napi_call_function"})
        changed = self.addon_raw.replace(b"uv_cond_wait\0", b"uv_cond_waiX\0")
        self.assertNotEqual(changed, self.addon_raw)
        with self.assertRaisesRegex(ValueError, "actual delay-import binding differs: uv_cond_wait"):
            host._mapping_engine_shutdown_delay_imports(changed)
        changed = bytearray(self.addon_raw)
        section = next(row for row in inspect_pe(changed)["sections"] if row["name"] == ".text")
        offset = section["file_offset"] + 0xa246 - section["rva"]
        changed[offset + 2] ^= 8
        with self.assertRaisesRegex(ValueError, "actual delay-import binding differs: uv_async_send"):
            host._mapping_engine_shutdown_delay_imports(bytes(changed))

    def test_cli_failures_and_dependency_changes_cannot_upgrade_shutdown_to_complete(self):
        import host
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "mapping-engine-shutdown-chain"])
        for condition in ("missing", "changed_snapshot", "changed_dependency"):
            result = {"failures": [], "upstream_sections": self.report["upstream_sections"]}
            with self.subTest(condition=condition), \
                    patch.object(host, "collect_mapping_engine_shutdown_chain", **(
                        {"side_effect": ValueError("missing exact shutdown owner")} if condition == "missing" else {"return_value": result})), \
                    patch.object(HostLocalAudit, "final_snapshot", return_value=["current.dll"] if condition == "changed_snapshot" else []), \
                    patch.object(host, "_otfm_macro_encoding_dependency_changes", return_value=["changed section"] if condition == "changed_dependency" else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "query remains unresolved"):
                audit_host_local(args)
            published = publish.call_args.args[0]
            self.assertFalse(published["source_chain_complete"])
            self.assertFalse(published["target_execution"])
            self.assertEqual(published["actual_dll_comparison"], "unverified")


class MappingInputProducerEligibilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_input_producer_eligibility(cls.audit)

    def native_captures(self):
        return {"image_base": self.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report["native"]["complete_selected_functions"]]}

    def test_actual_usb_caller_and_published_framing_have_distinct_identities(self):
        report = self.report
        self.assertEqual(report["kind"], "current_mapping_input_producer_eligibility")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 15)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 6)
        self.assertTrue(all(row["pseudocode_captured"] and row["input_chunks_verified"]
            and not row["failures"] and "capture" not in row
            for row in report["native"]["complete_selected_functions"]))
        proof = report["verified_contract"]
        self.assertEqual((proof["selected_product_id"], proof["actual_mapping_constructor_type"]), (756, "usb"))
        self.assertEqual(proof["published_bluetooth_le_field_offset"], 141)
        self.assertEqual(proof["usb_report_id_offset"], "published bluetoothLE ? 1 : 0")
        self.assertEqual(proof["usb_class1_consumer_rva"], "0x79bde")
        self.assertTrue(proof["selected_restore_has_generation_profile_version_gates"])
        self.assertTrue(proof["mapping_deduplication_commits_before_storage_result"])
        for key in ("custom_ble_constructor_selected_by_this_caller", "without_filter_driver_requested",
                "storage_completion_is_publication_confirmation", "post_storage_hook_awaited",
                "gamepad_writer_consumes_storage_boolean", "setup_consumes_add_usb_boolean",
                "whole_connection_generation_drain_proven", "live_firmware_transport_qualification_proven"):
            self.assertFalse(proof[key], key)
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key], key)
        self.assertEqual(report["new_native_captures"], 0)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_actual_caller_hook_acknowledgment_and_restore_gates_cannot_be_replaced(self):
        import host
        native = self.native_captures()
        for role, before, after in (
                ("setup_environment", 'yield Ve.A.addUsbDevice(o.device)', 'yield Ve.A.addDevice(o.device,{type:"ble"})'),
                ("post_storage_hook", 'e.bluetoothLE=!!f.A.runtimeData.useBle', 'e.bluetoothLE=!0'),
                ("storage_hook_invocation", '&&this.hooks.PostSetLocalStorage(t)', '&&(yield this.hooks.PostSetLocalStorage(t))'),
                ("gamepad_restore_app_engine", 'A.version!==l.version', 'false'),
                ("gamepad_configuration_owner", 'this.featureKey="gamepad_configuration"', 'this.featureKey="analog_configuration"')):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source owner differs: " + role):
                host._mapping_input_producer_proof(native, owners, self.report["selected"])

    def test_native_candidate_publication_and_published_report_offset_are_required(self):
        import host
        native = self.native_captures()
        for rva, replacement in (("0xe3f7c", "a802"), ("0xe5d12", "80b88c00000000"),
                                  ("0xa8b71", "e800000000")):
            changed = copy.deepcopy(native)
            instruction = next(line for row in changed["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "publication/framing differs: " + rva):
                host._mapping_input_producer_proof(changed, self.audit.owners, self.report["selected"])
        with self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
            self.audit.families.verify_native_functions(self.report["provenance"]["original"],
                {0xa8b20: "usb_raw_consumer"}, [{"role": "usb_raw_consumer", "vtable_rva": 0x2e8de8,
                    "slot_byte_offset": 16, "target_rva": 0xa8b20}])

    def test_failed_native_identity_or_incomplete_owner_cannot_publish_eligibility(self):
        import host
        provenance = self.report["provenance"]
        changed = {**provenance["original"], "sha256": "0" * 64}
        with patch.object(host, "_mapping_external_input_source", return_value=(changed, provenance["host_source"], {})):
            with self.assertRaisesRegex(ValueError, "native requires semantic review"):
                host.collect_mapping_input_producer_eligibility(HostLocalAudit())
        audit = HostLocalAudit()
        with patch.object(audit.families, "verify_native_functions", return_value={
                "complete_selected_functions": [{"role": "usb_add_export", "pseudocode_captured": False,
                    "failures": 0}]}):
            with self.assertRaisesRegex(ValueError, "native function remains incomplete: usb_add_export"):
                host.collect_mapping_input_producer_eligibility(audit)

    def test_shared_owner_requires_its_own_two_round_deployment_validation(self):
        import host
        audit = HostLocalAudit()
        artifact = audit.artifact
        receipt = self.report["frontend"]["deployment_revalidation"]["path"]

        def changed_artifact(path, expected=None):
            raw = artifact(path, expected)
            if audit.a.rel(path) == receipt:
                changed = json.loads(raw)
                row = next(row for row in changed["observations"]
                    if (row.get("retained_original") or {}).get("sha256") ==
                    self.report["frontend"]["shared_source"]["sha256"])
                row["rounds"].pop()
                return json.dumps(changed).encode()
            return raw

        with patch.object(audit, "artifact", side_effect=changed_artifact):
            with self.assertRaisesRegex(ValueError, "shared deployment revalidation differs"):
                host._mapping_input_producer_frontend(audit)


class MappingAnalogKeyRoutingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_analog_key_routing(cls.audit)

    def native_captures(self):
        return {"image_base": self.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report["native"]["complete_selected_functions"]]}

    def test_current_analog_key_receipt_preserves_actual_consumers_and_acceptance_limits(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "complete_source_owners": list(self.audit.owners.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_analog_key_routing")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"]
            and not row["failures"] and "capture" not in row
            for row in report["native"]["complete_selected_functions"]))
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        proof = report["verified_contract"]
        self.assertEqual((proof["input_id_bits"], proof["sample_bits"], proof["sample_unsigned"]), (8, 16, True))
        self.assertEqual(proof["fallback_full_scale_by_version"], {"2": 65535, "other": 255})
        self.assertEqual(proof["fallback_input_type"], 2)
        self.assertFalse(proof["fallback_zero_scancode_allocates_input"])
        self.assertEqual((proof["rising_mutator_slot_bytes"], proof["falling_mutator_slot_bytes"]), (88, 96))
        self.assertEqual((proof["make_actuation_word_offset"], proof["break_actuation_word_offset"]), (12, 14))
        self.assertEqual(proof["notification_bypass_gate_offsets_by_input_type"], {"2": 744, "35": 746})
        self.assertEqual((proof["sample_tree_base_offset"], proof["pending_mapping_tree_base_offset"],
            proof["notification_bypass_tree_base_offset"], proof["normal_analog_notification_tree_base_offset"]),
            (1472, 1512, 1656, 1616))
        self.assertEqual((proof["sample_event_type"], proof["redirect_event_type"], proof["input_notification_event_type"]),
                         (6, 3, 1))
        for key in ("release_clears_all_trees", "repeated_equal_sample_routes_mapping",
                    "sample_event_is_output_confirmation", "all_output_executors_reviewed",
                    "all_mapping_priorities_reviewed", "all_deferred_cancellation_proven",
                    "live_product_firmware_transport_qualification_proven",
                    "deferred_callback_environment_owns_input_values"):
            self.assertFalse(proof[key])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        product = report["product_evidence"]
        self.assertIn("keyboard-mw-756-current", product["selection"]["name"])
        self.assertEqual(product["default_report_ids"], {"11": "analogKeyboardV2ReportID", "4": "razerKeyReportID"})
        self.assertTrue(product["mounted_analog_v3"])
        self.assertFalse(product["live_published_configuration_proven"])
        self.assertFalse(product["hid_descriptor_firmware_transport_qualification_proven"])
        self.assertEqual(product["report_id_binding"]["resolution"], "declaration")
        self.assertIn("frontend-revalidation/3445d9b6", product["deployment_revalidation"]["path"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_native_or_host_identity_cannot_publish_analog_key_semantics(self):
        import host
        provenance = self.report["provenance"]
        for target, message in (("original", "native requires semantic review"),
                                ("host_source", "Host source requires semantic review")):
            changed = {**provenance[target], "sha256": "0" * 64}
            original = changed if target == "original" else provenance["original"]
            source = changed if target == "host_source" else provenance["host_source"]
            with self.subTest(target=target), patch.object(host, "_mapping_external_input_source",
                    return_value=(original, source, {})), \
                    patch.object(self.audit.families, "verify_native_functions") as verify:
                with self.assertRaisesRegex(ValueError, message):
                    host.collect_mapping_analog_key_routing(HostLocalAudit())
                verify.assert_not_called()

    def test_incomplete_key_decoder_is_not_promoted_from_its_rva(self):
        import host
        audit = HostLocalAudit()
        native = {"complete_selected_functions": [{"role": "ordinary_key_sample_mapping", "capture": {},
            "pseudocode_captured": False, "failures": 0}]}
        with patch.object(audit.families, "verify_native_functions", return_value=native):
            with self.assertRaisesRegex(ValueError, "native function remains incomplete: ordinary_key_sample_mapping"):
                host.collect_mapping_analog_key_routing(audit)

    def test_wrong_width_max_presence_direction_type_or_release_tree_is_rejected(self):
        import host
        native = self.native_captures()
        for rva, replacement in (("0x7de0c", "41807e0300"), ("0x7df8e", "440fb67812"),
                ("0x7dfff", "488b4060"), ("0x7e2be", "b8ff000000"),
                ("0x92578", "83f802"), ("0x7b2a8", "4881c678060000"),
                ("0x10c78e", "41b803000000"), ("0x3e4f1", "ba01000000")):
            changed = copy.deepcopy(native)
            instruction = next(line for row in changed["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "gate or release differs: " + rva):
                host._mapping_analog_key_proof(changed, self.audit.owners, self.report["selected"])

    def test_actual_key_consumer_and_release_edges_are_required(self):
        import host
        native = self.native_captures()
        for owner_rva, target in (("0x7a656", "0x1801009a2"), ("0x925d0", "0x1800802dc"),
                                  ("0x9307e", "0x1800794e8"), ("0x93e18", "0x18008e0d4")):
            changed = copy.deepcopy(native)
            owner = next(row["capture"] for row in changed["complete_selected_functions"]
                if row["capture"]["rva"] == owner_rva)
            for line in owner["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", []) if xref["to"] != target]
            with self.subTest(owner=owner_rva), self.assertRaisesRegex(ValueError,
                    "actual consumer or release edge differs: " + owner_rva):
                host._mapping_analog_key_proof(changed, self.audit.owners, self.report["selected"])

    def test_actual_fallback_table_and_make_break_slots_cannot_be_interchanged(self):
        import host
        original = self.report["provenance"]["original"]
        raw = bytearray(self.audit.read(original))
        pe = host.inspect_pe(raw)
        table = 0x2f0c30
        section = next(row for row in pe["sections"] if row["rva"] <= table < row["rva"] + row["bytes"])
        offset = section["file_offset"] + table - section["rva"]
        raw[offset + 4 * 4] ^= 1
        with self.assertRaisesRegex(ValueError, "actual fallback table differs: 0x2f0c30"):
            host._mapping_analog_key_tables(raw)
        with self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
            self.audit.families.verify_native_functions(original,
                {0x10cdc0: "key_make_mutator"}, [{"role": "key_make_mutator",
                    "vtable_rva": 0x2f0b48, "slot_byte_offset": 96, "target_rva": 0x10cdc0}])
        self.assertEqual([len(row["entries"]) for row in self.report["verified_fallback_tables"]], [111, 20])

    def test_three_event_channels_and_actual_product_binding_are_not_interchangeable(self):
        import host
        native = self.native_captures()
        for role, before, after in (
            ("createCbAnalogKeyEvent", '6===+t', '1===+t'),
            ("createCbInputRedirect", '3===+t', '6===+t'),
            ("createCbInputNotification", '1===+t', '6===+t'),
            ("product_report_ids", '11:"analogKeyboardV2ReportID"', '12:"analogKeyboardV2ReportID"'),
            ("product_mounted_analog", 'useFeature("analogV3","yes"', 'useFeature("analogV3","no"')):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode("utf-8")).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete Host or product owner differs: " + role):
                host._mapping_analog_key_proof(native, owners, self.report["selected"])

    def test_pid756_requires_its_own_successful_two_round_deployment_receipt(self):
        import host
        receipt = self.report["product_evidence"]["deployment_revalidation"]["path"]
        for failure in ("failed_receipt", "changed_main", "missing_round"):
            audit = HostLocalAudit()
            artifact = audit.artifact

            def changed_artifact(path, expected=None):
                raw = artifact(path, expected)
                if audit.a.rel(path) == receipt:
                    changed = json.loads(raw)
                    main = next(row for row in changed["observations"]
                        if (row.get("retained_original") or {}).get("sha256") ==
                        self.report["product_evidence"]["source"]["sha256"])
                    if failure == "failed_receipt":
                        changed["passed"] = False
                    elif failure == "changed_main":
                        main["classification"] = "changed"
                    else:
                        main["rounds"].pop()
                    return json.dumps(changed).encode("utf-8")
                return raw

            with self.subTest(failure=failure), patch.object(audit, "artifact", side_effect=changed_artifact):
                with self.assertRaisesRegex(ValueError, "PID756 deployment revalidation differs"):
                    host._mapping_analog_key_product(audit)


class MappingTriggerOutputsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        import host
        cls.audit = host.HostLocalAudit()
        cls.result = host.retain_mapping_trigger_outputs(cls.audit)
        cls.raw = cls.audit.read(cls.result["native_identity"])
        cls.previous = json.loads((analysis.ROOT / ".analysis/results/host-current/local-audit.json").read_bytes())
        cls.reused = [{"capture": json.loads((analysis.ROOT / row["artifact"]["path"]).read_bytes())}
            for row in cls.result["reused_native"]]

    def verify(self, native=None, reused=None):
        import host
        return host._mapping_trigger_outputs_proof(self.raw, self.result["native"] if native is None else native,
            self.reused if reused is None else reused)

    def change_body(self, rva, before, after):
        native = copy.deepcopy(self.result["native"])
        function = next(row["capture"] for row in native["complete_functions"] if row["capture"]["rva"] == rva)
        line = next(line for line in function["pseudocode"]["lines"] if before in line["text"])
        line["text"] = line["text"].replace(before, after, 1)
        return self.verify(native)

    def test_actual_type_and_bounded_software_scope_remain_distinct_from_firmware(self):
        result, proof = self.result, self.result["verified_contract"]
        self.assertEqual(result["kind"], "current_mapping_trigger_outputs")
        self.assertEqual(proof["type"], {"triggers": 38, "doubleMapping": 15, "joystick": 19, "keyboard": 21, "xboxGamepad": 41})
        self.assertEqual((proof["new_complete_functions"], proof["reused_complete_functions"]), (28, 8))
        self.assertEqual(len(proof["verified_consumer_edges"]), 40)
        self.assertFalse(proof["firmware_publication_proven"])
        self.assertFalse(proof["whole_executor_family_complete"])
        self.assertFalse(proof["actual_device_or_os_result_observed"])
        for field in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(result[field])
        self.assertEqual(result["actual_dll_comparison"], "unverified")

    def test_output_ids_and_int32_low16_actuation_do_not_become_invented_validation(self):
        parser = self.result["verified_contract"]["parser"]
        self.assertIn("zero, leading zeros and optional plus accepted", parser["output_id"])
        self.assertEqual(parser["actuation"], "required JSON int32; low16 copied without range validation")
        self.assertIn("reject insertion", parser["duplicate_numeric_id"])
        for rva, before, after in (("0xf4211", "v49 >= 0x10000", "v49 >= 0x100"),
                ("0xdbaf3", "*a3 = WORD2(v12)", "*a3 = v12"),
                ("0x1b5110", "(_BYTE)v6 != 2", "(_BYTE)v6 != 3")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.change_body(rva, before, after)

    def test_threshold_order_and_per_output_admission_cannot_be_changed(self):
        consumer = self.result["verified_contract"]["ordinary_consumer"]
        self.assertIn("make ascending, break descending", self.result["verified_contract"]["parser"]["order"])
        self.assertIn("pair(keyId,outputId)", consumer["per_output_admission"])
        self.assertFalse(consumer["per_output_result_aggregated"])
        self.assertFalse(consumer["notifications_prove_output_success"])
        for rva, before, after in (("0x96f02", "sub_180098156(v21, v20[16], v19, &v18)",
                "sub_180098156(v21, v19, v20[16], &v18)"),
                ("0x9743a", "v15 = a3", "v15 = 0"),
                ("0x98156", "if ( (_BYTE)result )", "if ( 1 )")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.change_body(rva, before, after)

    def test_selected_device_membership_branch_keeps_its_actual_limited_executor(self):
        consumer = self.result["verified_contract"]["ordinary_consumer"]
        self.assertIn("key ID in device+992 selects9719E", consumer["selector"])
        self.assertIn("executes only mode29", consumer["device_992_nested"])
        with self.assertRaisesRegex(ValueError, "complete native boundary changed"):
            self.change_body("0x986bc", "(_DWORD)result == 21", "(_DWORD)result == 27")

    def test_double_mapping_digital_parser_and_actual_rapid_selection_are_retained(self):
        semantics = self.result["verified_contract"]["double_mapping"]
        self.assertIn("ignores the selected analog executor result", semantics)
        self.assertIn("rapid config92748 or ordinary9307E", semantics)
        for rva, before, after in (("0xf50b4", "sub_1800F4211(a1, a2, a3)", "sub_1800F4C78(a1, a2, a3)"),
                ("0x953a0", "sub_18009307E(a1, a2, a3, v16, a5, a6, a7, &v73, 0)",
                    "sub_18009307E(a1, a2, a3, v16, a5, a6, a7, &v73, 1)")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.change_body(rva, before, after)

    def test_partial_tree_move_and_nested_release_do_not_hide_failure_cleanup(self):
        for rva, before, after in (("0x105100", "*(_OWORD *)(a2 + 1) = 0", "*(_OWORD *)(a2 + 1) = 1"),
                ("0xf6904", "sub_1800F6948(a2 + 4)", "nullsub_1()"),
                ("0x1046b0", "*(_OWORD *)(a1 + 24) = 0", "*(_OWORD *)(a1 + 24) = 1")):
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete native boundary changed"):
                self.change_body(rva, before, after)
        self.assertFalse(self.result["verified_contract"]["all_pending_tasks_cancelled_or_drained"])

    def test_physical_bytes_constants_slots_and_consumer_edges_cannot_be_forged(self):
        for mode in ("instruction", "region", "edge"):
            native = copy.deepcopy(self.result["native"])
            if mode == "region":
                row = next(row for row in native["verified_data_regions"] if row["role"] == "triggers_vtable")
                row["raw_hex"] = "00" * row["bytes"]
                row["sha256"] = hashlib.sha256(bytes(row["bytes"])).hexdigest()
            else:
                function = next(row["capture"] for row in native["complete_functions"] if row["capture"]["rva"] == "0xf4211")
                if mode == "instruction":
                    row = next(row for row in function["assembly"] if row["rva"] == "0xf4731")
                    row["bytes"] = "41b813000000"
                else:
                    for row in function["assembly"]:
                        row["xrefs_from"] = [edge for edge in row.get("xrefs_from", []) if edge["to"] != "0x180104cb0"]
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "differs|consumer edge changed"):
                self.verify(native)

    def test_exact_upstream_list_pointer_changes_fail_and_unrelated_sections_can_refresh(self):
        import host
        previous = copy.deepcopy(self.previous)
        previous["independent_refresh"] = {"new": True}
        _, dependencies = host._mapping_trigger_outputs_dependencies(previous)
        self.assertEqual(dependencies, self.result["dependencies"])
        self.assertTrue(any(row["pointer"].endswith("/21") for row in dependencies))
        for mode in ("absent", "changed", "unresolved"):
            previous = copy.deepcopy(self.previous)
            if mode == "absent":
                del previous["keypad_mapping_current_native"]["native"]["complete_selected_functions"][21]
            elif mode == "changed":
                previous["keypad_mapping_current_native"]["native"]["complete_selected_functions"][21]["rva"] = "0xad0cd"
            else:
                previous["mapping_analog_key_routing"]["bounded_observation_complete"] = False
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "upstream pointer requires review"):
                host._mapping_trigger_outputs_dependencies(previous)

    def test_selected_capture_failure_cannot_be_reported_complete(self):
        import host
        original_slice = host.HostLocalAudit.native_slice

        def failed_slice(audit, *args, **kwargs):
            result = original_slice(audit, *args, **kwargs)
            result["decompiler_failures"].append("selected owner unavailable")
            return result

        with patch.object(host.HostLocalAudit, "native_slice", failed_slice), \
                self.assertRaisesRegex(ValueError, "complete capture remains unresolved"):
            host.retain_mapping_trigger_outputs(host.HostLocalAudit())

    def test_current_byte_snapshot_and_no_temporary_or_target_resources(self):
        self.assertEqual(self.result["native_identity"]["sha256"], "c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9")
        self.assertTrue(all(row["input_chunks_verified"] for row in self.result["native"]["complete_functions"]))
        self.assertEqual(self.result["cleanup"]["temporary_files_created"], [])
        self.assertEqual(self.result["cleanup"]["private_databases_created"], [])
        self.assertEqual(json.loads(json.dumps(self.result, ensure_ascii=False)), self.result)
        self.assertEqual(self.audit.final_snapshot(), [])


class MappingBleAnalogStickRoutingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_ble_analog_stick_routing(cls.audit)

    def native_captures(self):
        return {"image_base": self.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report["native"]["complete_selected_functions"]]}

    def test_current_ble_class1_stick_evidence_preserves_actual_consumer_and_acceptance_boundaries(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "complete_source_owners": list(self.audit.owners.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_ble_analog_stick_routing")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertIn("/releases/4.0.896/", report["provenance"]["acquisition"]["path"])
        self.assertEqual(report["native"]["input"]["sha256"],
            "c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9")
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"]
            and not row["failures"] and "capture" not in row
            for row in report["native"]["complete_selected_functions"]))
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        proof = report["verified_contract"]
        self.assertEqual((proof["class_input_raw_offset"], proof["selected_report_class"]), (1, 1))
        self.assertEqual((proof["selected_class_prefix_bytes_removed"], proof["raw_first_input_id_offset"]), (1, 2))
        self.assertFalse(proof["all_ble_classes_strip_prefix"])
        self.assertEqual((proof["triple_bytes"], proof["sample_byte_order"], proof["parser_index_bits"]),
                         (3, "big-endian", 8))
        self.assertEqual(proof["stick_input_ids"], [251, 252])
        self.assertEqual((proof["previous_sample_tree_base_offset"], proof["stick_held_tree_base_offset"]),
                         (1488, 1896))
        self.assertFalse(proof["missing_samples_scan_stick_held_tree"])
        self.assertEqual(proof["threshold_parser_range_inclusive"], [1, 32766])
        self.assertEqual(proof["negative_minimum_sample_magnitude"], 32768)
        self.assertTrue(proof["threshold_equality_activates"])
        self.assertFalse(proof["direct_above_threshold_sign_reversal_emits_edge"])
        self.assertTrue(proof["break_axis_uses_current_sample_sign"])
        self.assertEqual((proof["input_type"], proof["make_flag"], proof["break_flag"]), (3, 0, 1))
        self.assertEqual((proof["delegate_lookup_slot_bytes"], proof["delegate_consumer_slot_bytes"]), (24, 32))
        self.assertFalse(proof["direct_input_notification_call"])
        self.assertFalse(proof["class1_direct_hardwareevent_delivery"])
        self.assertFalse(proof["mounted_product_qualification_proven"])
        self.assertTrue(report["verified_pe_tables"]["actual_pe_bytes_verified"])
        self.assertEqual(report["verified_pe_tables"]["axis_names"], ["left", "right", "up", "down", "invalid"])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_current_native_or_host_identity_is_not_promoted(self):
        import host
        provenance = self.report["provenance"]
        for target, changed, message in (
            ("native", {**provenance["original"], "sha256": "0" * 64}, "native requires semantic review"),
            ("host", {**provenance["host_source"], "sha256": "0" * 64}, "Host source requires semantic review")):
            original = changed if target == "native" else provenance["original"]
            source = changed if target == "host" else provenance["host_source"]
            with self.subTest(target=target), patch.object(host, "_mapping_external_input_source",
                    return_value=(original, source, {})), \
                    patch.object(self.audit.families, "verify_native_functions") as verify:
                with self.assertRaisesRegex(ValueError, message):
                    host.collect_mapping_ble_analog_stick_routing(HostLocalAudit())
                verify.assert_not_called()

    def test_incomplete_ble_decoder_cannot_publish_semantics(self):
        import host
        audit = HostLocalAudit()
        native = {"complete_selected_functions": [{"role": "ble_actual_report_dispatch", "capture": {},
            "pseudocode_captured": False, "failures": 0}]}
        with patch.object(audit.families, "verify_native_functions", return_value=native):
            with self.assertRaisesRegex(ValueError, "native function remains incomplete: ble_actual_report_dispatch"):
                host.collect_mapping_ble_analog_stick_routing(audit)

    def test_wrong_prefix_endian_index_threshold_or_delegate_is_rejected(self):
        import host
        native = self.native_captures()
        for rva, replacement in (("0x53e62", "8a10"), ("0x53ec4", "4883c202"),
                ("0x79cc1", "c1e210"), ("0x79d05", "450fb7e4"),
                ("0x7e93c", "0fbfd9"), ("0x7e94d", "726d"),
                ("0x7ea37", "80f202"), ("0x7ea6e", "488b4020")):
            changed = copy.deepcopy(native)
            instruction = next(line for row in changed["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "edge or consumer differs: " + rva):
                host._mapping_ble_analog_stick_proof(changed, self.audit.owners, self.report["selected"])

    def test_missing_actual_ble_or_delegate_edge_is_not_satisfied_by_function_names(self):
        import host
        native = self.native_captures()
        for owner_rva, target in (("0x52540", "0x18007d260"), ("0x53c70", "0x180079bde"),
                                  ("0x69530", "0x1800794e8")):
            changed = copy.deepcopy(native)
            owner = next(row["capture"] for row in changed["complete_selected_functions"]
                if row["capture"]["rva"] == owner_rva)
            for line in owner["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", []) if xref["to"] != target]
            with self.subTest(owner=owner_rva), self.assertRaisesRegex(ValueError,
                    "actual producer or release edge differs: " + owner_rva):
                host._mapping_ble_analog_stick_proof(changed, self.audit.owners, self.report["selected"])

    def test_wrong_ble_actual_class_table_and_virtual_slot_are_rejected(self):
        import host
        raw = bytearray(self.audit.read(self.report["provenance"]["original"]))
        pe = host.inspect_pe(raw)
        section = next(row for row in pe["sections"] if row["rva"] <= 0x2e55f8 < row["rva"] + row["bytes"])
        offset = section["file_offset"] + 0x2e55f8 - section["rva"]
        raw[offset + 4:offset + 8] = raw[offset:offset + 4]
        with self.assertRaisesRegex(ValueError, "actual report-class switch differs"):
            host._mapping_ble_analog_stick_tables(raw)
        with self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
            self.audit.families.verify_native_functions(self.report["provenance"]["original"],
                {0x66d10: "actual_mapping_lookup"}, [{"role": "ble_mapping_lookup",
                    "vtable_rva": 0x2e4f88, "slot_byte_offset": 32, "target_rva": 0x66d10}])

    def test_host_event_type_and_disconnect_cleanup_are_not_interchangeable(self):
        import host
        native = self.native_captures()
        for role, before, after in (
            ("createCbHardwareEvent", '0===+t', '1===+t'),
            ("createCbInputNotification", '1===+t', '0===+t'),
            ("handleBLEEvent_removeDevice", 'this.usbDeviceMap.delete(i)', 'this.usbDeviceMap.has(i)')):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode("utf-8")).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "Host boundary differs: " + role):
                host._mapping_ble_analog_stick_proof(native, owners, self.report["selected"])


class MappingExternalInputRoutingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_external_input_routing(cls.audit)

    def native_captures(self):
        return {"image_base": self.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report["native"]["complete_selected_functions"]]}

    def test_current_external_raw_route_has_complete_evidence_and_separate_acceptance(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "complete_source_owners": list(self.audit.owners.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_external_input_routing")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertIn("/releases/4.0.896/", report["provenance"]["acquisition"]["path"])
        self.assertEqual(report["native"]["input"]["sha256"],
            "c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9")
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"]
            and not row["failures"] and "capture" not in row
            for row in report["native"]["complete_selected_functions"]))
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        proof = report["verified_contract"]
        self.assertEqual((proof["hardware_observer_subobject_offset"], proof["queued_virtual_slot"]), (128, 24))
        self.assertEqual((proof["read_capacity"], proof["completion_subobject_offset"]), (100, 16))
        self.assertEqual(proof["overlapped_base_offsets"], [72, 104])
        self.assertEqual(proof["buffer_base_offsets"], [136, 237])
        self.assertTrue(proof["uses_actual_completion_length"])
        self.assertFalse(proof["strips_prefix_or_report_id"])
        self.assertFalse(proof["classifier_result_controls_dispatch"])
        self.assertEqual(proof["event_type"], 0)
        self.assertEqual(proof["event_abi"], "void(const char*, int32, const char*, uint64)")
        self.assertEqual(proof["callback_base_offsets"], {"single": 648, "interval": 704})
        self.assertEqual(proof["forced_flush_minimum_records"], 10001)
        self.assertEqual(proof["interval_json_shape"], "array<array<uint8>>")
        self.assertFalse(proof["unregister_clears_callbacks_or_proves_queue_drain"])
        self.assertTrue(proof["json_injection_is_distinct_from_raw_hardware_route"])
        for name in ("source_chain_complete", "target_execution", "real_device_operations", "product_code_modified"):
            self.assertFalse(report[name])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["ui_backend_wiring"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_native_or_host_identity_cannot_promote_reviewed_external_route(self):
        import host
        provenance = self.report["provenance"]
        for target, changed, message in (
            ("native", {**provenance["original"], "sha256": "0" * 64}, "native requires semantic review"),
            ("host", {**provenance["host_source"], "sha256": "0" * 64}, "host source requires semantic review")):
            original = changed if target == "native" else provenance["original"]
            source = changed if target == "host" else provenance["host_source"]
            with self.subTest(target=target), patch.object(host, "_mapping_external_input_source",
                    return_value=(original, source, {})), \
                    patch.object(self.audit.families, "verify_native_functions") as verify:
                with self.assertRaisesRegex(ValueError, message):
                    host.collect_mapping_external_input_routing(HostLocalAudit())
                verify.assert_not_called()

    def test_incomplete_provider_owner_cannot_publish_current_external_semantics(self):
        import host
        audit = HostLocalAudit()
        native = {"complete_selected_functions": [{"role": "actual_hid_completion", "capture": {},
            "pseudocode_captured": False, "failures": 0}]}
        with patch.object(audit.families, "verify_native_functions", return_value=native):
            with self.assertRaisesRegex(ValueError, "native function remains incomplete: actual_hid_completion"):
                host.collect_mapping_external_input_routing(audit)

    def test_wrong_length_prefix_type_or_flush_threshold_is_rejected(self):
        import host
        native = self.native_captures()
        for rva, replacement in (("0x12380f", "41b830010000"), ("0x123b98", "41b964000000"),
                ("0x64ab5", "4883c601"), ("0x2b849", "ba01000000"),
                ("0x7f76a", "4881beb802000010270000")):
            changed = copy.deepcopy(native)
            instruction = next(line for row in changed["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "layout, gate or ABI differs: " + rva):
                host._mapping_external_input_proof(changed, self.audit.owners, self.report["selected"])

    def test_missing_actual_provider_edge_is_not_satisfied_by_retained_function_names(self):
        import host
        native = self.native_captures()
        provider = next(row["capture"] for row in native["complete_selected_functions"]
            if row["capture"]["rva"] == "0x123fe4")
        for line in provider["assembly"]:
            line["xrefs_from"] = [xref for xref in line.get("xrefs_from", [])
                if xref["to"] != "0x180124130"]
        with self.assertRaisesRegex(ValueError, "release edge differs: 0x123fe4 -> 0x124130"):
            host._mapping_external_input_proof(native, self.audit.owners, self.report["selected"])

    def test_wrong_actual_external_virtual_slot_is_rejected_by_original_bytes(self):
        with self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
            self.audit.families.verify_native_functions(self.report["provenance"]["original"],
                {0x64a60: "external_raw_handler"}, [{"role": "external_raw_consumer",
                    "vtable_rva": 0x2e6668, "slot_byte_offset": 16, "target_rva": 0x64a60}])

    def test_changed_type0_host_consumer_is_rejected_after_body_identity_update(self):
        import host
        native = self.native_captures()
        owners = copy.deepcopy(self.audit.owners)
        owner = owners[self.report["selected"]["createCbHardwareEvent"]]
        owner["body"] = owner["body"].replace('0===+t', '1===+t')
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode("utf-8")).hexdigest()
        with self.assertRaisesRegex(ValueError, "host consumer differs: createCbHardwareEvent"):
            host._mapping_external_input_proof(native, owners, self.report["selected"])


class OtfmMacroEncodingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        dependency = host._otfm_macro_encoding_dependencies
        def retained(audit):
            result = dependency(audit)
            cls.upstream = result[0]
            return result
        with patch.object(host, "_otfm_macro_encoding_dependencies", side_effect=retained), \
                patch.object(host, "collect_otfm_device_writeback") as old:
            cls.report = host.collect_otfm_macro_encoding_chain(cls.audit)
        old.assert_not_called()
        cls.owners = cls.audit.owners

    def test_complete_current_sources_and_reused_native_evidence_keep_execution_unverified(self):
        report = self.report
        self.assertEqual(report["product_id"], 756)
        self.assertEqual(report["source_selections"]["product"]["name"], "keyboard-mw-756-current-refresh-ec3db4b29c5a-2")
        self.assertEqual(report["source_selections"]["background"]["name"], "background-manager-current-refresh-ec3db4b29c5a")
        self.assertEqual(len(report["upstream_sections"]), 1)
        self.assertFalse(report["upstream_sections"][0]["recaptured"])
        self.assertEqual(report["upstream_sections"][0]["reused_native_functions"], 8)
        self.assertNotIn(".analysis/results/host-current/local-audit.json", self.audit.artifacts)
        self.assertEqual(report["temporary_files_created"], [])
        self.assertEqual(report["native_evidence_reuse"]["new_native_captures"], 0)
        self.assertFalse(report["native_evidence_reuse"]["new_native_semantics_claimed"])
        for flag in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations", "product_code_modified"):
            self.assertFalse(report[flag], flag)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        for owner in self.owners.values():
            raw = self.audit.read(owner["original"])[owner["byte_start"]:owner["byte_end"]]
            self.assertEqual(raw, owner["body"].encode())
            self.assertEqual(hashlib.sha256(raw).hexdigest(), owner["body_sha256"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_worker_realm_and_actual_tables_are_joined_without_borrowing_protocol_duplicate(self):
        report = self.report
        opcodes = report["cross_chunk_exports"]["opcodes"]
        self.assertEqual(opcodes["module_id"], 65350)
        self.assertTrue(opcodes["original"]["path"].endswith("obmEngineKeyboard.548c12e603ff5072603e.js"))
        keys = self.owners[report["selected"]["worker_keys_factory"]]
        self.assertTrue(keys["original"]["path"].endswith("1451.d334c935ecafd99b2709.js"))
        self.assertIn("init_keyMapping", self.owners[report["selected"]["worker_keys_wrapper"]]["body"])
        self.assertIn("t.init_keyMapping=o", keys["body"])
        self.assertEqual(report["tables"]["opcodes"]["CURSOR"], 21)
        self.assertEqual(report["tables"]["scan_to_hid"]["30"], 4)
        self.assertEqual(report["tables"]["scan_to_hid"]["158"], 4)
        self.assertEqual(report["tables"]["hid_to_scan"]["228"], 57373)

    def test_independent_keyboard_and_extended_modifier_samples_use_source_flags(self):
        import host
        tables = self.report["tables"]
        for event, expected in (({"type": "keyboard", "scancode": 30, "flag": 0}, [1, 4]),
                ({"type": "keyboard", "scancode": 158, "flag": 1}, [2, 4]),
                ({"type": "keyboard", "scancode": 57373, "flag": 0}, [1, 228])):
            with self.subTest(event=event):
                ok, data, mouse = host._otfm_macro_encoding_project_event(event, 3, tables)
                self.assertEqual((ok, data, mouse), (True, expected, 3))
        for event in ({"type": "keyboard", "scancode": 0, "flag": 0},
                {"type": "keyboard", "scancode": 30, "flag": 2}):
            self.assertFalse(host._otfm_macro_encoding_project_event(event, 0, tables)[0])
        original = {"guid": "macro", "name": "modifier", "appEngine": {"mouseMoveType": "none",
            "events": [{"type": "keyboard", "scancode": 29, "flag": 3}]}}
        projected = host._otfm_macro_encoding_project_worker(original, tables)
        self.assertEqual(projected["macroItemList"], [{"type": "keyboard", "scancode": 57373, "flag": 1, "HID": 228}])
        self.assertIs(projected["macroItemList"][0], original["appEngine"]["events"][0])

    def test_relative_worker_filters_zero_motion_and_preserves_source_integer_wrapping(self):
        import host
        source = {"guid": "m", "name": "motion", "appEngine": {"mouseMoveType": "relative", "events": [
            {"type": "mouseMove", "x": 10, "y": 20}, {"type": "delay", "ms": 0},
            {"type": "mouseMove", "x": 7, "y": 22}, {"type": "keyboard", "scancode": 30, "flag": 0}]}}
        result = host._otfm_macro_encoding_project_worker(source, self.report["tables"])
        self.assertEqual(result["numItems"], 2)
        self.assertEqual(result["macroItemList"][0], {"type": "mouseMove", "x": 65533, "y": 2})
        self.assertEqual(host._otfm_macro_encoding_project_batch(result["macroItemList"], self.report["tables"])["fwMacroData"],
                         [21, 255, 253, 0, 2, 1, 4])
        self.assertEqual(source["appEngine"]["events"][-1], {"type": "keyboard", "scancode": 30, "flag": 0})
        source["appEngine"]["mouseMoveType"] = "absolute"
        self.assertIsNone(host._otfm_macro_encoding_project_worker(source, self.report["tables"]))
        self.assertEqual(self.report["verified_contract"]["worker"]["other_mode_result"], "SUCCESS payload undefined")

    def test_mouse_accumulation_literal_middle_bit_wheel_and_horizontal_behavior(self):
        import host
        cases = ((1, 0, [8, 1], 1), (4, 1, [8, 3], 3), (2, 3, [8, 2], 2),
            (16, 3, [8, 3], 3), (32, 4, [8, 4], 4), (2048, 3, [8, 3], 3))
        for button, before, expected, after in cases:
            with self.subTest(button=button):
                self.assertEqual(host._otfm_macro_encoding_project_event({"type": "mouse", "id": button}, before,
                    self.report["tables"]), (True, expected, after))
        for value, expected in ((120, 1), (-120, 255), (65416, 255), (0, 255)):
            self.assertEqual(host._otfm_macro_encoding_project_event({"type": "mouse", "id": 1024, "data": value},
                3, self.report["tables"]), (True, [10, expected], 3))

    def test_independent_delay_boundaries_and_low16_motion_bytes(self):
        import host
        for value, expected in ((255, [17, 255]), (256, [18, 1, 0]), (65535, [18, 255, 255]),
                (65536, [19, 1, 0, 0]), (16777215, [19, 255, 255, 255]),
                (16777216, [20, 1, 0, 0, 0]), (-1, [17, 255]), (4294967297, [20, 0, 0, 0, 1])):
            with self.subTest(delay=value):
                self.assertEqual(host._otfm_macro_encoding_project_event({"type": "delay", "ms": value}, 0,
                    self.report["tables"]), (True, expected, 0))
        self.assertEqual(host._otfm_macro_encoding_project_event({"type": "mouseMove", "x": -1, "y": 65538}, 0,
            self.report["tables"]), (True, [21, 255, 255, 0, 2], 0))

    def test_failed_item_returns_only_prior_bytes_while_original_count_reaches_name_and_cache(self):
        import host
        events = [{"type": "keyboard", "scancode": 30, "flag": 0},
                  {"type": "keyboard", "scancode": 0, "flag": 0}, {"type": "delay", "ms": 256}]
        self.assertEqual(host._otfm_macro_encoding_project_batch(events, self.report["tables"]),
            {"fwMacroData": [1, 4], "fwMacroDataSize": 2, "original_num_items": 3})
        contract = self.report["verified_contract"]
        self.assertTrue(contract["firmware"]["batch"]["original_num_items_preserved_in_name_and_cache"])
        self.assertFalse(contract["writer"]["local_name_or_data_write_error_propagation"])
        self.assertEqual(contract["writer"]["command"], [80, 6, 9])
        self.assertEqual(contract["writer"]["chunk_data_limit"], 72)
        frame = self.owners[self.report["selected"]["frame"]]["body"]
        self.assertIn("t<n.length&&t<o[5]", frame)
        self.assertFalse(contract["writer"]["ack_is_independent_full_macro_readback"])
        self.assertFalse(contract["static_projection_is_vendor_execution"])

    def test_changed_body_identity_table_or_missing_owner_cannot_reuse_selection(self):
        import host
        for role, before, after in (("macro_fw_items", "16&e.id&&(o&=-5)", "32&e.id&&(o&=-5)"),
                ("macro_fw_bytes", "break}", "throw new Error();}"),
                ("worker_keys_wrapper", "o.init_keyMapping.call(void 0)", "o.init_keyMapping"),
                ("worker_operation", '"SUCCESS"===c', '"ERROR"===c')):
            selected, owners = dict(self.report["selected"]), dict(self.owners)
            owner = copy.deepcopy(owners[selected[role]])
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after, 1)
            raw = owner["body"].encode()
            owner["body_sha256"] = hashlib.sha256(raw).hexdigest()
            owner["byte_end"] = owner["byte_start"] + len(raw)
            owners[selected[role]] = owner
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "selection requires review"):
                host._otfm_macro_encoding_contract(selected, owners, self.report["tables"])
        for mode in ("table", "missing", "identity"):
            selected, owners, tables = dict(self.report["selected"]), dict(self.owners), copy.deepcopy(self.report["tables"])
            if mode == "table":
                tables["opcodes"]["BUTTON"] = 10
            elif mode == "missing":
                del selected["worker_keys_factory"]
            else:
                owner = copy.deepcopy(owners[selected["macro_worker"]])
                owner["original"]["sha256"] = "0" * 64
                owners[selected["macro_worker"]] = owner
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "selection requires review"):
                host._otfm_macro_encoding_contract(selected, owners, tables)

    def test_changed_upstream_source_selection_or_resigned_missing_consumer_is_rejected(self):
        import host
        path = self.audit.a.ROOT / ".analysis/results/host-current/local-audit.json"
        read = Path.read_bytes
        for mode in ("semantics", "missing"):
            section = copy.deepcopy(self.upstream)
            if mode == "semantics":
                section["verified_contract"]["macro_local_write_error_propagated"] = True
            else:
                del section["selected"]["macro_worker"]
                section["self_signed"] = host._otfm_macro_encoding_digest(section)
            replacement = json.dumps({"otfm_device_writeback": section}).encode()
            def altered(selected):
                return replacement if selected == path else read(selected)
            with self.subTest(mode=mode), patch.object(Path, "read_bytes", altered), \
                    patch.object(host, "_otfm_device_writeback_sources") as sources, \
                    self.assertRaisesRegex(ValueError, "upstream section requires review"):
                host.collect_otfm_macro_encoding_chain(HostLocalAudit())
            sources.assert_not_called()
        changed = copy.deepcopy(self.report["source_selections"])
        changed["product"]["acquisition"]["sha256"] = "0" * 64
        with patch.object(host, "_otfm_device_writeback_sources", return_value=(changed, {"product": []})), \
                self.assertRaisesRegex(ValueError, "current source selection differs"):
            host.collect_otfm_macro_encoding_chain(HostLocalAudit())

    def test_dependency_recheck_retains_missing_changed_malformed_or_io_failure_and_allows_other_queries(self):
        import host
        path = self.audit.a.ROOT / ".analysis/results/host-current/local-audit.json"
        read = Path.read_bytes
        for mode in ("unchanged", "changed", "missing", "malformed", "io"):
            section = copy.deepcopy(self.upstream)
            if mode == "changed":
                section["snapshot_stable"] = False
            data = {"unrelated_query": {"changed": True}}
            if mode != "missing":
                data["otfm_device_writeback"] = section
            replacement = b"{" if mode == "malformed" else json.dumps(data).encode()
            def altered(selected):
                if selected == path:
                    if mode == "io":
                        raise OSError("receipt busy")
                    return replacement
                return read(selected)
            with self.subTest(mode=mode), patch.object(Path, "read_bytes", altered):
                changes = host._otfm_macro_encoding_dependency_changes(self.report["upstream_sections"])
            self.assertEqual(bool(changes), mode != "unchanged")
            if changes:
                self.assertEqual(changes, [".analysis/results/host-current/local-audit.json#/otfm_device_writeback"])


class OtfmDeviceWritebackTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_otfm_device_writeback(cls.audit)

    def test_current_product_host_and_ack_boundaries_roundtrip_without_runtime_promotion(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "complete_source_owners": list(self.audit.owners.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["source_selections"]["product"]["name"],
                         "keyboard-mw-756-current-refresh-ec3db4b29c5a-2")
        self.assertEqual(report["source_selections"]["background"]["name"],
                         "background-manager-current-refresh-ec3db4b29c5a")
        self.assertEqual(report["current_host"]["release_version"], "4.0.896")
        self.assertTrue(report["current_host"]["container_bytes_verified"])
        self.assertEqual(report["native"]["input"]["sha256"],
                         "5f185c3c865fb37a8cfb85d52a61704555543d70ea954bead84bbadda66fcfac")
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 8)
        self.assertTrue(all(row["pseudocode_captured"] and not row["failures"] and "capture" not in row
                            for row in report["native"]["complete_selected_functions"]))
        self.assertTrue(any(row.get("stage") == "30888:hexrays" for row in report["native"]["failures"]))
        proof = report["verified_contract"]
        self.assertEqual(proof["obm_engine"], "obmEngineKeyboardCalMaxMem")
        self.assertEqual(proof["mapping_device_method"], "setSingleKeyAssignment")
        self.assertEqual((proof["mapping_command_header"], proof["mapping_payload_size_override"]), ([80, 2, 13], 10))
        self.assertFalse(proof["legacy_analog_assignment_selected"])
        self.assertEqual(proof["ack_matches"], ["transaction_id", "class", "command"])
        self.assertEqual((proof["macro_name_bytes"], proof["macro_name_chunk_bytes"], proof["macro_data_chunk_bytes"]),
                         (250, 64, 72))
        for key in ("browser_save_waits_for_obm", "mapping_task_completed_waits_for_obm",
                    "macro_local_write_error_propagated", "ack_independent_setting_readback",
                    "ack_checks_received_length_or_checksum", "all_macro_bytes_read_back"):
            self.assertFalse(proof[key], key)
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key], key)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["runtime_acceptance"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_current_product_bytes_cannot_reuse_the_reviewed_chain(self):
        import host
        rows = copy.deepcopy(self.report["reviewed_inputs"])
        rows["tasks"]["sha256"] = "0" * 64
        originals = {"product": [row for name, row in rows.items() if name != "background"],
                     "background": [rows["background"]]}
        with patch.object(host, "_otfm_device_writeback_sources", return_value=({}, originals)), \
                self.assertRaisesRegex(ValueError, "requires semantic review: tasks"):
            host.collect_otfm_device_writeback(HostLocalAudit())

    def test_same_named_or_other_real_protocol_owner_cannot_replace_the_selected_writer(self):
        from host import _otfm_device_writeback_proof
        selected = {**self.report["selected"], "mapping_protocol": self.report["selected"]["macro_protocol"]}
        with self.assertRaisesRegex(ValueError, "boundary differs: mapping_protocol"):
            _otfm_device_writeback_proof(selected, self.audit.owners)

    def test_changed_v3_branch_failure_propagation_or_response_correlation_is_rejected(self):
        from host import _otfm_device_writeback_proof
        for role, before, after in (
                ("obm_init", 'this.options.analogKeyboard=!1', 'this.options.analogKeyboard=!0'),
                ("macro_local", 'catch(e){console.warn', 'catch(e){throw e;console.warn'),
                ("ack", 'c[1]!==n[1]', 'c[1]===n[1]')):
            owners = dict(self.audit.owners)
            key = self.report["selected"][role]
            owner = copy.deepcopy(owners[key])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            owners[key] = owner
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "boundary differs: " + role):
                _otfm_device_writeback_proof(self.report["selected"], owners)

    def test_old_host_hid_identity_or_incomplete_current_capture_is_rejected(self):
        import host
        original = {**self.report["native"]["input"],
                    "sha256": "f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f"}
        with patch.object(host, "current_host_extracted_inputs", return_value=[original]), \
                patch.object(host, "current_host_asar_members", return_value=[self.report["current_host"]["source"]]), \
                self.assertRaisesRegex(ValueError, "Current OTFM host HID requires semantic review"):
            host._otfm_device_writeback_host(HostLocalAudit())
        audit = HostLocalAudit()
        native = {"complete_selected_functions": [{"role": "async_operation_worker",
            "capture": {"rva": "0x4800"}, "pseudocode_captured": False, "failures": 0}]}
        with patch.object(audit.families, "verify_native_functions", return_value=native), \
                self.assertRaisesRegex(ValueError, "selected HID native owner is incomplete: async_operation_worker"):
            host._otfm_device_writeback_host(audit)


class LightingProductInputMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.audit = HostLocalAudit()
        cls.report = collect_lighting_product_input_map(cls.audit)

    def test_nine_manifest_branches_are_serializable_and_keep_runtime_unverified(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
                    "complete_source_owners": list(self.audit.owners.values()),
                    "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_lighting_product_input_map")
        self.assertEqual(report["manifest_count"], 9)
        self.assertEqual(report["table_count"], 27)
        self.assertEqual({row["request"]["product_id"] for row in report["manifest_branches"]},
                         {226, 227, 228, 756, 757, 758})
        special = next(row for row in report["manifest_branches"]
                       if row["request"]["product_id"] == 227 and row["request"]["edition_id"] == 128)
        self.assertEqual((special["declared_product_id"], special["filename_matches_declared_product"]), (226, False))
        self.assertEqual(report["product_sets"][0]["metadata"]["category"], "MOUSE")
        self.assertEqual(report["product_sets"][1]["metadata"]["category"], "KEYBOARD")
        mouse = next(row for row in report["manifest_branches"]
                     if row["request"]["product_id"] == 226 and row["request"]["edition_id"] == 0)
        self.assertEqual(mouse["tables"][0]["input_type_counts"], {"mou": 7, "dkm": 5})
        self.assertEqual(mouse["tables"][0]["whole_region_inputs"], 12)
        ble = next(row for row in report["manifest_branches"] if row["request"]["product_id"] == 758)
        self.assertTrue(ble["effective_flags"]["isRegisterInputEvents"])
        self.assertTrue(ble["effective_flags"]["isHardwareEffectsOnly"])
        self.assertFalse(ble["effective_flags"]["isChromaStudioSupported"])
        for name in ("source_chain_complete", "target_execution", "real_device_operations", "product_code_modified"):
            self.assertFalse(report.get(name, False))
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["runtime_acceptance"], "unverified")
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_manifest_identity_cannot_reuse_selected_product_mapping(self):
        import host
        audit = HostLocalAudit()
        original = audit.artifact

        def changed(path, *args, **kwargs):
            raw = original(path, *args, **kwargs)
            if Path(path).as_posix().endswith("DeviceManifest_227_128.json"):
                value = json.loads(raw)
                value["productId"] = 227
                return json.dumps(value).encode("utf-8")
            return raw

        with patch.object(audit, "artifact", side_effect=changed):
            with self.assertRaisesRegex(ValueError, "manifest identity differs"):
                host.collect_lighting_product_input_map(audit)

    def test_missing_actual_manifest_dependency_edge_cannot_be_reconstructed_from_filename(self):
        import host
        audit = HostLocalAudit()
        original = audit.artifact

        def changed(path, *args, **kwargs):
            raw = original(path, *args, **kwargs)
            if Path(path).as_posix().endswith("mouse-mw-226-snapshot-738e48c31af52973b33c/acquisition.json"):
                value = json.loads(raw)
                value["edges"] = [edge for edge in value["edges"]
                                   if "DeviceManifest_227_128.json" not in edge.get("to", "")]
                return json.dumps(value).encode("utf-8")
            return raw

        with patch.object(audit, "artifact", side_effect=changed):
            with self.assertRaisesRegex(ValueError, "manifest bytes or actual dependency edge"):
                host.collect_lighting_product_input_map(audit)


class MappingKernelInputProducerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_kernel_input_producer(cls.audit)

    def native_captures(self, key):
        report = self.report[key]
        return {"image_base": report["image_base"], "complete_selected_functions": [
            {"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
            for row in report["complete_selected_functions"]]}

    def test_current_producer_and_actual_product_qualification_keep_whole_chain_unverified(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
            "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False, allow_nan=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_kernel_input_producer")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual(report["native"]["input"]["sha256"],
            "64b72ef9ce409bb17d31096c119b68c4a6a02ab35b7535d763157a4e1a587e59")
        self.assertEqual(report["product_226_native"]["input"]["sha256"],
            "143ec6351d2078210ae7f0924407974b6e4f8c1b07e6129070e346d5e506b206")
        self.assertEqual([len(report[key]["complete_selected_functions"])
                          for key in ("native", "product_226_native")], [30, 11])
        for key in ("native", "product_226_native"):
            self.assertTrue(all(row["pseudocode_captured"] and row["input_chunks_verified"]
                and not row["failures"] and "capture" not in row for row in report[key]["complete_selected_functions"]))
        self.assertEqual(report["native"]["failures"], [])
        self.assertEqual([row["stage"] for row in report["product_226_native"]["failures"]], ["5a94:hexrays"])
        self.assertEqual(report["provenance"]["product_manifest"]["resource_version"], "26.0.0.50")
        for name in ("source_chain_complete", "whole_product_complete", "target_execution",
                     "real_device_operations", "product_code_modified"):
            self.assertFalse(report[name])
        self.assertEqual((report["new_native_captures"], report["new_analysis_sessions"]), (0, 0))
        self.assertEqual(report["temporary_files_created"], [])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["runtime_acceptance"], "unverified")

    def test_exact_frames_ring_failure_and_cleanup_contract_are_distinct(self):
        proof = self.report["verified_contract"]
        self.assertEqual((proof["receive_ioctl"], proof["notify_enable_ioctl"], proof["notify_mask"]),
                         ("0x88883018", "0x88883038", 4))
        self.assertEqual((proof["frame_bytes"], proof["event_offset"], proof["input_type_offset"],
                          proof["input_data_offset"]), (304, 8, 16, 20))
        self.assertEqual((proof["keyboard_record_bytes"], proof["mouse_record_bytes"]), (12, 24))
        self.assertEqual(proof["keyboard_notify_filter"],
                         {"suppressed_type": 1, "state_flag_offset": 1440, "scancode": 69})
        self.assertEqual(proof["mouse_notification_requires_nonzero_input_dword_offset"], 4)
        self.assertFalse(proof["ordinary_producers_clear_entire_frame"])
        self.assertEqual((proof["ring_allocation_bytes"], proof["ring_index_mask"]), (311600, 1023))
        self.assertEqual(proof["ring_overflow"], "advance read index, dropping oldest frame")
        self.assertTrue(proof["forward_failure_still_returns_pending"])
        self.assertEqual(proof["completion_information_bytes"], 304)
        self.assertTrue(proof["copy_failure_consumes_ring_frame"])
        self.assertFalse(proof["output_memory_failure_consumes_ring_frame"])
        self.assertTrue(proof["file_cleanup_removes_exact_file_object"])
        self.assertFalse(proof["full_pending_callback_drain_proven"])
        self.assertFalse(proof["other_product_qualification_proven"])

    def test_inf_actual_ids_and_provider_guid_are_not_inferred_from_family_name(self):
        infs = self.report["inf_contracts"]
        self.assertEqual([row["hardware_ids"] for row in infs[2:]], [
            ["HID\\Vid_1532&Pid_00e2&MI_00", "HID\\Vid_1532&Pid_00e3&MI_00"],
            ["HID\\Vid_1532&Pid_00e2&MI_02", "HID\\Vid_1532&Pid_00e3&MI_02"],
            ["USB\\Vid_1532&Pid_00e2&MI_01", "USB\\Vid_1532&Pid_00e3&MI_01"]])
        constants = {(row["input"], row["role"]): row["raw_hex"] for row in self.report["native_constants"]}
        self.assertEqual(constants[("common", "product_provider_guid")],
                         constants[("product_226", "product_provider_guid")])
        self.assertEqual(constants[("common", "mapping_interface_guid")], "5d00bee330d1104988ff09ae02f680e9")
        slots = self.report["product_226_native"]["verified_indirect_slots"]
        self.assertEqual([(row["slot_byte_offset"], row["target_rva"]) for row in slots],
                         [(8, "0x15c0"), (24, "0x1630"), (32, "0x1840"), (48, "0x19e0")])
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in slots))
        self.assertTrue(any("PID02f4/756" in line for line in self.report["remaining_chains"]))

    def test_changed_native_identity_cannot_reuse_reviewed_sys_semantics(self):
        import host
        inputs, provenance = host._mapping_kernel_input_inputs(HostLocalAudit())
        changed = {**inputs, "common": {**inputs["common"], "sha256": "0" * 64}}
        with patch.object(host, "_mapping_kernel_input_inputs", return_value=(changed, provenance)), \
                patch.object(self.audit.families, "verify_native_functions") as verify:
            with self.assertRaisesRegex(ValueError, "common requires semantic review"):
                host.collect_mapping_kernel_input_producer(HostLocalAudit())
        verify.assert_not_called()

    def test_unverified_or_quarantined_package_cannot_qualify_current_product(self):
        import host
        for change, error in (({"status": "quarantined"}, "package is absent or unverified"),
                ({"path": ".analysis/inputs/official-native-current/quarantine/PID00e2Drv.exe"}, "package declaration differs"),
                ({"declared_sha256": "0" * 64}, "package declaration differs")):
            audit = HostLocalAudit()
            original = audit.artifact

            def changed(path, expected=None):
                raw = original(path, expected)
                if audit.a.rel(path) == ".analysis/inputs/official-native-current/acquisition.json":
                    document = json.loads(raw)
                    row = next(row for row in document["resources"]
                               if row.get("url", "").endswith("/226/PID00e2Drv_v26.0.2.0.exe"))
                    row.update(change)
                    return json.dumps(document).encode()
                return raw

            with self.subTest(change=change), patch.object(audit, "artifact", side_effect=changed), \
                    patch.object(audit.families, "verify_native_functions") as verify:
                with self.assertRaisesRegex(ValueError, error):
                    host.collect_mapping_kernel_input_producer(audit)
            verify.assert_not_called()

    def test_changed_pe_version_cannot_publish_the_reviewed_contract(self):
        import host
        inputs, provenance = host._mapping_kernel_input_inputs(HostLocalAudit())
        pe = host.inspect_pe(self.audit.read(inputs["common"]))
        pe["version_resources"][0]["tree"]["file_version"] = "1.0.0.75"
        with patch.object(host, "_mapping_kernel_input_inputs", return_value=(inputs, provenance)), \
                patch.object(host, "inspect_pe", return_value=pe), \
                patch.object(self.audit.families, "verify_native_functions") as verify:
            with self.assertRaisesRegex(ValueError, "architecture or version differs"):
                host.collect_mapping_kernel_input_producer(HostLocalAudit())
        verify.assert_not_called()

    def test_changed_extracted_member_ancestry_is_rejected_before_native_analysis(self):
        import host
        audit = HostLocalAudit()
        original = audit.artifact

        def changed(path, expected=None):
            raw = original(path, expected)
            if audit.a.rel(path) == ".analysis/inputs.json":
                document = json.loads(raw)
                row = next(row for row in document["inputs"]
                           if row["path"] == self.report["provenance"]["product_original"]["path"])
                row["verified_against_manifest"] = False
                return json.dumps(document).encode()
            return raw

        with patch.object(audit, "artifact", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as verify:
            with self.assertRaisesRegex(ValueError, "member package ancestry differs"):
                host.collect_mapping_kernel_input_producer(audit)
        verify.assert_not_called()

    def test_incomplete_selected_capture_cannot_hide_behind_whole_capture_completeness(self):
        import host
        for captured, failures in ((False, 0), (True, 1)):
            native = {"complete_selected_functions": [{"role": "keyboard_frame_producer",
                       "pseudocode_captured": captured, "failures": failures}]}
            with self.subTest(captured=captured, failures=failures), \
                    patch.object(self.audit.families, "verify_native_functions", return_value=native):
                with self.assertRaisesRegex(ValueError, "function remains incomplete: keyboard_frame_producer"):
                    host.collect_mapping_kernel_input_producer(HostLocalAudit())

    def test_changed_event_offsets_motion_gate_or_ring_drop_oldest_is_rejected(self):
        import host
        common, product = self.native_captures("native"), self.native_captures("product_226_native")
        for rva, replacement in (("0x3c91", "c744244808000000"), ("0x3eec", "394b08"),
                ("0x3f2b", "4883c310"), ("0x5fdc", "bfff070000"), ("0x5ffb", "894340")):
            changed = copy.deepcopy(common)
            instruction = next(line for row in changed["complete_selected_functions"]
                               for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "common instruction differs: " + rva):
                host._mapping_kernel_input_proof(changed, product)

    def test_changed_pending_failure_or_completion_information_is_rejected(self):
        import host
        common, product = self.native_captures("native"), self.native_captures("product_226_native")
        for rva, replacement in (("0x5ac0", "b800000000"), ("0x5edb", "41b900000000"),
                                 ("0x5ed1", "894340"), ("0x5932", "488b5110")):
            changed = copy.deepcopy(common)
            instruction = next(line for row in changed["complete_selected_functions"]
                               for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "common instruction differs: " + rva):
                host._mapping_kernel_input_proof(changed, product)

    def test_missing_actual_keyboard_or_product_owner_edge_is_not_satisfied_by_function_names(self):
        import host
        for key, owner, target in (("native", 0x5680, 0x3bbc), ("product_226_native", 0x2594, 0x1198)):
            common, product = self.native_captures("native"), self.native_captures("product_226_native")
            changed = common if key == "native" else product
            function = next(row["capture"] for row in changed["complete_selected_functions"]
                            if int(row["capture"]["rva"], 16) == owner)
            for line in function["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", [])
                                      if int(xref["to"], 16) != 0x140000000 + target]
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "actual edge differs"):
                host._mapping_kernel_input_proof(common, product)


class MappingPhysicalInputDecodeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_physical_input_decode(cls.audit)

    def native_captures(self):
        return {"image_base": self.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads((self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report["native"]["complete_selected_functions"]]}

    def test_current_frame_chain_is_complete_serializable_and_keeps_runtime_unverified(self):
        report = self.report
        complete = {**report, "complete_originals": list(self.audit.sources.values()),
                    "complete_source_owners": list(self.audit.owners.values()),
                    "verified_artifacts": list(self.audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_mapping_physical_input_decode")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual(report["native"]["input"]["sha256"],
                         "c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9")
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"] and not row["failures"]
                            and "capture" not in row for row in report["native"]["complete_selected_functions"]))
        self.assertTrue(all(row["actual_pe_bytes_verified"] for row in report["native"]["verified_indirect_slots"]))
        proof = report["verified_contract"]
        self.assertEqual((proof["ioctl"], proof["frame_bytes"]), ("0x88883018", 304))
        self.assertEqual(proof["overlapped_base_offsets"], [536, 872])
        self.assertEqual(proof["output_base_offsets"], [568, 904])
        self.assertEqual((proof["frame_event_offset"], proof["frame_input_type_offset"]), (8, 16))
        self.assertEqual(proof["accepted_event_codes"], [1, 2, 4])
        self.assertEqual(proof["notify_event_code"], 4)
        self.assertEqual(proof["wheel_signed16_clamp"], {"minimum": -120, "maximum": 120})
        self.assertEqual(proof["mouse_frame_offsets"], {"unit_id": 20, "id": 24, "data": 26})
        self.assertEqual((proof["device_observer_subobject_offset"], proof["observer_notify_slot"]), (120, 40))
        self.assertTrue(proof["notification_rearms_other_buffer_before_decode"])
        self.assertFalse(proof["completion_checks_returned_byte_count"])
        constants = {row["role"]: row for row in report["native_constants"]}
        self.assertEqual(bytes.fromhex(constants["keyboard_json_type"]["raw_hex"]), b"keyboard\0")
        self.assertEqual(bytes.fromhex(constants["mouse_json_type"]["raw_hex"]), b"mouse\0")
        for name in ("source_chain_complete", "target_execution", "real_device_operations", "product_code_modified"):
            self.assertFalse(report[name])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["runtime_acceptance"], "unverified")

    def test_changed_current_native_identity_cannot_reuse_the_selected_decoder(self):
        import host
        original = {**self.report["provenance"]["original"], "sha256": "0" * 64}
        with patch.object(host, "_mapping_physical_input_inputs", return_value=(original, {})), \
                patch.object(self.audit.families, "verify_native_functions") as verify:
            with self.assertRaisesRegex(ValueError, "requires semantic review"):
                host.collect_mapping_physical_input_decode(HostLocalAudit())
        verify.assert_not_called()

    def test_incomplete_selected_native_function_cannot_publish_reviewed_semantics(self):
        import host
        for captured, failures in ((False, 0), (True, 1)):
            audit = HostLocalAudit()
            native = {"complete_selected_functions": [{"role": "mouse_driver_decode",
                "capture": {}, "pseudocode_captured": captured, "failures": failures}]}
            with self.subTest(captured=captured, failures=failures), \
                    patch.object(audit.families, "verify_native_functions", return_value=native):
                with self.assertRaisesRegex(ValueError, "native function remains incomplete: mouse_driver_decode"):
                    host.collect_mapping_physical_input_decode(audit)

    def test_unsigned_wheel_clamp_changed_event_mask_or_wrong_observer_slot_is_rejected(self):
        import host
        native = self.native_captures()
        for rva, replacement in (("0x115190", "0f42d0"), ("0x114de9", "b914000000"),
                                 ("0xc2093", "488b4020")):
            changed = copy.deepcopy(native)
            instruction = next(line for row in changed["complete_selected_functions"]
                               for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "frame or ABI differs: " + rva):
                host._mapping_physical_input_proof(changed)

    def test_missing_actual_mouse_decode_edge_is_not_satisfied_by_named_functions(self):
        import host
        native = self.native_captures()
        completion = next(row["capture"] for row in native["complete_selected_functions"]
                          if row["capture"]["rva"] == "0x114ad0")
        for line in completion["assembly"]:
            line["xrefs_from"] = [xref for xref in line.get("xrefs_from", [])
                                  if xref["to"] != "0x18011513f"]
        with self.assertRaisesRegex(ValueError, "decode or release edge differs: 0x114ad0 -> 0x11513f"):
            host._mapping_physical_input_proof(native)


class MacroNativeRecorderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from host import collect_macro_native_recorder
        cls.audit = HostLocalAudit()
        cls.report = collect_macro_native_recorder(cls.audit)

    def test_current_callback_chain_has_complete_owners_and_separate_acceptance(self):
        report, audit = self.report, self.audit
        complete = {**report, "complete_originals": list(audit.sources.values()),
                    "complete_source_owners": list(audit.owners.values()),
                    "verified_artifacts": list(audit.artifacts.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_macro_native_recorder")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertIn("/releases/4.0.896/", report["provenance"]["acquisition"]["path"])
        self.assertEqual(report["native"]["input"]["sha256"],
                         "c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9")
        self.assertEqual(len(audit.owners), 40)
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 109)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 20)
        self.assertEqual(len(report["native"]["verified_exports"]), 8)
        self.assertEqual(sum(row["failures"] for row in report["native"]["complete_selected_functions"]), 0)
        self.assertTrue(all("capture" not in row for row in report["native"]["complete_selected_functions"]))
        proof = report["verified_contract"]
        self.assertEqual(proof["observer_subobject_offset"], 40)
        self.assertEqual(proof["callback_base_offsets"], {"started": 176, "stopped": 184, "item": 192})
        self.assertEqual(proof["registered_flag_base_offset"], 168)
        self.assertEqual([proof["events"][name]["kind"] for name in ("started", "stopped", "item")], [0, 1, 2])
        self.assertTrue(proof["item_timestamp_r8_preserved"])
        self.assertFalse(proof["unregister_removes_observer"])
        self.assertFalse(proof["unregister_clears_callbacks"])
        self.assertFalse(proof["stop_joins_threads"])
        owners = audit.owners
        self.assertIn('{mode:e,eventType:t,timeTick:s}',
                      owners[report["selected_host_owners"]["createCbMacroRecorderStarted"]]["body"])
        self.assertIn('timerTick:e.timerTick', owners[report["selected_consumers"]["otfm_started"]]["body"])
        self.assertNotIn('macrorecordingstopped',
                         owners[report["selected_consumers"]["otfm_listener_lifecycle"]]["body"])
        for name in ("source_chain_complete", "target_execution", "real_device_operations", "product_code_modified"):
            self.assertFalse(report[name])
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["runtime_acceptance"], "unverified")
        self.assertEqual(audit.final_snapshot(), [])

    def test_changed_host_callback_owner_is_rejected_before_native_review(self):
        from host import collect_macro_native_recorder
        audit = HostLocalAudit()
        original = audit.callable_member

        def changed(role, *args, **kwargs):
            node, owner = original(role, *args, **kwargs)
            if role == "macro_native_host_createCbMacroItem":
                audit.owners[owner]["body_sha256"] = "0" * 64
            return node, owner

        with patch.object(audit, "callable_member", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "semantic review: createCbMacroItem"):
                collect_macro_native_recorder(audit)
        native.assert_not_called()

    def test_callback_body_does_not_hide_changed_ffi_thread_branch(self):
        from host import collect_macro_native_recorder
        audit = HostLocalAudit()
        original = audit.binding

        def changed(role, *args, **kwargs):
            value = original(role, *args, **kwargs)
            return {**value, "body": "t=false"} if role == "macro_native_host_binding_t" else value

        with patch.object(audit, "binding", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "host binding differs: t"):
                collect_macro_native_recorder(audit)
        native.assert_not_called()

    def test_consumer_owner_does_not_hide_a_different_api_bridge(self):
        from host import collect_macro_native_recorder
        audit = HostLocalAudit()
        original = audit.binding

        def changed(role, *args, **kwargs):
            value = original(role, *args, **kwargs)
            return {**value, "body": "{apiElectron:r}=window"} if role == "macro_native_consumer_binding_r" else value

        with patch.object(audit, "binding", side_effect=changed), \
                patch.object(audit.families, "verify_native_functions") as native:
            with self.assertRaisesRegex(ValueError, "consumer binding differs: r"):
                collect_macro_native_recorder(audit)
        native.assert_not_called()

    def test_previous_native_identity_cannot_reuse_current_recorder_addresses(self):
        import host
        provenance = self.report["provenance"]
        native = {**provenance["native"], "sha256":
                  "6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a"}
        audit = HostLocalAudit()
        with patch.object(host, "_macro_native_recorder_inputs", return_value=(
                native, provenance["source"], provenance)), \
                patch.object(audit.families, "verify_native_functions") as verify:
            with self.assertRaisesRegex(ValueError, "native input requires semantic review"):
                host.collect_macro_native_recorder(audit)
        verify.assert_not_called()

    def test_incomplete_current_callback_function_cannot_publish_reviewed_semantics(self):
        from host import collect_macro_native_recorder
        for captured, failures in ((False, 0), (True, 1)):
            with self.subTest(captured=captured, failures=failures):
                audit = HostLocalAudit()
                native = {"complete_selected_functions": [{"role": "item_kind2_adapter",
                    "capture": {}, "pseudocode_captured": captured, "failures": failures}]}
                with patch.object(audit.families, "verify_native_functions", return_value=native):
                    with self.assertRaisesRegex(ValueError, "native function remains incomplete: item_kind2_adapter"):
                        collect_macro_native_recorder(audit)

    def test_changed_event_kind_subobject_generation_or_lost_tick_rejects_native_proof(self):
        from host import _macro_native_recorder_proof
        root = Path(__file__).resolve().parents[1]
        native = {"image_base": self.report["native"]["image_base"], "complete_selected_functions": [
            {"capture": json.loads((root / row["artifact"]["path"]).read_bytes())}
            for row in self.report["native"]["complete_selected_functions"]]}
        self.assertTrue(_macro_native_recorder_proof(native)["item_timestamp_r8_preserved"])
        for address, replacement in (("0x10aa", "48894120"), ("0x49cc4", "b901000000"),
                                     ("0xb19d6", "483b4638")):
            with self.subTest(address=address):
                changed = copy.deepcopy(native)
                instruction = next(line for row in changed["complete_selected_functions"]
                                   for line in row["capture"]["assembly"] if line["rva"] == address)
                instruction["bytes"] = replacement
                with self.assertRaisesRegex(ValueError, "ownership or event ABI differs: " + address):
                    _macro_native_recorder_proof(changed)
        changed = copy.deepcopy(native)
        item = next(row["capture"] for row in changed["complete_selected_functions"]
                    if row["capture"]["rva"] == "0x49ca0")
        item["assembly"].append({"rva": "0x49ccd", "bytes": "4531c0", "text": "xor r8d, r8d"})
        with self.assertRaisesRegex(ValueError, "no longer preserves timestamp R8"):
            _macro_native_recorder_proof(changed)
        changed = copy.deepcopy(native)
        release = next(row["capture"] for row in changed["complete_selected_functions"]
                       if row["capture"]["rva"] == "0xd9a68")
        for line in release["assembly"]:
            line["xrefs_from"] = [xref for xref in line.get("xrefs_from", [])
                                  if xref["to"] != "0x1801b8e40"]
        with self.assertRaisesRegex(ValueError, "release edge differs: 0xd9a68 -> 0x1b8e40"):
            _macro_native_recorder_proof(changed)


class LightingTimedEffectsDispatchTests(unittest.TestCase):
    def test_timed_query_uses_its_own_collector_and_preserves_static_scope(self):
        import host
        parser = argparse.ArgumentParser()
        add_commands(parser.add_subparsers(dest="command", required=True))
        args = parser.parse_args(["audit-host-local", "--query", "lighting-timed-effects"])
        with patch.object(host, "collect_lighting_timed_effects", return_value={"failures": []}) as timed, \
                patch.object(host, "collect_lighting_static_layer") as static, \
                patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"):
            audit_host_local(args)
        timed.assert_called_once()
        static.assert_not_called()
        report, _, _, section = publish.call_args.args
        self.assertEqual(section, "lighting_timed_effects")
        self.assertTrue(report["bounded_observation_complete"])
        self.assertFalse(report["source_chain_complete"])
        self.assertFalse(report["target_execution"])

    def test_timed_failure_and_changed_snapshot_publish_unresolved_facts(self):
        import host
        for changed in (False, True):
            with self.subTest(changed=changed), \
                    patch.object(host, "collect_lighting_timed_effects",
                        **({"return_value": {"failures": []}} if changed else {"side_effect": ValueError("missing timed input")})), \
                    patch.object(HostLocalAudit, "final_snapshot", return_value=["changed.js"] if changed else []), \
                    patch.object(host, "publish_host_static_query") as publish, patch("builtins.print"), \
                    self.assertRaisesRegex(ValueError, "Lighting timed effects query remains unresolved"):
                audit_host_local(argparse.Namespace(query="lighting-timed-effects", verify_only=False))
            report, _, _, section = publish.call_args.args
            self.assertEqual(section, "lighting_timed_effects")
            self.assertFalse(report["bounded_observation_complete"])
            self.assertEqual(report["snapshot_stable"], not changed)
            self.assertIn("changed.js" if changed else "missing timed input", report["failures"][0])


class KeypadMappingConsumersTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_keypad_mapping_consumers(cls.audit)

    def test_current_mounted_consumers_reuse_complete_owners_without_promoting_acceptance(self):
        report = self.report
        complete = {**report, "complete_source_owners": list(self.audit.owners.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_keypad_mapping_consumers")
        self.assertEqual(len(report["reviewed_inputs"]), 13)
        self.assertEqual(report["deployment_revalidation"]["selected_owner_count"], 13)
        comparisons = report["prior_complete_owner_comparisons"]
        self.assertEqual(len(comparisons), 39)
        self.assertEqual(sum(row["complete_body_byte_identical"] for row in comparisons), 36)
        self.assertTrue(all(row["reviewed_difference"] for row in comparisons
            if not row["complete_body_byte_identical"]))
        proof = report["verified_contract"]
        self.assertEqual(proof["actual_compiler_path"],
            ["1610.B", "32226.A", "4325.RzAppEngineMapping_default", "Fe"])
        for name in ("ui_save_waits_for_middleware_or_device_ack", "direction_task_awaits_queued_runner",
                "direction_runner_checks_setter_status", "direction_runner_restores_partial_writes",
                "completed_proves_every_obm_write_succeeded", "single_key_thresholds_transmitted",
                "single_key_echo_is_independent_readback", "device_state_restored_on_discard_or_cancel",
                "current_native_consumers_revalidated"):
            self.assertFalse(proof[name], name)
        self.assertEqual(proof["single_key_packet_size"], 10)
        for name in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(report[name], name)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertIn("prior_4.0.827_reference_only", report["dependencies"][0]["status"])
        self.assertEqual(report["cleanup"]["temporary_files_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_current_source_change_cannot_reuse_the_reviewed_file_or_old_current_alias(self):
        import host
        selections = []
        for side in ("ui", "mw"):
            current = copy.deepcopy(self.report["source_selections"][side])
            current["originals"] = [copy.deepcopy(row) for role, row in self.report["reviewed_inputs"].items()
                if role.startswith("ui_") == (side == "ui")]
            if side == "mw":
                next(row for row in current["originals"] if "/6737." in row["path"])["sha256"] = "0" * 64
            selections.append(current)
        with patch("frontend.current_source_selection", side_effect=selections):
            with self.assertRaisesRegex(ValueError, "requires semantic review: tasks"):
                host._keypad_mapping_consumer_sources(HostLocalAudit())

    def test_optimistic_save_direction_abort_and_per_slot_failure_boundaries_are_guarded(self):
        import host
        for role, before, after in (
                ("ui_save", "t&&this.saveChanges()", "t&&await this.saveChanges()"),
                ("direction_runner", "s.result=!0", "s.result=!1"),
                ("mapping_runner", 'A.value.status="completed"', 'A.value.status="confirmed"'),
                ("keypad_slot_writer", "i&&(o=1)", "i&&(o=2)"),
                ("direction_defaults", "t.isHyperShift===e.isHyperShift", "!!t.isHyperShift===!!e.isHyperShift"),
                ("projection", "(0,Dl.cA)(t.directionMode,o)", "(0,Dl.cA)(3,o)")):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "reviewed owner differs: " + role):
                host._keypad_mapping_consumer_proof(self.report["selected"], owners)

    def test_missing_actual_compiler_adapter_does_not_pass_on_an_inline_same_named_compiler(self):
        import host
        selected = dict(self.report["selected"])
        selected["compiler_adapter"] = selected["mapping_task"]
        with self.assertRaisesRegex(ValueError, "reviewed owner differs: compiler_adapter"):
            host._keypad_mapping_consumer_proof(selected, self.audit.owners)

    def test_single_key_actual_payload_size_cannot_be_inferred_from_its_80_byte_constant(self):
        import host
        owners = copy.deepcopy(self.audit.owners)
        owner = owners[self.report["selected"]["single_key_writer"]]
        owner["body"] = owner["body"].replace("setSingleKeyAssignment(e,c,u,y,g,f,10)",
            "setSingleKeyAssignment(e,c,u,y,g,f,80)")
        owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "reviewed owner differs: single_key_writer"):
            host._keypad_mapping_consumer_proof(self.report["selected"], owners)

    def test_reused_Fe_strict_layer_matching_cannot_silently_change(self):
        import host
        audit = HostLocalAudit()
        original = audit.binding

        def changed(role, source, name, reference):
            result = original(role, source, name, reference)
            if role == "keypad_consumers_Fe_Ce":
                owner = audit.owners[result["owner_id"]]
                self.assertIn("e.isHyperShift===a", owner["body"])
                owner["body"] = owner["body"].replace("e.isHyperShift===a", "e.isHyperShift==a")
                owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            return result

        with patch.object(audit, "binding", side_effect=changed):
            with self.assertRaisesRegex(ValueError, "Fe dependency changed: Ce"):
                host.collect_keypad_mapping_consumers(audit)

    def test_missing_second_GET_or_wrong_acquisition_cannot_promote_current_deployment(self):
        import host
        identity = self.report["deployment_revalidation"]["receipt"]
        receipt = json.loads((self.audit.a.ROOT / identity["path"]).read_bytes())
        for change in ("round", "acquisition"):
            modified = copy.deepcopy(receipt)
            if change == "round":
                source = self.report["reviewed_inputs"]["tasks"]
                row = next(row for row in modified["observations"]
                    if (row.get("retained_original") or {}).get("path") == source["path"])
                row["rounds"].pop()
            else:
                modified["source_sets"][0]["acquisition"]["sha256"] = "0" * 64
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "deployment"):
                host._keypad_mapping_consumer_deployment(modified,
                    self.report["reviewed_inputs"], self.report["source_selections"])


class KeypadMappingCurrentNativeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_keypad_mapping_current_native(cls.audit)
        cls.raw = cls.audit.read(cls.report["provenance"]["original"])
        cls.native = {"image_base": cls.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads(
                (cls.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in cls.report["native"]["complete_selected_functions"]]}

    def test_current_consumers_keep_output_completion_readback_and_acceptance_distinct(self):
        report = self.report
        complete = {**report, "complete_source_owners": list(self.audit.owners.values())}
        self.assertEqual(json.loads(json.dumps(complete, ensure_ascii=False)), complete)
        self.assertEqual(report["kind"], "current_keypad_mapping_current_native")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual(report["native"]["input"]["sha256"],
            "c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9")
        self.assertEqual((report["verified_contract"]["functions_verified"],
            report["verified_contract"]["instructions_verified"]), (75, 10054))
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 24)
        self.assertTrue(all(row["input_chunks_verified"] and row["pseudocode_captured"]
            and not row["failures"] and "capture" not in row
            for row in report["native"]["complete_selected_functions"]))
        proof = report["verified_contract"]
        self.assertEqual(proof["make_lookup_priority"],
            ["gamemode", "otfs", "dkp", "snaptap", "global_shortcut", "ordinary"])
        self.assertEqual(proof["pending_pair_owner_offset"], 1288)
        self.assertTrue(proof["pending_pair_releases_oldest_before_new_output"])
        self.assertFalse(proof["pair_destructor_executes_release_mapping"])
        self.assertEqual(proof["driver_output"]["ioctl"], "0x88883020")
        self.assertEqual(proof["driver_output"]["bytes"], 32)
        self.assertFalse(proof["driver_output"]["independent_device_readback"])
        self.assertFalse(proof["whole_output_family_complete"])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key], key)
        for key in ("ui_backend_wiring", "actual_dll_comparison", "runtime_acceptance"):
            self.assertEqual(report[key], "unverified", key)
        self.assertEqual(report["cleanup"]["temporary_files_created"], [])
        self.assertFalse(report["cleanup"]["new_capture_created"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_every_dependency_resolves_to_the_canonical_formal_receipt_section(self):
        sections = []
        for dependency in self.report["dependencies"]:
            receipt = json.loads((self.audit.a.ROOT / dependency["receipt"]).read_bytes())
            self.assertIsInstance(receipt[dependency["section"]], dict)
            sections.append(dependency["section"])
        self.assertEqual(sections, ["keypad_mapping_consumers", "mapping_ble_analog_stick_routing",
            "mapping_physical_input_decode", "mapping_input_shutdown_chain"])

    def test_prior_native_or_a_same_named_nonselected_compiler_cannot_fill_the_current_join(self):
        import host
        prior = {**self.report["provenance"]["original"], "sha256":
            "6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a"}
        source = {"kind": "current_keypad_mapping_consumers", "product_id": 747,
            "verified_contract": {"actual_compiler_path":
                ["1610.B", "32226.A", "4325.RzAppEngineMapping_default", "Fe"]}}
        with patch.object(host, "collect_keypad_mapping_consumers", return_value=source), \
                patch.object(host, "_mapping_physical_input_inputs", return_value=(prior, {})):
            with self.assertRaisesRegex(ValueError, "native input requires semantic review"):
                host.collect_keypad_mapping_current_native(HostLocalAudit())
        source["verified_contract"]["actual_compiler_path"][2] = "inline.RzAppEngineMapping_default"
        with patch.object(host, "collect_keypad_mapping_consumers", return_value=source), \
                patch.object(host, "_mapping_physical_input_inputs") as native:
            with self.assertRaisesRegex(ValueError, "actual source compiler join differs"):
                host.collect_keypad_mapping_current_native(HostLocalAudit())
            native.assert_not_called()

    def test_missing_pending_pair_release_or_deferred_executor_is_rejected(self):
        import host
        for rva in ("0x88654", "0x878ae"):
            native = {**self.native, "complete_selected_functions": [row
                for row in self.native["complete_selected_functions"] if row["capture"]["rva"] != rva]}
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError,
                    "selected consumer or release owner differs"):
                host._keypad_mapping_current_native_proof(self.raw, native)

    def test_changed_instruction_or_rehashed_machine_code_cannot_disagree_with_the_pe(self):
        import host
        for change in ("instruction", "chunk"):
            native = copy.deepcopy(self.native)
            owner = next(row["capture"] for row in native["complete_selected_functions"]
                if row["capture"]["rva"] == "0x87842")
            if change == "instruction":
                instruction = owner["assembly"][0]
                encoded = bytearray.fromhex(instruction["bytes"])
                encoded[0] ^= 1
                instruction["bytes"] = encoded.hex()
                message = "instruction differs from PE bytes or owner"
            else:
                chunk = owner["chunks"][0]
                encoded = bytearray.fromhex(chunk["raw_hex"])
                encoded[0] ^= 1
                chunk.update(raw_hex=encoded.hex(), sha256=hashlib.sha256(encoded).hexdigest())
                message = "function chunk differs from PE bytes"
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, message):
                host._keypad_mapping_current_native_proof(self.raw, native)

    def test_actual_deferred_output_and_release_edges_are_required(self):
        import host
        for owner_rva, target in (("0x85f0a", 0x88654), ("0x88654", 0x878ae),
                ("0x878ae", 0xff82e), ("0x112ee0", 0x110dda), ("0x9acfa", 0x556fa)):
            native = copy.deepcopy(self.native)
            owner = next(row["capture"] for row in native["complete_selected_functions"]
                if row["capture"]["rva"] == owner_rva)
            for line in owner["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", [])
                    if int(xref["to"], 16) != int(native["image_base"], 16) + target]
            with self.subTest(owner=owner_rva), self.assertRaisesRegex(ValueError,
                    "consumer or release edge differs: " + owner_rva):
                host._keypad_mapping_current_native_proof(self.raw, native)

    def test_complete_priority_deferred_aggregate_output_and_destructor_bodies_are_required(self):
        import host
        for rva in ("0x66d10", "0x85f0a", "0x878ae", "0x87006", "0xaf5ca",
                "0x112ee0", "0xad0cc", "0x656ca"):
            native = copy.deepcopy(self.native)
            owner = next(row["capture"] for row in native["complete_selected_functions"]
                if row["capture"]["rva"] == rva)
            owner["pseudocode"]["lines"].append({"text": "return confirmed_device_readback;"})
            owner["pseudocode_sha256"] = hashlib.sha256("\n".join(
                line["text"] for line in owner["pseudocode"]["lines"]).encode()).hexdigest()
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError,
                    "complete reviewed owner differs: " + rva):
                host._keypad_mapping_current_native_proof(self.raw, native)

    def test_actual_driver_virtual_slot_cannot_be_replaced_by_a_neighbor(self):
        import host
        raw = bytearray(self.raw)
        rva = 0x2f1990 + 96
        section = next(row for row in inspect_pe(raw)["sections"]
            if row["rva"] <= rva < row["rva"] + row["bytes"])
        offset = section["file_offset"] + rva - section["rva"]
        raw[offset:offset + 8] = struct.pack("<Q", int(self.native["image_base"], 16) + 0x110dda)
        with self.assertRaisesRegex(ValueError, "adjusted delegate or driver slot differs"):
            host._keypad_mapping_current_native_proof(bytes(raw), self.native)


class KeypadTemporaryKeymapChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_keypad_temporary_keymap_chain(cls.audit)
        cls.raw = cls.audit.read(cls.report["provenance"]["original"])
        cls.native = {"image_base": cls.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads(
                (cls.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in cls.report["native"]["complete_selected_functions"]]}

    def test_actual_747_temporary_producers_remain_distinct_from_all_runtime_acceptance(self):
        report = self.report
        self.assertEqual(report["kind"], "current_keypad_temporary_keymap_chain")
        self.assertEqual(report["product_id"], 747)
        self.assertTrue(report["temporary_keymap_producer_slice_complete"])
        self.assertEqual(report["verified_contract"]["source"]["selected_source_owners"], 47)
        self.assertEqual(report["verified_contract"]["native"]["functions_verified"], 13)
        self.assertEqual(report["verified_contract"]["native"]["instructions_verified"], 1522)
        self.assertEqual(len(report["reused_native_owners"]), 11)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 5)
        self.assertFalse(report["verified_contract"]["source"]["temporary_active_keymap_id_persisted"])
        self.assertFalse(report["verified_contract"]["native"]["last_key_read_is_a_generation_barrier"])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key], key)
        for key in ("ui_backend_wiring", "actual_dll_comparison", "runtime_acceptance"):
            self.assertEqual(report[key], "unverified", key)
        self.assertEqual(report["cleanup"]["temporary_files_created"], [])
        self.assertFalse(report["cleanup"]["new_capture_created"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_clutch_press_release_persistence_and_failure_semantics_require_complete_reviewed_owners(self):
        import host
        for role, old, new in (("actual_keymap_compiler", 'guid:e.keymapGroup.previousKeymapGuid',
                    'guid:e.keymapGroup.previousKeymapGuid,id:"specificKeymap"'),
                ("context_factory", 's.appEngine=structuredClone(a.appEngine)', 'Object.assign(s,a)'),
                ("selection_task", 'loadKeyMappings)(s,T,o,i,!0)', 'loadKeyMappings)(s,T,o,i,!1)'),
                ("temporary_mapping_task", 'finally{I&&Il.Ay.setTemporaryKeypadKeymapContext(null)}',
                    'finally{Il.Ay.setTemporaryKeypadKeymapContext(null)}'),
                ("clutch_predicate", '!0===(null==o?void 0:o.isClutch)', '!1===(null==o?void 0:o.isClutch)'),
                ("storage_hook_owner", 'this.hooks.PostSetLocalStorage(t)', 'yield this.hooks.PostSetLocalStorage(t)')):
            owners = copy.deepcopy(self.audit.owners)
            row = owners[self.report["selected"][role]]
            self.assertIn(old, row["body"], role)
            row["body"] = row["body"].replace(old, new, 1)
            row["body_sha256"] = hashlib.sha256(row["body"].encode()).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete reviewed source selection differs"):
                host._keypad_temporary_keymap_source_proof(self.report["selected"], owners)

    def test_inline_same_named_compiler_cannot_replace_the_actual_cross_chunk_keymap_owner(self):
        import host
        selected = dict(self.report["selected"])
        selected["actual_keymap_compiler"] = selected["selection_task"]
        with self.assertRaisesRegex(ValueError, "complete reviewed source selection differs"):
            host._keypad_temporary_keymap_source_proof(selected, self.audit.owners)

    def test_missing_key_read_callback_notification_or_config_owner_is_rejected(self):
        import host
        for rva in ("0x77df0", "0x555b0", "0x8563e", "0xe3d90"):
            native = {**self.native, "complete_selected_functions": [row for row in self.native[
                "complete_selected_functions"] if row["capture"]["rva"] != rva]}
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "selected native owner differs"):
                host._keypad_temporary_keymap_native_proof(self.raw, native)

    def test_real_storage_key_and_notification_continuation_edges_are_required(self):
        import host
        for owner_rva, target in (("0x77df0", 0x784d0), ("0x55484", 0x555b0),
                ("0x78920", 0xe3d90), ("0x8563e", 0xa3afc), ("0xaa2c8", 0x7b350)):
            native = copy.deepcopy(self.native)
            owner = next(row["capture"] for row in native["complete_selected_functions"]
                if row["capture"]["rva"] == owner_rva)
            for line in owner["assembly"]:
                line["xrefs_from"] = [xref for xref in line.get("xrefs_from", [])
                    if int(xref["to"], 16) != int(native["image_base"], 16) + target]
            with self.subTest(rva=owner_rva), self.assertRaisesRegex(ValueError, "continuation/publication edge differs"):
                host._keypad_temporary_keymap_native_proof(self.raw, native)

    def test_changed_instruction_or_rehashed_chunk_cannot_disagree_with_actual_pe_bytes(self):
        import host
        for change in ("instruction", "chunk"):
            native = copy.deepcopy(self.native)
            owner = next(row["capture"] for row in native["complete_selected_functions"]
                if row["capture"]["rva"] == "0x555b0")
            if change == "instruction":
                line = owner["assembly"][0]
                raw = bytearray.fromhex(line["bytes"])
                raw[0] ^= 1
                line["bytes"] = raw.hex()
                message = "instruction differs from PE bytes or owner"
            else:
                chunk = owner["chunks"][0]
                raw = bytearray.fromhex(chunk["raw_hex"])
                raw[0] ^= 1
                chunk.update(raw_hex=raw.hex(), sha256=hashlib.sha256(raw).hexdigest())
                message = "function chunk differs from PE bytes"
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, message):
                host._keypad_temporary_keymap_native_proof(self.raw, native)

    def test_complete_native_interpretations_cannot_claim_additional_generation_or_device_ack(self):
        import host
        for rva in ("0x77df0", "0x784d0", "0x8563e", "0xaa2c8"):
            native = copy.deepcopy(self.native)
            owner = next(row["capture"] for row in native["complete_selected_functions"]
                if row["capture"]["rva"] == rva)
            owner["pseudocode"]["lines"].append({"text": "return confirmed_device_generation;"})
            owner["pseudocode_sha256"] = hashlib.sha256("\n".join(
                line["text"] for line in owner["pseudocode"]["lines"]).encode()).hexdigest()
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "complete reviewed native owner differs"):
                host._keypad_temporary_keymap_native_proof(self.raw, native)

    def test_actual_common_observer_and_device_data_slots_cannot_be_neighbors(self):
        import host
        for pointer_rva in (0x2e6a18 + 8, 0x2ed2e8 + 8):
            raw = bytearray(self.raw)
            section = next(row for row in inspect_pe(raw)["sections"]
                if row["rva"] <= pointer_rva < row["rva"] + row["bytes"])
            offset = section["file_offset"] + pointer_rva - section["rva"]
            raw[offset:offset + 8] = struct.pack("<Q", int(self.native["image_base"], 16) + 0x65cc0)
            if pointer_rva == 0x2e6a18 + 8:
                raw[offset:offset + 8] = struct.pack("<Q", int(self.native["image_base"], 16) + 0x656ca)
            with self.subTest(pointer=hex(pointer_rva)), self.assertRaisesRegex(ValueError, "configuration slot differs"):
                host._keypad_temporary_keymap_native_proof(bytes(raw), self.native)

    def test_exact_upstream_recheck_ignores_unrelated_queries_but_rejects_changed_shared_sections(self):
        import host
        from unittest.mock import Mock
        upstream = self.report["upstream_sections"]
        self.assertEqual([row["pointer"] for row in upstream], ["/keypad_mapping_consumers",
            "/keypad_mapping_current_native", "/mapping_input_producer_eligibility"])
        self.assertTrue(all(not row["recaptured"] for row in upstream))
        self.assertNotIn(".analysis/results/host-current/local-audit.json", self.audit.artifacts)
        document = {"shared": {"native": {"publication": "requested"}}, "other": {"progress": 1}}
        dependency = [{"receipt": ".analysis/results/host-current/local-audit.json",
            "pointer": "/shared/native", "canonical_sha256": host._keypad_temporary_keymap_digest(document["shared"]["native"])}]
        fake = Mock()
        fake.read_bytes.side_effect = lambda: json.dumps(document).encode()
        with patch.object(self.audit.a, "resolve", return_value=fake):
            self.assertEqual(host._keypad_temporary_keymap_dependency_changes(dependency), [])
            document["other"]["progress"] = 2
            self.assertEqual(host._keypad_temporary_keymap_dependency_changes(dependency), [])
            document["shared"]["native"]["publication"] = "confirmed"
            self.assertEqual(host._keypad_temporary_keymap_dependency_changes(dependency), [
                ".analysis/results/host-current/local-audit.json#/shared/native"])


class KeypadTwoTapChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_keypad_two_tap_chain(cls.audit)
        document = json.loads((cls.audit.a.ROOT / ".analysis/results/host-current/local-audit.json").read_bytes())
        cls.shared = {name: document[name] for name in host._keypad_two_tap_reviewed_dependencies}

    def body(self, role):
        return self.audit.owners[self.report["selected"][role]]["body"]

    def test_current_mounted_747_slice_keeps_all_runtime_stages_unverified(self):
        report = self.report
        self.assertEqual(report["kind"], "current_keypad_two_tap_chain")
        self.assertEqual(report["product_id"], 747)
        self.assertEqual(report["verified_contract"]["selected_source_owners"], 100)
        self.assertEqual(report["verified_contract"]["actual_ui_module"], 67742)
        self.assertTrue(report["two_tap_producer_slice_complete"])
        self.assertTrue(report["selected_static_semantics_reviewed"])
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key], key)
        for key in ("ui_backend_wiring", "actual_dll_comparison", "runtime_acceptance"):
            self.assertEqual(report[key], "unverified", key)
        self.assertEqual(report["cleanup"]["temporary_files_created"], [])
        self.assertFalse(report["cleanup"]["new_capture_created"])
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_complete_source_owners_reproduce_current_bytes_and_ast_boundaries(self):
        for role, oid in self.report["selected"].items():
            row = self.audit.owners[oid]
            raw = self.audit.read(row["original"])[row["byte_start"]:row["byte_end"]]
            with self.subTest(role=role):
                self.assertEqual(raw.decode(), row["body"])
                self.assertEqual(hashlib.sha256(raw).hexdigest(), row["body_sha256"])
                node = self.audit.tree(row["original"]).root_node.descendant_for_byte_range(
                    row["byte_start"], row["byte_end"] - 1)
                self.assertEqual((node.start_byte, node.end_byte, node.type),
                    (row["byte_start"], row["byte_end"], row["syntax"]))
        self.assertEqual(len(self.report["reused_source_owners"]), 31)

    def test_actual_input_identity_and_nested_stage_are_distinct_from_hypershift(self):
        self.assertIn('"AnalogInput"===n&&i===s', self.body("ui_initial_mapping"))
        root = self.body("ui_two_tap_getTwoTapMapping")
        for field in ("analogInput", "inputType", "inputID", "isHyperShift"):
            self.assertIn("delete t." + field, root)
            self.assertIn("delete e." + field, root)
        self.assertIn("this.keymapping.mapping[1]=this.getDefaultMapping()", root)
        self.assertFalse(self.report["verified_contract"]["secondary_mapping_is_same_as_hypershift_layer"])

    def test_save_syncs_other_layer_but_cancel_only_clears_editor(self):
        save = self.body("ui_editor_saveTwoTapChanges")
        self.assertIn("isHyperShift:!t.isHyperShift", save)
        self.assertIn("e.mapping[0].actuationPoint=D", save)
        self.assertIn("delete l.rapidTrigger", save)
        self.assertIn("n-i<16380", self.body("ui_editor_gap_gate"))
        self.assertIn('await this.props.setActiveButton("")', self.body("ui_two_tap_clear"))
        self.assertIn("this.props.displaySaveAlert(this.clear)", self.body("ui_two_tap_closeMapping"))
        self.assertFalse(self.report["verified_contract"]["ui_save_waits_for_device_confirmation"])
        self.assertFalse(self.report["verified_contract"]["rollback_on_cancel"])

    def test_actual_keypad_default_provider_and_preconfig_do_not_unify_source_defaults(self):
        proof = self.report["verified_contract"]
        self.assertEqual(proof["default_config_actual_module"], 24255)
        self.assertEqual((proof["ui_initial_device_default"], proof["mw_device_info_default"]), (32760, 39312))
        self.assertTrue(proof["ui_default_can_be_updated_by_keymap_preconfig"])
        self.assertIn("e?i(24255).default:i(72233).Ay", self.body("mw_default_provider_selector"))
        self.assertIn("thresholdL:19656,thresholdH:18018", self.body("mw_keypad_actuation_presets"))
        self.assertIn("thresholdL:13104,thresholdH:11466", self.body("mw_keypad_actuation_presets"))
        self.assertIn("specific:{KEY_A:{enabled:!0", self.body("mw_keypad_rapid_presets"))
        self.assertIn("defaultValue:a", self.body("mw_initialize_keymap_defaults"))
        self.assertIn("j.fH.updateConverter(n)", self.body("ui_actuation_preconfig"))

    def test_actual_432_compiler_preserves_trigger_and_scale_branches(self):
        self.assertFalse(self.report["verified_contract"]["inline_compiler_substituted"])
        self.assertIn('type:"triggers"', self.body("compiler_De"))
        self.assertIn('type:"doubleMapping"', self.body("compiler_ve"))
        self.assertIn('digital:{type:"triggers",outputs:{}}', self.body("compiler_ve"))
        self.assertIn('t.type="analogKey",a.type="analogKey"', self.body("compiler_ft"))
        self.assertEqual(self.report["verified_contract"]["software_rapid_two_byte_multiplier"], 1628)
        self.assertEqual(self.report["verified_contract"]["obm_rapid_multiplier"], 1638)
        self.assertIn("p=e.isAnalogGen2?1628:12", self.body("compiler_rapid_trigger"))
        self.assertIn("Math.round(1638*e)", self.body("engine_convertUIRapidTriggerToFW"))

    def test_current_analog_v3_primary_write_does_not_transmit_secondary_threshold(self):
        self.assertIn("e.mapping?e.mapping[0]:e", self.body("obm_analog_v3_projection"))
        self.assertIn("uiThresholdL:0,uiThresholdH:0", self.body("obm_scalar_projection"))
        collector = self.body("collect_primary_analog")
        self.assertIn('n.isHyperShift||"AnalogInput"!==n.inputType', collector)
        self.assertIn("const l=n.mapping[0]", collector)
        self.assertNotIn("n.mapping[1]", collector)
        self.assertFalse(self.report["verified_contract"]["secondary_threshold_transmitted_by_primary_actuation_command"])

    def test_real_headers_record_packing_and_result_consumers_keep_failure_boundaries(self):
        self.assertEqual(self.report["verified_contract"]["actuation_command"], [80, 2, 25])
        self.assertEqual(self.report["verified_contract"]["rapid_command"], [80, 2, 26])
        self.assertIn("2+5*o.length", self.body("protocol_setMultiAnalogKeyActuationPoint"))
        self.assertIn("d[n+3]=a.E4(o[t].thresholdH)", self.body("protocol_setMultiAnalogKeyActuationPoint"))
        self.assertIn("d[n+5]=a.E4(o[t].thresholdL)", self.body("protocol_setMultiAnalogKeyActuationPoint"))
        self.assertIn("2+6*o.length", self.body("protocol_setMultiAnalogKeyRapidTrigger"))
        self.assertIn("for(let e=2;e<t.packetSizeReturned;e+=5)", self.body("protocol_actuation_decoder"))
        self.assertIn("r.status=a.STATUS_SUCCESS", self.body("engine_setActuationPoint"))
        self.assertIn("r.status=a.STATUS_FAIL", self.body("engine_setActuationPoint"))
        rapid = self.body("rapid_runner")
        self.assertLess(rapid.index("setRapidTrigger(n.profileId,n.keys)"),
            rapid.index("setRapidTrigger(n.profileId,n.keys,n.keymapId)"))
        self.assertFalse(self.report["verified_contract"]["independent_post_write_get"])
        self.assertFalse(self.report["verified_contract"]["set_target_keymap_boolean_checked_by_analog_writer"])

    def test_rehashed_changes_to_identity_layers_defaults_cancel_or_command_need_review(self):
        import host
        for role, old, new in (("ui_initial_mapping", '"AnalogInput"===n&&i===s', '"AnalogInput"===n&&!!i===!!s'),
                ("ui_editor_saveTwoTapChanges", "e.mapping[0].actuationPoint=D", "e.mapping[1].actuationPoint=D"),
                ("ui_two_tap_clear", "this.props.toggleMappingPanel(!1)", "this.props.restoreDevice()"),
                ("mw_keypad_default_config", "thresholdL:32760", "thresholdL:39312"),
                ("compiler_rapid_trigger", "p=e.isAnalogGen2?1628:12", "p=e.isAnalogGen2?1638:12"),
                ("collect_primary_analog", "const l=n.mapping[0]", "const l=n.mapping[1]"),
                ("protocol_constant_iPi", "[80,2,25]", "[80,2,13]"),
                ("engine_setActuationPoint", "r.status=a.STATUS_FAIL", "r.status=a.STATUS_SUCCESS")):
            owners = copy.deepcopy(self.audit.owners)
            row = owners[self.report["selected"][role]]
            self.assertIn(old, row["body"], role)
            row["body"] = row["body"].replace(old, new, 1)
            row["body_sha256"] = hashlib.sha256(row["body"].encode()).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "Keypad two-tap"):
                host._keypad_two_tap_source_proof(self.report["selected"], owners)

    def test_missing_or_same_named_inline_compiler_owner_cannot_replace_actual_selection(self):
        import host
        for change in ("missing", "wrong"):
            selected = dict(self.report["selected"])
            if change == "missing":
                selected.pop("compiler_map_actuation")
            else:
                selected["compiler_map_actuation"] = selected["mw_converter_setup"]
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "Keypad two-tap"):
                host._keypad_two_tap_source_proof(selected, self.audit.owners)

    def test_exact_upstream_sections_ignore_siblings_and_reject_changed_shared_semantics(self):
        import host
        sources = {key: value for key, value in self.report["reviewed_inputs"].items() if key != "ui_two_tap"}
        document = copy.deepcopy(self.shared)
        document["unrelated"] = {"progress": 1}
        with patch.object(Path, "read_bytes", side_effect=lambda: json.dumps(document).encode()):
            expected = host._keypad_two_tap_dependencies(self.audit, self.report["source_selections"], sources)
            document["unrelated"]["progress"] = 2
            self.assertEqual(host._keypad_two_tap_dependencies(self.audit, self.report["source_selections"], sources), expected)
            document["keypad_mapping_consumers"]["selected_static_semantics_reviewed"] = False
            with self.assertRaisesRegex(ValueError, "shared section requires renewed review"):
                host._keypad_two_tap_dependencies(self.audit, self.report["source_selections"], sources)
        self.assertNotIn(".analysis/results/host-current/local-audit.json", self.audit.artifacts)
        shared = self.report["shared_trigger_outputs"]
        self.assertTrue(shared["selected_static_semantics_reviewed"])
        self.assertFalse(shared["target_execution"])
        self.assertTrue(all(row in self.report["upstream_sections"] for row in shared["dependencies"]))


class KeyboardBlePowerTransportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import analysis
        import host
        cls.host, cls.analysis = host, analysis
        cls.audit = HostLocalAudit()
        cls.report = host.collect_keyboard_ble_power_transport(cls.audit)
        cls.full_native = copy.deepcopy(cls.report["native"])
        for row in cls.full_native["complete_selected_functions"]:
            row["capture"] = json.loads((analysis.ROOT / row["artifact"]["path"]).read_bytes())

    def test_current_inputs_complete_owners_and_native_edges_remain_separate_from_acceptance(self):
        report = self.report
        self.assertEqual(report["current_host"]["version"], "4.0.896")
        self.assertEqual(report["native"]["input"]["sha256"],
                         "caa1bf44e236d00361af6294bce93433f1a3fcc597a1dc708601321bd69443ad")
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 52)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 10)
        self.assertGreaterEqual(len(report["actual_native_edges"]), 34)
        self.assertTrue(all(row["pseudocode_captured"] and row["input_chunks_verified"]
                            for row in report["native"]["complete_selected_functions"]))
        self.assertEqual(len(report["semantic_dependencies"][0]["byte_identical_current_roles"]), 17)
        self.assertEqual(report["semantic_dependencies"][0]["previous_owner_count"], 35)
        self.assertFalse(report["native"]["retained_successful_supplement"]["selected_by_this_slice"])
        self.assertEqual(report["native"]["retained_successful_supplement"]["failures"], [])
        for name in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(report[name], name)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["temporary_files_created"], [])
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)

    def test_native_selection_preserves_fs_order_and_does_not_choose_noble_by_name(self):
        first = self.host._keyboard_ble_power_transport_first_build
        self.assertEqual(first(["binding.exp", "binding.node", "noble.node"]), "binding.node")
        self.assertEqual(first(["noble.node", "binding.node"]), "noble.node")
        self.assertEqual(first(["ignore.NODE", "arbitrary.node", "binding.node"]), "arbitrary.node")
        self.assertIsNone(first(["binding.lib", "BINDING.NODE"]))
        loader = self.report["current_host"]["loader"]
        self.assertFalse(loader["runtime_environment_or_listing_observed"])
        self.assertIn("runtimeRequire.addon absent", loader["conditions"])
        self.assertIn("NODE_BLE_RZ_PREBUILD unset", loader["conditions"])

    def test_old_host_version_cannot_borrow_current_native_semantics(self):
        release = {**self.host.verified_host_release_receipt(), "version": "4.0.827"}
        with patch.object(self.host, "verified_host_release_receipt", return_value=release), \
                self.assertRaisesRegex(ValueError, "new current release review"):
            self.host._keyboard_ble_power_transport_inputs(self.audit)

    def test_packed_source_must_match_real_current_asar_offset(self):
        rows = copy.deepcopy(self.report["current_host"]["members"])
        row = next(item for item in rows if item["path"].endswith("/strategy/protocol30.js"))
        row["offset"] += 1
        with patch.object(self.host, "current_host_asar_members", return_value=rows), \
                self.assertRaisesRegex(ValueError, "actual ASAR offset"):
            self.host._keyboard_ble_power_transport_inputs(self.audit)

    def test_native_unpacked_state_and_package_mirror_cannot_be_assumed(self):
        rows = copy.deepcopy(self.report["current_host"]["members"])
        row = next(item for item in rows if item["path"].endswith("/binding.node"))
        row["unpacked"] = False
        with patch.object(self.host, "current_host_asar_members", return_value=rows), \
                self.assertRaisesRegex(ValueError, "unpacked native/package bytes"):
            self.host._keyboard_ble_power_transport_inputs(self.audit)

    def test_changed_loader_timer_write_mode_or_host_result_requires_new_owner_review(self):
        for role, before, after in (
                ("gyp_get_first", "files[0]", "files[1]"),
                ("win_binding", "{ NobleWinrt }", "{ Noble }"),
                ("protocol_write", "if(!r)return", "if(clearTimeout(l),!r)return"),
                ("protocol_write", 'return o.off("read",g)', 'o.off("read",g)'),
                ("win_write_data", "!!e.isSupportWriteWithoutResponse||r", "!!e.isSupportWriteWithoutResponse"),
                ("host_send", "await m.write(g,v,o,c,a,i,d)", "{state:!0,data:[]}"),
                ("host_stop", "e.disconnect()", "e.cleanUp()")):
            key = self.report["selected"][role]
            owner = copy.deepcopy(self.audit.owners[key])
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            with self.subTest(role=role, before=before), self.assertRaisesRegex(ValueError, "source boundary differs: " + role):
                self.host._keyboard_ble_power_transport_source_proof(self.report["selected"],
                    {**self.audit.owners, key: owner})

    def test_protocol_cannot_borrow_unchanged_characteristic_owner(self):
        selected = {**self.report["selected"],
                    "protocol_write": self.report["selected"]["reused_characteristic_write"]}
        with self.assertRaisesRegex(ValueError, "source boundary differs: protocol_write"):
            self.host._keyboard_ble_power_transport_source_proof(selected, self.audit.owners)

    def test_async_status_is_not_gatt_success_and_write_event_does_not_confirm_device(self):
        report = self.report
        self.assertFalse(report["write"]["reads_gatt_write_result"])
        self.assertTrue(report["write"]["emits_write_on_async_failure"])
        self.assertTrue(report["write"]["emits_write_when_native_peripheral_missing"])
        self.assertFalse(report["write"]["manager_bool_return_reaches_js"])
        self.assertEqual(report["write"]["gatt_write_options"], {"false": 0, "true": 1})
        changed = copy.deepcopy(self.full_native)
        callback = next(row["capture"] for row in changed["complete_selected_functions"]
                        if row["rva"] == "0x446f0")
        gate = next(line for line in callback["pseudocode"]["lines"] if "a3 == 1" in line["text"])
        gate["text"] = gate["text"].replace("a3 == 1", "a3 == 0")
        with self.assertRaisesRegex(ValueError, "completion/release boundary differs: 0x446f0"):
            self.host._keyboard_ble_power_transport_native_proof(changed)

    def test_cccd_result_token_revocation_and_bulk_drop_have_distinct_boundaries(self):
        report = self.report
        self.assertFalse(report["notify"]["reads_gatt_cccd_result"])
        self.assertIn("bit 0x20", report["notify"]["cccd_enable"])
        self.assertFalse(report["release"]["disconnect_calls_remove_value_changed"])
        self.assertFalse(report["release"]["manager_destructor_calls_remove_value_changed"])
        changed = copy.deepcopy(self.full_native)
        drop = next(row["capture"] for row in changed["complete_selected_functions"] if row["rva"] == "0x325e0")
        drop["pseudocode"]["lines"].insert(2, {"line": -1, "text": "RemoveValueChanged(saved_token);"})
        with self.assertRaisesRegex(ValueError, "completion/release boundary differs: 0x325e0"):
            self.host._keyboard_ble_power_transport_native_proof(changed)

    def test_missing_actual_callback_or_release_edge_fails_even_with_complete_function(self):
        changed = copy.deepcopy(self.full_native)
        callback = next(row["capture"] for row in changed["complete_selected_functions"] if row["rva"] == "0x45460")
        target = int(changed["image_base"], 16) + 0x45920
        for line in callback["assembly"]:
            line["xrefs_from"] = [ref for ref in line.get("xrefs_from", []) if int(ref["to"], 16) != target]
        with self.assertRaisesRegex(ValueError, "actual call/callback edge differs: 0x45460 -> 0x45920"):
            self.host._keyboard_ble_power_transport_native_proof(changed)

    def test_current_protocol_timer_fix_does_not_expand_other_confirmation_or_shutdown_claims(self):
        delta = self.report["protocol30_current_delta"]
        self.assertFalse(delta["clear_write_timer_before_filter"])
        self.assertIn("ignored/non-notify/partial packets retain", delta["write_timer_clear"])
        self.assertIn("read still clears", delta["unchanged_limits"])
        self.assertIn("returns after resolving false", delta["write_catch"])
        self.assertFalse(self.report["release"]["queued_event_drain_is_gatt_operation_drain"])
        self.assertFalse(self.report["release"]["gatt_cancel_or_join_proven"])
        self.assertIn("does not invoke NobleWinrt.cleanUp", self.report["release"]["host_shutdown"])


class CommonDriverInstallConsumerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.host = host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_common_driver_install_consumers(cls.audit)

    def test_current_concrete_action_registry_and_result_boundaries_are_separate(self):
        report = self.report
        self.assertEqual(report["current_host"]["version"], "4.0.896")
        self.assertEqual(report["current_host"]["driver_package"]["declaration"]["resourceName"],
                         "NormalDriver_RzComDriver")
        self.assertEqual(report["concrete_action"]["params"], "/S /LAUNCH")
        self.assertEqual(report["concrete_action"]["js_action"], "simpleLaunchUserAppProcess")
        self.assertFalse(report["concrete_action"]["js_force_elevated"])
        self.assertFalse(report["concrete_action"]["js_pid_drv_ascii_branch"])
        self.assertEqual(report["executable_manifest"]["requested_execution_level"]["level"], "requireAdministrator")
        self.assertEqual(report["driver_info"]["values"], ["ProductVersion", "UninstallPath"])
        self.assertEqual(report["result_consumers"]["uninstall_launch"]["params"], "")
        self.assertFalse(report["result_consumers"]["uninstall_checks_result_or_exit_code"])
        self.assertEqual(len(report["actual_source_calls"]), 13)
        self.assertEqual({name: len(row["complete_selected_functions"]) for name, row in report["native"].items()},
                         {"simple_service.dll": 22, "SysUtilsNative.dll": 5})
        self.assertTrue(all(row["pseudocode_captured"] and row["input_chunks_verified"] and "capture" not in row
                            for native in report["native"].values() for row in native["complete_selected_functions"]))
        for key in ("source_chain_complete", "whole_product_complete", "target_execution", "real_device_operations"):
            self.assertFalse(report[key], key)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(report["temporary_files_created"], [])
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(self.audit.final_snapshot(), [])

    def test_changed_branch_result_skip_or_cancel_cannot_reuse_reviewed_semantics(self):
        for role, before, after in (
                ("Ka", 'a=await q.simpleLaunchUserAppProcessJson', 'a=await q.simpleLaunchUserAppElevatedJson'),
                ("$l", 'a.result===!1', 'a.result!==!0'),
                ("kl", 'i!=null?i:!1', 'i?.result===!0'),
                ("yl", 'await Il(s,r)', 'r.isCancelling||await Il(s,r)'),
                ("driver_info", 'reason:"undefined"', 'reason:"observed not installed"')):
            owners = dict(self.audit.owners)
            key = self.report["selected"][role]
            owner = copy.deepcopy(owners[key])
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            owners[key] = owner
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "source boundary differs: " + re.escape(role)):
                self.host._common_driver_install_consumer_proof(self.report["selected"], owners)

    def test_uninstall_cannot_borrow_the_install_result_consumer(self):
        selected = {**self.report["selected"], "kl": self.report["selected"]["$l"]}
        with self.assertRaisesRegex(ValueError, "source boundary differs: kl"):
            self.host._common_driver_install_consumer_proof(selected, self.audit.owners)

    def test_historical_host_release_is_not_a_current_source_selection(self):
        release = {**self.host.verified_host_release_receipt(), "version": "4.0.827"}
        with patch.object(self.host, "verified_host_release_receipt", return_value=release), \
                self.assertRaisesRegex(ValueError, "new release review"):
            self.host._common_driver_install_consumer_inputs(self.audit)

    def test_new_manifest_elevation_flag_requires_review(self):
        release = copy.deepcopy(self.host.verified_host_release_receipt())
        driver = next(row for row in release["files"]
                      if row.get("declaration", {}).get("resourceName") == "NormalDriver_RzComDriver")
        driver["declaration"]["action"]["saveToDisk"]["runForceElevated"] = True
        with patch.object(self.host, "verified_host_release_receipt", return_value=release), \
                self.assertRaisesRegex(ValueError, "install action requires semantic review"):
            self.host._common_driver_install_consumer_inputs(self.audit)

    def test_real_member_must_match_current_asar_offset_and_packaging(self):
        members = [copy.deepcopy(row["member"]) for row in self.report["current_host"]["host_members"]]
        for field, value, message in (("offset", members[0]["offset"] + 1, "actual ASAR bytes"),
                ("unpacked", True, "package/ASAR ancestry")):
            changed = copy.deepcopy(members)
            changed[0][field] = value
            with self.subTest(field=field), patch.object(self.host, "current_host_asar_members", return_value=changed), \
                    self.assertRaisesRegex(ValueError, message):
                self.host._common_driver_install_consumer_inputs(self.audit)

    def test_similarly_named_native_cannot_replace_the_current_package_member(self):
        rows = [copy.deepcopy(row["input"]) for row in self.report["native"].values()]
        source = self.report["current_host"]["host_members"][0]["official_package"]
        for row in rows:
            row["container"], row["container_sha256"] = source["path"], source["sha256"]
        rows[0]["sha256"] = "0" * 64
        with patch.object(self.host, "current_host_extracted_inputs", return_value=rows), \
                patch.object(self.host, "current_host_asar_members", return_value=[
                    row["member"] for row in self.report["current_host"]["host_members"]]), \
                self.assertRaisesRegex(ValueError, "native identity requires review: simple_service"):
            self.host._common_driver_install_consumer_inputs(self.audit)

    def test_executable_manifest_must_be_the_real_resource_with_the_reviewed_level(self):
        raw = self.audit.raw[self.report["current_host"]["driver_package"]["path"]]
        changed = raw.replace(b'level="requireAdministrator"', b'level="asInvoker           "')
        self.assertNotEqual(raw, changed)
        with self.assertRaisesRegex(ValueError, "elevation manifest requires review"):
            self.host._common_driver_install_consumer_manifest(changed)
        with patch.object(self.host, "pe_resource_entries", return_value=[]), \
                self.assertRaisesRegex(ValueError, "manifest is absent or ambiguous"):
            self.host._common_driver_install_consumer_manifest(raw)

    def test_partial_or_changed_native_result_owner_is_not_promoted(self):
        native = self.report["native"]["simple_service.dll"]
        first = copy.deepcopy(native["complete_selected_functions"][0])
        first["capture"] = {"rva": first["rva"], "assembly": [{}]}
        originals = {name: row["input"] for name, row in self.report["native"].items()}
        for field, value in (("pseudocode_captured", False), ("failures", 1)):
            changed = {**first, field: value}
            with self.subTest(field=field), patch.object(self.audit.families, "verify_native_functions", return_value={
                    "capture_receipt": native["capture_receipt"], "complete_selected_functions": [changed]}), \
                    self.assertRaisesRegex(ValueError, "complete native boundary differs"):
                self.host._common_driver_install_consumer_native(self.audit, originals)


class MixedClrMethodIdentityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.host = host
        cls.root = HostLocalAudit().a.ROOT
        cls.original = {"path": ".analysis/inputs/official-native-current/blobs/0278fc29d16fa8334d22b8d62347f73ba9819139ef02ee10d20f1aa0ec7d1d56/RzIntelOverClockDLL_v1.2.23.0.dll"}
        cls.raw = (cls.root / cls.original["path"]).read_bytes()
        cls.metadata = host._profile_gog_clr(cls.raw)

    def test_mixed_native_and_runtime_bodies_are_never_parsed_as_il(self):
        methods = self.metadata["methods"]
        self.assertEqual(len(methods), 699)
        native = [row for row in methods.values() if row["implementation_kind"] == "native"]
        self.assertEqual(len(native), 46)
        self.assertTrue(all(not row["body_decoded"] and "code" not in row for row in native))
        method = methods[0x06000050]
        self.assertEqual((method["rva"], method["code_size"], method["implementation_kind"]),
                         ("0x41480", 4057, "il"))
        self.assertTrue(method["body_decoded"])
        row = bytes.fromhex(method["metadata_row_hex"])
        self.assertEqual(self.raw.count(row), 1)
        offset = self.raw.index(row)
        for flags, kind in ((1, "native"), (2, "optil"), (3, "runtime")):
            raw = bytearray(self.raw)
            raw[offset + 4:offset + 6] = flags.to_bytes(2, "little")
            changed = self.host._profile_gog_clr(bytes(raw))["methods"][0x06000050]
            self.assertEqual(changed["implementation_kind"], kind)
            self.assertEqual(changed["implementation_flags"], hex(flags))
            self.assertEqual(changed["method_flags"], method["method_flags"])
            self.assertFalse(changed["body_decoded"])
            self.assertNotIn("code", changed)

    def test_corrupt_real_il_still_fails(self):
        method = self.metadata["methods"][0x06000050]
        rva = int(method["rva"], 16)
        section, = [row for row in self.metadata["pe"]["sections"] if row["rva"] <= rva < row["rva"] + row["bytes"]]
        raw = bytearray(self.raw)
        raw[section["file_offset"] + rva - section["rva"]] = 0
        with self.assertRaisesRegex(ValueError, "CLR method header is invalid"):
            self.host._profile_gog_clr(bytes(raw))

    def test_original_il_calls_and_eh_still_verify_in_mixed_assembly(self):
        text = (self.root / ".analysis/results/host-current/managed/0278fc29d16f/il.stdout.txt").read_text(encoding="utf-8")
        selected = {"<Module>": ("RzIntelOverClockLib.UpdateAllTuningControlData",)}
        method, = self.host._profile_gog_il_methods(self.original, text, self.metadata, selected)
        self.assertEqual(method["raw_code_hex"], self.metadata["methods"][0x06000050]["code"].hex())
        self.assertTrue(method["exception_sections"])
        self.assertTrue(any(row["resolved_method"] and row["resolved_method"]["name"] == "Tune" for row in method["calls"]))


class MappingHardwareNotificationChainTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import host
        cls.audit = HostLocalAudit()
        cls.report = host.collect_mapping_hardware_notification_chain(cls.audit)

    def native_captures(self):
        return {"image_base": self.report["native"]["image_base"],
            "complete_selected_functions": [{"capture": json.loads(
                (self.audit.a.ROOT / row["artifact"]["path"]).read_bytes())}
                for row in self.report["native"]["complete_selected_functions"]]}

    def test_selected_usb_provider_reuses_current_evidence_and_keeps_acceptance_separate(self):
        report = self.report
        self.assertEqual(report["kind"], "current_mapping_hardware_notification_chain")
        self.assertEqual(report["provenance"]["release_version"], "4.0.896")
        self.assertEqual(len(report["selected"]), 19)
        self.assertEqual(len(report["native"]["complete_selected_functions"]), 26)
        self.assertEqual(len(report["native"]["verified_indirect_slots"]), 17)
        self.assertEqual(len(report["reused_native_functions"]), 64)
        self.assertEqual(len(report["upstream_sections"]), 4)
        self.assertTrue(all(not row["recaptured"] for row in report["upstream_sections"]))
        self.assertEqual(report["new_native_captures"], 0)
        self.assertEqual(report["new_analysis_sessions"], 0)
        self.assertEqual(report["temporary_files_created"], 0)
        for key in ("source_chain_complete", "whole_product_complete", "target_execution",
                    "real_device_operations", "product_code_modified"):
            self.assertFalse(report[key], key)
        self.assertEqual(report["actual_dll_comparison"], "unverified")
        self.assertEqual(json.loads(json.dumps(report, ensure_ascii=False)), report)
        self.assertEqual(self.audit.final_snapshot(), [])
        self.assertNotIn(".analysis/results/host-current/local-audit.json", self.audit.artifacts)

    def test_feature_io_registration_and_native_join_do_not_claim_notification_or_drain(self):
        proof = self.report["verified_contract"]
        self.assertEqual((proof["selected_product_id"], proof["actual_constructor_type"]), (756, "usb"))
        self.assertEqual(proof["read_capacity"], 100)
        self.assertEqual((proof["selected_report_ids"], proof["selected_event_id"]), ([5, 9], 128))
        self.assertEqual((proof["published_special_report_ids"], proof["unmapped_report_category"]), ([4, 11], 8))
        self.assertEqual(proof["callback_type"], 0)
        for key in ("feature_report_io_is_notification_provider", "category8_strips_prefix",
                "registration_result_consumes_callback_set_result", "register_is_physically_confirmed_notification",
                "unregister_clears_callback", "unregister_cancels_io", "unregister_proves_callback_drain",
                "whole_connection_generation_drain_proven", "actual_notification128_observed"):
            self.assertFalse(proof[key], key)

    def test_subscriber_order_callback_result_and_url_filter_cannot_be_rewritten(self):
        import host
        for role, before, after in (
                ("registerHardwareEvent", 'await this.setHardwareEventCallback(t,e)',
                    'o=await this.setHardwareEventCallback(t,e)'),
                ("unregisterHardwareEvent", 'g.hardwareEventList=g.hardwareEventList.filter(e=>e!==a)',
                    'g.hardwareEventList=[]'),
                ("createCbHardwareEvent", 'r?.hardwareEventList.includes(e.getURL())', 'true'),
                ("isHardwareEventRegistered", 'r.result=!1,g.hardwareEventList=[]', 'r.result=!0')):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, after)
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source owner differs: " + role):
                host._mapping_hardware_notification_verify(self.native_captures(), owners, self.report["selected"])

    def test_service_failed_unsubscribe_and_real_notification_parser_are_preserved(self):
        import host
        for role, before in (("hardware_service", 'this.deviceMap.delete(e.deviceContainerId)'),
                ("hardware_lifecycle", 's.A.off("hardwareevent",J)'),
                ("product_report_ids", '11:"analogKeyboardV2ReportID"')):
            owners = copy.deepcopy(self.audit.owners)
            owner = owners[self.report["selected"][role]]
            self.assertIn(before, owner["body"])
            owner["body"] = owner["body"].replace(before, 'undefined')
            owner["body_sha256"] = hashlib.sha256(owner["body"].encode()).hexdigest()
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete source owner differs: " + role):
                host._mapping_hardware_notification_verify(self.native_captures(), owners, self.report["selected"])

    def test_actual_engine_slot_interval_zero_and_hid_gate_have_machine_code_evidence(self):
        import host
        for rva, replacement in (("0x2a170", "488b8010010000"),
                ("0x2a17a", "41b801000000"), ("0x784c", "488b4040"),
                ("0x52b2c", "80bfb000000000")):
            native = self.native_captures()
            instruction = next(line for row in native["complete_selected_functions"]
                for line in row["capture"]["assembly"] if line["rva"] == rva)
            instruction["bytes"] = replacement
            with self.subTest(rva=rva), self.assertRaisesRegex(ValueError, "ABI or HID gate differs: " + rva):
                host._mapping_hardware_notification_verify(native, self.audit.owners, self.report["selected"])

    def test_native_query_and_cleanup_cannot_be_replaced_with_success_or_empty_body(self):
        import host
        for rva, role in (("0x6b4f0", "registered_flag_query"), ("0xa896c", "usb_destructor")):
            native = self.native_captures()
            function = next(row["capture"] for row in native["complete_selected_functions"]
                if row["capture"]["rva"] == rva)
            function["pseudocode"]["lines"] = [{"text": "return 1;"}]
            with self.subTest(role=role), self.assertRaisesRegex(ValueError, "complete native owner differs: " + role):
                host._mapping_hardware_notification_verify(native, self.audit.owners, self.report["selected"])

    def test_usb_observer_and_input_interfaces_must_resolve_from_actual_pe(self):
        with self.assertRaisesRegex(ValueError, "Native virtual slot target differs"):
            self.audit.families.verify_native_functions(self.report["native"]["input"],
                {0x6b4f0: "query"}, [{"role": "query", "vtable_rva": 0x2e8e18,
                    "slot_byte_offset": 56, "target_rva": 0x6b4f0}])

    def test_changed_shared_section_or_incomplete_native_blocks_publication(self):
        import host
        with patch.object(host, "_otfm_macro_encoding_digest", return_value="0" * 64):
            with self.assertRaisesRegex(ValueError, "upstream requires review: mapping_external_input_routing"):
                host._mapping_hardware_notification_dependencies(HostLocalAudit())
        audit = HostLocalAudit()
        with patch.object(audit.families, "verify_native_functions", return_value={
                "complete_selected_functions": [{"role": "register_export", "pseudocode_captured": False,
                    "failures": 0}]}):
            with self.assertRaisesRegex(ValueError, "native function remains incomplete: register_export"):
                host.collect_mapping_hardware_notification_chain(audit)


if __name__ == "__main__":
    unittest.main()
