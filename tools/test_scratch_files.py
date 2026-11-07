"""Published JSON intermediates use the same reference and exclusive-delete guards."""
import json
import os
import io
from contextlib import ExitStack, nullcontext, redirect_stdout
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import code_identity as ci
import scratch_files
import workspace


class CompletedScratchFilesTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.patch = patch.object(workspace, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.file = self.root / ".work/candidates.json"
        self.file.parent.mkdir()
        self.file.write_text('{"derived":true}', encoding="utf-8")
        self.owner = ".analysis/results/review.json"
        self.report = {"source_slice_complete": True, "snapshot_stable": True,
                       "target_execution": False, "failures": [], "changed_during_query": [],
                       "temporary_files_created": [".work/candidates.json"]}
        self.write(self.owner, {"done": self.report})
        self.ref = {"receipt": self.owner, "pointer": "/done", "canonical_sha256": ci.digest(self.report)}
        self.manifest = ".analysis/inputs/completed-scratch.json"
        self.item = {"path": ".work/candidates.json", "bytes": self.file.stat().st_size,
                     "sha256": workspace.digest(self.file), "completion": self.ref,
                     "historical_references": [{"section": self.ref, "fields": ["temporary_files_created"]}],
                     "release_reason": "Derived input fully published; creator has no follow-up dependency"}
        self.publish()

    def write(self, name, data):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data), encoding="utf-8")

    def publish(self):
        self.write(self.manifest, {"schema_version": 1, "files": [self.item]})

    def row(self):
        return scratch_files.candidates(workspace, [self.manifest])[0]

    def plan_environment(self):
        stack = ExitStack()
        stack.enter_context(patch.object(workspace, "_clean_candidates", side_effect=lambda *args, **kwargs: ([], [])))
        stack.enter_context(patch.object(workspace, "_verification_candidates", return_value=([], [])))
        stack.enter_context(patch.object(workspace, "_clean_process_guard", return_value=({}, [])))
        stack.enter_context(patch.object(workspace, "_clean_locks", side_effect=nullcontext))
        stack.enter_context(redirect_stdout(io.StringIO()))
        return stack

    def test_completed_file_preserves_parent_and_other_dependencies(self):
        row = self.row()
        view = scratch_files.reference_view({"done": self.report, "future_input": ".work/candidates.json"}, self.owner, row)
        self.assertNotIn("temporary_files_created", view["done"])
        self.assertEqual(view["future_input"], ".work/candidates.json")
        self.assertEqual(workspace._clean_tree(row["path"])["directories"], [])
        self.assertEqual(scratch_files.reference_view({}, self.manifest, row), {})
        self.assertEqual(scratch_files.reference_view({"self": row["path"]}, row["path"], row), {})

    def test_changed_bytes_changed_proof_and_unsafe_scope_fail(self):
        self.file.write_text('{"different":true}', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "bytes changed"):
            self.row()
        self.file.write_text('{"derived":true}', encoding="utf-8")
        self.write(self.owner, {"done": {**self.report, "failures": ["not finished"]}})
        with self.assertRaisesRegex(ValueError, "canonical"):
            self.row()
        self.write(self.owner, {"done": self.report})
        self.item["historical_references"][0]["fields"] = ["source_database"]
        self.publish()
        with self.assertRaisesRegex(ValueError, "temporary-creation"):
            self.row()

    def test_unfinished_snapshot_cannot_authorize_deletion(self):
        self.report["snapshot_stable"] = False
        self.write(self.owner, {"done": self.report})
        self.ref["canonical_sha256"] = ci.digest(self.report)
        self.publish()
        with self.assertRaisesRegex(ValueError, "stable source review"):
            self.row()

    def test_analysis_proofs_and_targets_reject_escape_or_link(self):
        with self.assertRaises(ValueError):
            scratch_files.candidates(workspace, [".analysis/inputs/../../outside.json"])
        original = workspace._scratch_link
        with patch.object(workspace, "_scratch_link", side_effect=lambda p: p == self.root / self.manifest or original(p)):
            with self.assertRaisesRegex(ValueError, "link"):
                self.row()
        self.item["path"] = ".work/../outside.json"
        self.publish()
        with self.assertRaises(ValueError):
            self.row()

    def test_full_plan_keeps_new_dependency_and_rechecks_changed_input(self):
        with self.plan_environment():
            self.write(".analysis/results/future.json", {"source_database": ".work/candidates.json"})
            plan = workspace.clean(dry_run=True, completed_file_manifests=[self.manifest],
                                   plan=".analysis/results/scratch-cleanup-held-plan.json")
            self.assertEqual(plan["candidates"], [])
            self.assertTrue(any(row["path"] == ".work/candidates.json" for row in plan["retained"]))
            (self.root / ".analysis/results/future.json").unlink()
            name = ".analysis/results/scratch-cleanup-ready-plan.json"
            plan = workspace.clean(dry_run=True, completed_file_manifests=[self.manifest], plan=name)
            self.assertEqual(len(plan["candidates"]), 1)
            self.file.write_text("changed after plan", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "bytes changed"):
                workspace.clean(apply=name, plan_sha256=workspace.digest(self.root / name))
            self.assertTrue(self.file.exists())

    @unittest.skipUnless(os.name == "nt", "Windows exclusive file disposition")
    def test_full_plan_apply_records_actual_file_release(self):
        with self.plan_environment():
            name = ".analysis/results/scratch-cleanup-file-plan.json"
            workspace.clean(dry_run=True, completed_file_manifests=[self.manifest], plan=name)
            result = workspace.clean(apply=name, plan_sha256=workspace.digest(self.root / name))
        self.assertEqual(result["status"], "completed")
        self.assertEqual(result["failures"], [])
        self.assertEqual(result["removed"][0]["files"], 1)
        self.assertFalse(self.file.exists())
        self.assertTrue(self.file.parent.exists())

    @unittest.skipUnless(os.name == "nt", "Windows exclusive file disposition")
    def test_exclusive_deletion_removes_only_selected_file(self):
        row = self.row()
        row["tree"] = workspace._clean_tree(row["path"])
        other = self.file.parent / "active.json"
        other.write_text("active", encoding="utf-8")
        with patch.object(workspace, "_clean_process_guard", return_value=({}, [])):
            result = workspace._clean_delete_group(row)
        self.assertFalse(self.file.exists())
        self.assertTrue(other.is_file())
        self.assertTrue(other.parent.is_dir())
        self.assertGreaterEqual(result["bytes_reclaimed"], 0)
