"""Preserve retained evidence while changing only its NTFS allocation."""
from contextlib import redirect_stdout
import argparse
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analysis
import analysis_storage as storage


class StorageBoundaryTests(unittest.TestCase):
    def test_dataset_scope_rejects_evidence_manifests_traversal_and_supplements(self):
        prefix = ".analysis/results/native/" + "a" * 64
        for name in ("C:/outside/strings.json", prefix + "/../strings.json", prefix + "/capture.json",
                     prefix + "/supplements/strings.json", prefix + "/unknown.json", None):
            with self.subTest(name=name), self.assertRaises(ValueError):
                storage.native_dataset(name)

    def test_report_cannot_overwrite_evidence_or_follow_links(self):
        for name in (".analysis/results/native/" + "a" * 64 + "/strings.json",
                     ".analysis/inputs/storage-compaction.json", ".analysis/results/report.json"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "reserved"):
                storage.report_path(name)
        with patch.object(Path, "is_symlink", return_value=True):
            with self.assertRaisesRegex(ValueError, "filesystem links"):
                storage.report_path(storage.PLAN)

    def test_dataset_link_is_rejected_before_open(self):
        name = ".analysis/results/native/" + "a" * 64 + "/strings.json"
        with patch.object(Path, "is_symlink", return_value=True):
            with self.assertRaisesRegex(ValueError, "filesystem link"):
                storage.native_dataset(name)

    def test_bounded_workers_reject_serial_and_non_integer_values(self):
        for workers in (1, 0, True, 17, 2.5, "2"):
            with self.subTest(workers=workers), patch.object(storage.os, "name", "nt"), \
                    self.assertRaisesRegex(ValueError, "bounded CPU"):
                storage.run(argparse.Namespace(workers=workers))


