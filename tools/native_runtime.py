"""Replay audited GET packets through the current host's C HID exports.

Validation only reads bytes. Target execution is confined to the disposable
worker used by replay_get_trace; no JavaScript or N-API entry point is called.
Domain decoding remains with the Rust protocol owners.
"""
from __future__ import annotations

import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import time

# Python's isolated worker excludes the script directory from its import path.
if str(Path(__file__).resolve().parent) not in sys.path:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
from host import inspect_pe


SOURCE_SHA256 = "f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f"
SOURCE_BYTES = 405704
SOURCE_CAPTURE = f".analysis/results/native/{SOURCE_SHA256}/capture.json"
SOURCE_EXPORTS = {
    "hid_init": ("0x165f0", "int32(void)"),
    "hid_exit": ("0x16800", "int32(void)"),
    "hid_open_path": ("0x17250", "void*(const char*)"),
    "hid_send_feature_report": ("0x17a10", "int32(void*, const uint8*, uint32)"),
    "hid_get_feature_report": ("0x17ac0", "int32(void*, uint8*, uint32)"),
    "hid_close": ("0x17de0", "void(void*)"),
    "hid_error": ("0x18140", "const wchar_t*(void*)"),
}
_MAX_REQUESTS = 64
_MAX_JOB_CHARACTERS = 1 << 20
# The audited physical PID190/191/679 factories use ten IN attempts and 5 ms.
_MAX_GET_ATTEMPTS = 10
_GET_INTERVAL_SECONDS = 0.005
_HID_GUID = "{4d1e55b2-f16f-11cf-88cb-001111000030}"
_MOUSE_GETS = {
    (80, 4, 134): bytes([1]),
    (7, 4, 133): bytes([0]),
    (2, 0, 192): bytes([1, 0]),
    (4, 0, 135): b"",
    (2, 7, 128): bytes([0]),
    (2, 7, 132): bytes([0]),
    (2, 7, 131): b"",
}
_KEYBOARD_GETS = {
    (80, 15, 128): b"",
    (3, 15, 132): bytes([1, 5]),
    (4, 0, 135): b"",
    (3, 0, 134): b"",
    (3, 3, 128): bytes([0, 8]),
    (2, 2, 160): bytes([1]),
    (4, 2, 161): bytes([1]),
}
_KEYBOARD_679_LAYOUT_1_KEYS = frozenset((
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 43, 44, 46, 47, 48, 49, 50, 51,
    52, 53, 54, 55, 57, 58, 60, 61, 62, 64, 75, 76, 79, 80, 81, 83, 84, 85, 86, 89, 110, 112, 113,
    114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 129,
))


def _keyboard_per_key_get(report):
    stride, limit = {153: (5, 15), 154: (6, 13)}[report[8]]
    size = report[6]
    if report[7] != 2 or size < 2 + stride or (size - 2) % stride:
        raise ValueError("Per-key read-only GET has an invalid source chunk length")
    payload = report[9:9 + size]
    keys = payload[2::stride]
    if (payload[:2] != bytes([0, 1]) or len(keys) > limit or len(set(keys)) != len(keys)
            or any(key not in _KEYBOARD_679_LAYOUT_1_KEYS for key in keys)):
        raise ValueError("Per-key read-only GET requires concrete layout 1 firmware keys and active profile 1")
    expected = bytes([0, 1]) + b"".join(bytes([key]) + bytes(stride - 1) for key in keys)
    if payload != expected:
        raise ValueError("Per-key read-only GET contains nonzero reserved parameters")
    return expected


