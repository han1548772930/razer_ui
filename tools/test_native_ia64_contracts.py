"""Independent IA64 byte, unwind and manual-review boundary checks."""
import copy
import hashlib
import json
from pathlib import Path
import struct
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import analysis
import native_adjudication as adjudication
import native_ia64_contracts as contracts
from host import inspect_pe, native_exception_region_coverage


class Ia64StaticContractTests(unittest.TestCase):
    def image(self):
        raw = bytearray(0xc00)
        raw[:2] = b"MZ"
        struct.pack_into("<I", raw, 0x3c, 0x80)
        raw[0x80:0x84] = b"PE\0\0"
        struct.pack_into("<HHIIIHH", raw, 0x84, 0x200, 2, 0, 0, 0, 240, 0x2022)
        struct.pack_into("<H", raw, 0x98, 0x20b)
        struct.pack_into("<I", raw, 0x98 + 16, 0x2000)
        struct.pack_into("<Q", raw, 0x98 + 24, 0x10000)
        struct.pack_into("<II", raw, 0x98 + 32, 0x1000, 0x200)
        struct.pack_into("<II", raw, 0x98 + 56, 0x3000, 0x200)
        struct.pack_into("<I", raw, 0x98 + 108, 16)
        struct.pack_into("<II", raw, 0x98 + 112 + 3 * 8, 0x2010, 12)
        struct.pack_into("<II", raw, 0x98 + 112 + 8 * 8, 0x4000, 0)
        for at, name, virtual, rva, size, file, flags in (
                (0x188, b".text", 48, 0x1000, 0x400, 0x200, 0x60000020),
                (0x1b0, b".data", 0x600, 0x2000, 0x600, 0x600, 0xc0000040)):
            struct.pack_into("<8sIIII", raw, at, name, virtual, rva, size, file)
            struct.pack_into("<I", raw, at + 36, flags)
        bundle = bytes.fromhex("0c000000010000000002000000000400")
        raw[0x200:0x230] = bundle * 3
        struct.pack_into("<QQ", raw, 0x600, 0x11000, 0x14000)
        struct.pack_into("<III", raw, 0x610, 0x1000, 0x1030, 0x2020)
        raw[0x620:0x630] = bytes.fromhex("03000000010000000900000000000000")
        return bytes(raw)

    def fixture(self, root):
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            data = value if isinstance(value, bytes) else (json.dumps(value, ensure_ascii=False) + "\n").encode()
            path.write_bytes(data)
            return {"path": path.relative_to(root).as_posix(), "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}

        raw = self.image()
        input_ref = write(root / ".analysis/inputs/synthetic.sys", raw)
        parent = write(root / ".analysis/inputs/container.dll", b"prefix!!" + raw)
        spec = write(root / ".analysis/inputs/spec.pdf", b"Independent normative fixture")
        source_db = write(root / ".work/native/source.i64", b"Inert static database fixture")
        directory = root / ".analysis/results/native" / input_ref["sha256"]
        output = directory / "exception-regions/0123456789abcdef"
        image = contracts.Image(raw)
        table = inspect_pe(raw)["exception_directory"]["regions"][0]
        assembly = []
        for begin in (0x1000, 0x1010):
            for offset, size in ((0, 6), (6, 6), (12, 4)):
                at = begin + offset
                edge = {"from": hex(at + 0x10000), "to": "0x11000", "type": 19, "code": True, "user": False}
                text = "br.few loc_11000;;" if at == 0x100c else "nop.i 0"
                assembly.append({"address": hex(at + 0x10000), "rva": hex(at), "size": size,
                    "bytes": image.read(at, size).hex(), "text": text, "idb_disassembly": text,
                    "xrefs_from": [edge] if at == 0x100c else [], "xrefs_to": []})
        chunk = {"start": "0x11000", "end": "0x11010", "raw_hex": raw[0x200:0x210].hex(),
                 "sha256": hashlib.sha256(raw[0x200:0x210]).hexdigest()}
        function = {"rva": "0x1000", "address": "0x11000", "name": "synthetic_entry", "end": "0x11010",
                    "flags": 0, "chunks": [chunk], "assembly": assembly[:3], "xrefs_to_entry": [], "pseudocode": None}
        function_ref = write(directory / "functions/1000.json", function)
        function_index = {**function_ref, "path": "functions/1000.json", "rva": "0x1000", "address": "0x11000",
                          "assembly_captured": True, "pseudocode_captured": False}
        base = {"schema_version": 1, "input": input_ref["path"], "input_sha256": input_ref["sha256"],
                "input_bytes": len(raw), "image_base": "0x10000", "capture_complete": True,
                "hexrays_available": False, "all_pseudocode_captured": False,
                "target_execution": False, "real_device_operations": False, "functions": [function_index],
                "failures": [{"stage": "1000:hexrays", "error": "original unavailable processor plugin"}]}
        base_ref = write(directory / "capture.json", base)
        imports_ref = write(directory / "imports.json", [])
        alignment = {"start_rva": "0x1020", "end_rva": "0x1030", "bytes": 16,
                     "raw_hex": raw[0x220:0x230].hex(), "sha256": hashlib.sha256(raw[0x220:0x230]).hexdigest(),
                     "classification": "existing_idb_alignment_item", "instruction_semantics_inferred": False}
        region = {**table, "raw_hex": raw[0x200:0x230].hex(), "assembly": assembly, "alignment_items": [alignment],
                  "undecoded_ranges": [], "decoded_bytes": 32, "alignment_bytes": 16,
                  "undecoded_bytes": 0, "decode_complete": False, "coverage_complete": True}
        region_ref = write(output / "regions/1000.json", region)
        selection = {"directory": inspect_pe(raw)["exception_directory"], "captured_chunk_coverage":
                     native_exception_region_coverage(inspect_pe(raw)["exception_directory"], [function], "0x10000")}
        snapshot = [{"address": "0x11000", "chunks": [["0x11000", "0x11010"]]}]
        snapshot_hash = hashlib.sha256(json.dumps(snapshot, sort_keys=True).encode()).hexdigest()
        capture = {"input": input_ref["path"], "input_sha256": input_ref["sha256"], "image_base": "0x10000",
            "capture_complete": True, "function_chunks_unchanged": True, "source_database_unchanged": True,
            "base_capture_unchanged": True, "functions_created": False, "decompilation_attempted": False,
            "function_discovery_complete": False, "failures": [], "automatic_analysis_enabled": False,
            "target_execution": False, "real_device_operations": False, "base_capture": base_ref,
            "source_database": source_db, "exception_regions": selection, "tool_sources": {},
            "function_chunks_snapshot": {"functions": 1, "chunks": 1, "sha256_before": snapshot_hash,
                                         "sha256_after": snapshot_hash, "rows": snapshot},
            "instruction_decode_complete": False,
            "regions": [{**region_ref, "path": "regions/1000.json", "decoded_bytes": 32, "alignment_bytes": 16,
                "undecoded_bytes": 0, "decode_complete": False, "coverage_complete": True}]}
        capture_ref = write(output / "capture.json", capture)
        plan_ref = write(output / "plan.json", {key: copy.deepcopy(capture[key]) for key in
            ("base_capture", "source_database", "exception_regions", "tool_sources", "target_execution", "real_device_operations")})
        session_ref = write(output / "session.json", {"passed": True, "exit_code": 0, "finished_at_utc": "2026-01-01T00:00:00Z",
            "target_execution": False, "real_device_operations": False, "tool_sources": {}})
        records = [{"offset": 0, "format": "R1", "record": "prologue", "instruction_slots": 9, "start_slot": 0,
                    "end_slot": 9, "raw_hex": "09"}] + [{"offset": i, "format": "R1", "record": "prologue",
                    "instruction_slots": 0, "start_slot": 9, "end_slot": 9, "zero_padding_noop": True, "raw_hex": "00"}
                    for i in range(1, 8)]
        boundary = {"entry_rva": "0x1010", "range_end_rva": "0x1020", "unique_captured_instruction_addresses_scanned": 6,
                    "external_inbound_captured_xrefs": [], "all36_imm64_and22_dir64_relocations_checked": True,
                    "pe_export_directory_absent": True, "pe_tls_directory_absent": True, "literal_pointer_to_entry_present": False}
        manual = {"schema_version": 1, "kind": "ia64_asrdrv_full_static_function_contract_review", "input": input_ref,
            "container": {**parent, "embedded_offset": 8, "embedded_bytes": len(raw), "embedded_bytes_independently_matched": True},
            "scope": {"source_or_idb_mutated": False, "target_execution": False, "real_device_operations": False,
                "semantics_complete": False, "static_function_contracts_complete": True,
                "static_unwind_descriptor_semantics_complete": True, "all_executable_virtual_bytes_accounted_for": True,
                "existing_functions": 1, "existing_chunks": 1, "function_contracts_reviewed": 1, "exception_ranges": 1,
                "additional_forward_copy_body": True},
            "architecture": {"machine": "0x200", "format": "PE32+", "image_base": "0x10000", "gp": "0x14000",
                "entrypoint_descriptor_rva": "0x2000", "entry_code_rva": "0x1000", "instruction_semantics_reference": spec},
            "sources": [base_ref, imports_ref, capture_ref, plan_ref, session_ref],
            "functions": [{"rva": "0x1000", "name": "synthetic_entry", "evidence": function_ref, "role": "pure_logic",
                "normal_path_contract": "Manual fixture preserves original static semantics.", "chunks": [
                    {"begin_rva": "0x1000", "end_rva": "0x1010", "sha256": chunk["sha256"]}],
                "captured_instruction_rows": 3, "calls_and_import_tails": [],
                "hexrays_pseudocode_available": False, "provider_execution_verified": False}],
            "exception_region_ownership": [{"begin_rva": "0x1000", "end_rva": "0x1030", "table_index": 0,
                "evidence": region_ref, "existing_chunk_owners": [
                    {"function_rva": "0x1000", "begin_rva": "0x1000", "end_rva": "0x1010"}],
                "unowned_instruction_rows": 3, "decoded_bytes": 32, "alignment_bytes": 16, "undecoded_bytes": 0,
                "coverage_complete": True, "unwind_rva": "0x2020", "unwind_header_hex": "0300000001000000",
                "original_capture_unwind_semantics_verified": False, "static_unwind_descriptor_semantics_verified": True}],
            "regions_outside_existing_chunks": [{"region_begin_rva": "0x1000", "begin_rva": "0x1010", "end_rva": "0x1030",
                "instruction_rows": 3, "evidence": region_ref, "classification": "forward_copy_and_retained_alignment",
                "no_function_ownership_changed": True}],
            "unwind_review": {"portable_descriptor_spec": spec, "unknown_opcode_count": 0,
                "static_descriptor_semantics_complete": True, "matching_windows_ia64_unwind_execution_verified": False,
                "unique_streams": [{"unwind_rva": "0x2020", "header_raw_hex": "0300000001000000", "version": 3,
                    "flags": 0, "descriptor_doublewords": 1, "descriptor_bytes": 8,
                    "descriptor_sha256": hashlib.sha256(bytes.fromhex("0900000000000000")).hexdigest(),
                    "region_begins": ["0x1000"], "records": records, "interpreted_slots": 9, "all_opcodes_decoded": True}],
                "opcode_counts_unique_streams": {"R1": 8}, "saved_action_instruction_checks": [],
                "epilogue_sp_instruction_checks": [], "inherited_recovery_frames": [], "floating_spill_instruction_checks": [],
                "cookie_handlers": [], "runtime_ranges": 1, "unique_stream_count": 1, "all_range_slot_lengths_match": True},
            "descriptors": [{"descriptor_rva": "0x2000", "raw_hex": raw[0x600:0x610].hex(), "code_rva": "0x1000", "gp": "0x14000"}],
            "fixed_tables": {"access_rights": [], "sid_aliases": []},
            "independent_pe_review": {"imports": [], "import_modules": [], "all_relocations": [], "relocation_kind_counts": {},
                "relocation_decode_reference": spec, "literal_ranges": [], "executable_section_ranges_outside_captures": [],
                "all_executable_virtual_bytes_accounted_for": True, "unclassified_nonzero_bytes": 0,
                "forward_copy_entry_boundary": boundary},
            "additional_forward_copy_contract": {"begin_rva": "0x1010", "end_rva": "0x1020", "evidence": region_ref,
                "normal_entry_bypasses": True, "contract": "Separate retained forward-copy body manual contract.",
                "entry_branch_rva": "0x100c", "entry_branch_target": "0x1000", "entry_boundary": copy.deepcopy(boundary)},
            "validation": {"failures": [], "matching_windows_ia64_execution_verified": False,
                "exception_capture_instruction_decode_complete": False, "decoded_exception_bytes": 32,
                "retained_original_alignment_bytes": 16, "no_independent_exception_entry": [], "unwind_range_slot_checks": 1,
                "unwind_save_instruction_checks": 0, "unwind_epilogue_checks": 0, "inherited_recovery_frame_checks": 0,
                "unknown_unwind_opcodes": 0, "descriptors_verified": 1, "fixed_access_rows_verified": 0,
                "fixed_sid_rows_verified": 0, "independent_imports_verified": 0, "function_chunks": 1,
                "function_instruction_rows_verified": 3},
            "cleanup": {"source_database": source_db}, "unverified_boundaries": [{"area": "hardware", "fact": "unverified"}]}
        ref = write(root / ".analysis/results/review.json", manual)
        return SimpleNamespace(review=manual, record=ref, write=write, raw=raw, function=function, function_ref=function_ref,
                               region=region, region_ref=region_ref, capture=capture, capture_ref=capture_ref, base=base,
                               base_ref=base_ref, image=image)

    def test_manual_contract_proof_preserves_original_failures_and_all_independent_dimensions(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            proof = contracts.verify(analysis, f.record)
            self.assertEqual(set(proof["complete_static_contracts"]), {"0x1000"})
            result = proof["complete_static_contracts"]["0x1000"]
            self.assertEqual(result["semantic_source"], "manual_review")
            self.assertEqual(result["function"], f.function_ref)
            self.assertEqual(result["base_capture"], f.base_ref)
            self.assertEqual(proof["original_decompiler_failures"], f.base["failures"])
            self.assertEqual(proof["validation"]["retained_original_alignment_bytes"], 16)
            for key in ("hexrays_pseudocode_available", "provider_execution_verified", "rust_implementation_verified",
                        "ui_backend_wiring_verified", "runtime_acceptance_verified", "target_execution"):
                self.assertIs(result[key], False)
            self.assertIs(proof["function_discovery_complete"], False)
            self.assertEqual(proof["official_dll_comparison"], "unverified")

    def test_review_mutations_cannot_drop_functions_bytes_ranges_slots_imports_or_static_boundaries(self):
        mutations = {
            "missing_function": lambda r: r["functions"].clear(),
            "duplicate_function": lambda r: r["functions"].append(copy.deepcopy(r["functions"][0])),
            "source_hash": lambda r: r["functions"][0]["evidence"].update(sha256="0" * 64),
            "missing_contract": lambda r: r["functions"][0].update(normal_path_contract=""),
            "chunk": lambda r: r["functions"][0]["chunks"][0].update(end_rva="0x1020"),
            "function_count": lambda r: r["functions"][0].update(captured_instruction_rows=2),
            "missing_range": lambda r: r["exception_region_ownership"].clear(),
            "range_owner": lambda r: r["exception_region_ownership"][0]["existing_chunk_owners"].clear(),
            "range_slots": lambda r: r["unwind_review"]["unique_streams"][0].update(interpreted_slots=8),
            "unwind_byte": lambda r: r["unwind_review"]["unique_streams"][0]["records"][0].update(raw_hex="08"),
            "unwind_meaning": lambda r: r["unwind_review"]["unique_streams"][0]["records"][0].update(instruction_slots=8),
            "unwind_missing": lambda r: r["unwind_review"]["unique_streams"].clear(),
            "align_promoted": lambda r: r["validation"].update(exception_capture_instruction_decode_complete=True),
            "align_count": lambda r: r["validation"].update(retained_original_alignment_bytes=0),
            "gp": lambda r: r["architecture"].update(gp="0x15000"),
            "descriptor": lambda r: r["descriptors"][0].update(code_rva="0x1010"),
            "import": lambda r: r["independent_pe_review"]["imports"].append({"iat_rva": "0x2000"}),
            "relocation": lambda r: r["independent_pe_review"]["all_relocations"].append({"kind": 10}),
            "copy_identity": lambda r: r["additional_forward_copy_contract"]["evidence"].update(sha256="0" * 64),
            "copy_branch": lambda r: r["additional_forward_copy_contract"].update(entry_branch_target="0x1010"),
            "copy_count": lambda r: r["additional_forward_copy_contract"]["entry_boundary"].update(unique_captured_instruction_addresses_scanned=5),
            "scope": lambda r: r["scope"].update(existing_functions=2),
            "runtime": lambda r: r["scope"].update(target_execution=True),
            "fake_cfunc": lambda r: r["functions"][0].update(hexrays_pseudocode_available=True),
            "fake_provider": lambda r: r["functions"][0].update(provider_execution_verified=True),
            "missing_validation": lambda r: r["validation"].pop("function_chunks"),
        }
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for name, mutate in mutations.items():
                with self.subTest(name=name):
                    review = copy.deepcopy(f.review)
                    mutate(review)
                    record = f.write(Path(directory) / ".analysis/results/mutated-review.json", review)
                    with self.assertRaises(ValueError):
                        contracts.verify(analysis, record)

    def test_original_artifact_identity_is_rechecked_at_end(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            original = contracts._copy_body
            def changed(*args):
                result = original(*args)
                path = Path(directory) / f.function_ref["path"]
                path.write_bytes(path.read_bytes() + b" ")
                return result
            with patch.object(contracts, "_copy_body", changed), self.assertRaisesRegex(ValueError, "evidence changed"):
                contracts.verify(analysis, f.record)

    def test_unwind_decoder_uses_original_record_fields_and_long_slot_lengths(self):
        records, slots = contracts.decode_unwind(bytes.fromhex("08e600b123e402b0a2e00301614681c00300"))
        self.assertEqual(slots, 78)
        self.assertEqual(records[1], {"offset": 1, "format": "P7", "record": "pfs_when", "t": 0, "raw_hex": "e600"})
        self.assertEqual(records[2]["register"], 35)
        self.assertEqual(records[5]["stack_bytes"], 16)
        self.assertEqual(records[6]["instruction_slots"], 70)
        self.assertEqual(records[8]["t_from_region_end"], 3)
        self.assertTrue(records[-1]["zero_padding_noop"])
        self.assertEqual(contracts.slot_rva(0x1000, 7), 0x1026)
        self.assertEqual(contracts.decode_unwind(bytes.fromhex("00b1236103"))[0][0]["instruction_slots"], 0)

    def test_unwind_unknown_truncated_overflow_and_outside_epilogue_are_rejected(self):
        for code in ("ff", "01b1", "01e080", "608080808080808080808000", "23c003", "03b4ff", "03e301"):
            with self.subTest(code=code), self.assertRaises(ValueError):
                contracts.decode_unwind(bytes.fromhex(code))

    def test_bundle_slot_and_containing_original_chunk_are_strict(self):
        image = contracts.Image(self.image())
        row = {"address": "0x11000", "rva": "0x1000", "bytes": image.read(0x1000, 6).hex(),
               "xrefs_from": [], "xrefs_to": []}
        contracts.instruction_rows(image, [row], [(0x1000, 0x1010)])
        for mutate in (lambda r: r.update(address="0x11006"), lambda r: r.update(rva="0x1001"),
                       lambda r: r.update(bytes="00"), lambda r: r.update(size=12),
                       lambda r: r.update(xrefs_from=[{"from": "0x11006", "to": "0x11000", "type": 19, "code": True}])):
            changed = copy.deepcopy(row)
            mutate(changed)
            with self.assertRaises(ValueError):
                contracts.instruction_rows(image, [changed], [(0x1000, 0x1010)])

    def test_pe_reads_reject_nonexecutables_and_overlapping_file_backing(self):
        image = contracts.Image(self.image())
        with self.assertRaises(ValueError):
            image.read(0x2000, 16, executable=True)
        with self.assertRaises(ValueError):
            image.read(0x13ff, 2)
        image.sections.append(copy.deepcopy(image.sections[0]))
        with self.assertRaises(ValueError):
            image.read(0x1000, 16)

    def test_imm64_bit_layout_has_independent_sign_and_split_field_expectations(self):
        self.assertEqual(contracts.imm64(bytes(16)), 0)
        bundle = bytearray(16)
        struct.pack_into("<I", bundle, 12, 1 << 27)
        self.assertEqual(contracts.imm64(bundle), 1 << 63)
        struct.pack_into("<I", bundle, 12, (1 << 27) | 0x7f << 4)
        struct.pack_into("<I", bundle, 4, 1 << 24)
        self.assertEqual(contracts.imm64(bundle), (1 << 63) | (1 << 32) | 0x7f)

    def test_imports_recompute_lookup_iat_hint_module_and_terminator_from_original_pe(self):
        raw = bytearray(self.image())
        struct.pack_into("<II", raw, 0x98 + 112 + 8, 0x2040, 40)
        struct.pack_into("<IIIII", raw, 0x640, 0x2080, 0, 0, 0x20e0, 0x20a0)
        struct.pack_into("<QQ", raw, 0x680, 0x20c0, 0)
        struct.pack_into("<QQ", raw, 0x6a0, 0x20c0, 0)
        struct.pack_into("<H", raw, 0x6c0, 37)
        raw[0x6c2:0x6c7] = b"Test\0"
        raw[0x6e0:0x6e7] = b"OS.sys\0"
        image = contracts.Image(bytes(raw))
        self.assertEqual(contracts.imports(image), ([{"module": "OS.sys", "name": "Test", "iat_rva": "0x20a0",
            "lookup_entry_rva": "0x2080", "hint_name_rva": "0x20c0", "hint": 37}],
            [{"name": "OS.sys", "descriptor_rva": "0x2040", "imports": 1}]))
        changed = bytearray(raw)
        struct.pack_into("<Q", changed, 0x6a0, 0x20c8)
        with self.assertRaisesRegex(ValueError, "IAT"):
            contracts.imports(contracts.Image(bytes(changed)))
        changed = bytearray(raw)
        struct.pack_into("<Q", changed, 0x680, (1 << 63) | 7)
        struct.pack_into("<Q", changed, 0x6a0, (1 << 63) | 7)
        with self.assertRaisesRegex(ValueError, "ordinal"):
            contracts.imports(contracts.Image(bytes(changed)))

    def test_relocations_recompute_imm64_dir64_and_keep_absolute_padding_count(self):
        raw = bytearray(self.image())
        raw[0x200:0x210] = bytes(16)
        struct.pack_into("<II", raw, 0x98 + 112 + 5 * 8, 0x2100, 22)
        struct.pack_into("<IIHH", raw, 0x700, 0x1000, 12, 0x9001, 0)
        struct.pack_into("<IIH", raw, 0x70c, 0x2000, 10, 0xa000)
        expected = ([{"kind": 9, "fixup_rva": "0x1001", "original_value": "0x0"},
                     {"kind": 10, "fixup_rva": "0x2000", "original_value": "0x11000"}],
                    {"0x9": 1, "0x0": 1, "0xa": 1})
        self.assertEqual(contracts.relocations(contracts.Image(bytes(raw))), expected)
        for field, value in ((0x704, 11), (0x708, 0x8001)):
            changed = bytearray(raw)
            struct.pack_into("<I" if field == 0x704 else "<H", changed, field, value)
            with self.assertRaises(ValueError):
                contracts.relocations(contracts.Image(bytes(changed)))


if __name__ == "__main__":
    unittest.main()
