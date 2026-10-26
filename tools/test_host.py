"""Pure static PE and source-contract tests; no target is loaded or executed."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import struct
import subprocess
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from host import (HostLocalAudit, add_commands, audit_host_local,
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
                  collect_mapping_descriptor_gamemode)


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


if __name__ == "__main__":
    unittest.main()
