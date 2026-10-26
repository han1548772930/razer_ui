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
        for name in (".analysis/results/reverse-pending/review-sources/review.json",
                     ".analysis/results/reverse-pending/review-sources/" + "a" * 64 + ".py",
                     ".analysis/results/reverse-pending/" + "a" * 64 + ".json"):
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

    def test_function_scope_rejects_noncanonical_rvas_attempts_and_traversal(self):
        prefix = ".analysis/results/native/" + "a" * 64
        for suffix in ("/functions/01000.json", "/functions/0X1000.json", "/functions/1000.json/other.json",
                       "/functions/../1000.json", "/functions/10000000000000000.json",
                       "/supplements/abc/functions/1000.json", "/supplements/" + "b" * 17 + "/functions/1000.json"):
            with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                storage.native_dataset(prefix + suffix)

    def test_function_dependencies_require_a_canonical_supplement_owner(self):
        prefix = ".analysis/results/native/" + "a" * 64
        for group in storage.FUNCTION_INDEXES.keys() - {"functions"}:
            for suffix in (f"/{group}/1000.json", f"/supplements/abc/{group}/1000.json",
                           f"/supplements/{'b' * 16}/{group}/01000.json",
                           f"/supplements/{'b' * 16}/{group}/../1000.json"):
                with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                    storage.native_dataset(prefix + suffix)

    def test_manifest_scope_requires_a_canonical_supplement_owner(self):
        prefix = ".analysis/results/native/" + "a" * 64
        for suffix in ("/capture.json", "/supplements/abc/capture.json",
                       "/supplements/" + "b"*17 + "/capture.json",
                       "/supplements/" + "b"*16 + "/plan.json",
                       "/supplements/" + "b"*16 + "/../capture.json"):
            with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                storage.native_dataset(prefix+suffix)


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

    def make_function(self, *, supplement=False, version1=False, failed=False):
        module = self.dataset.parent
        original = ".analysis/inputs/test.dll"
        self.manifest.update(input=original, target_execution=False, real_device_operations=False)
        self.write_capture()
        base = {"path": analysis.rel(self.capture), "sha256": analysis.sha(self.capture)}
        selection = {"mode": "retained_function_reanalysis", "targets": [{"rva": "0x1000"}]}
        plan = {"input": original, "sha256": "a" * 64, "input_bytes": 7,
                "target_execution": False, "real_device_operations": False}
        session = {"pid": 2147483647, "started_at_utc": "2026-01-01T00:00:00+00:00",
                   "finished_at_utc": "2026-01-01T00:00:01+00:00", "state": "failed" if failed else "finished",
                   "passed": not failed, "exit_code": 1 if failed else 0, "target_execution": False,
                   "real_device_operations": False, "process_released": True}
        if supplement:
            attempt = "b" * 16
            plan.update(base_capture=base, source_database={"path": ".work/native/source.i64", "sha256": "d" * 64},
                        function_selection=selection)
            if version1:
                request = {"schema_version": 1, "input": {"path": original, "bytes": 7, "sha256": "a" * 64},
                           "base_capture": {**base, "bytes": self.capture.stat().st_size},
                           "source_database": {**plan["source_database"], "bytes": 8},
                           "selection_key": "function_selection", "selection": selection, "tool_sources": {},
                           "analyzer": {"installation_path": "idat.exe", "sha256": "e" * 64}}
                digest = analysis.native_supplement_request_sha256(request)
                attempt = digest[:16] + "-" + "c" * 16
                plan.update(analysis_request=request, request_sha256=digest, attempt_id=attempt)
                session.update(request_sha256=digest, attempt_id=attempt, analyzer="idat.exe",
                               analyzer_sha256="e" * 64, tool_sources={}, source_identity_rechecked=True)
            output = module / "supplements" / attempt
        else:
            output = module
        plan["output"] = analysis.rel(output)
        path = output / "functions/1000.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        content = json.dumps({"rva": "0x1000", "independent_observation": "complete retained bytes " * 65536}).encode()
        path.write_bytes(content)
        row = {"rva": "0x1000", "path": "functions/1000.json", "sha256": hashlib.sha256(content).hexdigest()}
        capture = {"schema_version": 1, "input": original, "input_sha256": "a" * 64,
                   "target_execution": False, "real_device_operations": False,
                   "capture_complete": not failed, "functions": [row]}
        if supplement:
            capture.update(base_capture=base, function_selection=selection)
        capture_path, session_path, plan_path = (output / "capture.json",
            output / ("session.json" if supplement else "batch-session.json"), output / "plan.json")
        for destination, body in ((capture_path, capture), (session_path, session), (plan_path, plan)):
            destination.write_text(json.dumps(body), encoding="utf-8")
        return analysis.rel(path), path, capture_path, session_path, plan_path

    def make_function_dependency(self, group, *, failed=False):
        _, path, capture_path, session_path, plan_path = self.make_function(supplement=True, failed=failed)
        destination = path.parent.parent / group / path.name
        destination.parent.mkdir(exist_ok=True)
        path.replace(destination)
        capture = json.loads(capture_path.read_bytes())
        row = capture.pop("functions")[0]
        row.update(path=group + "/" + destination.name, bytes=destination.stat().st_size)
        capture[storage.FUNCTION_INDEXES[group]] = ([{"rva": row["rva"],
            "capture": {key: row[key] for key in ("path", "bytes", "sha256")}}]
            if group == "preanalysis" else [row])
        capture_path.write_text(json.dumps(capture), encoding="utf-8")
        return analysis.rel(destination), destination, capture_path, session_path, plan_path

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

    def test_explicit_immutable_review_keeps_its_path_hash_and_complete_logical_bytes(self):
        digest = hashlib.sha256(self.content).hexdigest()
        name = ".analysis/results/reverse-pending/review-sources/" + digest + ".json"
        review = self.root / name
        review.parent.mkdir(parents=True)
        review.write_bytes(self.content)
        row = storage.plan_file(name)
        self.assertIsNone(row["capture"]["capture_complete"])
        self.assertEqual(row["capture"]["scope"], "immutable_native_static_review")
        result = storage.apply_file(row)
        self.assertEqual(review.read_bytes(), self.content)
        self.assertEqual(review.name, digest + ".json")
        self.assertTrue(result["logical_bytes_unchanged"])
        self.assertGreater(result["bytes_reclaimed"], 0)
        forged = review.with_name("b" * 64 + ".json")
        forged.write_bytes(self.content)
        with self.assertRaisesRegex(ValueError, "differs"):
            storage.plan_file(forged.relative_to(self.root).as_posix())

    def test_explicit_root_function_compresses_without_changing_any_logical_evidence(self):
        name, path, capture, session, plan = self.make_function()
        originals = {item: item.read_bytes() for item in (path, capture, session, plan)}
        row = storage.plan_file(name)
        self.assertEqual(row["capture"]["scope"], "explicit_native_function")
        result = storage.apply_file(row)
        self.assertGreater(result["bytes_reclaimed"], 0)
        self.assertTrue(result["logical_bytes_unchanged"])
        self.assertEqual({item: item.read_bytes() for item in originals}, originals)

    def test_supplement_function_accepts_canonical_and_source_bound_version1_attempts(self):
        for version1 in (False, True):
            with self.subTest(version1=version1):
                name, path, capture, session, plan = self.make_function(supplement=True, version1=version1)
                row = storage.plan_file(name)
                self.assertEqual(row["capture"]["base_capture"]["sha256"], analysis.sha(self.capture))
                before = path.read_bytes()
                self.assertTrue(storage.apply_file(row)["logical_bytes_unchanged"])
                self.assertEqual(path.read_bytes(), before)

    def test_terminal_supplement_manifest_compression_preserves_audit_and_failure_status(self):
        for version1, failed in ((False, False), (True, False), (True, True)):
            with self.subTest(version1=version1, failed=failed):
                _, _, capture, session, plan = self.make_function(supplement=True, version1=version1, failed=failed)
                value = json.loads(capture.read_bytes())
                value["complete_audit"] = "complete original optimizer observations " * 65536
                capture.write_text(json.dumps(value), encoding="utf-8")
                original = {p: p.read_bytes() for p in (capture, session, plan, self.capture)}
                name = analysis.rel(capture)
                row = storage.plan_file(name)
                self.assertEqual(row["capture"]["scope"], "explicit_native_supplement_manifest")
                self.assertEqual(row["capture"]["session_passed"], not failed)
                self.assertEqual(row["capture"]["capture_complete"], not failed)
                result = storage.apply_file(row)
                self.assertGreaterEqual(result["bytes_reclaimed"], 0)
                if not row["compressed"]:
                    self.assertGreater(result["bytes_reclaimed"], 0)
                self.assertEqual({p: p.read_bytes() for p in original}, original)
                for receipt in (capture, session, plan, self.capture):
                    before = receipt.read_bytes()
                    receipt.write_bytes(before+b" ")
                    with self.subTest(changed_receipt=receipt), self.assertRaises(ValueError):
                        storage.apply_file(row)
                    receipt.write_bytes(before)

    def test_manifest_compaction_rejects_a_live_analyzer(self):
        _, _, capture, session, _ = self.make_function(supplement=True)
        value = json.loads(session.read_bytes())
        value["pid"] = os.getpid()
        session.write_text(json.dumps(value), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "terminal released"):
            storage.plan_file(analysis.rel(capture))

    def test_helper_and_preanalysis_compression_preserves_every_index_and_terminal_outcome(self):
        for group in storage.FUNCTION_INDEXES.keys() - {"functions"}:
            with self.subTest(group=group):
                name, path, capture, session, plan = self.make_function_dependency(group, failed=True)
                originals = {item: item.read_bytes() for item in (path, capture, session, plan)}
                row = storage.plan_file(name)
                self.assertEqual(row["capture"]["artifact_group"], group)
                self.assertEqual(row["capture"]["capture_index"], storage.FUNCTION_INDEXES[group])
                self.assertFalse(row["capture"]["session_passed"])
                result = storage.apply_file(row)
                self.assertGreater(result["bytes_reclaimed"], 0)
                self.assertTrue(result["logical_bytes_unchanged"])
                self.assertEqual({item: item.read_bytes() for item in originals}, originals)

    def test_preanalysis_requires_its_own_exact_nested_capture_identity(self):
        name, path, capture, _, _ = self.make_function_dependency("preanalysis")
        original = json.loads(capture.read_bytes())
        for modify in (lambda value: value.pop("decompiler_prepasses"),
                       lambda value: value["decompiler_prepasses"].append(copy.deepcopy(value["decompiler_prepasses"][0])),
                       lambda value: value["decompiler_prepasses"][0].update(rva="0x1001"),
                       lambda value: value["decompiler_prepasses"][0].pop("capture"),
                       lambda value: value["decompiler_prepasses"][0]["capture"].pop("bytes"),
                       lambda value: value["decompiler_prepasses"][0]["capture"].update(path="functions/1000.json"),
                       lambda value: value["decompiler_prepasses"][0]["capture"].update(sha256="e" * 64),
                       lambda value: value["decompiler_prepasses"][0]["capture"].update(bytes=path.stat().st_size + 1)):
            current = copy.deepcopy(original)
            modify(current)
            capture.write_text(json.dumps(current), encoding="utf-8")
            with self.subTest(modify=modify), self.assertRaises(ValueError):
                storage.plan_file(name)

    def test_helper_cannot_borrow_another_function_index_and_apply_rejects_late_reference_changes(self):
        name, _, capture, session, plan = self.make_function_dependency("entry-partition-helpers")
        original = json.loads(capture.read_bytes())
        row = storage.plan_file(name)
        current = copy.deepcopy(original)
        current["functions"] = current.pop("entry_partition_helpers")
        capture.write_text(json.dumps(current), encoding="utf-8")
        with self.assertRaises(ValueError):
            storage.plan_file(name)
        capture.write_text(json.dumps(original), encoding="utf-8")
        for receipt in (capture, session, plan, self.capture):
            before = receipt.read_bytes()
            receipt.write_bytes(before + b" ")
            with self.subTest(receipt=receipt), self.assertRaises(ValueError):
                storage.apply_file(row)
            receipt.write_bytes(before)

    def test_function_failed_terminal_session_keeps_its_real_failure_outcome(self):
        name, path, capture, session, plan = self.make_function(supplement=True, failed=True)
        originals = {item: item.read_bytes() for item in (capture, session, plan)}
        row = storage.plan_file(name)
        self.assertFalse(row["capture"]["capture_complete"])
        self.assertFalse(row["capture"]["session_passed"])
        self.assertEqual(row["capture"]["session_exit_code"], 1)
        storage.apply_file(row)
        self.assertEqual({item: item.read_bytes() for item in originals}, originals)

    def test_function_requires_terminal_session_and_released_process(self):
        name, path, capture, session, plan = self.make_function(supplement=True)
        original = json.loads(session.read_bytes())
        for modify in (lambda row: row.pop("finished_at_utc"), lambda row: row.update(state="running"),
                       lambda row: row.update(process_released=False), lambda row: row.update(exit_code=True),
                       lambda row: row.update(pid=0), lambda row: row.update(exit_code=1)):
            current = copy.deepcopy(original)
            modify(current)
            session.write_text(json.dumps(current), encoding="utf-8")
            with self.subTest(modify=modify), self.assertRaises(ValueError):
                storage.plan_file(name)
        session.write_text(json.dumps(original), encoding="utf-8")
        for alive in (True, None):
            with self.subTest(alive=alive), patch("reverse_pending.process_alive", return_value=alive), \
                    self.assertRaisesRegex(ValueError, "terminal released"):
                storage.plan_file(name)

    def test_function_index_requires_exact_unique_path_rva_sha_and_declared_size(self):
        name, path, capture, session, plan = self.make_function(supplement=True)
        original = json.loads(capture.read_bytes())
        for modify in (lambda row: row.update(functions=[]),
                       lambda row: row["functions"].append(copy.deepcopy(row["functions"][0])),
                       lambda row: row["functions"][0].update(rva="0x1001"),
                       lambda row: row["functions"][0].update(path="functions/../functions/1000.json"),
                       lambda row: row["functions"][0].update(sha256="e" * 64),
                       lambda row: row["functions"][0].update(bytes=path.stat().st_size + 1)):
            current = copy.deepcopy(original)
            modify(current)
            capture.write_text(json.dumps(current), encoding="utf-8")
            with self.subTest(modify=modify), self.assertRaises(ValueError):
                storage.plan_file(name)

    def test_function_apply_rejects_changed_capture_plan_session_and_base_identity(self):
        name, path, capture, session, plan = self.make_function(supplement=True)
        row = storage.plan_file(name)
        for receipt in (capture, session, plan, self.capture):
            before = receipt.read_bytes()
            receipt.write_bytes(before + b" ")
            with self.subTest(receipt=receipt), self.assertRaises(ValueError):
                storage.apply_file(row)
            receipt.write_bytes(before)
        with storage.open_dataset(path) as file:
            self.assertFalse(file.info()["compressed"])

    def test_function_version1_requires_request_and_rejects_changed_request(self):
        name, path, capture, session, plan = self.make_function(supplement=True, version1=True)
        original = json.loads(plan.read_bytes())
        for modify in (lambda row: row.pop("analysis_request"),
                       lambda row: row.update(request_sha256="f" * 64),
                       lambda row: row["analysis_request"]["input"].update(bytes=999)):
            current = copy.deepcopy(original)
            modify(current)
            plan.write_text(json.dumps(current), encoding="utf-8")
            with self.subTest(modify=modify), self.assertRaises(ValueError):
                storage.plan_file(name)

    def test_function_and_receipt_links_or_hardlinks_are_rejected(self):
        name, path, capture, session, plan = self.make_function(supplement=True)
        for target in (path, capture, session, plan):
            linked = target.with_name(target.name + ".linked")
            os.link(target, linked)
            with self.subTest(target=target), self.assertRaisesRegex(ValueError, "linked"):
                storage.plan_file(name)
            linked.unlink()

    def test_function_receipts_remain_locked_until_compression_finishes(self):
        name, path, capture, session, plan = self.make_function(supplement=True)
        row = storage.plan_file(name)
        compress = storage.NativeFile.set_compressed

        def check_locks(file):
            for receipt in (capture, session, plan, self.capture):
                with self.subTest(receipt=receipt), self.assertRaises(OSError):
                    with storage.open_dataset(receipt, writable=True):
                        pass
            compress(file)

        with patch.object(storage.NativeFile, "set_compressed", check_locks):
            self.assertTrue(storage.apply_file(row)["logical_bytes_unchanged"])

    def test_function_cli_is_explicit_duplicate_checked_and_frozen_on_apply(self):
        name, path, capture, session, plan = self.make_function(supplement=True)
        args = argparse.Namespace(command="compact-analysis", workers=2, minimum_mib=1000,
            plan=storage.PLAN, apply=False, plan_sha256=None, function_receipt=[name, name])
        with self.assertRaisesRegex(ValueError, "duplicates"):
            analysis.run(args)
        args.function_receipt = [name]
        with redirect_stdout(io.StringIO()):
            report = analysis.run(args)
        self.assertEqual([row["path"] for row in report["files"]], [name])
        args.apply, args.plan_sha256 = True, analysis.sha(self.root / storage.PLAN)
        with self.assertRaisesRegex(ValueError, "unchanged reviewed plan"):
            analysis.run(args)
        args.function_receipt = []
        with redirect_stdout(io.StringIO()):
            result = analysis.run(args)
        self.assertEqual(result["summary"]["files"], 1)

    def test_function_plan_still_rejects_duplicate_paths_and_changed_size(self):
        name, path, capture, session, plan = self.make_function(supplement=True)
        row = storage.plan_file(name)
        report = {"schema_version": 1, "mode": "plan", "finished_at_utc": analysis.stamp(),
                  "target_execution": False, "deletes_files": False, "files": [row, row], "failures": [],
                  "summary": {"files": 2, "logical_bytes": row["bytes"] * 2, "failures": 0, "bytes_reclaimed": 0}}
        with self.assertRaisesRegex(ValueError, "duplicates"):
            self.validate(report)
        changed = copy.deepcopy(row)
        changed["bytes"] += 1
        with self.assertRaises(ValueError):
            storage.apply_file(changed)

    def test_apply_holds_the_exact_reviewed_plan_against_writes_until_completion(self):
        name, _, _, _, _ = self.make_function(supplement=True)
        args = argparse.Namespace(command="compact-analysis", workers=2, minimum_mib=1000,
            plan=storage.PLAN, apply=False, plan_sha256=None, function_receipt=[name])
        with redirect_stdout(io.StringIO()):
            analysis.run(args)
        plan = self.root / storage.PLAN
        original = plan.read_bytes()
        args.apply, args.function_receipt, args.plan_sha256 = True, [], analysis.sha(plan)
        compress = storage.NativeFile.set_compressed
        denied = []

        def verify_plan_locked(file):
            with self.assertRaises(OSError):
                plan.write_bytes(original+b" ")
            denied.append(True)
            compress(file)

        with patch.object(storage.NativeFile, "set_compressed", verify_plan_locked), redirect_stdout(io.StringIO()):
            result = analysis.run(args)
        self.assertEqual(denied, [True])
        self.assertEqual(result["summary"]["failures"], 0)
        self.assertEqual(plan.read_bytes(), original)
        plan.write_bytes(original+b" ")
        self.assertEqual(plan.read_bytes(), original+b" ")

    def test_review_scope_is_opt_in_and_apply_uses_only_the_frozen_plan(self):
        digest = hashlib.sha256(self.content).hexdigest()
        name = ".analysis/results/reverse-pending/review-sources/" + digest + ".json"
        review = self.root / name
        review.parent.mkdir(parents=True)
        review.write_bytes(self.content)
        args = argparse.Namespace(command="compact-analysis", workers=2, minimum_mib=1000,
            plan=storage.PLAN, apply=False, plan_sha256=None, review_receipt=[name])
        with redirect_stdout(io.StringIO()):
            report = analysis.run(args)
        self.assertEqual([r["path"] for r in report["files"]], [name])
        args.apply, args.plan_sha256 = True, analysis.sha(self.root / storage.PLAN)
        with self.assertRaisesRegex(ValueError, "unchanged reviewed plan"):
            analysis.run(args)
        args.review_receipt = []
        with redirect_stdout(io.StringIO()):
            result = analysis.run(args)
        self.assertEqual(result["summary"]["files"], 1)
        self.assertEqual(review.read_bytes(), self.content)
        self.assertFalse(getattr(self.dataset.stat(), "st_file_attributes", 0) & 0x800)

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
