"""Pure queue tests; no vendor process or network is started."""
from concurrent.futures import ThreadPoolExecutor
from contextlib import redirect_stdout
from copy import deepcopy
from pathlib import Path
import argparse
import ast
import base64
import hashlib
import io
import json
import struct
import tempfile
import threading
import traceback
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch
import weakref

import analysis
import reverse_pending as queue


class PendingQueueTests(unittest.TestCase):
    def supplement_fixture(self, root):
        from test_evidence import NativeSupplementTests

        result, previous, capture, supplement, _, write = NativeSupplementTests().reanalysis_fixture(root)
        capture["capture_complete"] = True
        write(result / "capture.json", capture)
        selection = analysis.native_function_selection(bytes(NativeSupplementTests().fixture()), capture, [0x1000])
        selection_id = hashlib.sha256(json.dumps(selection, sort_keys=True).encode()).hexdigest()[:16]
        output = previous.with_name(selection_id)
        previous.rename(output)
        source = root / capture["input"]
        write(root / ".analysis/inputs.json", {"inputs": [{"path": capture["input"],
              "sha256": capture["input_sha256"], "bytes": capture["input_bytes"]}]})
        database = root / ".work/native" / capture["input_sha256"] / "batch" / (source.name + ".i64")
        database.parent.mkdir(parents=True)
        database.write_bytes(b"inert retained IDB; never opened")
        scratch = database.parent.parent / "supplements" / selection_id
        scratch.mkdir(parents=True)
        for name in ("ida.log", "process.log"):
            (scratch / name).write_bytes(b"old failure log\x00\xff " + name.encode())
        session = {"pid": 712, "started_at_utc": "synthetic interrupted session", "previous_attempts": [
            {"pid": 701, "passed": False, "exit_code": 1}]}
        write(output / "session.json", session)
        supplement["capture_complete"] = False
        write(output / "capture.json", supplement)
        analyzer = root / "static-tools/idat.exe"
        analyzer.parent.mkdir()
        analyzer.write_bytes(b"inert analyzer fixture; never executed")
        tool = root / "tools/ida/supplement.py"
        tool.parent.mkdir(parents=True)
        tool.write_bytes(b"inert capture tool fixture; never executed")
        args = argparse.Namespace(input=capture["input"], function_rva=[0x1000], function_type=[],
            type_evidence=None, max_function_kib=None, pointer_table_rva=None, pointer_count=None,
            ida_root=str(analyzer.parent), no_refresh_gaps=True, recover_interrupted=True)
        retained = {path: path.read_bytes() for path in (output / "plan.json", output / "capture.json",
                    output / "session.json", scratch / "ida.log", scratch / "process.log")}
        return args, output, database, retained

    def test_workers_are_bounded_and_never_serial(self):
        for value in (0, 1, -1, True, 2.5, "16", 1000):
            with self.subTest(value=value), self.assertRaises(ValueError):
                queue.Batch(value, "IDA")

    def test_exact_identity_keeps_type_evidence_limits_and_capture_distinct(self):
        original = queue.task_id("native", {"sha256": "a", "base_capture": "first"}, {"limits": 64, "declaration": "unsigned f(void);", "evidence": "ECX"})
        for inputs, options in [
            ({"sha256": "a", "base_capture": "second"}, {"limits": 64, "declaration": "unsigned f(void);", "evidence": "ECX"}),
            ({"sha256": "a", "base_capture": "first"}, {"limits": 1024, "declaration": "unsigned f(void);", "evidence": "ECX"}),
            ({"sha256": "a", "base_capture": "first"}, {"limits": 64, "declaration": "void f(void);", "evidence": "ECX"}),
            ({"sha256": "a", "base_capture": "first"}, {"limits": 64, "declaration": "unsigned f(void);", "evidence": "EDX"})]:
            self.assertNotEqual(original, queue.task_id("native", inputs, options))

    def test_active_session_is_adopted_and_unknown_process_never_restarted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "supplements/one/session.json"
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps({"pid": 12}), encoding="utf-8")
            with patch.object(queue, "identity", return_value={"path": "session"}):
                for state in (True, None):
                    with self.subTest(state=state), patch.object(queue, "process_alive", return_value=state):
                        self.assertEqual(len(queue.active_native_sessions(root)), 1)
                with patch.object(queue, "process_alive", return_value=False):
                    self.assertEqual(queue.active_native_sessions(root), [])

    def test_pool_runs_independent_tasks_before_a_waiting_external_task(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            barrier = threading.Barrier(2)
            jobs = [{"id": str(i), "kind": "native", "input": str(i), "status": "pending"} for i in range(2)]
            batch = queue.Batch(2, "IDA")
            def discover():
                batch.report["tasks"] = jobs
                batch.report["summary"] = {"by_kind": {"native": 2}, "by_status": {"pending": 2}}
                return jobs
            def execute(job):
                barrier.wait(timeout=5)
                return {"status": "unresolved" if job["id"] == "0" else "passed", "remaining": int(job["id"] == "0")}
            with patch.object(batch, "discover", side_effect=discover), patch.object(batch, "execute", side_effect=execute), patch.object(batch, "publish"), patch.object(batch, "publish_gaps"):
                result = batch.run()
            self.assertEqual({job["status"] for job in result["tasks"]}, {"unresolved", "passed"})
            self.assertIn("finished_at_utc", result)
            self.assertFalse(result["semantics_complete"])

    def test_native_exit_zero_does_not_resolve_missing_function_or_conflict(self):
        batch = queue.Batch(2, "IDA")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / ".analysis/inputs/native.dll"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"original")
            sha = analysis.sha(source)
            output = root / ".analysis/results/native" / sha
            output.mkdir(parents=True)
            capture = {"capture_complete": True, "functions": [{"rva": "0x1000"}]}
            (output / "capture.json").write_text(json.dumps(capture), encoding="utf-8")
            resolution = {"remaining_failures": [{"stage": "1000:hexrays", "error": "failed"}],
                          "verification_failures": [{"error": "conflict"}], "resolved_failures": [], "complete_functions": {}}
            with patch.object(analysis, "ROOT", root), patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "verify_native_capture", return_value=(capture, queue.identity(output / "capture.json"))), \
                 patch.object(analysis, "native_reanalysis_resolution", return_value=resolution):
                with patch.object(batch, "command", return_value={"exit_code": 0}):
                    result = batch.native({"id": "one", "input": ".analysis/inputs/native.dll", "sha256": sha})
            self.assertEqual(result["status"], "unresolved")
            self.assertEqual(result["remaining"], 1)
            self.assertEqual(result["verification_errors"], 1)

    def test_interrupted_supplement_recovers_only_after_confirming_process_absent(self):
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()):
            root = Path(directory)
            with patch.object(analysis, "ROOT", root):
                args, output, database, retained = self.supplement_fixture(root)
                database_before = database.read_bytes()
                process = MagicMock(pid=912)

                def finish():
                    (output / "capture.json").write_text('{"capture_complete":true}', encoding="utf-8")
                    return 0

                process.wait.side_effect = finish
                with patch.object(queue, "process_alive", return_value=False) as alive, \
                     patch.object(analysis.subprocess, "Popen", return_value=process) as start, \
                     patch.object(analysis, "refresh_gaps") as refresh:
                    result = analysis.supplement_native(args)
                alive.assert_called_once_with(712)
                start.assert_called_once()
                refresh.assert_not_called()
                self.assertTrue(result["passed"])
                self.assertEqual(database.read_bytes(), database_before)
                self.assertEqual(len(result["previous_attempts"]), 2)
                self.assertFalse(result["previous_attempts"][0]["passed"])
                prior = result["previous_attempts"][1]
                recovery = prior["interruption_recovery"]
                self.assertTrue(recovery["process_confirmed_absent"])
                self.assertIsNone(recovery["exit_code"])
                session_bytes = retained[output / "session.json"]
                self.assertEqual(recovery["original_session_sha256"], hashlib.sha256(session_bytes).hexdigest())
                self.assertEqual(base64.b64decode(recovery["original_session_raw_base64"]), session_bytes)
                self.assertEqual(len(prior["retained_outputs"]), 4)
                for record in prior["retained_outputs"]:
                    original = retained[root / record["path_at_attempt"]]
                    self.assertEqual(record["bytes"], len(original))
                    self.assertEqual(record["sha256"], hashlib.sha256(original).hexdigest())
                    self.assertEqual(base64.b64decode(record["raw_base64"]), original)
                saved = json.loads((output / "session.json").read_bytes())
                self.assertEqual(saved["previous_attempts"], result["previous_attempts"])

    def test_interrupted_supplement_active_or_unqueryable_process_preserves_all_evidence(self):
        for state in (True, None):
            with self.subTest(process_state=state), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    args, _, database, retained = self.supplement_fixture(root)
                    original_database = database.read_bytes()
                    with patch.object(queue, "process_alive", return_value=state) as alive, \
                         patch.object(analysis.subprocess, "Popen") as start:
                        with self.assertRaisesRegex(ValueError, "still running or cannot be verified absent"):
                            analysis.supplement_native(args)
                    alive.assert_called_once_with(712)
                    start.assert_not_called()
                    self.assertEqual(database.read_bytes(), original_database)
                    for path, raw in retained.items():
                        self.assertEqual(path.read_bytes(), raw)

    def test_interrupted_supplement_requires_explicit_recovery_even_if_process_may_be_absent(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(analysis, "ROOT", root):
                args, _, _, retained = self.supplement_fixture(root)
                args.recover_interrupted = False
                with patch.object(queue, "process_alive", return_value=False) as alive, \
                     patch.object(analysis.subprocess, "Popen") as start:
                    with self.assertRaisesRegex(ValueError, "has not finished"):
                        analysis.supplement_native(args)
                alive.assert_not_called()
                start.assert_not_called()
                for path, raw in retained.items():
                    self.assertEqual(path.read_bytes(), raw)

    def test_interrupted_supplement_changed_original_is_rejected_before_process_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(analysis, "ROOT", root):
                args, _, _, retained = self.supplement_fixture(root)
                source = root / args.input
                changed = bytearray(source.read_bytes())
                changed[0x200] ^= 1
                source.write_bytes(changed)
                with patch.object(queue, "process_alive", return_value=False) as alive, \
                     patch.object(analysis.subprocess, "Popen") as start:
                    with self.assertRaisesRegex(ValueError, "differs from its original input receipt"):
                        analysis.supplement_native(args)
                alive.assert_not_called()
                start.assert_not_called()
                for path, raw in retained.items():
                    self.assertEqual(path.read_bytes(), raw)

    def test_native_active_or_unqueryable_supplement_never_starts_another_command(self):
        batch = queue.Batch(2, "IDA")
        for state in (True, None):
            with self.subTest(process_state=state), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                with patch.object(analysis, "ROOT", root):
                    args, output, _, retained = self.supplement_fixture(root)
                    source = root / args.input
                    with patch.object(queue, "process_alive", return_value=state) as alive, \
                         patch.object(batch, "command") as command, \
                         patch.object(analysis, "native_reanalysis_resolution") as resolve:
                        result = batch.native({"id": "one", "input": args.input, "sha256": analysis.sha(source)})
                    alive.assert_called_once_with(712)
                    command.assert_not_called()
                    resolve.assert_not_called()
                    self.assertEqual(result["status"], "waiting_external")
                    self.assertEqual(result["external_sessions"][0]["process_state"], "running" if state else "unknown")
                    self.assertEqual((output / "session.json").read_bytes(), retained[output / "session.json"])

    def test_native_source_changed_before_start_is_rejected_without_any_command(self):
        batch = queue.Batch(2, "IDA")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / ".analysis/inputs/native.dll"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"original")
            digest = analysis.sha(source)
            source.write_bytes(b"tampered")
            with patch.object(analysis, "ROOT", root), patch.object(batch, "command") as command, \
                 patch.object(analysis, "native_reanalysis_resolution") as resolve:
                with self.assertRaisesRegex(ValueError, "original identity changed"):
                    batch.native({"id": "one", "input": ".analysis/inputs/native.dll", "sha256": digest})
            command.assert_not_called()
            resolve.assert_not_called()

    def test_native_source_changed_during_capture_cannot_be_published_as_passed(self):
        batch = queue.Batch(2, "IDA")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / ".analysis/inputs/native.dll"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"original")
            digest = analysis.sha(source)
            output = root / ".analysis/results/native" / digest
            output.mkdir(parents=True)

            def capture(*_):
                source.write_bytes(b"tampered")
                (output / "capture.json").write_text(json.dumps({"input": ".analysis/inputs/native.dll",
                    "input_sha256": digest, "input_bytes": 8, "capture_complete": True,
                    "functions": [], "failures": []}), encoding="utf-8")
                return {"exit_code": 0}

            with patch.object(analysis, "ROOT", root), patch.object(batch, "command", side_effect=capture), \
                 patch.object(queue, "active_native_sessions", return_value=[]):
                with self.assertRaisesRegex(ValueError, "identity changed|original.*changed"):
                    batch.native({"id": "one", "input": ".analysis/inputs/native.dll", "sha256": digest})

    def test_native_complete_flag_cannot_publish_unverified_inventory_or_artifacts(self):
        from test_evidence import EvidenceTests

        for issue in ("missing_inventory", "missing_aggregate", "changed_output"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = root / ".analysis/inputs/native.dll"
                source.parent.mkdir(parents=True)
                source.write_bytes(b"inert original")
                digest = analysis.sha(source)
                fixture = EvidenceTests()
                fixture.root = root
                capture, folder = fixture.native_capture(digest, source.stat().st_size)
                output = root / folder
                batch = queue.Batch(2, "IDA")
                resolution = {"remaining_failures": [], "verification_failures": [],
                              "resolved_failures": [], "complete_functions": {}}
                with patch.object(analysis, "ROOT", root):
                    analysis.verify_native_capture(output, {"sha256": digest, "bytes": source.stat().st_size})
                    if issue == "missing_inventory":
                        capture.pop("outputs")
                    elif issue == "missing_aggregate":
                        capture["outputs"] = [row for row in capture["outputs"] if row["path"] != "all-functions.asm"]
                    else:
                        (output / "strings.json").write_bytes(b"{}")
                    (output / "capture.json").write_text(json.dumps(capture), encoding="utf-8")
                    with patch.object(queue, "active_native_sessions", return_value=[]), \
                         patch.object(batch, "command") as command, \
                         patch.object(analysis, "native_reanalysis_resolution", return_value=resolution) as resolve:
                        with self.assertRaisesRegex(ValueError, "inventories|required outputs|capture changed"):
                            batch.native({"id": "one", "input": ".analysis/inputs/native.dll", "sha256": digest})
                    command.assert_not_called()
                    resolve.assert_not_called()
                    self.assertFalse((root / ".analysis/results/reverse-pending/native" / (digest + ".json")).exists())

    def test_verify_only_native_never_starts_capture_or_supplement_commands(self):
        from test_evidence import EvidenceTests

        for retained in (False, True):
            batch = queue.Batch(2, "IDA", verify_only=True)
            with self.subTest(retained=retained), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = root / ".analysis/inputs/native.dll"
                source.parent.mkdir(parents=True)
                source.write_bytes(b"inert original")
                digest = analysis.sha(source)
                resolution = {"remaining_failures": [{"stage": "1000:hexrays", "error": "failed"}],
                              "verification_failures": [], "resolved_failures": [], "complete_functions": {}}
                if retained:
                    fixture = EvidenceTests()
                    fixture.root = root
                    fixture.native_capture(digest, source.stat().st_size)
                with patch.object(analysis, "ROOT", root), \
                     patch.object(queue, "active_native_sessions", return_value=[]), \
                     patch.object(batch, "command") as command, \
                     patch.object(analysis, "native_reanalysis_resolution", return_value=resolution) as resolve:
                    result = batch.native({"id": "one", "input": ".analysis/inputs/native.dll", "sha256": digest})
                command.assert_not_called()
                self.assertEqual(result["status"], "unresolved")
                self.assertEqual(result["commands"], [])
                self.assertEqual(resolve.call_count, int(retained))
                self.assertFalse((root / ".analysis/results/reverse-pending/native" / (digest + ".json")).exists())
                verified = root / ".analysis/results/reverse-pending/verified-native" / (digest + ".json")
                self.assertEqual(verified.is_file(), retained)
                if retained:
                    self.assertTrue(json.loads(verified.read_bytes())["verification_only"])

    def test_verify_only_frontend_does_not_repair_missing_bytes_or_unstable_deployment(self):
        import evidence
        import frontend

        batch = queue.Batch(2, "IDA", verify_only=True)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_name = ".analysis/inputs/frontend/app-current"
            receipt = root / input_name / "acquisition.json"
            receipt.parent.mkdir(parents=True)
            raw = json.dumps({"entry_url": "https://apps.razer.com/app/",
                "source_current_during_acquisition": False,
                "files": [{"url": "https://apps.razer.com/app/main.js", "error": "HTTP Error 404"}]}).encode()
            receipt.write_bytes(raw)
            evidence_tool = root / "tools/evidence.py"
            evidence_tool.parent.mkdir()
            evidence_tool.write_bytes(b"inert static analyzer identity")
            with patch.object(analysis, "ROOT", root), \
                 patch.object(analysis, "fetch_frontend") as fetch, \
                 patch.object(analysis, "wakaru", return_value=root / "inert-wakaru.exe"), \
                 patch.object(frontend, "recover_source_set", return_value={"status": "no_javascript_originals"}), \
                 patch.object(evidence, "frontend_deployment_sources", return_value={"sources": [], "failures": [{"kind": "source_missing"}]}), \
                 patch.object(evidence, "__file__", str(evidence_tool)), \
                 patch.object(queue.importlib.metadata, "version", return_value="synthetic-parser-version"), \
                 patch.object(batch, "command") as command, \
                 patch.object(analysis.subprocess, "Popen") as process:
                result = batch.frontend({"id": "one", "input": input_name})
            fetch.assert_not_called()
            command.assert_not_called()
            process.assert_not_called()
            self.assertEqual(receipt.read_bytes(), raw)
            self.assertEqual(result["status"], "unresolved")
            self.assertIn("verified-frontend", result["receipt"]["path"])

    def test_verify_only_rejects_unfinished_previous_queue_without_discovery_or_commands(self):
        batch = queue.Batch(2, "IDA", verify_only=True)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipt = root / queue.REPORT
            receipt.parent.mkdir(parents=True)
            previous_raw = b'{"started_at_utc":"retained running queue","tasks":[{"status":"running"}]}'
            receipt.write_bytes(previous_raw)
            with patch.object(analysis, "ROOT", root), patch.object(batch, "discover") as discover, \
                 patch.object(batch, "execute") as execute, patch.object(batch, "command") as command:
                with self.assertRaisesRegex(ValueError, "Previous static queue must finish"):
                    batch.run()
            discover.assert_not_called()
            execute.assert_not_called()
            command.assert_not_called()
            self.assertEqual(receipt.read_bytes(), previous_raw)

    def test_verify_only_preserves_complete_previous_receipt_and_tool_bytes(self):
        batch = queue.Batch(2, "IDA", verify_only=True)
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()):
            root = Path(directory)
            receipt = root / queue.REPORT
            receipt.parent.mkdir(parents=True)
            previous_tool = b"inert previous queue source\x00\xff"
            digest = hashlib.sha256(previous_tool).hexdigest()
            tool_snapshot = root / ".analysis/results/reverse-pending/tool-sources" / (digest + ".py")
            tool_snapshot.parent.mkdir(parents=True)
            tool_snapshot.write_bytes(previous_tool)
            previous_raw = json.dumps({"tool": {"sha256": digest}, "summary": {"tasks": 1},
                "finished_at_utc": "previous finished queue", "tasks": [{"status": "unresolved"}]}).encode() + b"\n"
            receipt.write_bytes(previous_raw)
            document = root / queue.DOC
            document.parent.mkdir(parents=True)
            document.write_text("Existing scope.\n", encoding="utf-8")
            with patch.object(analysis, "ROOT", root), patch.object(batch, "command") as command:
                result = batch.run()
            command.assert_not_called()
            previous = result["previous_static_run"]
            self.assertEqual(base64.b64decode(previous["raw_base64"]), previous_raw)
            self.assertEqual(previous["sha256"], hashlib.sha256(previous_raw).hexdigest())
            self.assertEqual(tool_snapshot.read_bytes(), previous_tool)
            self.assertEqual(previous["tool_source"]["sha256"], digest)
            self.assertTrue(result["verification_only"])
            self.assertFalse(result["semantics_complete"])

    def test_command_wrapper_returns_nonzero_for_finished_unresolved_queue(self):
        args = argparse.Namespace(workers=2, ida_root="IDA", verify_only=True)
        for status in ("passed", "unresolved", "failed", "waiting_external"):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                report = {"finished_at_utc": "finished", "tasks": [{"status": status}]}
                with patch.object(analysis, "ROOT", root), patch.object(queue, "Batch") as batch:
                    batch.return_value.run.return_value = report
                    if status == "passed":
                        self.assertIs(queue.run(args), report)
                    else:
                        with self.assertRaises(SystemExit) as raised:
                            queue.run(args)
                        self.assertEqual(raised.exception.code, 1)
                batch.assert_called_once_with(2, "IDA", True)

    def test_finished_queue_keeps_conflicts_and_semantic_acceptance_pending_in_receipts(self):
        for conflict in (False, True):
            batch = queue.Batch(2, "IDA")
            with self.subTest(conflict=conflict), tempfile.TemporaryDirectory() as directory, \
                 redirect_stdout(io.StringIO()):
                root = Path(directory)
                document = root / queue.DOC
                document.parent.mkdir(parents=True)
                document.write_text("Existing reverse analysis scope.\n", encoding="utf-8")
                jobs = [{"id": str(i), "kind": "native", "input": f"native-{i}", "status": "pending"}
                        for i in range(2)]

                def discover():
                    batch.report["tasks"] = jobs
                    batch.publish()
                    return jobs

                def execute(job):
                    if conflict and job["id"] == "0":
                        return {"status": "unresolved", "remaining": 0, "verification_errors": 1,
                                "error": "Strict supplemental function conflict"}
                    return {"status": "passed", "remaining": 0, "verification_errors": 0}

                with patch.object(analysis, "ROOT", root), patch.object(batch, "discover", side_effect=discover), \
                     patch.object(batch, "execute", side_effect=execute):
                    result = batch.run()
                saved = json.loads((root / queue.REPORT).read_bytes())
                self.assertTrue(saved["summary"]["queue_finished"])
                self.assertFalse(saved["semantics_complete"] or saved["summary"]["semantics_complete"])
                self.assertEqual(saved["official_dll_comparison"], "not_executed")
                self.assertEqual(saved["real_device_acceptance"], "not_executed")
                self.assertTrue(all("finished_at_utc" in job for job in saved["tasks"]))
                self.assertEqual(result["tasks"], saved["tasks"])
                ledger = json.loads((root / ".analysis/results/gaps.json").read_bytes())
                rows = ledger["owners"]["tools/reverse_pending.py"]
                pending = [row for row in rows if row["kind"] == "reverse_pending_task_unresolved"]
                self.assertEqual(len(pending), int(conflict))
                if conflict:
                    self.assertEqual(pending[0]["result"]["remaining"], 0)
                    self.assertEqual(pending[0]["result"]["verification_errors"], 1)
                    self.assertEqual(saved["summary"]["by_status"], {"unresolved": 1, "passed": 1})
                self.assertEqual(sum(row["kind"] == "semantic_and_runtime_acceptance_pending" for row in rows), 1)


