"""Counterexamples for native byte identity and reference scope."""
import copy
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest
from unittest.mock import patch

import code_identity_native as native
from code_identity_native import function_identity


def fixture(base=0x180000000):
    start, raw = base + 0x1000, bytes.fromhex("90c3")
    body = {"address": hex(start), "rva": "0x1000", "end": hex(start + len(raw)),
            "chunks": [{"start": hex(start), "end": hex(start + len(raw)),
                        "raw_hex": raw.hex(), "sha256": hashlib.sha256(raw).hexdigest()}],
            "assembly": [{"address": hex(start), "rva": "0x1000", "bytes": "90",
                          "xrefs_from": [{"from": hex(start), "to": hex(start + 1), "type": 21, "code": True}]},
                         {"address": hex(start + 1), "rva": "0x1001", "bytes": "c3", "xrefs_from": []}]}
    return body, lambda rva, size: raw[rva - 0x1000:rva - 0x1000 + size]


class NativeIdentityTests(unittest.TestCase):
    def test_capture_inventory_without_explicit_selection_is_not_reviewed(self):
        entry = {"artifact": {"path": "owner", "sha256": "a" * 64},
                 "capture": {"rva": "0x1000"}, "input_chunks_verified": True}
        block = {"original": {}, "capture": {}, "complete_functions": [entry]}
        self.assertEqual(list(native._selected_native(block)), [])
        block["explicit_seed_rvas"] = ["0x1000"]
        block["complete_functions"].append({**entry, "capture": {"rva": "0x1001"}})
        self.assertEqual([r["rva"] for r in native._selected_native(block)], ["0x1000"])
        capture = {"target_execution": False, "failures": [], "changed_during_query": [], "source_slice_complete": True}
        self.assertFalse(native._clean_review(capture))
        self.assertTrue(native._clean_review({**capture, "semantics": {"effect": "reviewed local effect"}}))

    def test_review_propagation_requires_exact_artifact_and_proven_product_binding(self):
        with sqlite3.connect(":memory:") as db:
            native._schema(db)
            db.execute("INSERT INTO native_functions VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)", (
                "source", "0x1000", "name", "0x1002", "[]", "function.json", "function-sha",
                "instructions", "normalized", 2, 2, 0, 1, 0))
            for section, artifact_sha in (("valid", "function-sha"), ("changed", "different-sha")):
                db.execute("INSERT INTO native_review_sections VALUES(?,?,?,?,?,?)", (
                    section, "canonical", "[1,2]", '["page:TAB_CUSTOMIZE"]', "explicit_selection", None))
                db.execute("INSERT INTO native_review_owners VALUES(?,?,?,?,?,?,?,?,?,?,?)", (
                    section, "review", "review-sha", "/native/selected/0", "source", "0x1000",
                    "function.json", artifact_sha, None, "identity_pending", None))
            db.executemany("INSERT INTO native_bindings VALUES(?,?,?,?)", [
                (1, "source", "source", "declared_resource"),
                (2, "source", "source", "reviewed_dependency_basis"),
                (3, "source", "parent", "contained_candidate")])
            db.executemany("INSERT INTO native_review_dependencies VALUES(?,?,?,?,?)", [
                ("valid", 1, "source", "source.dll", 2), ("valid", 2, "source", "source.dll", 2)])
            # A short RET owner in a different declared installer has equal code
            # but no reviewed dependency on the customization capability.
            db.execute("INSERT INTO native_functions VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)", (
                "installer", "0x2000", "name", "0x2002", "[]", "installer.json", "installer-sha",
                "instructions", "normalized", 2, 2, 0, 1, 0))
            db.execute("INSERT INTO native_bindings VALUES(4,'installer','installer','declared_resource')")
            db.execute("INSERT INTO native_bindings VALUES(5,'source','source','reviewed_dependency_basis')")
            db.execute("INSERT INTO native_review_sections VALUES(?,?,?,?,?,?)", (
                "lighting", "canonical", "[5]", '["page:TAB_LIGHTING"]', "explicit_selection", None))
            db.execute("INSERT INTO native_review_dependencies VALUES('lighting',5,'source','source.dll',2)")
            native._bind_selected_owners(db)
            native._match_reviews(db)
            self.assertEqual(db.execute("SELECT section_id,product_id,basis FROM native_review_matches ORDER BY product_id").fetchall(),
                             [("valid", 1, "code_scope_only"), ("valid", 2, "code_scope_only")])
            self.assertEqual(db.execute("SELECT status FROM native_review_owners WHERE section_id='changed'").fetchone()[0],
                             "indexed_owner_unavailable")
            self.assertEqual(db.execute("SELECT product_id,basis FROM native_review_code_candidates ORDER BY product_id").fetchall(),
                             [(4, "code_identity_candidate"), (5, "code_identity_candidate")])

    def test_shared_hash_keeps_valid_declaration_and_excludes_wrong_length_peer(self):
        sys.path.insert(0, str(Path(__file__).resolve().parent / "analyzers/python-static"))
        import ijson  # The maintained streaming parser also operates under a temporary evidence root.
        with tempfile.TemporaryDirectory() as directory, sqlite3.connect(":memory:") as db:
            root = Path(directory)
            (root / ".analysis").mkdir()
            (root / ".analysis/inputs.json").write_text(json.dumps({"inputs": [
                {"path": ".analysis/inputs/parent.exe", "sha256": "a" * 64, "bytes": 3},
                {"path": ".analysis/inputs/child.dll", "sha256": "b" * 64, "bytes": 1,
                 "container": ".analysis/inputs/parent.exe", "container_sha256": "a" * 64},
                {"path": ".analysis/inputs/failed.dll", "sha256": "c" * 64, "bytes": 1,
                 "container": ".analysis/inputs/parent.exe", "container_sha256": "a" * 64,
                 "container_extraction_complete": False}]}), encoding="utf-8")
            with patch.object(native.a, "ROOT", root):
                native._schema(db)
                db.executemany("INSERT INTO native_declarations VALUES(?,?,?,?,?,?,?,?,?)", [
                    (1, "mw", "a" * 64, "manifest", "d" * 64, "driver", "1", "url", 3),
                    (2, "mw", "a" * 64, "manifest", "d" * 64, "driver", "1", "url", 4)])
                native._inputs(db)
            self.assertEqual(db.execute("SELECT product_id,sha256,binding_kind FROM native_bindings ORDER BY sha256").fetchall(),
                             [(1, "a" * 64, "declared_resource"), (1, "b" * 64, "contained_candidate")])
            self.assertEqual(db.execute("SELECT count(*) FROM native_declarations").fetchone()[0], 2)

    def test_current_resource_manifest_is_selected_by_url_for_extensionless_blob(self):
        with tempfile.TemporaryDirectory() as directory, sqlite3.connect(":memory:") as db:
            root = Path(directory)
            original = root / ".analysis/inputs/blobs/manifest-identity"
            original.parent.mkdir(parents=True)
            raw = json.dumps({"resourceManifest": {"resources": [{
                "sha256": "a" * 64, "resourceName": "localDriver", "resourceVersion": "1",
                "url": "https://app-assets.razer.com/driver.dll", "size": 3}]}}).encode()
            original.write_bytes(raw)
            receipt = root / ".analysis/inputs/acquisition.json"
            acquired = json.dumps({"files": [{"url": "https://apps.razer.com/synapse/products/1/mw/manifest.json",
                "path": original.relative_to(root).as_posix(), "sha256": hashlib.sha256(raw).hexdigest(),
                "bytes": len(raw)}]}).encode()
            receipt.write_bytes(acquired)
            db.execute("CREATE TABLE deployments(product_id,layer,acquisition,acquisition_sha,failure)")
            db.execute("INSERT INTO deployments VALUES(?,?,?,?,NULL)",
                       (1, "mw", receipt.relative_to(root).as_posix(), hashlib.sha256(acquired).hexdigest()))
            with patch.object(native.a, "ROOT", root):
                native._schema(db)
                native._declarations(db)
                self.assertEqual(db.execute("SELECT product_id,sha256 FROM native_declarations").fetchall(), [(1, "a" * 64)])
                original.write_bytes(raw + b" ")
                native._declarations(db)
                self.assertEqual(db.execute("SELECT count(*) FROM native_declarations").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT count(*) FROM native_failures").fetchone()[0], 1)

    def test_image_base_and_original_path_do_not_split_pure_instruction_identity(self):
        left, read_left = fixture()
        right, read_right = fixture(0x400000)
        a = function_identity(left, 0x180000000, "a" * 64, "x64", read_left)
        b = function_identity(right, 0x400000, "b" * 64, "x64", read_right)
        self.assertEqual(a["normalized_sha"], b["normalized_sha"])

    def test_external_target_scope_does_not_merge_equal_instruction_bytes(self):
        body, read = fixture()
        body["assembly"][0]["xrefs_from"][0]["to"] = "0x180002000"
        a = function_identity(body, 0x180000000, "a" * 64, "x64", read)
        b = function_identity(body, 0x180000000, "b" * 64, "x64", read)
        self.assertEqual(a["instruction_sha"], b["instruction_sha"])
        self.assertNotEqual(a["normalized_sha"], b["normalized_sha"])

    def test_missing_instruction_is_not_complete_function(self):
        body, read = fixture()
        complete = function_identity(body, 0x180000000, "a" * 64, "x64", read)
        body["assembly"].pop()
        incomplete = function_identity(body, 0x180000000, "a" * 64, "x64", read)
        self.assertEqual(incomplete["uncovered_bytes"], 1)
        self.assertEqual(incomplete["code_bytes"], 2)
        self.assertNotEqual(incomplete["normalized_sha"], complete["normalized_sha"])

    def test_changed_original_bytes_reject_capture(self):
        body, _ = fixture()
        with self.assertRaisesRegex(ValueError, "original bytes"):
            function_identity(body, 0x180000000, "a" * 64, "x64", lambda rva, size: b"\xcc" * size)

    def test_chunk_overlap_and_wrong_end_are_rejected(self):
        body, read = fixture()
        body["chunks"].append(copy.deepcopy(body["chunks"][0]))
        with self.assertRaisesRegex(ValueError, "overlapping"):
            function_identity(body, 0x180000000, "a" * 64, "x64", read)
        body["chunks"].pop()
        body["end"] = "0x180001001"
        with self.assertRaisesRegex(ValueError, "end differs"):
            function_identity(body, 0x180000000, "a" * 64, "x64", read)


if __name__ == "__main__":
    unittest.main()
