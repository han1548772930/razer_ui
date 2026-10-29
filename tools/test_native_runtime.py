"""Pure replay validation, fake C ABI lifecycle and process deadline tests."""
from __future__ import annotations

import copy
import ctypes
from contextlib import contextmanager, redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import native_runtime as native
import test_host as host_fixtures
from test_host import pe_fixture


class SimpleVersionEvidenceTests(unittest.TestCase):
    def fixture(self, root):
        import uuid
        raw = bytearray(2570440)
        raw[16:19] = b"abc"
        raw[0x1f8234:0x1f8244] = uuid.UUID("0edeaf3c-d36e-4e7e-9467-900b977dc4ff").bytes_le
        raw[0x1f8214:0x1f8224] = uuid.UUID("bfe24d59-6568-4179-8ae5-d9d53869a3e3").bytes_le
        digest = hashlib.sha256(raw).hexdigest()
        (root / "source.dll").write_bytes(raw)
        capture = root / f".analysis/results/native/{digest}"
        (capture / "functions").mkdir(parents=True)
        (capture / "capture.json").write_text(json.dumps({"input_sha256":digest,
            "input_bytes":len(raw), "ida_version":"test", "hexrays_version":"test"}), encoding="utf-8")
        function = {"rva":"0xf420", "pseudocode":{"lines":[{"text":"full body"}]},
            "assembly":["full assembly"], "chunks":[{"start":"0x180000010", "end":"0x180000013",
                "raw_hex":"616263", "sha256":hashlib.sha256(b"abc").hexdigest()}]}
        function_path = capture / "functions/f420.json"
        function_path.write_text(json.dumps(function), encoding="utf-8")
        pe = {"architecture":"x64", "unresolved":[], "image_base":"0x180000000",
              "sections":[{"rva":0,"file_offset":0,"bytes":len(raw)}],
              "exports":[{"names":[name],"rva":rva} for name,rva in (
                  ("simpleServiceInitialize","0xedf0"), ("simpleGetVersionInfo","0xf420"),
                  ("simpleServiceShutdown","0xf090"), ("simpleGetUserApps","0x10290"))]}
        return digest, pe, function, function_path

    def test_actual_chunk_bytes_guid_and_full_body_are_required(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            digest, pe, function, path = self.fixture(root)
            with patch.object(native, "_SIMPLE_SHA256", digest), \
                 patch.object(native, "_SIMPLE_INPUT", "source.dll"), \
                 patch.object(native, "_SIMPLE_FUNCTIONS", ("f420",)), \
                 patch.object(native, "inspect_pe", return_value=pe):
                receipt = native.verify_simple_version_source(root)
                self.assertEqual(receipt["abi"]["version_slot"], 8)
                self.assertTrue(receipt["functions"][0]["bytes_verified"])
                function["chunks"][0]["raw_hex"] = "616264"
                path.write_text(json.dumps(function), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "capture bytes differ"):
                    native.verify_simple_version_source(root)
                function["pseudocode"]["lines"] = []
                path.write_text(json.dumps(function), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "Complete native source body"):
                    native.verify_simple_version_source(root)

    def test_test_timeout_terminates_and_waits_for_exact_cargo_tree(self):
        from unittest.mock import Mock
        process = Mock(pid=12345, returncode=1)
        process.communicate.side_effect = [subprocess.TimeoutExpired("cargo", 180), ("partial", "failure")]
        termination = subprocess.CompletedProcess([], 0, "terminated", "")
        with patch.object(native.subprocess, "Popen", return_value=process), \
             patch.object(native.subprocess, "run", return_value=termination) as kill:
            result = native._run_service_version_test(["cargo", "test"], Path("."), {})
        self.assertTrue(result["timed_out"])
        self.assertTrue(result["worker_waited"])
        self.assertEqual(result["stdout"], "partial")
        self.assertEqual(kill.call_args.args[0], ["taskkill", "/PID", "12345", "/T", "/F"])
        process.wait.assert_called_once()


class SimpleComparisonTests(unittest.TestCase):
    def versions(self):
        return {"Simple Service Version": "1.2.2.10",
                "Elevation Service Version": {"state": "observed", "value": "2.3"}}

    def test_version_requires_stable_observation_and_exact_original_result(self):
        before = self.versions()
        original = {"result": True, "reason": "", "value": {
            "Simple Service Version": "1.2.2.10", "Elevation Service Version": "2.3"}}
        self.assertTrue(native._simple_comparison("version", before, original, before)["passed"])
        for changed in ({**original, "result": False}, {**original, "reason": "failed"},
                        {**original, "value": {}}, {**original, "result": 1}):
            self.assertFalse(native._simple_comparison("version", before, changed, before)["passed"])
        after = self.versions()
        after["Elevation Service Version"]["value"] = "2.4"
        self.assertFalse(native._simple_comparison("version", before, original, after)["passed"])
        for state in ("unobserved", "failed", "disconnected"):
            before["Elevation Service Version"] = {"state": state}
            self.assertFalse(native._simple_comparison("version", before, original, before)["passed"])

    def test_inventory_retains_empty_result_order_and_failure_distinctions(self):
        before = [{"name": name, "rust": {"result": True, "reason": "", "userApps": ""}}
                  for name in native._APP_FILTERS]
        before[0]["rust"]["userApps"] = '[{"name":"a","id":1},{"name":"b","id":2}]'
        original = [{"name": row["name"], "original": copy.deepcopy(row["rust"])} for row in before]
        original[0]["original"]["userApps"] = '[ {"id": 1, "name": "a"}, {"id": 2, "name": "b"} ]'
        self.assertTrue(native._simple_comparison("apps", before, original, before)["passed"])
        for key, value in (("userApps", "[]"), ("result", False), ("reason", "missing")):
            changed = copy.deepcopy(original)
            changed[1]["original"][key] = value
            self.assertFalse(native._simple_comparison("apps", before, changed, before)["passed"])
        changed = copy.deepcopy(original)
        changed[0]["original"]["userApps"] = '[{"name":"b","id":2},{"name":"a","id":1}]'
        self.assertFalse(native._simple_comparison("apps", before, changed, before)["passed"])
        self.assertFalse(native._simple_comparison("apps", before[:-1], original, before[:-1])["passed"])
        with self.assertRaises(json.JSONDecodeError):
            original[0]["original"]["userApps"] = "invalid"
            native._simple_comparison("apps", before, original, before)


class SimpleSessionTests(unittest.TestCase):
    def session(self, mode="version", replies=None, initialize=True, shutdown=True):
        from types import SimpleNamespace
        from unittest.mock import Mock
        calls = []
        def completion(name, enabled):
            def invoke(callback):
                calls.append(name)
                if enabled:
                    callback()
            return Mock(side_effect=invoke)
        def query(*args):
            calls.append(("query", args[0]) if len(args) == 2 else "query")
            for reply in replies if replies is not None else [(True, b"", b'{"version":"1"}')]:
                args[-1](*reply)
        library = SimpleNamespace(
            simpleServiceInitialize=completion("initialize", initialize),
            simpleServiceShutdown=completion("shutdown", shutdown),
            simpleGetVersionInfo=Mock(side_effect=query),
            simpleGetUserApps=Mock(side_effect=query))
        return library, calls

    def test_callbacks_are_owned_until_shutdown_and_query_strings_are_copied(self):
        library, calls = self.session()
        events = []
        with patch.object(native, "_SIMPLE_WORKER_OWNERS", []) as owners:
            result = native._simple_session(library, "version", events.append, timeout=0)
            self.assertIs(owners[0][0], library)
            self.assertEqual(len(owners[0][1]), 3)
            self.assertEqual(calls, ["initialize", "query", "shutdown"])
            self.assertEqual(result["value"], {"result": True, "reason": "", "value": {"version": "1"}})
            self.assertEqual(result["lifecycle"], events)

    def test_exact_app_filter_order_and_arguments_live_through_shutdown(self):
        library, calls = self.session(replies=[(True, b"", b"")])
        with patch.object(native, "_SIMPLE_WORKER_OWNERS", []) as owners:
            result = native._simple_session(library, "apps", lambda _: None, timeout=0)
            self.assertEqual(calls, ["initialize", *[("query", name.encode()) for name in native._APP_FILTERS], "shutdown"])
            self.assertEqual([row["name"] for row in result["value"]], list(native._APP_FILTERS))
            self.assertEqual([value for value in owners[0][1] if isinstance(value, bytes)],
                             [name.encode() for name in native._APP_FILTERS])

    def test_failed_queries_always_attempt_shutdown_and_preserve_callback_ownership(self):
        for replies, expected in (([], TimeoutError), ([(True, None, b"{}")], ValueError),
                                  ([(True, b"", None)], ValueError), ([(True, b"", b"invalid")], ValueError),
                                  ([(True, b"", b"{}"), (True, b"", b"{}")], ValueError)):
            with self.subTest(replies=replies), patch.object(native, "_SIMPLE_WORKER_OWNERS", []) as owners:
                library, calls = self.session(replies=replies)
                with self.assertRaises(expected):
                    native._simple_session(library, "version", lambda _: None, timeout=0)
                self.assertEqual(calls, ["initialize", "query", "shutdown"])
                self.assertEqual(len(owners[0][1]), 3)

    def test_query_and_cleanup_errors_both_survive(self):
        library, calls = self.session(replies=[], shutdown=False)
        with patch.object(native, "_SIMPLE_WORKER_OWNERS", []):
            with self.assertRaisesRegex(RuntimeError, "Query failed:.*shutdown failed:"):
                native._simple_session(library, "version", lambda _: None, timeout=0)
        self.assertEqual(calls, ["initialize", "query", "shutdown"])

    def test_initialization_timeout_keeps_owner_without_calling_uninitialized_service(self):
        library, calls = self.session(initialize=False)
        with patch.object(native, "_SIMPLE_WORKER_OWNERS", []) as owners:
            with self.assertRaisesRegex(TimeoutError, "initialize"):
                native._simple_session(library, "version", lambda _: None, timeout=0)
            self.assertEqual(calls, ["initialize"])
            self.assertEqual(len(owners[0][1]), 1)


MOUSE_PATH = r"\\?\HID#VID_1532&PID_00BF&MI_00#8&393c0d0b&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"
KEYBOARD_PATH = r"\\?\HID#VID_1532&PID_02A7&MI_03#b&25aecbb4&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"


def report(command=(2, 0, 192), payload=(1, 0), transaction=0):
    raw = [0] * 91
    raw[2], raw[6:9] = transaction, command
    raw[9:9 + len(payload)] = payload
    for value in raw[3:89]:
        raw[89] ^= value
    return raw


def trace(path=MOUSE_PATH, raw=None):
    return [{"path": path, "report": report() if raw is None else raw}]


def source_pe(bits=64):
    raw, directory = pe_fixture(bits)
    count = len(native.SOURCE_EXPORTS)
    struct.pack_into("<II", raw, directory, 0x1200, 0x400)
    struct.pack_into("<IIHHIIIIIII", raw, 0x400, 0, 0, 0, 0, 0, 1, count, count, 0x1250, 0x1280, 0x12c0)
    cursor = 0x500
    for index, (name, (rva, _)) in enumerate(native.SOURCE_EXPORTS.items()):
        struct.pack_into("<I", raw, 0x450 + index * 4, int(rva, 16))
        struct.pack_into("<I", raw, 0x480 + index * 4, cursor + 0xe00)
        struct.pack_into("<H", raw, 0x4c0 + index * 2, index)
        encoded = name.encode("ascii") + b"\0"
        raw[cursor:cursor + len(encoded)] = encoded
        cursor += len(encoded)
    return raw


class PacketValidationTests(unittest.TestCase):
    def test_exact_mouse_gets_and_correlations_remain_unchanged(self):
        rows = []
        for index, (command, payload) in enumerate(native._MOUSE_GETS.items()):
            rows.append({"path": MOUSE_PATH, "report": report(command, payload, index), "trace_index": index + 21})
        rows.append({"path": MOUSE_PATH, "report": report((1, 0, 184), [0], 3)})
        original = copy.deepcopy(rows)
        actual = native.validate_get_trace(rows)
        self.assertEqual(rows, original)
        self.assertEqual([row["get_report_id"] for row in actual], [0] * 9)
        self.assertEqual(actual[0]["report"], original[0]["report"])
        self.assertEqual(actual[0]["trace_index"], 21)

    def test_keyboard_uses_distinct_send_zero_and_get_interface_three(self):
        for command, payload in native._KEYBOARD_GETS.items():
            request = trace(KEYBOARD_PATH, report(command, payload))
            actual = native.validate_get_trace(request)[0]
            self.assertEqual(actual["report"][0], 0)
            self.assertEqual(actual["get_report_id"], 3)
            request[0]["get_report_id"] = 0
            with self.assertRaisesRegex(ValueError, "GET report selector"):
                native.validate_get_trace(request)

    def test_mouse_general_queries_match_current_source_without_payload_changes(self):
        for path in (MOUSE_PATH, MOUSE_PATH.replace("00BF", "00BE")):
            for command, payload in [
                ((22, 0, 130), []),
                ((4, 0, 135), []),
                ((2, 7, 128), [0]),
                ((2, 7, 132), [0]),
                ((2, 7, 131), []),
            ]:
                with self.subTest(path=path, command=command):
                    raw = report(command, payload, transaction=30)
                    actual = native.validate_get_trace(trace(path, raw))[0]
                    self.assertEqual(actual["report"], raw)
                    self.assertEqual(actual["get_report_id"], 0)

    def test_keyboard_metadata_gaming_and_active_profile_queries_match_source(self):
        for command, payload in [
            ((4, 0, 135), []),
            ((3, 0, 134), []),
            ((3, 3, 128), [0, 8]),
            ((2, 2, 160), [1]),
            ((4, 2, 161), [1]),
        ]:
            with self.subTest(command=command):
                raw = report(command, payload, transaction=30)
                actual = native.validate_get_trace(trace(KEYBOARD_PATH, raw))[0]
                self.assertEqual(actual["report"], raw)
                self.assertEqual(actual["report"][0], 0)
                self.assertEqual(actual["get_report_id"], 3)

    def test_new_queries_reject_other_product_branches_and_unobserved_selectors(self):
        for path, command, payload in [
            (KEYBOARD_PATH, (22, 0, 130), []),
            (MOUSE_PATH, (22, 0, 130), [1]),
            (MOUSE_PATH, (21, 0, 130), []),
            (KEYBOARD_PATH, (2, 7, 128), [0]),
            (KEYBOARD_PATH, (2, 7, 132), [0]),
            (KEYBOARD_PATH, (2, 7, 131), []),
            (MOUSE_PATH, (3, 0, 134), []),
            (MOUSE_PATH, (3, 3, 128), [0, 8]),
            (MOUSE_PATH, (2, 2, 160), [1]),
            (MOUSE_PATH, (4, 2, 161), [1]),
            (MOUSE_PATH, (2, 7, 128), [1]),
            (MOUSE_PATH, (2, 7, 132), [1]),
            (MOUSE_PATH, (2, 7, 131), [1]),
            (KEYBOARD_PATH, (4, 0, 135), [1]),
            (KEYBOARD_PATH, (3, 0, 134), [1]),
            (KEYBOARD_PATH, (3, 3, 128), [0, 9]),
            (KEYBOARD_PATH, (2, 2, 160), [0]),
            (KEYBOARD_PATH, (2, 2, 160), [2]),
            (KEYBOARD_PATH, (4, 2, 161), [0]),
            (KEYBOARD_PATH, (4, 2, 161), [2]),
        ]:
            with self.subTest(path=path, command=command, payload=payload), \
                    self.assertRaisesRegex(ValueError, "read-only GET"):
                native.validate_get_trace(trace(path, report(command, payload)))

    def test_per_key_gets_preserve_concrete_firmware_ids_and_source_chunks(self):
        for operation, stride, limit in [(153, 5, 15), (154, 6, 13)]:
            for count in (1, limit):
                keys = [18, 31, 32, 33, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][:count]
                payload = [0, 1] + [value for key in keys for value in [key] + [0] * (stride - 1)]
                raw = report((len(payload), 2, operation), payload, transaction=30)
                with self.subTest(operation=operation, count=count):
                    actual = native.validate_get_trace(trace(KEYBOARD_PATH, raw))[0]
                    self.assertEqual(actual["report"], raw)
                    self.assertEqual(actual["get_report_id"], 3)

    def test_per_key_gets_reject_sentinels_unknown_keys_profiles_padding_and_overflow(self):
        for operation, stride, limit in [(153, 5, 15), (154, 6, 13)]:
            valid = [0, 1, 31] + [0] * (stride - 1)
            invalid = []
            for offset, value in [(0, 1), (1, 0), (1, 2), (2, 255), (2, 0), (2, 14), (3, 1)]:
                payload = valid.copy()
                payload[offset] = value
                invalid.append(payload)
            invalid.extend(([0, 1], valid + valid[2:], valid + [0]))
            for payload in invalid:
                with self.subTest(operation=operation, payload=payload), self.assertRaises(ValueError):
                    native.validate_get_trace(trace(KEYBOARD_PATH, report((len(payload), 2, operation), payload)))
            keys = list(range(1, limit + 2))
            payload = [0, 1] + [value for key in keys for value in [key] + [0] * (stride - 1)]
            if len(payload) <= 80:
                with self.assertRaises(ValueError):
                    native.validate_get_trace(trace(KEYBOARD_PATH, report((len(payload), 2, operation), payload)))
            with self.assertRaises(ValueError):
                native.validate_get_trace(trace(MOUSE_PATH, report((len(valid), 2, operation), valid)))

    def test_per_key_gets_never_admit_the_corresponding_configuration_setters(self):
        for operation, stride in [(25, 5), (26, 6)]:
            payload = [0, 1, 31] + [0] * (stride - 1)
            with self.assertRaisesRegex(ValueError, "read-only GET"):
                native.validate_get_trace(trace(KEYBOARD_PATH, report((len(payload), 2, operation), payload)))

    def test_rejects_configuration_setters_even_with_a_valid_checksum(self):
        for path, command, payload in [
            (MOUSE_PATH, (2, 0, 64), [1, 4]),
            (MOUSE_PATH, (7, 4, 5), [0, 3, 32, 3, 32, 0, 0]),
            (MOUSE_PATH, (38, 4, 6), [1]),
            (KEYBOARD_PATH, (3, 15, 4), [1, 5, 255]),
            (KEYBOARD_PATH, (2, 0, 4), [3, 0]),
            (KEYBOARD_PATH, (3, 3, 0), [0, 8, 1]),
            (KEYBOARD_PATH, (2, 2, 32), [1, 1]),
            (KEYBOARD_PATH, (4, 2, 33), [1, 1, 4, 7]),
        ]:
            with self.subTest(command=command), self.assertRaisesRegex(ValueError, "read-only GET"):
                native.validate_get_trace(trace(path, report(command, payload)))

    def test_complete_payload_reserved_fields_and_byte_types_are_checked(self):
        for index, value in [(0, 3), (1, 2), (2, 31), (3, 1), (5, 1), (11, 1), (88, 1), (89, 0), (90, 1)]:
            raw = report()
            raw[index] = value
            with self.subTest(index=index), self.assertRaises(ValueError):
                native.validate_get_trace(trace(raw=raw))
        for invalid in [True, -1, 256, 1.0, "1"]:
            raw = report()
            raw[9] = invalid
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                native.validate_get_trace(trace(raw=raw))
        raw = report()
        raw[30] = 44
        raw[89] ^= 44
        with self.assertRaisesRegex(ValueError, "read-only GET"):
            native.validate_get_trace(trace(raw=raw))

    def test_path_factory_and_collection_are_required_without_fallback(self):
        for path in [
            MOUSE_PATH.replace("MI_00", "MI_03"),
            MOUSE_PATH.replace("VID_1532", "VID_1234"),
            MOUSE_PATH.replace("PID_00BF", "PID_00C0"),
            MOUSE_PATH.replace("&MI_00", ""),
            MOUSE_PATH + "\0",
            MOUSE_PATH.replace("HID#", "USB#"),
            KEYBOARD_PATH.replace("MI_03", "MI_00"),
        ]:
            with self.subTest(path=path), self.assertRaises(ValueError):
                native.validate_get_trace(trace(path))
        actual = native.validate_get_trace(trace(list(MOUSE_PATH.encode("ascii"))))[0]
        self.assertEqual(actual["path"], MOUSE_PATH)
        wired = MOUSE_PATH.replace("00BF", "00BE")
        with self.assertRaises(ValueError):
            native.validate_get_trace(trace(wired, report((1, 0, 184), [0])))

    def test_batch_and_schema_are_bounded_and_no_selector_is_guessed(self):
        for rows in [[], trace() * 65, {"requests": trace()}, [{"path": MOUSE_PATH}],
                     [{**trace()[0], "operation": "write"}], [{**trace()[0], "trace_index": True}]]:
            with self.subTest(rows=str(rows)[:120]), self.assertRaises(ValueError):
                native.validate_get_trace(rows)


class SourceIdentityTests(unittest.TestCase):
    def test_verification_is_static_and_missing_libraries_never_start_workers(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(native.subprocess, "Popen") as process, \
                patch.object(native, "_HidLibrary") as loader:
            result = native.replay_get_trace(trace(), Path(directory) / "HID.node")
        self.assertEqual(result["status"], "validation_failed")
        self.assertFalse(result["target_execution"])
        loader.assert_not_called()
        process.assert_not_called()

    def test_sha_architecture_and_exact_exports_are_checked_without_a_load(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(native.ctypes, "CDLL") as loader:
            path = Path(directory) / "HID.node"
            for bits, wrong_export in [(64, False), (32, False), (64, True)]:
                raw = source_pe(bits)
                if wrong_export:
                    struct.pack_into("<I", raw, 0x450, 0x17000)
                path.write_bytes(raw)
                with patch.object(native, "SOURCE_BYTES", len(raw)), \
                        patch.object(native, "SOURCE_SHA256", hashlib.sha256(raw).hexdigest()):
                    if bits == 64 and not wrong_export:
                        identity = native.verify_source_library(path)
                        self.assertFalse(identity["validation_loaded_library"])
                        self.assertEqual(identity["exports"]["hid_get_feature_report"]["rva"], "0x17ac0")
                    else:
                        with self.assertRaises(ValueError):
                            native.verify_source_library(path)
                with self.assertRaisesRegex(ValueError, "bytes differ"):
                    native.verify_source_library(path)
            loader.assert_not_called()

    def test_invalid_trace_is_rejected_before_source_inspection(self):
        with patch.object(native, "verify_source_library") as verify, patch.object(native.subprocess, "Popen") as process:
            result = native.replay_get_trace(trace(raw=report((2, 0, 64), [1, 4])), "HID.node")
        self.assertEqual(result["status"], "validation_failed")
        verify.assert_not_called()
        process.assert_not_called()


class FakeLibrary:
    def __init__(self, status=4, get_count=91, send_count=91, throw_get=False, throw_close=False):
        self.calls = []
        self.status, self.get_count, self.send_count = status, get_count, send_count
        self.throw_get, self.throw_close = throw_get, throw_close
        self.outgoing = None

    def hid_init(self):
        self.calls.append("init")
        return 0

    def hid_open_path(self, path):
        self.calls.append(("open", path))
        return 57

    def hid_send_feature_report(self, handle, data, length):
        self.calls.append(("send", handle, bytes(data), length))
        self.outgoing = bytes(data)
        return self.send_count

    def hid_get_feature_report(self, handle, data, length):
        self.calls.append(("get", handle, data[0], length))
        if self.throw_get:
            raise OSError("simulated native getter failure")
        selector = data[0]
        for index, value in enumerate(self.outgoing):
            data[index] = value
        data[0], data[1], data[10] = selector, self.status, 127
        return self.get_count

    def hid_error(self, handle):
        return "simulated native error"

    def hid_close(self, handle):
        self.calls.append(("close", handle))
        if self.throw_close:
            raise OSError("simulated close failure")

    def hid_exit(self):
        self.calls.append("exit")
        return 0

    def release(self):
        self.calls.append("release")


class NativeLifecycleTests(unittest.TestCase):
    def execute(self, rows, **settings):
        library = FakeLibrary(**settings)
        events = []
        with patch.object(native.time, "sleep"):
            result = native._execute_requests(native.validate_get_trace(rows), library, events.append)
        return result, library, events

    def test_native_status_and_unknown_bytes_are_returned_without_domain_decoding(self):
        result, library, _ = self.execute(trace())
        query = result["queries"][0]
        self.assertEqual(query["send_return"], 91)
        self.assertEqual(query["get_return"], 91)
        self.assertEqual(query["response_status"], 4)
        self.assertEqual(query["received_bytes"][10], 127)
        self.assertNotIn("hz", query)
        self.assertIsNone(result["error"])
        self.assertEqual(library.calls[-3:], [("close", 57), "exit", "release"])

    def test_busy_reports_are_retained_until_the_source_bounded_terminal_read(self):
        class BusyLibrary(FakeLibrary):
            def hid_get_feature_report(self, handle, data, length):
                self.status = 1 if len([call for call in self.calls if isinstance(call, tuple) and call[0] == "get"]) < 2 else 2
                return super().hid_get_feature_report(handle, data, length)
        library = BusyLibrary()
        with patch.object(native.time, "sleep"):
            result = native._execute_requests(native.validate_get_trace(trace()), library, lambda _: None)
        query = result["queries"][0]
        self.assertEqual([row["response_status"] for row in query["get_attempts"]], [1, 1, 2])
        self.assertEqual(query["response_status"], 2)
        self.assertIsNone(query["native_error"])
        self.assertEqual(len([call for call in library.calls if isinstance(call, tuple) and call[0] == "send"]), 1)
        result, _, _ = self.execute(trace(), status=1)
        query = result["queries"][0]
        self.assertEqual(len(query["get_attempts"]), 10)
        self.assertEqual(query["response_status"], 1)
        self.assertIn("remained busy", query["native_error"])

    def test_stale_response_is_not_polled_or_accepted_as_current_data(self):
        class StaleLibrary(FakeLibrary):
            def hid_get_feature_report(self, handle, data, length):
                count = super().hid_get_feature_report(handle, data, length)
                data[2] ^= 1
                return count
        library = StaleLibrary(status=1)
        with patch.object(native.time, "sleep"):
            result = native._execute_requests(native.validate_get_trace(trace()), library, lambda _: None)
        query = result["queries"][0]
        self.assertEqual(len(query["get_attempts"]), 1)
        self.assertIn("transaction or command", query["native_error"])
        self.assertEqual(library.calls[-3:], [("close", 57), "exit", "release"])

    def test_failed_or_short_get_after_busy_does_not_reuse_the_previous_response(self):
        for count in (-1, 0, 1, 100):
            class PartialLibrary(FakeLibrary):
                def hid_get_feature_report(self, handle, data, length):
                    if any(isinstance(call, tuple) and call[0] == "get" for call in self.calls):
                        self.get_count = count
                    return super().hid_get_feature_report(handle, data, length)
            library = PartialLibrary(status=1)
            with self.subTest(count=count), patch.object(native.time, "sleep"):
                result = native._execute_requests(native.validate_get_trace(trace()), library, lambda _: None)
                query = result["queries"][0]
                self.assertEqual([row["get_return"] for row in query["get_attempts"]], [91, count])
                self.assertIsNone(query["response_status"])
                if 0 <= count <= 91:
                    self.assertEqual(len(query["received_bytes"]), count)
                else:
                    self.assertIsNone(query["received_bytes"])
                self.assertEqual(library.calls[-3:], [("close", 57), "exit", "release"])

    def test_keyboard_get_retains_selector_three_in_the_native_buffer(self):
        result, library, _ = self.execute(trace(KEYBOARD_PATH, report((3, 15, 132), [1, 5])))
        self.assertEqual(result["queries"][0]["received_bytes"][0], 3)
        self.assertEqual([call for call in library.calls if isinstance(call, tuple) and call[0] == "get"],
                         [("get", 57, 3, 91)])

    def test_failed_send_does_not_invoke_get_or_synthesize_a_response(self):
        result, library, _ = self.execute(trace(), send_count=-1)
        query = result["queries"][0]
        self.assertEqual(query["send_return"], -1)
        self.assertIsNone(query["get_return"])
        self.assertIsNone(query["received_bytes"])
        self.assertEqual(query["native_error"], "simulated native error")
        self.assertFalse(any(isinstance(call, tuple) and call[0] == "get" for call in library.calls))

    def test_partial_and_failed_gets_preserve_actual_count_and_buffer(self):
        for count in [-1, 7, 100]:
            result, _, _ = self.execute(trace(), get_count=count)
            query = result["queries"][0]
            self.assertEqual(query["get_return"], count)
            self.assertEqual(len(query["response_buffer"]), 91)
            if count == 7:
                self.assertEqual(len(query["received_bytes"]), 7)
            else:
                self.assertIsNone(query["received_bytes"])

    def test_get_exception_and_close_failure_still_exit_and_release_in_order(self):
        result, library, _ = self.execute(trace(), throw_get=True, throw_close=True)
        self.assertIn("simulated native getter failure", result["error"])
        self.assertFalse(result["cleanup"][0]["returned"])
        self.assertTrue(result["cleanup"][1]["returned"])
        self.assertTrue(result["cleanup"][2]["returned"])
        self.assertEqual(library.calls[-3:], [("close", 57), "exit", "release"])

    def test_open_event_output_failure_keeps_handle_owned_and_runs_all_cleanup(self):
        library = FakeLibrary()

        def emit(row):
            if row["event"] in ("opened", "cleanup"):
                raise BrokenPipeError("simulated event output failure")

        with patch.object(native.time, "sleep"):
            result = native._execute_requests(native.validate_get_trace(trace()), library, emit)
        self.assertIn("simulated event output failure", result["error"])
        self.assertEqual(library.calls[-3:], [("close", 57), "exit", "release"])
        self.assertEqual([item["operation"] for item in result["cleanup"]],
                         ["hid_close", "hid_exit", "release_library"])
        self.assertTrue(all(item["returned"] for item in result["cleanup"]))
        self.assertEqual(len(result["event_output_errors"]), 3)
        self.assertEqual(result["queries"], [])


class FakeProcess:
    def __init__(self, output, timeout=False, code=0):
        self.output, self.timeout, self.returncode = output, timeout, code
        self.calls = []

    def communicate(self, data=None, timeout=None):
        self.calls.append(("communicate", data, timeout))
        if self.timeout and timeout is not None:
            raise subprocess.TimeoutExpired("pure-test-worker", timeout)
        return self.output, ""

    def kill(self):
        self.calls.append("kill")
        self.returncode = -9

    def wait(self):
        self.calls.append("wait")
        return self.returncode


class ProcessDeadlineTests(unittest.TestCase):
    def replay(self, process):
        source = {"path": "C:/verified/HID.node", "sha256": native.SOURCE_SHA256}
        with patch.object(native, "verify_source_library", return_value=source), \
                patch.object(native.subprocess, "Popen", return_value=process) as start:
            result = native.replay_get_trace(trace(), source["path"], timeout_seconds=0.1)
        self.assertIn("-I", start.call_args.args[0])
        self.assertTrue(start.call_args.kwargs["text"])
        return result

    def test_deadline_kills_and_waits_before_returning_partial_native_events(self):
        output = '\n'.join(json.dumps(row) for row in [
            {"event": "library_loaded"}, {"event": "sending_get", "trace_index": 0}]) + '\n'
        process = FakeProcess(output, timeout=True)
        result = self.replay(process)
        self.assertEqual(result["status"], "timed_out")
        self.assertTrue(result["worker_terminated"])
        self.assertTrue(result["worker_waited"])
        self.assertTrue(result["target_execution"])
        self.assertEqual(result["device_queries_attempted"], 1)
        self.assertIsNone(result["result"])
        self.assertEqual(process.calls[1], "kill")
        self.assertEqual(process.calls[-1], "wait")
        self.assertEqual(len([call for call in process.calls if isinstance(call, tuple)]), 2)

    def test_completed_worker_keeps_raw_result_and_is_waited(self):
        output = '\n'.join(json.dumps(row) for row in [
            {"event": "library_loaded"}, {"event": "sending_get"},
            {"event": "result", "result": {"queries": [{"response_status": 4}], "cleanup": []}}])
        process = FakeProcess(output)
        result = self.replay(process)
        self.assertEqual(result["status"], "completed")
        self.assertEqual(result["result"]["queries"][0]["response_status"], 4)
        self.assertEqual(process.calls[-1], "wait")
        self.assertNotIn("kill", process.calls)

    def test_unconfirmed_load_attempt_survives_a_worker_timeout(self):
        process = FakeProcess('{"event":"loading_library"}\n', timeout=True)
        result = self.replay(process)
        self.assertEqual(result["status"], "timed_out")
        self.assertTrue(result["target_execution_attempted"])
        self.assertFalse(result["target_execution"])
        self.assertEqual(result["device_queries_attempted"], 0)

    def test_malformed_result_and_event_are_reported_without_crashing_parent(self):
        for row in [{"event": "result"}, {"event": "result", "result": []}, {"event": []}]:
            with self.subTest(row=row):
                result = self.replay(FakeProcess(json.dumps(row)))
                self.assertEqual(result["status"], "worker_failed")
                self.assertEqual(len(result["worker_output_failures"]), 1)
                self.assertIsNone(result["result"])

    def test_incomplete_output_and_invalid_deadlines_do_not_claim_a_completed_worker(self):
        result = self.replay(FakeProcess('{"event":"library_loaded"}\nnot-json'))
        self.assertEqual(result["status"], "worker_failed")
        for timeout in [0, -1, 61, True, float("nan"), float("inf")]:
            with patch.object(native.subprocess, "Popen") as process:
                result = native.replay_get_trace(trace(), "HID.node", timeout)
            self.assertEqual(result["status"], "validation_failed")
            process.assert_not_called()

    def test_worker_revalidates_before_loading_a_missing_library(self):
        job = {"dll_path": "does-not-exist/HID.node", "requests": native.validate_get_trace(trace()),
               "source_sha256": native.SOURCE_SHA256}
        with patch.object(native.sys, "stdin", io.StringIO(json.dumps(job))), \
                patch.object(native.sys, "stdout", io.StringIO()) as output, \
                patch.object(native, "_HidLibrary") as loader:
            code = native._worker_main()
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(output.getvalue())["event"], "worker_error")
        loader.assert_not_called()

    def test_load_event_output_failure_releases_library_before_any_native_call(self):
        library = FakeLibrary()
        library.system_hid_identity = {"path": "simulated-system-hid"}
        source = {"path": "C:/verified/HID.node", "sha256": native.SOURCE_SHA256}
        job = {"dll_path": source["path"], "requests": trace(), "source_sha256": source["sha256"]}

        class BrokenOutput(io.StringIO):
            def write(self, value):
                if "library_loaded" in value or "worker_error" in value:
                    raise BrokenPipeError("simulated parent pipe failure")
                return super().write(value)

        with patch.object(native.sys, "stdin", io.StringIO(json.dumps(job))), \
                patch.object(native.sys, "stdout", BrokenOutput()), \
                patch.object(native, "verify_source_library", return_value=source), \
                patch.object(native, "_HidLibrary", return_value=library):
            code = native._worker_main()
        self.assertEqual(code, 1)
        self.assertEqual(library.calls, ["release"])

    def test_maximum_validated_paths_and_batch_fit_worker_input_limit(self):
        instance = "x" * (4096 - len(MOUSE_PATH) + len("8&393c0d0b&0&0000"))
        path = MOUSE_PATH.replace("8&393c0d0b&0&0000", instance)
        self.assertEqual(len(path), 4096)
        source = {"path": "C:/verified/HID.node", "sha256": native.SOURCE_SHA256}
        job = {"dll_path": source["path"], "requests": trace(path) * 64,
               "source_sha256": source["sha256"]}
        raw = json.dumps(job)
        self.assertGreater(len(raw), 262144)
        self.assertLessEqual(len(raw), native._MAX_JOB_CHARACTERS)
        with patch.object(native.sys, "stdin", io.StringIO(raw)), \
                patch.object(native.sys, "stdout", io.StringIO()) as output, \
                patch.object(native, "verify_source_library", return_value=source), \
                patch.object(native, "_HidLibrary") as loader, \
                patch.object(native, "_execute_requests", return_value={"queries": []}) as execute:
            loader.return_value.system_hid_identity = None
            code = native._worker_main()
        self.assertEqual(code, 0)
        self.assertEqual(len(execute.call_args.args[0]), 64)
        loader.assert_called_once_with(source["path"])
        self.assertEqual(json.loads(output.getvalue().splitlines()[-1])["event"], "result")


class LightingComparisonTests(unittest.TestCase):
    def replace_metadata_document(self, fixture, url, document):
        raw = json.dumps(document).encode()
        fixture.responses[url] = raw
        samples = [row for row in fixture.metadata["files"] if row["url"] == url]
        kind = samples[0]["kind"] if samples else "official_host_resource_manifest"
        row = fixture.blob(url, raw, kind)
        row.update(final_url=url, http_status=200)
        if samples:
            for sample in samples:
                sample.update(row)
        else:
            fixture.metadata["files"].extend([{**row, "acquired_at_utc": f"2026-10-29T17:00:0{n}+00:00"}
                                               for n in (0, 1)])
        fixture.metadata["stability"] = [item for item in fixture.metadata["stability"] if item["url"] != url]
        fixture.metadata["stability"].append({"url": url, "before_sha256": row["sha256"],
                                               "after_sha256": row["sha256"], "unchanged": True})
        fixture.save_metadata()

    @contextmanager
    def current_release_fixture(self):
        fixture = host_fixtures.HostReleaseAcquisitionTests()
        try:
            fixture.setUp()
            url = "https://app-assets.razer.com/files/synapse/LightingEngine/v4.0.55.0/RzLightingEngineApi_v4.0.55.0.dll"
            raw = fixture.resource_bytes["Shared"]
            fixture.responses[url] = raw
            manifest_url = "https://apps.razer.com/background-manager/manifests/manifest-1.0.139.json"
            manifest = json.loads(fixture.responses[manifest_url])
            manifest["resources"][0].update(resourceName="LightingEngineDLL", resourceVersion="4.0.55.0", url=url)
            self.replace_metadata_document(fixture, manifest_url, manifest)
            with redirect_stdout(io.StringIO()):
                release = fixture.acquire()
            artifact = next(row for row in release["files"] if row["url"] == url)
            with patch.object(native, "_LIGHTING_INPUT", artifact["path"]), \
                    patch.object(native, "_LIGHTING_SHA256", artifact["sha256"]):
                yield fixture, raw, release
        finally:
            self.assertTrue(fixture.doCleanups())
            if hasattr(fixture, "root"):
                self.assertFalse(fixture.root.exists())

    def replace_release_document(self, fixture, document):
        fixture.write(fixture.release_name, json.dumps(document).encode(), kind="official_host_release_acquisition")
        fixture.save_index()

    def test_current_release_binds_actual_stable_samples_complete_declaration_and_same_dll(self):
        with self.current_release_fixture() as (fixture, raw, release):
            evidence = native._lighting_release_evidence(fixture.root, raw)
            current = evidence["current_release"]
            artifact = next(row for row in release["files"] if row["declaration"].get("resourceName") == "LightingEngineDLL")
            self.assertEqual(current["version"], "4.0.896")
            self.assertEqual(current["acquisition"]["path"], fixture.release_name)
            self.assertEqual(current["artifact"], artifact)
            self.assertEqual(current["declaration"], artifact["declaration"])
            self.assertIn(current["metadata_snapshot"], current["verified_dependencies"])
            self.assertEqual(len(current["metadata_revalidation"]), 6)
            self.assertTrue(current["source_current_during_acquisition"])
            self.assertTrue(current["deployment_metadata_stable"])
            self.assertEqual({row["url"] for row in evidence["release_manifests"]},
                {"https://apps.razer.com/background-manager/installer-manifest.json",
                 "https://apps.razer.com/background-manager/manifests/manifest-1.0.139.json"})
            with self.assertRaisesRegex(ValueError, "audited DLL bytes"):
                native._lighting_release_evidence(fixture.root, b"same length wrong DLL!!")

    def test_current_release_requires_actual_acquisition_snapshot_sample_and_artifact_bytes(self):
        for scenario in ("acquisition", "snapshot", "sample", "changed_sample", "changed_artifact"):
            with self.subTest(scenario=scenario), self.current_release_fixture() as (fixture, raw, release):
                name = {"acquisition": fixture.release_name, "snapshot": release["metadata_snapshot"]["path"],
                        "sample": fixture.metadata["files"][0]["path"],
                        "changed_sample": fixture.metadata["files"][0]["path"],
                        "changed_artifact": native._LIGHTING_INPUT}[scenario]
                path = fixture.root / name
                if scenario.startswith("changed_"):
                    path.write_bytes(b"changed retained bytes")
                else:
                    path.unlink()
                with self.assertRaises((ValueError, FileNotFoundError)):
                    native._lighting_release_evidence(fixture.root, raw)

    def test_current_release_rejects_untrusted_receipt_and_false_current_during_claims(self):
        for scenario in ("source_kind", "current_during", "metadata_files", "authority"):
            with self.subTest(scenario=scenario), self.current_release_fixture() as (fixture, raw, release):
                if scenario == "source_kind":
                    next(row for row in fixture.index["inputs"] if row["path"] == fixture.release_name)["kind"] = "untrusted"
                    fixture.save_index()
                else:
                    if scenario == "current_during":
                        release["source_current_during_acquisition"] = False
                    elif scenario == "metadata_files":
                        release["metadata_files"][0]["final_url"] = "https://untrusted.invalid/manifest.json"
                    else:
                        next(row for row in release["files"] if row["path"] == native._LIGHTING_INPUT)["authorities"] = []
                    self.replace_release_document(fixture, release)
                with self.assertRaises(ValueError):
                    native._lighting_release_evidence(fixture.root, raw)

    def test_current_release_rejects_wrong_source_and_incomplete_actual_sampling(self):
        for scenario in ("requested_url", "final_url", "missing_sample", "stability_flag"):
            with self.subTest(scenario=scenario), self.current_release_fixture() as (fixture, raw, _):
                if scenario == "requested_url":
                    fixture.metadata["files"][0]["url"] = "https://untrusted.invalid/installer-manifest.json"
                elif scenario == "final_url":
                    fixture.metadata["files"][0]["final_url"] = "http://apps.razer.com/installer-manifest.json"
                elif scenario == "missing_sample":
                    fixture.metadata["files"].pop()
                else:
                    fixture.metadata["deployment_metadata_stable"] = False
                fixture.save_metadata()
                with self.assertRaises(ValueError):
                    native._lighting_release_evidence(fixture.root, raw)

    def test_current_release_cannot_reuse_same_sha_when_latest_points_to_another_manifest(self):
        with self.current_release_fixture() as (fixture, raw, _):
            installer_url = "https://apps.razer.com/background-manager/installer-manifest.json"
            installer = json.loads(fixture.responses[installer_url])
            installer["latest"] = {"version": "1.0.140", "url": "manifest-1.0.140.json"}
            resource = json.loads(fixture.responses["https://apps.razer.com/background-manager/manifests/manifest-1.0.139.json"])
            self.replace_metadata_document(fixture,
                "https://apps.razer.com/background-manager/manifests/manifest-1.0.140.json", resource)
            self.replace_metadata_document(fixture, installer_url, installer)
            with self.assertRaisesRegex(ValueError, "declarations differ from current stable metadata"):
                native._lighting_release_evidence(fixture.root, raw)

    def test_current_release_requires_complete_declaration_even_with_identical_dll_sha(self):
        with self.current_release_fixture() as (fixture, raw, _):
            url = "https://apps.razer.com/background-manager/manifests/manifest-1.0.139.json"
            resource = json.loads(fixture.responses[url])
            resource["resources"][0]["restartRequired"] = True
            self.replace_metadata_document(fixture, url, resource)
            with self.assertRaisesRegex(ValueError, "declarations differ from current stable metadata"):
                native._lighting_release_evidence(fixture.root, raw)

    def test_current_release_never_reads_another_workspace_root(self):
        with self.current_release_fixture() as (fixture, raw, _):
            other = fixture.root / "other"
            other.mkdir()
            with self.assertRaisesRegex(ValueError, "same workspace root"):
                native._lighting_release_evidence(other, raw)

    def test_single_receipt_retains_failed_raw_results_differences_and_cleanup(self):
        failed = {"schema_version": 1, "passed": False, "scope": "software frames",
                  "recorded_at_utc": "first", "runs": {"original": {"stdout": "raw native failure"}},
                  "comparisons": [{"matched": False, "original": [2], "rust": [1]}],
                  "original": {"errors": ["cleanup failed"], "cleanup": {"disabled": False}}}
        passed = {"schema_version": 1, "passed": True, "scope": "software frames",
                  "recorded_at_utc": "second", "runs": {"original": {"stdout": "raw native output"}}}
        previous = copy.deepcopy(failed)
        retained = native.retain_lighting_attempt(previous, passed)
        self.assertEqual(retained["attempts"], [failed, passed])
        self.assertEqual(previous, failed)
        again = native.retain_lighting_attempt(retained, failed)
        self.assertEqual(again["attempts"], [failed, passed, failed])
        self.assertFalse(again["passed"])
        self.assertEqual(again["latest_attempt"], 2)
        self.assertEqual(native.retain_lighting_attempt(None, failed)["attempts"], [failed])
        for malformed in ([], {"schema_version": 999}, {"schema_version": 2, "attempts": []},
                          {"schema_version": 2, "attempts": [failed], "latest_attempt": 5}):
            with self.subTest(previous=malformed), self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                native.retain_lighting_attempt(malformed, passed)

    def test_full_abi_capture_requires_complete_function_chunks_virtual_targets_and_pe_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            raw = bytearray(2153672)
            raw[16:19] = b"abc"
            for table, slot, target in (
                (0x1998c8, 56, 0x56f0), (0x1998c8, 88, 0x2cf0), (0x1998c8, 96, 0x30b0),
                (0x1a8478, 16, 0x47670), (0x1a8478, 24, 0x48f70),
                (0x1a83c8, 80, 0x476e0), (0x1a83c8, 96, 0x48270),
                (0x1a89a8, 8, 0x4b7c0), (0x1a89a8, 24, 0x4beb0),
                (0x1998c8, 8, 0x7bc0), (0x1998c8, 16, 0x7bd0),
                (0x1998c8, 24, 0x7be0), (0x1998c8, 32, 0x7bf0), (0x1998c8, 40, 0x7c00),
            ):
                struct.pack_into("<Q", raw, table + slot, 0x180000000 + target)
            digest = hashlib.sha256(raw).hexdigest()
            (root / "source.dll").write_bytes(raw)
            (root / "source.js").write_bytes(b"verified frontend ABI")
            frontend_sha = hashlib.sha256(b"verified frontend ABI").hexdigest()
            frontend = root / ".analysis/inputs/frontend/synapse-lighting-engine-current/acquisition.json"
            frontend.parent.mkdir(parents=True)
            frontend.write_text(json.dumps({"files": [{"path": "source.js", "http_status": 200,
                "sha256": frontend_sha, "bytes": len(b"verified frontend ABI")}]}), encoding="utf-8")
            directory = root / f".analysis/results/native/{digest}"
            (directory / "functions").mkdir(parents=True)
            function = {"rva": "0x10", "pseudocode": {"lines": [{"text": "complete"}]},
                        "assembly": ["complete"], "chunks": [{"start": "0x180000010",
                        "end": "0x180000013", "raw_hex": "616263",
                        "sha256": hashlib.sha256(b"abc").hexdigest()}]}

            def publish():
                encoded = json.dumps(function).encode("utf-8")
                (directory / "functions/10.json").write_bytes(encoded)
                (directory / "capture.json").write_text(json.dumps({"input_sha256": digest,
                    "input_bytes": len(raw), "image_base": "0x180000000", "ida_version": "test",
                    "hexrays_version": "test", "functions": [{"path": "functions/10.json",
                    "sha256": hashlib.sha256(encoded).hexdigest()}]}), encoding="utf-8")

            publish()
            pe = {"architecture": "x64", "unresolved": [], "delay_imports": [], "imports": [],
                  "image_base": "0x180000000", "sections": [{"rva": 0, "file_offset": 0, "bytes": len(raw)}],
                  "version_resources": [{"tree": {"file_version": "4.0.55.0", "product_version": "4.0.55.0"}}],
                  "exports": [{"names": [name], "rva": value[0]} for name, value in native._LIGHTING_EXPORTS.items()]}
            with patch.object(native, "_LIGHTING_INPUT", "source.dll"), \
                    patch.object(native, "_LIGHTING_SHA256", digest), \
                    patch.object(native, "_LIGHTING_JS", "source.js"), \
                    patch.object(native, "_LIGHTING_JS_SHA256", frontend_sha), \
                    patch.object(native, "_lighting_release_evidence", return_value={"current_release": {"version": "verified"}}), \
                    patch.object(native, "_LIGHTING_FUNCTIONS", ("10",)), \
                    patch.object(native, "inspect_pe", return_value=pe):
                result = native.verify_lighting_source(root)
                self.assertEqual(len(result["virtual_calls"]), 14)
                self.assertEqual(result["current_release"], {"version": "verified"})
                function["chunks"][0]["raw_hex"] = "616264"
                publish()
                with self.assertRaisesRegex(ValueError, "chunk differs"):
                    native.verify_lighting_source(root)
                function["chunks"][0]["raw_hex"] = "616263"
                function["pseudocode"]["lines"] = []
                publish()
                with self.assertRaisesRegex(ValueError, "body is missing"):
                    native.verify_lighting_source(root)
                function["pseudocode"]["lines"] = [{"text": "complete"}]
                publish()
                pe["version_resources"][0]["tree"]["file_version"] = "older"
                with self.assertRaisesRegex(ValueError, "version, architecture or dependency"):
                    native.verify_lighting_source(root)

    class Export:
        def __init__(self, call):
            self.call = call

        def __call__(self, *arguments):
            return self.call(*arguments)

    class Library:
        def __init__(self, *, fail_action=None, incomplete=False, invalid_json=False):
            self.calls, self.buffers, self.freed = [], {}, []
            self.fail_action, self.incomplete, self.invalid_json = fail_action, incomplete, invalid_json
            self.enabled = False
            self.frame_reads = 0
            export = LightingComparisonTests.Export
            self.GetDLLVersion = export(lambda: self.allocate("4.0.55.0"))
            self.PollEvents = export(lambda: self.allocate("[]"))
            self.RzLightingApi = export(self.api)
            self.RzLightingApiNoReturn = export(lambda raw: self.calls.append(json.loads(raw)))
            self.FreeMalloc = export(self.free)
            self.SetOperatingMode = export(lambda mode: self.calls.append(("mode", mode)))
            self.SetNodeFFIEvent = export(lambda pointer: self.calls.append(("callback", bool(pointer))) or True)

        def allocate(self, value):
            buffer = ctypes.create_string_buffer(value.encode("utf-8"))
            pointer = ctypes.addressof(buffer)
            self.buffers[pointer] = buffer
            return pointer

        def free(self, pointer):
            if pointer not in self.buffers:
                raise AssertionError("Unknown or already freed native string")
            self.freed.append(pointer)
            del self.buffers[pointer]

        def api(self, raw):
            request = json.loads(raw)
            self.calls.append(request)
            action = request["Action"]
            result = {"Status": 0}
            if action == self.fail_action:
                result["Status"] = 0x80004005
            elif action == 1:
                result["device_handle"] = 1
            elif action == 3:
                result["engine_handle"] = 1
            elif action == 49:
                self.enabled = bool(request["enable"])
            elif action == 52:
                if self.enabled:
                    self.frame_reads += 1
                    result["frame"] = [0] * 9 if self.incomplete or self.frame_reads == 1 else [0x123456] * 9
                else:
                    result["Status"] = 0x80004005
            if self.invalid_json and action == 33:
                return self.allocate("invalid native JSON")
            return self.allocate(json.dumps(result))

    def session(self, library, **arguments):
        with patch.object(native.time, "sleep"):
            return native._lighting_session(library, lambda row: None, **arguments)

    def test_public_json_only_no_device_clear_and_exact_cleanup_order(self):
        library = self.Library()
        result = self.session(library)
        self.assertEqual(result["errors"], [])
        self.assertTrue(result["terminated"])
        self.assertEqual([row["name"] for row in result["scenarios"]], ["static", "wheel_stationary"])
        self.assertTrue(all(row["frame"] == [0x123456] * 9 for row in result["scenarios"]))
        actions = [row for row in library.calls if isinstance(row, dict)]
        self.assertTrue(all(row["clearFrame"] == 0 for row in actions if row["Action"] == 49))
        self.assertEqual([row["Action"] for row in actions][-4:], [49, 4, 2, 65520])
        self.assertEqual(library.calls[-2:], [("mode", 0), ("callback", False)])
        self.assertFalse(library.buffers)
        self.assertIsNone(library.RzLightingApiNoReturn.restype)
        self.assertIsNone(library.SetOperatingMode.restype)
        self.assertEqual(library.RzLightingApi.restype, ctypes.c_void_p)

    def test_failed_or_malformed_result_still_frees_strings_and_destroys_owners(self):
        for arguments in ({"fail_action": 33}, {"invalid_json": True}):
            with self.subTest(arguments=arguments):
                library = self.Library(**arguments)
                result = self.session(library)
                self.assertTrue(result["terminated"])
                self.assertTrue(all("error" in row for row in result["scenarios"]))
                self.assertTrue(all(all(row["cleanup"].values()) for row in result["scenarios"]))
                self.assertFalse(library.buffers)
                self.assertEqual(library.calls[-1], ("callback", False))

    def test_cleanup_failure_is_retained_and_other_cleanup_continues(self):
        library = self.Library(fail_action=4)
        result = self.session(library)
        self.assertTrue(any("Cleanup action 4" in message for message in result["errors"]))
        self.assertTrue(all(row["cleanup"]["destroy_device"] for row in result["scenarios"]))
        self.assertFalse(library.buffers)
        self.assertEqual(library.calls[-1], ("callback", False))

    def test_incomplete_publication_is_a_timeout_not_an_expected_value_retry(self):
        library = self.Library(incomplete=True)
        result = self.session(library, ready_timeout=0)
        self.assertTrue(all(row["frame"] is None and "TimeoutError" in row["error"] for row in result["scenarios"]))
        self.assertTrue(result["terminated"])
        self.assertFalse(library.buffers)

    def snapshot(self):
        return {"schema_version": 1, "scenarios": [{"name": name,
            "geometry": native._lighting_geometry(), "effect": effect, "epoch_seconds": 0,
            "cleanup": {"engine_destroyed": True, "device_destroyed": True, "terminated": True},
            "stable_packed_colorref": [1] * 9,
            "ticks": [{"logical_ms": milliseconds, "frames": []} for milliseconds in range(0, 4001, 40)]}
            for name, effect in native._lighting_scenarios()]}

    def test_snapshot_rejects_arbitrary_native_actions_geometry_and_missing_timeline(self):
        original = self.snapshot()
        self.assertEqual(native._lighting_snapshot(original), original)
        for field in ("effect", "geometry", "ticks", "stable_packed_colorref", "cleanup"):
            value = copy.deepcopy(original)
            value["scenarios"][0][field] = {}
            with self.subTest(field=field), self.assertRaises(ValueError):
                native._lighting_snapshot(value)

    def test_comparison_requires_actual_frames_exact_equality_and_unchanged_inputs(self):
        snapshot = self.snapshot()
        original = {"errors": [], "terminated": True, "scenarios": [{"name": row["name"],
            "frame": row["stable_packed_colorref"], "cleanup": {
                "disable": True, "destroy_engine": True, "destroy_device": True}}
            for row in snapshot["scenarios"]]}

        def run_result(marker, value):
            return {"exit_code": 0, "timed_out": False,
                    "stdout": marker + " " + json.dumps(value), "stderr": "", "worker_waited": True}

        for scenario in ("equal", "different", "inputs_changed", "cleanup_failed"):
            value = copy.deepcopy(original)
            if scenario == "different":
                value["scenarios"][0]["frame"][0] = 2
            if scenario == "cleanup_failed":
                value["scenarios"][0]["cleanup"]["destroy_engine"] = False
            runs = [run_result("RAZER_LIGHTING_RESULT", snapshot),
                    run_result("RAZER_LIGHTING_NATIVE_RESULT", value),
                    run_result("RAZER_LIGHTING_RESULT", snapshot)]
            identities = [[], [{"changed": True}]] if scenario == "inputs_changed" else [[], []]
            with self.subTest(scenario=scenario), \
                    patch.object(native, "verify_lighting_source", return_value={"sha256": "audited"}), \
                    patch.object(native, "_lighting_input_identity", side_effect=identities), \
                    patch.object(native, "_run_service_version_test", side_effect=runs):
                receipt = native.compare_local_lighting(Path("."), lambda **kwargs: [])
            self.assertEqual(receipt["passed"], scenario == "equal")
            self.assertEqual(len(receipt["runs"]), 3)
            if scenario == "different":
                self.assertEqual(receipt["comparisons"][0]["original"][0], 2)
                self.assertFalse(receipt["comparisons"][0]["matched"])

    def test_preflight_failures_publish_a_failed_attempt_without_executing_a_process(self):
        for phase in ("source_verification", "input_identity"):
            published = []
            with self.subTest(phase=phase), tempfile.TemporaryDirectory() as directory, \
                    patch.object(native, "verify_lighting_source",
                                 side_effect=ValueError("invalid source") if phase == "source_verification" else None,
                                 return_value={"sha256": "audited"}), \
                    patch.object(native, "_lighting_input_identity",
                                 side_effect=OSError("unreadable input") if phase == "input_identity" else None,
                                 return_value=[]), \
                    patch.object(native, "_run_service_version_test") as execute:
                receipt = native.compare_local_lighting(Path(directory), lambda **kwargs: [],
                    publish=lambda value: published.append(copy.deepcopy(value)))
            self.assertEqual(published, [receipt])
            self.assertEqual(receipt["status"], "failed")
            self.assertEqual(receipt["phase"], phase)
            self.assertFalse(receipt["passed"])
            self.assertFalse(receipt["cancelled"])
            self.assertTrue(receipt["recorded_at_utc"])
            self.assertEqual(receipt["runs"], {})
            execute.assert_not_called()

    def test_each_interrupted_phase_publishes_partial_output_before_propagating_cancellation(self):
        snapshot = self.snapshot()
        original = {"errors": [], "terminated": True, "scenarios": []}
        values = [("RAZER_LIGHTING_RESULT", snapshot), ("RAZER_LIGHTING_NATIVE_RESULT", original),
                  ("RAZER_LIGHTING_RESULT", snapshot)]
        names = ("rust_before", "original", "rust_after")
        for interrupted_index in range(3):
            for interruption in (KeyboardInterrupt, SystemExit):
                published, calls = [], []
                partial = {"exit_code": -1, "stdout": "partial native observation", "stderr": "worker error",
                           "timed_out": False, "worker_waited": True,
                           "forced_tree_cleanup": {"exit_code": 0}, "interruption": interruption.__name__}

                def execute(command, root, environment, *, observe, **options):
                    index = len(calls)
                    calls.append(command)
                    if index == interrupted_index:
                        observe(copy.deepcopy(partial))
                        raise interruption()
                    marker, value = values[index]
                    return {"exit_code": 0, "stdout": marker + " " + json.dumps(value), "stderr": "",
                            "timed_out": False, "worker_waited": True}

                with self.subTest(phase=names[interrupted_index], interruption=interruption.__name__), \
                        patch.object(native, "verify_lighting_source", return_value={"sha256": "audited"}), \
                        patch.object(native, "_lighting_input_identity", return_value=[]), \
                        patch.object(native, "_run_service_version_test", side_effect=execute):
                    with self.assertRaises(interruption):
                        native.compare_local_lighting(Path("."), lambda **kwargs: [],
                            publish=lambda value: published.append(copy.deepcopy(value)))
                self.assertEqual(len(published), 1)
                receipt = published[0]
                self.assertEqual(receipt["phase"], names[interrupted_index])
                self.assertEqual(receipt["status"], "cancelled")
                self.assertTrue(receipt["cancelled"])
                self.assertFalse(receipt["passed"])
                self.assertEqual(receipt["runs"][names[interrupted_index]], partial)
                self.assertEqual(len(calls), interrupted_index + 1)

    def test_build_environment_pins_absent_empty_and_changed_flags_without_disclosing_values(self):
        environment = {"RUSTFLAGS": "--cfg private_test_value", "CARGO_ENCODED_RUSTFLAGS": "",
                       "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS": "--cfg target_value",
                       "RUSTC_WRAPPER": "private/compiler-wrapper", "UNRELATED_TOKEN": "not a build input"}
        identity = native._lighting_build_environment(environment)
        self.assertFalse(identity["CARGO_BUILD_RUSTFLAGS"]["present"])
        self.assertEqual(identity["CARGO_ENCODED_RUSTFLAGS"], {
            "present": True, "bytes": 0, "sha256": hashlib.sha256(b"").hexdigest()})
        self.assertNotIn("UNRELATED_TOKEN", identity)
        self.assertNotIn("private_test_value", json.dumps(identity))
        self.assertNotIn("private/compiler-wrapper", json.dumps(identity))
        for name in environment.keys() - {"UNRELATED_TOKEN"}:
            changed = dict(environment, **{name: environment[name] + " changed"})
            self.assertNotEqual(identity, native._lighting_build_environment(changed))
        changed = dict(environment)
        del changed["CARGO_ENCODED_RUSTFLAGS"]
        self.assertNotEqual(identity, native._lighting_build_environment(changed))

    def test_all_children_share_the_pinned_environment_and_changed_flags_reject_the_attempt(self):
        snapshot = self.snapshot()
        original = {"errors": [], "terminated": True, "scenarios": []}
        values = [("RAZER_LIGHTING_RESULT", snapshot), ("RAZER_LIGHTING_NATIVE_RESULT", original),
                  ("RAZER_LIGHTING_RESULT", snapshot)]
        environments = []

        def execute(command, root, environment, **options):
            index = len(environments)
            environments.append(environment)
            native.os.environ["RUSTFLAGS"] = "--cfg changed_during_comparison"
            marker, value = values[index]
            return {"exit_code": 0, "stdout": marker + " " + json.dumps(value), "stderr": "",
                    "timed_out": False, "worker_waited": True}

        with patch.dict(native.os.environ, {"RUSTFLAGS": "--cfg original"}, clear=True), \
                patch.object(native, "verify_lighting_source", return_value={"sha256": "audited"}), \
                patch.object(native, "_lighting_input_identity", return_value=[]), \
                patch.object(native, "_run_service_version_test", side_effect=execute):
            receipt = native.compare_local_lighting(Path("."), lambda **kwargs: [])
        self.assertFalse(receipt["passed"])
        self.assertEqual(receipt["status"], "failed")
        self.assertIn("inputs", receipt["error"])
        self.assertEqual(environments, [{"RUSTFLAGS": "--cfg original"}] * 3)
        self.assertIsNot(environments[0], environments[1])

    def test_taskkill_timeout_still_kills_and_waits_for_the_owned_process(self):
        from unittest.mock import Mock
        process = Mock(pid=12345, returncode=1)
        process.communicate.side_effect = [subprocess.TimeoutExpired("comparison", 1), ("partial", "")]
        process.poll.return_value = None
        with patch.object(native.subprocess, "Popen", return_value=process), \
                patch.object(native.subprocess, "run", side_effect=subprocess.TimeoutExpired("taskkill", 15)):
            receipt = native._run_service_version_test(["pure-comparison"], Path("."), {})
        self.assertTrue(receipt["timed_out"])
        self.assertIn("TimeoutExpired", receipt["forced_tree_cleanup"]["error"])
        process.kill.assert_called_once()
        process.wait.assert_called_once()

    def test_parent_cancellation_terminates_and_waits_for_comparison_tree(self):
        from unittest.mock import Mock
        process = Mock(pid=12345, returncode=1)
        process.communicate.side_effect = [KeyboardInterrupt(), ("partial", "")]
        process.poll.return_value = None
        observations, order = [], []
        process.wait.side_effect = lambda: order.append("wait")

        def observe(value):
            order.append("observe")
            observations.append(copy.deepcopy(value))

        with patch.object(native.subprocess, "Popen", return_value=process), \
                patch.object(native.subprocess, "run") as kill, self.assertRaises(KeyboardInterrupt):
            native._run_service_version_test(["pure-comparison"], Path("."), {}, observe=observe)
        self.assertEqual(kill.call_args.args[0], ["taskkill", "/PID", "12345", "/T", "/F"])
        process.kill.assert_called_once()
        process.wait.assert_called_once()
        self.assertEqual(order, ["wait", "observe"])
        self.assertEqual(observations[0]["stdout"], "partial")
        self.assertEqual(observations[0]["interruption"], "KeyboardInterrupt")
        self.assertTrue(observations[0]["worker_waited"])

    def test_failed_cleanup_drain_preserves_partial_bytes_and_real_wait_status(self):
        from unittest.mock import Mock
        for interrupted in (False, True):
            process = Mock(pid=12345, returncode=1)
            initial = KeyboardInterrupt() if interrupted else subprocess.TimeoutExpired("comparison", 1)
            process.communicate.side_effect = [initial,
                subprocess.TimeoutExpired("drain", 15, output=b"last partial output", stderr=b"partial error")]
            process.poll.return_value = None
            observations = []
            with self.subTest(interrupted=interrupted), \
                    patch.object(native.subprocess, "Popen", return_value=process), \
                    patch.object(native.subprocess, "run"):
                if interrupted:
                    with self.assertRaises(KeyboardInterrupt):
                        native._run_service_version_test(["pure-comparison"], Path("."), {}, observe=observations.append)
                else:
                    native._run_service_version_test(["pure-comparison"], Path("."), {}, observe=observations.append)
            self.assertEqual(len(observations), 1)
            self.assertEqual(observations[0]["stdout"], "last partial output")
            self.assertEqual(observations[0]["stderr"], "partial error")
            self.assertEqual(observations[0]["timed_out"], not interrupted)
            self.assertTrue(observations[0]["worker_waited"])
            self.assertIn("TimeoutExpired", observations[0]["output_cleanup_error"])
            process.wait.assert_called_once()


if __name__ == "__main__":
    unittest.main()