class NativeReviewQueueTests(unittest.TestCase):
    def shared_tail_fixture(self, root):
        from test_evidence import NativeSupplementTests

        raw, folder, capture, bodies, write = NativeSupplementTests().shared_tail_fixture(root)
        source = root / ".analysis/inputs/official-native-current/shared.dll"
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(raw)
        original = queue.identity(source)
        capture.update(input=original["path"], functions_complete=True, functions_captured=2,
            functions_total=2, settled_functions_total=2, aggregate_functions_written=2,
            newly_discovered_addresses=[], removed_function_boundaries=[], outputs=[])
        datasets = ("segments", "imports", "strings", "entrypoints", "names", "local_types", "structures")
        for name in [*(item + ".json" for item in datasets), "all-functions.c", "all-functions.asm"]:
            path = folder / name
            path.write_bytes(b"[]" if name.endswith(".json") else b"synthetic static shared-tail fixture\n")
            capture["outputs"].append({**queue.identity(path), "path": name})
        capture.update({name + "_count": 0 for name in datasets})
        captured = write(folder / "capture.json", capture)
        plan = {"schema_version": 1, "source_input": original, "source_capture": captured,
            "function_rvas": ["0x1000"], "type_recoveries": [],
            "function_shared_tails": [{"primary_owner_rva": "0x1000", "tail_start_rva": "0x1004",
                "tail_end_rva": "0x1007", "owner_rvas": ["0x1000", "0x1040"], "inline_owners": True,
                "assembly_evidence": "Both retained entries and their exact relative branch into a shared suffix."}],
            "evidence": [queue.identity(folder / record["path"]) for record, _ in bodies]}
        review = root / ".analysis/results/shared-tail-review.json"
        write(review, {"plans": {"threads/context": plan}})
        return {"source": source, "folder": folder, "original": original, "capture": capture,
            "plan": plan, "review": review, "spec": review.relative_to(root).as_posix() + "#/plans/threads~1context", "write": write}

    def partition_fixture(self, root):
        from test_evidence import NativeSupplementTests

        raw, folder, capture, _, write = NativeSupplementTests().partition_fixture(root)
        source = root / ".analysis/inputs/official-native-current/partition.dll"
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(raw)
        original = queue.identity(source)
        capture.update(input=original["path"], functions_complete=True, functions_captured=1,
            functions_total=1, settled_functions_total=1, aggregate_functions_written=1,
            newly_discovered_addresses=[], removed_function_boundaries=[], outputs=[])
        datasets = ("segments", "imports", "strings", "entrypoints", "names", "local_types", "structures")
        for name in [*(item + ".json" for item in datasets), "all-functions.c", "all-functions.asm"]:
            path = folder / name
            path.write_bytes(b"[]" if name.endswith(".json") else b"synthetic static partition fixture\n")
            capture["outputs"].append({**queue.identity(path), "path": name})
        capture.update({name + "_count": 0 for name in datasets})
        captured = write(folder / "capture.json", capture)
        plan = {"schema_version": 1, "source_input": original, "source_capture": captured,
            "function_rvas": ["0x1000"], "type_recoveries": [],
            "function_partitions": [{"owner_rva": "0x1000",
                "helper_ranges": [{"start_rva": "0x1002", "end_rva_exclusive": "0x1005"}],
                "assembly_evidence": "Exact original local CALL/RET, closed helper CFG and disjoint complete byte union."}],
            "evidence": [queue.identity(folder / "functions/1000.json")]}
        review = root / ".analysis/results/partition-review.json"
        write(review, {"plans": {"threads/context": plan}})
        return {"source": source, "folder": folder, "original": original, "capture": capture,
            "plan": plan, "review": review, "spec": review.relative_to(root).as_posix() + "#/plans/threads~1context", "write": write}

    def fixture(self, root, tag=0, *, imports=False):
        from test_evidence import NativeSupplementTests

        raw = bytearray(NativeSupplementTests().fixture())
        raw[0x200:0x205] = bytes.fromhex("e90b000000")
        raw[0x210:0x213] = bytes.fromhex("8bc1c3")
        raw[0x2ff] = tag
        if imports:
            struct.pack_into("<I", raw, 0x98 + 108, 16)
            struct.pack_into("<II", raw, 0x98 + 112 + 8, 0x2040, 40)
            struct.pack_into("<IIIII", raw, 0x340, 0x2080, 0, 0, 0x2070, 0x2090)
            raw[0x370:0x37c] = b"FIXTURE.dll\0"
            struct.pack_into("<QQ", raw, 0x380, 0x20b0, 0)
            struct.pack_into("<QQ", raw, 0x390, 0x20b0, 0)
            raw[0x3b0:0x3bb] = b"\0\0Observed\0"
        source = root / f".analysis/inputs/official-native-current/fixture-{tag}.dll"

        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(value if isinstance(value, bytes) else
                             (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
            return queue.identity(path)

        original = write(source, bytes(raw))
        folder = root / ".analysis/results/native" / original["sha256"]
        functions, artifacts = [], []
        for rva, instructions in [(0x1000, [(0x1000, "e90b000000", "jmp 0x140001010")]),
                                  (0x1010, [(0x1010, "8bc1", "mov eax, ecx"), (0x1012, "c3", "retn")])]:
            body_bytes = bytes.fromhex("".join(item[1] for item in instructions))
            body = {"address": hex(0x140000000 + rva), "rva": hex(rva),
                    "name": "fixture_" + hex(rva)[2:], "end": hex(0x140000000 + rva + len(body_bytes)),
                    "type": None,
                    "chunks": [{"start": hex(0x140000000 + rva),
                                "end": hex(0x140000000 + rva + len(body_bytes)),
                                "raw_hex": body_bytes.hex(), "sha256": hashlib.sha256(body_bytes).hexdigest()}],
                    "assembly": [{"address": hex(0x140000000 + at), "rva": hex(at),
                                  "bytes": code, "text": text} for at, code, text in instructions],
                    "xrefs_to_entry": [], "basic_blocks": [], "pseudocode": None}
            artifact = write(folder / "functions" / (hex(rva)[2:] + ".json"), body)
            artifacts.append(artifact)
            functions.append({**artifact, "path": "functions/" + hex(rva)[2:] + ".json",
                              "rva": hex(rva), "address": body["address"], "name": body["name"],
                              "assembly_captured": True, "pseudocode_captured": False, "new_failures": []})
        outputs = []
        datasets = ("segments", "imports", "strings", "entrypoints", "names", "local_types", "structures")
        for name in [*(item + ".json" for item in datasets), "all-functions.c", "all-functions.asm"]:
            record = write(folder / name, b"[]" if name.endswith(".json") else b"synthetic static fixture\n")
            outputs.append({**record, "path": name})
        capture = {"schema_version": 1, "input": original["path"], "input_sha256": original["sha256"],
                   "input_bytes": original["bytes"], "image_base": "0x140000000",
                   "capture_complete": True, "functions_complete": True,
                   "functions_captured": 2, "functions_total": 2, "settled_functions_total": 2,
                   "aggregate_functions_written": 2, "newly_discovered_addresses": [],
                   "removed_function_boundaries": [], "functions": functions, "outputs": outputs,
                   "failures": [{"stage": "1000:hexrays", "error": "synthetic call analysis failed"}],
                   **{name + "_count": 0 for name in datasets}}
        captured = write(folder / "capture.json", capture)
        declaration = "unsigned int __fastcall fixture_1010(unsigned int value);"
        plan = {"schema_version": 1, "source_input": original, "source_capture": captured,
                "function_rvas": ["0x1000", "0x1010"],
                "type_recoveries": [{"rva": "0x1010", "declaration": declaration,
                    "assembly_evidence": "Synthetic fixture: 1010 [8bc1] mov eax,ecx; 1012 [c3] retn."}],
                "evidence": artifacts}
        path = root / f".analysis/results/review-{tag}.json"
        write(path, {"plans": {"threads/context": plan}})
        spec = path.relative_to(root).as_posix() + "#/plans/threads~1context"
        return {"source": source, "folder": folder, "original": original, "capture": capture,
                "plan": plan, "review": path, "spec": spec, "write": write}

    def batch(self, root, **options):
        tool = root / "tools/reverse_pending.py"
        tool.parent.mkdir(parents=True, exist_ok=True)
        tool.write_bytes(Path(queue.__file__).read_bytes())
        with patch.object(queue, "__file__", str(tool)):
            return queue.Batch(2, "IDA", **options)

    @staticmethod
    def resolution(*, remaining=True):
        failure = {"stage": "1000:hexrays", "error": "synthetic call analysis failed"}
        return {"remaining_failures": [failure] if remaining else [], "verification_failures": [],
                "resolved_failures": [] if remaining else [{"original_failure": failure}], "complete_functions": {}}

    def save_plan(self, fixture, plan):
        fixture["write"](fixture["review"], {"plans": {"threads/context": plan}})

    def observation_fixture(self, root):
        fixture = self.fixture(root)
        observer = root / ".analysis/results/reverse-pending/tool-sources/observer.py"
        observer.parent.mkdir(parents=True)
        observer.write_bytes(b"inert read-only observer source; never executed\n")
        fixture["observer"] = observer
        fixture["plan"].update(function_rvas=["0x1000"], type_recoveries=[],
            static_observation={"reason": "Capture previously absent per-owner SP and call argument locations.",
                                "observer_source": queue.identity(observer)})
        fixture["plan"]["evidence"].append(queue.identity(observer))
        self.save_plan(fixture, fixture["plan"])
        return fixture


    def capture_selection_fixture(self, root, *, observed=True):
        """Run the real capture prefix through its manifest with fake IDA APIs."""
        from datetime import datetime, timezone
        import sys

        source = Path(queue.__file__).with_name("ida") / "supplement.py"
        runtime_tool = root / "tools/ida/supplement.py"
        runtime_tool.parent.mkdir(parents=True)
        runtime_tool.write_bytes(source.read_bytes())
        fixture = self.observation_fixture(root) if observed else self.fixture(root)
        options = {}
        if observed:
            fixture["observer"].write_bytes(runtime_tool.read_bytes())
            identity = queue.identity(fixture["observer"])
            fixture["plan"]["static_observation"]["observer_source"] = identity
            fixture["plan"]["evidence"] = [
                row for row in fixture["plan"]["evidence"] if row["path"] != identity["path"]
            ] + [identity]
            self.save_plan(fixture, fixture["plan"])
            review = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
            options["observation_evidence"] = review["observation_evidence_spec"]
        selection = analysis.native_function_selection(fixture["source"].read_bytes(), fixture["capture"],
                                                       [0x1000], **options)
        private = root / ".work/native" / fixture["original"]["sha256"] / "supplements/private.dll"
        private.parent.mkdir(parents=True)
        private.write_bytes(fixture["source"].read_bytes())
        database = private.with_suffix(".i64")
        database.write_bytes(b"inert private IDB fixture; never opened\n")
        output = fixture["folder"] / "supplements/capture-selection-fixture"
        plan_path = private.parent / "plan.json"
        plan = {"input": fixture["original"]["path"], "sha256": fixture["original"]["sha256"],
                "output": output.relative_to(root).as_posix(), "function_selection": selection,
                "base_capture": queue.identity(fixture["folder"] / "capture.json"),
                "database": queue.identity(database)}
        fixture["write"](plan_path, plan)
        sdk = SimpleNamespace(init_hexrays_plugin=MagicMock(return_value=True),
                              get_hexrays_version=MagicMock(return_value="fake-sdk"))
        namespace = {
            "__file__": str(runtime_tool), "Path": Path, "hashlib": hashlib, "json": json,
            "datetime": datetime, "timezone": timezone,
            "os": SimpleNamespace(environ={"RAZER_IDA_SUPPLEMENT_PLAN": str(plan_path)}),
            "sys": SimpleNamespace(path=list(sys.path), version=sys.version),
            "idaapi": SimpleNamespace(get_input_file_path=lambda: str(private),
                                      get_imagebase=lambda: 0x140000000),
            "ida_nalt": SimpleNamespace(
                retrieve_input_file_sha256=lambda: bytes.fromhex(fixture["original"]["sha256"])),
            "ida_auto": SimpleNamespace(auto_wait=MagicMock()),
            "ida_kernwin": SimpleNamespace(get_kernel_version=lambda: "fake-sdk"), "ida_hexrays": sdk}
        capture = next(node for node in ast.parse(source.read_text(encoding="utf-8")).body
                       if isinstance(node, ast.FunctionDef) and node.name == "capture")
        manifest_index = next(index for index, node in enumerate(capture.body)
                              if isinstance(node, ast.Assign)
                              and any(isinstance(target, ast.Name) and target.id == "manifest"
                                      for target in node.targets))
        capture.body = capture.body[:manifest_index + 1] + [
            ast.Return(value=ast.Name(id="manifest", ctx=ast.Load()))]
        module = ast.fix_missing_locations(ast.Module(body=[capture], type_ignores=[]))
        exec(compile(module, str(source), "exec"), namespace)
        unchanged = {path: path.read_bytes() for path in
                     (fixture["source"], fixture["folder"] / "capture.json")}
        return {**fixture, "selection": selection, "options": options, "runtime_tool": runtime_tool,
                "output": output, "sdk": sdk, "run_capture": namespace["capture"], "unchanged": unchanged}

    def test_capture_recomputes_frozen_observation_and_records_the_reviewed_running_tool(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.capture_selection_fixture(Path(directory))
            with patch.object(analysis, "native_function_selection",
                              wraps=analysis.native_function_selection) as recompute:
                manifest = fixture["run_capture"]()
            recompute.assert_called_once()
            self.assertEqual(recompute.call_args.kwargs, fixture["options"])
            self.assertEqual(recompute.call_args.args[2:5], ([0x1000], [], None))
            self.assertEqual(manifest["function_selection"], fixture["selection"])
            self.assertEqual(manifest["capture_tool"]["sha256"],
                             fixture["selection"]["static_observation"]["observer_source"]["sha256"])
            self.assertEqual(manifest["capture_tool"]["sha256"], analysis.sha(fixture["runtime_tool"]))
            fixture["sdk"].init_hexrays_plugin.assert_called_once_with()
            self.assertFalse(manifest["capture_complete"])
            for path, original in fixture["unchanged"].items():
                self.assertEqual(path.read_bytes(), original)

    def test_capture_rejects_unreviewed_running_observer_before_output_or_hexrays(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.capture_selection_fixture(Path(directory))
            fixture["runtime_tool"].write_bytes(fixture["runtime_tool"].read_bytes() + b"# different source\n")
            with patch.object(analysis, "native_function_selection",
                              wraps=analysis.native_function_selection) as recompute:
                with self.assertRaisesRegex(ValueError, "reviewed static observer"):
                    fixture["run_capture"]()
            recompute.assert_called_once()
            self.assertEqual(recompute.call_args.kwargs, fixture["options"])
            self.assertFalse(fixture["output"].exists())
            fixture["sdk"].init_hexrays_plugin.assert_not_called()
            for path, original in fixture["unchanged"].items():
                self.assertEqual(path.read_bytes(), original)

    def test_capture_rejects_changed_frozen_review_bytes_despite_identical_observation_value(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.capture_selection_fixture(root)
            retained = root / fixture["selection"]["static_observation"]["review_source"]["path"]
            retained.write_bytes(retained.read_bytes() + b" \n")
            with self.assertRaisesRegex(ValueError, "selection no longer matches"):
                fixture["run_capture"]()
            self.assertFalse(fixture["output"].exists())
            fixture["sdk"].init_hexrays_plugin.assert_not_called()

    def test_capture_without_observation_preserves_legacy_selection(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.capture_selection_fixture(Path(directory), observed=False)
            with patch.object(analysis, "native_function_selection",
                              wraps=analysis.native_function_selection) as recompute:
                manifest = fixture["run_capture"]()
            recompute.assert_called_once()
            self.assertEqual(recompute.call_args.kwargs, {})
            self.assertNotIn("static_observation", manifest["function_selection"])
            self.assertEqual(manifest["function_selection"], fixture["selection"])
            self.assertEqual(manifest["capture_tool"]["sha256"], analysis.sha(fixture["runtime_tool"]))

    def test_observation_without_repairs_has_a_distinct_retained_selection(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.observation_fixture(root)
            before = fixture["source"].read_bytes()
            loaded = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
            specification = loaded["observation_evidence_spec"]
            self.assertIn("/review-sources/", specification)
            self.assertTrue(specification.endswith("#/plans/threads~1context/static_observation"))
            plain = analysis.native_function_selection(before, fixture["capture"], [0x1000], max_function_kib=1024)
            observed = analysis.native_function_selection(before, fixture["capture"], [0x1000], max_function_kib=1024,
                                                         observation_evidence=specification)
            self.assertEqual({k: v for k, v in observed.items() if k != "static_observation"}, plain)
            self.assertNotEqual(hashlib.sha256(json.dumps(observed, sort_keys=True).encode()).hexdigest(),
                                hashlib.sha256(json.dumps(plain, sort_keys=True).encode()).hexdigest())
            fixture["review"].write_bytes(b"mutable canonical can change after review was frozen")
            self.assertEqual(analysis.native_observation_evidence(specification), observed["static_observation"])
            self.assertEqual(fixture["source"].read_bytes(), before)

    def test_observation_rejects_incomplete_schema_changed_source_and_unretained_evidence(self):
        for issue in ("reason", "oversized", "extra", "source", "evidence", "absolute", "suffix"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                    patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.observation_fixture(Path(directory))
                observation = fixture["plan"]["static_observation"]
                if issue == "reason": observation["reason"] = " "
                elif issue == "oversized": observation["reason"] = "x" * 16385
                elif issue == "extra": observation["patch_sp"] = True
                elif issue == "source": fixture["observer"].write_bytes(b"changed observer bytes")
                elif issue == "evidence": fixture["plan"]["evidence"].pop()
                elif issue == "absolute": observation["observer_source"]["path"] = str(fixture["observer"].resolve())
                elif issue == "suffix": observation["observer_source"]["path"] = ".analysis/results/observer.dll"
                self.save_plan(fixture, fixture["plan"])
                with self.assertRaises(ValueError):
                    queue.load_native_reviews([fixture["spec"]])

    def test_observation_forwards_frozen_review_without_adding_type_or_boundary_repairs(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.observation_fixture(root)
            batch = self.batch(root, native_only=True, review_paths=[fixture["spec"]])
            review = batch.reviews[fixture["original"]["sha256"]]
            task = {"id": "observed", "input": fixture["original"]["path"],
                    "sha256": fixture["original"]["sha256"], "review": review}
            with patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "native_reanalysis_resolution", return_value=self.resolution()), \
                 patch.object(batch, "command", return_value={"exit_code": 0}) as command:
                result = batch.native(task)
            command.assert_called_once()
            arguments = command.call_args.args[1]
            self.assertEqual(arguments[arguments.index("--observation-evidence") + 1], review["observation_evidence_spec"])
            for flag in ("--function-type", "--import-type", "--merge-functions", "--share-function-tail"):
                self.assertNotIn(flag, arguments)
            self.assertEqual(result["status"], "unresolved")

    def test_load_native_review_preserves_nested_pointer_identity_and_separate_typed_callee(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.fixture(root)
            before = {path: path.read_bytes() for path in (fixture["source"], fixture["review"],
                      fixture["folder"] / "capture.json")}
            with patch.object(analysis.subprocess, "Popen") as process:
                loaded = queue.load_native_reviews([fixture["spec"]])
            process.assert_not_called()
            self.assertEqual(set(loaded), {fixture["original"]["sha256"]})
            plan = loaded[fixture["original"]["sha256"]]
            self.assertEqual(plan["function_rvas"], ["0x1000", "0x1010"])
            self.assertEqual(plan["type_recoveries"], fixture["plan"]["type_recoveries"])
            reference = plan["review_reference"]
            self.assertEqual(reference["path"], fixture["review"].relative_to(root).as_posix())
            self.assertEqual(reference["pointer"], "/plans/threads~1context")
            self.assertEqual(reference["sha256"], hashlib.sha256(before[fixture["review"]]).hexdigest())
            self.assertEqual(reference["bytes"], len(before[fixture["review"]]))
            for path, raw in before.items():
                self.assertEqual(path.read_bytes(), raw)

    def test_native_review_retains_exact_raw_source_without_changing_reference_or_old_snapshots(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.fixture(root)
            raw = fixture["review"].read_bytes()
            digest = hashlib.sha256(raw).hexdigest()
            expected_reference = {**queue.identity(fixture["review"]), "pointer": "/plans/threads~1context"}
            first = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
            retained = root / ".analysis/results/reverse-pending/review-sources" / (digest + ".json")
            self.assertEqual(retained.read_bytes(), raw)
            self.assertEqual(first["review_reference"], expected_reference)
            second = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
            self.assertEqual(first, second)
            fixture["review"].write_bytes(raw + b" \n")
            updated = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
            self.assertNotEqual(updated["review_reference"]["sha256"], digest)
            self.assertEqual(retained.read_bytes(), raw)
            self.assertEqual(updated["review_reference"]["path"], expected_reference["path"])
            retained.write_bytes(b"changed frozen source")
            fixture["review"].write_bytes(raw)
            with self.assertRaisesRegex(ValueError, "Retained native review source bytes changed"):
                queue.load_native_reviews([fixture["spec"]])

    def test_native_partition_review_forwards_exact_ranges_without_abi_declarations(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.partition_fixture(root)
            batch = self.batch(root, native_only=True, review_paths=[fixture["spec"]])
            review = batch.reviews[fixture["original"]["sha256"]]
            self.assertEqual(review["function_partitions"], fixture["plan"]["function_partitions"])
            with patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "native_reanalysis_resolution", side_effect=[self.resolution(), self.resolution(remaining=False)]), \
                 patch.object(batch, "command", return_value={"exit_code": 0}) as command:
                result = batch.native({"id": "partition-command", "input": fixture["original"]["path"],
                    "sha256": fixture["original"]["sha256"], "review": review})
            arguments = command.call_args.args[1]
            self.assertEqual([arguments[i + 1] for i, value in enumerate(arguments) if value == "--partition-functions"],
                             ["0x1000=0x1002:0x1005"])
            self.assertNotIn("--function-type", arguments)
            self.assertNotIn("--merge-functions", arguments)
            self.assertIn(fixture["plan"]["function_partitions"][0]["assembly_evidence"], review["type_evidence"])
            self.assertEqual(result["status"], "passed")

    def test_shared_tail_review_forwards_exact_owner_range_and_mode(self):
        for inline in (True, False):
            with self.subTest(inline=inline), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.shared_tail_fixture(root)
                fixture["plan"]["function_shared_tails"][0]["inline_owners"] = inline
                self.save_plan(fixture, fixture["plan"])
                batch = self.batch(root, native_only=True, review_paths=[fixture["spec"]])
                review = batch.reviews[fixture["original"]["sha256"]]
                with patch.object(queue, "active_native_sessions", return_value=[]), \
                     patch.object(analysis, "native_reanalysis_resolution", side_effect=[self.resolution(), self.resolution(remaining=False)]), \
                     patch.object(batch, "command", return_value={"exit_code": 0}) as command:
                    result = batch.native({"id": "shared-command", "input": fixture["original"]["path"],
                        "sha256": fixture["original"]["sha256"], "review": review})
                arguments = command.call_args.args[1]
                self.assertEqual([arguments[i + 1] for i, value in enumerate(arguments) if value == "--share-function-tail"],
                    ["0x1000=0x1004:0x1007:0x1000,0x1040:" + ("inline" if inline else "retain")])
                self.assertNotIn("--function-type", arguments)
                self.assertEqual(result["status"], "passed")

    def test_shared_tail_review_rejects_bad_schema_edges_and_changed_evidence(self):
        for issue in ("null", "missing", "mode", "interior", "owner", "note", "changed"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.shared_tail_fixture(Path(directory))
                row = fixture["plan"]["function_shared_tails"][0]
                if issue == "null": fixture["plan"]["function_shared_tails"] = None
                elif issue == "missing": row.pop("tail_end_rva")
                elif issue == "mode": row["inline_owners"] = "false"
                elif issue == "interior": row["tail_start_rva"] = "0x1005"
                elif issue == "owner": row["owner_rvas"] = ["0x1000", "0x1041"]
                elif issue == "note": row["assembly_evidence"] = " "
                else: (fixture["folder"] / "functions/1040.json").write_bytes(b"changed retained owner")
                self.save_plan(fixture, fixture["plan"])
                with patch.object(analysis.subprocess, "Popen") as process, self.assertRaises(ValueError):
                    queue.load_native_reviews([fixture["spec"]])
                process.assert_not_called()

    def test_completed_shared_tail_attempt_reuses_exact_selection_and_rejects_changed_owner(self):
        for changed in (False, True):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.shared_tail_fixture(root)
                review = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
                selection = analysis.native_function_selection(fixture["source"].read_bytes(), fixture["capture"], [0x1000],
                    type_note=review["type_evidence"], max_function_kib=1024,
                    shared_tail_specs=queue.native_shared_tail_specifications(review["function_shared_tails"]))
                digest = hashlib.sha256(json.dumps(selection, sort_keys=True).encode()).hexdigest()[:16]
                output = fixture["folder"] / "supplements" / digest
                database = fixture["write"](root / ".work/native/source.i64", {"inert": "never opened"})
                private = {"path": ".work/native/private.i64", "sha256": database["sha256"]}
                plan = {"input": fixture["original"]["path"], "sha256": fixture["original"]["sha256"],
                    "function_selection": deepcopy(selection), "base_capture": {
                        "path": review["source_capture"]["path"], "sha256": review["source_capture"]["sha256"]},
                    "database": private, "source_database": database}
                if changed: plan["function_selection"]["function_shared_tails"][0]["primary_owner_rva"] = "0x1040"
                fixture["write"](output / "plan.json", plan)
                fixture["write"](output / "session.json", {"exit_code": 1, "finished_at_utc": "actual failed end", "database": private})
                batch = self.batch(root, resume=True)
                if changed:
                    with self.assertRaises(ValueError): batch.completed_review_attempt(fixture["source"], fixture["capture"], review)
                else:
                    record = batch.completed_review_attempt(fixture["source"], fixture["capture"], review)
                    self.assertEqual(record["exit_code"], 1)
                    self.assertEqual(record["plan"], queue.identity(output / "plan.json"))

    def test_native_partition_review_rejects_incomplete_ranges_owner_and_changed_evidence(self):
        for issue in ("null", "empty_ranges", "missing_end", "wrong_owner", "interior", "overlap", "empty_note", "changed_evidence"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.partition_fixture(root)
                plan = deepcopy(fixture["plan"])
                item = plan["function_partitions"][0]
                if issue == "null": plan["function_partitions"] = None
                elif issue == "empty_ranges": item["helper_ranges"] = []
                elif issue == "missing_end": item["helper_ranges"][0].pop("end_rva_exclusive")
                elif issue == "wrong_owner": item["owner_rva"] = "0x1001"
                elif issue == "interior": item["helper_ranges"][0]["start_rva"] = "0x1003"
                elif issue == "overlap": item["helper_ranges"] *= 2
                elif issue == "empty_note": item["assembly_evidence"] = " \t"
                else: (fixture["folder"] / "functions/1000.json").write_bytes(b"changed retained code")
                self.save_plan(fixture, plan)
                with self.assertRaises(ValueError), patch.object(analysis.subprocess, "Popen") as process:
                    queue.load_native_reviews([fixture["spec"]])
                process.assert_not_called()

    def test_completed_partition_attempt_uses_exact_partition_selection_and_rejects_changed_ranges(self):
        for changed in (False, True):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.partition_fixture(root)
                review = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
                selection = analysis.native_function_selection(fixture["source"].read_bytes(), fixture["capture"], [0x1000],
                    type_note=review["type_evidence"], max_function_kib=1024, partition_specs=["0x1000=0x1002:0x1005"])
                digest = hashlib.sha256(json.dumps(selection, sort_keys=True).encode()).hexdigest()[:16]
                output = fixture["folder"] / "supplements" / digest
                database = fixture["write"](root / ".work/native/source.i64", {"inert": "never opened"})
                private = {"path": ".work/native/private.i64", "sha256": database["sha256"]}
                plan = {"input": fixture["original"]["path"], "sha256": fixture["original"]["sha256"],
                    "function_selection": deepcopy(selection), "base_capture": {
                        "path": review["source_capture"]["path"], "sha256": review["source_capture"]["sha256"]},
                    "database": private, "source_database": database}
                if changed: plan["function_selection"]["function_partitions"][0]["helpers"][0]["expected_end"] = "0x140001004"
                fixture["write"](output / "plan.json", plan)
                fixture["write"](output / "session.json", {"exit_code": 1, "finished_at_utc": "actual failed end", "database": private})
                batch = self.batch(root, resume=True)
                if changed:
                    with self.assertRaises(ValueError): batch.completed_review_attempt(fixture["source"], fixture["capture"], review)
                else:
                    record = batch.completed_review_attempt(fixture["source"], fixture["capture"], review)
                    self.assertEqual(record["exit_code"], 1)
                    self.assertEqual(record["plan"], queue.identity(output / "plan.json"))

    def test_native_review_rejects_changed_original_capture_and_evidence_bytes(self):
        for subject in ("original", "capture", "evidence"):
            with self.subTest(subject=subject), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.fixture(root)
                target = {"original": fixture["source"], "capture": fixture["folder"] / "capture.json",
                          "evidence": root / fixture["plan"]["evidence"][1]["path"]}[subject]
                changed = bytearray(target.read_bytes())
                changed[-1] ^= 1
                target.write_bytes(changed)
                with patch.object(analysis.subprocess, "Popen") as process, self.assertRaises(ValueError):
                    queue.load_native_reviews([fixture["spec"]])
                process.assert_not_called()

    def test_native_review_rejects_incomplete_capture_despite_matching_identity(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            fixture = self.fixture(Path(directory))
            fixture["capture"]["capture_complete"] = False
            plan = deepcopy(fixture["plan"])
            plan["source_capture"] = fixture["write"](fixture["folder"] / "capture.json", fixture["capture"])
            self.save_plan(fixture, plan)
            with self.assertRaises(ValueError):
                queue.load_native_reviews([fixture["spec"]])

    def test_native_review_rejects_duplicate_module_target_and_type_selection(self):
        for issue in ("module", "target", "normalized_target", "type"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory))
                plan = deepcopy(fixture["plan"])
                specs = [fixture["spec"]]
                if issue == "module":
                    specs.append(fixture["spec"])
                elif issue == "target":
                    plan["function_rvas"].append("0x1000")
                elif issue == "normalized_target":
                    plan["function_rvas"].append("0X1000")
                else:
                    plan["type_recoveries"].append(deepcopy(plan["type_recoveries"][0]))
                self.save_plan(fixture, plan)
                with self.assertRaises(ValueError):
                    queue.load_native_reviews(specs)

    def test_native_review_rejects_nonboundary_targets_and_unselected_types(self):
        for issue in ("interior", "data", "unselected_type"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory))
                plan = deepcopy(fixture["plan"])
                if issue == "interior":
                    plan["function_rvas"].append("0x1001")
                elif issue == "data":
                    plan["function_rvas"].append("0x2000")
                else:
                    plan["function_rvas"] = ["0x1000"]
                self.save_plan(fixture, plan)
                with self.assertRaises(ValueError):
                    queue.load_native_reviews([fixture["spec"]])

    def test_native_review_rejects_empty_or_missing_type_and_identity_evidence(self):
        for issue in ("empty_note", "whitespace_note", "missing_note", "empty_declaration",
                      "empty_evidence", "missing_evidence_bytes", "missing_evidence_sha"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory))
                plan = deepcopy(fixture["plan"])
                item = plan["type_recoveries"][0]
                if issue == "empty_note":
                    item["assembly_evidence"] = ""
                elif issue == "whitespace_note":
                    item["assembly_evidence"] = " \t\n"
                elif issue == "missing_note":
                    item.pop("assembly_evidence")
                elif issue == "empty_declaration":
                    item["declaration"] = " \t"
                elif issue == "empty_evidence":
                    plan["evidence"] = []
                elif issue == "missing_evidence_bytes":
                    plan["evidence"][0].pop("bytes")
                else:
                    plan["evidence"][0].pop("sha256")
                self.save_plan(fixture, plan)
                with self.assertRaises(ValueError):
                    queue.load_native_reviews([fixture["spec"]])

    def test_native_review_rejects_outside_paths_and_unresolvable_json_pointers(self):
        for issue in ("outside_review", "absolute_review", "escape_review", "outside_original",
                      "outside_capture", "outside_evidence", "missing_pointer", "bad_pointer"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.fixture(root)
                plan = deepcopy(fixture["plan"])
                spec = fixture["spec"]
                if issue == "outside_review":
                    other = root / "tools/review.json"
                    fixture["write"](other, {"plan": plan})
                    spec = "tools/review.json#/plan"
                elif issue == "absolute_review":
                    spec = str(fixture["review"]) + "#/plans/threads~1context"
                elif issue == "escape_review":
                    spec = ".analysis/../tools/review.json#/plan"
                elif issue == "outside_original":
                    plan["source_input"]["path"] = "tools/original.dll"
                elif issue == "outside_capture":
                    plan["source_capture"]["path"] = "tools/capture.json"
                elif issue == "outside_evidence":
                    plan["evidence"][0]["path"] = "tools/evidence.json"
                elif issue == "missing_pointer":
                    spec = fixture["spec"].split("#")[0] + "#/plans/unknown"
                else:
                    spec = fixture["spec"].split("#")[0] + "#plans/threads"
                self.save_plan(fixture, plan)
                with self.assertRaises(ValueError):
                    queue.load_native_reviews([spec])

    def test_native_only_discovery_excludes_frontend_and_resolved_original_failures(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            unresolved = self.fixture(root, 0)
            resolved = self.fixture(root, 1)
            frontend = root / ".analysis/inputs/frontend/fixture/acquisition.json"
            frontend.parent.mkdir(parents=True)
            frontend.write_bytes(b"inert frontend fixture; must not be processed")
            verified_path = root / ".analysis/results/reverse-pending/verified-native" / (resolved["original"]["sha256"] + ".json")
            resolved["write"](verified_path, {**self.resolution(remaining=False),
                "verified_base_capture": queue.identity(resolved["folder"] / "capture.json")})
            batch = self.batch(root, native_only=True)
            with patch.object(batch, "publish"), patch.object(batch, "frontend") as frontend_call, \
                 patch.object(batch, "command") as command:
                tasks = batch.discover()
            self.assertEqual(len(tasks), 1)
            self.assertEqual(tasks[0]["kind"], "native")
            self.assertEqual(tasks[0]["sha256"], unresolved["original"]["sha256"])
            self.assertNotEqual(tasks[0]["sha256"], resolved["original"]["sha256"])
            frontend_call.assert_not_called()
            command.assert_not_called()

    def test_reviewed_callee_type_is_added_to_failed_wrapper_supplement_command(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.fixture(root)
            original_before = fixture["source"].read_bytes()
            capture_before = (fixture["folder"] / "capture.json").read_bytes()
            batch = self.batch(root, native_only=True, review_paths=[fixture["spec"]])
            task = {"id": "reviewed", "input": fixture["original"]["path"],
                    "sha256": fixture["original"]["sha256"],
                    "review": batch.reviews[fixture["original"]["sha256"]]}
            with patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "native_reanalysis_resolution", side_effect=[self.resolution(), self.resolution(remaining=False)]), \
                 patch.object(batch, "command", return_value={"exit_code": 0}) as command:
                result = batch.native(task)
            command.assert_called_once()
            arguments = command.call_args.args[1]
            self.assertEqual(arguments[0], "supplement-native")
            self.assertIn("--recover-interrupted", arguments)
            self.assertIn("--no-refresh-gaps", arguments)
            selected = [arguments[index + 1] for index, value in enumerate(arguments) if value == "--function-rva"]
            self.assertEqual(selected, ["0x1000", "0x1010"])
            declarations = [arguments[index + 1] for index, value in enumerate(arguments) if value == "--function-type"]
            self.assertEqual(declarations, ["0x1010=" + fixture["plan"]["type_recoveries"][0]["declaration"]])
            note = arguments[arguments.index("--type-evidence") + 1]
            self.assertIn(fixture["plan"]["type_recoveries"][0]["assembly_evidence"], note)
            self.assertEqual(result["status"], "passed")
            self.assertEqual(result["remaining"], 0)
            self.assertEqual(fixture["source"].read_bytes(), original_before)
            self.assertEqual((fixture["folder"] / "capture.json").read_bytes(), capture_before)

    def test_reviewed_types_never_launch_commands_in_verification_only_mode(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.fixture(root)
            batch = self.batch(root, verify_only=True, native_only=True, review_paths=[fixture["spec"]])
            with patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "native_reanalysis_resolution", return_value=self.resolution()), \
                 patch.object(batch, "command") as command:
                result = batch.native({"id": "read-only", "input": fixture["original"]["path"],
                                       "sha256": fixture["original"]["sha256"],
                                       "review": batch.reviews[fixture["original"]["sha256"]]})
            command.assert_not_called()
            self.assertEqual(result["status"], "unresolved")

    def test_review_identity_is_rechecked_after_loading_before_any_command(self):
        for subject in ("review", "original", "capture", "evidence"):
            with self.subTest(subject=subject), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.fixture(root)
                batch = self.batch(root, native_only=True, review_paths=[fixture["spec"]])
                task = {"id": "changed-plan", "input": fixture["original"]["path"],
                        "sha256": fixture["original"]["sha256"],
                        "review": batch.reviews[fixture["original"]["sha256"]]}
                target = {"review": fixture["review"], "original": fixture["source"],
                          "capture": fixture["folder"] / "capture.json",
                          "evidence": root / fixture["plan"]["evidence"][1]["path"]}[subject]
                target.write_bytes(target.read_bytes() + b" ")
                with patch.object(queue, "active_native_sessions", return_value=[]), \
                     patch.object(analysis, "native_reanalysis_resolution", return_value=self.resolution()), \
                     patch.object(batch, "command") as command, self.assertRaises(ValueError):
                    batch.native(task)
                command.assert_not_called()

    def test_review_for_another_module_is_rejected_before_any_command(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            reviewed = self.fixture(root, 0)
            other = self.fixture(root, 1)
            batch = self.batch(root, native_only=True, review_paths=[reviewed["spec"]])
            task = {"id": "wrong-module", "input": other["original"]["path"],
                    "sha256": other["original"]["sha256"],
                    "review": batch.reviews[reviewed["original"]["sha256"]]}
            with patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "native_reanalysis_resolution", return_value=self.resolution()), \
                 patch.object(batch, "command") as command, self.assertRaises(ValueError):
                batch.native(task)
            command.assert_not_called()

    def test_native_only_unreviewed_failure_is_reported_without_unchanged_retry(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.fixture(root)
            batch = self.batch(root, native_only=True)
            with patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "native_reanalysis_resolution", return_value=self.resolution()), \
                 patch.object(batch, "command") as command:
                result = batch.native({"id": "no-new-evidence", "input": fixture["original"]["path"],
                                       "sha256": fixture["original"]["sha256"]})
            command.assert_not_called()
            self.assertEqual(result["status"], "unresolved")
            saved = json.loads((root / result["resolution"]["path"]).read_bytes())
            self.assertTrue(saved["unchanged_retry_skipped"])
            self.assertFalse(saved["semantics_complete"])

    def test_stale_or_conflicted_verification_receipt_does_not_hide_remaining_native_work(self):
        for issue in ("stale_capture", "verification_conflict"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                fixture = self.fixture(root)
                verified = {**self.resolution(remaining=False),
                            "verified_base_capture": queue.identity(fixture["folder"] / "capture.json")}
                if issue == "stale_capture":
                    verified["verified_base_capture"]["sha256"] = "0" * 64
                else:
                    verified["verification_failures"] = [{"error": "synthetic supplemental conflict"}]
                fixture["write"](root / ".analysis/results/reverse-pending/verified-native" /
                                 (fixture["original"]["sha256"] + ".json"), verified)
                batch = self.batch(root, native_only=True)
                with patch.object(batch, "publish"):
                    tasks = batch.discover()
                self.assertEqual(len(tasks), 1)
                self.assertEqual(tasks[0]["sha256"], fixture["original"]["sha256"])

    def test_import_only_review_constructs_named_iat_type_arguments_without_function_types(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            fixture = self.fixture(root, imports=True)
            plan = deepcopy(fixture["plan"])
            plan["type_recoveries"] = []
            declaration = "unsigned int (__stdcall *Observed)(unsigned int value);"
            note = "Synthetic PE64: descriptor 2040, IAT slot 2090 contains 20b0, named FIXTURE.dll!Observed."
            plan["import_type_recoveries"] = [{"rva": "0x2090", "declaration": declaration,
                                               "assembly_evidence": note}]
            self.save_plan(fixture, plan)
            batch = self.batch(root, native_only=True, review_paths=[fixture["spec"]])
            task = {"id": "reviewed-import", "input": fixture["original"]["path"],
                    "sha256": fixture["original"]["sha256"],
                    "review": batch.reviews[fixture["original"]["sha256"]]}
            with patch.object(queue, "active_native_sessions", return_value=[]), \
                 patch.object(analysis, "native_reanalysis_resolution", side_effect=[self.resolution(), self.resolution(remaining=False)]), \
                 patch.object(batch, "command", return_value={"exit_code": 0}) as command:
                result = batch.native(task)
            command.assert_called_once()
            arguments = command.call_args.args[1]
            imports = [arguments[index + 1] for index, value in enumerate(arguments) if value == "--import-type"]
            self.assertEqual(imports, ["0x2090=" + declaration])
            self.assertNotIn("--function-type", arguments)
            self.assertIn(note, arguments[arguments.index("--type-evidence") + 1])
            self.assertEqual(result["status"], "passed")

    def test_import_review_rejects_duplicate_or_unnamed_slots_and_empty_evidence(self):
        for issue in ("duplicate", "non_iat", "empty_note"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                fixture = self.fixture(Path(directory), imports=True)
                plan = deepcopy(fixture["plan"])
                row = {"rva": "0x2090", "declaration": "unsigned int (__stdcall *Observed)(unsigned int value);",
                       "assembly_evidence": "Synthetic named import slot 2090."}
                plan["import_type_recoveries"] = [row]
                if issue == "duplicate":
                    plan["import_type_recoveries"].append(deepcopy(row))
                elif issue == "non_iat":
                    row["rva"] = "0x2000"
                else:
                    row["assembly_evidence"] = " "
                self.save_plan(fixture, plan)
                with self.assertRaises(ValueError):
                    queue.load_native_reviews([fixture["spec"]])

    def test_command_wrapper_forwards_native_scope_and_explicit_review_specs(self):
        specs = [".analysis/results/fixture-review.json#/plans/threads"]
        args = argparse.Namespace(workers=2, ida_root="IDA", verify_only=False,
                                  native_only=True, native_review=specs)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), \
             patch.object(queue, "Batch") as batch:
            report = {"finished_at_utc": "synthetic queue completion", "tasks": []}
            batch.return_value.run.return_value = report
            self.assertIs(queue.run(args), report)
        batch.assert_called_once_with(2, "IDA", False, native_only=True, review_paths=specs)


class QueueResumeTests(unittest.TestCase):
    def fixture(self, root):
        helper = NativeReviewQueueTests()
        fixtures = [helper.fixture(root, index) for index in range(2)]
        reviews = queue.load_native_reviews([fixture["spec"] for fixture in fixtures])
        workflow = b"inert retained previous workflow; never executed\x00\xff"
        digest = hashlib.sha256(workflow).hexdigest()
        fixtures[0]["write"](root / ".analysis/results/reverse-pending/tool-sources" / (digest + ".py"), workflow)
        tasks = []
        for fixture in fixtures:
            task = {"kind": "native", "input": fixture["original"]["path"],
                    "sha256": fixture["original"]["sha256"],
                    "base_capture": queue.identity(fixture["folder"] / "capture.json"),
                    "review": reviews[fixture["original"]["sha256"]]}
            task["id"] = queue.task_id("native", task, {"workflow": digest, "ida_root": "IDA", "max_function_kib": 1024})
            task["status"] = "pending"
            tasks.append(task)
        extra = fixtures[0]["write"](root / ".analysis/results/retained-extra.json", {"inert": "retained artifact"})
        resolution = {**helper.resolution(), "input": fixtures[0]["original"],
                      "verified_base_capture": tasks[0]["base_capture"], "extra_artifact": extra}
        result = fixtures[0]["write"](root / ".analysis/results/reverse-pending/reviewed-native/result.json", resolution)
        tasks[0].update({"status": "unresolved", "finished_at_utc": "retained task completion",
                         "resolution": result, "resolved": 0, "remaining": 1, "verification_errors": 0})
        previous = {"schema_version": 1, "started_at_utc": "original queue start", "workers": 2,
                    "tool": {"path": "tools/reverse_pending.py", "bytes": len(workflow), "sha256": digest},
                    "tasks": tasks, "native_only": True, "verification_only": False,
                    "native_reviews": [review["review_reference"] for review in reviews.values()],
                    "native_scope": {task["sha256"]: {"resolved": 0, "remaining": 1, "verification_errors": 0} for task in tasks},
                    "scratch_cleanup": {"deleted_files": 7}, "summary": {"tasks": 2}}
        report = root / queue.REPORT
        raw = json.dumps(previous, ensure_ascii=False, indent=2).encode() + b"\r\n"
        report.write_bytes(raw)
        document = root / queue.DOC
        document.parent.mkdir(parents=True)
        document.write_text("Retained scope.\n", encoding="utf-8")
        return helper.batch(root, resume=True), fixtures, previous, raw

    def test_resume_executes_only_pending_task_and_preserves_exact_original_receipt(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), \
             redirect_stdout(io.StringIO()):
            root = Path(directory)
            batch, _, previous, raw = self.fixture(root)
            completed = deepcopy(previous["tasks"][0])
            with patch.object(batch, "discover") as discover, \
                 patch.object(batch, "execute", return_value={"status": "unresolved", "resolved": 0,
                     "remaining": 1, "verification_errors": 0}) as execute:
                result = batch.run()
            discover.assert_not_called()
            self.assertEqual(execute.call_count, 1)
            self.assertEqual(execute.call_args.args[0]["id"], previous["tasks"][1]["id"])
            self.assertEqual(result["tasks"][0], completed)
            self.assertEqual(result["started_at_utc"], previous["started_at_utc"])
            self.assertEqual(result["scratch_cleanup"], previous["scratch_cleanup"])
            retained = result["interrupted_static_run"]
            self.assertEqual(base64.b64decode(retained["raw_base64"]), raw)
            self.assertEqual(retained["sha256"], hashlib.sha256(raw).hexdigest())
            self.assertIsNone(retained["exit_code"])
            self.assertIsNone(retained["coordinator_process_confirmed_absent"])
            self.assertEqual(retained["completed_task_ids"], [completed["id"]])
            self.assertEqual(result["summary"]["by_status"], {"unresolved": 2})
            self.assertTrue(result["summary"]["queue_finished"])
            self.assertEqual(result["queue_session"]["finished_at_utc"], result["finished_at_utc"])
            document = (root / queue.DOC).read_text(encoding="utf-8")
            self.assertIn("--resume", document)
            self.assertNotIn("--native-review", document)

    def test_resume_rejects_changed_frozen_inputs_and_results_without_publishing(self):
        for subject in ("original", "capture", "review", "evidence", "result", "nested_artifact", "workflow"):
            with self.subTest(subject=subject), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                batch, fixtures, previous, raw = self.fixture(root)
                targets = {"original": fixtures[1]["source"], "capture": fixtures[1]["folder"] / "capture.json",
                           "review": fixtures[1]["review"], "evidence": root / fixtures[1]["plan"]["evidence"][0]["path"],
                           "result": root / previous["tasks"][0]["resolution"]["path"],
                           "nested_artifact": root / ".analysis/results/retained-extra.json",
                           "workflow": root / ".analysis/results/reverse-pending/tool-sources" / (previous["tool"]["sha256"] + ".py")}
                target = targets[subject]
                target.write_bytes(target.read_bytes() + b" ")
                with patch.object(batch, "execute") as execute, patch.object(batch, "command") as command, \
                     self.assertRaises(ValueError):
                    batch.run()
                execute.assert_not_called()
                command.assert_not_called()
                self.assertEqual((root / queue.REPORT).read_bytes(), raw)

    def test_resume_rejects_task_identity_scope_and_completion_tampering(self):
        for issue in ("id", "ida_root", "finished", "pending_completion", "missing_completion"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                batch, _, previous, _ = self.fixture(root)
                if issue == "id":
                    previous["tasks"][1]["id"] = "0" * 24
                elif issue == "ida_root":
                    batch.ida_root = "another-static-analyzer"
                elif issue == "finished":
                    previous["finished_at_utc"] = "already finished queue"
                elif issue == "pending_completion":
                    previous["tasks"][1]["finished_at_utc"] = "contradictory completion"
                else:
                    previous["tasks"][0].pop("finished_at_utc")
                raw = json.dumps(previous).encode()
                (root / queue.REPORT).write_bytes(raw)
                with patch.object(batch, "execute") as execute, self.assertRaises(ValueError):
                    batch.run()
                execute.assert_not_called()
                self.assertEqual((root / queue.REPORT).read_bytes(), raw)

    def test_resume_never_restarts_live_or_unqueryable_coordinator(self):
        for state in (True, None):
            with self.subTest(state=state), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                batch, _, previous, _ = self.fixture(root)
                previous["queue_session"] = {"pid": 712}
                raw = json.dumps(previous).encode()
                (root / queue.REPORT).write_bytes(raw)
                with patch.object(queue, "process_alive", return_value=state), \
                     patch.object(batch, "execute") as execute, self.assertRaisesRegex(ValueError, "still running"):
                    batch.run()
                execute.assert_not_called()
                self.assertEqual((root / queue.REPORT).read_bytes(), raw)

    def test_resume_inherits_scope_instead_of_accepting_new_scope_options(self):
        for options in ({"native_only": True}, {"verify_only": True}, {"review_paths": ["new-plan"]}):
            with self.subTest(options=options), self.assertRaisesRegex(ValueError, "inherits"):
                queue.Batch(2, "IDA", resume=True, **options)

    def test_pending_task_adopts_exact_finished_supplement_without_repeating_ida(self):
        for finished in (True, False):
            with self.subTest(finished=finished), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                helper = NativeReviewQueueTests()
                fixture = helper.fixture(root)
                review = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
                selection = analysis.native_function_selection(fixture["source"].read_bytes(), fixture["capture"],
                    [0x1000, 0x1010], [row["rva"] + "=" + row["declaration"] for row in review["type_recoveries"]],
                    review["type_evidence"], 1024)
                selection_id = hashlib.sha256(json.dumps(selection, sort_keys=True).encode()).hexdigest()[:16]
                output = fixture["folder"] / "supplements" / selection_id
                database = fixture["write"](root / ".work/native/retained.i64", b"inert database fixture; never opened")
                private = {"path": ".work/native/private.i64", "sha256": database["sha256"]}
                fixture["write"](output / "plan.json", {"input": fixture["original"]["path"],
                    "sha256": fixture["original"]["sha256"], "function_selection": selection,
                    "base_capture": {"path": review["source_capture"]["path"], "sha256": review["source_capture"]["sha256"]},
                    "database": private, "source_database": database})
                fixture["write"](output / "session.json", {"pid": 712, "exit_code": 1,
                    "finished_at_utc": "actual failed attempt end" if finished else None, "database": private})
                batch = helper.batch(root, resume=True)
                with patch.object(queue, "active_native_sessions", return_value=[]), \
                     patch.object(analysis, "native_reanalysis_resolution", return_value=helper.resolution()), \
                     patch.object(batch, "command", return_value={"exit_code": 1}) as command:
                    result = batch.native({"id": "pending-task", "input": fixture["original"]["path"],
                        "sha256": fixture["original"]["sha256"], "review": review})
                self.assertEqual(command.call_count, 0 if finished else 1)
                self.assertEqual(result["status"], "unresolved")
                self.assertEqual(result["remaining"], 1)
                if finished:
                    self.assertEqual(result["completed_attempt_reused"]["exit_code"], 1)
                    self.assertEqual(result["commands"], [])
                else:
                    self.assertIn("--recover-interrupted", command.call_args.args[1])

    def test_command_wrapper_passes_explicit_resume(self):
        args = argparse.Namespace(workers=2, ida_root="IDA", verify_only=False, resume=True)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), \
             patch.object(queue, "Batch") as batch:
            batch.return_value.run.return_value = {"tasks": []}
            queue.run(args)
        batch.assert_called_once_with(2, "IDA", False, resume=True)

    def test_finished_supplement_adoption_rejects_changed_plan_session_or_source_database(self):
        for subject in ("selection", "base_capture", "session_database", "source_database"):
            with self.subTest(subject=subject), tempfile.TemporaryDirectory() as directory, \
                 patch.object(analysis, "ROOT", Path(directory)):
                root = Path(directory)
                helper = NativeReviewQueueTests()
                fixture = helper.fixture(root)
                review = queue.load_native_reviews([fixture["spec"]])[fixture["original"]["sha256"]]
                selection = analysis.native_function_selection(fixture["source"].read_bytes(), fixture["capture"],
                    [0x1000, 0x1010], [row["rva"] + "=" + row["declaration"] for row in review["type_recoveries"]],
                    review["type_evidence"], 1024)
                selection_id = hashlib.sha256(json.dumps(selection, sort_keys=True).encode()).hexdigest()[:16]
                output = fixture["folder"] / "supplements" / selection_id
                database = fixture["write"](root / ".work/native/retained.i64", b"inert source database")
                private = {"path": ".work/native/private.i64", "sha256": database["sha256"]}
                plan = {"input": fixture["original"]["path"], "sha256": fixture["original"]["sha256"],
                        "function_selection": selection, "base_capture": {
                            "path": review["source_capture"]["path"], "sha256": review["source_capture"]["sha256"]},
                        "database": private, "source_database": database}
                session = {"exit_code": 0, "finished_at_utc": "ended attempt", "database": deepcopy(private)}
                if subject == "selection":
                    plan["function_selection"] = {"targets": []}
                elif subject == "base_capture":
                    plan["base_capture"]["sha256"] = "0" * 64
                elif subject == "session_database":
                    session["database"]["sha256"] = "0" * 64
                else:
                    (root / database["path"]).write_bytes(b"changed inert database")
                fixture["write"](output / "plan.json", plan)
                fixture["write"](output / "session.json", session)
                batch = helper.batch(root, resume=True)
                with patch.object(batch, "command") as command, self.assertRaises(ValueError):
                    batch.completed_review_attempt(fixture["source"], fixture["capture"], review)
                command.assert_not_called()


class SharedTailDatabaseTests(unittest.TestCase):
    def test_helper_export_reuses_its_actual_path_and_cannot_masquerade_as_pseudocode(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "output"
            output.mkdir()
            helper = output / "shared-tail-helpers/1000.json"
            helper.parent.mkdir()
            helper.write_text('{"rva":"0x1000","pseudocode":null}', encoding="utf-8")
            source = Path(queue.__file__).with_name("ida") / "supplement.py"
            capture = next(node for node in ast.parse(source.read_text(encoding="utf-8")).body
                           if isinstance(node, ast.FunctionDef) and node.name == "capture")
            export = next(node for node in capture.body if isinstance(node, ast.FunctionDef) and node.name == "export")
            namespace = {"base": 0x140000000, "output": output, "functions": output / "functions", "table": None,
                         "captured": {"0x1000": (helper, True)}, "json": json}
            exec(compile(ast.Module(body=[export], type_ignores=[]), str(source), "exec"), namespace)
            self.assertEqual(namespace["export"](0x140001000, assembly_only=True, helper_kind="shared_tail_helpers"),
                             {"rva": "0x1000", "pseudocode": None})
            with self.assertRaisesRegex(ValueError, "assembly-only helper"):
                namespace["export"](0x140001000)

    def apply_fixture(self, root, issue=None, inline=True):
        from test_evidence import NativeSupplementTests

        raw, _, capture, bodies, _ = NativeSupplementTests().shared_tail_fixture(root)
        shared = analysis.native_function_selection(raw, capture, [0x1000], type_note="Reviewed complete suffix",
            shared_tail_specs=["0x1000=0x1004:0x1007:0x1000,0x1040:" + ("inline" if inline else "retain")])["function_shared_tails"][0]
        base, functions, ranges, parents, mutations = 0x140000000, {}, {}, set(), []
        for _, body in bodies:
            ea = int(body["address"], 16)
            functions[ea] = SimpleNamespace(start_ea=ea, end_ea=int(body["end"], 16), flags=body["flags"],
                                            frame=ea + 0x100000, frsize=32, frregs=8, argsize=0, fpd=0,
                                            points=[SimpleNamespace(ea=ea, spd=0)])
            ranges[ea] = [(int(c["start"], 16), int(c["end"], 16)) for c in body["chunks"]]
        frames = {ea: f.frame for ea, f in functions.items()}
        tail = SimpleNamespace(start_ea=base + 0x1004, end_ea=base + 0x1007, flags=2, owner=base + 0x1000, refqty=0)

        def original(rva, size):
            offset = rva - 0x1000 + 0x200
            return raw[offset:offset + size]

        def read(ea, size):
            code = original(ea - base, size)
            return b"\x90" * size if issue == "bytes" and ea == base + 0x1040 else code

        def shrink(ea, stop):
            mutations.append(("shrink", ea, stop))
            old = functions[ea].end_ea
            functions[ea].end_ea = stop
            ranges[ea] = [(lo, stop if hi == old else hi) for lo, hi in ranges[ea]]
            return True

        def append(function, start, stop):
            mutations.append(("append", function.start_ea, start, stop))
            if issue == "append": return False
            ranges[function.start_ea].append((start, stop))
            parents.add(function.start_ea)
            tail.refqty = len(parents)
            return True

        def set_owner(chunk, owner):
            chunk.owner = owner
            return True

        def wait():
            if issue == "parent": parents.add(base + 0x1080)
            if issue == "omitted": ranges[base + 0x1040].pop()
            if issue == "frame": functions[base + 0x1000].frsize += 8
            if issue == "stack": functions[base + 0x1000].points[0].spd = 8

        api = SimpleNamespace(FUNC_TAIL=2, FUNC_OUTLINE=8, get_func=lambda ea: functions.get(ea),
            set_func_end=shrink, append_func_tail=append, get_fchunk=lambda ea: tail,
            set_tail_owner=set_owner, func_parent_iterator_t=lambda chunk: iter(sorted(parents)), update_func=lambda f: True)
        if issue == "flags": functions[base + 0x1040].flags = 0x400
        source = Path(queue.__file__).with_name("ida") / "supplement.py"
        node = next(node for node in ast.parse(source.read_text(encoding="utf-8")).body
                    if isinstance(node, ast.FunctionDef) and node.name == "apply_shared_function_tail")
        namespace = {"hashlib": hashlib, "ida_funcs": api, "ida_auto": SimpleNamespace(auto_wait=wait),
                     "idc": SimpleNamespace(get_type=lambda ea: None),
                     "ida_typeinf": SimpleNamespace(tinfo_t=lambda: object()),
                     "ida_nalt": SimpleNamespace(get_tinfo=lambda typ, ea: False),
                     "ida_bytes": SimpleNamespace(get_bytes=read), "idautils": SimpleNamespace(Chunks=lambda ea: iter(ranges[ea]))}
        exec(compile(ast.Module(body=[node], type_ignores=[]), str(source), "exec"), namespace)
        return namespace["apply_shared_function_tail"], shared, base, original, functions, frames, mutations

    def test_shared_tail_database_application_keeps_frames_entries_full_bytes_and_exact_parents(self):
        for inline in (True, False):
            with self.subTest(inline=inline), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                apply, shared, base, original, functions, frames, _ = self.apply_fixture(Path(directory), inline=inline)
                result = apply(shared, base, original)
                self.assertEqual(set(functions), {base + 0x1000, base + 0x1040})
                self.assertEqual(result["applied_primary_owner_rva"], "0x1000")
                self.assertEqual(result["applied_owner_rvas"], ["0x1000", "0x1040"])
                self.assertEqual(result["applied_tail"]["raw_hex"], "31c0c3")
                self.assertEqual([row["end"] for row in result["applied_owners"]], ["0x140001004", "0x140001045"])
                self.assertEqual([row["applied_flags"] for row in result["applied_owners"]], [8, 8] if inline else [0, 0])
                for ea, frame in frames.items(): self.assertEqual(functions[ea].frame, frame)

    def test_shared_tail_database_rejects_changed_input_omitted_chunks_and_foreign_parents(self):
        for issue in ("bytes", "flags", "append", "parent", "omitted", "frame", "stack"):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
                apply, shared, base, original, _, _, mutations = self.apply_fixture(Path(directory), issue=issue)
                with self.assertRaises(ValueError): apply(shared, base, original)
                if issue in {"bytes", "flags"}: self.assertEqual(mutations, [])


class StaticMicrocodeObservationTests(unittest.TestCase):
    def fixture(self, *, hook_issue=None, unhook_issue=None, constructor_issue=False):
        source = Path(queue.__file__).with_name("ida") / "supplement.py"
        registered, instances, lifecycle, borrowed, visitors, decompiles = [], [], [], [], [], []

        class Borrowed:
            def __init__(self, **values):
                self.values, self.valid, self.expired_reads = values, True, []
                borrowed.append(self)

            def __setattr__(self, name, value):
                if name not in {"values", "valid", "expired_reads"}:
                    raise AssertionError("attempted SDK mutation: " + name)
                object.__setattr__(self, name, value)

            def __getattr__(self, name):
                if not self.valid:
                    self.expired_reads.append(name)
                    raise AssertionError("expired SDK proxy: " + name)
                if name not in self.values:
                    raise AttributeError(name)
                value = self.values[name]
                if isinstance(value, Exception):
                    raise value
                return value

        class Hooks:
            def __init__(self):
                if constructor_issue:
                    raise RuntimeError("hook construction failed")
                instances.append(self)

            def hook(self):
                lifecycle.append("hook")
                if hook_issue == "rejected":
                    return False
                registered.append(self)
                if hook_issue == "raised":
                    raise RuntimeError("registration failed after registration")
                return True

            def unhook(self):
                lifecycle.append("unhook")
                if unhook_issue == "raised":
                    raise RuntimeError("unhook failed")
                if unhook_issue == "rejected":
                    return False
                if self in registered:
                    registered.remove(self)
                return True

        class Visitor:
            def __init__(self, mba, block):
                self.mba, self.blk, self.curins = mba, block, None
                visitors.append(weakref.ref(self))

        def emit(phase, *args):
            return [getattr(observer, phase)(*args) for observer in list(registered)]

        def expire():
            for value in borrowed:
                value.valid = False

        def micro_operand(kind=0, *, size=4, **fields):
            return Borrowed(t=kind, size=size, **fields)

        def instruction(ea, *, text="call worker", opcode=17, callinfo=None, left=None, right=None, destination=None):
            destination = destination if destination is not None else micro_operand(7 if callinfo is not None else 0, f=callinfo)
            return Borrowed(ea=ea, opcode=opcode, dstr=lambda: text, d=destination,
                            l=left if left is not None else micro_operand(),
                            r=right if right is not None else micro_operand())

        internal_sp_reads, coordinate_reads = [], []

        def mba_fixture(*, entry=0x140001000, mappings=None, instructions=None, flags=0x30, serial=3):
            instructions = instructions if instructions is not None else [instruction(0xf1c00040)]
            mappings = mappings if mappings is not None else {0xf1c00040: 0x1400010a0}
            mba = Borrowed(entry_ea=entry, maturity=2, get_mba_flags=lambda: 7,
                           get_mba_flags2=lambda: 9, has_outlines=lambda: True,
                           frsize=32, frregs=4, fpd=0, fullsize=64, stacksize=60, inargoff=64, minstkref=-24,
                           tmpstk_size=24, retsize=4, spd_adjust=0, last_prolog_ea=entry + 4,
                           first_epilog_ea=entry + 0x80, mbr=Borrowed(pfn=None, ranges=[]))
            mba.values["this"] = id(mba)
            block = Borrowed(mba=mba, serial=serial, flags=flags, tail=instructions[-1] if instructions else None,
                             head=instructions[0] if instructions else None, maxbsp=60,
                             npred=lambda: 0, pred=lambda index: None, nsucc=lambda: 0, succ=lambda index: None)
            points = {}

            def locate(address):
                internal_sp_reads.append((entry, address))
                value = points.get(address)
                if isinstance(value, Exception):
                    raise value
                return value

            def convert(direction, offset):
                coordinate_reads.append((entry, direction, offset))
                return offset - 16 if direction == "vd2ida" else offset + 16

            def visit(visitor):
                # Model the SDK's documented inclusion of nested instructions.
                visitor.mba, visitor.blk = mba, block
                for insn in instructions:
                    visitor.curins = insn
                    result = visitor.visit_minsn()
                    if result:
                        return result
                return 0

            mba.values.update(map_fict_ea=lambda ea: mappings[ea], for_all_insns=visit,
                              get_mblock=lambda index: block if index == serial else None,
                              locate_stkpnt=locate, stack_points=points,
                              stkoff_vd2ida=lambda offset: convert("vd2ida", offset),
                              stkoff_ida2vd=lambda offset: convert("ida2vd", offset))
            block.values["for_all_insns"] = visit
            return mba, block, mappings

        failure = SimpleNamespace(errea=0xf1c00040, code=-12, str="original failure detail",
                                  desc=lambda: "call analysis failed")
        implementation = SimpleNamespace(run=lambda ea, failed: None)

        def decompile(ea, failed):
            decompiles.append((ea, failed))
            return implementation.run(ea, failed)

        api = SimpleNamespace(Hexrays_Hooks=Hooks, minsn_visitor_t=Visitor, MBL_INLINED=0x10,
                              MBL_EXTFRAME=0x20, decompile=decompile, hexrays_failure_t=lambda: failure,
                              clear_cached_cfuncs=lambda: None, m_call=17, m_icall=18, mop_f=7,
                              mop_S=5, mop_r=1, mop_n=2, mop_a=9, mop_d=4,
                              get_mreg_name=lambda register, width: {4: "esp", 5: "ebp"}.get(register, "r" + str(register)))
        chunks = {0x140002000: [(0x140002000, 0x140002008), (0x140002030, 0x140002038)]}

        def vector(items=(), **values):
            items = list(items)
            return Borrowed(size=lambda: len(items), at=lambda index: items[index], **values)

        def argloc(kind="none", **fields):
            kinds = {"none": 0, "stack": 1, "scattered": 2, "register": 3, "register_pair": 4,
                     "register_relative": 5, "static": 6, "custom": 7}
            return Borrowed(atype=lambda: kinds[kind], is_badloc=lambda: kind == "none",
                is_reg1=lambda: kind == "register", is_reg2=lambda: kind == "register_pair",
                is_stkoff=lambda: kind == "stack", is_ea=lambda: kind == "static",
                is_rrel=lambda: kind == "register_relative", is_scattered=lambda: kind == "scattered",
                is_custom=lambda: kind == "custom",
                reg1=lambda: fields["register"], regoff=lambda: fields.get("register_offset", 0),
                reg2=lambda: fields["second_register"], stkoff=lambda: fields["stack_offset"],
                get_ea=lambda: fields["address"],
                get_rrel=lambda: Borrowed(reg=fields["register"], off=fields["offset"]),
                scattered=lambda: vector(fields["parts"]),
                off=fields.get("argument_offset", 0), size=fields.get("size", 4))

        detail_requests, type_reads, sp_reads = [], [], []
        stored_types, original_instructions, function_chunks, functions, parent_lists, primary_owners = {}, {}, {}, {}, {}, {}

        def type_fixture(*, text="int", details=None, calculated=None, pointed=None):
            value = Borrowed(printed=text, is_func=lambda: details is not None,
                             is_funcptr=lambda: pointed is not None, get_pointed_object=lambda: pointed)

            def get_details(destination, mode):
                detail_requests.append(mode)
                selected = (calculated if calculated is not None else details) if mode == 2 else details
                if isinstance(selected, Exception):
                    raise selected
                if selected is None or selected is False:
                    return False
                destination.values.update(selected.values)
                return True

            value.values["get_func_details"] = get_details
            return value

        def function_details(arguments=(), *, cc=48, retloc=None, **overrides):
            value = vector(arguments, cc=cc, flags=0, stkargs=4, rettype=type_fixture(),
                           retloc=retloc if retloc is not None else argloc("register", register=0))
            value.values.update(overrides)
            return value

        def argument(location=None, *, name="value", type=None, **values):
            return Borrowed(flags=0, name=name, type=type if type is not None else type_fixture(),
                            argloc=location if location is not None else argloc("stack", stack_offset=0),
                            ea=0x140001020, t=6, size=4, dstr=lambda: "stack_value", **values)

        def read_type(destination, address, operand=None):
            type_reads.append((address, operand))
            value = stored_types.get((address, operand))
            if isinstance(value, Exception):
                raise value
            if value is None:
                return False
            destination.values.update(value.values)
            return True

        def operand(kind, **fields):
            return Borrowed(type=kind, n=fields.pop("n", 0), dtype=2, **fields)

        def original(address, *, operands=None, size=5, is_call=True, is_indirect_jump=False, **fields):
            values = dict(ea=address, size=size, itype=88, is_call=is_call, is_indirect_jump=is_indirect_jump,
                          ops=list(operands or [operand(7, addr=0x140002000)]) + [operand(0)])
            values.update(fields)
            result = Borrowed(**values)
            original_instructions[address] = result
            return result

        def decode(destination, address):
            value = original_instructions[address]
            if isinstance(value, Exception):
                raise value
            destination.values.update(value.values)
            return value.size

        def owner(address, *, start=0x140001000, tail=False, parents=None, primary=None):
            entries = list(parents if parents is not None else [start])
            for entry in entries:
                functions[entry] = Borrowed(start_ea=entry, end_ea=entry + 0x1000, flags=0,
                                            frame=entry + 0x2000, frsize=16, frregs=4, argsize=0, fpd=0)
            chunk = Borrowed(start_ea=start, end_ea=start + 0x1000, flags=0x8000 if tail else 0, is_tail=tail)
            function_chunks[address] = chunk
            parent_lists[start] = entries
            primary_owners[address] = primary if primary is not None else entries[0]
            return chunk

        class ParentIterator:
            def __init__(self):
                self.chunk = None

            def set(self, chunk):
                self.chunk = chunk
                return True

            def __iter__(self):
                return iter(parent_lists[self.chunk.start_ea])

        def get_func(address):
            return functions.get(primary_owners.get(address, address))

        def read_sp(name, function, address):
            sp_reads.append((name, function.start_ea, address))
            return {"spd_before": -12, "effective_spd": -16, "sp_change_point_delta": 0}[name]

        bytes_api = SimpleNamespace(is_loaded=lambda address: address in original_instructions,
            is_code=lambda flags: flags == 1, get_full_flags=lambda address: 1,
            get_item_head=lambda address: address, get_item_size=lambda address: original_instructions[address].size)
        ua_api = SimpleNamespace(insn_t=Borrowed, decode_insn=decode, o_void=0, o_reg=1, o_mem=2,
                                 o_phrase=3, o_displ=4, o_far=6, o_near=7)
        type_api = SimpleNamespace(tinfo_t=lambda: type_fixture(), func_type_data_t=lambda: vector(),
            GTD_NO_ARGLOCS=1, GTD_CALC_ARGLOCS=2, PRTYPE_1LINE=8,
            print_tinfo=lambda prefix, indent, cmtindent, flags, tif, name, cmt: tif.printed)
        funcs_api = SimpleNamespace(get_func=get_func, get_fchunk=lambda address: function_chunks.get(address),
            is_func_tail=lambda chunk: chunk.is_tail, func_parent_iterator_t=ParentIterator,
            get_func_chunknum=lambda function, address: 1 if function_chunks[address].is_tail else 0)
        frame_api = SimpleNamespace(get_spd=lambda function, address: read_sp("spd_before", function, address),
            get_effective_spd=lambda function, address: read_sp("effective_spd", function, address),
            get_sp_delta=lambda function, address: read_sp("sp_change_point_delta", function, address))
        namespace = {"ida_hexrays": api, "idaapi": SimpleNamespace(BADADDR=0xffffffffffffffff),
                     "ida_bytes": bytes_api, "ida_ua": ua_api, "ida_typeinf": type_api,
                     "ida_nalt": SimpleNamespace(get_tinfo=read_type, get_op_tinfo=read_type),
                     "ida_funcs": funcs_api, "ida_frame": frame_api,
                     "ida_idp": SimpleNamespace(is_call_insn=lambda instruction: instruction.is_call,
                         is_indirect_jump_insn=lambda instruction: instruction.is_indirect_jump),
                     "idautils": SimpleNamespace(Chunks=lambda ea: iter(chunks[ea]),
                         Heads=lambda start, stop: (ea for ea in sorted(original_instructions) if start <= ea < stop))}
        names = {"_static_diagnostic_error", "_FrozenInsnVisitor", "StaticMicrocodeTrace", "decompile_with_diagnostics"}
        definitions = [node for node in ast.parse(source.read_text(encoding="utf-8")).body
                       if isinstance(node, (ast.ClassDef, ast.FunctionDef)) and node.name in names]
        self.assertEqual({node.name for node in definitions}, names)
        exec(compile(ast.Module(body=definitions, type_ignores=[]), str(source), "exec"), namespace)
        return SimpleNamespace(namespace=namespace, source=source, api=api, failure=failure, emit=emit,
            expire=expire, implementation=implementation, mba=mba_fixture, instruction=instruction, Borrowed=Borrowed,
            chunks=chunks, registered=registered, instances=instances, lifecycle=lifecycle, borrowed=borrowed,
            visitors=visitors, decompiles=decompiles, observe=namespace["decompile_with_diagnostics"],
            vector=vector, argloc=argloc, type=type_fixture, details=function_details, argument=argument,
            stored_types=stored_types, original=original, operand=operand, owner=owner,
            original_instructions=original_instructions, function_chunks=function_chunks, functions=functions,
            parent_lists=parent_lists, bytes_api=bytes_api, ua_api=ua_api, funcs_api=funcs_api, frame_api=frame_api,
            type_api=type_api, detail_requests=detail_requests, type_reads=type_reads, sp_reads=sp_reads,
            internal_sp_reads=internal_sp_reads, coordinate_reads=coordinate_reads, micro_operand=micro_operand,
            Trace=namespace["StaticMicrocodeTrace"])

    def test_failed_decompile_freezes_nested_instructions_before_sdk_proxies_expire(self):
        fixture = self.fixture()
        nested = [fixture.instruction(0x140001030, text="nested load", opcode=5), fixture.instruction(0xf1c00040)]
        mba, block, _ = fixture.mba(instructions=nested, mappings={0x140001030: 0x140001030, 0xf1c00040: 0x1400010a0})
        out_type = fixture.Borrowed()
        out_type.valid = False

        def run(ea, failure):
            self.assertEqual(fixture.emit("microcode", mba), [0])
            self.assertEqual(fixture.emit("preoptimized", mba), [0])
            self.assertEqual(fixture.emit("resolve_stkaddrs", mba), [0])
            self.assertEqual(fixture.emit("build_callinfo", block, out_type), [None])
            fixture.expire()
            return None

        fixture.implementation.run = run
        diagnostics = {}
        self.assertIsNone(fixture.observe(0x140001000, fixture.failure, diagnostics))
        self.assertEqual(diagnostics["decompile_outcome"], "returned_none")
        self.assertEqual(diagnostics["events_recorded"], 4)
        self.assertEqual(diagnostics["instruction_records"], 8)
        self.assertTrue(diagnostics["diagnostics_complete"])
        self.assertNotIn("ctree_maturity", [row["phase"] for row in diagnostics["events"]])
        event = diagnostics["events"][-1]
        self.assertEqual(event["block_tail_ea"], "0xf1c00040")
        self.assertEqual(event["mba_maturity"], 2)
        self.assertEqual(event["mba_flags2"], 9)
        self.assertEqual(event["instructions"][0]["ea"], "0x140001030")
        self.assertTrue(event["instructions"][1]["inlined"])
        self.assertTrue(event["instructions"][1]["external_frame"])
        mapping = diagnostics["failure_mapping"]
        self.assertEqual(mapping["raw_failure_ea"], "0xf1c00040")
        self.assertEqual(mapping["target_context_mapping_candidates"], ["0x1400010a0"])
        self.assertEqual(mapping["mapping_status"], "unique_api_candidate_requires_original_pe_validation")
        self.assertFalse(mapping["original_pe_instruction_validated"])
        self.assertEqual(fixture.failure.errea, 0xf1c00040)
        self.assertEqual(fixture.lifecycle, ["hook", "unhook"])
        self.assertEqual(fixture.registered, [])
        self.assertTrue(all(ref() is None for ref in fixture.visitors))
        json.dumps(diagnostics)
        fixture.instances[0].lookup_failure(fixture.failure.errea)
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        self.assertEqual(set(out_type.__dict__), {"values", "valid", "expired_reads"})

    def test_inline_owner_ranges_and_inserted_interval_are_frozen_with_complete_chunks(self):
        fixture = self.fixture()
        mba, _, _ = fixture.mba()
        owner = fixture.Borrowed(start_ea=0x140002000, flags=0x20000)
        ranges = [fixture.Borrowed(start_ea=0x140002000, end_ea=0x140002008)]
        mbr, cdg = fixture.Borrowed(pfn=owner, ranges=ranges), fixture.Borrowed(mba=mba)

        def run(ea, failure):
            self.assertEqual(fixture.emit("inlining_func", cdg, 3, mbr), [0])
            self.assertEqual(fixture.emit("inlined_func", cdg, 3, mbr, 4, 7), [0])
            owner.values["start_ea"] = 0x140009000
            ranges[0].values["end_ea"] = 0x140009001
            fixture.chunks[0x140002000].clear()
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        event = diagnostics["events"][1]["inline_event"]
        self.assertEqual(event["owner_ea"], "0x140002000")
        self.assertEqual(event["owner_flags"], 0x20000)
        self.assertEqual(event["owner_chunks"], [{"start": "0x140002000", "end": "0x140002008"},
                                               {"start": "0x140002030", "end": "0x140002038"}])
        self.assertEqual(event["listed_ranges"], [{"start": "0x140002000", "end": "0x140002008"}])
        self.assertEqual(event["inserted_block_interval"], [4, 7])
        self.assertNotIn("inserted_block_interval", diagnostics["events"][0]["inline_event"])
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_unobserved_badaddr_conflicting_and_foreign_mba_mappings_remain_unresolved(self):
        for scenario, expected in (("unobserved", "unobserved_in_target_context"), ("badaddr", "badaddr"),
                                   ("conflict", "conflicting"), ("foreign", "unobserved_in_target_context")):
            with self.subTest(scenario=scenario):
                fixture = self.fixture()
                mba, _, mappings = fixture.mba(entry=0x140002000 if scenario == "foreign" else 0x140001000)

                def run(ea, failure):
                    if scenario == "badaddr": mappings[0xf1c00040] = 0xffffffffffffffff
                    if scenario != "unobserved": fixture.emit("microcode", mba)
                    if scenario == "conflict":
                        mappings[0xf1c00040] = 0x1400010b0
                        fixture.emit("locopt", mba)
                    fixture.expire()

                fixture.implementation.run = run
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                mapping = diagnostics["failure_mapping"]
                self.assertEqual(mapping["mapping_status"], expected)
                self.assertFalse(mapping["mapping_unique_in_target_context"])
                self.assertFalse(mapping["original_pe_instruction_validated"])
                if scenario == "conflict":
                    self.assertEqual(mapping["target_context_mapping_candidates"], ["0x1400010a0", "0x1400010b0"])
                elif scenario == "foreign":
                    self.assertEqual(len(mapping["matches"]), 1)
                    self.assertEqual(mapping["target_context_mapping_candidates"], [])
                elif scenario == "unobserved":
                    self.assertFalse(diagnostics["diagnostics_complete"])
                self.assertEqual(fixture.failure.errea, 0xf1c00040)

    def test_mapping_and_metadata_errors_keep_original_failure_and_other_instruction_observations(self):
        fixture = self.fixture()
        mba, _, mappings = fixture.mba(instructions=[fixture.instruction(0xf1c00040), fixture.instruction(0x140001030)],
                                      mappings={0xf1c00040: 0x1400010a0, 0x140001030: 0x140001030})
        mba.values["get_mba_flags"] = lambda: (_ for _ in ()).throw(RuntimeError("metadata unavailable"))

        def broken_mapping(ea):
            if ea == 0xf1c00040: raise ValueError("mapping unavailable")
            return mappings[ea]

        mba.values["map_fict_ea"] = broken_mapping

        def run(ea, failure):
            self.assertEqual(fixture.emit("microcode", mba), [0])
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        self.assertIsNone(fixture.observe(0x140001000, fixture.failure, diagnostics))
        self.assertFalse(diagnostics["diagnostics_complete"])
        self.assertGreaterEqual(diagnostics["diagnostic_error_count"], 2)
        self.assertEqual(diagnostics["events"][0]["instructions"][1]["api_mapped_ea"], "0x140001030")
        failed = diagnostics["events"][0]["instructions"][0]
        self.assertEqual(failed["mapping_status"], "diagnostic_error")
        self.assertIn("mapping unavailable", failed["diagnostic_errors"][0]["error"])
        self.assertEqual(diagnostics["failure_mapping"]["mapping_status"], "incomplete")
        self.assertEqual(fixture.failure.code, -12)
        self.assertEqual(fixture.failure.str, "original failure detail")
        json.dumps(diagnostics)

    def test_instruction_text_failure_cannot_prevent_address_mapping(self):
        fixture = self.fixture()
        insn = fixture.instruction(0xf1c00040)
        insn.values["dstr"] = lambda: (_ for _ in ()).throw(RuntimeError("text unavailable"))
        mba, _, _ = fixture.mba(instructions=[insn])
        fixture.implementation.run = lambda ea, failure: fixture.emit("microcode", mba) and None
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        self.assertEqual(diagnostics["failure_mapping"]["target_context_mapping_candidates"], ["0x1400010a0"])
        self.assertEqual(diagnostics["failure_mapping"]["mapping_status"], "incomplete")
        self.assertFalse(diagnostics["diagnostics_complete"])

    def test_event_instruction_and_text_limits_bound_records_without_changing_callback_codes(self):
        for per_event, total in ((1, 20), (20, 1)):
            with self.subTest(per_event=per_event, total=total):
                fixture = self.fixture()
                fixture.Trace.DEFAULT_LIMITS.update(events=2, instructions_per_event=per_event,
                                                  instructions_total=total, text_characters=4)
                mba, _, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040), fixture.instruction(0xf1c00040)])

                def run(ea, failure):
                    for phase in ("microcode", "preoptimized", "locopt", "resolve_stkaddrs"):
                        self.assertEqual(fixture.emit(phase, mba), [0])

                fixture.implementation.run = run
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                self.assertEqual(diagnostics["callbacks_seen"], 4)
                self.assertEqual(diagnostics["events_recorded"], 2)
                self.assertEqual(diagnostics["dropped_events"], 2)
                self.assertLessEqual(diagnostics["instruction_records"], total)
                self.assertTrue(diagnostics["truncation"]["event_records"])
                self.assertTrue(diagnostics["truncation"]["instruction_records"])
                self.assertTrue(diagnostics["truncation"]["instruction_text"])
                self.assertEqual(diagnostics["events"][0]["instructions"][0]["text"], "call")
                self.assertTrue(diagnostics["events"][0]["instructions"][0]["text_truncated"])
                self.assertFalse(diagnostics["diagnostics_complete"])
                self.assertEqual(diagnostics["failure_mapping"]["mapping_status"], "incomplete")
                self.assertEqual(diagnostics["diagnostic_error_count"], 0)

    def test_inline_range_and_owner_chunk_limits_mark_incomplete_without_dropping_owner_identity(self):
        fixture = self.fixture()
        fixture.Trace.DEFAULT_LIMITS.update(inline_ranges=1, owner_chunks=1)
        mba, _, _ = fixture.mba()
        owner = fixture.Borrowed(start_ea=0x140002000, flags=0x20000)
        ranges = [fixture.Borrowed(start_ea=0x140002000, end_ea=0x140002008),
                  fixture.Borrowed(start_ea=0x140002030, end_ea=0x140002038)]
        mbr, cdg = fixture.Borrowed(pfn=owner, ranges=ranges), fixture.Borrowed(mba=mba)
        fixture.implementation.run = lambda ea, failure: fixture.emit("inlined_func", cdg, 3, mbr, 4, 7) and None
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        inline = diagnostics["events"][0]["inline_event"]
        self.assertEqual(inline["owner_ea"], "0x140002000")
        self.assertEqual(len(inline["listed_ranges"]), 1)
        self.assertEqual(len(inline["owner_chunks"]), 1)
        self.assertTrue(diagnostics["truncation"]["inline_ranges"])
        self.assertTrue(diagnostics["truncation"]["owner_chunks"])
        self.assertFalse(diagnostics["diagnostics_complete"])

    def test_success_keeps_original_cfunc_and_full_bounded_events_after_sdk_proxies_expire(self):
        fixture = self.fixture()
        fixture.original(0x1400010a0)
        fixture.owner(0x1400010a0)
        fixture.stored_types[(0x140002000, None)] = fixture.type(details=fixture.details([fixture.argument()]))
        mba, _, _ = fixture.mba()
        cfunc = object()

        def run(ea, failure):
            fixture.emit("microcode", mba)
            fixture.emit("calls_done", mba)
            fixture.emit("maturity", fixture.Borrowed(mba=mba), 8)
            fixture.expire()
            return cfunc

        fixture.implementation.run = run
        diagnostics = {}
        self.assertIs(fixture.observe(0x140001000, fixture.failure, diagnostics), cfunc)
        self.assertEqual(diagnostics["decompile_outcome"], "returned_cfunc")
        self.assertEqual(diagnostics["events_recorded"], 3)
        self.assertGreater(diagnostics["type_records"], 0)
        self.assertGreater(diagnostics["argument_records"], 0)
        self.assertEqual([event["phase"] for event in diagnostics["events"]],
                         ["microcode", "calls_done", "ctree_maturity"])
        self.assertEqual(diagnostics["events"][0]["instructions"][0]["ea"], "0xf1c00040")
        self.assertNotIn("failure_mapping", diagnostics)
        self.assertTrue(diagnostics["hook_unregistered"])
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_constructor_rejection_and_partial_registration_failure_cannot_skip_decompile(self):
        for issue in ("constructor", "rejected", "raised"):
            with self.subTest(issue=issue):
                fixture = self.fixture(hook_issue=issue, constructor_issue=issue == "constructor")
                cfunc = object()
                fixture.implementation.run = lambda ea, failure: cfunc
                diagnostics = {}
                self.assertIs(fixture.observe(0x140001000, fixture.failure, diagnostics), cfunc)
                self.assertEqual(len(fixture.decompiles), 1)
                self.assertFalse(diagnostics["hook_registered"])
                self.assertEqual(diagnostics["diagnostic_errors"][0]["stage"], "hook_registration")
                self.assertEqual(fixture.registered, [])
                self.assertEqual(fixture.lifecycle, [] if issue == "constructor" else ["hook", "unhook"])
                self.assertNotIn("events", diagnostics)

    def test_unhook_failure_keeps_original_success_none_or_exception_and_disables_future_callbacks(self):
        for cleanup in ("rejected", "raised"):
            for outcome in ("success", "none", "raised"):
                with self.subTest(cleanup=cleanup, outcome=outcome):
                    fixture = self.fixture(unhook_issue=cleanup)
                    mba, _, _ = fixture.mba()
                    original_error, cfunc = RuntimeError("original decompiler failure"), object()

                    def run(ea, failure):
                        fixture.emit("microcode", mba)
                        fixture.expire()
                        if outcome == "raised": raise original_error
                        return cfunc if outcome == "success" else None

                    fixture.implementation.run = run
                    diagnostics = {}
                    if outcome == "raised":
                        with self.assertRaises(RuntimeError) as context:
                            fixture.observe(0x140001000, fixture.failure, diagnostics)
                        self.assertIs(context.exception, original_error)
                    else:
                        self.assertIs(fixture.observe(0x140001000, fixture.failure, diagnostics), cfunc if outcome == "success" else None)
                    self.assertEqual(diagnostics["diagnostic_errors"][0]["stage"], "hook_cleanup")
                    self.assertFalse(diagnostics["diagnostics_complete"])
                    self.assertFalse(fixture.instances[0].active)
                    self.assertEqual(fixture.emit("microcode", mba), [0])
                    self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
                    if outcome != "success":
                        self.assertEqual(diagnostics["failure_mapping"]["raw_failure_ea"], "0xf1c00040")
                        self.assertEqual(diagnostics["failure_mapping"]["mapping_status"], "incomplete")

    def test_call_types_locations_microcode_sp_and_all_shared_tail_owners_are_frozen(self):
        fixture = self.fixture()
        address, callee = 0x1400010a0, 0x140003000
        uncalculated = fixture.details([fixture.argument(fixture.argloc(), name="input")],
                                      retloc=fixture.argloc())
        calculated = fixture.details([fixture.argument(fixture.argloc("stack", stack_offset=8), name="input")],
                                    retloc=fixture.argloc("register", register=33))
        function = fixture.type(text="int __stdcall worker(int input)", details=uncalculated, calculated=calculated)
        fixture.stored_types[(address, 0)] = fixture.type(text="int (__stdcall *)(int)", pointed=function)
        fixture.stored_types[(callee, None)] = function
        fixture.original(address, operands=[fixture.operand(7, addr=callee)])
        fixture.owner(address, start=0x140001080, tail=True,
                      parents=[0x140002000, 0x140001000], primary=0x140002000)
        fixture.frame_api.get_spd = lambda owner, ea: (-28 if owner.start_ea == 0x140001000 else -4)
        arg = fixture.argument(fixture.argloc("stack", stack_offset=12), name="input")
        info = fixture.Borrowed(callee=callee, call_spd=-28, stkargs_top=16, cc=48, solid_args=1,
                               return_type=fixture.type(), return_argloc=fixture.argloc("register", register=33),
                               args=fixture.vector([arg]))
        mba, block, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, callinfo=info)])
        out_type = fixture.Borrowed()
        out_type.valid = False

        def run(ea, failed):
            self.assertEqual(fixture.emit("build_callinfo", block, out_type), [None])
            function.values["printed"] = "changed after observation"
            calculated.values["cc"] = 1
            info.values["call_spd"] = -999
            arg.values["argloc"] = fixture.argloc("stack", stack_offset=999)
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        self.assertTrue(diagnostics["diagnostics_complete"])
        call = diagnostics["events"][0]["instructions"][0]["callsite"]
        original = call["original_instruction"]
        self.assertEqual(original["address"], hex(address))
        self.assertFalse(original["original_pe_validated"])
        operand = original["operands"][0]
        self.assertEqual(operand["address_role"], "direct_callee")
        stored = operand["stored_tinfo"]
        self.assertEqual(stored["kind"], "function_pointer")
        self.assertEqual(stored["api"], "ida_nalt.get_op_tinfo")
        self.assertEqual(stored["without_argloc_calculation"]["parameters"]["arguments"][0]["location"]["status"], "unobserved")
        self.assertEqual(stored["with_argloc_calculation"]["parameters"]["arguments"][0]["location"]["stack_offset"], 8)
        self.assertEqual(stored["with_argloc_calculation"]["return_location"]["register"], 33)
        self.assertEqual(stored["with_argloc_calculation"]["cc"], 48)
        self.assertFalse(stored["serialized_locations_verified"])
        self.assertEqual(operand["address_stored_tinfo"]["declaration"], "int __stdcall worker(int input)")
        self.assertEqual(fixture.detail_requests, [1, 2, 1, 2])
        context = original["function_context"]
        self.assertEqual(context["primary_owner_ea"], "0x140002000")
        self.assertTrue(context["chunk"]["is_tail"])
        self.assertTrue(context["owners_complete"])
        self.assertEqual([owner["owner_ea"] for owner in context["owners"]], ["0x140002000", "0x140001000"])
        self.assertEqual([owner["spd_before"]["value"] for owner in context["owners"]], [-4, -28])
        self.assertEqual(context["sp_delta_zero_means"], "no_change_point_at_address")
        self.assertEqual(fixture.sp_reads, [(name, owner, address) for owner in (0x140002000, 0x140001000)
                                          for name in ("effective_spd", "sp_change_point_delta")])
        micro = call["microcode_callinfo"]
        self.assertEqual((micro["call_spd"], micro["stkargs_top"], micro["cc"]), (-28, 16, 48))
        self.assertEqual(micro["return_location"]["register"], 33)
        self.assertEqual(micro["parameters"]["arguments"][0]["location"]["stack_offset"], 12)
        self.assertEqual(micro["parameters"]["arguments"][0]["value_text"], "stack_value")
        self.assertEqual(out_type.values, {})
        self.assertTrue(all(ref() is None for ref in fixture.visitors))
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)
        fixture.instances[0].lookup_failure(fixture.failure.errea)

    def test_callinfo_built_observes_each_analyzed_call_before_a_later_call_fails(self):
        fixture = self.fixture()
        argument = fixture.argument()
        info = fixture.Borrowed(callee=0x140003000, call_spd=-24, stkargs_top=8, cc=48, solid_args=1,
            return_type=fixture.type(), return_argloc=fixture.argloc(), args=fixture.vector([argument]))
        insn = fixture.instruction(0xf1c00040)
        mba, block, _ = fixture.mba(instructions=[insn])
        argument.values.update(t=5, s=fixture.Borrowed(off=12, mba=mba))

        def run(ea, failed):
            self.assertEqual(fixture.emit("build_callinfo", block, None), [None])
            insn.values["d"] = fixture.micro_operand(7, f=info)
            self.assertEqual(fixture.emit("callinfo_built", block), [0])
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        self.assertEqual([event["phase"] for event in diagnostics["events"]], ["build_callinfo", "callinfo_built"])
        before, after = [event["instructions"][0]["callsite"]["microcode_callinfo"] for event in diagnostics["events"]]
        self.assertEqual(before["reason"], "callinfo_operand_not_present")
        self.assertEqual(after["call_spd"], -24)
        self.assertEqual(after["parameters"]["count"], 1)
        stack = after["parameters"]["arguments"][0]["operand"]["stack"]
        self.assertEqual(stack["decompiler_offset"], 12)
        self.assertEqual(stack["coordinate_conversion"]["ida_offset"], -4)
        self.assertTrue(diagnostics["diagnostics_complete"])
        self.assertEqual(fixture.failure.code, -12)
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_direct_memory_register_and_displacement_call_operands_keep_distinct_type_sources(self):
        for kind, fields, expected_role in ((7, {"addr": 0x140003000}, "direct_callee"),
                (6, {"addr": 0x140003000}, "direct_callee"),
                (2, {"addr": 0x140003000}, "memory_slot"),
                (1, {"reg": 6}, None), (3, {"phrase": 5}, None),
                (4, {"phrase": 5, "addr": 0x140003000}, None)):
            with self.subTest(kind=kind):
                fixture = self.fixture()
                address = 0x1400010a0
                fixture.original(address, operands=[fixture.operand(kind, **fields)])
                fixture.owner(address)
                mba, block, _ = fixture.mba()
                fixture.implementation.run = lambda ea, failed: fixture.emit("build_callinfo", block, None) and None
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                call = diagnostics["events"][0]["instructions"][0]["callsite"]
                op = call["original_instruction"]["operands"][0]
                self.assertEqual(op["stored_tinfo"]["status"], "unobserved")
                self.assertEqual(op["stored_tinfo"]["reason"], "no_stored_tinfo")
                self.assertEqual(call["microcode_callinfo"]["reason"], "callinfo_operand_not_present")
                self.assertEqual(op.get("address_role"), expected_role)
                self.assertEqual(fixture.type_reads, [(address, 0)] + ([(0x140003000, None)] if expected_role else []))
                if kind == 2:
                    self.assertEqual(op["runtime_target"], {"status": "unobserved", "reason": "memory_slot_not_dereferenced"})
                elif kind == 4:
                    self.assertEqual(op["displacement"], 0x140003000)
                    self.assertNotIn("address_stored_tinfo", op)
                self.assertTrue(diagnostics["diagnostics_complete"])

    def test_indirect_jump_preserves_jump_identity_and_three_independent_stored_types(self):
        fixture = self.fixture()
        address, slot = 0x1400010a0, 0x140003000
        details = fixture.details([fixture.argument()], cc=67)
        function = fixture.type(text="int __cdecl thunk(const char *format, ...)", details=details)
        pointer = fixture.type(text="int (__cdecl *)(const char *, ...)", pointed=function)
        fixture.stored_types[(address, None)] = function
        fixture.stored_types[(address, 0)] = pointer
        fixture.stored_types[(slot, None)] = pointer
        original = fixture.original(address, size=6, is_call=False, is_indirect_jump=True,
                                    operands=[fixture.operand(2, addr=slot)])
        original_fields = dict(original.values)
        fixture.owner(address, start=address)
        mba, block, _ = fixture.mba()

        def run(ea, failed):
            self.assertEqual(fixture.emit("build_callinfo", block, None), [None])
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        self.assertIsNone(fixture.observe(0x140001000, fixture.failure, diagnostics))
        callsite = diagnostics["events"][0]["instructions"][0]["callsite"]
        transfer = callsite["original_instruction"]
        self.assertFalse(transfer["is_call"])
        self.assertTrue(transfer["is_indirect_jump"])
        self.assertEqual(transfer["transfer_kind"], "indirect_jump")
        self.assertEqual(transfer["call_operands"]["reason"], "mapped_instruction_is_not_call")
        self.assertFalse(transfer["original_pe_validated"])
        self.assertEqual(transfer["instruction_stored_tinfo"]["kind"], "function")
        self.assertEqual(transfer["instruction_stored_tinfo"]["without_argloc_calculation"]["cc"], 67)
        operand = transfer["operands"][0]
        self.assertEqual(operand["stored_tinfo"]["kind"], "function_pointer")
        self.assertEqual(operand["address_stored_tinfo"]["kind"], "function_pointer")
        self.assertEqual((operand["address"], operand["address_role"]), (hex(slot), "memory_slot"))
        self.assertEqual(operand["runtime_target"], {"status": "unobserved", "reason": "memory_slot_not_dereferenced"})
        self.assertEqual(fixture.type_reads, [(address, None), (address, 0), (slot, None)])
        self.assertEqual(callsite["microcode_callinfo"]["reason"], "callinfo_operand_not_present")
        self.assertEqual(original.values, original_fields)
        self.assertEqual(fixture.failure.code, -12)
        self.assertTrue(diagnostics["diagnostics_complete"])
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_indirect_jump_absent_or_failed_type_stays_separate_from_other_sources(self):
        for issue in ("missing_slot_type", "failed_thunk_type", "failed_operand_type"):
            with self.subTest(issue=issue):
                fixture = self.fixture()
                address, slot = 0x1400010a0, 0x140003000
                function = fixture.type(details=fixture.details(cc=67))
                pointer = fixture.type(pointed=function)
                fixture.stored_types[(address, None)] = function
                fixture.stored_types[(address, 0)] = pointer
                fixture.stored_types[(slot, None)] = pointer
                if issue == "missing_slot_type": del fixture.stored_types[(slot, None)]
                if issue == "failed_thunk_type": fixture.stored_types[(address, None)] = RuntimeError("thunk type unavailable")
                if issue == "failed_operand_type": fixture.stored_types[(address, 0)] = RuntimeError("operand type unavailable")
                fixture.original(address, size=6, is_call=False, is_indirect_jump=True,
                                 operands=[fixture.operand(2, addr=slot)])
                fixture.owner(address, start=address)
                mba, block, _ = fixture.mba()
                fixture.implementation.run = lambda ea, failed: fixture.emit("build_callinfo", block, None) and None
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                transfer = diagnostics["events"][0]["instructions"][0]["callsite"]["original_instruction"]
                operand = transfer["operands"][0]
                self.assertEqual(transfer["status"], "observed")
                self.assertFalse(transfer["is_call"])
                self.assertTrue(transfer["is_indirect_jump"])
                self.assertEqual(operand["runtime_target"]["reason"], "memory_slot_not_dereferenced")
                self.assertEqual(fixture.type_reads, [(address, None), (address, 0), (slot, None)])
                if issue == "missing_slot_type":
                    self.assertEqual(operand["address_stored_tinfo"]["reason"], "no_stored_tinfo")
                    self.assertTrue(diagnostics["diagnostics_complete"])
                else:
                    failed_type = transfer["instruction_stored_tinfo"] if issue == "failed_thunk_type" else operand["stored_tinfo"]
                    self.assertEqual(failed_type["status"], "diagnostic_error")
                    self.assertEqual(operand["address_stored_tinfo"]["status"], "observed_stored_tinfo")
                    self.assertFalse(diagnostics["diagnostics_complete"])
                self.assertEqual(fixture.failure.code, -12)

    def test_original_decode_missing_mismatch_or_exception_never_uses_a_guessed_call_target(self):
        for issue, expected in (("missing", "mapped_address_is_not_loaded_code"),
                ("noncode", "mapped_address_is_not_loaded_code"), ("middle", "mapped_address_is_not_instruction_head"),
                ("zero", "decode_returned_zero"), ("mismatch", "decoded_instruction_disagrees_with_idb_item"),
                ("exception", None), ("api_missing", None), ("noncall", None)):
            with self.subTest(issue=issue):
                fixture = self.fixture()
                address = 0x1400010a0
                if issue != "missing": fixture.original(address, is_call=issue != "noncall")
                if issue == "noncode": fixture.bytes_api.get_full_flags = lambda ea: 0
                if issue == "middle": fixture.bytes_api.get_item_head = lambda ea: ea - 1
                if issue == "zero": fixture.ua_api.decode_insn = lambda out, ea: 0
                if issue == "mismatch": fixture.bytes_api.get_item_size = lambda ea: 1
                if issue == "exception": fixture.ua_api.decode_insn = lambda out, ea: (_ for _ in ()).throw(RuntimeError("decode unavailable"))
                if issue == "api_missing": del fixture.ua_api.decode_insn
                mba, block, _ = fixture.mba()
                fixture.implementation.run = lambda ea, failed: fixture.emit("build_callinfo", block, None) and None
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                call = diagnostics["events"][0]["instructions"][0]["callsite"]
                original = call["original_instruction"]
                if expected:
                    self.assertEqual((original["status"], original["reason"]), ("unobserved", expected))
                elif issue == "noncall":
                    self.assertEqual(original["call_operands"]["reason"], "mapped_instruction_is_not_call")
                    self.assertFalse(original["is_call"])
                else:
                    self.assertEqual(original["status"], "diagnostic_error")
                    self.assertFalse(diagnostics["diagnostics_complete"])
                self.assertEqual(call["microcode_callinfo"]["reason"], "callinfo_operand_not_present")
                self.assertEqual(fixture.type_reads, [])
                self.assertEqual(fixture.failure.code, -12)

    def test_type_and_sp_errors_preserve_independent_sources_and_other_shared_owners(self):
        fixture = self.fixture()
        address, callee = 0x1400010a0, 0x140003000
        fixture.original(address, operands=[fixture.operand(7, addr=callee)])
        fixture.owner(address, start=0x140001080, tail=True, parents=[0x140002000, 0x140001000])
        fixture.stored_types[(address, 0)] = RuntimeError("operand type unavailable")
        details = fixture.details([fixture.argument()])
        fixture.stored_types[(callee, None)] = fixture.type(details=details, calculated=RuntimeError("argloc calculation unavailable"))

        def get_spd(owner, ea):
            if owner.start_ea == 0x140002000:
                raise RuntimeError("first owner SP unavailable")
            return -24

        fixture.frame_api.get_spd = get_spd
        mba, block, _ = fixture.mba()
        fixture.implementation.run = lambda ea, failed: fixture.emit("build_callinfo", block, None) and None
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        self.assertFalse(diagnostics["diagnostics_complete"])
        original = diagnostics["events"][0]["instructions"][0]["callsite"]["original_instruction"]
        operand = original["operands"][0]
        self.assertEqual(operand["stored_tinfo"]["status"], "diagnostic_error")
        self.assertEqual(operand["address_stored_tinfo"]["without_argloc_calculation"]["status"], "observed")
        self.assertEqual(operand["address_stored_tinfo"]["with_argloc_calculation"]["status"], "diagnostic_error")
        owners = original["function_context"]["owners"]
        self.assertEqual(owners[0]["spd_before"]["status"], "diagnostic_error")
        self.assertEqual(owners[0]["effective_spd"]["value"], -16)
        self.assertEqual(owners[1]["spd_before"]["value"], -24)
        self.assertEqual(diagnostics["diagnostic_error_count"], 3)

    def test_structured_argument_locations_include_pairs_relative_static_scattered_and_unobserved_custom(self):
        fixture = self.fixture()
        locations = [fixture.argloc(), fixture.argloc("register", register=1, register_offset=2),
            fixture.argloc("register_pair", register=1, second_register=2), fixture.argloc("stack", stack_offset=-8),
            fixture.argloc("register_relative", register=5, offset=-12), fixture.argloc("static", address=0x140008000),
            fixture.argloc("scattered", parts=[fixture.argloc("register", register=1, argument_offset=0, size=4),
                                               fixture.argloc("stack", stack_offset=4, argument_offset=4, size=4)]),
            fixture.argloc("custom")]
        args = [fixture.argument(location) for location in locations]
        args[0].values["ea"] = 0xffffffffffffffff
        info = fixture.Borrowed(callee=0xffffffffffffffff, call_spd=-16, stkargs_top=8, cc=48, solid_args=8,
            return_type=fixture.type(), return_argloc=fixture.argloc(), args=fixture.vector(args))
        mba, _, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, callinfo=info)])

        def run(ea, failed):
            fixture.emit("calls_done", mba)
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        micro = diagnostics["events"][0]["instructions"][0]["callsite"]["microcode_callinfo"]
        values = [arg["location"] for arg in micro["parameters"]["arguments"]]
        self.assertEqual(micro["callee_status"], "unobserved")
        self.assertEqual(micro["parameters"]["arguments"][0]["initialized_ea_status"], "unobserved")
        self.assertEqual(values[0]["reason"], "no_location")
        self.assertEqual((values[1]["register"], values[1]["register_offset"]), (1, 2))
        self.assertEqual(values[2]["second_register"], 2)
        self.assertEqual(values[3]["stack_offset"], -8)
        self.assertEqual((values[4]["register"], values[4]["offset"]), (5, -12))
        self.assertEqual(values[5]["address"], "0x140008000")
        self.assertEqual(values[6]["parts"][1]["argument_offset"], 4)
        self.assertEqual(values[6]["parts"][1]["stack_offset"], 4)
        self.assertEqual(values[7]["reason"], "custom_location_payload_not_observed")
        self.assertTrue(diagnostics["diagnostics_complete"])
        json.dumps(diagnostics)
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))

    def test_missing_details_and_sdk_exceptions_remain_explicit_without_overwriting_failure(self):
        for issue in ("details_false", "formatter_raised", "callinfo_raised", "owner_missing", "parents_raised"):
            with self.subTest(issue=issue):
                fixture = self.fixture()
                address, callee = 0x1400010a0, 0x140003000
                fixture.original(address, operands=[fixture.operand(7, addr=callee)])
                fixture.owner(address, start=address, tail=True, parents=[0x140002000, 0x140001000])
                fixture.stored_types[(callee, None)] = fixture.type(details=False if issue == "details_false" else fixture.details())
                if issue == "formatter_raised":
                    fixture.type_api.print_tinfo = lambda *args: (_ for _ in ()).throw(RuntimeError("formatter unavailable"))
                if issue == "owner_missing":
                    del fixture.functions[0x140002000]
                if issue == "parents_raised":
                    fixture.funcs_api.func_parent_iterator_t = lambda: (_ for _ in ()).throw(RuntimeError("parent API unavailable"))
                insn = fixture.instruction(0xf1c00040)
                if issue == "callinfo_raised":
                    insn.values["d"] = RuntimeError("callinfo unavailable")
                mba, block, _ = fixture.mba(instructions=[insn])
                fixture.implementation.run = lambda ea, failed: fixture.emit("build_callinfo", block, None) and None
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                call = diagnostics["events"][0]["instructions"][0]["callsite"]
                op = call["original_instruction"]["operands"][0]
                if issue == "details_false":
                    self.assertEqual(op["address_stored_tinfo"]["without_argloc_calculation"]["reason"], "get_func_details_returned_false")
                    self.assertEqual(op["address_stored_tinfo"]["with_argloc_calculation"]["reason"], "get_func_details_returned_false")
                    self.assertTrue(diagnostics["diagnostics_complete"])
                else:
                    self.assertFalse(diagnostics["diagnostics_complete"])
                    self.assertGreater(diagnostics["diagnostic_error_count"], 0)
                if issue == "callinfo_raised":
                    self.assertEqual(call["microcode_callinfo"]["status"], "diagnostic_error")
                    self.assertEqual(op["address_stored_tinfo"]["without_argloc_calculation"]["status"], "observed")
                if issue == "owner_missing":
                    context = call["original_instruction"]["function_context"]
                    self.assertFalse(context["owners_complete"])
                    self.assertEqual(context["owners"][0]["status"], "diagnostic_error")
                    self.assertEqual(context["owners"][1]["spd_before"]["value"], -12)
                self.assertEqual((fixture.failure.errea, fixture.failure.code, fixture.failure.str),
                                 (0xf1c00040, -12, "original failure detail"))
                json.dumps(diagnostics)

    def test_call_type_owner_argument_location_and_total_text_limits_are_explicit(self):
        cases = (("calls_per_event", 1, "call_records"), ("calls_total", 1, "call_records"),
                 ("types_total", 1, "type_records"), ("arguments_per_type", 1, "argument_records"),
                 ("arguments_total", 1, "argument_records"), ("function_owners", 1, "function_owners"),
                 ("argloc_parts", 1, "argloc_parts"), ("argloc_depth", 0, "argloc_depth"),
                 ("argloc_records_total", 1, "argloc_records"), ("text_characters_total", 8, "detail_text"))
        for limit, maximum, truncated in cases:
            with self.subTest(limit=limit):
                fixture = self.fixture()
                fixture.Trace.DEFAULT_LIMITS[limit] = maximum
                address, callee = 0x1400010a0, 0x140003000
                fixture.original(address, operands=[fixture.operand(7, addr=callee)])
                fixture.owner(address, start=address, tail=True, parents=[0x140002000, 0x140001000])
                parts = [fixture.argloc("stack", stack_offset=index * 4) for index in range(2)]
                location = fixture.argloc("scattered", parts=parts)
                details = fixture.details([fixture.argument(location), fixture.argument(location)])
                fixture.stored_types[(address, 0)] = fixture.type(details=details)
                fixture.stored_types[(callee, None)] = fixture.type(details=details)
                mba, block, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040), fixture.instruction(0xf1c00040)])
                fixture.implementation.run = lambda ea, failed: fixture.emit("build_callinfo", block, None) and None
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                self.assertTrue(diagnostics["truncation"][truncated])
                self.assertFalse(diagnostics["diagnostics_complete"])
                self.assertEqual(fixture.failure.code, -12)
                for counter, bound in (("call_records", "calls_total"), ("type_records", "types_total"),
                        ("argument_records", "arguments_total"), ("argloc_records", "argloc_records_total"),
                        ("text_characters", "text_characters_total")):
                    self.assertLessEqual(diagnostics[counter], diagnostics["limits"][bound])
                if truncated == "function_owners":
                    context = diagnostics["events"][0]["instructions"][0]["callsite"]["original_instruction"]["function_context"]
                    self.assertFalse(context["owners_complete"])
                    self.assertEqual(context["owners"][0]["owner_ea"], "0x140002000")
                json.dumps(diagnostics)

    def test_observer_sdk_calls_are_read_only_and_never_reference_out_type(self):
        fixture = self.fixture()
        tree = ast.parse(fixture.source.read_text(encoding="utf-8"))
        classes = [node for node in tree.body if isinstance(node, ast.ClassDef)
                   and node.name in {"_FrozenInsnVisitor", "StaticMicrocodeTrace"}]
        allowed = {"ida_typeinf": {"print_tinfo", "func_type_data_t", "tinfo_t"},
            "ida_nalt": {"get_tinfo", "get_op_tinfo"},
            "ida_funcs": {"get_func", "get_fchunk", "is_func_tail", "func_parent_iterator_t", "get_func_chunknum"},
            "ida_ua": {"insn_t", "decode_insn"},
            "ida_bytes": {"is_loaded", "is_code", "get_full_flags", "get_item_head", "get_item_size"},
            "ida_idp": {"is_call_insn", "is_indirect_jump_insn"}, "idautils": {"Chunks", "Heads"}}
        for node in (value for owner in classes for value in ast.walk(owner)):
            if isinstance(node, ast.Name):
                self.assertNotEqual(node.id, "out_type")
            if isinstance(node, ast.Attribute) and isinstance(node.ctx, ast.Store):
                self.assertIsInstance(node.value, ast.Name)
                if node.value.id == "trace":
                    self.assertEqual(node.attr, "instruction_records")
                else:
                    self.assertEqual(node.value.id, "self")
            if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Attribute):
                continue
            function = node.func
            if isinstance(function.value, ast.Name) and function.value.id in allowed:
                self.assertIn(function.attr, allowed[function.value.id])
            self.assertNotIn(function.attr, {"dump", "guess_cc", "guess_tinfo", "set_type", "set_tinfo",
                                           "set_op_tinfo", "set_auto_spd", "add_user_stkpnt", "mark_dirty"})

    def test_mba_frame_scalars_are_independent_and_frozen_before_callback_return(self):
        fixture = self.fixture()
        mba, _, _ = fixture.mba()
        del mba.values["frregs"]
        mba.values["fpd"] = RuntimeError("frame delta unavailable")

        def run(ea, failed):
            self.assertEqual(fixture.emit("microcode", mba), [0])
            mba.values["frsize"] = 999
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        frame = diagnostics["events"][0]["mba_frame"]
        self.assertEqual(frame["frsize"], {"status": "api_returned", "value": 32})
        self.assertEqual(frame["frregs"]["status"], "unobserved")
        self.assertEqual(frame["frregs"]["reason"], "sdk_attribute_unavailable")
        self.assertNotIn("value", frame["frregs"])
        self.assertEqual(frame["fpd"]["status"], "diagnostic_error")
        self.assertEqual(frame["fullsize"]["value"], 64)
        self.assertEqual(frame["stacksize"]["value"], 60)
        self.assertEqual(frame["inargoff"]["value"], 64)
        self.assertEqual(frame["minstkref"]["value"], -24)
        self.assertEqual(diagnostics["diagnostic_error_count"], 2)
        self.assertFalse(diagnostics["diagnostics_complete"])
        self.assertEqual(diagnostics["events"][0]["instructions"][0]["mapping_status"], "api_returned")
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_inline_callbacks_observe_both_real_parent_calls_with_explicit_shared_owners_and_frames(self):
        fixture = self.fixture()
        parent, helper, other = 0x10011818, 0x10012f87, 0x10012eb5
        first, second = 0x100118c9, 0x10012ea9
        fixture.original(first, operands=[fixture.operand(7, addr=helper)])
        fixture.original(second, operands=[fixture.operand(7, addr=helper)])
        fixture.owner(first, start=parent)
        fixture.owner(second, start=0x10012e8c, tail=True, parents=[other, parent], primary=other)
        fixture.functions[parent].values.update(frsize=0, frregs=4, argsize=0, fpd=0)
        fixture.functions[other].values["frsize"] = 20
        del fixture.functions[other].values["argsize"]
        fixture.stored_types[(helper, None)] = fixture.type(details=fixture.details([fixture.argument()]))
        mba, _, _ = fixture.mba(entry=parent, instructions=[fixture.instruction(first), fixture.instruction(second)],
                                mappings={first:first, second:second})
        fixture.chunks[helper] = [(helper, helper + 9), (helper + 9, helper + 60)]
        mbr = fixture.Borrowed(pfn=fixture.Borrowed(start_ea=helper, flags=0x20000),
                              ranges=[fixture.Borrowed(start_ea=helper, end_ea=helper + 60)])

        def run(ea, failed):
            self.assertEqual(fixture.emit("inlining_func", fixture.Borrowed(mba=mba), 21, mbr), [0])
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(parent, fixture.failure, diagnostics)
        contexts = [i["callsite"]["original_instruction"]["function_context"] for i in diagnostics["events"][0]["instructions"]]
        self.assertEqual(contexts[1]["primary_owner_ea"], hex(other))
        parent_snapshots = [owner for context in contexts for owner in context["owners"] if owner["owner_ea"] == hex(parent)]
        self.assertEqual(len(parent_snapshots), 2)
        self.assertTrue(all(owner["frame"]["frsize"]["value"] == 0 for owner in parent_snapshots))
        self.assertEqual(contexts[1]["owners"][0]["frame"]["frsize"]["value"], 20)
        self.assertEqual(contexts[1]["owners"][0]["frame"]["argsize"]["status"], "unobserved")
        self.assertIn(("spd_before", parent, first), fixture.sp_reads)
        self.assertIn(("spd_before", parent, second), fixture.sp_reads)
        self.assertTrue(all(ea in {first, second} for name, owner, ea in fixture.sp_reads))
        self.assertTrue(all((name, parent, second) in fixture.sp_reads
                            for name in ("spd_before", "effective_spd", "sp_change_point_delta")))
        self.assertFalse(diagnostics["diagnostics_complete"])
        self.assertEqual(diagnostics["events"][0]["mba_frame"]["frsize"]["value"], 32)
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_early_inline_points_distinguish_live_actual_mapped_and_nearest_points_without_reading_opaque_payloads(self):
        fixture = self.fixture()
        actual, mapped, helper = 0xf1c00040, 0x1400010a0, 0x140002000
        fixture.original(helper)
        fixture.original(helper + 4)
        parent, _, _ = fixture.mba(instructions=[fixture.instruction(actual, opcode=5)])
        owner = fixture.Borrowed(start_ea=helper, flags=0x20000)
        mbr = fixture.Borrowed(pfn=owner, ranges=[])
        child, _, _ = fixture.mba(entry=helper, instructions=[])
        child.values["mbr"] = mbr
        child.values["for_all_insns"] = MagicMock(side_effect=AssertionError("microcode not generated"))
        child.values["stack_points"][helper + 4] = fixture.Borrowed(ea=helper + 4, spd=-4)
        parent.values["stack_points"].update({actual: fixture.Borrowed(ea=actual, spd=-48),
                                             mapped: fixture.Borrowed(ea=mapped - 1, spd=-36)})
        opaque = fixture.Borrowed()
        opaque.valid = False
        cdg = fixture.Borrowed(mba=parent)

        def run(ea, failed):
            self.assertEqual(fixture.emit("stkpnts", parent, opaque), [0])
            self.assertEqual(fixture.emit("inlining_func", cdg, 3, mbr), [0])
            self.assertEqual(fixture.emit("stkpnts", child, opaque), [0])
            self.assertEqual(fixture.emit("prolog", child, opaque, opaque, 123), [0])
            self.assertEqual(fixture.emit("inlined_func", cdg, 3, mbr, 4, 7), [0])
            self.assertEqual(fixture.emit("stkpnts", parent, opaque), [0])
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        events = diagnostics["events"]
        self.assertEqual([event["phase"] for event in events],
                         ["stkpnts", "inlining_func", "stkpnts", "prolog", "inlined_func", "stkpnts"])
        self.assertTrue(diagnostics["diagnostics_complete"])
        self.assertNotIn("inline_context", events[0])
        self.assertNotIn("inline_context", events[5])
        self.assertEqual(events[2]["instructions"], [])
        self.assertEqual(events[2]["inline_context"]["source_block"], 3)
        self.assertEqual(events[2]["inline_context"]["owner_ea"], hex(helper))
        self.assertEqual(events[2]["inline_context"]["source_call"]["api_mapped_ea"], hex(mapped))
        self.assertEqual(events[2]["mba_entry_ea"], hex(helper))
        points = {point["query_ea"]: point for point in events[2]["stack_points"]}
        self.assertEqual(points[hex(helper + 4)]["spd"], -4)
        self.assertEqual(points[hex(helper)]["reason"], "locate_stkpnt_returned_null")
        self.assertEqual(events[3]["decompilation_flags"], 123)
        self.assertEqual(events[3]["mba_boundaries"]["last_prolog_ea"]["value"], helper + 4)
        self.assertEqual(events[2]["stack_points_payload"]["reason"], "opaque_sdk_stkpnts")
        instruction = events[1]["instructions"][0]
        queries = instruction["stack_point_queries"]
        actual_point = events[1]["stack_points"][queries["instruction_ea"]["record"]]
        original_point = events[1]["stack_points"][queries["mapped_original_ea"]["record"]]
        self.assertEqual(actual_point["query_ea"], hex(actual))
        self.assertEqual(actual_point["spd"], -48)
        self.assertTrue(actual_point["point_ea_matches_query"])
        self.assertEqual(original_point["point_ea"], hex(mapped - 1))
        self.assertFalse(original_point["point_ea_matches_query"])
        self.assertEqual(fixture.sp_reads, [])
        child.values["for_all_insns"].assert_not_called()
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        self.assertTrue(all(reference() is None for reference in fixture.visitors))
        self.assertEqual(diagnostics["open_inline_contexts"], 0)
        json.dumps(diagnostics)

    def test_nested_inline_scope_returns_to_outer_context_and_never_retains_sdk_objects(self):
        fixture = self.fixture()
        mba, _, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, opcode=5)])
        cdg = fixture.Borrowed(mba=mba)
        fixture.chunks[0x140003000] = [(0x140003000, 0x140003008)]
        outer = fixture.Borrowed(pfn=fixture.Borrowed(start_ea=0x140002000, flags=0x20000), ranges=[])
        inner = fixture.Borrowed(pfn=fixture.Borrowed(start_ea=0x140003000, flags=0x20000), ranges=[])

        def run(ea, failed):
            fixture.emit("inlining_func", cdg, 3, outer)
            fixture.emit("inlining_func", cdg, 3, inner)
            fixture.emit("stkpnts", mba, None)
            fixture.emit("inlined_func", cdg, 3, inner, 8, 12)
            fixture.emit("prolog", mba, None, None, 0)
            fixture.emit("inlined_func", cdg, 3, outer, 4, 8)
            fixture.emit("stkpnts", mba, None)
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        contexts = [event.get("inline_context", {}) for event in diagnostics["events"]]
        self.assertEqual([context.get("depth") for context in contexts], [1, 2, 2, 2, 1, 1, None])
        self.assertEqual(contexts[2]["owner_ea"], "0x140003000")
        self.assertEqual(contexts[4]["owner_ea"], "0x140002000")
        self.assertEqual(diagnostics["open_inline_contexts"], 0)
        self.assertTrue(diagnostics["diagnostics_complete"])

        def assert_plain(value):
            self.assertNotIsInstance(value, fixture.Borrowed)
            if isinstance(value, dict):
                for child in value.values(): assert_plain(child)
            elif isinstance(value, (list, tuple, set)):
                for child in value: assert_plain(child)

        assert_plain(fixture.instances[0].__dict__)
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_raw_stack_offsets_use_actual_owning_mba_even_when_entries_frames_and_printed_names_match(self):
        fixture = self.fixture()
        parent, block, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, opcode=5)])
        foreign, _, _ = fixture.mba(instructions=[])
        foreign.values.update(stkoff_vd2ida=lambda off: off - 80, stkoff_ida2vd=lambda off: off + 80)
        source = fixture.Borrowed(mba=foreign, off=68)
        destination = fixture.Borrowed(mba=parent, off=12)
        instruction = block.tail
        instruction.values.update(
            l=fixture.micro_operand(9, a=fixture.micro_operand(5, s=source)),
            r=fixture.micro_operand(2, unsigned_value=lambda: 0xffffffe0, signed_value=lambda: -32),
            d=fixture.micro_operand(5, s=destination))
        block.values.update(npred=lambda: 2, pred=lambda i: [21, 29][i],
                            nsucc=lambda: 1, succ=lambda i: [34][i], maxbsp=44)

        def run(ea, failed):
            fixture.emit("microcode", parent)
            source.values["off"] = 999
            destination.values["off"] = 888
            fixture.expire()

        fixture.implementation.run = run
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        event = diagnostics["events"][0]
        operands = event["instructions"][0]["operands"]
        foreign_stack = operands["l"]["address_operand"]["stack"]
        parent_stack = operands["d"]["stack"]
        self.assertEqual(foreign_stack["decompiler_offset"], 68)
        self.assertEqual(parent_stack["decompiler_offset"], 12)
        self.assertEqual(foreign_stack["coordinate_conversion"]["ida_offset"], -12)
        self.assertEqual(parent_stack["coordinate_conversion"]["ida_offset"], -4)
        self.assertTrue(foreign_stack["coordinate_conversion"]["roundtrip_matches"])
        self.assertEqual(len(event["mba_objects"]), 2)
        foreign_mba = event["mba_objects"][foreign_stack["mba_reference"]]
        parent_mba = event["mba_objects"][parent_stack["mba_reference"]]
        self.assertEqual(foreign_mba["entry_ea"], parent_mba["entry_ea"])
        self.assertEqual(foreign_mba["frame"], parent_mba["frame"])
        self.assertFalse(foreign_mba["same_as_observed_mba"]["value"])
        self.assertTrue(parent_mba["same_as_observed_mba"]["value"])
        self.assertEqual((operands["r"]["unsigned_value"], operands["r"]["signed_value"]), (0xffffffe0, -32))
        cfg = event["blocks"][0]
        self.assertEqual((cfg["predecessors"], cfg["successors"], cfg["maxbsp"]), ([21, 29], [34], 44))
        self.assertEqual(cfg["tail"]["ea"], "0xf1c00040")
        self.assertEqual(cfg["tail"]["api_mapped_ea"], "0x1400010a0")
        self.assertTrue(diagnostics["diagnostics_complete"])
        self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
        json.dumps(diagnostics)

    def test_nested_typed_esp_to_ebp_definitions_and_conversion_mismatches_stay_numeric(self):
        fixture = self.fixture()
        parent, block, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, opcode=5)])
        parent.values["stkoff_ida2vd"] = lambda off: off + 17
        nested = fixture.instruction(0x140001030, opcode=4,
            left=fixture.micro_operand(1, r=4), destination=fixture.micro_operand(1, r=5))
        block.tail.values.update(l=fixture.micro_operand(4, d=nested),
            d=fixture.micro_operand(5, s=fixture.Borrowed(off=12, mba=parent)))
        fixture.implementation.run = lambda ea, failed: fixture.emit("microcode", parent) and None
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        operands = diagnostics["events"][0]["instructions"][0]["operands"]
        definition = operands["l"]["instruction"]["operands"]
        self.assertEqual((definition["l"]["register"], definition["l"]["register_name"]), (4, "esp"))
        self.assertEqual((definition["d"]["register"], definition["d"]["register_name"]), (5, "ebp"))
        conversion = operands["d"]["stack"]["coordinate_conversion"]
        self.assertEqual(conversion["status"], "api_returned")
        self.assertEqual(conversion["roundtrip_decompiler_offset"], 13)
        self.assertFalse(conversion["roundtrip_matches"])
        self.assertTrue(diagnostics["diagnostics_complete"])

    def test_stack_lookup_and_conversion_errors_preserve_raw_offsets_and_original_failure(self):
        fixture = self.fixture()
        parent, block, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, opcode=5)])
        parent.values["stack_points"][0xf1c00040] = RuntimeError("internal point unavailable")
        parent.values["stkoff_vd2ida"] = lambda off: (_ for _ in ()).throw(RuntimeError("conversion unavailable"))
        block.tail.values["d"] = fixture.micro_operand(5, s=fixture.Borrowed(off=12, mba=parent))
        fixture.implementation.run = lambda ea, failed: fixture.emit("microcode", parent) and None
        diagnostics = {}
        fixture.observe(0x140001000, fixture.failure, diagnostics)
        event = diagnostics["events"][0]
        stack = event["instructions"][0]["operands"]["d"]["stack"]
        self.assertEqual(stack["decompiler_offset"], 12)
        self.assertEqual(stack["coordinate_conversion"]["status"], "diagnostic_error")
        query = event["instructions"][0]["stack_point_queries"]["instruction_ea"]
        self.assertEqual(event["stack_points"][query["record"]]["status"], "diagnostic_error")
        self.assertFalse(diagnostics["diagnostics_complete"])
        self.assertEqual(fixture.failure.errea, 0xf1c00040)
        self.assertEqual(fixture.failure.code, -12)
        json.dumps(diagnostics)

    def test_coordinate_observation_budgets_mark_truncation_without_rewriting_inputs_or_hook_results(self):
        cases = [("operand_records_total", 2, "operand_records"), ("operand_depth", 0, "operand_depth"),
                 ("stack_points_per_event", 1, "stack_point_records"), ("stack_points_total", 1, "stack_point_records"),
                 ("blocks_per_event", 0, "block_records"), ("blocks_total", 0, "block_records"),
                 ("cfg_edges", 1, "cfg_edges"), ("mba_objects_per_event", 0, "mba_objects")]
        for limit, maximum, truncated in cases:
            with self.subTest(limit=limit):
                fixture = self.fixture()
                fixture.Trace.DEFAULT_LIMITS = {**fixture.Trace.DEFAULT_LIMITS, limit: maximum}
                parent, block, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, opcode=5)])
                block.tail.values["l"] = fixture.micro_operand(9, a=fixture.micro_operand(5,
                    s=fixture.Borrowed(off=12, mba=parent)))
                block.values.update(npred=lambda: 2, pred=lambda i: [21, 29][i])

                def run(ea, failed):
                    self.assertEqual(fixture.emit("microcode", parent), [0])
                    fixture.expire()

                fixture.implementation.run = run
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                self.assertTrue(diagnostics["truncation"][truncated])
                self.assertFalse(diagnostics["diagnostics_complete"])
                self.assertEqual(fixture.failure.errea, 0xf1c00040)
                self.assertEqual(diagnostics["events"][0]["instructions"][0]["ea"], "0xf1c00040")
                self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
                json.dumps(diagnostics)

    def test_inline_depth_and_head_probe_limits_are_bounded_and_restore_scope(self):
        for limit, maximum, key in (("inline_context_depth", 0, "inline_context_depth"), ("probe_heads", 1, "probe_heads")):
            with self.subTest(limit=limit):
                fixture = self.fixture()
                fixture.Trace.DEFAULT_LIMITS = {**fixture.Trace.DEFAULT_LIMITS, limit: maximum}
                fixture.original(0x140002000)
                fixture.original(0x140002004)
                parent, _, _ = fixture.mba(instructions=[fixture.instruction(0xf1c00040, opcode=5)])
                cdg = fixture.Borrowed(mba=parent)
                mbr = fixture.Borrowed(pfn=fixture.Borrowed(start_ea=0x140002000, flags=0x20000), ranges=[])

                def run(ea, failed):
                    self.assertEqual(fixture.emit("inlining_func", cdg, 3, mbr), [0])
                    self.assertEqual(fixture.emit("stkpnts", parent, None), [0])
                    self.assertEqual(fixture.emit("inlined_func", cdg, 3, mbr, 4, 7), [0])
                    fixture.expire()

                fixture.implementation.run = run
                diagnostics = {}
                fixture.observe(0x140001000, fixture.failure, diagnostics)
                self.assertTrue(diagnostics["truncation"][key])
                self.assertFalse(diagnostics["diagnostics_complete"])
                self.assertEqual((diagnostics["open_inline_contexts"], diagnostics["inline_context_overflow"]), (0, 0))
                self.assertLessEqual(len(diagnostics["events"][0]["inline_event"]["probe_eas"]), maximum if key == "probe_heads" else 512)
                self.assertTrue(all(not value.expired_reads for value in fixture.borrowed))
                json.dumps(diagnostics)

    def test_export_diagnostic_registration_failure_preserves_original_pseudocode_pass_or_failure(self):
        import traceback

        for success in (False, True):
            with self.subTest(success=success), tempfile.TemporaryDirectory() as directory:
                fixture = self.fixture(hook_issue="rejected")
                root = Path(directory)
                output = root / "output"
                output.mkdir()
                manifest = {"failures": [], "decoded_thunks": [], "functions": []}
                ea = 0x140001000
                func = SimpleNamespace(start_ea=ea, end_ea=ea + 2, flags=0)
                fixture.api.decompile = lambda ea, failed: (SimpleNamespace(entry_ea=ea,
                    get_pseudocode=lambda: [SimpleNamespace(line="return;")], get_lvars=lambda: [],
                    mba=SimpleNamespace(entry_ea=ea), get_stkoff_delta=lambda: 0) if success else None)
                typed_values = {"__file__": str(fixture.source.with_name("capture_values.py"))}
                exec(compile(fixture.source.with_name("capture_values.py").read_text(encoding="utf-8"),
                             str(fixture.source.with_name("capture_values.py")), "exec"), typed_values)
                namespace = {**fixture.namespace, "base": 0x140000000, "root": root, "output": output,
                    "capture_values": SimpleNamespace(local_variables=typed_values["local_variables"]),
                    "functions": output / "functions", "manifest": manifest, "captured": {}, "table": None,
                    "shared_owner_rvas": set(), "selection": {}, "hashlib": hashlib, "json": json, "traceback": traceback,
                    "file_bytes": lambda rva, size: b"\x90\xc3"[:size], "save": lambda path, row: path.write_text(json.dumps(row)),
                    "ida_funcs": SimpleNamespace(get_func=lambda address: func, get_func_name=lambda address: "fixture",
                                                 get_func_cmt=lambda function, repeatable: None),
                    "ida_bytes": SimpleNamespace(get_bytes=lambda address, size: b"\x90\xc3"[:size]),
                    "idautils": SimpleNamespace(Chunks=lambda address: [(ea, ea + 2)], FuncItems=lambda address: [],
                                                XrefsTo=lambda address, flags: []),
                    "ida_gdl": SimpleNamespace(FlowChart=lambda function: []),
                    "idc": SimpleNamespace(get_type=lambda address: None), "ida_lines": SimpleNamespace(tag_remove=lambda text: text)}
                capture = next(node for node in ast.parse(fixture.source.read_text(encoding="utf-8")).body
                               if isinstance(node, ast.FunctionDef) and node.name == "capture")
                export = next(node for node in capture.body if isinstance(node, ast.FunctionDef) and node.name == "export")
                exec(compile(ast.Module(body=[export], type_ignores=[]), str(fixture.source), "exec"), namespace)
                row = namespace["export"](ea)
                self.assertEqual(row["hexrays_diagnostics"]["hook_registered"], False)
                self.assertEqual(manifest["functions"][0]["pseudocode_captured"], success)
                self.assertEqual(len(manifest["failures"]), 0 if success else 1)
                if success:
                    self.assertEqual(row["pseudocode"]["lines"], [{"line": 0, "text": "return;"}])
                else:
                    self.assertIsNone(row["pseudocode"])
                    error = manifest["failures"][0]
                    self.assertEqual(error["stage"], "hexrays")
                    self.assertEqual(error["hexrays_failure"], {"code": -12, "address": "0xf1c00040", "rva": None,
                        "description": "call analysis failed", "detail": "original failure detail"})
                    self.assertIn("Hex-Rays: call analysis failed", error["error"])
                    self.assertNotIn("rejected diagnostic hooks", error["error"])


