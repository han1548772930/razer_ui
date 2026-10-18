"""Pure replay validation, fake C ABI lifecycle and process deadline tests."""
from __future__ import annotations

import copy
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


if __name__ == "__main__":
    unittest.main()
