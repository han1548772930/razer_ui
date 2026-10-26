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
        sdk.types.get_tinfo = get_callee
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


if __name__ == "__main__":
    unittest.main()
