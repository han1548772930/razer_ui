"""Conservative scratch cleanup tests; no analyzer or vendor code runs."""
from contextlib import redirect_stdout
from pathlib import Path
import io
import json
import os
import tempfile
import time
import unittest
from unittest.mock import MagicMock, patch

import workspace


class StaleScratchTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        (self.root / ".work").mkdir()
        self.before = time.time() - 100
        self.root_patch = patch.object(workspace, "ROOT", self.root)
        self.root_patch.start()

    def tearDown(self):
        self.root_patch.stop()
        self.directory.cleanup()

    def file(self, name, content=b"discardable log", old=True):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        if old:
            os.utime(path, (self.before - 100, self.before - 100))
        return path

    def run_clean(self, references=None, processes=None):
        with patch.object(workspace, "_scratch_references", return_value=(references or {}, [])), \
             patch.object(workspace, "_scratch_processes", return_value=(processes or [], [])), \
             redirect_stdout(io.StringIO()):
            return workspace.clean(stale=True, before=self.before)

    def test_only_old_root_logs_and_cache_are_removed(self):
        old = self.file(".work/old.log", b"old")
        cache = self.file(".work/__pycache__/helper.cpython-314.pyc", b"cache")
        protected = [self.file(".work/current.py"), self.file(".work/proof.json"),
                     self.file(".work/native/base.i64"), self.file(".work/native/supplements/new.i64"),
                     self.file(".work/analyzers/python.zip"), self.file(".work/reverse-pending/logs/current.log"),
                     self.file(".work/unknown/subtree.log")]
        report = self.run_clean()
        self.assertFalse(old.exists() or cache.exists())
        self.assertTrue(all(p.exists() for p in protected))
        self.assertEqual(report["summary"]["removed_files"], 2)
        self.assertEqual(report["summary"]["removed_bytes"], 8)
        self.assertTrue(all(r["path"].startswith(".work/") for r in report["removed"]))

    def test_references_active_paths_and_new_files_are_retained(self):
        referenced = self.file(".work/referenced.txt")
        active = self.file(".work/active.log")
        recent = self.file(".work/new.log", old=False)
        report = self.run_clean({"referenced.txt": ["docs/re/tooling.md"]},
                                [{"pid": 123, "command_line": str(active)}])
        self.assertTrue(referenced.exists() and active.exists() and recent.exists())
        reasons = {r["path"]: r["reason"] for r in report["retained"]}
        self.assertEqual(reasons[".work/referenced.txt"], "referenced")
        self.assertEqual(reasons[".work/active.log"], "active_process_path")
        self.assertEqual(reasons[".work/new.log"], "newer_than_run")

    def test_reference_or_process_scan_failure_fails_closed(self):
        for failing in ("references", "processes"):
            with self.subTest(failing=failing):
                path = self.file(".work/retain.log")
                with patch.object(workspace, "_scratch_references", return_value=({}, ["unreadable receipt"] if failing == "references" else [])), \
                     patch.object(workspace, "_scratch_processes", return_value=([], ["unqueryable process list"] if failing == "processes" else [])), \
                     redirect_stdout(io.StringIO()):
                    report = workspace.clean(stale=True, before=self.before)
                self.assertTrue(path.exists())
                self.assertEqual(report["summary"]["removed_files"], 0)

    def test_changed_candidate_is_retained_after_snapshot(self):
        path = self.file(".work/changing.log")
        def scan(_):
            path.write_bytes(b"changed after candidate selection")
            return {}, []
        with patch.object(workspace, "_scratch_references", side_effect=scan), \
             patch.object(workspace, "_scratch_processes", return_value=([], [])), \
             redirect_stdout(io.StringIO()):
            report = workspace.clean(stale=True, before=self.before)
        self.assertTrue(path.exists())
        self.assertIn("changed", next(r["reason"] for r in report["retained"] if r["path"] == ".work/changing.log"))

    def test_linked_scratch_root_is_rejected_without_touching_target(self):
        with tempfile.TemporaryDirectory() as outside:
            target = Path(outside) / "keep.log"
            target.write_bytes(b"outside")
            (self.root / ".work").rmdir()
            try:
                (self.root / ".work").symlink_to(outside, target_is_directory=True)
            except OSError:
                self.skipTest("Creating test directory symlinks is unavailable")
            with self.assertRaisesRegex(ValueError, "link|escape"):
                self.run_clean()
            self.assertEqual(target.read_bytes(), b"outside")

    def test_link_guard_refuses_scratch_root_without_scanning_or_deleting(self):
        path = self.file(".work/keep.log")
        with patch.object(workspace, "_scratch_link", return_value=True), \
             patch.object(workspace, "_scratch_references") as references, \
             patch.object(workspace, "_scratch_unlink") as unlink:
            with self.assertRaisesRegex(ValueError, "link|escape"):
                workspace.clean(stale=True, before=self.before)
        references.assert_not_called()
        unlink.assert_not_called()
        self.assertTrue(path.exists())

    def test_outside_file_is_rejected_by_deletion_boundary(self):
        with tempfile.TemporaryDirectory() as outside:
            path = Path(outside) / "keep.log"
            path.write_bytes(b"outside")
            snapshot = path.stat()
            with self.assertRaisesRegex(ValueError, "linked or escaped"):
                workspace._scratch_unlink(path, (snapshot.st_size, snapshot.st_mtime_ns, snapshot.st_ino))
            self.assertEqual(path.read_bytes(), b"outside")

    def test_link_or_junction_candidate_is_retained_without_descent(self):
        path = self.file(".work/keep.log")
        cache = self.file(".work/__pycache__/keep.pyc")
        original = workspace._scratch_link
        def is_link(candidate):
            return candidate in {path, cache.parent} or original(candidate)
        with patch.object(workspace, "_scratch_link", side_effect=is_link):
            report = self.run_clean()
        self.assertTrue(path.exists() and cache.exists())
        self.assertEqual(report["summary"]["removed_files"], 0)

    @unittest.skipUnless(os.name == "nt", "Windows exclusive file ownership guard")
    def test_file_open_by_a_process_is_retained_by_exclusive_access_guard(self):
        path = self.file(".work/open.log")
        with path.open("ab"):
            report = self.run_clean()
        self.assertTrue(path.exists())
        self.assertEqual(report["summary"]["removed_files"], 0)
        self.assertIn("exclusive_access_unavailable", next(r["reason"] for r in report["retained"]
                      if r["path"] == ".work/open.log"))

    def test_linked_candidate_or_cache_directory_is_never_followed(self):
        with tempfile.TemporaryDirectory() as outside:
            target = Path(outside) / "keep.log"
            target.write_bytes(b"outside")
            try:
                (self.root / ".work/escape.log").symlink_to(target)
                (self.root / ".work/__pycache__").symlink_to(outside, target_is_directory=True)
            except OSError:
                self.skipTest("Creating test symlinks is unavailable")
            report = self.run_clean()
            self.assertEqual(target.read_bytes(), b"outside")
            self.assertEqual(report["summary"]["removed_files"], 0)

    def test_reference_scan_includes_analysis_receipts_and_does_not_enter_raw_or_ref(self):
        self.file("docs/re/tooling.md", b".work/one.log")
        self.file("tools/helper.py", b"'two.txt'")
        self.file(".analysis/results/native/hash/supplements/id/session.json", b'{"log":".work/three.out"}')
        forbidden = self.file(".analysis/inputs/frontend/app/raw/never.txt", b"must not scan")
        original = Path.open
        def guarded(path, *args, **kwargs):
            if path == forbidden or ".ref" in path.parts:
                raise AssertionError("Forbidden tree was read")
            return original(path, *args, **kwargs)
        with patch.object(Path, "open", guarded):
            references, failures = workspace._scratch_references(["one.log", "two.txt", "three.out"])
        self.assertFalse(failures)
        self.assertEqual(set(references), {"one.log", "two.txt", "three.out"})

    def test_fixed_string_reference_scanner_validates_mapping_and_no_match_status(self):
        self.file("docs/re/tooling.md", b"one.log")
        cases = [(0, b"docs/re/tooling.md:one.log\n", True, {"one.log"}),
                 (1, b"", True, set()), (2, b"", False, set()),
                 (0, b"", False, set()), (1, b"docs/re/tooling.md:one.log\n", False, set()),
                 (0, b"docs/re/tooling.md:unknown.log\n", False, set())]
        for code, output, passed, expected in cases:
            with self.subTest(code=code, output=output), \
                 patch.object(workspace.shutil, "which", return_value="inert-rg"), \
                 patch.object(workspace.subprocess, "run", return_value=MagicMock(
                     returncode=code, stdout=output, stderr=b"synthetic error")) as execute:
                references, errors = workspace._scratch_references(["one.log"])
            self.assertEqual(not errors, passed)
            self.assertEqual(set(references), expected)
            arguments = execute.call_args.args[0]
            self.assertIn("--fixed-strings", arguments)
            self.assertEqual(arguments[arguments.index("--threads") + 1], "16")
            self.assertIn("!**/.ref/**", arguments)
            self.assertNotIn("--follow", arguments)

    def test_current_queue_start_is_the_cleanup_cutoff(self):
        path = self.file(".work/current-round.log")
        receipt = self.root / ".analysis/results/reverse-pending.json"
        receipt.parent.mkdir(parents=True)
        started = workspace.datetime.fromtimestamp(self.before - 200, workspace.timezone.utc).isoformat()
        receipt.write_text(json.dumps({"started_at_utc": started}), encoding="utf-8")
        report = self.run_clean()
        self.assertTrue(path.exists())
        self.assertEqual(report["summary"]["removed_files"], 0)


if __name__ == "__main__":
    unittest.main()
