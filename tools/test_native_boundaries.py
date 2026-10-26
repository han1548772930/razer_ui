"""Exact native boundaries, explicit derivation passes and SDK failure cases."""
import ast
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
import native_boundaries as boundaries
import test_evidence


class ExistingSharedTailContextTests(unittest.TestCase):
    def fixture(self, root):
        base, raw = 0x10000000, bytearray(0x200)
        tail_code = b"\x0f\xb6\x01\x83\xc1\x08\x50\xe8" + struct.pack("<i", 0x1100 - 0x108c) + b"\xc3"
        raw[0x80:0x8d] = tail_code
        for start in (0x1000, 0x1020):
            raw[start - 0x1000:start - 0x1000 + 8] = b"\x8d\x4d\xe0\xe9" + struct.pack("<i", 0x1080 - start - 8)
        normal = b"\x55\x8b\xec\x8b\xd9\x8a\x45\x08\x8b\xe5\x5d\xc2\x04\x00"
        raw[0x100:0x100 + len(normal)] = normal
        raw = bytes(raw)
        pe = {"architecture": "x86", "image_base": hex(base),
              "sections": [{"rva": 0x1000, "bytes": len(raw), "file_offset": 0}]}

        def chunk(start, size):
            code = raw[start - 0x1000:start - 0x1000 + size]
            return {"start": hex(base + start), "end": hex(base + start + size),
                    "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}

        def instructions(start, lengths):
            rows = []
            for length in lengths:
                rows.append({"address": hex(base + start), "rva": hex(start), "text": "original instruction",
                    "bytes": raw[start - 0x1000:start - 0x1000 + length].hex(), "xrefs_to": [], "xrefs_from": []})
                start += length
            return rows

        tail = chunk(0x1080, 13)
        tail_rows = instructions(0x1080, [3, 3, 1, 5, 1])
        tail_rows[0]["xrefs_to"] = [{"from": hex(base + at), "to": hex(base + 0x1080),
            "type": 19, "code": True, "user": False} for at in (0x1003, 0x1023)]
        bodies = []
        for start in (0x1000, 0x1020, 0x1100):
            size = len(normal) if start == 0x1100 else 8
            rows = instructions(start, [1, 2, 2, 3, 2, 1, 3] if start == 0x1100 else [3, 5])
            bodies.append({"address": hex(base + start), "rva": hex(start), "end": hex(base + start + size),
                "flags": 0x5400, "chunks": [chunk(start, size), copy.deepcopy(tail)],
                "assembly": rows + copy.deepcopy(tail_rows)})
        source = root / ".analysis/inputs/cleanup.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)

        parents, records = [], []
        for body in bodies:
            record = write(result / "functions" / (body["rva"][2:] + ".json"), body)
            records.append({**record, "path": "functions/" + body["rva"][2:] + ".json",
                            "rva": body["rva"], "address": body["address"], "assembly_captured": True})
            parents.append({"owner_rva": body["rva"], "owner_function": record})
        capture = {"input": analysis.rel(source), "input_sha256": hashlib.sha256(raw).hexdigest(),
            "input_bytes": len(raw), "image_base": hex(base), "capture_complete": True, "functions": records}
        cap = write(result / "capture.json", capture)
        database = root / ".work/native" / capture["input_sha256"] / "batch/cleanup.dll.i64"
        database.parent.mkdir(parents=True)
        database.write_bytes(b"exact original static database")
        operation = write(root / ".analysis/results/proof.json", {"target_execution": False})
        review = {"schema_version": 1, "review_kind": "x86_existing_shared_tail_primary_context",
            "source_input": evidence.identity(analysis, source), "base_capture": cap,
            "source_database": evidence.identity(analysis, database), "owner_function": parents[0]["owner_function"],
            "callee_function": parents[2]["owner_function"], "tail": tail, "parent_functions": parents,
            "original_primary_owner_rva": "0x1020", "proof_operation": operation,
            "reason": "Complete exact parent-EBP cleanup and thiscall consumer with an existing shared tail",
            "target_execution": False, "raw_artifacts_modified": False}
        path = root / ".analysis/results/tail-review.json"
        write(path, review)
        types = [{"rva": "0x1000", "declaration": "void __usercall recovered(void *parent_frame@<ebp>);"},
                 {"rva": "0x1100", "declaration": "void __thiscall recovered(void *payload, unsigned char value_kind);"}]
        return SimpleNamespace(raw=raw, pe=pe, base=base, bodies=bodies, source=source, result=result,
            database=database, review=review, path=path, capture=capture, types=types, write=write)

    def select(self, fixture, selected=(0x1000, 0x1100)):
        with patch("host.inspect_pe", return_value=fixture.pe):
            return boundaries.select_shared_tail_contexts(analysis, fixture.raw, fixture.capture, selected,
                [analysis.rel(fixture.path)], fixture.types)[0]

    def sdk(self, fixture, issue=None):
        functions = {int(body["address"], 16): SimpleNamespace(start_ea=int(body["address"], 16),
            end_ea=int(body["end"], 16), flags=0x5400, frame=index, frsize=0, frregs=0, argsize=0, fpd=0)
            for index, body in enumerate(fixture.bodies)}
        by_address = {int(body["address"], 16): body for body in fixture.bodies}
        tail = SimpleNamespace(start_ea=fixture.base + 0x1080, end_ea=fixture.base + 0x108d,
                               owner=fixture.base + 0x1020, flags=0x8000, refqty=3)
        points, calls = {}, []
        memory = bytearray(fixture.raw)

        class Type:
            def __str__(self): return "unchanged reviewed function type"

        def set_owner(item, owner):
            calls.append(("set_tail_owner", owner))
            if issue == "select" and len(calls) == 1 or issue == "restore" and owner == fixture.base + 0x1020:
                return False
            item.owner = owner
            return True

        def reanalyze(function):
            calls.append(("reanalyze_function", function.start_ea))
            points[(function.start_ea, fixture.base + 0x1087)] = (-4, -4)
            points[(function.start_ea, fixture.base + 0x108c)] = (0, 4)
            if issue == "frame": functions[fixture.base + 0x1100].frsize = 8
            if issue == "bytes": memory[0x80] = 0x90

        def update(function):
            calls.append(("update_func", function.start_ea))
            if issue == "readback": function.flags |= 0x400
            return issue != "update"

        sdk = SimpleNamespace(root=analysis.ROOT, bytes=SimpleNamespace(
            get_bytes=lambda ea, size: bytes(memory[ea - fixture.base - 0x1000:ea - fixture.base - 0x1000 + size])),
            funcs=SimpleNamespace(get_func=lambda ea: functions.get(ea), get_fchunk=lambda ea: tail,
                func_parent_iterator_t=lambda item: iter(sorted(functions)[:-1] if issue == "parents" else sorted(functions)),
                set_tail_owner=set_owner, FUNC_TAIL=0x8000, FUNC_SP_READY=0x400,
                reanalyze_function=reanalyze, update_func=update),
            utils=SimpleNamespace(Chunks=lambda ea: [(int(c["start"], 16), int(c["end"], 16))
                                                    for c in by_address[ea]["chunks"]]),
            frame=SimpleNamespace(get_spd=lambda fn, ea: points.get((fn.start_ea, ea), (0, 0))[0],
                                  get_sp_delta=lambda fn, ea: points.get((fn.start_ea, ea), (0, 0))[1]),
            types=SimpleNamespace(tinfo_t=Type), nalt=SimpleNamespace(get_tinfo=lambda typ, ea: True),
            auto=SimpleNamespace(auto_wait=lambda: issue != "cancel"),
            hexrays=SimpleNamespace(clear_cached_cfuncs=lambda: calls.append(("clear_cached_cfuncs",))))
        return sdk, tail, calls

    def test_complete_original_tail_parent_set_and_effect_types_are_bound(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.fixture(Path(directory))
            request = self.select(fixture)
            self.assertEqual(len(request["parent_functions"]), 3)
            self.assertEqual(request["proof"]["parent_value_offset"], -32)
            self.assertEqual(request["proof"]["callsite_rva"], "0x1087")
            self.assertEqual(request["proof"]["callee_pops_stack_bytes"], 4)

    def test_supplied_stack_values_missing_parents_wrong_types_and_source_changes_are_rejected(self):
        mutations = [lambda f: f.review.update(stack_delta=-4),
                     lambda f: f.review["parent_functions"].pop(),
                     lambda f: f.review.update(original_primary_owner_rva="0x1000"),
                     lambda f: f.types[1].update(declaration="int __cdecl recovered(void *payload);"),
                     lambda f: f.database.write_bytes(b"changed database"),
                     lambda f: f.review["tail"].update(raw_hex="90")]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory))
                mutation(fixture)
                fixture.write(fixture.path, fixture.review)
                with self.assertRaises((ValueError, OSError)): self.select(fixture)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.fixture(Path(directory))
            with self.assertRaises(ValueError): self.select(fixture, (0x1000,))

    def test_original_machine_proof_rejects_wrong_ebp_input_call_ret_and_kind_read(self):
        for body_index, key, value in [(0, "rva", "0x1010"), (0, "bytes", "8d45e0"),
                                      (2, "bytes", "8b4508")]:
            with self.subTest(key=key, value=value), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory))
                owner, callee = copy.deepcopy(fixture.bodies[0]), copy.deepcopy(fixture.bodies[2])
                body = owner if body_index == 0 else callee
                if key == "rva": body["assembly"][0][key] = value
                else: body["assembly"][0 if body_index == 0 else 3][key] = value
                with patch("host.inspect_pe", return_value=fixture.pe), self.assertRaises(ValueError):
                    boundaries.shared_tail_context_proof(fixture.raw, owner, callee)

    def test_context_holds_owner_through_capture_and_restores_without_supplied_sp(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.fixture(Path(directory)); request = self.select(fixture)
            sdk, tail, calls = self.sdk(fixture); audit = {}
            with boundaries.shared_tail_context(request, fixture.base, sdk, audit):
                self.assertEqual(tail.owner, fixture.base + 0x1000)
                self.assertEqual(sdk.frame.get_spd(sdk.funcs.get_func(fixture.base + 0x1000), fixture.base + 0x1087), -4)
            self.assertEqual(tail.owner, fixture.base + 0x1020)
            self.assertEqual([call for call in calls if call[0] == "set_tail_owner"],
                             [("set_tail_owner", fixture.base + 0x1000), ("set_tail_owner", fixture.base + 0x1020)])
            boundaries.verify_shared_tail_contexts(analysis, [request], [audit])

    def test_failures_cancellation_and_keyboard_interrupt_always_restore_the_existing_primary(self):
        for issue in ("select", "update", "readback", "cancel", "bytes", "interrupt"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory)); request = self.select(fixture)
                sdk, tail, calls = self.sdk(fixture, issue); audit = {}
                with self.assertRaises((ValueError, KeyboardInterrupt)):
                    with boundaries.shared_tail_context(request, fixture.base, sdk, audit):
                        raise KeyboardInterrupt()
                self.assertEqual(tail.owner, fixture.base + 0x1020)
                self.assertIn(("set_tail_owner", fixture.base + 0x1020), calls)

    def test_missing_parents_restore_failures_and_forged_or_changed_frames_remain_failed(self):
        for issue in ("parents", "restore"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory)); request = self.select(fixture)
                sdk, _, _ = self.sdk(fixture, issue); audit = {}
                with self.assertRaises(ValueError):
                    with boundaries.shared_tail_context(request, fixture.base, sdk, audit): pass
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.fixture(Path(directory)); request = self.select(fixture)
            sdk, _, _ = self.sdk(fixture, "frame"); audit = {}
            with boundaries.shared_tail_context(request, fixture.base, sdk, audit): pass
            with self.assertRaises(ValueError): boundaries.verify_shared_tail_contexts(analysis, [request], [audit])
            sdk, _, _ = self.sdk(fixture); audit = {}
            with boundaries.shared_tail_context(request, fixture.base, sdk, audit): pass
            for mutate in (lambda row: row.update(restored=False),
                           lambda row: row["captured"].update(primary_owner=hex(fixture.base + 0x1020)),
                           lambda row: row["after"]["parents"].pop(),
                           lambda row: row["captured"]["functions"][0].update(flags=0x5401),
                           lambda row: row["selected"]["functions"][0]["stack_points"][3].update(spd=-4),
                           lambda row: row["analyzed"]["functions"][0]["stack_points"][0].update(spd=-4)):
                bad = copy.deepcopy(audit); mutate(bad)
                with self.assertRaises(ValueError): boundaries.verify_shared_tail_contexts(analysis, [request], [bad])


