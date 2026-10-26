"""Original-byte, closed caller inventory and architectural-state import-jump proofs."""
import copy
from dataclasses import asdict
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
import native_import_forwarders as forwarders


class ImportForwarderTests(unittest.TestCase):
    def fixture(self, root):
        raw, base = bytearray(0x600), 0x400000
        raw[:2] = b"MZ"; struct.pack_into("<I", raw, 0x3c, 0x80); raw[0x80:0x84] = b"PE\0\0"
        struct.pack_into("<HHIIIHH", raw, 0x84, 0x14c, 2, 0, 0, 0, 0xe0, 0x102)
        struct.pack_into("<H", raw, 0x98, 0x10b); struct.pack_into("<I", raw, 0x98 + 28, base)
        struct.pack_into("<I", raw, 0x98 + 60, 0x200); struct.pack_into("<I", raw, 0x98 + 92, 16)
        struct.pack_into("<II", raw, 0x98 + 104, 0x2000, 40)
        for index, (name, rva, offset, flags) in enumerate(((b".text", 0x1000, 0x200, 0x60000020),
                                                          (b".idata", 0x2000, 0x400, 0x40000040))):
            struct.pack_into("<8sIIIIIIHHI", raw, 0x178 + index * 40,
                             name, 0x200, rva, 0x200, offset, 0, 0, 0, 0, flags)
        struct.pack_into("<IIIII", raw, 0x400, 0x2060, 0, 0, 0x2090, 0x2080)
        struct.pack_into("<I", raw, 0x460, 0x20a0); struct.pack_into("<I", raw, 0x480, 0x20a0)
        raw[0x490:0x49d] = b"ntoskrnl.exe\0"; raw[0x4a0:0x4ab] = b"\x59\0DbgPrint\0"
        call_sites = ((0x1020, 0x1020), (0x1020, 0x1025), (0x1040, 0x1040))
        raw[0x200:0x206] = b"\xff\x25" + struct.pack("<I", base + 0x2080)
        for _, rva in call_sites:
            raw[0x200 + rva - 0x1000:0x205 + rva - 0x1000] = b"\xe8" + struct.pack("<i", 0x1000 - rva - 5)
        raw[0x22a] = raw[0x245] = 0xc3
        raw = bytes(raw)
        source = root / ".analysis/inputs/native/jump.sys"; source.parent.mkdir(parents=True); source.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True); path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        def xref(at, target, kind=17, code=True):
            return {"from": hex(base + at), "to": hex(base + target), "type": kind, "code": code, "user": False}
        incoming = [xref(at, 0x1000) for _, at in call_sites]
        bodies, records, identities = [], [], []
        for start, lengths in ((0x1000, [6]), (0x1020, [5, 5, 1]), (0x1040, [5, 1])):
            at, rows = start, []
            for length in lengths:
                code = raw[0x200 + at - 0x1000:0x200 + at - 0x1000 + length]
                rows.append({"address": hex(base + at), "rva": hex(at), "bytes": code.hex(),
                    "text": "actual original retained instruction", "xrefs_from": [xref(at, 0x2080, 3, False)]
                        if at == 0x1000 else [xref(at, 0x1000), xref(at, at + 5, 21)] if length == 5 else [],
                    "xrefs_to": incoming if at == 0x1000 else []})
                at += length
            code = raw[0x200 + start - 0x1000:0x200 + at - 0x1000]
            body = {"address": hex(base + start), "rva": hex(start), "end": hex(base + at), "flags": 21696,
                "name": "DbgPrint" if start == 0x1000 else "caller", "type": "ULONG(PCSTR Format, ...)" if start == 0x1000 else "void()",
                "chunks": [{"start": hex(base + start), "end": hex(base + at), "raw_hex": code.hex(),
                            "sha256": hashlib.sha256(code).hexdigest()}],
                "xrefs_to_entry": incoming if start == 0x1000 else [], "assembly": rows, "pseudocode": None}
            identity = write(result / "functions" / (hex(start)[2:] + ".json"), body)
            bodies.append(body); identities.append(identity)
            records.append({**identity, "path": "functions/" + hex(start)[2:] + ".json", "address": body["address"],
                            "rva": body["rva"], "assembly_captured": True, "pseudocode_captured": False})
        capture = {"input": analysis.rel(source), "input_sha256": hashlib.sha256(raw).hexdigest(), "input_bytes": len(raw),
                   "image_base": hex(base), "capture_complete": True, "functions_complete": True,
                   "functions_total": 3, "functions_captured": 3, "settled_functions_total": 3,
                   "target_execution": False, "functions": records,
                   "failures": [{"stage": "1000:hexrays", "error": "call analysis failed (code -12, address 0x401000)"}]}
        cap = write(result / "capture.json", capture)
        binding = forwarders.Pe32(raw).import_slot(0x2080)
        calls = [{"owner_function": identities[1 if owner == 0x1020 else 2], "call_rva": hex(at),
                  "raw_hex": raw[0x200 + at - 0x1000:0x205 + at - 0x1000].hex(), "return_rva": hex(at + 5),
                  "reciprocal_xref": xref(at, 0x1000)} for owner, at in call_sites]
        false_keys = ("provider_semantics_complete", "runtime_iat_target_observed", "provider_executed",
                      "rust_implementation_acceptance", "pseudocode_recovered", "target_execution")
        observation = {"source_input": evidence.identity(analysis, source), "base_capture": cap,
            "function": identities[0], "rva": "0x1000", "original_complete_bytes": bodies[0]["chunks"][0]["raw_hex"],
            "original_import": {k: binding[k] for k in ("library", "name", "iat_rva", "lookup_rva", "lookup_raw_hex", "iat_raw_hex", "descriptor_raw_hex")},
            "incoming_call_count": 3, "caller_owner_count": 2, "incoming_calls": calls,
            "capability_result": {"actual_warning_id_3_emitted": False}, **{k: False for k in false_keys}}
        state = {"address": bodies[0]["address"], "end": bodies[0]["end"], "flags": bodies[0]["flags"],
            "type": bodies[0]["type"], "chunks": bodies[0]["chunks"], "assembly": bodies[0]["assembly"],
            "frame": {key: 0 for key in ("frame", "frsize", "frregs", "argsize", "fpd")},
            "stack_points": [{"address": bodies[0]["address"], "spd": 0, "delta": 0}],
            "entry_type": {"observed": True, "serialized": "original variadic type bytes", "declaration": "ULONG(PCSTR, ...)"},
            "iat_type": {"observed": True, "serialized": "original variadic pointer type bytes", "declaration": "ULONG(*)(PCSTR, ...)"}}
        experiments = []
        for mode in ("original_function_no_cache", "original_function_outline_no_cache"):
            experiments.append({"mode": mode, "success": False, "failure": {"code": -12, "address": bodies[0]["address"]}})
        for snippet in (False, True):
            for maturity in ("MMAT_GENERATED", "MMAT_PREOPTIMIZED", "MMAT_LOCOPT", "MMAT_CALLS", "MMAT_GLBOPT3"):
                success = maturity not in ("MMAT_CALLS", "MMAT_GLBOPT3")
                experiments.append({"mode": "original_bytes_microcode", "snippet": snippet, "maturity": maturity,
                    "success": success, "failure": {"code": 0 if success else -12, "address": "0xffffffffffffffff" if success else bodies[0]["address"]},
                    **({"warnings": []} if success else {})})
        observer = "complete exact static observer"
        matrix = {"review_kind": "x86_variadic_import_tail_sdk_capability_matrix", "target_execution": False,
            "observer_source": {"source": observer, "bytes": len(observer), "sha256": hashlib.sha256(observer.encode()).hexdigest()},
            "observations": [{"source_input": observation["source_input"], "base_capture": cap, "rva": "0x1000",
                "target_execution": False, "analyzer_exit_code": 0,
                "observation": {"target_execution": False, "before": state, "after": copy.deepcopy(state),
                    "original_metadata_unchanged": False, "experiments": experiments}}]}
        probe_path = root / ".analysis/results/probe.py"; probe_path.write_text("static only", encoding="utf-8")
        abi = {"review_kind": "variadic_import_tail_exact_explicit_stack_abi", "target_execution": False,
               "probe_source": evidence.identity(analysis, probe_path), "trials": []}
        database = {"path": ".work/native/" + capture["input_sha256"] + "/batch/jump.sys.i64", "bytes": 1, "sha256": "0" * 64}
        for mode in ("explicit_thunk_abi", "explicit_thunk_and_iat_abi"):
            abi["trials"].append({"rva": "0x1000", "source_database": database, "source_database_after": database,
                "mode": mode, "errors": [], "parsed": True, "ellipsis_preserved": True, "returned_cfunc": False,
                "temporary_removed": True, "source_database_unchanged": True, "declaration": "ULONG(PCSTR, ...)",
                "readback": "ULONG(PCSTR, ...)", "stack_argument_size": 4,
                "argument_locations": [{"kind": 1, "stack_offset": 0}],
                "failure": {"code": -12, "ea": bodies[0]["address"]}})
        machine = {"review_kind": "x86_variadic_import_tail_exact_machine_and_sdk_boundary", "target_execution": False,
            "raw_artifacts_modified": False, "source_matrix": write(root / ".analysis/results/matrix.json", matrix),
            "observations": [observation]}
        machine_path, abi_path = root / ".analysis/results/machine.json", root / ".analysis/results/abi.json"
        return SimpleNamespace(raw=raw, root=root, base=base, source=source, result=result, capture=capture, bodies=bodies,
            identities=identities, machine=machine, abi=abi, matrix=matrix, machine_path=machine_path, abi_path=abi_path,
            matrix_path=root / ".analysis/results/matrix.json", write=write)

    def prove(self, f):
        return forwarders.prove_forwarder(analysis, f.result, "0x1000",
            f.write(f.machine_path, f.machine), f.write(f.abi_path, f.abi))

    def rebind(self, f):
        for index, body in enumerate(f.bodies):
            identity = f.write(f.result / "functions" / (body["rva"][2:] + ".json"), body)
            f.identities[index] = identity; f.capture["functions"][index].update(bytes=identity["bytes"], sha256=identity["sha256"])
        cap = f.write(f.result / "capture.json", f.capture)
        observation = f.machine["observations"][0]; observation.update(base_capture=cap, function=f.identities[0])
        for call in observation["incoming_calls"]:
            owner = next(i for i in f.identities if Path(i["path"]).name == Path(call["owner_function"]["path"]).name)
            call["owner_function"] = owner
        f.matrix["observations"][0]["base_capture"] = cap
        f.machine["source_matrix"] = f.write(f.matrix_path, f.matrix)

    def test_complete_machine_import_and_all_original_callers_are_independently_recomputed(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); proof = self.prove(f)
            self.assertEqual(len(proof["incoming_calls"]), 3); self.assertEqual(len(proof["caller_owners"]), 2)
            self.assertEqual(proof["original_import"]["name"], "DbgPrint")
            self.assertEqual(proof["original_import"]["hint"], 89)
            self.assertEqual(proof["normal_transfer"]["memory_writes"], [])
            self.assertFalse(proof["sdk_evidence"]["actual_warning_id_3_emitted"])
            self.assertFalse(proof["sdk_evidence"]["original_matrix_metadata_equality_flag"])
            self.assertFalse(proof["pseudocode_recovered"])

    def test_cpu_transfer_preserves_every_register_flags_return_and_unbounded_argument_bytes(self):
        model = forwarders.ImportJump32(0x401000, 0x402080)
        stack = bytes.fromhex("78563412") + bytes(range(256)) * 9
        for value in (0, 1, 0x7fffffff, 0xffffffff):
            before = forwarders.Cpu32(1, 2, 3, 4, 0x800000, 6, 7, 8, 0x202, 0x401000, stack)
            after = model.normal_step(before, value)
            expected = asdict(before); expected["eip"] = value
            self.assertEqual(asdict(after), expected)
            self.assertIs(after.stack_bytes, before.stack_bytes)
        with self.assertRaises(ValueError): model.normal_step(before, -1)
        with self.assertRaises(ValueError): forwarders.ImportJump32(0x401001, 0x402080).normal_step(before, 4)

    def test_machine_architecture_opcode_addressing_prefix_iat_and_section_mutations_fail(self):
        mutations = [lambda r: struct.pack_into("<H", r, 0x84, 0x8664),
            lambda r: struct.pack_into("<H", r, 0x98, 0x20b),
            lambda r: struct.pack_into("<H", r, 0x96, 2),
            lambda r: struct.pack_into("<I", r, 0x178 + 36, 0x40000040),
            lambda r: struct.pack_into("<I", r, 0x178 + 40 + 12, 0x1000),
            lambda r: r.__setitem__(0x200, 0x66), lambda r: r.__setitem__(0x201, 0x15),
            lambda r: struct.pack_into("<I", r, 0x202, 0x402084),
            lambda r: struct.pack_into("<I", r, 0x460, 0x80000001),
            lambda r: struct.pack_into("<I", r, 0x480, 0x20a2),
            lambda r: struct.pack_into("<I", r, 0x400, 0),
            lambda r: r.__setitem__(0x4a2, ord('X'))]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); changed = bytearray(f.raw); mutation(changed); f.source.write_bytes(changed)
                with self.assertRaises(ValueError): self.prove(f)

    def test_exact_pe_import_parser_rejects_mutated_descriptors_and_terminators(self):
        mutations = [lambda r: struct.pack_into("<I", r, 0x404, 1),
            lambda r: struct.pack_into("<I", r, 0x408, 1),
            lambda r: struct.pack_into("<I", r, 0x400, 0),
            lambda r: struct.pack_into("<I", r, 0x484, 0x20a0),
            lambda r: struct.pack_into("<I", r, 0x460, 0x80000001),
            lambda r: struct.pack_into("<I", r, 0x480, 0x20a2),
            lambda r: struct.pack_into("<I", r, 0x98 + 108, 20)]
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    raw = bytearray(f.raw); mutation(raw)
                    with self.assertRaises(ValueError): forwarders.Pe32(raw).import_slot(0x2080)

    def test_pe32_default_code_mode_and_unique_executable_mappings_are_required_independently(self):
        mutations = [lambda r: struct.pack_into("<H", r, 0x84, 0x8664),
            lambda r: struct.pack_into("<H", r, 0x98, 0x20b),
            lambda r: struct.pack_into("<H", r, 0x96, 2),
            lambda r: struct.pack_into("<I", r, 0x178 + 36, 0x40000040),
            lambda r: struct.pack_into("<I", r, 0x178 + 40 + 12, 0x1000)]
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    raw = bytearray(f.raw); mutation(raw)
                    with self.assertRaises(ValueError): forwarders.Pe32(raw).at(0x1000, 6, executable=True)

    def test_decoder_rejects_wrong_modrm_prefix_iat_and_boundaries_with_consistent_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for opcode in ("ff1520804000", "662520804000", "ff2524804000", "ff252080400090"):
                with self.subTest(opcode=opcode):
                    raw, body = bytearray(f.raw), copy.deepcopy(f.bodies[0]); code = bytes.fromhex(opcode)
                    raw[0x200:0x200 + len(code)] = code
                    body["end"] = hex(f.base + 0x1000 + len(code))
                    body["chunks"][0].update(end=body["end"], raw_hex=opcode, sha256=hashlib.sha256(code).hexdigest())
                    body["assembly"][0]["bytes"] = opcode
                    with self.assertRaises(ValueError): forwarders.decode_import_jump(forwarders.Pe32(raw), body)
            body = copy.deepcopy(f.bodies[0]); body["rva"] = "0x1001"
            with self.assertRaises(ValueError): forwarders.decode_import_jump(forwarders.Pe32(f.raw), body)

    def test_omitted_callers_owners_crossrefs_return_and_iat_claims_are_rejected(self):
        mutations = [lambda f: f.machine["observations"][0]["incoming_calls"].pop(),
            lambda f: f.machine["observations"][0].update(caller_owner_count=1),
            lambda f: f.machine["observations"][0]["incoming_calls"][0].update(return_rva="0x1026"),
            lambda f: f.machine["observations"][0]["incoming_calls"][0]["reciprocal_xref"].update(type=19),
            lambda f: f.machine["observations"][0]["original_import"].update(name="OtherPrint"),
            lambda f: f.machine["observations"][0]["original_import"].update(iat_rva="0x2084"),
            lambda f: f.machine["observations"][0].update(provider_executed=True),
            lambda f: f.machine["observations"][0]["function"].update(sha256="0" * 64)]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); mutation(f)
                with self.assertRaises(ValueError): self.prove(f)

    def test_rebound_truncated_owner_missing_xrefs_or_changed_instruction_still_fails(self):
        mutations = [lambda f: f.bodies[1]["assembly"].pop(),
            lambda f: f.bodies[1]["assembly"][0]["xrefs_from"].clear(),
            lambda f: f.bodies[0]["xrefs_to_entry"].pop(),
            lambda f: f.bodies[0]["assembly"][0]["xrefs_to"].pop(),
            lambda f: f.bodies[0]["assembly"][0]["xrefs_from"].clear(),
            lambda f: f.bodies[1]["assembly"][0].update(bytes="e8daffffff"),
            lambda f: f.bodies[1]["assembly"][0].update(rva="0x1021"),
            lambda f: f.bodies[0].update(type="ULONG(PCSTR Format)"),
            lambda f: f.bodies[0]["chunks"].append(copy.deepcopy(f.bodies[0]["chunks"][0]))]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); mutation(f); self.rebind(f)
                with self.assertRaises(ValueError): self.prove(f)

    def test_changed_inventory_identities_evidence_bytes_or_missing_original_failure_fail(self):
        mutations = [lambda f: f.capture["functions"].pop(),
            lambda f: f.capture.update(functions_total=2),
            lambda f: f.capture["functions"][1].update(sha256="0" * 64),
            lambda f: f.capture["failures"].clear(),
            lambda f: f.capture["failures"][0].update(error="no error"),
            lambda f: f.capture.update(image_base="0x500000"),
            lambda f: f.capture.update(capture_complete=False)]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); mutation(f)
                f.write(f.result / "capture.json", f.capture)
                with self.assertRaises(ValueError): self.prove(f)

    def test_ellipsis_failures_type_bytes_cleanup_and_actual_warning_observations_are_preserved(self):
        mutations = [lambda f: f.abi["trials"][0].update(ellipsis_preserved=False),
            lambda f: f.abi["trials"][0].update(declaration="ULONG(PCSTR)"),
            lambda f: f.abi["trials"][0]["failure"].update(code=0),
            lambda f: f.abi["trials"][0].update(temporary_removed=False),
            lambda f: f.abi["trials"].pop(),
            lambda f: f.matrix["observations"][0]["observation"]["after"]["entry_type"].update(serialized="changed"),
            lambda f: f.matrix["observations"][0]["observation"]["experiments"].pop(),
            lambda f: f.matrix["observations"][0]["observation"]["experiments"][0]["failure"].update(code=0)]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); mutation(f); f.machine["source_matrix"] = f.write(f.matrix_path, f.matrix)
                with self.assertRaises(ValueError): self.prove(f)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); f.matrix["observations"][0]["observation"]["experiments"][2]["warnings"].append({"id": 3, "text": "actual observed warning"})
            f.machine["source_matrix"] = f.write(f.matrix_path, f.matrix)
            self.assertTrue(self.prove(f)["sdk_evidence"]["actual_warning_id_3_emitted"])

    def test_review_api_has_separate_complete_forwarders_and_recomputes_all_proof_fields(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); machine = f.write(f.machine_path, f.machine); abi = f.write(f.abi_path, f.abi)
            with patch.object(forwarders, "source_identity", return_value={"path": "tools/native_import_forwarders.py", "bytes": 1, "sha256": "1" * 64}):
                review = forwarders.build_review(analysis, [(f.result, "0x1000")], machine, abi)
                path = f.root / ".analysis/results/review.json"; identity = f.write(path, review)
                result = forwarders.verify_review(analysis, identity)
                self.assertIn("0x1000", result["complete_forwarders"][f.capture["input_sha256"]])
                self.assertNotIn("complete_functions", result); self.assertNotIn("pseudocode", result)
                for mutation in (lambda r: r["proofs"][0]["normal_transfer"].update(stack_adjustment_bytes=4),
                                 lambda r: r["proofs"][0].update(pseudocode_recovered=True),
                                 lambda r: r.update(queue_status_modified=True)):
                    changed = copy.deepcopy(review); mutation(changed)
                    with self.assertRaises(ValueError): forwarders.verify_review(analysis, f.write(path, changed))

    def reviewed(self, f):
        review = forwarders.build_review(analysis, [(f.result, "0x1000")],
            f.write(f.machine_path, f.machine), f.write(f.abi_path, f.abi))
        identity = f.write(f.root / ".analysis/results/forwarder-review.json", review)
        args = SimpleNamespace(input=analysis.rel(f.source), review=identity["path"], review_sha256=identity["sha256"])
        return review, identity, args

    def resolution(self, f):
        return {"base_capture": evidence.identity(analysis, f.result / "capture.json"),
                "remaining_failures": copy.deepcopy(f.capture["failures"]), "resolved_failures": [],
                "verification_failures": [], "complete_functions": {}, "target_execution": False}

    def test_publication_is_explicit_source_bound_idempotent_and_preserves_original_capture(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), patch.object(
                forwarders, "source_identity", return_value={"path": "tools/native_import_forwarders.py", "bytes": 1, "sha256": "1" * 64}):
            f = self.fixture(Path(directory)); _, identity, args = self.reviewed(f)
            before = (f.result / "capture.json").read_bytes()
            published = forwarders.publish_binding(analysis, args)
            self.assertEqual(published["review"], identity)
            self.assertEqual(published["complete_forwarder_rvas"], ["0x1000"])
            self.assertFalse(published["queue_status_modified"])
            binding_path = f.result / "import-forwarder-review.json"
            binding = json.loads(binding_path.read_bytes())
            self.assertEqual(binding["source_input"], evidence.identity(analysis, f.source))
            self.assertEqual(binding["base_capture"], evidence.identity(analysis, f.result / "capture.json"))
            self.assertEqual(forwarders.publish_binding(analysis, args), published)
            self.assertEqual((f.result / "capture.json").read_bytes(), before)
            self.assertFalse(list(f.result.glob(".import-forwarder-*.tmp")))
            args.review_sha256 = "0" * 64
            with self.assertRaises(ValueError): forwarders.publish_binding(analysis, args)
            self.assertEqual(json.loads(binding_path.read_bytes()), binding)

    def test_publication_rejects_unreviewed_input_conflicts_and_changed_proofs_before_writing(self):
        for issue in ("wrong_input", "existing_binding", "changed_proof", "changed_input", "changed_review_sha"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), patch.object(
                    forwarders, "source_identity", return_value={"path": "tools/native_import_forwarders.py", "bytes": 1, "sha256": "1" * 64}):
                f = self.fixture(Path(directory)); review, _, args = self.reviewed(f)
                path = f.result / "import-forwarder-review.json"
                if issue == "wrong_input":
                    other = f.root / ".analysis/inputs/native/other.sys"; other.write_bytes(b"different original")
                    args.input = analysis.rel(other)
                elif issue == "existing_binding": f.write(path, {"previous": "valid decisions must not be overwritten"})
                elif issue == "changed_proof":
                    review["proofs"][0]["normal_transfer"]["memory_writes"] = ["arbitrary write"]
                    changed = f.write(f.root / args.review, review); args.review_sha256 = changed["sha256"]
                elif issue == "changed_input": f.source.write_bytes(f.raw + b"different")
                elif issue == "changed_review_sha": args.review_sha256 = "0" * 64
                previous = path.read_bytes() if path.exists() else None
                with self.assertRaises((ValueError, OSError)): forwarders.publish_binding(analysis, args)
                self.assertEqual(path.read_bytes() if path.exists() else None, previous)
                self.assertFalse(list(f.result.glob(".import-forwarder-*.tmp")))

    def test_publication_failures_and_cancellation_release_only_their_owned_temporary(self):
        for issue in ("fsync", "interrupt", "concurrent_conflict"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), patch.object(
                    forwarders, "source_identity", return_value={"path": "tools/native_import_forwarders.py", "bytes": 1, "sha256": "1" * 64}):
                f = self.fixture(Path(directory)); _, _, args = self.reviewed(f)
                destination = f.result / "import-forwarder-review.json"; before = (f.result / "capture.json").read_bytes()
                def race(*args):
                    destination.write_bytes(b'{"existing":"another publication"}')
                    raise FileExistsError()
                if issue == "fsync": mutation = patch.object(forwarders.os, "fsync", side_effect=OSError("disk write failed"))
                elif issue == "interrupt": mutation = patch.object(forwarders.os, "link", side_effect=KeyboardInterrupt())
                else: mutation = patch.object(forwarders.os, "link", side_effect=race)
                with mutation, self.assertRaises((OSError, ValueError, KeyboardInterrupt)):
                    forwarders.publish_binding(analysis, args)
                self.assertFalse(list(f.result.glob(".import-forwarder-*.tmp")))
                self.assertEqual((f.result / "capture.json").read_bytes(), before)
                if issue == "concurrent_conflict": self.assertEqual(json.loads(destination.read_bytes()), {"existing": "another publication"})
                else: self.assertFalse(destination.exists())

    def test_resolution_requires_a_binding_and_preserves_unrelated_errors_and_original_failure_text(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), patch.object(
                forwarders, "source_identity", return_value={"path": "tools/native_import_forwarders.py", "bytes": 1, "sha256": "1" * 64}):
            f = self.fixture(Path(directory)); _, _, args = self.reviewed(f); resolution = self.resolution(f)
            before = copy.deepcopy(resolution)
            self.assertEqual(forwarders.apply_resolution(analysis, f.result, resolution, f.capture), {})
            self.assertEqual(resolution, before)
            forwarders.publish_binding(analysis, args)
            unrelated = {"stage": "2040:hexrays", "error": "unrelated original failure"}
            resolution["remaining_failures"].append(unrelated)
            resolution["verification_failures"] = [{"supplement": "old-failed-attempt/capture.json", "error": "preserve this entire attempt"}]
            resolution["complete_functions"] = {"0x2040": {"capture": "already complete typed function"}}
            resolution["resolved_failures"] = [{"original_failure": {"stage": "older"}, "complete_function": "previous"}]
            verification = copy.deepcopy(resolution["verification_failures"]); functions = copy.deepcopy(resolution["complete_functions"])
            capture = copy.deepcopy(f.capture); raw = (f.result / "capture.json").read_bytes()
            applied = forwarders.apply_resolution(analysis, f.result, resolution, f.capture)
            self.assertEqual(set(applied), {"0x1000"})
            self.assertEqual(resolution["remaining_failures"], [unrelated])
            self.assertEqual(resolution["verification_failures"], verification)
            self.assertEqual(resolution["complete_functions"], functions)
            row = resolution["resolved_failures"][-1]
            self.assertEqual(row["original_failure"], f.capture["failures"][0])
            self.assertEqual(row["resolved_by"], "exact_original_import_jump")
            self.assertFalse(row["complete_forwarder"]["pseudocode_recovered"])
            self.assertEqual(f.capture, capture); self.assertEqual((f.result / "capture.json").read_bytes(), raw)
            snapshot = copy.deepcopy(resolution)
            forwarders.apply_resolution(analysis, f.result, resolution, f.capture)
            self.assertEqual(resolution, snapshot)

    def test_invalid_binding_changed_base_failure_or_existing_proof_never_resolves_anything(self):
        for issue in ("input", "base", "rva", "review", "claim", "failure", "in_memory_capture", "existing_proof"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), patch.object(
                    forwarders, "source_identity", return_value={"path": "tools/native_import_forwarders.py", "bytes": 1, "sha256": "1" * 64}):
                f = self.fixture(Path(directory)); _, _, args = self.reviewed(f)
                forwarders.publish_binding(analysis, args); resolution = self.resolution(f)
                path = f.result / "import-forwarder-review.json"; binding = json.loads(path.read_bytes())
                if issue == "input": binding["source_input"]["sha256"] = "0" * 64
                elif issue == "base": resolution["base_capture"]["sha256"] = "0" * 64
                elif issue == "rva": binding["function_rvas"] = ["0x1020"]
                elif issue == "review": binding["review"]["sha256"] = "0" * 64
                elif issue == "claim": binding["provider_semantics_complete"] = True
                elif issue == "failure": resolution["remaining_failures"][0]["error"] = "different failure original text"
                elif issue == "in_memory_capture": f.capture["image_base"] = "0x500000"
                elif issue == "existing_proof": resolution["complete_forwarders"] = {"0x1000": {"wrong": "proof"}}
                f.write(path, binding); before = copy.deepcopy(resolution)
                applied = forwarders.apply_resolution(analysis, f.result, resolution, f.capture)
                self.assertEqual(applied, {})
                self.assertEqual(resolution["remaining_failures"], before["remaining_failures"])
                self.assertEqual(resolution["resolved_failures"], before["resolved_failures"])
                self.assertEqual(resolution.get("complete_forwarders"), before.get("complete_forwarders"))
                self.assertEqual(len(resolution["verification_failures"]), 1)

    def test_old_artifact_coverage_requires_all_bytes_type_iat_and_caller_references_without_retiring_attempt(self):
        mutations = [None, lambda b: b.update(type="unsigned __int32(const char *Format, ...)"),
            lambda b: b["assembly"][0].update(bytes="ff2524804000"),
            lambda b: b["assembly"][0]["xrefs_from"].clear(),
            lambda b: b["xrefs_to_entry"].pop(), lambda b: b["assembly"][0]["xrefs_to"].pop(),
            lambda b: b.update(type_after_decompile="ULONG(PCSTR)"),
            lambda b: b.update(pseudocode={"lines": [{"text": "invented cfunc"}]}),
            lambda b: b["chunks"][0].update(end="0x401005")]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory)); proof = self.prove(f)
                old = copy.deepcopy(f.bodies[0])
                if mutation is not None: mutation(old)
                previous = f.write(f.result / "supplements/previous/functions/1000.json", old)
                if mutation is not None:
                    with self.assertRaises(ValueError): forwarders.previous_function_coverage(analysis, f.result, proof, previous)
                else:
                    coverage = forwarders.previous_function_coverage(analysis, f.result, proof, previous)
                    self.assertFalse(coverage["whole_attempt_retired"])
                    self.assertFalse(coverage["pseudocode_recovered"])
                    self.assertEqual(coverage["previous_function"], previous)
                    previous["sha256"] = "0" * 64
                    with self.assertRaises(ValueError): forwarders.previous_function_coverage(analysis, f.result, proof, previous)


if __name__ == "__main__":
    unittest.main()