class RetainedFunctionChainTests(unittest.TestCase):
    def capture_chain(self, *, table, branches):
        source = Path(queue.__file__).with_name("ida") / "supplement.py"
        capture = next(node for node in ast.parse(source.read_text(encoding="utf-8")).body
                       if isinstance(node, ast.FunctionDef) and node.name == "capture")
        loop = next(node for node in capture.body if isinstance(node, ast.For)
                    and ast.unparse(node.iter) == "selection['targets']")
        base, entry = 0x400000, 0x492ac0
        target = {"address": hex(entry), "rva": hex(entry - base), "index": 0}
        if table:
            target["slot_rva"] = "0x3000"
        manifest, exported, queried = {"failures": [], "chains": []}, [], []

        def export(ea):
            exported.append(ea)
            if not table and ea != entry:
                raise ValueError("Entry is an interior block of the retained complete function")

        def jump(ea):
            queried.append(ea)
            return (branches[ea],) if ea in branches else None

        namespace = {"selection": {"targets": [target]}, "base": base, "table": table,
                     "manifest": manifest, "export": export, "jump_target": jump,
                     "traceback": traceback, "save": lambda *args: None, "output": Path("fixture")}
        exec(compile(ast.Module(body=[loop], type_ignores=[]), str(source), "exec"), namespace)
        return manifest, exported, queried

    def test_retained_function_entry_jump_keeps_all_chunks_without_interior_export(self):
        manifest, exported, queried = self.capture_chain(table=None, branches={0x492ac0: 0x492ae6})
        self.assertEqual(exported, [0x492ac0])
        self.assertEqual(queried, [])
        self.assertEqual(manifest["failures"], [])
        self.assertTrue(manifest["chains"][0]["complete"])

    def test_pointer_table_still_follows_complete_static_jump_chain_and_rejects_cycles(self):
        for cycle in (False, True):
            with self.subTest(cycle=cycle):
                branches = {0x492ac0: 0x493000}
                if cycle:
                    branches[0x493000] = 0x492ac0
                manifest, exported, queried = self.capture_chain(table=True, branches=branches)
                self.assertEqual(exported, [0x492ac0, 0x493000])
                self.assertEqual(queried, exported)
                self.assertEqual(manifest["chains"][0]["complete"], not cycle)
                self.assertEqual(bool(manifest["failures"]), cycle)