class DerivedArmStackTests(unittest.TestCase):
    def fixture(self, root):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        struct.pack_into("<H", raw, 0x84, 0xaa64)
        base = 0x140000000
        owner_words = [0xd503237f, 0xa9bf7bfd, 0x910003fd, 0xd280042f,
                       0x9400001c, 0xcb2f73ff, 0x910103e8, 0xf90003ff, 0x94000006]
        probe_words = [0xf9400a51, 0xeb2f73f0, 0x9a9033f0, 0xeb11021f,
                       0x54000043, 0xd65f03c0, 0x9274ce10, 0xd1400631,
                       0xf940023f, 0xeb10023f, 0x54ffffa1, 0xd65f03c0]
        bodies = []
        for start, words in [(0x1000, owner_words), (0x1080, probe_words)]:
            code = b"".join(struct.pack("<I", word) for word in words)
            raw[0x200 + start - 0x1000:0x200 + start - 0x1000 + len(code)] = code
            assembly = [{"address": hex(base + start + i * 4), "rva": hex(start + i * 4),
                         "bytes": struct.pack("<I", word).hex(), "text": "retained instruction"}
                        for i, word in enumerate(words)]
            bodies.append({"address": hex(base + start), "rva": hex(start), "end": hex(base + start + len(code)),
                "flags": 1 if start == 0x1000 else 0, "assembly": assembly,
                "chunks": [{"start": hex(base + start), "end": hex(base + start + len(code)),
                            "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}]})
        raw = bytes(raw)
        source = root / ".analysis/inputs/stack.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)

        records = []
        for body in bodies:
            identity = write(result / "functions" / (body["rva"][2:] + ".json"), body)
            records.append({**identity, "path": "functions/" + body["rva"][2:] + ".json",
                "rva": body["rva"], "address": body["address"], "assembly_captured": True})
        capture = {"input": analysis.rel(source), "input_sha256": hashlib.sha256(raw).hexdigest(),
            "image_base": hex(base), "functions": records}
        cap = write(result / "capture.json", capture)
        owner, probe = bodies
        proof = boundaries.arm64_probe_stack_proof(raw, owner, probe)
        review = {"schema_version": 1, "review_kind": "arm64_complete_probe_derived_stack_point",
            "source_input": evidence.identity(analysis, source), "base_capture": cap,
            "owner_function": evidence.identity(analysis, result / "functions/1000.json"),
            "probe_function": evidence.identity(analysis, result / "functions/1080.json"),
            "proof": proof, "target_execution": False, "raw_artifacts_modified": False,
            "reason": "Complete original ARM64 allocation and preserving probe"}
        path = root / ".analysis/results/stack-review.json"
        write(path, review)
        return SimpleNamespace(raw=raw, owner=owner, probe=probe, proof=proof, review=review,
            path=path, write=write, capture=capture)

    def snapshots(self, f):
        old = {"address": f.owner["address"], "end": f.owner["end"], "chunks": f.owner["chunks"],
            "flags": 1, "type_present": True, "type": "void __usercall __noreturn()",
            "native_frame": {"frame": 4, "frsize": 0, "frregs": 16, "argsize": 0, "fpd": 0},
            "stack_points": [{"ea": row["address"], "spd": 0 if i < 2 else -16,
                              "delta": -16 if i == 2 else 0} for i, row in enumerate(f.owner["assembly"])]}
        new = copy.deepcopy(old)
        for i, point in enumerate(new["stack_points"]):
            if i >= 6:
                point["spd"] = -544
            if i == 6:
                point["delta"] = -528
        return old, new

    def test_complete_machine_proof_derives_the_post_instruction_point(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            self.assertEqual((f.proof["units"], f.proof["shift"], f.proof["delta"]), (33, 4, -528))
            self.assertEqual(f.proof["point_rva"], "0x1018")
            rows = boundaries.select_stack_points(analysis, f.raw, f.capture, [0x1000], [analysis.rel(f.path)])
            self.assertEqual(rows[0]["proof"], f.proof)
            old, new = self.snapshots(f)
            boundaries._verify_stack_point_snapshots(f.owner, f.proof, old, new)

    def test_probe_clobber_branch_definition_and_instruction_gaps_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for issue in ("probe", "mov", "shift", "call", "branch", "entry", "noret", "gap"):
                with self.subTest(issue=issue):
                    owner, probe, raw = copy.deepcopy(f.owner), copy.deepcopy(f.probe), bytearray(f.raw)
                    if issue == "entry": owner["assembly"][3]["xrefs_to"] = [{"from": "0x140001020", "code": True}]
                    elif issue == "noret": owner["flags"] = 0
                    elif issue == "gap": owner["assembly"].pop(2)
                    else:
                        body, index, word = (probe, 0, 0xf9400a4f) if issue == "probe" else {
                            "mov": (owner, 3, 0xd2800430), "shift": (owner, 5, 0xcb2f6fff),
                            "call": (owner, 4, 0x9400001d), "branch": (owner, 6, 0x14000002)}[issue]
                        code = struct.pack("<I", word)
                        body["assembly"][index]["bytes"] = code.hex()
                        offset = 0x200 + int(body["assembly"][index]["rva"], 16) - 0x1000
                        raw[offset:offset + 4] = code
                        chunk = body["chunks"][0]
                        at = 0x200 + int(body["rva"], 16) - 0x1000
                        original = bytes(raw[at:at + int(body["end"], 16) - int(body["address"], 16)])
                        chunk.update(raw_hex=original.hex(), sha256=hashlib.sha256(original).hexdigest())
                    with self.assertRaises(ValueError): boundaries.arm64_probe_stack_proof(bytes(raw), owner, probe)

    def test_point_readback_rejects_wrong_coordinate_and_unrelated_frame_changes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for issue in ("early", "delta", "spd", "before", "frame", "type", "flags", "missing"):
                with self.subTest(issue=issue), self.assertRaises(ValueError):
                    old, new = self.snapshots(f)
                    if issue == "early": new["stack_points"][5]["spd"] = -544
                    elif issue == "delta": new["stack_points"][6]["delta"] = -527
                    elif issue == "spd": new["stack_points"][7]["spd"] = -543
                    elif issue == "before": old["stack_points"][6]["delta"] = -528
                    elif issue == "frame": new["native_frame"]["frsize"] = 528
                    elif issue == "type": new["type"] = "void ()"
                    elif issue == "flags": new["flags"] = 5
                    elif issue == "missing": new["stack_points"].pop()
                    boundaries._verify_stack_point_snapshots(f.owner, f.proof, old, new)

    def test_review_cannot_supply_a_stack_value_or_select_an_unrelated_owner(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for issue in ("delta", "probe", "scope", "foreign", "duplicate"):
                with self.subTest(issue=issue), self.assertRaises(ValueError):
                    review = copy.deepcopy(f.review)
                    if issue == "delta": review["proof"]["delta"] = -1024
                    elif issue == "probe": review["probe_function"]["sha256"] = "0" * 64
                    elif issue == "scope": review["target_execution"] = True
                    f.write(f.path, review)
                    selected = [0x1080] if issue == "foreign" else [0x1000]
                    specs = [analysis.rel(f.path)] * (2 if issue == "duplicate" else 1)
                    boundaries.select_stack_points(analysis, f.raw, f.capture, selected, specs)


class NativeBoundaryTests(unittest.TestCase):
    def fixture(self, root, mutation=None):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        struct.pack_into("<I", raw, 0x98 + 108, 16)
        struct.pack_into("<II", raw, 0x98 + 112 + 3 * 8, 0x2000, 48)
        raw[0x200:0x206] = b"\xe8\x3b\0\0\0\xc3"
        raw[0x210:0x215] = b"\xe9\x2b\0\0\0"
        raw[0x240:0x244] = b"\x90\x90\x90\xc3"
        triples = [(0x1000, 0x1006, 0x2080), (0x1010, 0x1015, 0x2080),
                   (0x1040, 0x1042, 0x2084), (0x1042, 0x1044, 0x2094)]
        for index, triple in enumerate(triples):
            struct.pack_into("<III", raw, 0x300 + index * 12, *triple)
        raw[0x380:0x384] = b"\x01\0\0\0"
        raw[0x384:0x394] = b"\x21\0\0\0" + struct.pack("<III", 0x1042, 0x1044, 0x2094)
        raw[0x394:0x398] = b"\x01\0\0\0"
        if mutation:
            mutation(raw)
        raw = bytes(raw)
        digest, base = hashlib.sha256(raw).hexdigest(), 0x140000000
        result = root / ".analysis/results/native" / digest
        original = root / ".analysis/inputs/native/chained.dll"
        original.parent.mkdir(parents=True)
        original.write_bytes(raw)

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)

        def chunk(start, end):
            code = raw[0x200 + start - 0x1000:0x200 + end - 0x1000]
            return {"start": hex(base + start), "end": hex(base + end), "raw_hex": code.hex(),
                    "sha256": hashlib.sha256(code).hexdigest()}

        def insn(start, length, text):
            return {"address": hex(base + start), "rva": hex(start),
                    "bytes": raw[0x200 + start - 0x1000:0x200 + start - 0x1000 + length].hex(), "text": text}

        tail = [insn(0x1040, 1, "nop"), insn(0x1041, 1, "nop")]
        bodies, records = {}, []
        for start, end, head in [(0x1000, 0x1006, [insn(0x1000, 5, "call 0x140001040"), insn(0x1005, 1, "ret")]),
                                 (0x1010, 0x1015, [insn(0x1010, 5, "jmp 0x140001040")])]:
            body = {"address": hex(base + start), "rva": hex(start), "end": hex(base + end), "flags": 0x5400,
                    "chunks": [chunk(start, end), chunk(0x1040, 0x1042)], "assembly": head + copy.deepcopy(tail)}
            bodies[hex(start)] = body
            record = write(result / "functions" / (hex(start)[2:] + ".json"), body)
            records.append({**record, "path": "functions/" + hex(start)[2:] + ".json", "rva": hex(start),
                            "address": body["address"], "assembly_captured": True, "pseudocode_captured": False})
        capture = {"input": analysis.rel(original), "input_sha256": digest, "input_bytes": len(raw),
                   "image_base": hex(base), "capture_complete": True, "functions": records, "failures": []}
        write(result / "capture.json", capture)
        entries = [{"table_entry_rva": hex(0x2000 + index * 12), "raw_hex": struct.pack("<III", *triple).hex(),
                    "begin_rva": hex(triple[0]), "end_rva": hex(triple[1]), "unwind_rva": hex(triple[2])}
                   for index, triple in enumerate(triples)]
        code = raw[0x240:0x244]
        review = {"schema_version": 1, "review_kind": "native_x64_chained_runtime_helper_complete_bytes",
            "source_input": evidence.identity(analysis, original),
            "base_capture": evidence.identity(analysis, result / "capture.json"),
            "owner_function": evidence.identity(analysis, result / "functions/1000.json"),
            "callsite": bodies["0x1000"]["assembly"][0],
            "exception_directory": {"rva": "0x2000", "bytes": 48, "raw_sha256": hashlib.sha256(raw[0x300:0x330]).hexdigest(),
                                    "entries": [entries[0], entries[2], entries[3]]},
            "helper": {"start_rva": "0x1040", "end_rva": "0x1044", "bytes": 4, "raw_hex": code.hex(),
                "sha256": hashlib.sha256(code).hexdigest(), "complete_linear_instruction_bytes_verified": True,
                "ret_rva": "0x1043", "assembly": tail + [insn(0x1042, 1, "nop"), insn(0x1043, 1, "ret")],
                "unwind_chain": {"unwind_rva": "0x2084", "version": 1, "flags": 4, "prolog_bytes": 0,
                    "code_slots": 0, "frame_register": 0, "frame_offset_units": 0, "raw_hex": raw[0x384:0x394].hex(),
                    "chain_triple": ["0x1042", "0x1044", "0x2094"], "terminal_unwind_raw_hex": "01000000"}},
            "missing_retained_range": {"start_rva": "0x1042", "end_rva": "0x1044", "bytes": 2,
                "raw_hex": raw[0x242:0x244].hex(), "sha256": hashlib.sha256(raw[0x242:0x244]).hexdigest(),
                "base_function_owners": []}, "target_execution": False, "raw_artifacts_modified": False}
        review_path = root / ".analysis/results/chained-review.json"
        write(review_path, review)
        return SimpleNamespace(raw=raw, base=base, capture=capture, result=result, bodies=bodies,
            original=original, review=review, review_path=review_path, write=write, specification=analysis.rel(review_path))

    def select(self, f, selected=(0x1000, 0x1010)):
        return boundaries.select(analysis, f.raw, f.capture, selected, [f.specification])

    def test_full_original_bytes_chain_and_every_shared_owner_are_bound(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            original = (f.result / "functions/1000.json").read_bytes()
            row = self.select(f)[0]
            self.assertEqual([item["owner_rva"] for item in row["shared_owners"]], ["0x1000", "0x1010"])
            self.assertEqual(row["helper"]["raw_hex"], "909090c3")
            self.assertEqual(len(row["unwind_chain"]), 2)
            self.assertEqual(len(row["exception_directory"]["entries"]), 4)
            self.assertEqual(len(boundaries.owner_body(analysis, row)["chunks"]), 1)
            self.assertEqual(len(boundaries.owner_body(analysis, row, "0x1010")["assembly"]), 1)
            self.assertEqual(boundaries.helper_body(row)["chunks"][0]["sha256"], hashlib.sha256(b"\x90\x90\x90\xc3").hexdigest())
            self.assertEqual((f.result / "functions/1000.json").read_bytes(), original)

    def test_review_accepts_an_exact_root_or_pointer_without_changing_selection(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            self.assertEqual(self.select(f)[0]["review_source"]["pointer"], "")
            f.write(f.review_path, {"reviews": [f.review]})
            f.specification += "#/reviews/0"
            self.assertEqual(self.select(f)[0]["review_source"]["pointer"], "/reviews/0")
            for suffix in ("#/reviews/00", "#/reviews/9", "#bad", "#/reviews~2/0"):
                with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                    boundaries.select(analysis, f.raw, f.capture, (0x1000, 0x1010), [analysis.rel(f.review_path) + suffix])

    def test_unselected_shared_owner_and_duplicate_or_unbounded_reviews_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            with self.assertRaises(ValueError): self.select(f, (0x1000,))
            for specs in ([], [f.specification] * 2, [f.specification + str(n) for n in range(65)]):
                with self.subTest(count=len(specs)), self.assertRaises(ValueError):
                    boundaries.select(analysis, f.raw, f.capture, (0x1000, 0x1010), specs)

    def test_changed_input_capture_owner_review_and_missing_instruction_are_rejected(self):
        for issue in ("raw", "source", "owner", "capture", "byte", "ret", "gap", "target", "chain", "table", "missing", "execution"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "raw": f.raw += b"changed"
                elif issue == "source": f.original.write_bytes(b"changed source")
                elif issue == "owner": (f.result / "functions/1000.json").write_bytes(b"changed owner")
                elif issue == "capture": f.review["base_capture"]["sha256"] = "0" * 64
                elif issue == "byte": f.review["helper"]["raw_hex"] = "90909090"
                elif issue == "ret": f.review["helper"]["ret_rva"] = "0x1042"
                elif issue == "gap": f.review["helper"]["assembly"].pop(1)
                elif issue == "target": f.review["callsite"] = {**f.review["callsite"], "bytes": "e93b000000"}
                elif issue == "chain": f.review["helper"]["unwind_chain"]["chain_triple"][0] = "0x1043"
                elif issue == "table": f.review["exception_directory"]["raw_sha256"] = "0" * 64
                elif issue == "missing": f.review["missing_retained_range"]["base_function_owners"] = ["0x1010"]
                else: f.review["target_execution"] = True
                f.write(f.review_path, f.review)
                with self.assertRaises(ValueError): self.select(f)

    def test_malformed_table_and_noncontiguous_cyclic_or_foreign_chain_are_rejected(self):
        mutations = {
            "x86": lambda raw: struct.pack_into("<H", raw, 0x84, 0x14c),
            "partial": lambda raw: struct.pack_into("<I", raw, 0x98 + 112 + 3 * 8 + 4, 47),
            "overlap": lambda raw: struct.pack_into("<I", raw, 0x300 + 3 * 12, 0x1041),
            "nonexecutable": lambda raw: struct.pack_into("<I", raw, 0x188 + 36, 0x40000040),
            "handler": lambda raw: raw.__setitem__(0x384, 0x29),
            "version": lambda raw: raw.__setitem__(0x384, 0x22),
            "unbacked": lambda raw: struct.pack_into("<I", raw, 0x384 + 12, 0x3000),
            "cycle": lambda raw: struct.pack_into("<III", raw, 0x388, 0x1040, 0x1042, 0x2084),
            "gap": lambda raw: struct.pack_into("<III", raw, 0x388, 0x1043, 0x1044, 0x2094),
        }
        for issue, mutation in mutations.items():
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), mutation)
                with self.assertRaises(ValueError): self.select(f)

    def sdk(self, f, row, issue=None):
        calls, memory, parent_locks = [], bytearray(f.raw), [0]
        functions = {int(body["address"], 16): SimpleNamespace(start_ea=int(body["address"], 16),
            end_ea=int(body["end"], 16), flags=body.get("flags", 0x5400), frame=7, frsize=32, frregs=8, argsize=0, fpd=0)
            for body in f.bodies.values()}
        chunks = {ea: [(int(c["start"], 16), int(c["end"], 16)) for c in f.bodies[hex(ea - f.base)]["chunks"]]
                  for ea in functions}
        start = f.base + 0x1040

        def get_func(ea):
            for owner, ranges in chunks.items():
                if any(lo <= ea < hi for lo, hi in ranges): return functions[owner]
            return None

        def get_bytes(ea, size):
            at = 0x200 + ea - f.base - 0x1000
            return bytes(memory[at:at + size])

        class Parents:
            def __init__(self): parent_locks[0] += 1
            def __del__(self): parent_locks[0] -= 1
            def set(self, tail): return issue != "parents_missing"
            def __iter__(self):
                values = [ea for ea, ranges in chunks.items() if (start, start + 2) in ranges]
                return iter(values + ([f.base + 0x1020] if issue == "extra_parent" else []))

        def remove(owner, ea):
            calls.append(("remove", owner.start_ea))
            if parent_locks[0]: raise AssertionError("Cannot mutate a tail while its parent iterator owns a range lock")
            if issue == "remove": return False
            chunks[owner.start_ea] = [c for c in chunks[owner.start_ea] if c[0] != ea]
            return True

        def decode(out, ea):
            out.size, out.itype = 1, 42
            return 0 if issue == "decode" else 1

        def add(ea, end):
            calls.append(("add", ea, end))
            if issue == "add": return False
            functions[ea] = SimpleNamespace(start_ea=ea, end_ea=end - (issue == "boundary"),
                flags=0x5400, frame=9, frsize=0, frregs=0, argsize=0, fpd=0)
            chunks[ea] = [(ea, end)]
            return True

        def update(function):
            calls.append(("update", function.start_ea))
            if issue == "flag_readback": function.flags |= 0x400
            return issue != "update"

        def reanalyze(function):
            calls.append(("reanalyze", function.start_ea))
            if issue == "changed_bytes": memory[0x240] = 0xc3

        sdk = SimpleNamespace(root=analysis.ROOT, bytes=SimpleNamespace(get_bytes=get_bytes),
            funcs=SimpleNamespace(get_func=get_func, get_fchunk=lambda ea: SimpleNamespace(start_ea=ea, end_ea=ea + 2),
                func_parent_iterator_t=Parents, remove_func_tail=remove, add_func=add, update_func=update,
                reanalyze_function=reanalyze, FUNC_SP_READY=0x400),
            utils=SimpleNamespace(Chunks=lambda ea: chunks[ea]),
            ua=SimpleNamespace(insn_t=SimpleNamespace, decode_insn=decode,
                create_insn=lambda ea: 0 if issue == "create" else 1),
            auto=SimpleNamespace(auto_wait=lambda: issue != "cancel"),
            frame=SimpleNamespace(get_spd=lambda function, ea: 0, get_sp_delta=lambda function, ea: 0),
            types=SimpleNamespace(tinfo_t=SimpleNamespace), nalt=SimpleNamespace(get_tinfo=lambda tif, ea: False))
        if issue == "bytes": memory[0x242] = 0xcc
        if issue == "occupied":
            functions[start + 2] = SimpleNamespace(start_ea=start + 2, end_ea=start + 4)
            chunks[start + 2] = [(start + 2, start + 4)]
        return sdk, calls

    def test_sdk_detaches_every_shared_tail_and_cpu_decodes_every_original_byte(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            row = self.select(f)[0]
            sdk, calls = self.sdk(f, row)
            applied = boundaries.apply(row, f.base, sdk)
            boundaries.verify_applied(analysis, [row], [applied])
            self.assertEqual(applied["removed_owner_rvas"], ["0x1000", "0x1010"])
            self.assertEqual(len(applied["cpu_decoded_instructions"]), 4)
            self.assertEqual(len(applied["automatic_reanalysis"]), 3)
            self.assertEqual(calls[:3], [("remove", f.base + 0x1000), ("remove", f.base + 0x1010),
                                         ("add", f.base + 0x1040, f.base + 0x1044)])
            self.assertFalse(applied["manual_stack_points_modified"])
            self.assertFalse(applied["manual_frames_modified"])
            self.assertFalse(applied["instructions_patched"])

    def test_sdk_stops_on_changed_sources_ownership_cpu_decode_or_analysis_failure(self):
        for issue in ("bytes", "occupied", "extra_parent", "parents_missing", "decode", "remove", "create",
                      "add", "boundary", "update", "flag_readback", "changed_bytes", "cancel", "source"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                row = self.select(f)[0]
                sdk, calls = self.sdk(f, row, issue)
                if issue == "source": f.review_path.write_bytes(b"changed review")
                with self.assertRaises(ValueError): boundaries.apply(row, f.base, sdk)
                if issue in ("bytes", "occupied", "extra_parent", "parents_missing", "decode", "source"):
                    self.assertFalse(calls)

    def test_verifier_rejects_incomplete_or_forged_boundary_observations(self):
        for issue in ("missing", "duplicate", "request", "owner", "parent", "cpu", "frame", "stack", "type", "flag", "cancel", "manual"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                row = self.select(f)[0]
                sdk, _ = self.sdk(f, row)
                observed = copy.deepcopy(boundaries.apply(row, f.base, sdk))
                applied = [observed]
                if issue == "missing": applied = []
                elif issue == "duplicate": applied *= 2
                elif issue == "request": observed["helper"]["sha256"] = "0" * 64
                elif issue == "owner": observed["removed_owner_rvas"].pop()
                elif issue == "parent": observed["original_shared_tail_parents"].pop()
                elif issue == "cpu": observed["cpu_decoded_instructions"].pop()
                elif issue == "frame": observed["after"]["helper"]["native_frame"]["fpd"] = False
                elif issue == "stack": observed["after"]["owners"][0]["snapshot"]["stack_points"].pop()
                elif issue == "type": observed["after"]["helper"]["type"] = "invented type"
                elif issue == "flag": observed["automatic_reanalysis"][0]["invalidated_flags"] = 0x5400
                elif issue == "cancel": observed["auto_wait_completed"] = False
                else: observed["manual_stack_points_modified"] = True
                with self.assertRaises(ValueError): boundaries.verify_applied(analysis, [row], applied)

    def test_source_retention_keeps_an_immutable_validator_and_rejects_changed_bytes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            path = root / "tools/native_boundaries.py"
            path.parent.mkdir(parents=True)
            path.write_bytes(boundaries._SOURCE)
            record = evidence.identity(analysis, path)
            retained = boundaries.retain_source(analysis, record)
            path.write_bytes(b"next validator version")
            self.assertEqual(boundaries.retain_source(analysis, record), retained)
            (root / retained["path"]).write_bytes(b"changed retained validator")
            with self.assertRaises(ValueError): boundaries.retain_source(analysis, record)

    def instruction_fixture(self, root):
        f = self.fixture(root)
        raw = bytearray(f.raw)
        raw[0x200:0x211] = b"\xe9\0\0\0\0\xe8\x36\0\0\0\x90\x0f\x82\xf4\xff\xff\xff"
        f.raw = bytes(raw)
        f.original.write_bytes(f.raw)
        digest = hashlib.sha256(f.raw).hexdigest()
        f.result = root / ".analysis/results/native" / digest
        base = f.base

        def insn(rva, code, text):
            return {"address": hex(base + rva), "rva": hex(rva), "bytes": code.hex(), "text": text,
                    "xrefs_from": [], "xrefs_to": [], "comment": None, "repeatable_comment": None}

        branch = insn(0x100b, f.raw[0x20b:0x211], "jb 0x140001005")
        branch["xrefs_from"] = [{"from": branch["address"], "to": hex(base + 0x1005),
                                 "type": 19, "code": True, "user": False}]
        assembly = [insn(0x1000, f.raw[0x200:0x205], "jmp 0x140001005"),
                    insn(0x1006, f.raw[0x206:0x207], "other"),
                    insn(0x1007, f.raw[0x207:0x208], "other"),
                    insn(0x1008, f.raw[0x208:0x20a], "other"), insn(0x100a, b"\x90", "nop"), branch]
        assembly[1]["comment"] = "original displaced comment"
        assembly[1]["xrefs_to"] = [{"from": hex(base + 0x2050), "to": assembly[1]["address"],
                                     "type": 1, "code": False, "user": False}]

        def body(rva, end, instructions):
            code = f.raw[0x200 + rva - 0x1000:0x200 + end - 0x1000]
            return {"rva": hex(rva), "address": hex(base + rva), "end": hex(base + end), "flags": 0x5400,
                    "chunks": [{"start": hex(base + rva), "end": hex(base + end), "raw_hex": code.hex(),
                                "sha256": hashlib.sha256(code).hexdigest()}], "assembly": instructions}

        owner = body(0x1000, 0x1011, assembly)
        callee = body(0x1040, 0x1044, [insn(0x1040 + i, f.raw[0x240 + i:0x241 + i], "nop" if i < 3 else "ret") for i in range(4)])
        f.bodies = {"0x1000": owner, "0x1040": callee}
        records = []
        for rva, value in f.bodies.items():
            path = f.result / "functions" / (rva[2:] + ".json")
            record = f.write(path, value)
            records.append({**record, "path": "functions/" + rva[2:] + ".json", "rva": rva,
                            "address": value["address"], "assembly_captured": True})
        f.capture.update(input_sha256=digest, input_bytes=len(f.raw), functions=records)
        f.write(f.result / "capture.json", f.capture)
        f.review = {"schema_version": 1, "review_kind": "native_exact_e8_instruction_boundary",
            "source_input": evidence.identity(analysis, f.original),
            "base_capture": evidence.identity(analysis, f.result / "capture.json"),
            "owner_rva": "0x1000", "owner_function": evidence.identity(analysis, f.result / "functions/1000.json"),
            "instruction_rva": "0x1005", "raw_hex": "e836000000", "callee_rva": "0x1040",
            "callee_function": evidence.identity(analysis, f.result / "functions/1040.json"),
            "incoming_branches": [branch], "assembly_evidence": "Independent synthetic E8 bytes and exact incoming JB target",
            "target_execution": False, "raw_artifacts_modified": False}
        f.write(f.review_path, f.review)
        return f

    def instruction_select(self, f):
        return boundaries.select_instructions(analysis, f.raw, f.capture, [0x1000], [f.specification])

    def instruction_sdk(self, f, row, issue=None):
        sdk, calls = self.sdk(f, row)
        start, phase = f.base + 0x1005, [False]
        refs = lambda rows: [SimpleNamespace(frm=int(ref["from"], 16), to=int(ref["to"], 16),
                            type=ref["type"], iscode=ref["code"], user=ref["user"]) for ref in rows]
        old = {int(ins["address"], 16): ins for ins in row["displaced_instructions"]}

        def annotations(address, key):
            if phase[0]:
                if key == "xrefs_from" and address == start:
                    return refs([{"from": hex(start), "to": hex(f.base + int(row["callee_rva"], 16)), "type": 17, "code": True, "user": False}])
                return []
            return refs(old.get(address, {}).get(key, []))

        def head(address):
            if phase[0]: return start
            return start + 3 if address == start + 4 else address

        def end(address):
            if phase[0]: return start + 5 + (issue == "item_readback")
            return start + 5 if address in (start + 3, start + 4) else address + 1

        def decode(out, address):
            out.size, out.itype = 5, 16
            out.ops = [SimpleNamespace(type=7, addr=f.base + int(row["callee_rva"], 16) + (issue == "target"))]
            return 0 if issue == "decode" else 5

        def delete(address, flags, size):
            calls.append(("delete", address, size))
            return issue != "delete"

        def create(address, out):
            calls.append(("create", address))
            phase[0] = True
            return 0 if issue == "create" else decode(out, address)

        sdk.bytes.get_full_flags = lambda ea: 1
        sdk.bytes.get_item_head = head
        sdk.bytes.get_item_end = end
        sdk.bytes.del_items = delete
        sdk.bytes.DELIT_SIMPLE = 0
        sdk.utils.XrefsFrom = lambda ea, flags: [] if phase[0] and issue == "xref_readback" else annotations(ea, "xrefs_from")
        sdk.utils.XrefsTo = lambda ea, flags: annotations(ea, "xrefs_to")
        sdk.idc = SimpleNamespace(get_name=lambda ea: "original_name" if ea == start + 1 else "",
            get_cmt=lambda ea, repeatable: ("changed" if issue == "annotation" else old.get(ea, {}).get("repeatable_comment" if repeatable else "comment")))
        sdk.ua = SimpleNamespace(insn_t=SimpleNamespace, decode_insn=decode, create_insn=create, o_near=7)
        if issue == "cancel": sdk.auto.auto_wait = lambda: False
        return sdk, calls

    def test_instruction_selection_proves_near_call_and_preserves_every_displaced_annotation(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.instruction_fixture(Path(directory))
            row = self.instruction_select(f)[0]
            body = boundaries.instruction_body(analysis, row)
            self.assertEqual(row["instruction"]["bytes"], "e836000000")
            self.assertEqual([ins["rva"] for ins in row["displaced_instructions"]], ["0x1006", "0x1007", "0x1008"])
            self.assertEqual(row["displaced_instructions"][0]["comment"], "original displaced comment")
            self.assertEqual(row["displaced_instructions"][0]["xrefs_to"][0]["type"], 1)
            self.assertEqual(body["chunks"], f.bodies["0x1000"]["chunks"])
            self.assertEqual([ins["rva"] for ins in body["assembly"]], ["0x1000", "0x1005", "0x100a", "0x100b"])

    def test_instruction_selection_rejects_wrong_byte_target_branch_or_external_interior_edge(self):
        for issue in ("byte", "target", "branch", "owner", "callee", "source", "external", "overlap", "unselected"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.instruction_fixture(Path(directory))
                if issue == "byte": f.review["raw_hex"] = "e936000000"
                elif issue == "target": f.review["callee_rva"] = "0x1041"
                elif issue == "branch": f.review["incoming_branches"][0]["bytes"] = "0f82f3ffffff"
                elif issue == "owner": f.review["owner_function"]["sha256"] = "0" * 64
                elif issue == "callee": (f.result / "functions/1040.json").write_bytes(b"changed")
                elif issue == "source": f.original.write_bytes(b"changed")
                elif issue in ("external", "overlap"):
                    owner = f.bodies["0x1000"]
                    if issue == "external": owner["assembly"][1]["xrefs_to"][0]["code"] = True
                    else: owner["assembly"][1]["address"] = hex(f.base + 0x1004)
                    identity = f.write(f.result / "functions/1000.json", owner)
                    f.capture["functions"][0].update(sha256=identity["sha256"], bytes=identity["bytes"])
                    f.review["owner_function"] = identity
                f.write(f.review_path, f.review)
                with self.assertRaises(ValueError):
                    boundaries.select_instructions(analysis, f.raw, f.capture,
                        [0x1040] if issue == "unselected" else [0x1000], [f.specification])

    def test_instruction_sdk_redecodes_only_five_bytes_and_keeps_raw_chunks(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.instruction_fixture(Path(directory))
            row = self.instruction_select(f)[0]
            sdk, calls = self.instruction_sdk(f, row)
            observed = boundaries.apply_instructions(row, f.base, sdk)
            boundaries.verify_instructions(analysis, [row], [observed])
            self.assertEqual(calls[:2], [("delete", f.base + 0x1005, 5), ("create", f.base + 0x1005)])
            self.assertEqual(observed["before"]["chunks"], observed["after"]["chunks"])
            self.assertEqual(observed["cpu_decoding"]["target"], hex(f.base + 0x1040))
            self.assertFalse(observed["instructions_patched"])
            self.assertFalse(observed["manual_frames_modified"])

    def test_instruction_sdk_stops_on_processor_target_annotations_or_readback_failure(self):
        for issue in ("decode", "target", "annotation", "delete", "create", "cancel", "item_readback", "xref_readback"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.instruction_fixture(Path(directory))
                row = self.instruction_select(f)[0]
                sdk, calls = self.instruction_sdk(f, row, issue)
                with self.assertRaises(ValueError): boundaries.apply_instructions(row, f.base, sdk)
                if issue in ("decode", "target", "annotation"): self.assertFalse(calls)

    def test_instruction_verifier_rejects_forged_original_annotations_or_incomplete_cpu_audit(self):
        for issue in ("missing", "bytes", "annotation", "xref", "cpu", "item", "frame", "manual", "flags"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.instruction_fixture(Path(directory))
                row = self.instruction_select(f)[0]
                sdk, calls = self.instruction_sdk(f, row)
                observed = copy.deepcopy(boundaries.apply_instructions(row, f.base, sdk))
                if issue == "bytes": observed["instruction"]["bytes"] = "e936000000"
                elif issue == "annotation": observed["annotations_before"][1]["comment"] = None
                elif issue == "xref": observed["annotations_after"][0]["xrefs_from"] = []
                elif issue == "cpu": observed["cpu_decoding"]["readback_target"] = "0x0"
                elif issue == "item": observed["annotations_after"][1]["item_end"] = "0x0"
                elif issue == "frame": observed["after"]["native_frame"]["fpd"] = False
                elif issue == "manual": observed["manual_stack_points_modified"] = True
                elif issue == "flags": observed["automatic_reanalysis"]["invalidated_flags"] = 0x5400
                with self.assertRaises(ValueError): boundaries.verify_instructions(analysis, [row], [] if issue == "missing" else [observed])

    def merge_fixture(self, root):
        f = self.fixture(root)
        helper = boundaries.helper_body(self.select(f)[0])
        members = []
        for index, (start, end) in enumerate(((f.base + 0x1040, f.base + 0x1042), (f.base + 0x1042, f.base + 0x1044))):
            body = copy.deepcopy(helper)
            body.update(address=hex(start), rva=hex(start - f.base), end=hex(end))
            body["assembly"] = [ins for ins in helper["assembly"] if start <= int(ins["address"], 16) < end]
            code = f.raw[0x200 + start - f.base - 0x1000:0x200 + end - f.base - 0x1000]
            body["chunks"] = [{"start": hex(start), "end": hex(end), "raw_hex": code.hex(),
                               "sha256": hashlib.sha256(code).hexdigest()}]
            member = f.write(f.result / "functions" / ("merge" + str(index) + ".json"), body)
            members.append({**member, "rva": body["rva"]})
        f.bodies = {"0x1040": helper}
        merge = {"owner_rva": "0x1040", "members": members, "expected_end": helper["end"],
                 "expected_chunks": helper["chunks"]}
        return f, merge

    def test_merged_owner_reanalysis_uses_complete_union_and_invalidates_only_derived_readiness(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f, merge = self.merge_fixture(Path(directory))
            sdk, calls = self.sdk(f, merge)
            result = boundaries.reanalyze_merges([merge], f.base, sdk)
            boundaries.verify_merge_reanalysis(analysis, [merge], result)
            self.assertEqual(calls, [("update", f.base + 0x1040), ("reanalyze", f.base + 0x1040)])
            self.assertEqual(result[0]["before"]["chunks"], result[0]["after"]["chunks"])
            self.assertEqual(result[0]["analysis_invalidation"]["invalidated_flags"], 0x5000)
            self.assertFalse(result[0]["manual_stack_points_modified"])

    def test_merged_owner_reanalysis_rejects_changed_members_sdk_failures_and_forged_audits(self):
        for issue in ("source", "base", "update", "flag_readback", "changed_bytes", "cancel", "audit", "stack", "flags", "missing"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f, merge = self.merge_fixture(Path(directory))
                sdk, calls = self.sdk(f, merge, issue)
                if issue == "source": (analysis.ROOT / merge["members"][0]["path"]).write_bytes(b"changed")
                if issue in ("source", "base", "update", "flag_readback", "changed_bytes", "cancel"):
                    with self.assertRaises(ValueError): boundaries.reanalyze_merges([merge], f.base + (issue == "base"), sdk)
                else:
                    result = copy.deepcopy(boundaries.reanalyze_merges([merge], f.base, sdk))
                    if issue == "audit": result[0]["manual_frames_modified"] = True
                    elif issue == "stack": result[0]["after"]["stack_points"].pop()
                    elif issue == "flags": result[0]["analysis_invalidation"]["invalidated_flags"] ^= 0x1000
                    else: result = []
                    with self.assertRaises(ValueError): boundaries.verify_merge_reanalysis(analysis, [merge], result)

    def test_prepass_is_explicit_and_binds_complete_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            plain = analysis.native_function_selection(f.raw, f.capture, [0x1000])
            self.assertNotIn("decompiler_prepasses", plain)
            selected = analysis.native_function_selection(f.raw, f.capture, [0x1000],
                type_note="Observe the exact first derivation.", decompiler_prepass_rvas=[0x1000])
            row = selected["decompiler_prepasses"][0]
            self.assertEqual(row["original_chunks"], f.bodies["0x1000"]["chunks"])
            self.assertEqual(row["source_function"]["sha256"], f.capture["functions"][0]["sha256"])
            for requested, reason in (([0x1010], "reason"), ([0x1000] * 2, "reason"), ([True], "reason"), ([0x1000], "")):
                with self.subTest(requested=requested, reason=reason), self.assertRaises(ValueError):
                    boundaries.select_prepasses(analysis, f.raw, f.capture, [0x1000], requested, reason)
            (f.result / "functions/1000.json").write_bytes(b"changed")
            with self.assertRaises(ValueError):
                boundaries.select_prepasses(analysis, f.raw, f.capture, [0x1000], [0x1000], "reason")

    def prepass_fixture(self, root):
        f = self.fixture(root)
        requested = boundaries.select_prepasses(analysis, f.raw, f.capture, [0x1000], [0x1000], "Observe first derivation.")[0]
        sdk, calls = self.sdk(f, requested)
        before = boundaries.prepass_snapshot(requested, sdk)
        body = copy.deepcopy(f.bodies["0x1000"])
        body["type_before_decompile"] = None
        body["type_after_decompile"] = "int __fastcall(void *)"
        body["assembly_after_decompile"] = copy.deepcopy(body["assembly"])
        body["assembly_after_decompile"][0]["text"] += "; retained SDK type display change"
        body["pseudocode"] = None
        body["decompiler_prepass"] = {**requested, "before": before,
            "after": boundaries.prepass_snapshot(requested, sdk), "manual_stack_points_modified": False,
            "manual_frames_modified": False, "instructions_patched": False,
            "base_database_modified": False, "target_execution": False}
        return f, requested, body, calls

    def test_prepass_retains_type_display_and_failure_facts_without_mutating_the_sdk(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f, requested, body, calls = self.prepass_fixture(Path(directory))
            output = f.result / "supplements/explicit"
            record = f.write(output / "preanalysis/1000.json", body)
            applied = [{**requested, "capture": {**record, "path": "preanalysis/1000.json"}}]
            boundaries.verify_prepasses(analysis, [requested], applied, output)
            self.assertFalse(calls)
            self.assertIsNone(body["pseudocode"])
            self.assertNotEqual(body["assembly"], body["assembly_after_decompile"])
            self.assertIsNone(body["type_before_decompile"])
            self.assertEqual(body["type_after_decompile"], "int __fastcall(void *)")

    def test_prepass_rejects_missing_or_changed_machine_audits_and_escaping_capture_paths(self):
        for issue in ("before", "after", "raw", "manual", "original", "path", "missing", "capture"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f, requested, body, calls = self.prepass_fixture(Path(directory))
                if issue in ("before", "after"): body["decompiler_prepass"][issue]["stack_points"].pop()
                elif issue == "raw": body["assembly_after_decompile"][0]["bytes"] = "cc"
                elif issue == "manual": body["decompiler_prepass"]["manual_frames_modified"] = True
                elif issue == "original": requested["original_chunks"] = []
                output = f.result / "supplements/explicit"
                record = f.write(output / "preanalysis/1000.json", body)
                applied = [{**requested, "capture": {**record, "path": "preanalysis/1000.json"}}]
                if issue == "path": applied[0]["capture"]["path"] = "../../1000.json"
                elif issue == "capture": applied[0]["capture"]["sha256"] = "0" * 64
                elif issue == "missing": applied = []
                with self.assertRaises(ValueError): boundaries.verify_prepasses(analysis, [requested], applied, output)

    def far_return_fixture(self, root):
        f = self.instruction_fixture(root)
        raw = bytearray(f.raw)
        raw[0x206], raw[0x211] = 0xcb, 0xc3
        raw[0x2d5:0x2d9] = raw[0x240:0x244]
        f.raw = bytes(raw)
        f.original.write_bytes(f.raw)
        f.result = root / ".analysis/results/native" / hashlib.sha256(f.raw).hexdigest()
        owner, callee = f.bodies["0x1000"], f.bodies.pop("0x1040")
        owner["flags"], owner["end"] = 0x5402, hex(f.base + 0x1012)
        owner["assembly"][1].update(bytes="cb", text="retf")
        owner["assembly"].append({"address": hex(f.base + 0x1011), "rva": "0x1011", "bytes": "c3", "text": "retn",
                                  "comment": None, "repeatable_comment": None, "xrefs_from": [], "xrefs_to": []})
        owner["chunks"] = [{"start": owner["address"], "end": owner["end"], "raw_hex": f.raw[0x200:0x212].hex(),
                             "sha256": hashlib.sha256(f.raw[0x200:0x212]).hexdigest()}]
        callee.update(rva="0x10d5", address=hex(f.base + 0x10d5), end=hex(f.base + 0x10d9))
        for ins in callee["assembly"]:
            offset = int(ins["rva"], 16) - 0x1040
            ins.update(address=hex(f.base + 0x10d5 + offset), rva=hex(0x10d5 + offset))
        callee["chunks"] = [{"start": callee["address"], "end": callee["end"], "raw_hex": f.raw[0x2d5:0x2d9].hex(),
                              "sha256": hashlib.sha256(f.raw[0x2d5:0x2d9]).hexdigest()}]
        f.bodies["0x10d5"] = callee
        records = []
        for rva, body in f.bodies.items():
            name = "functions/" + rva[2:] + ".json"
            record = f.write(f.result / name, body)
            records.append({**record, "path": name, "rva": rva, "address": body["address"], "assembly_captured": True})
        f.capture.update(input_sha256=hashlib.sha256(f.raw).hexdigest(), functions=records)
        f.write(f.result / "capture.json", f.capture)
        f.review.update(source_input=evidence.identity(analysis, f.original),
            base_capture=evidence.identity(analysis, f.result / "capture.json"),
            owner_function=evidence.identity(analysis, f.result / "functions/1000.json"), raw_hex="e8cb000000",
            callee_rva="0x10d5", callee_function=evidence.identity(analysis, f.result / "functions/10d5.json"),
            clear_stale_far_return=True)
        f.write(f.review_path, f.review)
        return f

    def test_stale_far_return_is_cleared_only_with_complete_near_return_and_sdk_proof(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.far_return_fixture(Path(directory))
            requested = self.instruction_select(f)[0]
            self.assertEqual(requested["near_return_recovery"]["near_returns"][0]["bytes"], "c3")
            sdk, calls = self.instruction_sdk(f, requested)
            original_decode = sdk.ua.decode_insn
            def decode(out, address):
                if address == f.base + 0x1011:
                    out.size, out.itype, out.get_canon_mnem = 1, 42, lambda: "retn"
                    return 1
                return original_decode(out, address)
            sdk.ua.decode_insn = decode
            sdk.funcs.FUNC_FAR, sdk.funcs.FUNC_USERFAR = 2, 32
            observed = boundaries.apply_instructions(requested, f.base, sdk)
            boundaries.verify_instructions(analysis, [requested], [observed])
            self.assertEqual(observed["after_decode"]["flags"], 0x5402)
            self.assertEqual(observed["after"]["flags"], 0x5000)
            self.assertFalse(observed["manual_frames_modified"])
            for key in ("cpu_decodings", "far_flag", "automatic_far_flag_cleared"):
                changed = copy.deepcopy(observed)
                changed["near_return_flag_reanalysis"].pop(key)
                with self.assertRaises(ValueError): boundaries.verify_instructions(analysis, [requested], [changed])

    def test_stale_far_return_requires_explicit_true_and_rejects_user_flags_or_missing_near_return(self):
        for issue in ("false", "user", "no_far", "near", "gap"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.far_return_fixture(Path(directory))
                if issue == "false":
                    f.review["clear_stale_far_return"] = False
                    f.write(f.review_path, f.review)
                    with self.assertRaises(ValueError): self.instruction_select(f)
                else:
                    original = f.bodies["0x1000"]
                    if issue == "user": original["flags"] |= 32
                    elif issue == "no_far": original["flags"] &= ~2
                    elif issue == "near": original["assembly"][-1]["text"] = "retf"
                    else: original["assembly"].pop(-2)
                    displaced = original["assembly"][1:4]
                    instruction = {"address": hex(f.base + 0x1005), "rva": "0x1005", "bytes": "e8cb000000", "text": "call target"}
                    with self.assertRaises(ValueError): boundaries._near_return_proof(f.raw, original, displaced, instruction)

    def test_partition_prepass_keeps_the_complete_owner_and_helper_byte_union(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.instruction_fixture(Path(directory))
            original = f.bodies["0x1000"]
            start = int(original["address"], 16)
            def piece(first, end):
                raw = f.raw[0x200 + first - start:0x200 + end - start]
                return {"start": hex(first), "end": hex(end), "raw_hex": raw.hex(), "sha256": hashlib.sha256(raw).hexdigest()}
            partition = {"owner_rva": "0x1000", "source_function": f.review["owner_function"],
                "original_chunks": original["chunks"], "owner_chunks": [piece(start, start + 5), piece(start + 10, start + 17)],
                "helpers": [{"expected_chunks": [piece(start + 5, start + 10)]}]}
            requested = boundaries.select_prepasses(analysis, f.raw, f.capture, [0x1000], [0x1000],
                "Derive types only after the reviewed partition.", [partition])[0]
            self.assertEqual(requested["partition_boundary"], partition)
            body = boundaries._prepass_body(requested, original)
            self.assertEqual(body["end"], hex(start + 5))
            self.assertEqual(len(body["chunks"]), 2)
            self.assertEqual([ins["rva"] for ins in body["assembly"]], ["0x1000", "0x100a", "0x100b"])
            for issue in ("raw", "missing", "source"):
                changed = copy.deepcopy(requested)
                if issue == "raw": changed["partition_boundary"]["owner_chunks"][0]["raw_hex"] = "90" * 5
                elif issue == "missing": changed["partition_boundary"]["helpers"] = []
                else: changed["partition_boundary"]["source_function"]["sha256"] = "0" * 64
                with self.assertRaises(ValueError): boundaries._prepass_body(changed, original)

    def test_exact_instruction_prepass_retains_the_original_chunks_and_repaired_e8(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.instruction_fixture(Path(directory))
            instruction = self.instruction_select(f)[0]
            requested = boundaries.select_prepasses(analysis, f.raw, f.capture, [0x1000], [0x1000],
                "Observe derivation after the exact instruction repair.", instructions=[instruction])[0]
            body = boundaries._prepass_body(requested, f.bodies["0x1000"])
            self.assertEqual(body["chunks"], f.bodies["0x1000"]["chunks"])
            self.assertEqual(body["assembly"][1]["bytes"], "e836000000")
            changed = copy.deepcopy(requested)
            changed["instruction_boundary"]["instruction"]["bytes"] = "e8cb000000"
            with self.assertRaises(ValueError): boundaries._prepass_body(changed, f.bodies["0x1000"])


class NativeFrameReconstructionTests(unittest.TestCase):
    def fixture(self, root):
        f = NativeBoundaryTests().fixture(root)
        body = copy.deepcopy(f.bodies["0x1000"])
        native = {"start_ea": int(body["address"], 16), "end_ea": int(body["end"], 16),
                  "flags": 0x5400, "frame": 7, "frsize": 32, "frregs": 8, "argsize": 0, "fpd": 16}
        member = {"index": 0, "offset_bits": 40 * 8, "size_bits": 32 * 8,
                  "name": "misplaced", "comment": "Retain this exact original comment.", "type": "int[8]",
                  "return_address": False, "special_member": False}
        layout = {"schema_version": 1, "status": "api_returned", "target_execution": False,
                  "frame_modified": False, "references_complete": True, "native_function": native,
                  "parts": {"lvars": 0, "savregs": 32, "retaddr": 40, "args": 48},
                  "return_address_bytes": 8, "full_frame_bytes": 48,
                  "layout": {"total_size": 72}, "member_count": 1, "members": [member],
                  "member_references": [{"member_index": 0, "offset_bits": 320, "size_bits": 256,
                                         "references": []}], "memory_operands": []}
        body["native_frame_layout_before_decompile"] = layout
        frame_record = f.write(f.result / "supplements/observed/functions/1000.json", body)
        original_owner = f.result / "functions/1000.json"
        review = {"schema_version": 1, "review_kind": "native_cpu_frame_reconstruction",
                  "source_input": evidence.identity(analysis, f.original),
                  "base_capture": evidence.identity(analysis, f.result / "capture.json"),
                  "owner_function": evidence.identity(analysis, original_owner), "owner_rva": "0x1000",
                  "frame_evidence": frame_record, "original_frame": layout,
                  "discard_corrupt_frame": True, "reason": "Complete machine references prove a PDB frame overlap.",
                  "member_reviews": [{"member": copy.deepcopy(member), "reason": "Exact original machine evidence retained.",
                                      "machine_evidence": [body["assembly"][0]]}],
                  "target_execution": False, "raw_artifacts_modified": False}
        f.review_path = f.result / "frame-review.json"
        f.write(f.review_path, review)
        f.review, f.layout, f.observed = review, layout, body
        f.specification = analysis.rel(f.review_path)
        return f

    def select(self, f):
        return boundaries.select_frame_rebuilds(analysis, f.raw, f.capture, [0x1000], [f.specification])

    def test_complete_frame_comments_references_and_each_invalid_member_are_bound(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            row = self.select(f)[0]
            self.assertEqual(row["original_frame"], f.layout)
            self.assertEqual(row["member_reviews"][0]["member"]["comment"], "Retain this exact original comment.")
            parsed = analysis.native_function_selection(f.raw, f.capture, [0x1000],
                observation_evidence=None)
            self.assertNotIn("frame_rebuilds", parsed)
            with self.assertRaisesRegex(ValueError, "explicit observation"):
                analysis.native_function_selection(f.raw, f.capture, [0x1000], frame_reviews=[f.specification])

    def test_missing_member_comment_reference_exact_bytes_and_return_overlap_proof_are_rejected(self):
        for issue in ("comment", "references", "member", "missing", "reason", "instruction", "owner", "source", "discard", "execution"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "comment": f.review["original_frame"]["members"][0]["comment"] = "Changed"
                elif issue == "references": f.review["original_frame"]["member_references"] = []
                elif issue == "member": f.review["member_reviews"][0]["member"]["size_bits"] = 8
                elif issue == "missing": f.review["member_reviews"] = []
                elif issue == "reason": f.review["member_reviews"][0]["reason"] = ""
                elif issue == "instruction": f.review["member_reviews"][0]["machine_evidence"][0]["bytes"] = "cc"
                elif issue == "owner": f.review["owner_function"]["sha256"] = "0" * 64
                elif issue == "source": f.original.write_bytes(b"changed")
                elif issue == "discard": f.review["discard_corrupt_frame"] = 1
                else: f.review["target_execution"] = True
                f.write(f.review_path, f.review)
                with self.assertRaises(ValueError): self.select(f)
        for issue in ("retaddr", "healthy", "truncated", "machine"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "retaddr": f.layout["members"][0]["return_address"] = True
                elif issue == "healthy": f.layout["layout"]["total_size"] = 48
                elif issue == "truncated": f.layout["references_complete"] = False
                else: f.layout["memory_operands"] = [{"address": f.observed["address"], "instruction_bytes": "cc"}]
                f.review["frame_evidence"] = f.write(f.result / "supplements/observed/functions/1000.json", f.observed)
                f.review["original_frame"] = f.layout
                f.write(f.review_path, f.review)
                with self.assertRaises(ValueError): self.select(f)

    def sdk(self, f, issue=None):
        native = f.layout["native_function"]
        function = SimpleNamespace(**native)
        current = [copy.deepcopy(f.layout)]
        calls = []

        def get_bytes(ea, size):
            return f.raw[0x200 + ea - f.base - 0x1000:0x200 + ea - f.base - 0x1000 + size]

        def delete(owner):
            calls.append(("delete", owner.start_ea))
            return issue != "delete"

        def create(owner):
            calls.append(("create", owner.start_ea))
            if issue != "unchanged":
                marker = {"index": 0, "offset_bits": 320, "size_bits": 64, "name": "__return_address",
                          "comment": "", "type": "_QWORD", "return_address": True, "special_member": True}
                current[0] = {**copy.deepcopy(f.layout), "members": [marker], "layout": {"total_size": 48},
                    "member_references": [{"member_index": 0, "offset_bits": 320, "size_bits": 64, "references": []}]}
            return 0 if issue == "create" else 1

        sdk = SimpleNamespace(root=analysis.ROOT,
            funcs=SimpleNamespace(get_func=lambda ea: function,
                                  reanalyze_function=lambda owner: calls.append(("reanalyze", owner.start_ea))),
            utils=SimpleNamespace(Chunks=lambda ea: [(int(row["start"], 16), int(row["end"], 16))
                                                    for row in f.observed["chunks"]]),
            bytes=SimpleNamespace(get_bytes=get_bytes),
            types=SimpleNamespace(tinfo_t=lambda: SimpleNamespace()),
            nalt=SimpleNamespace(get_tinfo=lambda tif, ea: False),
            frame=SimpleNamespace(get_spd=lambda owner, ea: -32, get_sp_delta=lambda owner, ea: 0, del_frame=delete),
            auto=SimpleNamespace(auto_wait=lambda: issue != "cancel"),
            idp=SimpleNamespace(get_ph=lambda: SimpleNamespace(create_func_frame=create)),
            frame_layout=lambda owner: copy.deepcopy(current[0]))
        return sdk, calls, current

    def test_processor_reconstruction_supplies_no_sizes_or_offsets_and_preserves_every_stage(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            requested = self.select(f)[0]
            sdk, calls, current = self.sdk(f)
            stages = []
            row = boundaries.apply_frame_rebuild(requested, f.base, sdk, lambda row: stages.append(copy.deepcopy(row)))
            boundaries.verify_frame_rebuilds(analysis, [requested], [row])
            self.assertEqual([call[0] for call in calls], ["delete", "create", "reanalyze"])
            self.assertEqual(len(stages), 5)
            self.assertFalse(stages[0]["completed"])
            self.assertNotIn("corrupt_frame_deleted", stages[0])
            self.assertTrue(stages[1]["corrupt_frame_deleted"])
            self.assertFalse(stages[2]["completed"])
            self.assertEqual(row["frame_before"], f.layout)
            self.assertTrue(row["frame_after"]["members"][0]["return_address"])
            for issue in ("completed", "auto_wait_completed", "manual_stack_points_modified", "manual_frame_values_applied", "processor_result", "old_comment", "machine", "marker"):
                changed = copy.deepcopy(row)
                if issue in ("completed", "auto_wait_completed"): changed[issue] = False
                elif issue in ("manual_stack_points_modified", "manual_frame_values_applied"): changed[issue] = True
                elif issue == "processor_result": changed[issue] = True
                elif issue == "old_comment": changed["frame_before"]["members"][0]["comment"] = "changed"
                elif issue == "machine": changed["after"]["chunks"][0]["raw_hex"] = "cc"
                else: changed["frame_after"]["members"][0]["return_address"] = False
                with self.subTest(issue=issue), self.assertRaises(ValueError):
                    boundaries.verify_frame_rebuilds(analysis, [requested], [changed])

    def test_processor_failure_cancel_and_unchanged_corruption_remain_failed_with_complete_checkpoints(self):
        for issue in ("delete", "create", "cancel", "unchanged"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                requested = self.select(f)[0]
                sdk, calls, current = self.sdk(f, issue)
                stages = []
                with self.assertRaises(ValueError):
                    boundaries.apply_frame_rebuild(requested, f.base, sdk, lambda row: stages.append(copy.deepcopy(row)))
                self.assertTrue(stages)
                self.assertFalse(stages[-1]["completed"])
                self.assertEqual(stages[-1]["frame_before"], f.layout)
                with self.assertRaises(ValueError): boundaries.verify_frame_rebuilds(analysis, [requested], [stages[-1]])

    def test_missing_processor_api_is_rejected_before_deleting_a_private_frame(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            requested = self.select(f)[0]
            sdk, calls, current = self.sdk(f)
            sdk.idp.get_ph = lambda: SimpleNamespace()
            with self.assertRaisesRegex(ValueError, "verified frame reconstruction API"):
                boundaries.apply_frame_rebuild(requested, f.base, sdk, lambda row: None)
            self.assertFalse(calls)
            self.assertEqual(current[0], f.layout)


class X86EntryPartitionTests(unittest.TestCase):
    def fixture(self, root, sse=False, mutate=None):
        base, first, helper = 0x10000000, 0x1000, 0x1120
        raw = bytearray(0x600)
        raw[:2] = b"MZ"
        struct.pack_into("<I", raw, 0x3c, 0x80)
        raw[0x80:0x84] = b"PE\0\0"
        struct.pack_into("<HHIIIHH", raw, 0x84, 0x14c, 1, 0, 0, 0, 0xe0, 0x2102)
        struct.pack_into("<H", raw, 0x98, 0x10b)
        struct.pack_into("<I", raw, 0x98 + 28, base)
        struct.pack_into("<I", raw, 0x98 + 60, 0x200)
        struct.pack_into("<8sIIIIIIHHI", raw, 0x178, b".text", 0x400, first, 0x400, 0x200, 0, 0, 0, 0, 0x60000020)
        call = lambda at, target: b"\xe8" + struct.pack("<i", target - at - 5)
        if sse:
            stack, context = first + 24, first + 30
            code = bytes.fromhex("558bec83ec0883e4f0dd1c24f30f7e0424") + call(first + 17, context)
            code += bytes.fromhex("c9c3660f12442404660fc5c003") + call(context + 5, helper) + b"\xc3"
            lengths = [1, 2, 3, 3, 3, 5, 5, 1, 1, 6, 5, 5, 1]
        else:
            stack, context = first + 20, first + 29
            code = bytes.fromhex("83ec0cdd1424") + call(first + 6, 0x1100) + call(first + 11, context)
            code += bytes.fromhex("83c40cc38d542404") + call(stack + 4, 0x1110)
            code += bytes.fromhex("529bd93c24") + call(context + 5, helper) + bytes.fromhex("5ac3")
            lengths = [3, 3, 5, 5, 3, 1, 4, 5, 1, 4, 5, 1, 1]
        raw[0x200:0x200 + len(code)] = code
        raw[0x280:0x285] = b"\xe9" + struct.pack("<i", stack - 0x1085)
        raw[0x320] = raw[0x340] = 0xc3
        if mutate:
            mutate(raw)
        raw, end = bytes(raw), first + len(code)
        source = root / ".analysis/inputs/entries.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)

        def body(rva, sizes):
            lo, assembly = rva, []
            for size in sizes:
                assembly.append({"address": hex(base + lo), "rva": hex(lo), "text": "original instruction",
                    "bytes": raw[0x200 + lo - first:0x200 + lo - first + size].hex(),
                    "xrefs_to": [], "xrefs_from": []})
                lo += size
            code = raw[0x200 + rva - first:0x200 + lo - first]
            return {"address": hex(base + rva), "rva": hex(rva), "end": hex(base + lo), "flags": 0x5400,
                "type": None, "chunks": [{"start": hex(base + rva), "end": hex(base + lo),
                    "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}],
                "assembly": assembly, "xrefs_to_entry": []}

        owner, caller = body(first, lengths), body(0x1080, [5])
        edge = {"from": hex(base + 0x1080), "to": hex(base + stack), "code": True, "type": 19, "user": False}
        caller["assembly"][0]["xrefs_from"] = [edge]
        next(ins for ins in owner["assembly"] if ins["rva"] == hex(stack))["xrefs_to"] = [edge]
        bodies = [owner, caller, body(helper, [1]), body(0x1140, [1])]
        identities, records = [], []
        for value in bodies:
            identity = write(result / "functions" / (value["rva"][2:] + ".json"), value)
            identities.append(identity)
            records.append({**identity, "path": "functions/" + value["rva"][2:] + ".json", "rva": value["rva"],
                            "address": value["address"], "assembly_captured": True})
        capture = {"input": analysis.rel(source), "input_sha256": hashlib.sha256(raw).hexdigest(),
                   "input_bytes": len(raw), "image_base": hex(base), "capture_complete": True, "functions": records}
        cap = write(result / "capture.json", capture)
        review = {"schema_version": 1, "review_kind": "x86_ci_stack_context_complete_entry_partition",
            "source_input": evidence.identity(analysis, source), "base_capture": cap, "owner_function": identities[0],
            "boundaries": [hex(x) for x in (first, stack, context, end)],
            "external_incoming": [{"target_rva": hex(stack), "source_rva": "0x1080", "xref": edge,
                "source_function": identities[1], "source_instruction": caller["assembly"][0]}],
            "outline_helper": identities[2], "reason": "Complete exact three-entry byte union and real external stack caller",
            "target_execution": False, "raw_artifacts_modified": False}
        path = root / ".analysis/results/entries-review.json"
        write(path, review)
        return SimpleNamespace(root=root, base=base, raw=raw, source=source, capture=capture, result=result,
            bodies=bodies, identities=identities, review=review, path=path, write=write,
            first=first, stack=stack, context=context, end=end)

    def select(self, f):
        return boundaries.select_entry_partitions(analysis, f.raw, f.capture, [f.first, 0x1140],
                                                  [analysis.rel(f.path)], [0x1120])

    def test_x87_and_sse_patterns_keep_all_three_entries_and_complete_original_byte_union(self):
        for sse in (False, True):
            with self.subTest(sse=sse), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), sse=sse)
                row = self.select(f)[0]
                bodies = boundaries.entry_partition_bodies(analysis, row)
                self.assertEqual([body["rva"] for body in bodies], [hex(x) for x in (f.first, f.stack, f.context)])
                self.assertEqual("".join(body["chunks"][0]["raw_hex"] for body in bodies), f.bodies[0]["chunks"][0]["raw_hex"])
                self.assertFalse(row["semantics_complete"])
                selection = analysis.native_function_selection(f.raw, f.capture, [f.first, 0x1140],
                    type_note="Complete original separately outlined helper", outline_rvas=[0x1120], boundary_reviews=[analysis.rel(f.path)])
                self.assertEqual(len(selection["decompiler_prepasses"]), 4)
                self.assertNotIn("function_boundary_recoveries", selection)

    def test_review_rejects_supplied_sp_or_abi_wrong_owners_missing_edges_and_helper_changes(self):
        for mutate in (lambda r: r.update(stack_delta=8), lambda r: r.update(type="guessed ABI"),
                       lambda r: r.update(external_incoming=[]), lambda r: r["boundaries"].__setitem__(1, "0x1013"),
                       lambda r: r.update(outline_helper=r["owner_function"]),
                       lambda r: r["external_incoming"][0]["source_instruction"].update(bytes="90")):
            with self.subTest(mutate=mutate), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                mutate(f.review)
                f.write(f.path, f.review)
                with self.assertRaises(ValueError): self.select(f)

    def test_changed_machine_pattern_original_instruction_and_source_artifact_are_rejected(self):
        for offset in (0x200, 0x20c, 0x218, 0x21d):
            with self.subTest(offset=offset), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory), mutate=lambda raw: raw.__setitem__(offset, raw[offset] ^ 1))
                with self.assertRaises(ValueError): self.select(f)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            target = Path(directory) / f.identities[0]["path"]
            target.write_bytes(target.read_bytes() + b" ")
            with self.assertRaises(ValueError): self.select(f)

    def sdk(self, f, issue=None):
        memory, functions, bodies, calls = bytearray(f.raw), {}, {}, []
        for body in f.bodies:
            ea = int(body["address"], 16)
            functions[ea] = SimpleNamespace(start_ea=ea, end_ea=int(body["end"], 16), flags=0x5400,
                                           frame=0, frsize=0, frregs=0, argsize=0, fpd=0)
            bodies[ea] = body
        class Type:
            def __str__(self): return "observed SDK type"
        def delete(ea):
            calls.append(("delete", ea))
            if issue == "delete": return False
            functions.pop(ea)
            return True
        def add(ea, end):
            calls.append(("add", ea, end))
            if issue == "add": return False
            if issue == "interrupt": raise KeyboardInterrupt()
            functions[ea] = SimpleNamespace(start_ea=ea, end_ea=end + (issue == "boundary"), flags=0x5400,
                                           frame=0, frsize=0, frregs=0, argsize=0, fpd=0)
            return True
        def chunks(ea):
            if len(functions) == len(f.bodies) and ea == f.base + f.first:
                return [(int(c["start"], 16), int(c["end"], 16)) for c in f.bodies[0]["chunks"]]
            return [(ea, functions[ea].end_ea)]
        def wait():
            if issue == "bytes": memory[0x200] ^= 1
            return issue != "cancel"
        sdk = SimpleNamespace(root=f.root, funcs=SimpleNamespace(get_func=lambda ea: functions.get(ea),
            del_func=delete, add_func=add), utils=SimpleNamespace(Chunks=chunks),
            bytes=SimpleNamespace(get_bytes=lambda ea,size: bytes(memory[0x200 + ea - f.base - f.first:0x200 + ea - f.base - f.first + size])),
            types=SimpleNamespace(tinfo_t=Type), nalt=SimpleNamespace(get_tinfo=lambda typ,ea: True),
            frame=SimpleNamespace(get_spd=lambda fn,ea: 0, get_sp_delta=lambda fn,ea: 0),
            auto=SimpleNamespace(auto_wait=wait))
        return sdk, calls

    def test_sdk_changes_only_private_boundaries_and_audits_complete_before_after_bytes(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            requests = self.select(f)
            sdk, calls = self.sdk(f)
            checkpoints = []
            row = boundaries.apply_entry_partition(requests[0], f.base, sdk, lambda r: checkpoints.append(copy.deepcopy(r)))
            boundaries.verify_entry_partitions(analysis, requests, [row])
            self.assertEqual([item[0] for item in calls], ["delete", "add", "add", "add"])
            self.assertEqual(len(row["after"]), 3)
            self.assertFalse(checkpoints[0]["deleted_original"])
            self.assertEqual(checkpoints[-1], row)
            for key in ("request", "created_entries", "after", "auto_wait_completed"):
                forged = copy.deepcopy(row)
                forged[key] = None
                with self.subTest(key=key), self.assertRaises(ValueError):
                    boundaries.verify_entry_partitions(analysis, requests, [forged])

    def test_sdk_failure_cancel_interrupt_and_byte_changes_preserve_failed_checkpoints(self):
        for issue in ("delete", "add", "interrupt", "cancel", "boundary", "bytes"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                row = self.select(f)[0]
                sdk, calls = self.sdk(f, issue)
                checkpoints = []
                with self.assertRaises((ValueError, KeyboardInterrupt)):
                    boundaries.apply_entry_partition(row, f.base, sdk, lambda r: checkpoints.append(copy.deepcopy(r)))
                self.assertTrue(checkpoints)
                with self.assertRaises(ValueError): boundaries.verify_entry_partitions(analysis, [row], [checkpoints[-1]])

    def test_prepass_derives_every_entry_and_helpers_require_complete_live_typed_coordinates(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            requests = self.select(f)
            prepasses = boundaries.entry_partition_prepasses(analysis, f.raw, f.capture, [f.first, 0x1140], requests)
            self.assertEqual([r["rva"] for r in prepasses], [hex(x) for x in (f.first, f.stack, f.context, 0x1140)])
            for request in prepasses[:3]:
                self.assertEqual(boundaries._prepass_body(request, f.bodies[0])["rva"], request["rva"])
            output, records = f.result / "supplements/current", []
            for body in boundaries.entry_partition_bodies(analysis, requests[0])[1:]:
                body["pseudocode"] = {"lines": [{"text": "real typed capture"}], "locals": []}
                test_evidence.NativeSupplementTests.typed_fixture(body)
                name = "entry-partition-helpers/" + body["rva"][2:] + ".json"
                identity = f.write(output / name, body)
                records.append({**identity, "rva": body["rva"], "path": name,
                                "assembly_captured": True, "pseudocode_captured": True})
            self.assertEqual(len(boundaries.verify_entry_helpers(analysis, f.raw, requests, records, output)), 2)
            with self.assertRaises(ValueError): boundaries.verify_entry_helpers(analysis, f.raw, requests, records[:1], output)
            body.pop("type_before_decompile")
            identity = f.write(output / records[-1]["path"], body)
            records[-1].update(bytes=identity["bytes"], sha256=identity["sha256"])
            with self.assertRaises(ValueError): boundaries.verify_entry_helpers(analysis, f.raw, requests, records, output)


class OptimizerPropertyTests(unittest.TestCase):
    def fixture(self, root, mulx_prefix=""):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        code = {0x1000: ["e83b000000", "c3"],
                0x1040: [mulx_prefix + "c4e2fbf6c1", "66480f38f6c1", "f3480f38f6c1", "c3"]}
        for start, parts in code.items():
            data = bytes.fromhex("".join(parts))
            raw[0x200 + start - 0x1000:0x200 + start - 0x1000 + len(data)] = data
        raw, base = bytes(raw), 0x140000000
        source = root / ".analysis/inputs/native/optimizer.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        bodies, records, identities = [], [], []
        for start, parts in code.items():
            at, rows = start, []
            for part in parts:
                rows.append({"address": hex(base + at), "rva": hex(at), "bytes": part,
                             "text": "complete original instruction", "xrefs_from": [], "xrefs_to": []})
                at += len(bytes.fromhex(part))
            machine = bytes.fromhex("".join(parts))
            body = {"rva": hex(start), "address": hex(base + start), "end": hex(base + at),
                    "assembly": rows, "chunks": [{"start": hex(base + start), "end": hex(base + at),
                        "raw_hex": machine.hex(), "sha256": hashlib.sha256(machine).hexdigest()}]}
            identity = write(result / "functions" / (hex(start)[2:] + ".json"), body)
            bodies.append(body); identities.append(identity)
            records.append({**identity, "path": "functions/" + hex(start)[2:] + ".json",
                            "rva": hex(start), "address": body["address"], "assembly_captured": True})
        capture = {"input": analysis.rel(source), "input_bytes": len(raw),
                   "input_sha256": hashlib.sha256(raw).hexdigest(), "capture_complete": True,
                   "image_base": hex(base), "functions": records}
        cap = write(result / "capture.json", capture)
        review = {"schema_version": 1, "review_kind": "exact_fict_ext_MULX_optimizer_properties",
                  "source_input": evidence.identity(analysis, source), "base_capture": cap,
                  "owners": identities[:1], "helpers": identities[1:],
                  "reason": "Exact SDK propagation constraints with original machine mappings and full def-use closure",
                  "target_execution": False, "raw_artifacts_modified": False}
        path = root / ".analysis/results/optimizer-review.json"
        write(path, review)
        outlines = [{"rva": "0x1040", "source_function": identities[1], "expected_chunks": bodies[1]["chunks"]}]
        return SimpleNamespace(raw=raw, base=base, source=source, result=result, bodies=bodies,
            capture=capture, path=path, review=review, write=write, outlines=outlines)

    def select(self, f):
        return boundaries.select_optimizer_properties(analysis, f.raw, f.capture, [0x1000],
            [analysis.rel(f.path)], f.outlines)[0]

    @staticmethod
    def snapshot():
        def reg(number): return {"kind": 1, "size": 8, "register": number}
        def ins(opcode, ea, mapped, destination, left=None, right=None, may=(), must=()):
            return {"opcode": opcode, "ea": ea, "mapped_ea": mapped, "iprops": 0,
                    "text": "exact SDK instruction", "may_def_registers": list(may), "must_def_registers": list(must),
                    "operands": {"l": left or {"kind": 0, "size": 0},
                                 "r": right or {"kind": 0, "size": 0}, "d": destination}}
        mulx_ea, mapped = "0xf1c0000000000001", "0x140001040"
        nested = ins(9, mulx_ea, mapped, reg(48), reg(56))
        add = ins(12, mulx_ea, mapped, reg(40), {"kind": 9, "size": 8,
            "address_operand": {"kind": 3, "size": 8, "instruction": nested}})
        top = [ins(2, mulx_ea, mapped, reg(0), reg(24), may=range(8), must=range(8)),
               ins(4, mulx_ea, mapped, reg(8), {"kind": 3, "size": 8, "instruction": add},
                   may=range(8, 16), must=range(8, 16)),
               ins(62, "0xf1c0000000000002", "0x140001045", reg(32), reg(8), reg(16),
                   may=range(32, 40), must=range(32, 40)),
               ins(4, "0x140001000", "0x140001000", reg(64), reg(0), may=range(64, 72), must=range(64, 72))]
        return {"entry_rva": "0x1000", "maturity": 1, "complete": True, "frame": {"frsize": 32},
                "blocks": [{"serial": 0, "predecessors": [], "successors": [], "instructions": top}]}

    def applied(self, request, before=None):
        before = self.snapshot() if before is None else before
        scope = boundaries.optimizer_expected_changes(request, before)
        after = copy.deepcopy(before)
        coordinates = {(r["block"], r["top"], tuple(r["path"])) for r in scope["changes"]}
        for b, t, p, ins in boundaries.optimizer_instruction_rows(after):
            if (b, t, tuple(p)) in coordinates: ins["iprops"] |= 131072
        return {"before": before, "after": after, "scope": scope, "iprop_dont_prop": 131072}

    def audit(self, request):
        native = []
        for original in [*request["owners"], *request["helpers"]]:
            native.append({"address": original["address"], "end": original["end"],
                "chunks": copy.deepcopy(original["chunks"]), "flags": 0, "type_present": False, "type": None,
                "native_frame": {k: 0 for k in ("frame", "frsize", "frregs", "argsize", "fpd")},
                "stack_points": [{"ea": i["address"], "spd": 0, "delta": 0} for i in original["instructions"]]})
        audit = {"request": request, "complete": True, "hook_registered": True, "hook_unregistered": True,
            "errors": [], "optimizer_properties_changed": True, "passes": [self.applied(request)],
            "original_state_before": native, "original_state_after": copy.deepcopy(native), "stages": [],
            **{k: False for k in ("target_execution", "base_database_modified", "machine_bytes_modified",
                "opcode_ea_operands_modified", "manual_stack_or_frame_modified")}}
        for phase, maturity in (("generated", 0), ("locopt", 2), ("calls_done", 3), ("glbopt", 6), ("final_mba", 8)):
            snapshot = self.snapshot(); snapshot["maturity"] = maturity
            audit["stages"].append({"phase": phase, "snapshot": snapshot})
        return audit

    def test_exact_original_selection_rejects_changed_scope_identity_and_machine(self):
        mutations = [lambda f: f.review.update(property="IPROP_DONT_COMB"),
            lambda f: f.review.update(target_execution=True), lambda f: f.review["helpers"].clear(),
            lambda f: f.review["owners"].append(f.review["owners"][0]),
            lambda f: f.review["base_capture"].update(sha256="0" * 64),
            lambda f: f.outlines[0].update(rva="0x1050"),
            lambda f: f.capture.update(capture_complete=False),
            lambda f: f.source.write_bytes(f.raw + b"changed")]
        for mutation in [None, *mutations]:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if mutation is None:
                    request = self.select(f)
                    self.assertEqual(request["ext_machine_eas"], ["0x140001045", "0x14000104b"])
                    self.assertEqual(request["mulx_machine_eas"], ["0x140001040"])
                else:
                    mutation(f); f.write(f.path, f.review)
                    with self.assertRaises((ValueError, OSError)): self.select(f)

    def test_full_nested_closure_is_derived_from_actual_nearest_sdk_definition(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); request = self.select(f)
            row = self.applied(request)
            self.assertEqual(boundaries.verify_optimizer_property_pass(request, row), row["scope"])
            self.assertEqual(len(row["scope"]["changes"]), 5)
            self.assertEqual(row["scope"]["ext_count"], 1)
            self.assertIn(["l", "l", "address_operand"], [c["path"] for c in row["scope"]["changes"]])
            self.assertEqual([w["status"] for w in row["scope"]["def_use_witnesses"]],
                ["exact_nearest_full_direct_MULX_definition", "no_same_block_definition"])

    def test_repeated_original_address_prefixes_do_not_hide_a_real_mulx_definition(self):
        for prefix in ("", "67", "6767"):
            with self.subTest(prefix=prefix), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                request = self.select(self.fixture(Path(directory), prefix))
                self.assertEqual(request["mulx_machine_eas"], ["0x140001040"])
                self.assertEqual(request["ext_machine_eas"][0], hex(0x140001045 + len(bytes.fromhex(prefix))))

    def test_shared_tail_context_keeps_full_original_bytes_and_rejects_changed_owners(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); request = self.select(f)
            owner, helper = request["owners"][0], request["helpers"][0]
            start = int(helper["address"], 16) + 5
            def chunk(first, last):
                code = f.raw[0x200 + first - f.base - 0x1000:0x200 + last - f.base - 0x1000]
                return {"start": hex(first), "end": hex(last), "raw_hex": code.hex(),
                        "sha256": hashlib.sha256(code).hexdigest()}
            tail = chunk(start, int(helper["end"], 16))
            plans = []
            for row, chunks, end in ((owner, [*owner["chunks"], tail], owner["end"]),
                (helper, [chunk(int(helper["address"], 16), start), tail], hex(start))):
                plans.append({"rva": row["rva"], "source_function": row["source_function"],
                    "original_chunks": row["chunks"], "original_end": row["end"],
                    "expected_chunks": chunks, "expected_end": end})
            request["shared_tails"] = [{"primary_owner_rva": helper["rva"], "expected_tail": tail, "owners": plans}]
            expanded = boundaries.optimizer_context_body(request, owner)
            evidence.original_machine(f.raw, expanded)
            self.assertEqual([ins["address"] for ins in expanded["assembly"]],
                [ins["address"] for ins in [*owner["instructions"], *helper["instructions"][1:]]])
            split = boundaries.optimizer_context_body(request, helper)
            evidence.original_machine(f.raw, split)
            self.assertEqual(split["assembly"], helper["instructions"])
            self.assertEqual(split["end"], hex(start))
            request["shared_tails"][0]["owners"][0]["original_end"] = "0x140001005"
            with self.assertRaises(ValueError): boundaries.optimizer_context_body(request, owner)

    def test_partial_nearest_kills_non_mulx_nested_ext_and_bad_def_sets_are_rejected(self):
        mutations = [lambda s: s["blocks"][0]["instructions"][1].update(must_def_registers=list(range(8, 15))),
            lambda s: s["blocks"][0]["instructions"][1].update(mapped_ea="0x140001000"),
            lambda s: s["blocks"][0]["instructions"][1]["operands"]["d"].update(register=9),
            lambda s: s["blocks"][0]["instructions"][1].update(may_def_registers=[8, 8]),
            lambda s: s["blocks"][0]["instructions"][0].update(opcode=62),
            lambda s: s["blocks"][0]["instructions"][2].update(mapped_ea="0x140001000"),
            lambda s: s.update(maturity=2), lambda s: s.update(entry_rva="0x1040"),
            lambda s: s["blocks"][0].update(serial=1)]
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); request = self.select(f)
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    snapshot = self.snapshot(); mutation(snapshot)
                    with self.assertRaises(ValueError): boundaries.optimizer_expected_changes(request, snapshot)
            snapshot = self.snapshot()
            kill = copy.deepcopy(snapshot["blocks"][0]["instructions"][1]); kill["mapped_ea"] = "0x140001000"
            snapshot["blocks"][0]["instructions"].insert(2, kill)
            with self.assertRaises(ValueError): boundaries.optimizer_expected_changes(request, snapshot)

    def test_any_out_of_scope_bit_operand_ea_opcode_cfg_or_missing_instruction_fails(self):
        mutations = [lambda r: r["after"]["blocks"][0]["instructions"][3].update(iprops=131072),
            lambda r: r["after"]["blocks"][0]["instructions"][0].update(iprops=393216),
            lambda r: r["after"]["blocks"][0]["instructions"][0].update(iprops=0),
            lambda r: r["after"]["blocks"][0]["instructions"][0].update(ea="0x140001000"),
            lambda r: r["after"]["blocks"][0]["instructions"][0].update(opcode=4),
            lambda r: r["after"]["blocks"][0]["instructions"][0]["operands"]["l"].update(register=25),
            lambda r: r["after"]["blocks"][0]["successors"].append(1),
            lambda r: r["after"]["frame"].update(frsize=64),
            lambda r: r["after"]["blocks"][0]["instructions"].pop(),
            lambda r: r["scope"]["changes"].pop(),
            lambda r: r.update(iprop_dont_prop=262144)]
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            request = self.select(self.fixture(Path(directory)))
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    row = self.applied(request); mutation(row)
                    with self.assertRaises(ValueError): boundaries.verify_optimizer_property_pass(request, row)

    def test_complete_typed_capture_requires_original_states_every_stage_and_hook_cleanup(self):
        mutations = [lambda b: b["optimizer_property_audit"].update(hook_unregistered=False),
            lambda b: b["optimizer_property_audit"]["original_state_before"].pop(),
            lambda b: b["optimizer_property_audit"]["original_state_after"][1]["native_frame"].update(frsize=8),
            lambda b: b["optimizer_property_audit"]["stages"].pop(2),
            lambda b: b["optimizer_property_audit"]["stages"][0]["snapshot"].update(entry_rva="0x1040"),
            lambda b: b["optimizer_property_audit"]["stages"][1].update(phase="generated"),
            lambda b: b["optimizer_property_audit"]["passes"][0]["before"].update(entry_rva="0x1040"),
            lambda b: b.pop("type_after_decompile"),
            lambda b: b["pseudocode"]["locals_capture"].update(complete=False)]
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory)); request = self.select(f)
            body = copy.deepcopy(f.bodies[0]); body["pseudocode"] = {"lines": [{"text": "complete"}], "locals": []}
            test_evidence.NativeSupplementTests.typed_fixture(body)
            body["optimizer_property_audit"] = self.audit(request)
            boundaries.verify_optimizer_properties([request], body)
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    bad = copy.deepcopy(body); mutation(bad)
                    with self.assertRaises(ValueError): boundaries.verify_optimizer_properties([request], bad)
            with self.assertRaises(ValueError): boundaries.verify_optimizer_properties([], body)

    def test_sdk_application_changes_only_the_selected_bit_and_preserves_failure_checkpoints(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            request = self.select(self.fixture(Path(directory)))
            for issue in (None, "extra_bit", "interrupt", "checkpoint_cancel"):
                with self.subTest(issue=issue):
                    before, actual = self.snapshot(), []
                    class Instruction:
                        def clr_propagatable(self):
                            if issue == "interrupt": raise KeyboardInterrupt()
                            self.iprops |= 131072 | (262144 if issue == "extra_bit" else 0)
                    def operand(row):
                        result = SimpleNamespace(t=row["kind"])
                        if "instruction" in row: result.d = instruction(row["instruction"])
                        if "address_operand" in row: result.a = operand(row["address_operand"])
                        return result
                    def instruction(row):
                        result = Instruction(); result.iprops = row["iprops"]
                        actual.append(result)
                        for slot in ("l", "r", "d"): setattr(result, slot, operand(row["operands"][slot]))
                        return result
                    top = [instruction(row) for row in before["blocks"][0]["instructions"]]
                    for index, ins in enumerate(top): ins.next = top[index + 1] if index + 1 < len(top) else None
                    mba = SimpleNamespace(qty=1, get_mblock=lambda serial: SimpleNamespace(head=top[0]))
                    sdk = SimpleNamespace(hexrays=SimpleNamespace(IPROP_DONT_PROP=131072, mop_d=3, mop_a=9))
                    def freeze(*args, **kwargs):
                        result = copy.deepcopy(before)
                        for (_, _, _, row), ins in zip(boundaries.optimizer_instruction_rows(result), actual):
                            row["iprops"] = ins.iprops
                        return result
                    checkpoints = []
                    def checkpoint(row):
                        checkpoints.append(copy.deepcopy(row))
                        if issue == "checkpoint_cancel": raise KeyboardInterrupt()
                    with patch.object(boundaries, "optimizer_snapshot", side_effect=freeze):
                        if issue is None:
                            row = boundaries.apply_optimizer_properties(request, mba, 0, sdk, checkpoint)
                            self.assertEqual(row, self.applied(request))
                            self.assertEqual(len(checkpoints), 2)
                        else:
                            with self.assertRaises((ValueError, KeyboardInterrupt)):
                                boundaries.apply_optimizer_properties(request, mba, 0, sdk, checkpoint)
                            self.assertTrue(checkpoints)
                            if issue == "checkpoint_cancel": self.assertTrue(all(ins.iprops == 0 for ins in actual))

    def test_owned_sdk_hooks_are_inert_and_released_on_failure_or_cancellation(self):
        source = Path(boundaries.__file__).parent / "ida/supplement.py"
        tree = ast.parse(source.read_text(encoding="utf-8"))
        definition = next(row for row in tree.body if isinstance(row, ast.FunctionDef)
                          and row.name == "decompile_with_optimizer_properties")
        for issue in ("decompile", "interrupt", "hook", "unhook"):
            with self.subTest(issue=issue):
                controllers = []
                class Hooks:
                    def __init__(self): controllers.append(self)
                    def hook(self): return issue != "hook"
                    def unhook(self): self.released = True; return issue != "unhook"
                def decompile(*args):
                    if issue == "interrupt": raise KeyboardInterrupt()
                    raise ValueError("decompilation failed")
                namespace = {"ida_hexrays": SimpleNamespace(Hexrays_Hooks=Hooks),
                             "traceback": __import__("traceback"), "decompile_with_diagnostics": decompile}
                exec(compile(ast.Module(body=[definition], type_ignores=[]), str(source), "exec"), namespace)
                audit = {}
                with patch.object(boundaries, "optimizer_original_states", return_value=[{"unchanged": True}]):
                    with self.assertRaises((ValueError, KeyboardInterrupt)):
                        namespace[definition.name](0x140001000, None, {}, {}, 0x140000000, audit, None)
                self.assertTrue(controllers[0].released)
                self.assertFalse(controllers[0].active)
                self.assertFalse(audit["complete"])
                self.assertEqual(audit["original_state_before"], audit["original_state_after"])
                self.assertEqual(audit["hook_unregistered"], issue != "unhook")



class RecursiveFallthroughContextTests(unittest.TestCase):
    def fixture(self, root):
        raw = bytearray(test_evidence.NativeSupplementTests().fixture())
        raw.extend(b"\0" * 0x400)
        struct.pack_into("<I", raw, 0x188 + 8, 0x400)
        struct.pack_into("<I", raw, 0x188 + 16, 0x400)
        struct.pack_into("<I", raw, 0x188 + 40 + 20, 0x600)
        base, start, middle = 0x140000000, 0x1000, 0x1070
        prefix = ["57", "56", "53", "55", "4889e5", "4883e4e0", "4881ecb0010000"]
        suffix = ["4c6311", "4c8b5908", "4c89d6", "6690"]
        length = len(bytes.fromhex("".join(prefix + suffix)))
        root_parts = prefix + ["90"] * (112 - length) + suffix
        core_parts, cursor = [], middle
        for _ in range(11):
            core_parts.append("4152")
            cursor += 2
            for _ in range(2):
                core_parts.append("e8" + struct.pack("<i", middle - cursor - 5).hex())
                cursor += 5
            core_parts.extend(["415a", "c3"])
            cursor += 3
        core_parts += ["c3"] * 3
        bodies = []
        for at, parts in ((start, root_parts), (middle, core_parts)):
            code = bytes.fromhex("".join(parts))
            raw[0x200 + at - start:0x200 + at - start + len(code)] = code
            first, instructions = at, []
            for part in parts:
                data = bytes.fromhex(part)
                instructions.append({"address": hex(base + at), "rva": hex(at), "bytes": part,
                    "text": "original instruction", "xrefs_from": [], "xrefs_to": []})
                at += len(data)
            bodies.append({"address": hex(base + first), "rva": hex(first), "end": hex(base + at),
                "flags": 0x400, "type": None, "xrefs_to_entry": [], "assembly": instructions,
                "chunks": [{"start": hex(base + first), "end": hex(base + at),
                    "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}]})
        first, second = bodies
        flow = {"from": first["assembly"][-1]["address"], "to": second["address"],
                "type": 21, "code": True, "user": False}
        first["assembly"][-1]["xrefs_from"] = [flow]
        calls = [row for row in second["assembly"] if row["bytes"].startswith("e8")]
        second["xrefs_to_entry"] = [flow, *[{"from": row["address"], "to": second["address"],
            "type": 17, "code": True, "user": False} for row in calls]]
        source = root / ".analysis/inputs/recursive.dll"
        source.parent.mkdir(parents=True)
        source.write_bytes(raw)
        result = root / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        identities, records = [], []
        for body in bodies:
            identity = write(result / "functions" / (body["rva"][2:] + ".json"), body)
            identities.append(identity)
            records.append({**identity, "path": "functions/" + body["rva"][2:] + ".json",
                "rva": body["rva"], "address": body["address"], "assembly_captured": True})
        capture = {"input": analysis.rel(source), "input_sha256": hashlib.sha256(raw).hexdigest(),
            "input_bytes": len(raw), "capture_complete": True, "image_base": hex(base), "functions": records}
        write(result / "capture.json", capture)
        proof = boundaries.recursive_context_proof(bytes(raw), first, second)
        request = {"root_rva": first["rva"], "core_rva": second["rva"], "root_function": identities[0],
            "core_function": identities[1], "review_source": {"path": ".analysis/results/review.json",
                "bytes": 1, "sha256": "0" * 64, "pointer": ""}, "proof": proof}
        return SimpleNamespace(raw=bytes(raw), bodies=bodies, source=source, result=result,
            request=request, write=write, capture=capture)

    def audit(self, f):
        snapshots = []
        for body in f.bodies:
            snapshots.append({"address": body["address"], "end": body["end"], "chunks": body["chunks"],
                "flags": 0x400, "type_present": False, "type": None,
                "native_frame": {key: 0 for key in ("frame", "frsize", "frregs", "argsize", "fpd")},
                "stack_points": [{"ea": row["address"], "spd": 0, "delta": 0} for row in body["assembly"]]})
        args = [{"name": name, "location": {"kind": "register", "register": reg, "complete": True}}
                for name, reg in zip(("destination", "source", "permutation", "subtree_size", "root_size"),
                                     (2, 8, 11, 10, 6))]
        return {"request": f.request, "before": snapshots, "after": copy.deepcopy(snapshots),
            **{key: True for key in ("complete", "hook_registered", "hook_unregistered",
                "abi_hook_registered", "abi_hook_unregistered", "type_cache_computed_under_abi_hook")},
            **{key: False for key in ("target_execution", "manual_stack_points_modified",
                "manual_frames_modified", "instructions_patched", "microcode_modified")},
            "warnings": [], "errors": [],
            "sdk_inline": {"root_ea": f.bodies[0]["address"], "core_ea": f.bodies[1]["address"],
                "edge_ea": hex(int(f.bodies[1]["address"], 16) - 1), "flag_names": ["INLINE_DONTCOPY"],
                "return_code": 0, "inserted_block_interval": [2, 80]},
            "core_type_readback": {"cc": 240, "stack_argument_bytes": 0, "arguments": args,
                "return_location": {"kind": "register_pair", "register": 2, "second_register": 11}},
            "recursive_calls": [{"ea": row["address"], "callee": f.bodies[1]["address"], "cc": 240,
                "stkargs_top": 0, "root_shadow_size": 32, "return_registers": "rdx.8,r11.8",
                "arguments": args, "spoiled": ",".join("ymm" + str(i) + ".32" for i in range(16))}
                for row in f.request["proof"]["recursive_call_instructions"]],
            "saved_r10_pairs": [{**row, "store": {"ea": row["push_ea"], "opcode": 4,
                "source_register": "r10", "stack_offset": 0, "size": 8}, "load": {
                    "ea": row["pop_ea"], "opcode": 4, "destination_register": "r10", "stack_offset": 0, "size": 8}}
                for row in f.request["proof"]["saved_r10_instructions"]]}

    def test_complete_physical_union_keeps_original_call_targets_all_returns_and_r10_pairs(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            proof = f.request["proof"]
            self.assertEqual(len(proof["recursive_call_instructions"]), 22)
            self.assertEqual(len(proof["all_original_return_instructions"]), 14)
            self.assertEqual(len(proof["saved_r10_instructions"]), 11)
            self.assertEqual(proof["original_machine_union"]["bytes"],
                             sum(len(bytes.fromhex(body["chunks"][0]["raw_hex"])) for body in f.bodies))
            boundaries.verify_recursive_contexts(analysis, [f.request], [self.audit(f)])

    def test_modified_machine_missing_ret_call_foreign_entry_or_broken_frame_are_rejected(self):
        mutations = [lambda first, core: first.update(end=first["address"]),
            lambda first, core: first["assembly"][-1].update(bytes="90"),
            lambda first, core: core["assembly"].pop(),
            lambda first, core: core["assembly"].pop(1),
            lambda first, core: core["assembly"][1].update(bytes="e800000000"),
            lambda first, core: core["xrefs_to_entry"].pop(),
            lambda first, core: core["xrefs_to_entry"].append({"from": "0x140001999", "type": 17, "code": True}),
            lambda first, core: first["assembly"][-1]["xrefs_from"].clear(),
            lambda first, core: core["chunks"][0].update(raw_hex="90"),
            lambda first, core: core["assembly"][0].update(address="0x140001071")]
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    first, core = copy.deepcopy(f.bodies)
                    mutation(first, core)
                    with self.assertRaises((ValueError, IndexError)):
                        boundaries.recursive_context_proof(f.raw, first, core)

    def test_no_shadow_cached_abi_missing_calls_wrong_root_ownership_and_dropped_stack_values_fail(self):
        mutations = [lambda row: row.update(type_cache_computed_under_abi_hook=False),
            lambda row: row.update(abi_hook_unregistered=False),
            lambda row: row.update(hook_unregistered=False),
            lambda row: row["sdk_inline"].update(flag_names=["INLINE_EXTFRAME", "INLINE_DONTCOPY"]),
            lambda row: row["sdk_inline"].update(root_ea="0x140001070"),
            lambda row: row["sdk_inline"].update(inserted_block_interval=[4, 4]),
            lambda row: row["core_type_readback"].update(stack_argument_bytes=32),
            lambda row: row["core_type_readback"]["arguments"][0]["location"].update(register=1),
            lambda row: row["recursive_calls"].pop(),
            lambda row: row["recursive_calls"][0].update(stkargs_top=32),
            lambda row: row["recursive_calls"][0].update(root_shadow_size=0),
            lambda row: row["recursive_calls"][0].update(callee="0x140001080"),
            lambda row: row["recursive_calls"][0].update(spoiled="rax"),
            lambda row: row["recursive_calls"][0].update(return_registers="rax.8"),
            lambda row: row["recursive_calls"][0].update(arguments=[]),
            lambda row: row["saved_r10_pairs"].pop(),
            lambda row: row["saved_r10_pairs"][0]["load"].update(stack_offset=8),
            lambda row: row["saved_r10_pairs"][0]["store"].update(source_register="rax"),
            lambda row: row["before"][0]["native_frame"].update(frsize=32),
            lambda row: row["after"][1]["stack_points"][0].update(spd=-8),
            lambda row: row.update(warnings=[{"id": 45}]),
            lambda row: row.update(errors=[{"stage": "calls_done"}]),
            lambda row: row.update(microcode_modified=True),
            lambda row: row.update(complete=False)]
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            for mutation in mutations:
                with self.subTest(mutation=mutation):
                    row = self.audit(f)
                    mutation(row)
                    with self.assertRaises(ValueError):
                        boundaries.verify_recursive_contexts(analysis, [f.request], [row])
            with self.assertRaises(ValueError):
                boundaries.verify_recursive_contexts(analysis, [f.request], [])

    def test_recursive_review_cannot_combine_with_type_stack_frame_or_inline_repair(self):
        raw = bytes(test_evidence.NativeSupplementTests().fixture())
        capture = {"functions": [{"rva": "0x1000", "address": "0x140001000"}]}
        for options in ({"stack_reviews": ["review"]}, {"shared_tail_context_reviews": ["review"]},
                        {"fresh_inline_rvas": [0x1000]}, {"frame_reviews": ["review"]},
                        {"type_specs": ["0x1000=void recovered();"], "type_note": "explicit"},
                        {"optimizer_property_reviews": ["review"]}):
            with self.subTest(options=options), self.assertRaisesRegex(ValueError, "Recursive root contexts"):
                analysis.native_function_selection(raw, capture, [0x1000],
                    recursive_context_reviews=["review"], **options)



if __name__ == "__main__":
    unittest.main()
