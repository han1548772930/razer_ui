"""Static byte-ownership proofs must reject incomplete or forged evidence."""
from __future__ import annotations

from contextlib import redirect_stdout
import copy
import hashlib
import io
import json
from pathlib import Path
import struct
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import analysis
from host import inspect_embedded_pe_ranges
import native_embedded as embedded


def pe_image(bits, sections, total_bytes):
    raw = bytearray(total_bytes)
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3c, 0x80)
    raw[0x80:0x84] = b"PE\0\0"
    size = 240 if bits == 64 else 224
    struct.pack_into("<HHIIIHH", raw, 0x84, 0x8664 if bits == 64 else 0x14c,
                     len(sections), 0, 0, 0, size, 0x2102)
    optional = 0x98
    struct.pack_into("<H", raw, optional, 0x20b if bits == 64 else 0x10b)
    struct.pack_into("<Q" if bits == 64 else "<I", raw, optional + (24 if bits == 64 else 28),
                     0x180000000 if bits == 64 else 0x10000)
    struct.pack_into("<I", raw, optional + 16, 0x1000)
    struct.pack_into("<II", raw, optional + 32, 0x1000, 0x200)
    struct.pack_into("<II", raw, optional + 56, 0x3000 if bits == 64 else 0x2000, 0x200)
    struct.pack_into("<H", raw, optional + 68, 1)
    struct.pack_into("<I", raw, optional + (108 if bits == 64 else 92), 16)
    for index, (name, rva, offset, length, flags) in enumerate(sections):
        at = optional + size + index * 40
        struct.pack_into("<8sIIII", raw, at, name.encode(), length, rva, length, offset)
        struct.pack_into("<I", raw, at + 36, flags)
    return raw