class FreshInlineControllerTests(unittest.TestCase):
    def fixture(self, *, issue=None):
        source = Path(queue.__file__).with_name("ida") / "supplement.py"
        registered, instances, sdk_calls, lifecycle = [], [], [], []
        base, parent, helper = 0x140000000, 0x140001040, 0x140001000
        code = bytes.fromhex("554889e5c9c3")
        chunks = [{"start": hex(helper), "end": hex(helper + len(code)), "raw_hex": code.hex(),
                   "sha256": hashlib.sha256(code).hexdigest()}]
        reviewed = [{"rva": "0x1000", "expected_chunks": chunks,
                     "flag_names": ["INLINE_EXTFRAME", "INLINE_DONTCOPY"],
                     "callsites": [{"parent_rva": "0x1040", "callsite_rva": hex(rva), "target_rva": "0x1000",
                                    "raw_hex": raw, "parent_function": {"path": "fixture", "bytes": 1, "sha256": "a" * 64}}
                                   for rva, raw in [(0x1040, "e8bbffffff"), (0x1046, "e8b5ffffff")]]}]
        call_bytes = {parent: bytes.fromhex("e8bbffffff"), parent + 6: bytes.fromhex("e8b5ffffff")}
        function = SimpleNamespace(start_ea=helper, flags=0x40000)
        blocks = [SimpleNamespace(serial=index, tail=SimpleNamespace(ea=at, opcode=17, l=SimpleNamespace(t=6, g=helper)))
                  for index, at in enumerate(call_bytes)]
        mba = SimpleNamespace(entry_ea=parent, qty=len(blocks), get_mblock=lambda index: blocks[index])
        cdg = SimpleNamespace(mba=mba)

        class Hooks:
            def __init__(self): instances.append(self)

            def hook(self):
                lifecycle.append("hook")
                if issue == "register_rejected": return False
                registered.append(self)
                if issue == "register_raised": raise ValueError("partial registration")
                return True

            def unhook(self):
                lifecycle.append("unhook")
                if issue == "unhook_raised": raise ValueError("cleanup raised")
                if issue == "unhook_rejected": return False
                if self in registered: registered.remove(self)
                return True

        def inline(cdg_arg, serial, ranges, decomp_flags, flags):
            self.assertIs(cdg_arg, cdg)
            self.assertIs(ranges.pfn, function)
            self.assertEqual((decomp_flags, flags), (0x80, 3))
            tail = blocks[serial].tail
            sdk_calls.append(tail.ea)
            if issue == "sdk_error": return -12
            if issue == "sdk_raised": raise ValueError("SDK exception")
            tail.opcode = 0
            blocks.reverse()
            for index, block in enumerate(blocks): block.serial = index
            return 0

        mba.inline_func = inline
        api = SimpleNamespace(Hexrays_Hooks=Hooks, INLINE_EXTFRAME=1, INLINE_DONTCOPY=2, MERR_OK=0,
                              MERR_CANCELED=-99, m_call=17, mop_v=6, mba_ranges_t=SimpleNamespace)
        namespace = {"ida_hexrays": api, "hashlib": hashlib,
                     "ida_funcs": SimpleNamespace(FUNC_OUTLINE=0x40000, get_func=lambda ea: function),
                     "ida_bytes": SimpleNamespace(get_bytes=lambda ea, size: code if ea == helper else call_bytes[ea]),
                     "idautils": SimpleNamespace(Chunks=lambda ea: [(helper, helper + len(code))])}
        names = {"FreshInlineController", "decompile_with_fresh_inlines"}
        nodes = [node for node in ast.parse(source.read_text(encoding="utf-8")).body
                 if isinstance(node, (ast.ClassDef, ast.FunctionDef)) and node.name in names]
        exec(compile(ast.Module(body=nodes, type_ignores=[]), str(source), "exec"), namespace)
        token = object()

        def decompile(ea, failure, diagnostics):
            self.assertEqual(ea, parent)
            if issue == "missing_callback": return token
            for hook in list(registered):
                self.assertEqual(hook.begin_inlining(SimpleNamespace(mba=SimpleNamespace(entry_ea=helper)), 0), 0)
                result = hook.begin_inlining(cdg, 0x80)
                if result: return None
            return token

        namespace["decompile_with_diagnostics"] = decompile
        return SimpleNamespace(observe=namespace["decompile_with_fresh_inlines"], api=api, token=token,
            base=base, parent=parent, helper=helper, reviewed=reviewed, blocks=blocks, mba=mba, cdg=cdg,
            instances=instances, registered=registered, sdk_calls=sdk_calls, lifecycle=lifecycle, function=function,
            namespace=namespace)

    def test_fresh_sdk_rebuild_rescans_blocks_and_preserves_distinct_original_calls(self):
        fixture = self.fixture()
        audit = {}
        result = fixture.observe(fixture.parent, object(), {}, fixture.reviewed, fixture.base, audit)
        self.assertIs(result, fixture.token)
        self.assertEqual(fixture.sdk_calls, [fixture.parent, fixture.parent + 6])
        self.assertEqual([row["block_serial_before"] for row in audit["calls"]], [0, 0])
        self.assertTrue(audit["complete"])
        self.assertEqual(fixture.lifecycle, ["hook", "unhook"])
        self.assertFalse(fixture.registered)
        self.assertFalse(fixture.instances[0].active)
        self.assertEqual(set(fixture.instances[0].__dict__), {"target_ea", "base", "helpers", "audit", "active", "entered"})
        json.dumps(audit)

    def test_missing_wrong_or_changed_original_call_never_invokes_sdk(self):
        for issue in ("missing", "opcode", "callee", "bytes", "outline", "chunks"):
            with self.subTest(issue=issue):
                fixture = self.fixture()
                if issue == "missing": fixture.blocks[0].tail.ea += 1
                elif issue == "opcode": fixture.blocks[0].tail.opcode = 18
                elif issue == "callee": fixture.blocks[0].tail.l.g += 1
                elif issue == "bytes": fixture.reviewed[0]["callsites"][0]["raw_hex"] = "e8baffffff"
                elif issue == "outline": fixture.function.flags = 0
                elif issue == "chunks": fixture.reviewed[0]["expected_chunks"][0]["raw_hex"] = "90"
                audit = {}
                with self.assertRaisesRegex(ValueError, "incomplete"):
                    fixture.observe(fixture.parent, object(), {}, fixture.reviewed, fixture.base, audit)
                self.assertFalse(fixture.sdk_calls)
                self.assertFalse(audit["complete"])
                self.assertTrue(audit["hook_unregistered"])

    def test_sdk_failures_and_hook_cleanup_cannot_report_completion(self):
        for issue in ("sdk_error", "sdk_raised", "unhook_rejected", "unhook_raised", "missing_callback",
                      "register_rejected", "register_raised"):
            with self.subTest(issue=issue):
                fixture = self.fixture(issue=issue)
                audit = {}
                with self.assertRaises(ValueError):
                    fixture.observe(fixture.parent, object(), {}, fixture.reviewed, fixture.base, audit)
                self.assertFalse(audit["complete"])
                self.assertFalse(fixture.instances[0].active)
                self.assertEqual(fixture.lifecycle[-1], "unhook")
                before = list(fixture.sdk_calls)
                fixture.instances[0].begin_inlining(fixture.cdg, 0x80)
                self.assertEqual(fixture.sdk_calls, before)


