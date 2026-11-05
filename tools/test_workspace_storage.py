"""Check file accounting and deletion authorization boundaries without Cargo."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import workspace_storage as storage


class StorageTests(unittest.TestCase):
    def test_inventory_counts_distinct_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()
            (root / "target/a").write_bytes(b"a" * 9000)
            (root / "target/b").write_bytes(b"b" * 17000)
            with patch.object(storage.a, "ROOT", root):
                actual = storage.measure("target")
            self.assertEqual(actual["logical_bytes"], 26000)
            self.assertEqual(actual["files"], 2)
            self.assertGreaterEqual(actual["allocated_bytes"], 26000)
            self.assertEqual(actual["failures"], [])

    def test_changed_plan_and_wrong_scope_never_run_cargo(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            p = root / "plan.json"
            p.write_text(json.dumps({"schema_version": 2, "kind": "workspace_storage_plan",
                "cargo_cleanup": {"argv": ["cargo", "clean"]}}), encoding="utf-8")
            with patch.object(storage.a, "ROOT", root), patch.object(storage.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "SHA changed"):
                    storage.apply(p, "0" * 64)
                with self.assertRaisesRegex(ValueError, "scope changed"):
                    storage.apply(p, storage.a.sha(p))
                run.assert_not_called()

    def test_incomplete_process_inventory_stops_cleanup(self):
        with patch("workspace._scratch_processes", return_value=([], ["unavailable"])):
            with self.assertRaisesRegex(ValueError, "inventory failed"):
                storage._no_build_process()

    def test_active_build_stops_cleanup(self):
        processes = [{"pid": 7, "executable": "cargo.exe", "command_line": "cargo check"}]
        with patch("workspace._scratch_processes", return_value=(processes, [])):
            with self.assertRaisesRegex(ValueError, "still active"):
                storage._no_build_process()


if __name__ == "__main__":
    unittest.main()