def verify_source_library(dll_path):
    """Verify the pinned PE, exports and ABI without loading any library."""
    path = Path(dll_path).resolve(strict=True)
    if not path.is_file() or path.name.casefold() != "hid.node":
        raise ValueError("The source library must be the verified HID.node file")
    raw = path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if len(raw) != SOURCE_BYTES or digest != SOURCE_SHA256:
        raise ValueError("HID.node bytes differ from the current complete native capture")
    pe = inspect_pe(raw)
    if pe.get("architecture") != "x64" or pe.get("kind") != "native" or pe.get("unresolved"):
        raise ValueError("HID.node architecture or PE inspection differs from its audited ABI")
    exports = {name: row for row in pe["exports"] for name in row["names"]}
    for name, (rva, _) in SOURCE_EXPORTS.items():
        row = exports.get(name)
        if row is None or row["rva"] != rva or "forwarder" in row:
            raise ValueError("HID.node export differs from its complete function capture: " + name)
    return {
        "path": str(path), "bytes": len(raw), "sha256": digest,
        "architecture": pe["architecture"], "capture": SOURCE_CAPTURE,
        "exports": {name: {"rva": rva, "abi": abi} for name, (rva, abi) in SOURCE_EXPORTS.items()},
        "version_resources": pe["version_resources"],
        "validation_loaded_library": False,
    }


def _byte_list(value, size, name):
    if not isinstance(value, (list, bytes, bytearray)) or len(value) != size:
        raise ValueError(f"{name} must contain exactly {size} bytes")
    if any(type(item) is not int or not 0 <= item <= 255 for item in value):
        raise ValueError(name + " contains a non-byte value")
    return bytes(value)


def _path_identity(path):
    if isinstance(path, list):
        path = _byte_list(path, len(path), "HID path").decode("ascii")
    if not isinstance(path, str) or not 1 <= len(path) <= 4096:
        raise ValueError("A replay requires the exact observed HID path")
    path.encode("ascii")
    if not path.startswith("\\\\?\\") or any(ord(ch) < 33 or ord(ch) > 126 for ch in path):
        raise ValueError("HID path is not an observed ASCII interface path")
    pieces = path[4:].split("#")
    if len(pieces) != 4 or pieces[0].casefold() != "hid" or pieces[3].casefold() != _HID_GUID:
        raise ValueError("HID path does not identify the source HID interface class")
    match = re.fullmatch(r"VID_1532&PID_([0-9A-F]{4})&MI_([0-9A-F]{2})(?:&COL[0-9A-F]{2})?",
                         pieces[1], re.IGNORECASE)
    if match is None or re.fullmatch(r"[A-Za-z0-9&._-]+", pieces[2]) is None:
        raise ValueError("HID path identity is incomplete or ambiguous")
    physical, interface = int(match[1], 16), int(match[2], 16)
    expected_interface = {190: 0, 191: 0, 679: 3}.get(physical)
    if expected_interface is None or interface != expected_interface:
        raise ValueError("HID path has no audited read-only replay factory")
    return path, physical, interface


def validate_get_trace(trace):
    """Copy and validate exact requests; never load a DLL or touch a device.

    Each row is {path, report:[91 u8]}, with optional get_report_id and
    trace_index. The GET report selector is checked against the source's
    truthy-interface fallback independently of SET's explicit zero selector.
    """
    if not isinstance(trace, list) or not 1 <= len(trace) <= _MAX_REQUESTS:
        raise ValueError(f"A replay requires 1..{_MAX_REQUESTS} audited GET requests")
    requests = []
    for index, row in enumerate(trace):
        if not isinstance(row, dict) or not {"path", "report"} <= row.keys():
            raise ValueError("Replay request omits its exact path or report")
        if row.keys() - {"path", "report", "get_report_id", "trace_index"}:
            raise ValueError("Replay request contains an unknown transport field")
        path, physical, interface = _path_identity(row["path"])
        report = _byte_list(row["report"], 91, "Feature report")
        if report[0] != 0 or report[1] != 0 or report[2] >= 31 or any(report[3:6]) or report[90] != 0:
            raise ValueError("Feature report prefix, source transaction or reserved fields differ")
        checksum = 0
        for byte in report[3:89]:
            checksum ^= byte
        if checksum != report[89]:
            raise ValueError("Feature report checksum differs")
        allowed = dict(_KEYBOARD_GETS if physical == 679 else _MOUSE_GETS)
        if physical == 191:
            allowed[(1, 0, 184)] = bytes([0])
        payload = allowed.get(tuple(report[6:9]))
        if physical == 679 and tuple(report[7:9]) in {(2, 153), (2, 154)}:
            payload = _keyboard_per_key_get(report)
        if payload is None or report[9:89] != payload.ljust(80, b"\0"):
            raise ValueError("Feature report is not an exact audited read-only GET")
        selector = row.get("get_report_id", interface)
        if type(selector) is not int or selector != interface:
            raise ValueError("GET report selector differs from its source interface fallback")
        reference = row.get("trace_index", index)
        if type(reference) not in (str, int) or isinstance(reference, str) and len(reference) > 256:
            raise ValueError("Trace correlation must be an integer or short string")
        requests.append({"path": path, "report": list(report), "get_report_id": selector,
                         "trace_index": reference})
    return requests