class ProcessWorkerTests(unittest.TestCase):
    def test_spawned_static_workers_preserve_unresolved_capture_and_input_failures(self):
        batch = queue.Batch(2, "IDA", verify_only=True, process_workers=True)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            source = root / ".analysis/inputs/static-only/native.dll"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"inert native input without a capture")
            jobs = [
                {"id": "missing-capture", "kind": "native", "input": analysis.rel(source),
                 "sha256": analysis.sha(source)},
                {"id": "changed-input", "kind": "native", "input": analysis.rel(source),
                 "sha256": "0" * 64},
            ]
            with batch.process_pool() as pool:
                results = [future.result(timeout=30) for future in
                           [pool.submit(queue.execute_process_task, job) for job in jobs]]
            self.assertEqual([row["status"] for row in results], ["unresolved", "failed"])
            self.assertEqual(results[0]["commands"], [])
            self.assertIn("No complete native capture", results[0]["error"])
            self.assertIn("Native original identity changed", results[1]["error"])
            self.assertTrue(all(row["worker_pid"] != queue.os.getpid() for row in results))
            self.assertFalse((root / queue.REPORT).exists())
            self.assertFalse((root / ".analysis/results/gaps.json").exists())

    def test_spawned_resume_verification_rejects_a_changed_nested_artifact(self):
        batch = queue.Batch(2, "IDA", verify_only=True, process_workers=True)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)):
            root = Path(directory)
            artifact = root / ".analysis/results/static/artifact.json"
            analysis.save(artifact, {"original": True})
            receipt = root / ".analysis/results/static/receipt.json"
            analysis.save(receipt, {"nested": {"evidence": queue.identity(artifact)}})
            task = {"id": "nested", "receipt": queue.identity(receipt)}
            with batch.process_pool() as pool:
                self.assertEqual(pool.submit(queue.check_retained_task, task).result(timeout=30), "nested")
                artifact.write_bytes(b"changed original")
                with self.assertRaisesRegex(ValueError, "Retained queue artifact changed"):
                    pool.submit(queue.check_retained_task, task).result(timeout=30)
            self.assertFalse((root / queue.REPORT).exists())

    def test_command_forwards_process_isolation_without_changing_retained_scope(self):
        args = argparse.Namespace(workers=2, ida_root="IDA", verify_only=False, resume=True,
                                  process_workers=True)
        with tempfile.TemporaryDirectory() as directory, patch.object(analysis, "ROOT", Path(directory)), \
             patch.object(queue, "Batch") as batch:
            batch.return_value.run.return_value = {"tasks": []}
            queue.run(args)
        batch.assert_called_once_with(2, "IDA", False, resume=True, process_workers=True)


if __name__ == "__main__":
    unittest.main()
