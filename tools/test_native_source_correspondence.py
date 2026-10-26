"""Pure COFF/PE proofs, rejected counterexamples and static compiler cleanup."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import native_source_correspondence as sources


def coff(data, symbols, relocations=(), timestamp=0, flags=0x60000020):
    """Small independently encoded COFF fixtures; symbol aux records remain real slots."""
    strings = bytearray(b"\0" * 4)
    entries = bytearray()
    symbol_count = 0
    for name, value, section, aux in symbols:
        encoded = name.encode("ascii")
        if len(encoded) > 8:
            name_field = struct.pack("<II", 0, len(strings))
            strings.extend(encoded + b"\0")
        else:
            name_field = encoded.ljust(8, b"\0")
        entries.extend(name_field + struct.pack("<IhHBB", value, section, 0x20, 2, aux))
        entries.extend(b"\0" * (18 * aux))
        symbol_count += 1 + aux
    struct.pack_into("<I", strings, 0, len(strings))
    rels = b"".join(struct.pack("<IIH", *row) for row in relocations)
    symbol_offset = 60 + len(data) + len(rels)
    header = struct.pack("<HHIIIHH", 0x8664, 1, timestamp, symbol_offset, symbol_count, 0, 0)
    section = b".text\0\0\0" + struct.pack("<IIIIIIHHI", 0, 0, len(data), 60,
        60 + len(data), 0, len(relocations), 0, flags)
    return header + section + data + rels + entries + strings


def pe64(data, rva=0x1000):
    raw = bytearray(0x200 + len(data))
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3c, 0x80)
    raw[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", raw, 0x84, 0x8664, 1)
    struct.pack_into("<H", raw, 0x94, 0xf0)
    struct.pack_into("<H", raw, 0x98, 0x20b)
    struct.pack_into("<Q", raw, 0xb0, 0x180000000)
    raw[0x188:0x190] = b".text\0\0\0"
    struct.pack_into("<IIII", raw, 0x190, len(data), rva, len(data), 0x200)
    raw[0x200:] = data
    return bytes(raw)


class CoffRelocationTests(unittest.TestCase):
    def test_all_six_rel32_types_resolve_signed_addends_and_real_symbol_indexes(self):
        for relocation_type in range(4, 10):
            for addend in (-7, 0, 11):
                with self.subTest(type=relocation_type, addend=addend):
                    data = b"\xe8" + struct.pack("<i", addend) + b"\xc3" + b"\xcc" * 58 + b"\xc3"
                    obj = sources.parse_coff(coff(data, [("Kernel", 0, 1, 1),
                        ("TargetLongName", 64, 1, 0)], [(1, 2, relocation_type)]))
                    actual, rows = sources.relocate_function(obj, "Kernel", 0x1000, 6,
                        {"Kernel": 0x1000, "TargetLongName": 0x2000})
                    # Index 1 is auxiliary; the real target symbol is index 2.
                    expected = 0x2000 + addend - 0x1005 - (relocation_type - 4)
                    self.assertEqual(actual, b"\xe8" + struct.pack("<i", expected) + b"\xc3")
                    self.assertEqual(rows[0]["target_name"], "TargetLongName")
                    self.assertEqual(rows[0]["addend"], addend)

    def test_timestamp_is_not_a_machine_byte_or_symbol_identity(self):
        data = b"\x90\xc3"
        first = sources.parse_coff(coff(data, [("Kernel", 0, 1, 0)], timestamp=1))
        second = sources.parse_coff(coff(data, [("Kernel", 0, 1, 0)], timestamp=0xffffffff))
        self.assertEqual(first, second)

    def test_auxiliary_symbol_reference_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "auxiliary"):
            sources.parse_coff(coff(b"\0" * 8, [("Kernel", 0, 1, 1)], [(0, 1, 4)]))

    def test_duplicate_symbol_names_are_not_silently_overwritten(self):
        with self.assertRaisesRegex(ValueError, "duplicate"):
            sources.parse_coff(coff(b"\0" * 8, [("Kernel", 0, 1, 0), ("Kernel", 4, 1, 0)]))

    def test_truncated_and_wrong_machine_objects_fail(self):
        raw = coff(b"\0" * 8, [("Kernel", 0, 1, 0)])
        for size in (0, 19, 59, len(raw) - 1):
            with self.subTest(size=size), self.assertRaises(ValueError):
                sources.parse_coff(raw[:size])
        altered = bytearray(raw)
        struct.pack_into("<H", altered, 0, 0x14c)
        with self.assertRaisesRegex(ValueError, "AMD64"):
            sources.parse_coff(bytes(altered))

    def test_unknown_external_and_overlapping_relocations_fail(self):
        for rels, addresses, pattern in [
            ([(0, 1, 3)], {"Target": 0x2000}, "Unsupported"),
            ([(0, 1, 4)], {}, "outside"),
            ([(0, 1, 4), (1, 1, 4)], {"Target": 0x2000}, "Overlapping"),
            ([(6, 1, 4)], {"Target": 0x2000}, "boundary"),
        ]:
            with self.subTest(relocations=rels), self.assertRaisesRegex(ValueError, pattern):
                obj = sources.parse_coff(coff(b"\0" * 12,
                    [("Kernel", 0, 1, 0), ("Target", 8, 1, 0)], rels))
                sources.relocate_function(obj, "Kernel", 0x1000, 8, addresses)

    def test_missing_symbols_noncode_sections_and_signed_overflow_fail(self):
        obj = sources.parse_coff(coff(b"\0" * 8, [("Kernel", 0, 1, 0)]))
        with self.assertRaisesRegex(ValueError, "defined"):
            sources.relocate_function(obj, "Missing", 0x1000, 1, {})
        noncode = sources.parse_coff(coff(b"\0" * 8, [("Kernel", 0, 1, 0)], flags=0x40000040))
        with self.assertRaisesRegex(ValueError, "code section"):
            sources.relocate_function(noncode, "Kernel", 0x1000, 1, {})
        relocated = sources.parse_coff(coff(b"\0" * 8, [("Kernel", 0, 1, 0), ("Target", 4, 1, 0)], [(0, 1, 4)]))
        with self.assertRaisesRegex(ValueError, "overflows"):
            sources.relocate_function(relocated, "Kernel", 0x1000, 4, {"Target": 1 << 40})

    def test_pe_parser_requires_full_raw_bytes_and_exact_coordinates(self):
        pe = sources.Pe64.parse(pe64(b"\x90\xc3"))
        self.assertEqual(pe.image_base, 0x180000000)
        self.assertEqual(pe.at(0x1000, 2), b"\x90\xc3")
        for address, count in ((0xfff, 1), (0x1001, 2), (0x1000, 0)):
            with self.subTest(address=address, count=count), self.assertRaises(ValueError):
                pe.at(address, count)
        with self.assertRaises(ValueError):
            sources.Pe64.parse(pe64(b"\x90\xc3")[:-1])


class StaticCompilerLifecycleTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.compiler = self.root / "ml64.exe"
        self.compiler.write_bytes(b"static fixture: never executable")
        self.compiler_hash = hashlib.sha256(self.compiler.read_bytes()).hexdigest()
        self.a = SimpleNamespace(resolve=lambda name, boundary: self.root / name)
        self.sources = {name: b"; static source snapshot\n" for name in sources._SOURCE_FILES}
        self.asm_id = {"sha256": self.compiler_hash, "bytes": self.compiler.stat().st_size}
        for target, value in (("_ASSEMBLER_PATH", self.compiler), ("_ASSEMBLER_SHA256", self.compiler_hash)):
            patcher = patch.object(sources, target, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def assert_released(self):
        self.assertEqual(list((self.root / "target/verification-temp").iterdir()), [])

    def test_success_uses_private_snapshots_and_removes_outputs(self):
        def run(command, **kwargs):
            folder = Path(kwargs["cwd"])
            self.assertEqual(kwargs["env"]["TEMP"], str(folder))
            self.assertEqual({p.name for p in folder.iterdir()}, sources._SOURCE_FILES)
            self.assertTrue(command[1:3] == ["/nologo", "/c"])
            (folder / "kernel.obj").write_bytes(b"object bytes")
            return SimpleNamespace(returncode=0, stderr=b"")
        with patch.object(sources.subprocess, "run", side_effect=run) as call:
            self.assertEqual(sources._assemble(self.a, self.sources, self.asm_id), b"object bytes")
            call.assert_called_once()
        self.assert_released()

    def test_failure_timeout_and_cancel_all_release_owned_files(self):
        for failure in (SimpleNamespace(returncode=1, stderr=b"compile failed"),
                        subprocess.TimeoutExpired("ml64", 45), KeyboardInterrupt()):
            with self.subTest(kind=type(failure).__name__):
                kwargs = {"side_effect": failure} if isinstance(failure, BaseException) else {"return_value": failure}
                with patch.object(sources.subprocess, "run", **kwargs):
                    with self.assertRaises(type(failure) if isinstance(failure, BaseException) else ValueError):
                        sources._assemble(self.a, self.sources, self.asm_id)
                self.assert_released()

    def test_changed_compiler_and_escaping_source_are_rejected(self):
        with patch.object(sources.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "assembler"):
                sources._assemble(self.a, self.sources, {"sha256": "0" * 64, "bytes": 1})
            run.assert_not_called()
            with self.assertRaisesRegex(ValueError, "owned include"):
                sources._assemble(self.a, {"../escape.asm": b"code"}, self.asm_id)
            run.assert_not_called()
        self.assert_released()
        self.assertFalse((self.root / "target/escape.asm").exists())


class SourceReviewFixture:
    def __init__(self, root):
        self.root = root
        self.a = SimpleNamespace(ROOT=root, resolve=self.resolve, rel=lambda p: p.relative_to(root).as_posix())
        entries = [(rva, row[0]) for rva, row in sources._OWNERS.items()]
        entries += [(hex(0xa42000 + i * 16), "PrivateHelper" + str(i)) for i in range(25)]
        self.entries = sorted(entries, key=lambda row: int(row[0], 0))
        start = int(self.entries[0][0], 0)
        end = int(self.entries[-1][0], 0) + 5
        machine = bytearray(b"\xcc" * (end - start))
        self.function_bodies = {}
        for i, (rva, _name) in enumerate(self.entries):
            address = int(rva, 0)
            if i + 1 == len(self.entries):
                code, text, edges = b"\xc3", "retn", []
            else:
                target = int(self.entries[i + 1][0], 0)
                code = b"\xe8" + struct.pack("<i", target - address - 5)
                text, edges = "call helper", [{"code": True, "type": 17, "to": hex(0x180000000 + target)}]
            machine[address - start:address - start + len(code)] = code
            self.function_bodies[rva] = {"rva": rva, "address": hex(0x180000000 + address),
                "end": hex(0x180000000 + address + len(code)), "pseudocode": None,
                "chunks": [{"start": hex(0x180000000 + address), "end": hex(0x180000000 + address + len(code)),
                    "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}],
                "assembly": [{"rva": rva, "address": hex(0x180000000 + address), "bytes": code.hex(),
                    "text": text, "xrefs_from": edges}]}
        self.image_id = self.write(".analysis/inputs/test-source/image.dll", pe64(bytes(machine), start))
        self.base = ".analysis/results/native/" + self.image_id["sha256"]
        self.object_raw = coff(bytes(machine), [(name, int(rva, 0) - start, 1, 0) for rva, name in self.entries])
        self.object_id = self.write(".analysis/results/reverse-pending/review-sources/source.obj", self.object_raw)
        self.source_rows = []
        for name in sorted(sources._SOURCE_FILES):
            self.source_rows.append({**self.write(".analysis/inputs/test-source/" + name, b"; static fixture\n"), "includes": []})
        self.acquisition_id = self.write(".analysis/inputs/test-source/acquisition.json", {
            "source_image": self.image_id, "dependency_closure_complete": True,
            "target_execution": False, "semantics_complete": False, "files": self.source_rows})
        self.review_source_rows = [sources._identity(x) for x in self.source_rows] + [self.acquisition_id]
        self.write(".analysis/inputs.json", {"inputs": self.review_source_rows})
        self.rebuild()

    def resolve(self, name, boundary):
        path = self.root / name
        resolved = path.resolve()
        if not resolved.is_relative_to((self.root / boundary).resolve()):
            raise ValueError("Fixture path leaves its boundary")
        return path

    def write(self, name, value):
        raw = value if isinstance(value, bytes) else json.dumps(value).encode()
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    def rebuild(self):
        rows, closure = [], []
        for rva, name in self.entries:
            body = self.function_bodies[rva]
            reference = self.write(self.base + "/functions/" + rva[2:] + ".json", body)
            rows.append({"path": "functions/" + rva[2:] + ".json", "sha256": reference["sha256"]})
            raw = bytes.fromhex(body["chunks"][0]["raw_hex"])
            closure.append({"rva": rva, "symbol_name_lead": name, "function": reference,
                "machine_bytes": len(raw), "instruction_count": 1, "bytes_equal": True,
                "byte_differences": [], "original_sha256": hashlib.sha256(raw).hexdigest(),
                "relocated_object_sha256": hashlib.sha256(raw).hexdigest(), "relocations": []})
        capture_id = self.write(self.base + "/capture.json", {"input": self.image_id["path"],
            "input_sha256": self.image_id["sha256"], "input_bytes": self.image_id["bytes"],
            "image_base": "0x180000000", "functions": rows})
        correspondence_id = self.write(".analysis/results/reverse-pending/review-sources/correspondence.json", {
            "review_kind": "onnx_avx512_original_assembly_source_correspondence", "source_image": self.image_id,
            "capture": capture_id, "all_28_closure_bytes_equal": True, "target_execution": False,
            "database_mutation": False, "semantics_complete": False, "source_acquisition": self.acquisition_id,
            "object": self.object_id, "closure": closure, "assembler": {"sha256": sources._ASSEMBLER_SHA256}})
        self.review = {"schema_version": 1,
            "review_kind": "onnx_avx512_original_three_owner_complete_static_semantics",
            "static_semantics_review_complete": True, "original_machine_bytes_and_current_cpp_callers_reviewed": True,
            "semantics_complete": False, "rust_independent_implementation_complete": False,
            "dll_function_execution_verified": False, "target_execution": False, "debugger_used": False,
            "database_mutation": False, "source_image": self.image_id, "capture": capture_id,
            "image_base": "0x180000000", "input_sources": self.review_source_rows,
            "assembly_correspondence_receipt": correspondence_id,
            "instruction_coverage": {"inner_compute_blocks": [{}] * 36, "postprocess_variants": [{}] * 16,
                "all_active_accumulator_clear_fma_store_sets_verified": True},
            "actual_consumers": [{"kind": values[1], "execute_rva": values[2],
                "actual_cfg_indirect_call_rva": values[3]} for values in sources._OWNERS.values()],
            "capture_integrity": {"original_failures_preserved": {"no_hexrays_success_claim": True,
                "pseudocode_failures": [{"stage": rva[2:] + ":hexrays", "error": "test preserved code -12"}
                    for rva in sources._OWNERS]}}}
        self.review_id = self.write(".analysis/results/reverse-pending/review-sources/review.json", self.review)

    def verify(self, object_raw=None):
        with patch.object(sources, "_SEMANTIC_REVIEW_SHA256", self.review_id["sha256"]), \
                patch.object(sources, "_IMAGE_SHA256", self.image_id["sha256"]), \
                patch.object(sources, "_assemble", return_value=object_raw or self.object_raw), \
                patch.object(sources, "source_identity", return_value={"path": "tools/validator.py", "bytes": 1, "sha256": "0" * 64}):
            return sources.verify_review(self.a, self.review_id)


class SourceReviewTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.f = SourceReviewFixture(Path(directory.name))

    def test_complete_closure_proves_only_three_owners_and_preserves_all_other_dimensions(self):
        verified = self.f.verify()
        proofs = verified["complete_sources"][self.f.image_id["sha256"]]
        self.assertEqual(set(proofs), set(sources._OWNERS))
        self.assertEqual(verified["source_assembly"]["function_count"], 28)
        self.assertFalse(verified["pseudocode_recovered"])
        self.assertFalse(verified["runtime_acceptance"])
        for rva, proof in proofs.items():
            self.assertEqual(proof["rva"], rva)
            self.assertTrue(proof["manual_semantic_review"]["static_semantics_review_complete"])
            self.assertFalse(proof["rust_implementation_acceptance"])
            self.assertFalse(proof["target_execution"])

    def test_new_object_timestamp_is_ignored_but_instruction_changes_are_rejected(self):
        changed = bytearray(self.f.object_raw)
        struct.pack_into("<I", changed, 4, 0xffffffff)
        self.assertEqual(self.f.verify(bytes(changed))["source_assembly"]["function_count"], 28)
        changed[60] ^= 1
        with self.assertRaisesRegex(ValueError, "machine bytes differ"):
            self.f.verify(bytes(changed))

    def test_changing_a_call_edge_cannot_be_authorized_by_rehashing_receipts(self):
        rva = self.f.entries[0][0]
        self.f.function_bodies[rva]["assembly"][0]["xrefs_from"][0]["to"] = "0x180000001"
        self.f.rebuild()
        with self.assertRaisesRegex(ValueError, "CALL displacement"):
            self.f.verify()

    def test_missing_instruction_bytes_and_synthesized_pseudocode_fail(self):
        rva = self.f.entries[0][0]
        body = self.f.function_bodies[rva]
        original = deepcopy(body)
        body["assembly"][0]["bytes"] = "90"
        self.f.rebuild()
        with self.assertRaisesRegex(ValueError, "instruction bytes"):
            self.f.verify()
        self.f.function_bodies[rva] = original
        self.f.function_bodies[rva]["pseudocode"] = {"lines": ["fabricated success"]}
        self.f.rebuild()
        with self.assertRaisesRegex(ValueError, "cfunc failure"):
            self.f.verify()

    def test_source_and_manual_review_changes_do_not_get_implicit_approval(self):
        path = self.f.root / self.f.source_rows[0]["path"]
        path.write_bytes(b"changed source")
        with self.assertRaisesRegex(ValueError, "bytes changed"):
            self.f.verify()
        with patch.object(sources, "_SEMANTIC_REVIEW_SHA256", "0" * 64):
            with self.assertRaisesRegex(ValueError, "explicitly approved"):
                sources.verify_review(self.f.a, self.f.review_id)

    def test_true_runtime_flags_and_removed_consumers_or_failures_are_rejected(self):
        for key in ("target_execution", "dll_function_execution_verified", "rust_independent_implementation_complete"):
            changed = deepcopy(self.f.review)
            changed[key] = True
            with self.subTest(key=key), patch.object(sources, "_SEMANTIC_REVIEW_SHA256", self.f.review_id["sha256"]):
                with self.assertRaisesRegex(ValueError, "boundary"):
                    sources._semantic_boundary(changed, self.f.review_id)
        changed = deepcopy(self.f.review)
        changed["actual_consumers"].pop()
        with patch.object(sources, "_SEMANTIC_REVIEW_SHA256", self.f.review_id["sha256"]):
            with self.assertRaisesRegex(ValueError, "consumer"):
                sources._semantic_boundary(changed, self.f.review_id)
        changed = deepcopy(self.f.review)
        changed["capture_integrity"]["original_failures_preserved"]["pseudocode_failures"].clear()
        with patch.object(sources, "_SEMANTIC_REVIEW_SHA256", self.f.review_id["sha256"]):
            with self.assertRaisesRegex(ValueError, "Hex-Rays"):
                sources._semantic_boundary(changed, self.f.review_id)

    def test_rehashed_but_unreachable_helpers_fail(self):
        first = self.f.entries[3][0]
        offset = int(first, 0) - int(self.f.entries[0][0], 0)
        # End the helper chain and update source/image identities consistently.
        body = self.f.function_bodies[first]
        body["chunks"][0]["raw_hex"] = "c3"
        body["chunks"][0]["end"] = body["end"] = hex(int(body["address"], 0) + 1)
        body["chunks"][0]["sha256"] = hashlib.sha256(b"\xc3").hexdigest()
        body["assembly"][0].update(bytes="c3", text="retn", xrefs_from=[])
        image = bytearray((self.f.root / self.f.image_id["path"]).read_bytes())
        image[0x200 + offset] = 0xc3
        self.f.image_id = self.f.write(self.f.image_id["path"], bytes(image))
        # The acquisition must bind even a rehashed image before closure checks run.
        acquisition = json.loads((self.f.root / self.f.acquisition_id["path"]).read_bytes())
        acquisition["source_image"] = self.f.image_id
        self.f.acquisition_id = self.f.write(self.f.acquisition_id["path"], acquisition)
        self.f.review_source_rows[-1] = self.f.acquisition_id
        self.f.write(".analysis/inputs.json", {"inputs": self.f.review_source_rows})
        self.f.base = ".analysis/results/native/" + self.f.image_id["sha256"]
        raw_object = bytearray(self.f.object_raw)
        raw_object[60 + offset] = 0xc3
        self.f.object_raw = bytes(raw_object)
        self.f.object_id = self.f.write(self.f.object_id["path"], self.f.object_raw)
        self.f.rebuild()
        with self.assertRaisesRegex(ValueError, "unreachable"):
            self.f.verify()

    def test_indexed_source_identity_is_required_even_when_bytes_exist(self):
        self.f.write(".analysis/inputs.json", {"inputs": []})
        with self.assertRaisesRegex(ValueError, "exact byte identity"):
            self.f.verify()

    def test_compiler_time_mutation_of_original_capture_is_rejected(self):
        first = self.f.entries[0][0]
        function_path = self.f.root / self.f.base / "functions" / (first[2:] + ".json")
        def alter(*_args):
            function_path.write_text("{}", encoding="utf-8")
            return self.f.object_raw
        with patch.object(sources, "_SEMANTIC_REVIEW_SHA256", self.f.review_id["sha256"]), \
                patch.object(sources, "_IMAGE_SHA256", self.f.image_id["sha256"]), \
                patch.object(sources, "_assemble", side_effect=alter):
            with self.assertRaisesRegex(ValueError, "bytes changed"):
                sources.verify_review(self.f.a, self.f.review_id)


if __name__ == "__main__":
    unittest.main()