class _HidLibrary:
    """Only the pinned x64 C ABI; no N-API initialization or callbacks."""
    def __init__(self, path):
        if sys.platform != "win32" or struct.calcsize("P") != 8:
            raise RuntimeError("The audited source DLL requires a Windows x64 Python worker")
        self._library = None
        self._system_hid = None
        self.system_hid_identity = None
        try:
            kernel = ctypes.WinDLL("kernel32.dll", use_last_error=True)
            system_directory = kernel.GetSystemDirectoryW
            system_directory.argtypes = [ctypes.c_wchar_p, ctypes.c_uint32]
            system_directory.restype = ctypes.c_uint32
            buffer = ctypes.create_unicode_buffer(32768)
            count = system_directory(buffer, len(buffer))
            if count == 0 or count >= len(buffer):
                raise OSError("GetSystemDirectoryW did not return the system directory")
            system_hid = Path(buffer.value) / "hid.dll"
            raw = system_hid.read_bytes()
            pe = inspect_pe(raw)
            if pe.get("architecture") != "x64" or pe.get("unresolved"):
                raise ValueError("The system HID library architecture is not verified")
            self.system_hid_identity = {"path": str(system_hid), "sha256": hashlib.sha256(raw).hexdigest(),
                                        "bytes": len(raw), "architecture": pe["architecture"],
                                        "version_resources": pe["version_resources"]}
            # hid_init uses LoadLibraryA("hid.dll"); retain the known system
            # module first so that the source's extra reference resolves there.
            self._system_hid = ctypes.WinDLL(str(system_hid), winmode=0x800)
            self._library = ctypes.CDLL(str(path), winmode=0x100 | 0x800)
            signatures = {
                "hid_init": ([], ctypes.c_int32),
                "hid_exit": ([], ctypes.c_int32),
                "hid_open_path": ([ctypes.c_char_p], ctypes.c_void_p),
                "hid_send_feature_report": ([ctypes.c_void_p, ctypes.POINTER(ctypes.c_ubyte), ctypes.c_uint32], ctypes.c_int32),
                "hid_get_feature_report": ([ctypes.c_void_p, ctypes.POINTER(ctypes.c_ubyte), ctypes.c_uint32], ctypes.c_int32),
                "hid_close": ([ctypes.c_void_p], None),
                "hid_error": ([ctypes.c_void_p], ctypes.c_wchar_p),
            }
            for name, (arguments, result) in signatures.items():
                function = getattr(self._library, name)
                function.argtypes, function.restype = arguments, result
                setattr(self, name, function)
        except BaseException:
            self.release()
            raise

    def release(self):
        import _ctypes
        errors = []
        for attribute in ("_library", "_system_hid"):
            library = getattr(self, attribute, None)
            if library is not None:
                handle, library._handle = library._handle, 0
                setattr(self, attribute, None)
                try:
                    _ctypes.FreeLibrary(handle)
                except Exception as error:
                    errors.append(f"{attribute}: {error}")
        if errors:
            raise RuntimeError("; ".join(errors))


