"""Resolved indirect-call evidence and SDK propagation failure boundaries."""
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
import native_adjudication as evidence
import native_callsites
import test_evidence


class NativeCallsiteTests(unittest.TestCase):
    def fixture(self, root, call=b"\xff\x12"):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        raw[0x200:0x200 + len(call) + 1] = call + b"\xc3"
        for offset in (0x210, 0x220, 0x230):
            raw[offset] = 0xc3
        struct.pack_into("<Q", raw, 0x280, 0x140001010)
        raw = bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        result = root / ".analysis/results/native" / digest
        original = root / ".analysis/inputs/native/resolved.dll"
        original.parent.mkdir(parents=True)
        original.write_bytes(raw)

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)

        functions = []
        for rva, code in ((0x1000, call + b"\xc3"), (0x1010, b"\xc3"),
                          (0x1020, b"\xc3"), (0x1030, b"\xc3")):
            first = 0x140000000 + rva
            assembly = [{"address": hex(first), "rva": hex(rva), "bytes": code.hex(), "text": "ret"}]
            if rva == 0x1000:
                assembly = [{"address": hex(first), "rva": hex(rva), "bytes": call.hex(),
                             "text": "call    qword ptr [rdx]"},
                            {"address": hex(first + len(call)), "rva": hex(rva + len(call)),
                             "bytes": "c3", "text": "ret"}]
            body = {"address": hex(first), "rva": hex(rva), "end": hex(first + len(code)),
                    "chunks": [{"start": hex(first), "end": hex(first + len(code)), "raw_hex": code.hex()}],
                    "assembly": assembly, "xrefs_to_entry": [], "pseudocode": None}
            record = write(result / "functions" / (hex(rva)[2:] + ".json"), body)
            functions.append({**record, "path": "functions/" + hex(rva)[2:] + ".json", "rva": hex(rva),
                              "address": hex(first), "assembly_captured": True, "pseudocode_captured": False})
        capture = {"input": analysis.rel(original), "input_sha256": digest, "input_bytes": len(raw),
                   "image_base": "0x140000000", "capture_complete": True, "functions": functions,
                   "failures": [{"stage": "1000:hexrays", "error": "original unknown indirect-call cleanup"}]}
        write(result / "capture.json", capture)
        review = {"schema_version": 1, "review_kind": "native_resolved_vtable_callsite_type",
                  "source_input": evidence.identity(analysis, original),
                  "base_capture": evidence.identity(analysis, result / "capture.json"),
                  "owner_rva": "0x1000", "callsite_rva": "0x1000", "callee_rva": "0x1010",
                  "vtable_rva": "0x1080", "vtable_count": 1, "vtable_index": 0,
                  "declaration": "void __fastcall resolved(void *context);",
                  "assembly_evidence": "Independent synthetic constructor/lifetime and callee ABI review",
                  "object_binding_evidence": [evidence.identity(analysis, result / "functions/1020.json"),
                                              evidence.identity(analysis, result / "functions/1030.json")],
                  "target_execution": False, "raw_artifacts_modified": False}
        review_path = root / ".analysis/results/callsite-review.json"
        write(review_path, {"calls": [review]})
        return SimpleNamespace(raw=raw, result=result, capture=capture, review=review, write=write,
                               review_path=review_path, specification=analysis.rel(review_path) + "#/calls/0")

    def select(self, f):
        return analysis.native_function_selection(f.raw, f.capture, [0x1000], callsite_reviews=[f.specification])

    def test_review_binds_original_call_vtable_target_and_lifetime_functions(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            before = [p.read_bytes() for p in (f.result / "capture.json", f.result / "functions/1000.json")]
            selected = self.select(f)["callsite_type_recoveries"][0]
            self.assertEqual(selected["instruction"]["bytes"], "ff12")
            self.assertEqual(selected["callee_rva"], selected["vtable"]["targets"][0]["rva"])
            self.assertEqual(selected["owner_function"]["sha256"], f.capture["functions"][0]["sha256"])
            self.assertEqual(selected["object_binding_evidence"], f.review["object_binding_evidence"])
            self.assertEqual(before, [p.read_bytes() for p in (f.result / "capture.json", f.result / "functions/1000.json")])

    def test_wrong_scope_unresolved_target_and_changed_bindings_are_rejected(self):
        for issue in ("input", "capture", "callee", "index", "declaration", "reason", "bindings",
                      "duplicate_binding", "changed_binding", "owner", "instruction", "execute", "extra"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "input": f.review["source_input"]["sha256"] = "0" * 64
                elif issue == "capture": f.review["base_capture"]["sha256"] = "0" * 64
                elif issue == "callee": f.review["callee_rva"] = "0x1020"
                elif issue == "index": f.review["vtable_index"] = True
                elif issue == "declaration": f.review["declaration"] = ""
                elif issue == "reason": f.review["assembly_evidence"] = ""
                elif issue == "bindings": f.review["object_binding_evidence"] = []
                elif issue == "duplicate_binding": f.review["object_binding_evidence"][1] = f.review["object_binding_evidence"][0]
                elif issue == "changed_binding": (f.result / "functions/1020.json").write_bytes(b"changed")
                elif issue == "owner": f.review["owner_rva"] = "0x1010"
                elif issue == "instruction": f.review["callsite_rva"] = "0x1002"
                elif issue == "execute": f.review["target_execution"] = True
                else: f.review["inferred_target"] = "0x1010"
                f.write(f.review_path, {"calls": [f.review]})
                with self.assertRaises(ValueError): self.select(f)

    def test_direct_jump_far_call_and_non_call_bytes_cannot_receive_a_type(self):
        for call in (b"\xff\x22", b"\xff\x1a", b"\x90\x90", b"\xe8\0\0\0\0"):
            with self.subTest(code=call.hex()), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), call)
                with self.assertRaisesRegex(ValueError, "indirect CALL"): self.select(f)

    def test_duplicate_review_and_boundary_changes_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            with self.assertRaises(ValueError):
                analysis.native_function_selection(f.raw, f.capture, [0x1000],
                    callsite_reviews=[f.specification, f.specification])
            with self.assertRaises(ValueError):
                analysis.native_function_selection(f.raw, f.capture, [0x1000],
                    callsite_reviews=[f.specification], outline_rvas=[0x1010], type_note="separate reviewed boundary")

    def sdk(self, requested, issue=None):
        class Type:
            def __init__(self): self.text = ""
            def __str__(self): return self.text
            def __eq__(self, other): return self.text == other.text
            def is_func(self): return issue != "not_function"
        calls, operand = [], {}
        ea, table = int(requested["instruction"]["address"], 16), requested["vtable"]
        memory = {ea: bytes.fromhex(requested["instruction"]["bytes"]),
                  0x140000000 + int(table["rva"], 16): bytes.fromhex(table["raw_hex"])}
        if issue == "bytes": memory[ea] = b"\xff\x22"
        def parse(tif, _, declaration, flags):
            calls.append("parse")
            tif.text = declaration
            return issue != "parse"
        def get(tif, address, n):
            if address not in operand: return False
            tif.text = "different type" if issue == "readback" else operand[address]
            return issue != "missing_readback"
        def set_type(address, n, tif):
            calls.append("set")
            operand[address] = str(tif)
            return issue != "set"
        def propagate(address, tif):
            calls.append("propagate")
            return issue != "propagate"
        sdk = SimpleNamespace(bytes=SimpleNamespace(get_bytes=lambda address, size: memory.get(address)),
            types=SimpleNamespace(tinfo_t=Type, parse_decl=parse, PT_SIL=0, apply_callee_tinfo=propagate),
            nalt=SimpleNamespace(get_op_tinfo=get, set_op_tinfo=set_type))
        return sdk, calls

    def test_sdk_applies_and_reads_back_the_exact_type_without_patching_machine_or_stack(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            requested = self.select(f)["callsite_type_recoveries"][0]
            sdk, calls = self.sdk(requested)
            applied = native_callsites.apply(requested, 0x140000000, sdk)
            self.assertEqual(calls, ["parse", "set", "propagate"])
            self.assertEqual(applied["parsed_type"], requested["declaration"])
            self.assertTrue(applied["operand_type_readback_matches"])
            self.assertFalse(applied["base_database_modified"])
            native_callsites.verify_applied([requested], [applied])

    def direct(self, f):
        owners = [{"owner_rva": rva, "owner_function": evidence.identity(
            analysis, f.result / "functions" / (rva[2:] + ".json"))} for rva in ("0x1000", "0x1010")]
        recoveries = [{"rva": "0x1010", "declaration": "void __fastcall resolved(void *context);",
                       "assembly_evidence": "Independent synthetic E8 target and callee contract"}]
        return owners, recoveries

    def direct_sdk(self, f, requested, issue=None):
        sdk, calls = self.sdk({**requested, "vtable": {"rva": "0x1080", "bytes": 8,
                                                      "raw_hex": "1010004001000000"}}, issue)
        memory = bytearray(f.raw)
        if issue == "bytes": memory[0x200] = 0xe9
        if issue == "callee_bytes": memory[0x210] = 0x90
        def get_bytes(address, size):
            offset = address - 0x140001000 + 0x200
            return bytes(memory[offset:offset + size])
        functions = {}
        for rva in ("1000", "1010"):
            body = json.loads((f.result / "functions" / (rva + ".json")).read_bytes())
            ea = int(body["address"], 16)
            functions[ea] = SimpleNamespace(start_ea=ea, end_ea=int(body["end"], 16))
        def get_function(ea):
            return None if issue == "missing_function" else functions.get(ea)
        def get_callee(tif, ea):
            tif.text = "wrong applied callee" if issue == "callee_type" else requested["declaration"]
            return issue != "missing_callee_type"
        sdk.root = analysis.ROOT
        sdk.bytes.get_bytes = get_bytes
        sdk.funcs = SimpleNamespace(get_func=get_function)
        sdk.utils = SimpleNamespace(Chunks=lambda ea: [(functions[ea].start_ea,
                                                        functions[ea].end_ea + (issue == "boundary"))])
        sdk.nalt.get_tinfo = get_callee
        return sdk, calls

    def test_direct_selection_requires_actual_e8_target_and_selected_reviewed_callee(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory), b"\xe8\x0b\0\0\0")
            owners, recoveries = self.direct(f)
            selected = native_callsites.select_direct(analysis, f.raw, f.capture, owners, recoveries)
            self.assertEqual(len(selected), 1)
            self.assertEqual(selected[0]["callee_rva"], "0x1010")
            self.assertEqual(selected[0]["instruction"]["bytes"], "e80b000000")
            self.assertEqual(selected[0]["owner_function"], owners[0]["owner_function"])
            self.assertEqual(native_callsites.select_direct(analysis, f.raw, f.capture, owners, []), [])
        for code in (b"\xe9\x0b\0\0\0", b"\xff\x12", b"\xe8\x1b\0\0\0"):
            with self.subTest(code=code), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), code)
                self.assertEqual(native_callsites.select_direct(analysis, f.raw, f.capture, *self.direct(f)), [])

    def test_direct_propagation_selection_is_explicit_and_requires_owner_reanalysis(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory), b"\xe8\x0b\0\0\0")
            options = {"type_specs": ["0x1010=void __fastcall resolved(void *context);"],
                       "type_note": "Independent synthetic E8 target and callee contract"}
            ordinary = analysis.native_function_selection(f.raw, f.capture, [0x1000, 0x1010],
                **options, reanalyze_type_owners=True)
            self.assertNotIn("direct_call_type_propagations", ordinary)
            selected = analysis.native_function_selection(f.raw, f.capture, [0x1000, 0x1010],
                **options, reanalyze_type_owners=True, propagate_direct_calls=True)
            self.assertEqual(len(selected["direct_call_type_propagations"]), 1)
            self.assertEqual(selected["direct_call_type_propagations"][0]["callee_rva"], "0x1010")
            for additional in ({"propagate_direct_calls": True},
                               {"reanalyze_type_owners": True, "propagate_direct_calls": 1}):
                with self.subTest(additional=additional), self.assertRaises(ValueError):
                    analysis.native_function_selection(f.raw, f.capture, [0x1000, 0x1010], **options, **additional)

    def arm_direct_fixture(self, root, call=b"\x04\0\0\x94"):
        f = self.fixture(root, call)
        raw = bytearray(f.raw)
        pe_offset = struct.unpack_from("<I", raw, 0x3c)[0]
        struct.pack_into("<H", raw, pe_offset + 4, 0xaa64)
        raw[0x204:0x208] = bytes.fromhex("c0035fd6")
        for offset in (0x210, 0x220, 0x230): raw[offset:offset + 4] = bytes.fromhex("c0035fd6")
        old_result = f.result
        f.raw = bytes(raw)
        original = root / f.capture["input"]
        original.write_bytes(f.raw)
        f.capture["input_sha256"] = hashlib.sha256(f.raw).hexdigest()
        f.result = root / ".analysis/results/native" / f.capture["input_sha256"]
        for record in f.capture["functions"]:
            body = json.loads((old_result / record["path"]).read_bytes())
            address = int(body["address"], 16)
            rva = int(body["rva"], 16)
            size = 8 if rva == 0x1000 else 4
            code = f.raw[0x200 + rva - 0x1000:0x200 + rva - 0x1000 + size]
            body["end"] = hex(address + size)
            body["chunks"] = [{"start": hex(address), "end": body["end"], "raw_hex": code.hex()}]
            body["assembly"] = [{"address": hex(address), "rva": hex(rva), "bytes": code[-4:].hex(), "text": "RET"}]
            if rva == 0x1000:
                body["assembly"] = [{"address": hex(address), "rva": hex(rva), "bytes": call.hex(), "text": "BL 0x140001010"},
                    {"address": hex(address + 4), "rva": hex(rva + 4), "bytes": code[-4:].hex(), "text": "RET"}]
            changed = f.write(f.result / record["path"], body)
            record.update(bytes=changed["bytes"], sha256=changed["sha256"])
        f.write(f.result / "capture.json", f.capture)
        return f

    def test_arm64_bl_selection_uses_exact_signed_immediate_and_verified_pe_machine(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.arm_direct_fixture(Path(directory))
            selected = native_callsites.select_direct(analysis, f.raw, f.capture, *self.direct(f))
            self.assertEqual(len(selected), 1)
            self.assertEqual(selected[0]["instruction_architecture"], "arm64")
            self.assertEqual(selected[0]["callee_rva"], "0x1010")
            sdk, _ = self.direct_sdk(f, selected[0])
            applied = native_callsites.apply_direct(selected[0], 0x140000000, sdk)
            native_callsites.verify_direct_applied(selected, [applied])
            self.assertTrue(applied["actual_callee_type_matches"])
        for code in (bytes.fromhex("04000014"), bytes.fromhex("00023fd6"), bytes.fromhex("08000094")):
            with self.subTest(code=code), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.arm_direct_fixture(Path(directory), code)
                self.assertEqual(native_callsites.select_direct(analysis, f.raw, f.capture, *self.direct(f)), [])
        self.assertEqual(native_callsites._direct_target(bytes.fromhex("fcffff97"), 0x1010, "arm64"), 0x1000)
        self.assertIsNone(native_callsites._direct_target(bytes.fromhex("04000094"), 0x1002, "arm64"))
        self.assertIsNone(native_callsites._direct_target(bytes.fromhex("04000094"), 0x1000, "x64"))

    def test_arm64_bl_sdk_rejects_changed_architecture_target_or_original_bytes(self):
        for issue in ("architecture", "target", "bytes", "callee_type", "readback"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.arm_direct_fixture(Path(directory))
                selected = native_callsites.select_direct(analysis, f.raw, f.capture, *self.direct(f))[0]
                sdk, _ = self.direct_sdk(f, selected, issue if issue in {"bytes", "callee_type", "readback"} else None)
                if issue == "architecture": selected["instruction_architecture"] = "x64"
                elif issue == "target": selected["callee_rva"] = "0x1030"
                with self.assertRaises(ValueError):
                    native_callsites.apply_direct(selected, 0x140000000, sdk)

    def test_direct_selection_rejects_changed_original_and_unreviewed_scope(self):
        for issue in ("raw", "source", "owner", "callee", "duplicate", "reason", "duplicate_owner"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), b"\xe8\x0b\0\0\0")
                owners, recoveries = self.direct(f)
                if issue == "raw": f.raw = f.raw + b"changed"
                elif issue == "source": (f.result / "functions/1010.json").write_bytes(b"changed")
                elif issue == "owner": owners[0]["owner_function"]["sha256"] = "0" * 64
                elif issue == "callee": recoveries[0]["rva"] = "0x1020"
                elif issue == "duplicate": recoveries.append(copy.deepcopy(recoveries[0]))
                elif issue == "reason": recoveries[0]["assembly_evidence"] = ""
                else: owners.append(copy.deepcopy(owners[0]))
                with self.assertRaises(ValueError):
                    native_callsites.select_direct(analysis, f.raw, f.capture, owners, recoveries)

    def test_direct_sdk_preserves_old_operand_and_uses_actual_callee_type(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory), b"\xe8\x0b\0\0\0")
            selected = native_callsites.select_direct(analysis, f.raw, f.capture, *self.direct(f))[0]
            sdk, calls = self.direct_sdk(f, selected)
            old = sdk.types.tinfo_t()
            old.text = "previous operand contract"
            sdk.nalt.set_op_tinfo(0x140001000, 0, old)
            calls.clear()
            applied = native_callsites.apply_direct(selected, 0x140000000, sdk)
            self.assertEqual(calls, ["parse", "set", "propagate"])
            self.assertTrue(applied["previous_operand_type_present"])
            self.assertEqual(applied["previous_operand_type"], "previous operand contract")
            self.assertEqual(applied["actual_callee_type"], selected["declaration"])
            native_callsites.verify_direct_applied([selected], [applied])

    def test_direct_sdk_stops_on_changed_bytes_boundaries_types_or_propagation(self):
        for issue in ("bytes", "callee_bytes", "boundary", "missing_function", "callee_type", "missing_callee_type",
                      "parse", "not_function", "set", "propagate", "readback", "missing_readback", "source"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), b"\xe8\x0b\0\0\0")
                selected = native_callsites.select_direct(analysis, f.raw, f.capture, *self.direct(f))[0]
                sdk, _ = self.direct_sdk(f, selected, issue)
                if issue == "source": (f.result / "functions/1010.json").write_bytes(b"changed")
                with self.assertRaises(ValueError): native_callsites.apply_direct(selected, 0x140000000, sdk)

    def test_direct_verifier_rejects_missing_actual_type_and_changed_call_identity(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory), b"\xe8\x0b\0\0\0")
            selected = native_callsites.select_direct(analysis, f.raw, f.capture, *self.direct(f))[0]
            sdk, _ = self.direct_sdk(f, selected)
            applied = native_callsites.apply_direct(selected, 0x140000000, sdk)
            for key, value in (("actual_callee_type", "different"), ("actual_callee_type_matches", False),
                               ("callee_rva", "0x1020"), ("owner_function", {}), ("previous_operand_type", "invented")):
                with self.subTest(key=key):
                    changed = {**copy.deepcopy(applied), key: value}
                    with self.assertRaises(ValueError): native_callsites.verify_direct_applied([selected], [changed])

    def test_any_sdk_failure_or_changed_original_bytes_stops_recovery(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            requested = self.select(f)["callsite_type_recoveries"][0]
            for issue in ("bytes", "parse", "not_function", "set", "propagate", "readback", "missing_readback"):
                with self.subTest(issue=issue):
                    sdk, calls = self.sdk(requested, issue)
                    with self.assertRaises(ValueError): native_callsites.apply(requested, 0x140000000, sdk)
                    if issue == "bytes": self.assertFalse(calls)

    def test_automatic_reanalysis_uses_processor_and_retains_exact_original_stack_observations(self):
        for issue in (None, "cancel", "machine", "boundary", "source", "update", "readback", "ready_flag"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                requested = self.select(f)["callsite_type_recoveries"][0]
                owner_path = Path(directory) / requested["owner_function"]["path"]
                body = json.loads(owner_path.read_bytes())
                for chunk in body["chunks"]:
                    chunk["sha256"] = hashlib.sha256(bytes.fromhex(chunk["raw_hex"])).hexdigest()
                f.write(owner_path, body)
                requested["owner_function"] = evidence.identity(analysis, owner_path)
                calls, phase = [], [False]
                function = SimpleNamespace(start_ea=int(body["address"], 16), end_ea=int(body["end"], 16),
                                           flags=0x5400, frame=7, frsize=32, frregs=4, argsize=0, fpd=0)

                def original_bytes(address, size):
                    for chunk in body["chunks"]:
                        offset = address - int(chunk["start"], 16)
                        code = bytes.fromhex(chunk["raw_hex"])
                        if 0 <= offset and offset + size <= len(code):
                            if phase[0] and issue == "machine": return b"\x90" * size
                            return code[offset:offset + size]
                    return None

                def reanalyze(current):
                    calls.append(("reanalyze_function", current.start_ea))
                    phase[0] = True
                    if issue == "boundary": current.end_ea -= 1

                def update(current):
                    calls.append(("update_func", current.flags))
                    if issue == "readback": current.flags |= 0x400
                    return issue != "update"

                sdk = SimpleNamespace(root=Path(directory), bytes=SimpleNamespace(get_bytes=original_bytes),
                    funcs=SimpleNamespace(get_func=lambda ea: function, reanalyze_function=reanalyze,
                                          FUNC_SP_READY=True if issue == "ready_flag" else 0x400,
                                          update_func=update),
                    utils=SimpleNamespace(Chunks=lambda ea: [(int(c["start"], 16), int(c["end"], 16)) for c in body["chunks"]]),
                    auto=SimpleNamespace(auto_wait=lambda: calls.append(("auto_wait",)) or issue != "cancel"),
                    frame=SimpleNamespace(get_spd=lambda owner, ea: -4 if phase[0] else 0,
                                          get_sp_delta=lambda owner, ea: 4 if phase[0] else 0))
                if issue == "source": owner_path.write_bytes(b"changed original owner")
                if issue is None:
                    result = native_callsites.reanalyze_owners([requested, copy.deepcopy(requested)],
                                                               0x140000000, sdk)
                    self.assertEqual(calls, [("update_func", 0x5000),
                                             ("reanalyze_function", function.start_ea), ("auto_wait",)])
                    self.assertEqual(len(result), 1)
                    self.assertEqual(result[0]["before"]["stack_points"][0]["spd"], 0)
                    self.assertEqual(result[0]["after"]["stack_points"][0]["spd"], -4)
                    self.assertEqual(result[0]["before"]["native_frame"],
                                     {"frame": 7, "frsize": 32, "frregs": 4, "argsize": 0, "fpd": 0})
                    self.assertFalse(result[0]["manual_stack_points_modified"])
                    self.assertFalse(result[0]["target_execution"])
                    self.assertEqual(result[0]["analysis_invalidation"]["previous_flags"], 0x5400)
                    self.assertEqual(result[0]["analysis_invalidation"]["invalidated_flags"], 0x5000)
                    native_callsites.verify_reanalysis(analysis, [requested], result)
                    for mutation in ("missing_point", "altered_chunk", "manual", "incomplete",
                                     "invalidated_flags", "invalidated_readback", "invalidated_manual",
                                     "missing_frame_field", "bad_frame_scalar"):
                        changed = copy.deepcopy(result)
                        if mutation == "missing_point": changed[0]["after"]["stack_points"].pop()
                        elif mutation == "altered_chunk": changed[0]["before"]["chunks"][0]["raw_hex"] = "90"
                        elif mutation == "manual": changed[0]["manual_stack_points_modified"] = True
                        elif mutation == "incomplete": changed[0]["auto_wait_completed"] = False
                        elif mutation == "invalidated_flags": changed[0]["analysis_invalidation"]["invalidated_flags"] ^= 0x1000
                        elif mutation == "invalidated_readback": changed[0]["analysis_invalidation"]["flag_readback_matches"] = False
                        elif mutation == "missing_frame_field": changed[0]["after"]["native_frame"].pop("fpd")
                        elif mutation == "bad_frame_scalar": changed[0]["after"]["native_frame"]["fpd"] = False
                        else: changed[0]["analysis_invalidation"]["manual_stack_points_modified"] = True
                        with self.assertRaises(ValueError):
                            native_callsites.verify_reanalysis(analysis, [requested], changed)
                else:
                    with self.assertRaises(ValueError):
                        native_callsites.reanalyze_owners([requested], 0x140000000, sdk)

    def test_verifier_rejects_changed_target_sdk_result_or_missing_operation(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            requested = self.select(f)["callsite_type_recoveries"][0]
            sdk, _ = self.sdk(requested)
            original = native_callsites.apply(requested, 0x140000000, sdk)
            for key, value in (("callee_rva", "0x1020"), ("instruction", {}), ("applied_type", "different"),
                               ("callee_type_applied", False), ("operand_type_readback_matches", False),
                               ("base_database_modified", True), ("previous_operand_type_present", 0)):
                with self.subTest(key=key):
                    applied = {**copy.deepcopy(original), key: value}
                    with self.assertRaises(ValueError): native_callsites.verify_applied([requested], [applied])
            for applied in ([], [None], [original, original]):
                with self.assertRaises(ValueError): native_callsites.verify_applied([requested], applied)

    def recovery(self, f):
        selection = self.select(f)
        requested = selection["callsite_type_recoveries"][0]
        sdk, _ = self.sdk(requested)
        output = f.result / "supplements/resolved-callsite"
        body = json.loads((f.result / "functions/1000.json").read_bytes())
        body.update(type="void resolved()", type_before_decompile="void resolved()",
                    type_after_decompile="void resolved()", assembly_after_decompile=body["assembly"])
        body["pseudocode"] = {"lines": [{"line": 0, "text": "void resolved() {}"}], "locals": [],
                              "locals_capture": {"complete": True, "mba_entry_ea": body["address"]}}
        record = f.write(output / "functions/1000.json", body)
        record.update(path="functions/1000.json", address=body["address"], rva=body["rva"],
                      assembly_captured=True, pseudocode_captured=True)
        base = {key: f.review["base_capture"][key] for key in ("path", "sha256")}
        source = analysis.ROOT / "tools/native_callsites.py"
        source.parent.mkdir(parents=True)
        source.write_bytes(native_callsites._SOURCE)
        tool = {"path": "tools/ida/supplement.py", "sha256": "a" * 64}
        database = {"path": ".work/private.i64", "sha256": "b" * 64}
        manifest = {"input_sha256": f.capture["input_sha256"], "base_capture": base,
                    "capture_complete": True, "all_pseudocode_captured": True, "failures": [],
                    "database_integrity": {"verified": True}, "base_capture_unchanged": True,
                    "base_database_unchanged": True, "target_execution": False, "capture_tool": tool,
                    "function_selection": selection, "functions": [record], "type_recoveries": [],
                    "source_database": database, "ida_version": "synthetic", "hexrays_version": "synthetic",
                    "callsite_type_validator": evidence.identity(analysis, source),
                    "callsite_type_recoveries": [native_callsites.apply(requested, 0x140000000, sdk)]}
        f.write(output / "capture.json", manifest)
        f.write(output / "plan.json", {"base_capture": base, "function_selection": selection,
                                        "source_database": database})
        f.write(output / "session.json", {"passed": True, "exit_code": 0, "capture_tool": tool})
        return output, manifest, source

    def test_strict_resolver_rechecks_review_sdk_audit_and_source_before_recovery(self):
        for issue in (None, "readback", "target", "source", "review", "missing_operation"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                output, manifest, source = self.recovery(f)
                if issue == "readback": manifest["callsite_type_recoveries"][0]["operand_type_readback_matches"] = False
                elif issue == "target": manifest["callsite_type_recoveries"][0]["callee_rva"] = "0x1020"
                elif issue == "source": source.write_bytes(b"changed validator source")
                elif issue == "review":
                    f.review["callee_rva"] = "0x1020"
                    f.write(f.review_path, {"calls": [f.review]})
                elif issue == "missing_operation": manifest["callsite_type_recoveries"] = []
                f.write(output / "capture.json", manifest)
                resolved = analysis.native_reanalysis_resolution(f.result)
                if issue is None:
                    self.assertFalse(resolved["remaining_failures"])
                    self.assertFalse(resolved["verification_failures"])
                    self.assertIn("0x1000", resolved["complete_functions"])
                    self.assertEqual(len(resolved["supplements"][0]["callsite_type_recoveries"]), 1)
                else:
                    self.assertEqual(resolved["remaining_failures"], f.capture["failures"])
                    self.assertTrue(resolved["verification_failures"])
                    self.assertFalse(resolved["complete_functions"])

    def test_retained_validator_source_is_verified_after_live_tool_changes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            _, manifest, source = self.recovery(f)
            self.assertFalse(analysis.native_reanalysis_resolution(f.result)["verification_failures"])
            source.write_bytes(b"next independent validator version")
            self.assertFalse(analysis.native_reanalysis_resolution(f.result)["verification_failures"])
            retained = analysis.ROOT / ".analysis/results/reverse-pending/tool-sources" / (
                manifest["callsite_type_validator"]["sha256"] + ".py")
            retained.write_bytes(b"changed retained validator")
            self.assertTrue(analysis.native_reanalysis_resolution(f.result)["verification_failures"])

    def type_owner_selection(self, f):
        return analysis.native_function_selection(f.raw, f.capture, [0x1000],
            type_specs=["0x1000=void __fastcall fixed(void *context);"],
            type_note="Synthetic exact ABI evidence", reanalyze_type_owners=True)

    def test_type_owner_reanalysis_is_explicit_exact_and_preserves_source_bytes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            source = f.result / "functions/1000.json"
            raw = source.read_bytes()
            selected = self.type_owner_selection(f)
            self.assertEqual(selected["type_owner_reanalysis"], [{"owner_rva": "0x1000",
                "owner_function": evidence.identity(analysis, source)}])
            self.assertEqual(source.read_bytes(), raw)
            ordinary = analysis.native_function_selection(f.raw, f.capture, [0x1000],
                type_specs=["0x1000=void __fastcall fixed(void *context);"], type_note="Synthetic exact ABI evidence")
            self.assertNotEqual(selected, ordinary)
            self.assertNotIn("type_owner_reanalysis", ordinary)

    def test_type_owner_reanalysis_rejects_no_type_boundary_changes_and_changed_machine(self):
        for issue in ("no_type", "non_boolean", "outline", "changed_hash", "different_machine"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                options = {"type_specs": ["0x1000=void __fastcall fixed(void *context);"],
                           "type_note": "Synthetic exact ABI evidence", "reanalyze_type_owners": True}
                if issue == "no_type": options["type_specs"] = []
                elif issue == "non_boolean": options["reanalyze_type_owners"] = 1
                elif issue == "outline": options["outline_rvas"] = [0x1010]
                else:
                    path = f.result / "functions/1000.json"
                    if issue == "changed_hash": path.write_bytes(b"damaged original function")
                    else:
                        body = json.loads(path.read_bytes())
                        body["assembly"][0]["bytes"] = "9090"
                        record = f.write(path, body)
                        f.capture["functions"][0]["sha256"] = record["sha256"]
                with self.assertRaises(ValueError):
                    analysis.native_function_selection(f.raw, f.capture, [0x1000], **options)

    def test_strict_type_owner_reanalysis_requires_complete_sdk_audit_and_readiness_invalidation(self):
        for issue in (None, "missing_operation", "missing_invalidation", "manual_stack", "missing_point", "extra_owner"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                output, manifest, _ = self.recovery(f)
                selected = self.type_owner_selection(f)
                manifest["function_selection"] = selected
                manifest.pop("callsite_type_recoveries")
                manifest["type_recoveries"] = [{**selected["type_recoveries"][0],
                    "applied_type": "void __fastcall(void *)", "base_database_modified": False}]
                body = json.loads((f.result / "functions/1000.json").read_bytes())
                before = {"flags": 0x400, "chunks": body["chunks"], "stack_points": [
                    {"ea": ins["address"], "spd": 0, "delta": 0} for ins in body["assembly"]]}
                observed = {**copy.deepcopy(selected["type_owner_reanalysis"][0]), "before": before,
                    "after": copy.deepcopy(before), "reanalyze_function_requested": True,
                    "auto_wait_completed": True, "manual_stack_points_modified": False,
                    "base_database_modified": False, "target_execution": False,
                    "analysis_invalidation": {"operation": "clear_completed_automatic_stack_analysis_readiness",
                        "readiness_flag": 0x400, "previous_flags": 0x400, "invalidated_flags": 0,
                        "update_func_completed": True, "flag_readback_matches": True,
                        "manual_stack_points_modified": False}}
                manifest["type_owner_automatic_reanalysis"] = [observed]
                if issue == "missing_operation": manifest.pop("type_owner_automatic_reanalysis")
                elif issue == "missing_invalidation": observed.pop("analysis_invalidation")
                elif issue == "manual_stack": observed["manual_stack_points_modified"] = True
                elif issue == "missing_point": observed["after"]["stack_points"].pop()
                elif issue == "extra_owner": manifest["type_owner_automatic_reanalysis"].append(copy.deepcopy(observed))
                f.write(output / "plan.json", {"base_capture": manifest["base_capture"],
                    "function_selection": selected, "source_database": manifest["source_database"]})
                f.write(output / "capture.json", manifest)
                resolved = analysis.native_reanalysis_resolution(f.result)
                if issue is None:
                    self.assertFalse(resolved["remaining_failures"])
                    self.assertFalse(resolved["verification_failures"])
                    self.assertEqual(resolved["supplements"][0]["type_owner_automatic_reanalysis"], [observed])
                else:
                    self.assertTrue(resolved["verification_failures"])
                    self.assertEqual(resolved["remaining_failures"], f.capture["failures"])


class NativeSwitchMetadataTests(unittest.TestCase):
    def fixture(self, root):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        start, table = 0x1000, 0x1080
        dispatch = bytes.fromhex("89c183e101488d15") + struct.pack("<i", table - start - 12) + bytes.fromhex("48630c8a4801d1ffe1")
        raw[0x200:0x241] = b"\x90" * 0x41
        raw[0x200:0x215] = dispatch
        raw[0x230] = raw[0x240] = 0xc3
        struct.pack_into("<ii", raw, 0x280, 0x1030 - table, 0x1040 - table)
        raw = bytes(raw)
        original = root / ".analysis/inputs/switch.dll"
        original.parent.mkdir(parents=True)
        original.write_bytes(raw)
        digest = hashlib.sha256(raw).hexdigest()
        result = root / ".analysis/results/native" / digest
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        base, code = 0x140000000, raw[0x200:0x241]
        assembly = [{"address": hex(base + start + at), "rva": hex(start + at), "bytes": dispatch[at:at + size].hex(), "text": "dispatch"}
                    for at, size in zip((0, 2, 5, 12, 16, 19), (2, 3, 7, 4, 3, 2))]
        assembly.extend({"address": hex(base + target), "rva": hex(target), "bytes": "c3", "text": "ret"} for target in (0x1030, 0x1040))
        body = {"address": hex(base + start), "rva": hex(start), "end": hex(base + 0x1041),
            "chunks": [{"start": hex(base + start), "end": hex(base + 0x1041), "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}],
            "assembly": assembly, "flags": 0, "pseudocode": None, "xrefs_to_entry": []}
        fn = write(result / "functions/1000.json", body)
        capture = {"input": analysis.rel(original), "input_sha256": digest, "input_bytes": len(raw), "image_base": hex(base),
            "capture_complete": True, "functions": [{**fn, "rva": "0x1000", "address": body["address"],
                "path": "functions/1000.json", "assembly_captured": True, "pseudocode_captured": False}], "failures": []}
        capture_id = write(result / "capture.json", capture)
        review = {"schema_version": 1, "review_kind": "native_x64_masked_rel32_switch_complete_bytes",
            "source_input": evidence.identity(analysis, original), "base_capture": capture_id,
            "owner_rva": "0x1000", "owner_function": fn, "dispatch_start_rva": "0x1000", "switch_rva": "0x1013",
            "table_rva": "0x1080", "mask": 1,
            "case_targets": [{"value": 0, "target_rva": "0x1030"}, {"value": 1, "target_rva": "0x1040"}],
            "default_policy": "all_masked_values_have_explicit_entries", "assembly_evidence": "Independent synthetic exhaustive 1-bit mask and two signed32 targets",
            "target_execution": False, "raw_artifacts_modified": False}
        review_path = root / ".analysis/results/switch-review.json"
        write(review_path, {"review": review})
        return SimpleNamespace(raw=raw, body=body, result=result, review=review, review_path=review_path,
            capture=capture, write=write, specification=analysis.rel(review_path) + "#/review", base=base)

    def select(self, f):
        return analysis.native_function_selection(f.raw, f.capture, [0x1000], switch_reviews=[f.specification])["switch_metadata_recoveries"][0]

    def test_complete_mask_and_signed_original_table_resolve_every_case(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            selected = self.select(f)
            self.assertEqual(selected["case_targets"], f.review["case_targets"])
            self.assertEqual(selected["table"]["bytes"], 8)
            self.assertEqual(selected["dispatch_raw_hex"], f.body["chunks"][0]["raw_hex"][:42])
            self.assertEqual(selected["owner_function"], f.review["owner_function"])

    def test_changed_scope_partial_default_or_unknown_target_are_rejected(self):
        for issue in ("input", "capture", "owner", "mask_bool", "mask", "switch", "table", "target", "missing_case", "default", "execute", "extra"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "input": f.review["source_input"]["sha256"] = "0" * 64
                elif issue == "capture": f.review["base_capture"]["sha256"] = "0" * 64
                elif issue == "owner": f.review["owner_rva"] = "0x1010"
                elif issue == "mask_bool": f.review["mask"] = True
                elif issue == "mask": f.review["mask"] = 3
                elif issue == "switch": f.review["switch_rva"] = "0x1012"
                elif issue == "table": f.review["table_rva"] = "0x1084"
                elif issue == "target": f.review["case_targets"][0]["target_rva"] = "0x1040"
                elif issue == "missing_case": f.review["case_targets"].pop()
                elif issue == "default": f.review["default_policy"] = "skip_default"
                elif issue == "execute": f.review["target_execution"] = True
                else: f.review["manual_sp"] = 0
                f.write(f.review_path, {"review": f.review})
                with self.assertRaises(ValueError): self.select(f)

    def sdk(self, f, requested, issue=None):
        stored, xrefs, dirty = {}, [], []
        class Info:
            def __init__(self):
                self.flags = 16
                self.ncases = self.jumps = self.lowcase = self.defjump = self.startea = self.elbase = 0
                self.regnum = self.regdtype = self.expr_ea = self.jcases = self.ind_lowcase = 0
                self.marks = []
            def set_jtable_element_size(self, size): self.flags |= 8
            def set_elbase(self, value): self.flags |= 4; self.elbase = value
            def set_expr(self, register, dtype): self.regnum, self.regdtype = register, dtype
            def get_jtable_element_size(self): return 4
            def get_jtable_size(self): return self.ncases
            def get_shift(self): return 0
            def has_default(self): return self.defjump != 0xffffffffffffffff
        def get_bytes(address, size):
            offset = address - f.base - 0x1000 + 0x200
            if issue == "bytes" and address == f.base + 0x1080: return b"\0" * size
            return f.raw[offset:offset + size]
        function = SimpleNamespace(start_ea=f.base + 0x1000, end_ea=f.base + 0x1041, flags=0,
            frame=1, frsize=0, frregs=0, argsize=0, fpd=0)
        def set_switch(ea, info):
            stored[ea] = copy.deepcopy(info)
            if issue == "readback": stored[ea].jumps += 4
        def cases(ea, info):
            return SimpleNamespace(cases=[[0], [1]], targets=[f.base + 0x1030, f.base + (0x1030 if issue == "case" else 0x1040)])
        def create_xrefs(ea, info):
            xrefs.extend(SimpleNamespace(to=f.base + value, iscode=True) for value in (0x1030, 0x1040))
            if issue == "xref": xrefs.pop()
            return issue != "create"
        def mark_dirty(ea, close):
            dirty.append(ea)
            if issue == "frame": function.frsize = 8
        sdk = SimpleNamespace(root=analysis.ROOT, bytes=SimpleNamespace(get_bytes=get_bytes),
            funcs=SimpleNamespace(get_func=lambda ea: function),
            utils=SimpleNamespace(Chunks=lambda ea: [(function.start_ea, function.end_ea)], XrefsFrom=lambda ea: xrefs),
            types=SimpleNamespace(tinfo_t=lambda: "void()"),
            nalt=SimpleNamespace(get_tinfo=lambda tif, ea: False, get_switch_info=lambda ea: stored.get(ea),
                set_switch_info=set_switch, switch_info_t=Info, SWI_USER=1, SWI_SIGNED=2, SWI_ELBASE=4, SWI_J32=8, SWI_VERSION=16),
            frame=SimpleNamespace(get_spd=lambda function, ea: 0, get_sp_delta=lambda function, ea: 0),
            api=SimpleNamespace(BADADDR=0xffffffffffffffff), idp=SimpleNamespace(str2reg=lambda name: 1),
            ua=SimpleNamespace(dt_dword=2), xref=SimpleNamespace(calc_switch_cases=cases, create_switch_xrefs=create_xrefs),
            hexrays=SimpleNamespace(mark_cfunc_dirty=mark_dirty))
        return sdk

    def test_sdk_metadata_readback_checks_all_cases_xrefs_and_unchanged_machine(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); requested = self.select(f)
            actual = native_callsites.apply_switch(requested, f.base, self.sdk(f, requested))
            native_callsites.verify_switches(analysis, [requested], [actual])
            self.assertEqual(actual["before"], actual["after"])
            self.assertFalse(actual["applied_switch"]["has_default"])
            for field in ("sdk_case_targets", "sdk_xref_targets", "before", "sdk_switch_constants"):
                changed = copy.deepcopy(actual); changed[field] = None
                with self.subTest(field=field), self.assertRaises((ValueError, TypeError)):
                    native_callsites.verify_switches(analysis, [requested], [changed])

    def test_sdk_failures_cannot_hide_wrong_table_default_target_or_frame(self):
        for issue in ("bytes", "readback", "case", "xref", "create", "frame"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); requested = self.select(f)
                with self.assertRaises(ValueError): native_callsites.apply_switch(requested, f.base, self.sdk(f, requested, issue))

    def test_repair_cannot_combine_with_type_or_boundary_changes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            with self.assertRaisesRegex(ValueError, "separate supplement"):
                analysis.native_function_selection(f.raw, f.capture, [0x1000], switch_reviews=[f.specification],
                    type_specs=["0x1000=void __fastcall f(void);"], type_note="synthetic type")

    def test_switch_allows_a_byte_unchanged_same_owner_sdk_prepass(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            selected = analysis.native_function_selection(f.raw, f.capture, [0x1000],
                switch_reviews=[f.specification], decompiler_prepass_rvas=[0x1000],
                type_note="Complete unchanged synthetic owner SDK derivation before stable final capture")
            self.assertEqual(selected["decompiler_prepasses"][0]["source_function"], f.review["owner_function"])
            self.assertEqual(selected["decompiler_prepasses"][0]["rva"], selected["switch_metadata_recoveries"][0]["owner_rva"])
            with self.assertRaises(ValueError):
                analysis.native_function_selection(f.raw, f.capture, [0x1000], switch_reviews=[f.specification],
                    decompiler_prepass_rvas=[0x1010], type_note="Unselected prepass owner")


class NativeAbsoluteSwitchTests(unittest.TestCase):
    def fixture(self, root, form="neg"):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        sections = raw[0x188:0x1d8]
        struct.pack_into("<H", raw, 0x84, 0x14c)
        struct.pack_into("<H", raw, 0x94, 0xe0)
        struct.pack_into("<H", raw, 0x98, 0x10b)
        struct.pack_into("<II", raw, 0x98 + 24, 0x2000, 0x400000)
        raw[0x178:0x1c8] = sections
        raw[0x200:0x300] = b"\x90" * 0x100
        base, table_base = 0x400000, 0x1080
        assembly = []
        def put(at, code):
            raw[0x200 + at - 0x1000:0x200 + at - 0x1000 + len(code)] = code
            row = {"address": hex(base + at), "rva": hex(at), "bytes": code.hex(), "text": "original",
                   "xrefs_to": [], "xrefs_from": []}
            assembly.append(row)
            return row
        def edge(source, target, kind):
            start = next(row for row in assembly if row["rva"] == hex(source))
            end = next(row for row in assembly if row["rva"] == hex(target))
            ref = {"from": start["address"], "to": end["address"], "code": True, "type": kind, "user": False}
            start["xrefs_from"].append(ref); end["xrefs_to"].append(ref)
        if form == "neg":
            low, count, jump, register = -7, 8, 0x1012, "ecx"
            put(0x1000, bytes.fromhex("83f908")); put(0x1003, bytes.fromhex("720b"))
            put(0x1010, bytes.fromhex("f7d9"))
            put(jump, bytes.fromhex("ff248d") + struct.pack("<I", base + table_base))
            edge(0x1000, 0x1003, 21); edge(0x1003, 0x1010, 19); edge(0x1010, jump, 21)
        elif form == "borrow":
            low, count, jump, register = -4, 4, 0x1010, "ecx"
            put(0x1000, bytes.fromhex("83e904")); put(0x1003, bytes.fromhex("720b"))
            put(jump, bytes.fromhex("ff248d") + struct.pack("<I", base + table_base))
            edge(0x1000, 0x1003, 21); edge(0x1003, jump, 19)
        else:
            low, count, jump, register = 1, 3, 0x1021, "eax"
            put(0x1000, bytes.fromhex("f7c703000000")); put(0x1006, bytes.fromhex("7508"))
            at = 0x1010
            parts = [bytes.fromhex(code) for code in ("8bc7", "ba03000000", "83e904" if form == "align_sub" else "83f904",
                                                       "7234", "83e003", "03c8" if form == "align_sub" else "2bc8")]
            previous = None
            for part in [*parts, bytes.fromhex("ff2485") + struct.pack("<I", base + table_base)]:
                put(at, part)
                if previous is not None: edge(previous, at, 21)
                previous, at = at, at + len(part)
            edge(0x1000, 0x1006, 21); edge(0x1006, 0x1010, 19)
        entries = []
        for i, index in enumerate(range(low, low + count)):
            target, slot = 0x1090 + i, table_base + 4 * index
            encoded = struct.pack("<I", base + target)
            raw[0x200 + slot - 0x1000:0x204 + slot - 0x1000] = encoded
            instruction = put(target, b"\xc3")
            entries.append({"index": index, "index_u32": index & 0xffffffff, "entry_rva": hex(slot),
                "raw_hex": encoded.hex(), "target_rva": hex(target), "target_instruction": instruction})
        raw = bytes(raw)
        original = root / ".analysis/inputs/absolute.dll"; original.parent.mkdir(parents=True); original.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True); path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        code = raw[0x200:0x2a0]
        body = {"address": hex(base + 0x1000), "rva": "0x1000", "end": hex(base + 0x10a0), "flags": 0,
            "assembly": sorted(assembly, key=lambda row: int(row["rva"], 16)), "pseudocode": None, "xrefs_to_entry": [],
            "chunks": [{"start": hex(base + 0x1000), "end": hex(base + 0x10a0), "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}]}
        function_id = write(result / "functions/1000.json", body)
        capture = {"input": analysis.rel(original), "input_sha256": hashlib.sha256(raw).hexdigest(), "input_bytes": len(raw),
            "image_base": hex(base), "capture_complete": True, "functions": [{**function_id, "rva": "0x1000",
                "address": body["address"], "path": "functions/1000.json", "assembly_captured": True, "pseudocode_captured": False}], "failures": []}
        capture_id = write(result / "capture.json", capture)
        table = {"jump_rva": hex(jump), "original_table_base_rva": hex(table_base), "entry_bytes": 4,
            "index_low": low, "index_count": count, "index_width": 4, "pointer_width": 4,
            "address_arithmetic": "modulo_u32", "guard_proof": "Independent synthetic complete original guard", "entries": entries}
        review = {"schema_version": 1, "review_kind": "complete_x86_memmove_guarded_absolute_table_static_review",
            "source_input": evidence.identity(analysis, original), "base_capture": capture_id, "function": function_id,
            "target_execution": False, "machine_bytes_modified": False, "tables": [table]}
        path = root / ".analysis/results/absolute-switch.json"; write(path, review)
        return SimpleNamespace(raw=raw, body=body, result=result, capture=capture, review=review,
            table=table, review_path=path, write=write, base=base, specification=analysis.rel(path) + "#/tables/0")

    def select(self, f):
        return analysis.native_function_selection(f.raw, f.capture, [0x1000], switch_reviews=[f.specification])["switch_metadata_recoveries"][0]

    def test_all_guard_forms_preserve_every_original_modulo32_table_index(self):
        for form in ("neg", "borrow", "align_sub", "align_cmp"):
            with self.subTest(form=form), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), form); selected = self.select(f)
                self.assertEqual(len(selected["case_targets"]), f.table["index_count"])
                self.assertEqual(selected["case_targets"][0]["value"], f.table["index_low"])
                self.assertEqual(int(selected["table"]["rva"], 16), 0x1080 + f.table["index_low"] * 4)

    def test_incomplete_domain_wrong_entry_guard_bypass_or_target_are_rejected(self):
        for issue in ("domain", "count", "entry", "target", "guard_bypass", "guard_opcode", "input", "execute"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "domain": f.table["index_low"] = -6
                elif issue == "count": f.table["entries"].pop()
                elif issue == "entry": f.table["entries"][0]["raw_hex"] = "00000000"
                elif issue == "target": f.table["entries"][0]["target_rva"] = "0x1091"
                elif issue in ("guard_bypass", "guard_opcode"):
                    body = f.body
                    if issue == "guard_bypass":
                        body["assembly"][2]["xrefs_to"].append({"from": hex(f.base + 0x1000), "to": hex(f.base + 0x1010), "type": 19, "code": True})
                    else: body["assembly"][0]["bytes"] = "83f909"
                    identity = f.write(f.result / "functions/1000.json", body)
                    f.review["function"] = identity; f.capture["functions"][0].update(sha256=identity["sha256"], bytes=identity["bytes"])
                    f.review["base_capture"] = f.write(f.result / "capture.json", f.capture)
                elif issue == "input": f.review["source_input"]["sha256"] = "0" * 64
                else: f.review["target_execution"] = True
                f.write(f.review_path, f.review)
                with self.assertRaises(ValueError): self.select(f)

    def test_x86_case_readback_preserves_zero_or_sign_extension_without_dropping_indices(self):
        for raw in (list(range(-7, 1)),
                    [0xfffffff9, 0xfffffffa, 0xfffffffb, 0xfffffffc, 0xfffffffd, 0xfffffffe, 0xffffffff, 0],
                    [0xfffffffffffffff9, 0xfffffffffffffffa, 0xfffffffffffffffb, 0xfffffffffffffffc,
                     0xfffffffffffffffd, 0xfffffffffffffffe, 0xffffffffffffffff, 0]):
            cases = [{"value": value, "target_rva": "0x1090"} for value in raw]
            self.assertEqual([row["value"] for row in native_callsites._x86_switch_cases(cases, True)], list(range(-7, 1)))
        with self.assertRaises(ValueError): native_callsites._x86_switch_cases([{"value": 0x123456789, "target_rva": "0x1090"}], True)

    def test_absolute_sdk_uses_original_32bit_targets_and_complete_negative_case_readback(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); requested = self.select(f)
            sdk = NativeSwitchMetadataTests().sdk(f, requested)
            function = SimpleNamespace(start_ea=f.base + 0x1000, end_ea=f.base + 0x10a0, flags=0,
                frame=1, frsize=0, frregs=0, argsize=0, fpd=0)
            sdk.funcs.get_func = lambda ea: function
            sdk.utils.Chunks = lambda ea: [(function.start_ea, function.end_ea)]
            sdk.hexrays.mark_cfunc_dirty = lambda ea, close: None
            expected = requested["case_targets"]
            sdk.xref.calc_switch_cases = lambda ea, info: SimpleNamespace(
                cases=[[row["value"] & 0xffffffffffffffff] for row in expected],
                targets=[f.base + int(row["target_rva"], 16) for row in expected])
            sdk.xref.create_switch_xrefs = lambda ea, info: True
            sdk.utils.XrefsFrom = lambda ea: [SimpleNamespace(to=f.base + int(row["target_rva"], 16), iscode=True) for row in expected]
            actual = native_callsites.apply_switch(requested, f.base, sdk)
            native_callsites.verify_switches(analysis, [requested], [actual])
            self.assertEqual(actual["applied_switch"]["lowcase"], 0xfffffffffffffff9)
            self.assertEqual(actual["applied_switch"]["elbase"], 0)
            self.assertEqual(actual["sdk_case_targets_raw"][0]["value"], 0)
            changed = copy.deepcopy(actual); changed["sdk_case_targets_raw"].pop()
            with self.assertRaises(ValueError): native_callsites.verify_switches(analysis, [requested], [changed])


class NativeStackForwarderTests(unittest.TestCase):
    def fixture(self, root):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        sections = raw[0x188:0x1d8]
        struct.pack_into("<H", raw, 0x84, 0x14c); struct.pack_into("<H", raw, 0x94, 0xe0)
        struct.pack_into("<H", raw, 0x98, 0x10b); struct.pack_into("<II", raw, 0x98 + 24, 0x2000, 0x400000)
        raw[0x178:0x1c8] = sections
        raw[0x200:0x300] = b"\x90" * 0x100
        base, helper = 0x400000, 0x10e0
        bodies, caller_rows = {}, []
        sites = [(0x1000, 0x1004, 2), (0x1020, 0x1024, 3), (0x1040, 0x1044, 4),
                 (0x1060, 0x1064, 2), (0x1060, 0x1084, 3), (0x1060, 0x10a4, 4)]
        def put(at, code, text="original"):
            raw[0x200 + at - 0x1000:0x200 + at - 0x1000 + len(code)] = code
            return {"rva": hex(at), "address": hex(base + at), "bytes": code.hex(), "text": text,
                    "xrefs_to": [], "xrefs_from": []}
        def edge(first, second, kind):
            ref = {"from": first["address"], "to": second["address"], "type": kind, "code": True, "user": False}
            first["xrefs_from"].append(ref); second["xrefs_to"].append(ref)
        helper_parts = []
        for at, code in zip((helper, helper + 1, helper + 2, helper + 5), ("58", "59", "870424", "ffe0")):
            helper_parts.append(put(at, bytes.fromhex(code)))
        for first, second in zip(helper_parts, helper_parts[1:]): edge(first, second, 21)
        for owner, call, words in sites:
            rows = [put(call - 4, bytes.fromhex("ff7204"), "push dword ptr [edx+4]"), put(call - 1, b"\x51", "push ecx"),
                    put(call, b"\xe8" + struct.pack("<i", helper - call - 5), "call helper"), put(call + 5, b"\xc3", "ret")]
            for first, second in zip(rows, rows[1:]): edge(first, second, 21)
            edge(rows[2], helper_parts[0], 17)
            bodies.setdefault(owner, []).extend(rows)
            caller_rows.append({"owner_rva": hex(owner), "callsite_rva": hex(call), "argument_words": words,
                                "assembly_evidence": "Synthetic independently reviewed differing-arity caller"})
        bodies[helper] = helper_parts
        raw = bytes(raw)
        original = root / ".analysis/inputs/forwarder.dll"; original.parent.mkdir(parents=True); original.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True); path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        functions, documents = [], {}
        helper_type = "void __stdcall(void *pthis, void *pmfn, void *pthat, int val2)"
        for owner, rows in bodies.items():
            end = max(int(row["rva"], 16) + len(bytes.fromhex(row["bytes"])) for row in rows)
            code = raw[0x200 + owner - 0x1000:0x200 + end - 0x1000]
            body = {"address": hex(base + owner), "rva": hex(owner), "end": hex(base + end),
                "type": helper_type if owner == helper else "void()", "flags": 0, "assembly": rows,
                "chunks": [{"start": hex(base + owner), "end": hex(base + end), "raw_hex": code.hex()}],
                "pseudocode": None, "xrefs_to_entry": helper_parts[0]["xrefs_to"] if owner == helper else []}
            identity = write(result / "functions" / (hex(owner)[2:] + ".json"), body)
            functions.append({**identity, "path": "functions/" + hex(owner)[2:] + ".json", "rva": hex(owner),
                "address": body["address"], "assembly_captured": True, "pseudocode_captured": False})
            documents[owner] = body
        capture = {"input": analysis.rel(original), "input_bytes": len(raw), "input_sha256": hashlib.sha256(raw).hexdigest(),
                   "capture_complete": True, "image_base": hex(base), "functions": functions, "failures": []}
        capture_id = write(result / "capture.json", capture)
        def fn(owner): return evidence.identity(analysis, result / "functions" / (hex(owner)[2:] + ".json"))
        for row in caller_rows: row["owner_function"] = fn(int(row["owner_rva"], 16))
        review = {"schema_version": 1, "review_kind": "native_x86_stack_forwarder_callsite_type",
            "source_input": evidence.identity(analysis, original), "base_capture": capture_id,
            "owner_rva": "0x1000", "owner_function": fn(0x1000), "callsite_rva": "0x1004",
            "callee_rva": hex(helper), "callee_function": fn(helper), "callee_original_type": helper_type,
            "declaration": "void __stdcall recovered(void *pthis, void *pmfn);", "callers": caller_rows,
            "assembly_evidence": "Independent synthetic exact stack transformation and all six physical E8 callers",
            "target_execution": False, "raw_artifacts_modified": False}
        database = root / ".work/native" / capture["input_sha256"] / "batch/forwarder.dll.i64"
        database.parent.mkdir(parents=True, exist_ok=True); database.write_bytes(b"inert source database identity")
        database_type = "void __stdcall(void *pthis, void *pmfn, void *pthat, void *val2)"
        observer = "Synthetic independent read-only SDK four-argument type observation"
        observation = {"schema_version": 1, "review_kind": "native_private_static_type_observation",
            "source_input": review["source_input"], "source_database": evidence.identity(analysis, database),
            "target_execution": False, "shared_tools_modified": False, "temporary_directory_absent": True,
            "source_database_unchanged": True, "analyzer_exit_code": 0,
            "observer_source": {"source": observer, "sha256": hashlib.sha256(observer.encode()).hexdigest()},
            "observation": {"idc_type": database_type, "tinfo_text": database_type, "global_present": True,
                "details_present": True, "cc": 80, "return_type": "void",
                "arguments": [{"name": name, "type": "void *"} for name in ("pthis", "pmfn", "pthat", "val2")],
                "helper_hex": "5859870424ffe0", "callsite_hex": raw[0x200:0x209].hex(), "target_execution": False}}
        observation_path = root / ".analysis/results/forwarder-observation.json"
        review["database_type_observation"] = write(observation_path, observation)
        review["callee_database_type"] = database_type
        review_path = root / ".analysis/results/forwarder-review.json"; write(review_path, {"review": review})
        return SimpleNamespace(raw=raw, capture=capture, result=result, review=review, review_path=review_path,
            write=write, base=base, bodies=documents, helper=helper, observation=observation, observation_path=observation_path,
            database=database, specification=analysis.rel(review_path) + "#/review")

    def select(self, f):
        return native_callsites.select(analysis, f.raw, f.capture, [0x1000], [f.specification])[0]

    def test_complete_polymorphic_helper_keeps_every_caller_and_global_four_word_type(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); selected = self.select(f)
            self.assertEqual(len(selected["callers"]), 6)
            self.assertEqual({row["argument_words"] for row in selected["callers"]}, {2, 3, 4})
            self.assertEqual(selected["callee_original_type"], f.review["callee_original_type"])
            self.assertEqual(selected["callee_raw_hex"], "5859870424ffe0")

    def test_missing_caller_changed_global_type_wrong_arity_and_setup_bypass_are_rejected(self):
        for issue in ("missing", "arity", "global", "declaration", "input", "execute", "callee", "setup", "bypass", "physical"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "missing": f.review["callers"].pop()
                elif issue == "arity": f.review["callers"][0]["argument_words"] = 3
                elif issue == "global": f.review["callee_original_type"] = "void __stdcall(void *pthis, void *pmfn)"
                elif issue == "declaration": f.review["declaration"] = "void __stdcall recovered(void *pthis, void *pmfn, int extra);"
                elif issue == "input": f.review["source_input"]["sha256"] = "0" * 64
                elif issue == "execute": f.review["target_execution"] = True
                elif issue == "callee": (f.result / "functions/10e0.json").write_text("{}")
                elif issue in {"setup", "bypass"}:
                    body = f.bodies[0x1000]
                    if issue == "setup": body["assembly"][0]["bytes"] = "ff7208"
                    else: body["assembly"][1]["xrefs_to"].append({"from": hex(f.base + 0x1060), "to": hex(f.base + 0x1003), "type": 19, "code": True})
                    identity = f.write(f.result / "functions/1000.json", body)
                    f.capture["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                    f.review["owner_function"] = identity
                    for row in f.review["callers"]:
                        if row["owner_rva"] == "0x1000": row["owner_function"] = identity
                    f.review["base_capture"] = f.write(f.result / "capture.json", f.capture)
                else:
                    body = f.bodies[f.helper]
                    body["xrefs_to_entry"] = body["xrefs_to_entry"][:-1]
                    body["assembly"][0]["xrefs_to"] = body["assembly"][0]["xrefs_to"][:-1]
                    identity = f.write(f.result / "functions/10e0.json", body)
                    f.capture["functions"][-1].update(bytes=identity["bytes"], sha256=identity["sha256"])
                    f.review["callee_function"] = identity; f.review["callers"].pop()
                    f.review["base_capture"] = f.write(f.result / "capture.json", f.capture)
                f.write(f.review_path, {"review": f.review})
                with self.assertRaises(ValueError): self.select(f)

    def sdk(self, f, requested, issue=None):
        sdk, calls = NativeCallsiteTests().sdk({**requested, "vtable": {"rva": "0x1080", "bytes": 8, "raw_hex": "00" * 8}}, issue)
        def get_bytes(ea, size):
            offset = ea - f.base - 0x1000 + 0x200
            value = f.raw[offset:offset + size]
            return bytes(size) if issue == "helper_bytes" and ea == f.base + f.helper else value
        def get_type(tif, ea):
            tif.text = "changed global type" if issue == "global" or issue == "mutate_global" and "propagate" in calls else requested["callee_database_type"]
            return issue != "missing_global"
        sdk.bytes.get_bytes = get_bytes; sdk.nalt.get_tinfo = get_type
        class Details(list):
            cc, rettype = 80, "void"
        def get_details(tif, details):
            details.extend(SimpleNamespace(name=name, type="void *") for name in ("pthis", "pmfn", "pthat", "val2"))
            if issue == "arg_count": details.pop()
            if issue == "arg_type": details[-1].type = "int"
            if issue == "cc": details.cc = 48
            return issue != "details"
        sdk.types.func_type_data_t = Details; sdk.types.tinfo_t.get_func_details = get_details
        return sdk

    def test_sdk_changes_only_operand_and_requires_original_global_type_readback(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); selected = self.select(f)
            actual = native_callsites.apply(selected, f.base, self.sdk(f, selected))
            native_callsites.verify_applied([selected], [actual])
            self.assertEqual(actual["callee_global_type_before"], actual["callee_global_type_after"])
            for key in ("callee_global_type_before", "callee_global_type_after", "callee_global_type_unchanged"):
                changed = copy.deepcopy(actual); changed[key] = None
                with self.subTest(key=key), self.assertRaises(ValueError): native_callsites.verify_applied([selected], [changed])
            for issue in ("helper_bytes", "global", "missing_global", "mutate_global", "readback", "propagate", "arg_count", "arg_type", "cc", "details"):
                with self.subTest(issue=issue), self.assertRaises(ValueError):
                    native_callsites.apply(selected, f.base, self.sdk(f, selected, issue))

    def test_database_type_observation_cannot_alias_capture_type_omit_arguments_or_change_source(self):
        for issue in ("database", "capture_alias", "idc", "tinfo", "arguments", "cc", "helper", "callsite", "execute", "released", "source", "observer"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); observed = f.observation["observation"]
                if issue == "database": f.database.write_bytes(b"changed source database")
                elif issue == "capture_alias": f.review["callee_database_type"] = f.review["callee_original_type"]
                elif issue == "idc": observed["idc_type"] = f.review["callee_original_type"]
                elif issue == "tinfo": observed["tinfo_text"] = f.review["callee_original_type"]
                elif issue == "arguments": observed["arguments"].pop()
                elif issue == "cc": observed["cc"] = 48
                elif issue == "helper": observed["helper_hex"] = "00" * 7
                elif issue == "callsite": observed["callsite_hex"] = "00" * 9
                elif issue == "execute": observed["target_execution"] = True
                elif issue == "released": f.observation["temporary_directory_absent"] = False
                elif issue == "source": f.observation["source_input"] = {**f.observation["source_input"], "sha256": "0" * 64}
                else: f.observation["observer_source"]["sha256"] = "0" * 64
                f.review["database_type_observation"] = f.write(f.observation_path, f.observation)
                f.write(f.review_path, {"review": f.review})
                with self.assertRaises(ValueError): self.select(f)

    def test_automatic_reanalysis_requires_unchanged_global_abi_after_processor_refresh(self):
        for issue in (None, "late_global"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); requested = self.select(f)
                body = f.bodies[0x1000]
                for chunk in body["chunks"]: chunk["sha256"] = hashlib.sha256(bytes.fromhex(chunk["raw_hex"])).hexdigest()
                requested["owner_function"] = f.write(f.result / "functions/1000.json", body)
                sdk = self.sdk(f, requested); phase = [False]
                function = SimpleNamespace(start_ea=f.base + 0x1000, end_ea=int(body["end"], 16), flags=0x5400,
                    frame=7, frsize=32, frregs=4, argsize=0, fpd=0)
                sdk.root = analysis.ROOT
                sdk.funcs = SimpleNamespace(get_func=lambda ea: function, update_func=lambda current: True,
                    reanalyze_function=lambda current: phase.__setitem__(0, True), FUNC_SP_READY=0x400)
                sdk.utils = SimpleNamespace(Chunks=lambda ea: [(int(row["start"], 16), int(row["end"], 16)) for row in body["chunks"]])
                sdk.auto = SimpleNamespace(auto_wait=lambda: True)
                sdk.frame = SimpleNamespace(get_spd=lambda current, ea: 0, get_sp_delta=lambda current, ea: 0)
                previous = sdk.nalt.get_tinfo
                def get_type(tif, ea):
                    present = previous(tif, ea)
                    if issue == "late_global" and phase[0]: tif.text = requested["callee_original_type"]
                    return present
                sdk.nalt.get_tinfo = get_type
                if issue:
                    with self.assertRaises(ValueError): native_callsites.reanalyze_owners([requested], f.base, sdk)
                else:
                    actual = native_callsites.reanalyze_owners([requested], f.base, sdk)
                    native_callsites.verify_reanalysis(analysis, [requested], actual)
                    for key in ("callee_global_before_reanalysis", "callee_global_after_reanalysis", "callee_global_unchanged_after_reanalysis"):
                        changed = copy.deepcopy(actual); changed[0][key] = None
                        with self.subTest(key=key), self.assertRaises(ValueError):
                            native_callsites.verify_reanalysis(analysis, [requested], changed)


class NativeFsmSwitchTests(unittest.TestCase):
    def fixture(self, root):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        sections = raw[0x188:0x1d8]
        struct.pack_into("<H", raw, 0x84, 0x14c); struct.pack_into("<H", raw, 0x94, 0xe0)
        struct.pack_into("<H", raw, 0x98, 0x10b); struct.pack_into("<II", raw, 0x98 + 24, 0x2000, 0x400000)
        raw[0x178:0x1c8] = sections
        raw.extend(b"\0" * 0x500)
        struct.pack_into("<I", raw, 0x178 + 8, 0x500); struct.pack_into("<I", raw, 0x178 + 16, 0x500)
        struct.pack_into("<I", raw, 0x178 + 40 + 20, 0x700)
        raw[0x200:0x700] = b"\x90" * 0x500
        base, classify, transition, table = 0x400000, 0x1020, 0x1380, 0x1400
        assembly, links = [], []
        def put(at, code):
            raw[0x200 + at - 0x1000:0x200 + at - 0x1000 + len(code)] = code
            row = {"rva": hex(at), "address": hex(base + at), "bytes": code.hex(), "text": "original", "xrefs_to": [], "xrefs_from": []}
            assembly.append(row); return row
        def edge(source, target, kind=21):
            links.append((source, target, kind))
        def near(at, target, condition=None):
            prefix = b"\xe9" if condition is None else bytes((0x0f, 0x80 + condition))
            return prefix + struct.pack("<i", target - at - len(prefix) - 4)
        put(0x1000, bytes.fromhex("33d2")); put(0x1002, bytes.fromhex("899560f7ffff"))
        put(0x1008, near(0x1008, classify)); edge(0x1000, 0x1002); edge(0x1002, 0x1008)
        parts = [bytes.fromhex(x) for x in ("8d41e0", "3c58", "770f", "0fbec1")]
        parts += [bytes.fromhex("0fb680") + struct.pack("<I", base + transition - 0x20)]
        parts += [bytes.fromhex(x) for x in ("83e00f", "eb02", "33c0", "8b8d60f7ffff", "6bc009")]
        parts += [bytes.fromhex("0fb68c08") + struct.pack("<I", base + transition)]
        parts += [bytes.fromhex(x) for x in ("c1e904", "898d60f7ffff", "83f901", "0f85e3000000")]
        offsets, at = [], classify
        for code in parts: offsets.append(at); put(at, code); at += len(code)
        edge(0x1008, classify, 19)
        for index in range(len(offsets) - 1):
            if index not in {6, 7}: edge(offsets[index], offsets[index + 1])
        edge(offsets[2], offsets[7], 19); edge(offsets[6], offsets[8], 19); edge(offsets[7], offsets[8])
        one_branch = offsets[-1]; eight_cmp = one_branch + 6 + 0xe3
        eight_branch, load, switch = eight_cmp + 3, eight_cmp + 9, eight_cmp + 15
        error, reset, finish = 0x1320, 0x1330, 0x1340
        put(at, near(at, load)); edge(one_branch, at); edge(at, load, 19)
        put(eight_cmp, bytes.fromhex("83f908")); put(eight_branch, near(eight_branch, error, 4))
        put(load, bytes.fromhex("8b9d60f7ffff")); put(switch, bytes.fromhex("ff249d") + struct.pack("<I", base + table))
        edge(one_branch, eight_cmp, 19); edge(eight_cmp, eight_branch); edge(eight_branch, error, 19); edge(eight_branch, load); edge(load, switch)
        targets = list(range(0x1280, 0x1300, 0x10))
        # One non-transition read plus two mandatory reads makes all six state
        # references; recovery has its own independently proven zero source.
        for index, target in enumerate(targets):
            if index == 0:
                put(target, bytes.fromhex("8b8d60f7ffff")); put(target + 6, near(target + 6, reset)); edge(target, target + 6); edge(target + 6, reset, 19)
            else:
                put(target, near(target, classify)); edge(target, classify, 19)
            edge(switch, target, 19)
            raw[0x200 + table - 0x1000 + index * 4:0x204 + table - 0x1000 + index * 4] = struct.pack("<I", base + target)
        put(error, b"\xc3")
        put(reset, bytes.fromhex("33c0")); put(reset + 2, bytes.fromhex("898560f7ffff")); put(reset + 8, near(reset + 8, finish))
        put(finish, b"\xc3"); edge(reset, reset + 2); edge(reset + 2, reset + 8); edge(reset + 8, finish, 19)
        for source, target, kind in links:
            first = next(row for row in assembly if row["rva"] == hex(source)); second = next(row for row in assembly if row["rva"] == hex(target))
            ref = {"from": first["address"], "to": second["address"], "type": kind, "code": True, "user": False}
            first["xrefs_from"].append(ref); second["xrefs_to"].append(ref)
        lookup = bytes((((index // 9) % 9) << 4) | (index % 9) for index in range(89))
        raw[0x200 + transition - 0x1000:0x200 + transition - 0x1000 + len(lookup)] = lookup
        raw = bytes(raw)
        original = root / ".analysis/inputs/fsm.dll"; original.parent.mkdir(parents=True); original.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True); path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        code = raw[0x200:0x541]
        body = {"address": hex(base + 0x1000), "rva": "0x1000", "end": hex(base + 0x1341), "flags": 0,
            "assembly": sorted(assembly, key=lambda row: int(row["rva"], 16)), "pseudocode": None, "xrefs_to_entry": [],
            "chunks": [{"start": hex(base + 0x1000), "end": hex(base + 0x1341), "raw_hex": code.hex()}]}
        identity = write(result / "functions/1000.json", body)
        capture = {"input": analysis.rel(original), "input_bytes": len(raw), "input_sha256": hashlib.sha256(raw).hexdigest(),
            "image_base": hex(base), "capture_complete": True, "functions": [{**identity, "path": "functions/1000.json", "rva": "0x1000",
                "address": body["address"], "assembly_captured": True, "pseudocode_captured": False}], "failures": []}
        capture_id = write(result / "capture.json", capture)
        review = {"schema_version": 1, "review_kind": "native_x86_printf_fsm_switch_complete_bytes",
            "source_input": evidence.identity(analysis, original), "base_capture": capture_id, "owner_rva": "0x1000", "owner_function": identity,
            "classification_start_rva": hex(classify), "transition_table_rva": hex(transition), "character_class_table_rva": hex(transition),
            "character_class_raw_hex": lookup.hex(), "transition_raw_hex": lookup[:81].hex(),
            "state_references": [row for row in body["assembly"] if "60f7ffff" in row["bytes"]],
            "switch_rva": hex(switch), "table_rva": hex(table),
            "case_targets": [{"value": index, "target_rva": hex(target)} for index, target in enumerate(targets)],
            "assembly_evidence": "Independent synthetic nine-state transition matrix and complete initialized/recovered state graph",
            "target_execution": False, "raw_artifacts_modified": False}
        path = root / ".analysis/results/fsm-review.json"; write(path, {"review": review})
        return SimpleNamespace(raw=raw, body=body, review=review, capture=capture, result=result, write=write, review_path=path,
            base=base, specification=analysis.rel(path) + "#/review", switch=switch, error=error, reset=reset, edge=edge)

    def select(self, f):
        return native_callsites.select_switches(analysis, f.raw, f.capture, [0x1000], [f.specification])[0]

    def test_full_classification_transition_initialization_recovery_and_domain_are_proven(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); selected = self.select(f)
            self.assertEqual(selected["domain_proof"]["dispatch_domain"], list(range(8)))
            self.assertEqual(len(selected["state_references"]), 6)
            self.assertEqual([row["bytes"] for row in selected["lookup_tables"]], [89, 81])
            self.assertEqual(selected["expression_register"], "ebx")

    def test_partial_table_unknown_state_missing_recovery_and_wrong_target_are_rejected(self):
        for issue in ("class_length", "transition_length", "state9", "missing_state", "missing_case", "target", "source", "execute", "register"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "class_length": f.review["character_class_raw_hex"] = f.review["character_class_raw_hex"][:-2]
                elif issue == "transition_length": f.review["transition_raw_hex"] = f.review["transition_raw_hex"][:-2]
                elif issue == "state9": f.review["transition_raw_hex"] = "99" + f.review["transition_raw_hex"][2:]
                elif issue == "missing_state": f.review["state_references"].pop()
                elif issue == "missing_case": f.review["case_targets"].pop()
                elif issue == "target": f.review["case_targets"][0]["target_rva"] = "0x1290"
                elif issue == "source": f.review["source_input"]["sha256"] = "0" * 64
                elif issue == "execute": f.review["target_execution"] = True
                else: f.review["expression_register"] = "ecx"
                f.write(f.review_path, {"review": f.review})
                with self.assertRaises(ValueError): self.select(f)

    def test_reciprocal_cfg_checks_reject_bypass_missing_edge_and_zero_source_overwrite(self):
        for issue in ("bypass", "missing_edge", "zero_source", "extra_state"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "bypass":
                    row = next(row for row in f.body["assembly"] if row["rva"] == "0x1002")
                    ref = {"from": row["address"], "to": hex(f.base + f.switch), "code": True, "type": 19}
                    row["xrefs_from"].append(ref); next(row for row in f.body["assembly"] if row["rva"] == hex(f.switch))["xrefs_to"].append(ref)
                elif issue == "missing_edge": f.body["assembly"][0]["xrefs_from"] = []
                elif issue == "zero_source": f.body["assembly"][0]["bytes"] = "33c0"
                else: f.body["assembly"].append({"rva": "0x1342", "address": hex(f.base + 0x1342), "bytes": "898560f7ffff", "text": "state write"})
                identity = f.write(f.result / "functions/1000.json", f.body)
                f.capture["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                f.review["owner_function"] = identity
                f.review["base_capture"] = f.write(f.result / "capture.json", f.capture)
                f.write(f.review_path, {"review": f.review})
                with self.assertRaises(ValueError): self.select(f)

    def test_coherent_changed_originals_cannot_hide_unknown_transition_or_nonzero_initialization(self):
        for issue in ("class_domain", "transition_domain", "initial_zero", "recovery_zero", "state_eight_branch", "reload_register"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); changed = bytearray(f.raw)
                if issue in {"class_domain", "transition_domain"}:
                    at = 0x1380; changed[0x200 + at - 0x1000] = 0x09 if issue == "class_domain" else 0x90
                else:
                    if issue == "initial_zero": at, code = 0x1000, "33c0"
                    elif issue == "recovery_zero": at, code = f.reset, "33d2"
                    elif issue == "state_eight_branch":
                        at = int(self.select(f)["domain_proof"]["state_eight_branch_rva"], 16)
                        code = next(row["bytes"] for row in f.body["assembly"] if row["rva"] == hex(at))
                        code = "0f85" + code[4:]
                    else: at, code = f.switch - 6, "8b8d60f7ffff"
                    encoded = bytes.fromhex(code); changed[0x200 + at - 0x1000:0x200 + at - 0x1000 + len(encoded)] = encoded
                    next(row for row in f.body["assembly"] if row["rva"] == hex(at))["bytes"] = code
                f.raw = bytes(changed)
                original = analysis.ROOT / f.capture["input"]; original.write_bytes(f.raw)
                f.result = analysis.ROOT / ".analysis/results/native" / hashlib.sha256(f.raw).hexdigest()
                f.body["chunks"][0]["raw_hex"] = f.raw[0x200:0x541].hex()
                identity = f.write(f.result / "functions/1000.json", f.body)
                f.capture.update(input_sha256=hashlib.sha256(f.raw).hexdigest(), input_bytes=len(f.raw))
                f.capture["functions"][0].update(bytes=identity["bytes"], sha256=identity["sha256"])
                f.review["source_input"] = evidence.identity(analysis, original); f.review["owner_function"] = identity
                f.review["base_capture"] = f.write(f.result / "capture.json", f.capture)
                f.review["character_class_raw_hex"] = f.raw[0x580:0x5d9].hex()
                f.review["transition_raw_hex"] = f.raw[0x580:0x5d1].hex()
                f.review["state_references"] = [row for row in f.body["assembly"] if "60f7ffff" in row["bytes"]]
                f.write(f.review_path, {"review": f.review})
                with self.assertRaises(ValueError): self.select(f)

    def test_sdk_requires_full_lookup_bytes_and_preserves_original_machine_state(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); requested = self.select(f)
            sdk = NativeSwitchMetadataTests().sdk(f, requested)
            function = SimpleNamespace(start_ea=f.base + 0x1000, end_ea=f.base + 0x1341, flags=0, frame=1, frsize=0, frregs=0, argsize=0, fpd=0)
            sdk.funcs.get_func = lambda ea: function
            sdk.utils.Chunks = lambda ea: [(function.start_ea, function.end_ea)]
            sdk.hexrays.mark_cfunc_dirty = lambda ea, close: None
            cases = requested["case_targets"]
            sdk.xref.calc_switch_cases = lambda ea, info: SimpleNamespace(cases=[[row["value"]] for row in cases], targets=[f.base + int(row["target_rva"], 16) for row in cases])
            sdk.xref.create_switch_xrefs = lambda ea, info: True
            sdk.utils.XrefsFrom = lambda ea: [SimpleNamespace(to=f.base + int(row["target_rva"], 16), iscode=True) for row in cases]
            actual = native_callsites.apply_switch(requested, f.base, sdk)
            native_callsites.verify_switches(analysis, [requested], [actual])
            self.assertFalse(actual["applied_switch"]["has_default"])
            self.assertEqual(actual["before"], actual["after"])
            original_get = sdk.bytes.get_bytes
            sdk.bytes.get_bytes = lambda ea, size: bytes(size) if ea == f.base + 0x1380 else original_get(ea, size)
            with self.assertRaisesRegex(ValueError, "lookup table"):
                native_callsites.apply_switch(requested, f.base, sdk)


if __name__ == "__main__":
    unittest.main()
