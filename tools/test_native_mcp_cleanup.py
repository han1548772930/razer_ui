"""MCP-only cleanup requires an ended owner and complete retained query scope."""
from contextlib import ExitStack
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import code_identity as ci
from code_identity_native import function_identity
import native_mcp_cleanup as cleanup
from test_code_identity_native import fixture
import workspace as w


class McpCleanupTests(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory())).resolve()
        self.stack.enter_context(patch.object(w, "ROOT", self.root))
        self.absence = self.stack.enter_context(patch("reverse_pending.coordinator_instance_absence", return_value={"confirmed_absent": True}))
        self.stack.enter_context(patch("host.inspect_pe", return_value={"image_base": "0x180000000", "architecture": "x64",
            "sections": [{"rva": 0x1000, "bytes": 2, "file_offset": 0}]}))
        self.original = self.file(".analysis/inputs/test.dll", bytes.fromhex("90c3"))
        self.sha = w.digest(self.original)
        self.base = ".analysis/results/native/" + self.sha
        self.name = ".work/native/" + self.sha + "/mcp"
        self.file(self.name + "/test.dll", self.original.read_bytes())
        self.file(self.name + "/test.dll.i64", b"owned private database")
        self.owner = ".analysis/results/frontend/example-audit.json"
        self.selector = self.owner + "#/client"
        self.session = {"pid": 42, "started_at_utc": "2000-01-01T00:00:00+00:00", "mode": "mcp",
            "input": self.name + "/test.dll", "argv": [self.name + "/test.dll"], "target_execution": False,
            "real_device_operations": False, "private_analysis_database_required": False,
            "finished_at_utc": "2000-01-01T00:00:02+00:00", "exit_code": None,
            "shutdown": {"requested_at_utc": "2000-01-01T00:00:01+00:00", "observed_at_utc": "2000-01-01T00:00:02+00:00",
                "process_instance": {"pid": 42}, "process_instance_confirmed_absent": True, "target_execution": False}}
        self.write(self.base + "/mcp-session.json", self.session)
        self.write(self.base + "/plan.json", {"input": ".analysis/inputs/test.dll", "sha256": self.sha, "output": self.base})
        body, read = fixture()
        body["pseudocode"] = {"lines": [{"text": "return;"}]}
        query = {"input_sha256": self.sha, "target_execution": False, "functions": [body], "failures": []}
        raw = json.dumps(query).encode()
        query_sha = ci.hashlib.sha256(raw).hexdigest()
        self.query = self.base + "/queries/" + query_sha + ".json"
        self.file(self.query, raw)
        self.report = {"private_analysis_database_required": False, "source_slice_complete": True,
            "bounded_observation_complete": True, "snapshot_stable": True, "failures": [], "changed_during_query": [],
            "target_execution": False, "recorded_at_utc": "2000-01-01T00:00:03+00:00",
            "binary_scope": {"original": self.identity(".analysis/inputs/test.dll")},
            "temporary_resources_created": [{"path": self.name, "required_after_publication": False}],
            "new_analysis_sessions": [self.identity(self.base + "/mcp-session.json")],
            "analysis_plan": self.identity(self.base + "/plan.json"), "analyzer_shutdown": self.session["shutdown"],
            "query_receipts": [self.identity(self.query)],
            "native_evidence": {"owners": [{"capture": self.identity(self.query), "pointer": "/functions/0",
                "canonical_sha256": ci.digest(body), "function_identity": function_identity(body, 0x180000000, self.sha, "x64", read)}]}}
        self.publish()

    def file(self, name, raw):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def write(self, name, value):
        return self.file(name, json.dumps(value).encode())

    def identity(self, name):
        path = self.root / name
        return {"path": name, "bytes": path.stat().st_size, "sha256": w.digest(path)}

    def publish(self):
        self.write(self.owner, {"client": self.report})

    def test_selected_queries_release_without_whole_capture_or_faked_exit(self):
        rows = cleanup.candidates(w, [self.selector])
        self.assertEqual(rows[0]["kind"], "ended_native_mcp_private_copy")
        self.assertEqual(rows[0]["selected_functions_verified"], 1)
        self.assertIsNone(rows[0]["observed_exit_code"])
        self.assertFalse((self.root / self.base / "capture.json").exists())
        self.assertTrue((self.root / self.name).exists())

    def test_live_process_and_retained_database_are_rejected(self):
        self.absence.return_value = {"confirmed_absent": False}
        with self.assertRaisesRegex(ValueError, "has not ended"):
            cleanup.candidates(w, [self.selector])
        self.absence.return_value = {"confirmed_absent": True}
        self.report["private_analysis_database_required"] = True
        self.publish()
        with self.assertRaisesRegex(ValueError, "has not released"):
            cleanup.candidates(w, [self.selector])

    def test_changed_query_or_missing_query_inventory_stops_release(self):
        self.file(self.query, b"changed")
        with self.assertRaisesRegex(ValueError, "retained evidence changed"):
            cleanup.candidates(w, [self.selector])

    def test_unlisted_query_and_unknown_private_file_are_rejected(self):
        extra = self.file(self.base + "/queries/extra.json", b"{}")
        with self.assertRaisesRegex(ValueError, "query inventory"):
            cleanup.candidates(w, [self.selector])
        extra.unlink()
        self.file(self.name + "/unowned.bin", b"must retain")
        with self.assertRaisesRegex(ValueError, "unknown or linked"):
            cleanup.candidates(w, [self.selector])

    def test_explicit_source_database_reference_remains_a_dependency(self):
        value = {"input": self.name + "/test.dll", "source_database": self.name + "/test.dll.i64"}
        remaining = list(w._clean_json_dependencies(value, own_receipt=True))
        self.assertIn(value["source_database"], remaining)

    def test_reference_scan_ignores_only_owned_ended_history(self):
        rows = cleanup.candidates(w, [self.selector])
        references, errors = w._clean_references(rows)
        self.assertEqual(errors, [])
        self.assertEqual(references, {})
        self.write(".analysis/results/future.json", {"source_database": self.name + "/test.dll.i64"})
        references, errors = w._clean_references(rows)
        self.assertEqual(errors, [])
        self.assertIn(".analysis/results/future.json", references[self.name])

    def test_wrong_directory_and_changed_session_are_rejected(self):
        self.report["temporary_resources_created"][0]["path"] = ".work"
        self.publish()
        with self.assertRaisesRegex(ValueError, "exact released directory"):
            cleanup.candidates(w, [self.selector])
        self.report["temporary_resources_created"][0]["path"] = self.name
        self.publish()
        self.session["input"] = ".work/other/test.dll"
        self.write(self.base + "/mcp-session.json", self.session)
        with self.assertRaisesRegex(ValueError, "retained evidence changed"):
            cleanup.candidates(w, [self.selector])


if __name__ == "__main__":
    unittest.main()