def _execute_requests(requests, library, emit):
    """Run one sequential session; injectable library keeps unit tests pure."""
    result = {"queries": [], "cleanup": [], "init_return": None, "error": None,
              "event_output_errors": []}
    handles = {}
    try:
        result["init_return"] = library.hid_init()
        emit({"event": "initialized", "return_value": result["init_return"]})
        if result["init_return"] != 0:
            raise RuntimeError("The source hid_init returned failure")
        for request in requests:
            path = request["path"]
            if path not in handles:
                handle = library.hid_open_path(path.encode("ascii"))
                if handle:
                    handles[path] = handle
                emit({"event": "opened", "path": path, "handle_opened": bool(handle)})
                if not handle:
                    raise RuntimeError("The source hid_open_path returned a null handle")
            handle = handles[path]
            row = {**request, "send_return": None, "get_return": None, "response_buffer": None,
                   "received_bytes": None, "response_status": None, "native_error": None,
                   "get_attempts": []}
            result["queries"].append(row)
            outgoing = (ctypes.c_ubyte * 91).from_buffer_copy(bytes(request["report"]))
            time.sleep(0.005)
            emit({"event": "sending_get", "trace_index": request["trace_index"], "path": path})
            row["send_return"] = library.hid_send_feature_report(handle, outgoing, 91)
            if row["send_return"] != 91:
                row["native_error"] = library.hid_error(handle)
                emit({"event": "query", "query": dict(row)})
                continue
            for _ in range(_MAX_GET_ATTEMPTS):
                incoming = (ctypes.c_ubyte * 91)()
                incoming[0] = request["get_report_id"]
                row["received_bytes"], row["response_status"] = None, None
                time.sleep(_GET_INTERVAL_SECONDS)
                row["get_return"] = library.hid_get_feature_report(handle, incoming, 91)
                row["response_buffer"] = list(incoming)
                if 0 <= row["get_return"] <= 91:
                    row["received_bytes"] = list(incoming[:row["get_return"]])
                    if row["get_return"] >= 2:
                        row["response_status"] = incoming[1]
                attempt = {key: row[key] for key in (
                    "get_return", "response_buffer", "received_bytes", "response_status")}
                row["get_attempts"].append(attempt)
                if row["get_return"] != 91:
                    row["native_error"] = library.hid_error(handle)
                    break
                if incoming[2] != outgoing[2] or bytes(incoming[7:9]) != bytes(outgoing[7:9]):
                    row["native_error"] = "Native response transaction or command differs"
                    break
                if incoming[1] != 1:
                    break
            else:
                row["native_error"] = "Native response remained busy after source IN attempts"
            emit({"event": "query", "query": dict(row)})
    except Exception as error:
        result["error"] = f"{type(error).__name__}: {error}"
    finally:
        cleanup = [("hid_close", path, lambda handle=handle: library.hid_close(handle))
                   for path, handle in reversed(list(handles.items()))]
        cleanup.extend([("hid_exit", None, library.hid_exit), ("release_library", None, library.release)])
        for operation, path, function in cleanup:
            item = {"operation": operation, "path": path, "returned": False}
            try:
                item["return_value"] = function()
                item["returned"] = True
            except Exception as error:
                item["error"] = f"{type(error).__name__}: {error}"
            result["cleanup"].append(item)
            try:
                emit({"event": "cleanup", **item})
            except Exception as error:
                result["event_output_errors"].append({"event": "cleanup", "operation": operation,
                    "error": f"{type(error).__name__}: {error}"})
    return result


def _worker_main():
    def emit(row):
        print(json.dumps(row, ensure_ascii=True, separators=(",", ":")), flush=True)

    try:
        raw = sys.stdin.read(_MAX_JOB_CHARACTERS + 1)
        if len(raw) > _MAX_JOB_CHARACTERS:
            raise ValueError("Worker request exceeds its validated batch size")
        job = json.loads(raw)
        if not isinstance(job, dict) or set(job) != {"dll_path", "requests", "source_sha256"}:
            raise ValueError("Worker request schema differs")
        requests = validate_get_trace(job["requests"])
        source = verify_source_library(job["dll_path"])
        if job["source_sha256"] != source["sha256"]:
            raise ValueError("Source library changed after parent validation")
        emit({"event": "loading_library", "source": source})
        library = _HidLibrary(source["path"])
        try:
            emit({"event": "library_loaded", "source": source, "system_hid": library.system_hid_identity})
        except BaseException:
            library.release()
            raise
        result = _execute_requests(requests, library, emit)
        emit({"event": "result", "result": result})
        return 0
    except Exception as error:
        try:
            emit({"event": "worker_error", "error": f"{type(error).__name__}: {error}"})
        except (OSError, ValueError):
            pass
        return 1