class AttributionFixture:
    def __init__(self, root, *, parent_executable=False, child_executable=True):
        self.root = root
        self.parent_base, self.child_base = 0x180000000, 0x10000
        self.parent_rva, self.child_rva, self.embedded_offset = 0x2410, 0x1010, 0x600
        self.child_raw = pe_image(32, [(".text", 0x1000, 0x200, 0x200,
            0x60000020 if child_executable else 0x40000040)], 0x400)
        self.child_raw[0x200] = 0xc3
        self.child_raw[0x210:0x213] = bytes.fromhex("4090c3")
        self.child_raw[0x220] = 0xc3
        self.parent_raw = pe_image(64, [(".text", 0x1000, 0x200, 0x200, 0x60000020),
            (".data", 0x2000, 0x400, 0x800, 0x60000020 if parent_executable else 0xc0000040)], 0xc00)
        self.parent_raw[0x200] = self.parent_raw[0x210] = 0xc3
        self.parent_raw[0x600:0xa00] = self.child_raw
        self.parent = self.write(".analysis/inputs/official-native-current/parent.dll", bytes(self.parent_raw))
        self.child = self.write(".analysis/inputs/official-native-current/embedded/driver.sys", bytes(self.child_raw))
        self.parent_dir = ".analysis/results/native/" + self.parent["sha256"]
        self.child_dir = ".analysis/results/native/" + self.child["sha256"]
        self.parent_failure = {"stage": "2410:hexrays", "error": "fixture x64 decode of child x86 bytes failed"}
        self.other_failure = {"stage": "1010:hexrays", "error": "independent parent failure"}
        self.child_failure = {"stage": "1020:hexrays", "error": "independent child failure"}
        self.parent_capture = self.capture(self.parent, self.parent_base,
            [self.function(self.parent_dir, self.parent_base, 0x1000, [("c3", "retn")], pseudo=True, comment=True),
             self.function(self.parent_dir, self.parent_base, 0x1010, [("c3", "retn")], pseudo=False),
             self.function(self.parent_dir, self.parent_base, self.parent_rva,
                           [("4090", "nop with REX"), ("c3", "retn")], pseudo=False)],
            [self.parent_failure, self.other_failure])
        self.child_capture = self.capture(self.child, self.child_base,
            [self.function(self.child_dir, self.child_base, self.child_rva,
                [("40", "inc eax"), ("90", "nop"), ("c3", "retn")], pseudo=True),
             self.function(self.child_dir, self.child_base, 0x1020, [("c3", "retn")], pseudo=False)],
            [self.child_failure])
        self.review = {"schema_version": 1, "review_kind": "embedded_pe_function_attribution",
            "source_input": self.parent,
            "base_capture": self.write(self.parent_dir + "/capture.json", self.parent_capture),
            "child_input": self.child,
            "child_capture": self.write(self.child_dir + "/capture.json", self.child_capture),
            "extraction_receipt": {},
            "mappings": [{"parent_rva": hex(self.parent_rva), "child_rva": hex(self.child_rva),
                          "reason": "Complete child x86 function resides in parent data with different decode boundaries"}],
            "target_execution": False, "raw_artifacts_modified": False}
        self.range_review = {"schema_version": 1, "source_input": self.parent,
            "ranges": [{"name": "driver.sys", "offset": self.embedded_offset, "bytes": self.child["bytes"],
                "sha256": self.child["sha256"], "machine": "0x14c",
                "evidence": [self.identity(self.parent_dir + "/functions/1000.json")],
                "basis": "Synthetic complete owning callable supplies exact driver pointer and length"}]}
        self.publish_extraction()

    def write(self, name, value):
        raw = value if isinstance(value, bytes) else (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode()
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return self.identity(name)

    def identity(self, name):
        raw = (self.root / name).read_bytes()
        return {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}

    def read(self, name):
        return json.loads((self.root / name).read_bytes())

    def function(self, directory, base, rva, instructions, *, pseudo, comment=False):
        code = bytes.fromhex("".join(row[0] for row in instructions))
        address = base + rva
        body = {"address": hex(address), "rva": hex(rva), "end": hex(address + len(code)),
                "name": "fixture_" + format(rva, "x"), "chunks": [{"start": hex(address),
                    "end": hex(address + len(code)), "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}],
                "assembly": [], "xrefs_to_entry": [], "pseudocode": None}
        for raw, text in instructions:
            body["assembly"].append({"address": hex(address), "rva": hex(address - base), "bytes": raw, "text": text})
            address += len(bytes.fromhex(raw))
        if pseudo:
            texts = (["// Hidden exception states"] if comment else []) + ["int fixture(void)", "{", "  return 0;", "}"]
            body["pseudocode"] = {"entry": hex(base + rva), "lines": [
                {"line": index, "text": text} for index, text in enumerate(texts)],
                "locals": [{"historical_coordinate": "<Swig Object of type sval_t>"}]}
        record = self.write(directory + "/functions/" + format(rva, "x") + ".json", body)
        return {"path": "functions/" + format(rva, "x") + ".json", "sha256": record["sha256"],
                "rva": hex(rva), "address": hex(base + rva), "name": body["name"],
                "assembly_captured": True, "pseudocode_captured": pseudo, "new_failures": 0 if pseudo else 1}

    def capture(self, original, base, functions, failures):
        directory = ".analysis/results/native/" + original["sha256"]
        outputs = []
        for name in ("all-functions.asm", "all-functions.c", "entrypoints.json", "imports.json",
                     "local_types.json", "names.json", "segments.json", "strings.json", "structures.json"):
            record = self.write(directory + "/" + name, b"[]\n" if name.endswith(".json") else b"fixture capture\n")
            outputs.append({**record, "path": name})
        return {"input": original["path"], "input_sha256": original["sha256"], "input_bytes": original["bytes"],
                "image_base": hex(base), "capture_complete": True, "functions_complete": True,
                "functions_captured": len(functions), "settled_functions_total": len(functions),
                "aggregate_functions_written": len(functions), "functions": functions, "outputs": outputs,
                "failures": failures, "target_execution": False, "real_device_operations": False}

    def publish_extraction(self):
        group = {"ranges/owner": self.range_review}
        source = self.write(".analysis/results/range-review.json", group)
        ranges = inspect_embedded_pe_ranges(bytes(self.parent_raw), self.range_review["ranges"])
        row = ranges[0]
        self.extracted_child = {**self.child, "kind": "static_embedded_native_input",
            "container": self.parent["path"], "container_sha256": self.parent["sha256"],
            "container_offset": self.embedded_offset, "container_aliases": [{**self.parent, "byte_identity_verified": True}],
            "byte_range_verified": True, "extraction": {"kind": "reviewed_embedded_pe_range",
                "pe": row["pe"], "layout": row["layout"], "review": row["review"],
                "review_source": source, "review_pointer": "/ranges~1owner"}}
        self.extraction = {"inputs": [self.extracted_child], "target_execution": False}
        self.publish_index()
        self.review["extraction_receipt"] = self.write(".analysis/results/native-packages/embedded-inputs.json", self.extraction)

    def publish_index(self):
        self.write(".analysis/inputs.json", {"inputs": [self.parent, self.extracted_child]})

    def refresh_capture(self, *, child=False):
        key = "child_capture" if child else "base_capture"
        self.review[key] = self.write(self.review[key]["path"], self.child_capture if child else self.parent_capture)

    def edit_function(self, rva, edit, *, child=False):
        directory = self.child_dir if child else self.parent_dir
        path = directory + "/functions/" + format(rva, "x") + ".json"
        body = self.read(path)
        edit(body)
        reference = self.write(path, body)
        capture = self.child_capture if child else self.parent_capture
        next(row for row in capture["functions"] if row["rva"] == hex(rva))["sha256"] = reference["sha256"]
        self.refresh_capture(child=child)
        if not child and rva == 0x1000:
            self.range_review["ranges"][0]["evidence"] = [reference]
            self.publish_extraction()

    def resolution(self):
        return {"base_capture": self.review["base_capture"], "remaining_failures": list(self.parent_capture["failures"]),
                "resolved_failures": [], "verification_failures": [], "complete_functions": {}}

    def publish_formal(self, *, pointer=False):
        raw = (json.dumps({"review": self.review} if pointer else self.review, indent=2) + "\n").encode()
        source = analysis.retain_embedded_review_source(raw)
        proof = embedded.verify_function_attribution(analysis, self.review)
        proof["review_source"] = {**source, "pointer": "/review" if pointer else None}
        reference = self.write(self.parent_dir + "/embedded-function-attribution.json", proof)
        return proof, reference


class NativeEmbeddedTests(unittest.TestCase):
    def fixture(self, **options):
        temp = tempfile.TemporaryDirectory(prefix="native-embedded-test-")
        self.addCleanup(temp.cleanup)
        root = Path(temp.name).resolve()
        changed = patch.object(analysis, "ROOT", root)
        changed.start()
        self.addCleanup(changed.stop)
        return AttributionFixture(root, **options)

    def verify(self, fixture):
        return embedded.verify_function_attribution(analysis, fixture.review)

    def test_full_mapping_accepts_different_decoder_boundaries_and_preserves_child_failure(self):
        f = self.fixture()
        result = self.verify(f)
        mapping = result["mappings"][0]
        self.assertEqual((mapping["parent_instruction_count"], mapping["child_instruction_count"]), (2, 3))
        self.assertEqual(mapping["instruction_mapping"][0]["parent_interpretation"],
                         mapping["instruction_mapping"][1]["parent_interpretation"])
        self.assertEqual(result["resolved_parent_failures"], [f.parent_failure])
        self.assertEqual(result["child_remaining_failures"], [f.child_failure])
        self.assertFalse(mapping["child_typed_coordinates_verified"])
        self.assertFalse(result["semantics_complete"])
        self.assertEqual(result["runtime_acceptance"], "unverified")
        self.assertFalse(result["target_execution"])

    def test_explicit_schema_and_exact_identity_are_required(self):
        f = self.fixture()
        for name, value in (("schema_version", 2), ("target_execution", True), ("raw_artifacts_modified", True),
                            ("review_kind", "other"), ("mappings", []), ("extra", "field")):
            with self.subTest(name=name):
                review = dict(f.review, **{name: value})
                with self.assertRaises(ValueError):
                    embedded.verify_function_attribution(analysis, review)
        for identity in (dict(f.parent, sha256="0" * 64), dict(f.parent, bytes=True),
                         dict(f.parent, path=".analysis/inputs/../parent.dll"), dict(f.parent, extra=1)):
            with self.subTest(identity=identity), self.assertRaises(ValueError):
                embedded.verify_function_attribution(analysis, dict(f.review, source_input=identity))

    def test_wrong_parent_permission_rejects_an_executable_parent_function(self):
        f = self.fixture(parent_executable=True)
        with self.assertRaisesRegex(ValueError, "execution permission"):
            self.verify(f)

    def test_non_executable_embedded_child_cannot_supply_code(self):
        f = self.fixture()
        struct.pack_into("<I", f.child_raw, 0x98 + 224 + 36, 0x40000040)
        changed = bytes(f.child_raw)
        f.write(f.child["path"], changed)
        f.review["child_input"] = f.identity(f.child["path"])
        with self.assertRaises(ValueError):
            self.verify(f)
        image = embedded._Image(changed)
        with self.assertRaisesRegex(ValueError, "execution permission"):
            image.at(0x1010, 3, executable=True)

    def test_complete_chunks_cannot_be_truncated_even_when_assembly_and_hashes_are_rewritten(self):
        f = self.fixture()
        def truncate(body):
            body["end"] = body["chunks"][0]["end"] = hex(f.child_base + f.child_rva + 2)
            body["chunks"][0]["raw_hex"] = "4090"
            body["chunks"][0]["sha256"] = hashlib.sha256(bytes.fromhex("4090")).hexdigest()
            body["assembly"].pop()
        f.edit_function(f.child_rva, truncate, child=True)
        with self.assertRaisesRegex(ValueError, "complete parent and child byte mapping"):
            self.verify(f)

    def test_original_instruction_bytes_are_checked_after_a_forged_artifact_is_rehashed(self):
        f = self.fixture()
        f.edit_function(f.child_rva, lambda body: body["assembly"][0].update(bytes="41"), child=True)
        with self.assertRaisesRegex(ValueError, "instruction differs"):
            self.verify(f)

    def test_instruction_gap_and_overlap_are_both_rejected(self):
        f = self.fixture()
        path = f.child_dir + "/functions/1010.json"
        original = f.read(path)
        for change in (lambda body: body["assembly"].pop(1),
                       lambda body: body["assembly"].append(copy.deepcopy(body["assembly"][0]))):
            with self.subTest(change=change):
                f.write(path, original)
                f.edit_function(f.child_rva, change, child=True)
                with self.assertRaisesRegex(ValueError, "coverage.*gap|coverage overlaps"):
                    self.verify(f)

    def test_pseudocode_lines_declaration_and_capture_flags_are_required(self):
        f = self.fixture()
        path = f.child_dir + "/functions/1010.json"
        original = f.read(path)
        for change in (lambda body: body["pseudocode"]["lines"][1].update(line=9),
                       lambda body: body["pseudocode"]["lines"].pop(),
                       lambda body: body["pseudocode"]["lines"][1].update(text="partial"),
                       lambda body: body["pseudocode"].update(entry="0x11011")):
            with self.subTest(change=change):
                f.write(path, original)
                f.edit_function(f.child_rva, change, child=True)
                with self.assertRaises(ValueError):
                    self.verify(f)
        f.write(path, original)
        f.edit_function(f.child_rva, lambda body: None, child=True)
        f.child_capture["functions"][0]["pseudocode_captured"] = False
        f.refresh_capture(child=True)
        with self.assertRaisesRegex(ValueError, "complete pseudocode"):
            self.verify(f)

    def test_all_child_outputs_and_independent_function_artifacts_must_remain_byte_identical(self):
        f = self.fixture()
        for name in ("all-functions.c", "functions/1020.json"):
            with self.subTest(name=name):
                path = f.root / f.child_dir / name
                original = path.read_bytes()
                path.write_bytes(original + b" ")
                with self.assertRaisesRegex(ValueError, "artifact bytes changed|function inventory changed"):
                    self.verify(f)
                path.write_bytes(original)
        f.child_capture["outputs"].pop()
        f.refresh_capture(child=True)
        with self.assertRaisesRegex(ValueError, "missing complete retained output inventories"):
            self.verify(f)

    def test_parent_executable_call_reference_prevents_attribution(self):
        f = self.fixture()
        f.edit_function(f.parent_rva, lambda body: body["xrefs_to_entry"].append(
            {"from": hex(f.parent_base + 0x1000), "code": True}))
        with self.assertRaisesRegex(ValueError, "executable-parent code reference"):
            self.verify(f)

    def test_parent_data_code_reference_is_retained_without_claiming_it_is_callable(self):
        f = self.fixture()
        f.edit_function(f.parent_rva, lambda body: body["xrefs_to_entry"].append(
            {"from": hex(f.parent_base + f.parent_rva), "code": True}))
        reference = self.verify(f)["mappings"][0]["parent_entry_references"][0]
        self.assertTrue(reference["source_inside_embedded_child"])
        self.assertFalse(reference["source_section"]["executable"])

    def test_stale_or_ambiguous_input_index_and_extraction_metadata_fail(self):
        f = self.fixture()
        f.extracted_child["container_offset"] += 1
        f.publish_index()
        with self.assertRaisesRegex(ValueError, "extraction receipt differs"):
            self.verify(f)
        f.review["extraction_receipt"] = f.write(".analysis/results/native-packages/embedded-inputs.json", f.extraction)
        with self.assertRaisesRegex(ValueError, "exact reviewed complete parent byte range"):
            self.verify(f)
        f.extracted_child["container_offset"] -= 1
        f.extracted_child["extraction"]["layout"]["complete_file_bytes"] -= 1
        f.publish_index()
        f.review["extraction_receipt"] = f.write(".analysis/results/native-packages/embedded-inputs.json", f.extraction)
        with self.assertRaisesRegex(ValueError, "extraction metadata differs"):
            self.verify(f)

    def test_reviewed_full_extent_and_original_owner_are_reverified(self):
        f = self.fixture()
        path = f.root / f.extracted_child["extraction"]["review_source"]["path"]
        path.write_bytes(path.read_bytes() + b" ")
        with self.assertRaisesRegex(ValueError, "artifact bytes changed"):
            self.verify(f)
        f.publish_extraction()
        f.edit_function(0x1000, lambda body: body["assembly"][0].update(bytes="90"))
        with self.assertRaisesRegex(ValueError, "instruction differs"):
            self.verify(f)

    def test_missing_or_wrong_json_pointer_is_rejected(self):
        f = self.fixture()
        for pointer in ("", "/missing", "/ranges~2owner"):
            with self.subTest(pointer=pointer):
                f.extracted_child["extraction"]["review_pointer"] = pointer
                f.publish_index()
                f.review["extraction_receipt"] = f.write(".analysis/results/native-packages/embedded-inputs.json", f.extraction)
                with self.assertRaises(ValueError):
                    self.verify(f)

    def test_failed_child_selection_and_duplicate_or_wrong_mappings_cannot_resolve_parent(self):
        f = self.fixture()
        mapping = f.review["mappings"][0]
        for mappings in ([mapping, mapping], [dict(mapping, child_rva="0x1020")],
                         [dict(mapping, parent_rva="0x1000")], [dict(mapping, child_rva="0x1001")]):
            with self.subTest(mappings=mappings), self.assertRaises(ValueError):
                embedded.verify_function_attribution(analysis, dict(f.review, mappings=mappings))

    def test_formal_resolution_moves_only_exact_parent_failures_and_preserves_base_and_child(self):
        f = self.fixture()
        proof, reference = f.publish_formal(pointer=True)
        originals = {name: (f.root / name).read_bytes() for name in
                     (f.review["base_capture"]["path"], f.review["child_capture"]["path"], f.parent["path"], f.child["path"])}
        result = f.resolution()
        embedded.apply_resolution(analysis, f.root / f.parent_dir, result, f.parent_capture)
        self.assertFalse(result["verification_failures"])
        self.assertEqual(result["remaining_failures"], [f.other_failure])
        self.assertEqual(result["resolved_failures"][0]["original_failure"], f.parent_failure)
        self.assertEqual(result["resolved_failures"][0]["resolution_kind"], "corrected_embedded_pe_function_attribution")
        self.assertEqual(result["resolved_failures"][0]["proof"], {"receipt": reference, "mapping_index": 0})
        self.assertEqual(result["attribution_resolution"]["child_remaining_failures"], [f.child_failure])
        self.assertEqual(result["complete_functions"], {})
        self.assertFalse(result["attribution_resolution"]["parent_decompilation_recovered"])
        for name, raw in originals.items():
            self.assertEqual((f.root / name).read_bytes(), raw)

    def test_absent_formal_result_leaves_resolution_unchanged(self):
        f = self.fixture()
        result = f.resolution()
        expected = copy.deepcopy(result)
        embedded.apply_resolution(analysis, f.root / f.parent_dir, result, f.parent_capture)
        self.assertEqual(result, expected)

    def test_every_formal_audit_field_is_checked_against_fresh_proof(self):
        f = self.fixture()
        proof, _ = f.publish_formal()
        for key, value in (("child_remaining_failures", []), ("resolved_parent_failures", []),
                           ("embedded_file_offset", 0), ("parent_decompilation_recovered", True),
                           ("mappings", []), ("status", "passed"), ("extra", True)):
            with self.subTest(key=key):
                f.write(f.parent_dir + "/embedded-function-attribution.json", dict(proof, **{key: value}))
                result = f.resolution()
                embedded.apply_resolution(analysis, f.root / f.parent_dir, result, f.parent_capture)
                self.assertEqual(result["remaining_failures"], f.parent_capture["failures"])
                self.assertFalse(result["resolved_failures"])
                self.assertEqual(len(result["verification_failures"]), 1)

    def test_stale_review_or_evidence_and_mismatched_base_capture_do_not_partially_resolve(self):
        f = self.fixture()
        proof, _ = f.publish_formal()
        result = f.resolution()
        result["base_capture"] = dict(result["base_capture"], sha256="0" * 64)
        embedded.apply_resolution(analysis, f.root / f.parent_dir, result, f.parent_capture)
        self.assertTrue(result["verification_failures"])
        self.assertFalse(result["resolved_failures"])
        source = f.root / proof["review_source"]["path"]
        source.write_bytes(source.read_bytes() + b" ")
        result = f.resolution()
        embedded.apply_resolution(analysis, f.root / f.parent_dir, result, f.parent_capture)
        self.assertTrue(result["verification_failures"])
        self.assertFalse(result["resolved_failures"])

    def test_cli_retains_original_review_and_uses_parent_specific_output(self):
        f = self.fixture()
        review_path = ".analysis/results/review.json"
        original = f.write(review_path, {"specific/owner": f.review})
        args = SimpleNamespace(review=review_path + "#/specific~1owner", output=None)
        with redirect_stdout(io.StringIO()):
            proof = embedded.audit_function_attribution(args)
            repeated = embedded.audit_function_attribution(args)
        self.assertEqual(proof, repeated)
        self.assertEqual(proof["review_source"]["pointer"], "/specific~1owner")
        self.assertEqual(proof["review_source"]["sha256"], original["sha256"])
        self.assertEqual((f.root / proof["review_source"]["path"]).read_bytes(), (f.root / review_path).read_bytes())
        self.assertTrue((f.root / f.parent_dir / "embedded-function-attribution.json").is_file())
        self.assertFalse((f.root / ".analysis/results/native-packages/embedded-function-attribution.json").exists())

    def refresh_fixture(self, *, legacy=False):
        f = self.fixture()
        proof, original = f.publish_formal(pointer=True)
        if legacy:
            proof.pop("input_index_registration")
            original = f.write(f.parent_dir + "/embedded-function-attribution.json", proof)
        source = proof["review_source"]
        args = SimpleNamespace(review=source["path"] + "#" + source["pointer"], output=None,
                               previous_audit_sha256=original["sha256"])
        index = json.loads((f.root / ".analysis/inputs.json").read_bytes())
        index["inputs"].append({"path": ".analysis/inputs/another-independent-evidence.txt",
                                "bytes": 9, "sha256": "7" * 64})
        f.write(".analysis/inputs.json", index)
        return f, proof, original, args

    def test_explicit_index_refresh_retains_legacy_audit_and_first_complete_rows_with_honest_boundary(self):
        f, before, old, args = self.refresh_fixture(legacy=True)
        path = f.root / old["path"]
        old_raw = path.read_bytes()
        with redirect_stdout(io.StringIO()):
            after = embedded.audit_function_attribution(args)
        self.assertNotEqual(f.identity(old["path"]), old)
        self.assertEqual((f.root / after["previous_audit_snapshot"]["path"]).read_bytes(), old_raw)
        rows = json.loads((f.root / ".analysis/inputs.json").read_bytes())["inputs"]
        for role in ("source_input", "child_input"):
            self.assertEqual(after["input_index_registration"][role],
                             next(row for row in rows if row["path"] == f.review[role]["path"]))
        self.assertFalse(after["index_refresh"]["historical_full_index_replay"])
        self.assertFalse(after["index_refresh"]["previous_complete_input_rows_retained"])
        self.assertEqual(after["index_refresh"]["unrecorded_historical_parent_row_metadata_equality"], "unverified")
        self.assertEqual(after["resolved_parent_failures"], before["resolved_parent_failures"])
        self.assertEqual(after["mappings"], before["mappings"])
        result = f.resolution()
        embedded.apply_resolution(analysis, f.root / f.parent_dir, result, f.parent_capture)
        self.assertFalse(result["verification_failures"])
        self.assertEqual(result["remaining_failures"], [f.other_failure])
        self.assertFalse(result["attribution_resolution"]["parent_decompilation_recovered"])
        args.previous_audit_sha256 = f.identity(old["path"])["sha256"]
        with redirect_stdout(io.StringIO()):
            self.assertEqual(embedded.audit_function_attribution(args), after)

    def test_index_refresh_requires_explicit_exact_previous_sha_and_stable_nonindex_evidence(self):
        f, proof, old, args = self.refresh_fixture()
        path = f.root / old["path"]
        original = path.read_bytes()
        for sha in (None, "f" * 64, "wrong"):
            args.previous_audit_sha256 = sha
            with self.subTest(sha=sha), self.assertRaises(ValueError):
                embedded.audit_function_attribution(args)
            self.assertEqual(path.read_bytes(), original)
        args.previous_audit_sha256 = old["sha256"]
        for key, value in (("mappings", []), ("resolved_parent_failures", []), ("child_remaining_failures", []),
                           ("parent_decompilation_recovered", True), ("extra", "unreviewed")):
            changed = dict(proof, **{key: value})
            args.previous_audit_sha256 = f.write(old["path"], changed)["sha256"]
            raw = path.read_bytes()
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "changed function evidence"):
                embedded.audit_function_attribution(args)
            self.assertEqual(path.read_bytes(), raw)
        path.write_bytes(original)

    def test_complete_retained_parent_and_child_registration_changes_are_rejected(self):
        f, proof, old, args = self.refresh_fixture()
        index_path = f.root / ".analysis/inputs.json"
        original = index_path.read_bytes()
        audit_raw = (f.root / old["path"]).read_bytes()
        for role in ("source_input", "child_input"):
            for field in ("extra_metadata", "sha256"):
                value = json.loads(original)
                row = next(row for row in value["inputs"] if row["path"] == f.review[role]["path"])
                row[field] = "unreviewed" if field == "extra_metadata" else "f" * 64
                f.write(".analysis/inputs.json", value)
                with self.subTest(role=role, field=field), self.assertRaises(ValueError):
                    embedded.audit_function_attribution(args)
                self.assertEqual((f.root / old["path"]).read_bytes(), audit_raw)
        index_path.write_bytes(original)

    def test_index_refresh_cannot_rebind_changed_original_capture_or_extraction_artifact(self):
        f, proof, old, args = self.refresh_fixture(legacy=True)
        audit_raw = (f.root / old["path"]).read_bytes()
        for reference in (f.parent, f.child, f.review["base_capture"], f.review["child_capture"], f.review["extraction_receipt"]):
            path = f.root / reference["path"]
            raw = path.read_bytes()
            path.write_bytes(raw + b" ")
            with self.subTest(path=path), self.assertRaises(ValueError):
                embedded.audit_function_attribution(args)
            self.assertEqual((f.root / old["path"]).read_bytes(), audit_raw)
            path.write_bytes(raw)

    def test_index_refresh_history_rejects_changed_previous_bytes_flags_or_missing_current_rows(self):
        f, before, old, args = self.refresh_fixture(legacy=True)
        with redirect_stdout(io.StringIO()):
            proof = embedded.audit_function_attribution(args)
        audit = f.root / old["path"]
        current_raw = audit.read_bytes()
        previous = f.root / proof["previous_audit_snapshot"]["path"]
        old_raw = previous.read_bytes()
        for issue in ("prior_bytes", "boundary", "rows", "snapshot", "new_mapping"):
            value = copy.deepcopy(proof)
            if issue == "prior_bytes": previous.write_bytes(old_raw + b" ")
            if issue == "boundary": value["index_refresh"]["historical_full_index_replay"] = True
            if issue == "rows": value.pop("input_index_registration")
            if issue == "snapshot": value["previous_audit_snapshot"]["sha256"] = "f" * 64
            if issue == "new_mapping": value["mappings"] = []
            f.write(old["path"], value)
            result = f.resolution()
            embedded.apply_resolution(analysis, f.root / f.parent_dir, result, f.parent_capture)
            with self.subTest(issue=issue):
                self.assertTrue(result["verification_failures"])
                self.assertFalse(result["resolved_failures"])
            previous.write_bytes(old_raw)
        audit.write_bytes(current_raw)

    def test_index_refresh_failed_publication_releases_temporary_and_keeps_previous_audit(self):
        f, proof, old, args = self.refresh_fixture(legacy=True)
        audit = f.root / old["path"]
        raw = audit.read_bytes()
        with patch.object(embedded.os, "replace", side_effect=OSError("injected atomic publication failure")):
            with self.assertRaisesRegex(OSError, "injected"):
                embedded.audit_function_attribution(args)
        self.assertEqual(audit.read_bytes(), raw)
        self.assertFalse(list(audit.parent.glob(".embedded-attribution-*.tmp")))

    def test_retained_attribution_reference_requires_actual_verified_owner_chain_membership(self):
        f, proof, old, args = self.refresh_fixture(legacy=True)
        source = f.parent["sha256"]
        self.assertFalse(embedded.retained_attribution_reference_valid(analysis, old, source))
        with redirect_stdout(io.StringIO()):
            current = embedded.audit_function_attribution(args)
        self.assertTrue(embedded.retained_attribution_reference_valid(analysis, old, source))
        self.assertTrue(embedded.retained_attribution_reference_valid(analysis, f.identity(old["path"]), source))
        snapshot = f.root / current["previous_audit_snapshot"]["path"]
        raw = snapshot.read_bytes()
        snapshot.write_bytes(raw + b" ")
        self.assertFalse(embedded.retained_attribution_reference_valid(analysis, old, source))
        snapshot.write_bytes(raw)
        for bad in (dict(old, bytes=old["bytes"] + 1), dict(old, sha256="f" * 64),
                    dict(old, path=current["previous_audit_snapshot"]["path"]), dict(old, path=".analysis/inputs/original.dll")):
            self.assertFalse(embedded.retained_attribution_reference_valid(analysis, bad, source))
        self.assertFalse(embedded.retained_attribution_reference_valid(analysis, old, "f" * 64))
        audit = f.root / old["path"]
        current_raw = audit.read_bytes()
        orphan = dict(proof, untouched=False)
        orphan_raw = (json.dumps(orphan, ensure_ascii=False, indent=2) + "\n").encode()
        orphan_id = analysis.retain_embedded_review_source(orphan_raw)
        self.assertFalse(embedded.retained_attribution_reference_valid(analysis,
            {**orphan_id, "path": old["path"]}, source))
        audit.write_bytes(raw)
        self.assertFalse(embedded.retained_attribution_reference_valid(analysis, old, source))
        audit.write_bytes(current_raw)

    def test_retained_attribution_reference_does_not_accept_changed_current_proof_or_source(self):
        f, proof, old, args = self.refresh_fixture(legacy=True)
        with redirect_stdout(io.StringIO()):
            current = embedded.audit_function_attribution(args)
        audit = f.root / old["path"]
        current_raw = audit.read_bytes()
        for issue in ("mapping", "scope", "input_row", "current_index"):
            value = copy.deepcopy(current)
            index = f.root / ".analysis/inputs.json"
            index_raw = index.read_bytes()
            if issue == "mapping": value["mappings"] = []
            if issue == "scope": value["source_input"]["sha256"] = "f" * 64
            if issue == "input_row": value["input_index_registration"]["source_input"]["unverified"] = True
            if issue == "current_index": index.write_bytes(index_raw + b" ")
            f.write(old["path"], value)
            self.assertFalse(embedded.retained_attribution_reference_valid(analysis, old, f.parent["sha256"]))
            index.write_bytes(index_raw)
        audit.write_bytes(current_raw)


if __name__ == "__main__":
    unittest.main()
