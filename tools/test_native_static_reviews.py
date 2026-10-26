"""Check exact static review publication and independent completion dimensions."""
import copy
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import analysis
import native_adjudication as evidence
import native_static_reviews as reviews


class NativeStaticReviewTests(unittest.TestCase):
    def fixture(self, root):
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value), encoding="utf-8")
            return evidence.identity(analysis, path)
        original = root / ".analysis/inputs/native/inert.dll"
        original.parent.mkdir(parents=True)
        original.write_bytes(b"inert original for publication ownership")
        source = evidence.identity(analysis, original)
        result = root / ".analysis/results/native" / source["sha256"]
        function = write(result / "functions/1000.json", {"rva": "0x1000", "type": None})
        failure = {"stage": "1000:hexrays", "error": "original decompiler failure"}
        capture = {"input": source["path"], "input_sha256": source["sha256"], "input_bytes": source["bytes"],
            "functions": [{"rva": "0x1000", "path": "functions/1000.json", "sha256": function["sha256"]}],
            "failures": [failure]}
        base = write(result / "capture.json", capture)
        review = write(root / ".analysis/results/review.json", {"explicit_review": True})
        proof = {"rva": "0x1000", "source_input": source, "base_capture": base,
                 "function": function, "coverage_kind": "independent_test_contract"}
        normalized = {"review_kind": "source-assembly", "review": review, "proof": proof,
            "rva": "0x1000", "source_input": source, "base_capture": base, "function": function,
            "original_hexrays_failures": [failure], "pseudocode_recovered": False,
            "target_execution": False, "rust_independent_implementation_complete": False,
            "ui_backend_wiring_complete": False, "runtime_acceptance": False}
        resolution = {"base_capture": base, "target_execution": False, "complete_functions": {},
            "remaining_failures": [failure], "resolved_failures": [],
            "verification_failures": [{"error": "unrelated failure"}]}
        args = SimpleNamespace(kind="source-assembly", input=source["path"],
                               review=review["path"], review_sha256=review["sha256"])
        return SimpleNamespace(**locals())

    def test_publication_is_explicit_idempotent_and_retains_unrelated_failures(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            f = self.fixture(Path(directory))
            before = (f.result / "capture.json").read_bytes()
            with patch.object(reviews, "_verified", return_value=(f.source, f.base, {"0x1000": f.normalized})):
                first = reviews.publish_binding(analysis, f.args)
                self.assertEqual(first, reviews.publish_binding(analysis, f.args))
                reviews.apply_resolution(analysis, f.result, f.resolution, f.capture)
            self.assertEqual(f.resolution["remaining_failures"], [])
            self.assertEqual(f.resolution["resolved_failures"][0]["original_failure"], f.failure)
            self.assertEqual(f.resolution["verification_failures"], [{"error": "unrelated failure"}])
            self.assertEqual(f.resolution["complete_functions"], {})
            self.assertFalse(f.resolution["complete_static_contracts"]["0x1000"]["pseudocode_recovered"])
            self.assertEqual((f.result / "capture.json").read_bytes(), before)
            self.assertEqual(list(f.result.glob(".static-contract-*")), [])

    def test_changed_sha_input_or_existing_decision_cannot_be_published(self):
        for issue in ("sha", "input", "decision"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                if issue == "sha": f.args.review_sha256 = "0" * 64
                source = {**f.source, "bytes": 1} if issue == "input" else f.source
                if issue == "decision": (f.result / reviews.BINDING).write_bytes(b"retained different decision")
                with patch.object(reviews, "_verified", return_value=(source, f.base, {"0x1000": f.normalized})):
                    with self.assertRaises(ValueError): reviews.publish_binding(analysis, f.args)
                self.assertEqual(list(f.result.glob(".static-contract-*")), [])

    def test_cancelled_or_failed_publication_releases_private_file(self):
        for error in (OSError("cannot publish"), KeyboardInterrupt()):
            with self.subTest(error=type(error).__name__), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                with patch.object(reviews, "_verified", return_value=(f.source, f.base, {"0x1000": f.normalized})), \
                        patch.object(reviews.os, "link", side_effect=error):
                    with self.assertRaises(type(error)): reviews.publish_binding(analysis, f.args)
                self.assertFalse((f.result / reviews.BINDING).exists())
                self.assertEqual(list(f.result.glob(".static-contract-*")), [])

    def test_replacement_requires_previous_bytes_and_preserves_all_decisions(self):
        for issue in (None, "sha", "source", "scope"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                with patch.object(reviews, "_verified", return_value=(f.source, f.base, {"0x1000": f.normalized})):
                    first = reviews.publish_binding(analysis, f.args)
                before = (f.result / reviews.BINDING).read_bytes()
                updated = f.write(f.root / ".analysis/results/updated-review.json", {"updated_review": True})
                f.args.review, f.args.review_sha256 = updated["path"], updated["sha256"]
                f.args.previous_binding_sha256 = "0" * 64 if issue == "sha" else first["binding"]["sha256"]
                source = {**f.source, "bytes": 1} if issue == "source" else f.source
                normalized = {**f.normalized, "review": updated}
                proofs = {"0x2000": {**normalized, "rva": "0x2000"}} if issue == "scope" else {"0x1000": normalized}
                with patch.object(reviews, "_verified", return_value=(source, f.base, proofs)):
                    if issue is None:
                        reviews.publish_binding(analysis, f.args)
                        bound = json.loads((f.result / reviews.BINDING).read_bytes())
                        self.assertEqual(evidence.checked(analysis, bound["previous_binding_snapshot"]), before)
                        reviews.apply_resolution(analysis, f.result, f.resolution, f.capture)
                        self.assertEqual(f.resolution["remaining_failures"], [])
                    else:
                        with self.assertRaises(ValueError): reviews.publish_binding(analysis, f.args)
                        self.assertEqual((f.result / reviews.BINDING).read_bytes(), before)
                self.assertEqual(list(f.result.glob(".static-contract-*")), [])

    def test_absent_or_invalid_binding_preserves_original_outstanding_work(self):
        for issue in (None, "base", "functions", "execution", "failure", "snapshot_list", "snapshot_json"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                with patch.object(reviews, "_verified", return_value=(f.source, f.base, {"0x1000": f.normalized})):
                    if issue is not None:
                        reviews.publish_binding(analysis, f.args)
                        path = f.result / reviews.BINDING
                        binding = json.loads(path.read_bytes())
                        if issue == "base": binding["base_capture"]["sha256"] = "0" * 64
                        elif issue == "functions": binding["function_rvas"] = []
                        elif issue == "execution": binding["target_execution"] = True
                        elif issue == "failure": f.resolution["remaining_failures"][0] = {"stage": "1000:hexrays", "error": "changed"}
                        elif issue == "snapshot_list": binding["previous_binding_snapshot"] = []
                        elif issue == "snapshot_json": binding["previous_binding_snapshot"] = f.write(
                            f.root / ".analysis/results/bad-snapshot.json", [])
                        path.write_text(json.dumps(binding), encoding="utf-8")
                    before = copy.deepcopy(f.resolution["remaining_failures"])
                    reviews.apply_resolution(analysis, f.result, f.resolution, f.capture)
                self.assertEqual(f.resolution["remaining_failures"], before)
                self.assertEqual(f.resolution["resolved_failures"], [])
                self.assertEqual(len(f.resolution["verification_failures"]), 1 if issue is None else 2)

    def test_source_contract_route_rejects_changed_identity_scope_and_execution_claims(self):
        for issue in (None, "source", "base", "function", "rva", "kind", "runtime", "pseudocode", "execution"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                f = self.fixture(Path(directory))
                proof = copy.deepcopy(f.proof)
                verified = {"complete_sources": {f.source["sha256"]: {"0x1000": proof}},
                            "target_execution": False, "pseudocode_recovered": False, "runtime_acceptance": False}
                if issue in ("source", "base", "function"):
                    proof[{"source": "source_input", "base": "base_capture", "function": "function"}[issue]]["sha256"] = "0" * 64
                elif issue == "rva": proof["rva"] = "0x2000"
                elif issue == "kind": proof.pop("coverage_kind")
                elif issue == "runtime": verified["runtime_acceptance"] = True
                elif issue == "pseudocode": verified["pseudocode_recovered"] = True
                elif issue == "execution": verified["target_execution"] = True
                validator = SimpleNamespace(verify_review=lambda *args: verified)
                with patch.dict(sys.modules, {"native_source_correspondence": validator}):
                    if issue is None:
                        source, base, proofs = reviews._verified(analysis, "source-assembly", f.review)
                        self.assertEqual((source, base), (f.source, f.base))
                        self.assertFalse(proofs["0x1000"]["ui_backend_wiring_complete"])
                        self.assertEqual(proofs["0x1000"]["original_hexrays_failures"], [f.failure])
                    else:
                        with self.assertRaises((ValueError, KeyError)):
                            reviews._verified(analysis, "source-assembly", f.review)


if __name__ == "__main__":
    unittest.main()
