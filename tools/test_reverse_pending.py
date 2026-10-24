"""Pure queue tests; no vendor process or network is started."""
from concurrent.futures import ThreadPoolExecutor
from contextlib import redirect_stdout
from copy import deepcopy
from pathlib import Path
import argparse
import base64
import hashlib
import io
import json
import struct
import tempfile
import threading
import unittest
from unittest.mock import MagicMock, patch

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


if __name__ == "__main__":
    unittest.main()
