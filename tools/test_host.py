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
                  help_system_mount_layout, help_system_native_declaration, help_system_report_constants,
                  inspect_host, inspect_pe, pe_cursor_groups, pe_resource_entries,
                  setup_factory_static_extraction, native_ascii_pointer_table,
                  receiver_hardware_dispatch_proof, native_pseudocode_failures, lighting_catalog_projection)


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