@unittest.skipUnless(os.name == "nt", "The production compaction API requires Windows NTFS")
class NtfsEvidenceCompactionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.patcher = patch.object(analysis, "ROOT", self.root)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)
        self.name = ".analysis/results/native/" + "a" * 64 + "/strings.json"
        self.dataset = self.root / self.name
        self.dataset.parent.mkdir(parents=True)
        self.content = b'{"independent":"retained original evidence"}\n' * 65536
        self.dataset.write_bytes(self.content)
        self.capture = self.dataset.parent / "capture.json"
        self.manifest = {"schema_version": 1, "input_sha256": "a" * 64, "capture_complete": True,
            "outputs": [{"path": "strings.json", "bytes": len(self.content),
                         "sha256": hashlib.sha256(self.content).hexdigest()}]}
        self.write_capture()

    def write_capture(self):
        self.capture.write_text(json.dumps(self.manifest), encoding="utf-8")

    def make_report(self):
        row = storage.plan_file(self.name)
        return {"schema_version": 1, "mode": "plan", "finished_at_utc": analysis.stamp(),
                "target_execution": False, "deletes_files": False, "files": [row], "failures": [],
                "summary": {"files": 1, "logical_bytes": len(self.content), "failures": 0, "bytes_reclaimed": 0}}

    def validate(self, report):
        raw = json.dumps(report).encode()
        return storage.reviewed_plan(raw, hashlib.sha256(raw).hexdigest())

    def test_real_compression_preserves_content_identity_and_reduces_physical_allocation(self):
        row = storage.plan_file(self.name)
        result = storage.apply_file(row)
        self.assertEqual(self.dataset.read_bytes(), self.content)
        self.assertEqual(result["sha256"], hashlib.sha256(self.content).hexdigest())
        self.assertTrue(result["logical_bytes_unchanged"])
        self.assertGreater(result["bytes_reclaimed"], 0)
        self.assertEqual(result["allocated_before"] - result["allocated_after"], result["bytes_reclaimed"])
        with storage.open_dataset(self.dataset) as file:
            storage.verify_snapshot(file.info(), row)
            self.assertTrue(file.info()["compressed"])

    def test_unfinished_capture_retains_its_status_without_inventing_output_identity(self):
        self.manifest.pop("capture_complete")
        self.manifest.pop("outputs")
        self.write_capture()
        row = storage.plan_file(self.name)
        self.assertFalse(row["capture"]["capture_complete"])
        self.assertFalse(row["capture_output_identity_available"])
        storage.apply_file(row)
        self.assertEqual(json.loads(self.capture.read_bytes()), self.manifest)
        self.assertEqual(self.dataset.read_bytes(), self.content)

    def test_completed_capture_requires_exact_unique_output_hash(self):
        for outputs in ([], self.manifest["outputs"] * 2,
                        [{**self.manifest["outputs"][0], "sha256": "b" * 64}]):
            with self.subTest(outputs=outputs), self.assertRaises(ValueError):
                self.manifest["outputs"] = outputs
                self.write_capture()
                storage.plan_file(self.name)

    def test_changed_capture_rejects_apply_before_compression(self):
        row = storage.plan_file(self.name)
        self.manifest["new_observation"] = "changed capture identity"
        self.write_capture()
        with self.assertRaisesRegex(ValueError, "capture differs"):
            storage.apply_file(row)
        with storage.open_dataset(self.dataset) as file:
            self.assertFalse(file.info()["compressed"])

    def test_byte_hash_rejects_change_even_when_current_file_metadata_is_supplied(self):
        row = storage.plan_file(self.name)
        self.dataset.write_bytes(b"!" + self.content[1:])
        with storage.open_dataset(self.dataset) as file:
            row.update(file.info())
        with self.assertRaisesRegex(ValueError, "bytes changed"):
            storage.apply_file(row)

    def test_existing_writable_handle_blocks_compaction(self):
        row = storage.plan_file(self.name)
        with storage.open_dataset(self.dataset, writable=True):
            with self.assertRaises(OSError):
                storage.apply_file(row)
        with storage.open_dataset(self.dataset) as file:
            self.assertFalse(file.info()["compressed"])

    def test_hardlinked_dataset_is_rejected(self):
        os.link(self.dataset, self.dataset.parent / "other.json")
        with self.assertRaisesRegex(ValueError, "linked"):
            storage.plan_file(self.name)

    def test_plan_tampering_incompletion_duplicate_and_summary_mismatch_are_rejected(self):
        report = self.make_report()
        raw = json.dumps(report).encode()
        with self.assertRaisesRegex(ValueError, "exact reviewed"):
            storage.reviewed_plan(raw + b" ", hashlib.sha256(raw).hexdigest())
        for modify in (lambda r: r.pop("finished_at_utc"),
                       lambda r: r.update(mode="apply"),
                       lambda r: r.update(failures=[{"error": "incomplete"}]),
                       lambda r: r["summary"].update(files=2),
                       lambda r: r["files"].append(copy.deepcopy(r["files"][0])),
                       lambda r: r["files"][0].update(file_id=[1, 2])):
            changed = copy.deepcopy(report)
            modify(changed)
            with self.subTest(modify=modify), self.assertRaises(ValueError):
                self.validate(changed)

    def test_unified_command_keeps_exact_plan_and_records_separate_apply_receipt(self):
        args = argparse.Namespace(command="compact-analysis", workers=2, minimum_mib=1,
                                  plan=storage.PLAN, apply=False, plan_sha256=None)
        with redirect_stdout(io.StringIO()):
            report = analysis.run(args)
            self.assertEqual(report["summary"]["files"], 1)
            plan = self.root / storage.PLAN
            before = plan.read_bytes()
            args.apply, args.plan_sha256 = True, hashlib.sha256(before).hexdigest()
            result = analysis.run(args)
        self.assertEqual(plan.read_bytes(), before)
        receipt = json.loads(plan.with_name("storage-compaction-apply.json").read_bytes())
        self.assertEqual(receipt["plan"]["sha256"], args.plan_sha256)
        self.assertEqual(result["summary"]["failures"], 0)
        self.assertGreater(result["summary"]["bytes_reclaimed"], 0)
        with self.assertRaisesRegex(ValueError, "already exists"):
            analysis.run(args)

    def test_existing_plan_or_apply_receipt_cannot_be_replaced_by_planning(self):
        args = argparse.Namespace(command="compact-analysis", workers=2, minimum_mib=1,
                                  plan=storage.PLAN, apply=False, plan_sha256=None)
        sentinel = b"retained old receipt bytes"
        for name in (storage.PLAN, ".analysis/results/storage-compaction-apply.json"):
            receipt = self.root / name
            receipt.parent.mkdir(parents=True, exist_ok=True)
            receipt.write_bytes(sentinel)
            args.plan = name
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "Retain existing"):
                analysis.run(args)
            self.assertEqual(receipt.read_bytes(), sentinel)

    def test_later_plan_does_not_repeat_already_compressed_evidence(self):
        storage.apply_file(storage.plan_file(self.name))
        args = argparse.Namespace(command="compact-analysis", workers=2, minimum_mib=1,
                                  plan=storage.PLAN, apply=False, plan_sha256=None)
        with redirect_stdout(io.StringIO()), patch.object(storage, "plan_file") as capture:
            report = analysis.run(args)
        capture.assert_not_called()
        self.assertEqual(report["files"], [])
        self.assertEqual(report["summary"]["logical_bytes"], 0)

    def test_initial_apply_journal_is_durable_before_first_mutation(self):
        report = self.make_report()
        plan = self.root / storage.PLAN
        analysis.save(plan, report)
        args = argparse.Namespace(command="compact-analysis", workers=2, minimum_mib=1,
                                  plan=storage.PLAN, apply=True, plan_sha256=analysis.sha(plan))
        apply = storage.apply_file

        def check_journal(row):
            receipt = json.loads(plan.with_name("storage-compaction-apply.json").read_bytes())
            self.assertEqual(receipt["mode"], "apply")
            self.assertEqual(receipt["plan"]["sha256"], args.plan_sha256)
            self.assertEqual(receipt["files"], report["files"])
            self.assertNotIn("finished_at_utc", receipt)
            return apply(row)

        with patch.object(storage, "apply_file", side_effect=check_journal), redirect_stdout(io.StringIO()):
            self.assertEqual(analysis.run(args)["summary"]["failures"], 0)


if __name__ == "__main__":
    unittest.main()