def _events(stdout):
    events, failures = [], []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
            if not isinstance(event, dict) or not isinstance(event.get("event"), str):
                raise ValueError("Worker event is not an object")
            if event["event"] == "result" and not isinstance(event.get("result"), dict):
                raise ValueError("Worker result omits its native session receipt")
            events.append(event)
        except (ValueError, TypeError) as error:
            failures.append({"line": line[:1024], "error": str(error)})
    return events, failures


def replay_get_trace(trace, dll_path, timeout_seconds=20):
    """Return an execution receipt for exact GETs in a deadline-owned worker.

    Invalid traces/missing or unverified libraries return validation_failed
    before process creation. Native counts/statuses and failed replies are
    preserved; completed denotes worker completion, not a device setting.
    target_execution confirms the load event; target_execution_attempted also
    records an attempt that failed or timed out before load confirmation.
    """
    receipt = {"schema_version": 1, "status": "validation_failed", "target_execution": False,
               "target_execution_attempted": False,
               "device_queries_attempted": 0, "source": None, "requests": [], "events": [],
               "worker_terminated": False, "worker_waited": False, "error": None}
    started = time.monotonic()
    try:
        if isinstance(timeout_seconds, bool) or not isinstance(timeout_seconds, (int, float)) \
                or not math.isfinite(timeout_seconds) or not 0 < timeout_seconds <= 60:
            raise ValueError("Worker deadline must be greater than zero and at most 60 seconds")
        requests = validate_get_trace(trace)
        receipt["requests"] = requests
        source = verify_source_library(dll_path)
        receipt["source"] = source
    except (ValueError, TypeError, OSError) as error:
        receipt["error"] = f"{type(error).__name__}: {error}"
        return receipt
    job = {"dll_path": source["path"], "requests": requests, "source_sha256": source["sha256"]}
    flags = getattr(subprocess, "CREATE_NO_WINDOW", 0) if os.name == "nt" else 0
    command = [sys.executable, "-I", str(Path(__file__).resolve()), "--worker"]
    try:
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace",
            cwd=Path(__file__).resolve().parent, creationflags=flags)
    except OSError as error:
        receipt["status"], receipt["error"] = "worker_failed", f"{type(error).__name__}: {error}"
        return receipt
    try:
        stdout, stderr = process.communicate(json.dumps(job), timeout=timeout_seconds)
        receipt["status"] = "completed" if process.returncode == 0 else "worker_failed"
    except subprocess.TimeoutExpired:
        receipt["status"] = "timed_out"
        process.kill()
        receipt["worker_terminated"] = True
        stdout, stderr = process.communicate()
    except BaseException:
        process.kill()
        process.communicate()
        process.wait()
        raise
    process.wait()
    receipt["worker_waited"] = True
    events, parsing_failures = _events(stdout)
    receipt.update({"events": events, "worker_exit_code": process.returncode,
                    "worker_stderr": stderr[:8192], "worker_output_failures": parsing_failures,
                    "elapsed_ms": round((time.monotonic() - started) * 1000)})
    receipt["target_execution"] = any(event["event"] == "library_loaded" for event in events)
    receipt["target_execution_attempted"] = any(
        event["event"] in ("loading_library", "library_loaded") for event in events)
    receipt["device_queries_attempted"] = sum(event["event"] == "sending_get" for event in events)
    results = [event["result"] for event in events if event["event"] == "result"]
    receipt["result"] = results[-1] if results else None
    if receipt["status"] == "completed" and (len(results) != 1 or parsing_failures):
        receipt["status"], receipt["error"] = "worker_failed", "Worker output is incomplete or malformed"
    return receipt


if __name__ == "__main__":
    if sys.argv[1:] != ["--worker"]:
        raise SystemExit("Use tools/workspace.py for native runtime verification")
    raise SystemExit(_worker_main())
