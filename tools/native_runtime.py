"""Compare independent Rust observations with audited local native exports.

Validation only reads bytes. Target execution is confined to disposable
comparison workers; no JavaScript or N-API entry point is called.
Domain decoding remains with the Rust protocol owners.
"""
from __future__ import annotations

import ctypes
from datetime import datetime, timezone
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
import threading

# Python's isolated worker excludes the script directory from its import path.
if str(Path(__file__).resolve().parent) not in sys.path:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
from host import host_release_metadata, inspect_pe, verified_host_release_receipt


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
_SIMPLE_SHA256 = "f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b"
_SIMPLE_INPUT = ".analysis/inputs/host-current/extracted/56b7865f424fb4f0c92d3abd210b128a6429df9b8b026cbb6c0f80945bdf4b06/win-unpacked/CommonDLL/simple_service.dll"
_SIMPLE_FUNCTIONS = (
    "f420", "1000", "2780", "2970", "21ad2", "21cc0", "1d4ba", "28200",
    "280a0", "2db6c", "2dc9a", "2e030", "760c0", "761e0", "f1d0",
    "edf0", "f090", "d754", "ecf0", "ef90", "12f0", "1730", "19c0",
    "1cf0", "852e0", "af50", "afc0", "b120", "b270", "b3e0", "218ac",
    "31e66", "b640", "b7c0",
    "10290", "100b0", "2a50", "2c30", "21dd4", "21fb0", "26caa",
    "1bd28", "114e", "7f400", "7f940", "e5870", "96ed0", "7e0a0",
    "7e370", "7e260", "972f0", "85c20", "93610",
    "25fd4", "25d08", "2612a", "25e5a", "25ee0", "25c10", "7c150",
)
_APPS_FUNCTIONS = (
    "10660", "2d10", "2ee0", "221de", "223a0", "274f4", "4bdb0", "4c10c", "4c725",
    "10f50", "2fa0", "3150", "22582", "22720", "2793e",
    "11400", "3210", "33d0", "22908", "22ac0", "27b22",
    "11a50", "3490", "3660", "22ca8", "22e70", "27d72", "7fe30", "7b8e0", "7b8f0",
    "4bccb", "4bb64", "4997a", "4991d", "49448", "4adc3", "4ae43",
    "4b78e", "4b3ab", "4b8eb", "49cb0", "4a002", "4a334", "4a394", "4a857", "4aa08",
    "f760", "fa70", "ff30", "1f90", "2150", "2230", "23f0", "24d0", "2690",
    "1dcb6", "1dd2c", "1df10", "1df22", "1df8c", "1e190", "1e1a2", "1e220", "1e232", "1e2b4",
    "8920", "8b90", "8da0", "9010",
)

_LIGHTING_SHA256 = "cd9fc2a61ff920b9b73e1f5e27632020e2f8586f443e426a875ad3b042b714fa"
_LIGHTING_INPUT = f".analysis/inputs/host-current/blobs/{_LIGHTING_SHA256}/RzLightingEngineApi_v4.0.55.0.dll"
_LIGHTING_JS = ".analysis/inputs/frontend/synapse-lighting-engine-current/raw/apps.razer.com/synapse/lighting-engine/static/js/main.a383ea9e.js"
_LIGHTING_JS_SHA256 = "c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2"
_LIGHTING_EXPORTS = {
    "SetOperatingMode": ("0x32d00", "void(int32)"),
    "PollEvents": ("0x32d90", "malloc char*(void)"),
    "RzLightingApi": ("0x33490", "malloc char*(const char*)"),
    "RzLightingApiNoReturn": ("0x33b60", "void(const char*)"),
    "GetDLLVersion": ("0x33cf0", "malloc char*(void)"),
    "FreeMalloc": ("0x340c0", "void(void*)"),
    "SetNodeFFIEvent": ("0x340d0", "bool(void (*)(const char*))"),
}
_LIGHTING_FUNCTIONS = (
    "32d00", "32d90", "33490", "33b60", "33cf0", "340c0", "340d0",
    "33600", "33300", "1fd90", "1f2b0", "323d0", "32330", "40420", "40a70",
    "20300", "20c50", "20e10", "21190", "21270", "214e0", "21ac0", "24330",
    "24cd0", "24e30", "24ef0", "24f80", "2a5c0", "2b20", "2960", "2cf0", "30b0",
    "47080", "4d5b0", "45230", "4b2e0", "4b6c0", "4c8f0", "4c620", "4cd40",
    "4cfd0", "4d290", "4b470", "419c0", "41be0", "41b20", "52c50", "52df0",
    "52600", "521e0", "66b40", "66d60", "66f20", "691a0", "692e0",
    "10cb8c", "10ca64", "1830", "18b0", "43630", "43660",
    "45170", "50cb0", "47670", "4d720", "48f70", "48f60", "4d760", "50d50",
    "56f0", "4b7c0", "4a740", "4a910", "476e0", "4b910", "4ad90", "4beb0", "4bdc0",
    "48fe0", "4dd00", "49050", "4f0d0", "49060", "4f390", "49070", "4f690",
    "49080", "4fa40", "490a0", "4fb40", "49030", "4f200", "48270",
    "7bc0", "7bd0", "7be0", "7bf0", "7c00",
    "1f940", "1fc10", "2a8f0", "2aa10", "490c0", "473c0", "50d10", "50cd0",
    "4d690", "48b10", "4fcf0", "42250",
)


def _lighting_release_evidence(root, raw):
    """Bind audited DLL bytes to the verified current stable declaration closure."""
    import analysis
    if Path(root).resolve(strict=True) != analysis.ROOT.resolve(strict=True):
        raise ValueError("Lighting source and release evidence require the same workspace root")
    metadata = host_release_metadata()
    release = verified_host_release_receipt(metadata=metadata)
    url = "https://app-assets.razer.com/files/synapse/LightingEngine/v4.0.55.0/RzLightingEngineApi_v4.0.55.0.dll"
    declarations = [entry for entry in metadata["declarations"]
                    if entry["declaration"].get("resourceName") == "LightingEngineDLL"]
    if len(declarations) != 1:
        raise ValueError("Current stable release has no unique LightingEngineDLL declaration")
    selected = declarations[0]
    declaration = selected["declaration"]
    artifacts = [entry for entry in release["files"] if entry["url"] == selected["url"]]
    if len(artifacts) != 1:
        raise ValueError("Current lighting acquisition identity is ambiguous")
    artifact = artifacts[0]
    if (selected["url"] != url or selected["algorithm"] != "sha256"
            or declaration["url"] != url or declaration["resourceVersion"] != "4.0.55.0"
            or declaration["sha256"].lower() != _LIGHTING_SHA256
            or declaration["size"] != len(raw) or hashlib.sha256(raw).hexdigest() != _LIGHTING_SHA256
            or artifact["path"] != _LIGHTING_INPUT or artifact["sha256"] != _LIGHTING_SHA256
            or artifact["bytes"] != len(raw) or artifact["authorities"] != selected["authorities"]):
        raise ValueError("Current lighting declaration differs from the audited DLL bytes")
    identity_fields = ("url", "final_url", "http_status", "kind", "path", "bytes", "sha256")
    acquired = release.get("metadata_files", [])
    acquired_identities = {row["url"]: {key: row[key] for key in identity_fields} for row in acquired}
    current_identities = {row["url"]: {key: row[key] for key in identity_fields}
                          for row in metadata["metadata_files"]}
    if len(acquired_identities) != len(acquired) or acquired_identities != current_identities:
        raise ValueError("Lighting acquisition metadata file identities differ from current deployment samples")
    authority_urls = {value for authority in selected["authorities"] for value in authority.values()}
    manifests = [{key: row[key] for key in ("path", "url", "bytes", "sha256")}
                 for row in metadata["active_metadata_files"] if row["url"] in authority_urls]
    return {"acquisition": {**release["receipt"], "url": url}, "release_manifests": manifests,
            "current_release": {"version": release["version"], "acquisition": release["receipt"],
                "metadata_audit": metadata["metadata_receipt"], "metadata_snapshot": release["metadata_snapshot"],
                "metadata_audited_at_utc": metadata["metadata"]["finished_at_utc"],
                "acquisition_finished_at_utc": release["finished_at_utc"],
                "metadata_files": metadata["metadata_files"], "metadata_revalidation": release["metadata_revalidation"],
                "verified_dependencies": release["verified_dependencies"], "declaration": declaration,
                "authorities": selected["authorities"], "artifact": artifact,
                "source_current_during_acquisition": release["source_current_during_acquisition"],
                "deployment_metadata_stable": metadata["metadata"]["deployment_metadata_stable"],
                "currentness": "Retained two-sample deployment audit and acquisition-end revalidation; no HTTP performed by this comparison"}}


def verify_lighting_source(root):
    """Verify the current public JSON ABI and selected complete software call chains."""
    root = Path(root)
    raw = (root / _LIGHTING_INPUT).read_bytes()
    if len(raw) != 2153672 or hashlib.sha256(raw).hexdigest() != _LIGHTING_SHA256:
        raise ValueError("Lighting DLL bytes differ from the current official capture")
    release_evidence = _lighting_release_evidence(root, raw)
    pe = inspect_pe(raw)
    versions = pe["version_resources"]
    if (pe["architecture"] != "x64" or pe["unresolved"] or pe.get("delay_imports")
            or len(versions) != 1 or versions[0]["tree"].get("file_version") != "4.0.55.0"
            or versions[0]["tree"].get("product_version") != "4.0.55.0"):
        raise ValueError("Lighting version, architecture or dependency parsing is incomplete")
    exports = {name: entry for entry in pe["exports"] for name in entry["names"]}
    for name, (rva, _) in _LIGHTING_EXPORTS.items():
        if exports.get(name, {}).get("rva") != rva or "forwarder" in exports[name]:
            raise ValueError("Lighting export differs from audited ABI: " + name)
    capture_path = f".analysis/results/native/{_LIGHTING_SHA256}/capture.json"
    capture_raw = (root / capture_path).read_bytes()
    capture = json.loads(capture_raw)
    if (capture["input_sha256"] != _LIGHTING_SHA256 or capture["input_bytes"] != len(raw)
            or capture["image_base"] != pe["image_base"]):
        raise ValueError("Lighting capture identity differs")
    indexed = {entry["path"]: entry for entry in capture["functions"]}
    functions = []
    for rva in _LIGHTING_FUNCTIONS:
        relative = "functions/" + rva + ".json"
        path = Path(capture_path).parent / relative
        encoded = (root / path).read_bytes()
        if hashlib.sha256(encoded).hexdigest() != indexed[relative]["sha256"]:
            raise ValueError("Lighting captured function identity differs: " + rva)
        function = json.loads(encoded)
        if not (function.get("pseudocode") or {}).get("lines") or not function.get("assembly"):
            raise ValueError("Lighting complete function body is missing: " + rva)
        chunks = []
        for chunk in function["chunks"]:
            start, end = int(chunk["start"], 16), int(chunk["end"], 16)
            position = start - int(pe["image_base"], 16)
            sections = [s for s in pe["sections"] if s["rva"] <= position
                        and position + end - start <= s["rva"] + s["bytes"]]
            if len(sections) != 1:
                raise ValueError("Lighting function chunk has no unique raw section")
            offset = sections[0]["file_offset"] + position - sections[0]["rva"]
            actual = raw[offset:offset + end - start]
            if actual.hex() != chunk["raw_hex"] or hashlib.sha256(actual).hexdigest() != chunk["sha256"]:
                raise ValueError("Lighting function chunk differs: " + rva)
            chunks.append({key: chunk[key] for key in ("start", "end", "sha256")})
        functions.append({"path": path.as_posix(), "sha256": indexed[relative]["sha256"],
                          "rva": function["rva"], "chunks": chunks})
    virtual_calls = []
    for table, slot, target in (
        (0x1998c8, 56, 0x56f0), (0x1998c8, 88, 0x2cf0), (0x1998c8, 96, 0x30b0),
        (0x1a8478, 16, 0x47670), (0x1a8478, 24, 0x48f70),
        (0x1a83c8, 80, 0x476e0), (0x1a83c8, 96, 0x48270),
        (0x1a89a8, 8, 0x4b7c0), (0x1a89a8, 24, 0x4beb0),
        (0x1998c8, 8, 0x7bc0), (0x1998c8, 16, 0x7bd0),
        (0x1998c8, 24, 0x7be0), (0x1998c8, 32, 0x7bf0), (0x1998c8, 40, 0x7c00),
    ):
        position = table + slot
        section = next(s for s in pe["sections"] if s["rva"] <= position
                       and position + 8 <= s["rva"] + s["bytes"])
        offset = section["file_offset"] + position - section["rva"]
        encoded = raw[offset:offset + 8]
        if struct.unpack("<Q", encoded)[0] != int(pe["image_base"], 16) + target:
            raise ValueError("Lighting software virtual call target differs")
        virtual_calls.append({"table_rva": hex(table), "slot": slot, "target_rva": hex(target),
                              "raw_hex": encoded.hex(), "sha256": hashlib.sha256(encoded).hexdigest()})
    js_raw = (root / _LIGHTING_JS).read_bytes()
    js_receipt_path = ".analysis/inputs/frontend/synapse-lighting-engine-current/acquisition.json"
    js_rows = [r for r in json.loads((root / js_receipt_path).read_bytes())["files"]
               if r.get("path") == _LIGHTING_JS]
    if (len(js_rows) != 1 or js_rows[0]["http_status"] != 200
            or len(js_raw) != js_rows[0]["bytes"]
            or hashlib.sha256(js_raw).hexdigest() != js_rows[0]["sha256"]
            or hashlib.sha256(js_raw).hexdigest() != _LIGHTING_JS_SHA256):
        raise ValueError("Lighting frontend ABI source identity differs")
    return {"input": _LIGHTING_INPUT, "sha256": _LIGHTING_SHA256, "bytes": len(raw),
            **release_evidence,
            "capture": {"path": capture_path, "sha256": hashlib.sha256(capture_raw).hexdigest(),
                        "ida_version": capture["ida_version"], "hexrays_version": capture["hexrays_version"]},
            "frontend": {"path": _LIGHTING_JS, "sha256": hashlib.sha256(js_raw).hexdigest()},
            "architecture": pe["architecture"], "version_resources": versions,
            "imports": [entry["library"] for entry in pe["imports"]],
            "dynamic_system_imports": ["mfplat.dll"], "functions": functions,
            "virtual_calls": virtual_calls,
            "abi": {name: {"rva": rva, "signature": abi} for name, (rva, abi) in _LIGHTING_EXPORTS.items()},
            "software_creation": "config selects CDeviceAccess geometry, not PID discovery; CDevice init only queries the selected access object's in-memory dimensions and allocates an unsubmitted threadpool work item; add-device registers engine ownership and copies geometry/maps, position only resizes software layout; CDeviceAccess transport slots 8..40 return E_FAIL",
            "sampling": "Action 50 preview=1 sets fade denominator=1 and suppresses CDevice output; fixed stationary nonblack scenarios wait only for all nine initialized-zero pixels to publish, independently of Rust expected values; source buffer then remains constant",
            "cleanup": "disable waits for engine timer callbacks; destroy engine/device; terminate; stop operating timer and unregister callback. Native CoInitialize and MFStartup are retained to isolated process exit; no manual unload"}


def _lighting_geometry():
    return {"Version": 2, "VID": 5426, "PID": 3942, "EID": 0, "Layout": 0,
            "MatrixMaxRow": 3, "MatrixMaxCol": 3, "DeviceMaxRow": 3, "DeviceMaxCol": 3,
            "LedWidth": 40, "LedHeight": 40, "LedWidthOffset": 0, "LedHeightOffset": 0,
            "MaxRegion": 0, "RegionMaxRow": 0, "RegionMaxCol": 0,
            "LedMatrix": [{"MatrixPos": [r, c], "DevicePos": [r, c]} for r in range(3) for c in range(3)],
            "LedInputMap": []}


def _lighting_scenarios():
    return [("static", {"effect": 6, "EffectParam": {"Color": 0x332211}}),
            ("wheel_stationary", {"effect": 13, "EffectParam": {
                "Mode": 2048, "Speed": 0.0, "Cycles": -1, "CenterRow": 1, "CenterCol": 1,
                "ColorStops": [{"Stop": 0, "Color": 0x0000ff}, {"Stop": 100, "Color": 0xff0000}]}})]


def _lighting_snapshot(value):
    if not isinstance(value, dict) or value.get("schema_version") != 1 or not isinstance(value.get("scenarios"), list):
        raise ValueError("Rust lighting snapshot schema differs")
    if len(value["scenarios"]) != 2:
        raise ValueError("Rust lighting snapshot must contain both fixed scenarios")
    for row, (name, effect) in zip(value["scenarios"], _lighting_scenarios(), strict=True):
        if (not isinstance(row, dict) or row.get("name") != name or row.get("geometry") != _lighting_geometry()
                or row.get("effect") != effect or row.get("epoch_seconds") != 0
                or row.get("cleanup") != {"engine_destroyed": True, "device_destroyed": True, "terminated": True}):
            raise ValueError("Rust lighting snapshot differs from audited software-only scenario")
        frame = row.get("stable_packed_colorref")
        if not isinstance(frame, list) or len(frame) != 9 or any(type(v) is not int or not 0 <= v <= 0xffffff for v in frame):
            raise ValueError("Rust lighting snapshot frame is incomplete")
        ticks = row.get("ticks")
        if (not isinstance(ticks, list) or any(not isinstance(tick, dict) for tick in ticks)
                or [tick.get("logical_ms") for tick in ticks] != list(range(0, 4001, 40))):
            raise ValueError("Rust lighting logical timeline is incomplete")
        for tick in ticks:
            frames = tick.get("frames")
            if not isinstance(frames, list) or len(frames) > 1:
                raise ValueError("Rust lighting tick frame count differs")
            for emitted in frames:
                pixels = emitted.get("packed_colorref") if isinstance(emitted, dict) else None
                if (not isinstance(emitted, dict) or type(emitted.get("device_handle")) is not int or emitted["device_handle"] <= 0
                        or not isinstance(pixels, list) or len(pixels) != 9
                        or any(type(v) is not int or not 0 <= v <= 0xffffff for v in pixels)):
                    raise ValueError("Rust lighting tick frame is incomplete")
    return value


def _lighting_session(library, emit, *, ready_timeout=5):
    """Only public JSON exports and fixed in-memory geometry; no device I/O."""
    for name in ("GetDLLVersion", "PollEvents"):
        getattr(library, name).argtypes, getattr(library, name).restype = [], ctypes.c_void_p
    library.RzLightingApi.argtypes, library.RzLightingApi.restype = [ctypes.c_char_p], ctypes.c_void_p
    library.RzLightingApiNoReturn.argtypes, library.RzLightingApiNoReturn.restype = [ctypes.c_char_p], None
    library.FreeMalloc.argtypes, library.FreeMalloc.restype = [ctypes.c_void_p], None
    library.SetOperatingMode.argtypes, library.SetOperatingMode.restype = [ctypes.c_int32], None
    library.SetNodeFFIEvent.argtypes, library.SetNodeFFIEvent.restype = [ctypes.c_void_p], ctypes.c_bool
    events, errors, scenarios = [], [], []

    def record(event):
        events.append(event)
        emit(event)

    def receive(pointer):
        try:
            record({"operation": "callback", "raw": ctypes.string_at(pointer).decode("utf-8")})
        except Exception as error:
            errors.append("Callback copy failed: " + str(error))

    callback = ctypes.CFUNCTYPE(None, ctypes.c_void_p)(receive)
    _SIMPLE_WORKER_OWNERS.append((library, callback))

    def owned(function, *arguments):
        pointer = function(*arguments)
        if not pointer:
            raise ValueError("Lighting export returned a null owned string")
        try:
            return ctypes.string_at(pointer).decode("utf-8")
        finally:
            library.FreeMalloc(pointer)

    def api(action, expected=0, **arguments):
        request = {"Action": action, **arguments}
        encoded = json.dumps(request, separators=(",", ":")).encode("utf-8")
        raw = owned(library.RzLightingApi, encoded)
        result = json.loads(raw)
        record({"operation": "api", "request": request, "raw": raw, "result": result})
        if type(result.get("Status")) is not int or result["Status"] != expected:
            raise ValueError("Lighting action status differs: " + str(action))
        return result

    def cleanup(action, **arguments):
        try:
            api(action, **arguments)
            return True
        except Exception as error:
            errors.append(f"Cleanup action {action}: {error}")
            return False

    try:
        version = owned(library.GetDLLVersion)
        record({"operation": "GetDLLVersion", "raw": version})
        if version != "4.0.55.0":
            raise ValueError("Executed lighting version differs from source version")
        if not library.SetNodeFFIEvent(ctypes.cast(callback, ctypes.c_void_p)):
            raise ValueError("Lighting callback registration failed")
        library.SetOperatingMode(1)
        record({"operation": "SetOperatingMode", "mode": 1, "returns": "void"})
        record({"operation": "PollEvents", "raw": owned(library.PollEvents)})
        for name, effect in _lighting_scenarios():
            device = engine = None
            result = {"name": name, "frame": None, "cleanup": {}}
            scenarios.append(result)
            try:
                device = api(1, config=_lighting_geometry())["device_handle"]
                engine = api(3, fps=25, type="")["engine_handle"]
                if type(device) is not int or type(engine) is not int or device <= 0 or engine <= 0:
                    raise ValueError("Lighting software handle is invalid")
                api(17, engine_handle=engine, device_handle=device, region=0, orientation=0)
                api(19, engine_handle=engine, device_handle=device, region=0, row=0, col=0,
                    layer=0, check_overlapping=True)
                api(33, engine_handle=engine, **effect)
                api(50, engine_handle=engine, enable=1)
                reset = {"Action": 51, "engine_handle": engine}
                library.RzLightingApiNoReturn(json.dumps(reset).encode("utf-8"))
                record({"operation": "RzLightingApiNoReturn", "request": reset, "returns": "void"})
                api(51, engine_handle=engine)
                api(49, engine_handle=engine, enable=1, device_handle=0, region=0, clearFrame=0)
                deadline = time.monotonic() + ready_timeout
                # No expected-frame match or wall-time approximation selects a
                # sample. The fixed inputs produce a constant nonblack frame;
                # initialized zero pixels distinguish incomplete publication.
                while True:
                    frame = api(52, engine_handle=engine, device_handle=device, region=0).get("frame")
                    if not isinstance(frame, list) or len(frame) != 9 or any(type(v) is not int or not 0 <= v <= 0xffffff for v in frame):
                        raise ValueError("Lighting frame shape differs")
                    if all(frame):
                        result["frame"] = frame
                        break
                    if time.monotonic() >= deadline:
                        raise TimeoutError("Lighting full stationary frame was not published")
                    # Yield between observations; only the published pixel
                    # state, never elapsed time, admits a comparison sample.
                    time.sleep(0.005)
                api(49, engine_handle=engine, enable=0, device_handle=0, region=0, clearFrame=0)
                result["disabled_frame"] = api(52, expected=0x80004005,
                    engine_handle=engine, device_handle=device, region=0)
            except Exception as error:
                result["error"] = f"{type(error).__name__}: {error}"
            finally:
                if type(engine) is int and engine > 0:
                    result["cleanup"]["disable"] = cleanup(49, engine_handle=engine, enable=0,
                        device_handle=0, region=0, clearFrame=0)
                    result["cleanup"]["destroy_engine"] = cleanup(4, engine_handle=engine)
                if type(device) is int and device > 0:
                    result["cleanup"]["destroy_device"] = cleanup(2, device_handle=device)
        record({"operation": "PollEvents", "raw": owned(library.PollEvents)})
    except Exception as error:
        errors.append(f"{type(error).__name__}: {error}")
    finally:
        terminated = cleanup(65520)
        try:
            library.SetOperatingMode(0)
            record({"operation": "SetOperatingMode", "mode": 0, "returns": "void"})
        except Exception as error:
            errors.append("Stop operating timer: " + str(error))
        try:
            if not library.SetNodeFFIEvent(None):
                raise ValueError("Callback unregister returned false")
            record({"operation": "SetNodeFFIEvent", "registered": False})
        except Exception as error:
            errors.append("Callback unregister: " + str(error))
    return {"scenarios": scenarios, "errors": errors, "terminated": terminated,
            "events": events, "module_and_callback_retained_until_process_exit": True,
            "native_com_and_media_foundation_cleanup":
                "matching shutdown exports not found in the audited lifecycle; owners remain until isolated process exit"}


def _lighting_system_modules(kernel):
    process = kernel.GetCurrentProcess()
    modules = (ctypes.c_void_p * 4096)()
    needed = ctypes.c_uint32()
    if not kernel.K32EnumProcessModules(process, modules, ctypes.sizeof(modules), ctypes.byref(needed)):
        raise ctypes.WinError(ctypes.get_last_error())
    if needed.value > ctypes.sizeof(modules):
        raise ValueError("Loaded module inventory exceeded its complete buffer")
    result = set()
    for module in modules[:needed.value // ctypes.sizeof(ctypes.c_void_p)]:
        buffer = ctypes.create_unicode_buffer(32768)
        count = kernel.K32GetModuleFileNameExW(process, module, buffer, len(buffer))
        if count == 0 or count >= len(buffer):
            raise ValueError("Loaded module path is incomplete")
        result.add(Path(buffer.value).resolve(strict=True))
    return result


def _lighting_worker_main(root):
    def emit(value):
        print("RAZER_LIGHTING_NATIVE_EVENT " + json.dumps(value, ensure_ascii=True), flush=True)

    try:
        if sys.platform != "win32" or ctypes.sizeof(ctypes.c_void_p) != 8:
            raise ValueError("Lighting comparison requires Windows x64 Python")
        root = Path(root).resolve(strict=True)
        evidence = verify_lighting_source(root)
        kernel = ctypes.WinDLL("kernel32.dll", use_last_error=True, winmode=0x800)
        for name, arguments, result in (
            ("GetCurrentProcess", [], ctypes.c_void_p),
            ("GetSystemDirectoryW", [ctypes.c_wchar_p, ctypes.c_uint32], ctypes.c_uint32),
            ("SetDefaultDllDirectories", [ctypes.c_uint32], ctypes.c_bool),
            ("K32EnumProcessModules", [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint32)], ctypes.c_bool),
            ("K32GetModuleFileNameExW", [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint32], ctypes.c_uint32),
        ):
            getattr(kernel, name).argtypes, getattr(kernel, name).restype = arguments, result
        directory = ctypes.create_unicode_buffer(32768)
        count = kernel.GetSystemDirectoryW(directory, len(directory))
        if count == 0 or count >= len(directory):
            raise ValueError("System DLL directory is unavailable")
        system = Path(directory.value).resolve(strict=True)
        if not kernel.SetDefaultDllDirectories(0x800):
            raise ctypes.WinError(ctypes.get_last_error())

        def identity(path):
            raw = path.read_bytes()
            pe = inspect_pe(raw)
            if pe["architecture"] != "x64" or pe["unresolved"]:
                raise ValueError("Loaded system dependency architecture or PE closure is incomplete")
            return {"path": str(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                    "architecture": pe["architecture"], "version_resources": pe["version_resources"],
                    "imports": [entry["library"] for entry in pe["imports"]],
                    "delay_imports": [entry["library"] for entry in pe.get("delay_imports", [])],
                    "unresolved": pe["unresolved"]}

        dependencies = []
        for name in evidence["imports"] + evidence["dynamic_system_imports"]:
            if Path(name).name != name:
                raise ValueError("Lighting dependency contains a path")
            path = (system / name).resolve(strict=True)
            if path.parent != system:
                raise ValueError("Lighting dependency escaped System32")
            dependencies.append(identity(path))
        baseline = _lighting_system_modules(kernel)
        names = {name.casefold() for name in evidence["imports"] + evidence["dynamic_system_imports"]}
        if any(module.name.casefold() in names and module.parent != system for module in baseline):
            raise ValueError("A lighting dependency name is already mapped outside System32")
        emit({"operation": "dependencies_verified", "direct_and_dynamic": dependencies,
              "search": "LOAD_LIBRARY_SEARCH_SYSTEM32; absolute target path"})
        path = (root / evidence["input"]).resolve(strict=True)
        library = ctypes.CDLL(str(path), winmode=0x800)
        _SIMPLE_WORKER_OWNERS.append(library)
        emit({"operation": "loaded", "sha256": evidence["sha256"]})
        result = _lighting_session(library, emit)
        inventory = _lighting_system_modules(kernel)
        for module in sorted(inventory - baseline):
            if module != path and module.parent != system:
                raise ValueError("Lighting loaded a dependency outside System32: " + module.name)
        loaded = [identity(module) for module in sorted(inventory) if module == path or module.parent == system]
        for dependency in dependencies:
            if identity(Path(dependency["path"])) != dependency:
                raise ValueError("System dependency changed during comparison")
        result["dependencies"] = {"direct_and_dynamic": dependencies, "loaded_system_module_closure": loaded,
                                  "unexpected_non_system_modules": False}
        print("RAZER_LIGHTING_NATIVE_RESULT " + json.dumps(result, ensure_ascii=True), flush=True)
        return 0
    except Exception as error:
        emit({"operation": "error", "error": f"{type(error).__name__}: {error}"})
        return 1


def _lighting_input_identity(root, input_owner):
    identities = {row["path"]: row for row in input_owner(include_asset_pack=False)}
    for relative in ("tools/native_runtime.py", "tools/workspace.py", "tools/host.py", "tools/analysis.py",
                     "tools/test_native_runtime.py", "tools/test_workspace.py"):
        raw = (root / relative).read_bytes()
        identities[relative] = {"path": relative, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
    return [identities[key] for key in sorted(identities)]


def _lighting_build_environment(environment):
    """Pin compiler options without publishing their potentially private values."""
    names = {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS",
             "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_BUILD_RUSTDOCFLAGS",
             "RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
             "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
             "RUSTUP_TOOLCHAIN", "RUSTUP_HOME", "CARGO_HOME", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR"}
    names.update(name for name in environment if re.fullmatch(r"CARGO_TARGET_[A-Z0-9_]+_RUSTFLAGS", name))
    result = {}
    for name in sorted(names):
        if name in environment:
            raw = environment[name].encode("utf-8")
            result[name] = {"present": True, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        else:
            result[name] = {"present": False}
    return result


def compare_local_lighting(root, input_owner, *, publish=None):
    """Explicit isolated stationary software output comparison, without devices."""
    command = ["cargo", "test", "--locked", "--offline", "-p", "razer-device", "--lib",
               "lighting_engine::renderer::tests::explicit_native_lighting_snapshots", "--",
               "--ignored", "--exact", "--nocapture", "--test-threads=1"]
    worker = [sys.executable, "-I", str(Path(__file__).resolve()), "--lighting-worker", str(root)]
    receipt = {"schema_version": 1, "evidence": None, "rust_inputs": [], "build_environment": {},
               "command": command, "original_command": worker, "runs": {}, "passed": False,
               "status": "preparing", "phase": "source_verification", "cancelled": False,
               "scope": "Static and stationary Wheel exact opacity=1 software frames through public JSON exports; no physical device output",
               "unverified": ["Fade timing", "Dynamic effect timing", "Other effects and parameter branches",
                              "Physical device, service and UI output", "Cancellation during a native call"]}
    try:
        root = Path(root).resolve(strict=True)
        worker[-1] = str(root)
        evidence = receipt["evidence"] = verify_lighting_source(root)
        receipt["phase"] = "input_identity"
        inputs = receipt["rust_inputs"] = _lighting_input_identity(root, input_owner)
        environment = os.environ.copy()
        receipt["build_environment"] = _lighting_build_environment(environment)
        runs = receipt["runs"]

        def run(name, argv, **options):
            receipt["phase"] = name
            return _run_service_version_test(argv, root, environment.copy(),
                observe=lambda observation: runs.__setitem__(name, observation), **options)

        runs["rust_before"] = run("rust_before", command)
        before = _lighting_snapshot(_marked_result(runs["rust_before"], "RAZER_LIGHTING_RESULT"))
        receipt["rust_before"] = before
        runs["original"] = run("original", worker, timeout_seconds=45)
        original = _marked_result(runs["original"], "RAZER_LIGHTING_NATIVE_RESULT")
        receipt["original"] = original
        runs["rust_after"] = run("rust_after", command)
        after = _lighting_snapshot(_marked_result(runs["rust_after"], "RAZER_LIGHTING_RESULT"))
        receipt["rust_after"] = after
        receipt["phase"] = "comparison"
        if (before != after or inputs != _lighting_input_identity(root, input_owner)
                or evidence != verify_lighting_source(root)
                or receipt["build_environment"] != _lighting_build_environment(os.environ.copy())):
            raise ValueError("Comparison inputs or independent Rust output changed during execution")
        if original["errors"] or not original["terminated"] or len(original["scenarios"]) != 2:
            raise ValueError("Native session or cleanup failed")
        comparisons = []
        for actual, expected in zip(original["scenarios"], before["scenarios"], strict=True):
            cleanup = actual.get("cleanup", {})
            comparisons.append({"name": expected["name"], "rust": expected["stable_packed_colorref"],
                "original": actual.get("frame"), "matched": actual.get("name") == expected["name"]
                and actual.get("frame") == expected["stable_packed_colorref"]
                and not actual.get("error") and set(cleanup) == {"disable", "destroy_engine", "destroy_device"}
                and all(cleanup.values())})
        receipt["comparisons"] = comparisons
        receipt["passed"] = all(row["matched"] for row in comparisons)
        receipt["status"] = "passed" if receipt["passed"] else "failed"
        receipt["phase"] = "completed"
    except Exception as error:
        receipt["status"] = "failed"
        receipt["error"] = f"{type(error).__name__}: {error}"
    except BaseException as error:
        receipt["status"] = "cancelled"
        receipt["cancelled"] = True
        receipt["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        receipt["recorded_at_utc"] = datetime.now(timezone.utc).isoformat()
        if publish is not None:
            publish(receipt)
    return receipt


def lighting_attempt_history(previous):
    """Reject malformed existing evidence before starting another comparison."""
    if previous is None:
        attempts = []
    elif not isinstance(previous, dict):
        raise ValueError("Existing lighting receipt schema is unknown; refusing to overwrite")
    elif previous.get("schema_version") == 2:
        attempts = previous.get("attempts")
        if (not isinstance(attempts, list) or not attempts
                or previous.get("latest_attempt") != len(attempts) - 1
                or any(not isinstance(row, dict) for row in attempts)):
            raise ValueError("Existing lighting attempt receipt is incomplete; refusing to overwrite")
    elif previous.get("schema_version") == 1 and "passed" in previous:
        attempts = [previous]
    else:
        raise ValueError("Existing lighting receipt schema is unknown; refusing to overwrite")
    return attempts


def retain_lighting_attempt(previous, attempt):
    """Keep failed raw observations in the single comparison receipt owner."""
    attempts = [*lighting_attempt_history(previous), attempt]
    return {"schema_version": 2, "latest_attempt": len(attempts) - 1,
            "passed": attempt["passed"], "scope": attempt["scope"],
            "recorded_at_utc": attempt["recorded_at_utc"], "attempts": attempts}


def verify_simple_version_source(root, *, include_apps=False):
    """Static source/ABI receipt; no target DLL or local COM activation."""
    import uuid
    root = Path(root)
    raw = (root / _SIMPLE_INPUT).read_bytes()
    if len(raw) != 2570440 or hashlib.sha256(raw).hexdigest() != _SIMPLE_SHA256:
        raise ValueError("Simple Service bytes differ from the current native capture")
    pe = inspect_pe(raw)
    if pe["architecture"] != "x64" or pe["unresolved"]:
        raise ValueError("Simple Service PE/architecture is not verified")
    image_base = int(pe["image_base"], 16)

    def at(rva, length):
        for section in pe["sections"]:
            if section["rva"] <= rva and rva + length <= section["rva"] + section["bytes"]:
                offset = section["file_offset"] + rva - section["rva"]
                return raw[offset:offset + length]
        raise ValueError("Source evidence RVA has no complete raw bytes")

    capture_path = f".analysis/results/native/{_SIMPLE_SHA256}/capture.json"
    capture = json.loads((root / capture_path).read_text(encoding="utf-8"))
    if capture["input_sha256"] != _SIMPLE_SHA256 or capture["input_bytes"] != len(raw):
        raise ValueError("Simple Service capture input differs")
    functions = []
    for rva in dict.fromkeys(_SIMPLE_FUNCTIONS + (_APPS_FUNCTIONS if include_apps else ())):
        path = Path(capture_path).parent / "functions" / (rva + ".json")
        encoded = (root / path).read_bytes()
        function = json.loads(encoded)
        if not (function.get("pseudocode") or {}).get("lines") or not function.get("assembly"):
            raise ValueError("Complete native source body is absent: " + rva)
        for chunk in function["chunks"]:
            start, end = int(chunk["start"], 16), int(chunk["end"], 16)
            actual = at(start - image_base, end - start)
            if actual.hex() != chunk["raw_hex"] or hashlib.sha256(actual).hexdigest() != chunk["sha256"]:
                raise ValueError("Native capture bytes differ: " + rva)
        functions.append({"path": path.as_posix(), "sha256": hashlib.sha256(encoded).hexdigest(),
                          "rva": function["rva"], "chunks": function["chunks"],
                          "complete_function": True, "bytes_verified": True})
    guids = {}
    for name, rva, expected in (
        ("clsid", 0x1f8234, "0edeaf3c-d36e-4e7e-9467-900b977dc4ff"),
        ("iid", 0x1f8214, "bfe24d59-6568-4179-8ae5-d9d53869a3e3"),
    ):
        encoded = at(rva, 16)
        if str(uuid.UUID(bytes_le=encoded)) != expected:
            raise ValueError("COM identity differs from captured caller")
        guids[name] = {"rva": hex(rva), "bytes": encoded.hex(), "value": expected}
    exports = {name: row for row in pe["exports"] for name in row["names"]}
    for name, rva in (("simpleServiceInitialize", "0xedf0"),
                      ("simpleGetVersionInfo", "0xf420"), ("simpleServiceShutdown", "0xf090"),
                      ("simpleGetUserApps", "0x10290")):
        if exports.get(name, {}).get("rva") != rva or "forwarder" in exports[name]:
            raise ValueError("Simple Service export differs: " + name)
    apps = {}
    if include_apps:
        signer_names = []
        for index in range(2):
            pointer = struct.unpack("<Q", at(0x24b090 + index * 8, 8))[0]
            signer_names.append(at(pointer - image_base, 32).split(b"\0", 1)[0].decode("ascii"))
        if signer_names != ["Razer USA Ltd.", "THX LTD."]:
            raise ValueError("Apps signer policy differs from source bytes")
        producers = {}
        for producer, caller in (("1dcb6", "0x1800278d3"), ("1df22", "0x180027a65"),
                                 ("1e1a2", "0x180027ccf"), ("1e232", "0x180028086")):
            path = Path(capture_path).parent / "functions" / (producer + ".json")
            body = json.loads((root / path).read_bytes())
            calls = body["xrefs_to_entry"]
            if len(calls) != 1 or calls[0]["from"] != caller or not calls[0]["code"]:
                raise ValueError("Apps mutation event has an unclassified producer: " + producer)
            producers[producer] = calls
        apps = {"allowed_signer_common_names": signer_names,
                "signature": "ZIP bypass; offline WinTrust file/handle verification for PE/MSI/MSIX; custom ICO iTXt PKCS#7 reconstructed-content digest, signed-attribute digest and RSA-SHA1/RSA-SHA256/ECDSA-SHA256 verification; signer attempts run Razer then THX and the last failed attempt owns the failure reason",
                "mutation_event_producers": producers,
                "subscriptions": "0x2150/0x23f0 toggle singleton enable flag; 0x2690 owns callback. No filesystem watch or cross-process mutation producer. Already posted callbacks survive unregister; JS broadcasts to all live webContents",
                "cleanup": "Rust owns input/file/provider/certificate/key and staging lifetimes; verification failure and cleanup failure are retained together; queue drains independently and shutdown clears its owner",
                "runtime_scope": "Pure tests use synthetic certificates and disposable directories only; no installed file write, remove, helper launch or vendor mutation comparison"}
    return {"input": _SIMPLE_INPUT, "sha256": _SIMPLE_SHA256, "bytes": len(raw),
            "architecture": pe["architecture"], "capture": capture_path,
            "ida_version": capture["ida_version"], "hexrays_version": capture["hexrays_version"],
            "functions": functions, "com_identity": guids, "apps": apps,
            "abi": {"apartment": 6, "class_context": 4, "version_slot": 8,
                    "method": "HRESULT(this, BSTR*)", "cleanup": "SysFreeString, IUnknown::Release, CoUninitialize"},
            "semantics": {
                "source_build_constants": {"Simple Service Version":"1.2.2.10", "Apps Service Version":"1.2.2.4", "Audio Service Version":"1.2.1.16"},
                "observed_field": "Elevation Service Version",
                "source_failure": "RVA 0x28200 discards RVA 0x2dc9a failure and serializes an empty elevation string as callback success",
                "rust_failure": "Source build metadata is retained; elevation uses observed/value or failed/error, with no fabricated observation",
                "lifecycle": "Initialize callback precedes query; shutdown stops service owners and waits before its completion callback; the test retains the mapped module until isolated process exit"},
            "remaining": ["This query does not replace other Simple Service methods or the installed Elevation Service implementation"]}


_APP_FILTERS = ("", "ChromaApp", "chromaapp", "__razer_inventory_missing__")


def _simple_comparison(mode, before, original, after):
    """Keep state changes and failed observations distinct from equivalence."""
    if before != after:
        return {"passed": False, "reason": "Direct observations changed during comparison"}
    if mode == "version":
        elevation = before.get("Elevation Service Version", {})
        if elevation.get("state") != "observed":
            return {"passed": False, "reason": "Direct service version was not observed"}
        expected = dict(before)
        expected["Elevation Service Version"] = elevation["value"]
        equal = original.get("result") is True and original.get("reason") == "" and original.get("value") == expected
    elif mode == "apps":
        if [row.get("name") for row in before] != list(_APP_FILTERS):
            return {"passed": False, "reason": "Direct inventory filters are incomplete"}
        def inventory(rows, key):
            result = []
            for row in rows:
                value = dict(row[key])
                if value["userApps"] != "":
                    value["userApps"] = json.loads(value["userApps"])
                result.append({"name": row["name"], "value": value})
            return result
        equal = inventory(original, "original") == inventory(before, "rust")
    else:
        raise ValueError("Unknown Simple Service comparison")
    return {"passed": equal, "reason": None if equal else "Original and independent observations differ"}


def _marked_result(completed, marker):
    if completed["exit_code"] != 0 or completed["timed_out"]:
        raise ValueError("Comparison process did not complete successfully")
    rows = [line.split(marker + " ", 1)[1] for line in completed["stdout"].splitlines()
            if marker + " " in line]
    if len(rows) != 1:
        raise ValueError("Comparison process must emit exactly one result")
    return json.loads(rows[0])


def _compare_simple(root, mode):
    test, marker = {
        "version": ("simple_version::tests::direct_service_version_read_only", "RAZER_VERSION_RESULT"),
        "apps": ("user_app_files::tests::direct_user_apps_read_only", "RAZER_APPS_RESULT"),
    }[mode]
    command = ["cargo", "test", "--locked", "-p", "razer-service", "--lib",
               "runtime::native::" + test,
               "--", "--ignored", "--exact", "--nocapture", "--test-threads=1"]
    worker = [sys.executable, "-I", str(Path(__file__).resolve()), "--simple-worker", str(root), mode]
    runs = {}
    try:
        runs["before"] = _run_service_version_test(command, root, os.environ.copy())
        before = _marked_result(runs["before"], marker)
        runs["original"] = _run_service_version_test(worker, root, os.environ.copy())
        original = _marked_result(runs["original"], "RAZER_SIMPLE_RESULT")
        runs["after"] = _run_service_version_test(command, root, os.environ.copy())
        after = _marked_result(runs["after"], marker)
        result = _simple_comparison(mode, before, original["value"], after)
        result.update({"before": before, "original": original, "after": after})
    except (ValueError, KeyError, TypeError) as error:
        result = {"passed": False, "reason": str(error)}
    return {"command": command, "original_command": worker, "runs": runs, "result": result,
            "passed": result["passed"]}


def compare_local_service_version(root):
    """Audited read-only original DLL versus direct Rust COM test receipt."""
    import shutil
    import winreg
    root = Path(root).resolve()
    evidence = verify_simple_version_source(root)
    clsid = evidence["com_identity"]["clsid"]["value"]
    with winreg.OpenKey(winreg.HKEY_CLASSES_ROOT, f"CLSID\\{{{clsid}}}\\LocalServer32") as key:
        registered, _ = winreg.QueryValueEx(key, None)
    registered = registered.strip()
    if not (registered.startswith('"') and registered.endswith('"')):
        raise ValueError("Elevation LocalServer32 must be the observed quoted executable without arguments")
    server = Path(registered[1:-1]).resolve(strict=True)
    if server.name.casefold() != "razer_elevation_service.exe":
        raise ValueError("COM registration points to a different local server")
    raw = server.read_bytes()
    pe = inspect_pe(raw)
    if pe["architecture"] != "x64" or pe["unresolved"]:
        raise ValueError("Installed Elevation Service PE/architecture is not verified")
    quoted = "'" + str(server).replace("'", "''") + "'"
    signature = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command",
        "$s=Get-AuthenticodeSignature -LiteralPath " + quoted +
        "; @{status=$s.Status.ToString();subject=$s.SignerCertificate.Subject;thumbprint=$s.SignerCertificate.Thumbprint}|ConvertTo-Json -Compress"],
        capture_output=True, text=True, encoding="utf-8", check=True, timeout=30)
    signer = json.loads(signature.stdout)
    if signer["status"] != "Valid" or not signer["subject"].startswith("CN=Razer USA Ltd.,"):
        raise ValueError("Installed Elevation Service signature is not verified Razer")
    server_input = root / ".analysis/inputs/installed-current/files/elevation-service/razer_elevation_service.exe"
    server_input.parent.mkdir(parents=True, exist_ok=True)
    if not server_input.exists() or server_input.read_bytes() != raw:
        shutil.copyfile(server, server_input)
    if server_input.read_bytes() != raw:
        raise ValueError("Private Elevation Service evidence copy differs")
    implementation_paths = ["Cargo.lock", "crates/razer-service/src/runtime/windows/native.rs",
        "crates/razer-service/src/runtime/windows/simple_version.rs",
        "crates/razer-service/src/runtime/windows/simple_version_tests.rs", "tools/native_runtime.py"]
    inputs = {path: hashlib.sha256((root / path).read_bytes()).hexdigest() for path in implementation_paths}
    completed = _compare_simple(root, "version")
    unchanged = all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest
                    for path, digest in inputs.items()) and server.read_bytes() == raw
    passed = completed["passed"] and unchanged
    return {"evidence": evidence, "mode": "read-only",
            "server": {"input": server_input.relative_to(root).as_posix(),
                       "sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw),
                       "architecture": pe["architecture"], "versions": pe["version_resources"], "signature": signer},
            "implementation_inputs": inputs, "inputs_unchanged": unchanged,
            **completed,
            "passed": passed,
            "scope": "Actual source DLL Initialize/GetVersionInfo/Shutdown callbacks and direct Rust local COM version query; no application window, installer, JS, firmware or device write"}


def compare_local_app_files(root):
    """Original callback versus actual direct inventory; never changes files."""
    root = Path(root).resolve()
    evidence = verify_simple_version_source(root, include_apps=True)
    evidence["apps_inventory"] = {
        "export": "simpleGetUserApps(const char*, void(bool,const char*,const char*))",
        "chain": ["10290", "2a50", "2c30", "21dd4", "21fb0", "26caa"],
        "root": "SHGetFolderPathW(NULL,28,NULL,0) / Razer / RazerAppEngine / User Data / Apps",
        "format": "JSON array of single-entry app-name dictionaries; each value is a recursive file list with name and uppercase sha256",
        "ordering": "Windows enumerator order; current directory files precede child directories, pending children traversed LIFO; reparse children skipped",
        "errors": "PathExists is used before enumeration (not IsDirectory). Missing root/app have separate reasons; empty root and case-mismatched filter return false with empty reason and empty userApps: 0x21fb0 discards producer [] on its failure branch",
        "read_failure": "Rust returns result=false/reason for actual I/O failure, retaining all independent successful file observations in userApps; no fabricated empty hash",
        "remaining": ["This live comparison covers inventory only. Mutation/signature/event implementations use pure/synthetic tests; process launch and system monitor still require independent implementation"],
    }
    paths = ["Cargo.lock", "crates/razer-storage/src/user_app_files.rs",
             "crates/razer-service/src/runtime/windows/user_app_files.rs",
             "crates/razer-platform/src/native_paths.rs", "tools/native_runtime.py"]
    inputs = {path: hashlib.sha256((root / path).read_bytes()).hexdigest() for path in paths}
    completed = _compare_simple(root, "apps")
    unchanged = all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest
                    for path, digest in inputs.items())
    return {"evidence": evidence, "mode": "read-only",
            "implementation_inputs": inputs, "inputs_unchanged": unchanged,
            **completed,
            "passed": completed["passed"] and unchanged,
            "scope": "Actual source DLL Initialize/GetUserApps/Shutdown callbacks compared with direct Rust current-file names/hashes; no writes, helpers or application windows"}


def _run_service_version_test(command, root, environment, *, timeout_seconds=180, observe=None):
    process = subprocess.Popen(command, cwd=root, env=environment, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace",
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    observation = {"exit_code": None, "stdout": "", "stderr": "", "timed_out": False,
                   "worker_waited": False, "forced_tree_cleanup": None}

    def terminate_tree():
        try:
            result = subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                capture_output=True, text=True, errors="replace", timeout=15)
            return {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
        except (OSError, subprocess.TimeoutExpired) as error:
            return {"exit_code": None, "error": f"{type(error).__name__}: {error}"}
        finally:
            if process.poll() is None:
                process.kill()

    def retain_partial(error):
        for name in ("stdout", "stderr"):
            value = getattr(error, name, None)
            if value is not None:
                observation[name] = value.decode("utf-8", errors="replace") if isinstance(value, bytes) else value

    def drain():
        try:
            observation["stdout"], observation["stderr"] = process.communicate(timeout=15)
        except (OSError, subprocess.TimeoutExpired) as error:
            retain_partial(error)
            observation["output_cleanup_error"] = f"{type(error).__name__}: {error}"

    try:
        try:
            observation["stdout"], observation["stderr"] = process.communicate(timeout=timeout_seconds)
        except subprocess.TimeoutExpired as error:
            observation["timed_out"] = True
            retain_partial(error)
            observation["forced_tree_cleanup"] = terminate_tree()
            drain()
        except BaseException as error:
            observation["interruption"] = type(error).__name__
            observation["forced_tree_cleanup"] = terminate_tree()
            drain()
            raise
    finally:
        try:
            process.wait()
            observation["worker_waited"] = True
        finally:
            observation["exit_code"] = process.returncode
            if observe is not None:
                observe(observation)
    return observation


# Callbacks and the mapped module remain alive until the isolated process exits,
# including when a native operation never calls back or shutdown fails.
_SIMPLE_WORKER_OWNERS = []


def _simple_session(library, mode, emit, timeout=30):
    if mode not in ("version", "apps"):
        raise ValueError("Unknown Simple Service query")
    completion_type = ctypes.CFUNCTYPE(None)
    query_type = ctypes.CFUNCTYPE(None, ctypes.c_bool, ctypes.c_char_p, ctypes.c_char_p)
    callbacks = []
    _SIMPLE_WORKER_OWNERS.append((library, callbacks))
    initialize = library.simpleServiceInitialize
    initialize.argtypes, initialize.restype = [completion_type], None
    shutdown = library.simpleServiceShutdown
    shutdown.argtypes, shutdown.restype = [completion_type], None
    query = library.simpleGetVersionInfo if mode == "version" else library.simpleGetUserApps
    query.argtypes = [query_type] if mode == "version" else [ctypes.c_char_p, query_type]
    query.restype = None
    events = []

    def record(event):
        events.append(event)
        emit(event)

    def complete(function, name):
        ready = threading.Event()
        callback = completion_type(ready.set)
        callbacks.append(callback)
        record({"operation": name, "phase": "call"})
        function(callback)
        if not ready.wait(timeout):
            raise TimeoutError(name + " callback did not complete")
        record({"operation": name, "phase": "callback"})

    def read(name=None):
        ready = threading.Event()
        values = []

        def receive(result, reason, value):
            # c_char_p converts bytes while the original callback buffer lives.
            values.append((bool(result), reason, value))
            ready.set()

        callback = query_type(receive)
        callbacks.append(callback)
        record({"operation": "query", "name": name, "phase": "call"})
        if name is None:
            query(callback)
        else:
            argument = name.encode("utf-8")
            callbacks.append(argument)
            query(argument, callback)
        if not ready.wait(timeout):
            raise TimeoutError("Query callback did not complete")
        if len(values) != 1:
            raise ValueError("Query emitted multiple callbacks")
        result, reason, payload = values[0]
        if reason is None or payload is None:
            raise ValueError("Query returned a null callback string")
        reason, payload = reason.decode("utf-8"), payload.decode("utf-8")
        value = {"result": result, "reason": reason,
                 "value" if mode == "version" else "userApps":
                     json.loads(payload) if mode == "version" and payload else payload}
        record({"operation": "query", "name": name, "phase": "callback", "value": value})
        return value

    complete(initialize, "initialize")
    try:
        result = read() if mode == "version" else [
            {"name": name, "original": read(name)} for name in _APP_FILTERS]
    except Exception as error:
        # A pending query may still own its callback; retain it through shutdown
        # and process exit even when the shutdown callback itself fails.
        try:
            complete(shutdown, "shutdown")
        except Exception as cleanup:
            raise RuntimeError(f"Query failed: {error}; shutdown failed: {cleanup}") from error
        raise
    complete(shutdown, "shutdown")
    return {"value": result, "lifecycle": events}


def _simple_worker_main(root, mode):
    def emit(value):
        print("RAZER_SIMPLE_EVENT " + json.dumps(value, ensure_ascii=True), flush=True)

    try:
        if sys.platform != "win32" or ctypes.sizeof(ctypes.c_void_p) != 8:
            raise RuntimeError("Simple Service comparison requires Windows x64 Python")
        if mode not in ("version", "apps"):
            raise ValueError("Unknown Simple Service query")
        root = Path(root).resolve(strict=True)
        evidence = verify_simple_version_source(root, include_apps=mode == "apps")
        path = root / evidence["input"]
        directory = os.add_dll_directory(str(path.parent))
        _SIMPLE_WORKER_OWNERS.append(directory)
        emit({"operation": "load", "sha256": evidence["sha256"]})
        library = ctypes.CDLL(str(path))
        _SIMPLE_WORKER_OWNERS.append(library)
        result = _simple_session(library, mode, emit)
        print("RAZER_SIMPLE_RESULT " + json.dumps(result, ensure_ascii=True), flush=True)
        return 0
    except Exception as error:
        emit({"operation": "error", "error": f"{type(error).__name__}: {error}"})
        return 1


_MOUSE_GETS = {
    # Current PID190 84816.I_ -> 30580.kD; the identity reply is not a setting.
    (22, 0, 130): b"",
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
    if len(sys.argv) == 3 and sys.argv[1] == "--lighting-worker":
        status = _lighting_worker_main(sys.argv[2])
        sys.stdout.flush()
        sys.stderr.flush()
        os._exit(status)
    if len(sys.argv) == 4 and sys.argv[1] == "--simple-worker":
        # Avoid Python finalization releasing callback objects while native
        # threads can still return through them after a failed cleanup.
        status = _simple_worker_main(sys.argv[2], sys.argv[3])
        sys.stdout.flush()
        sys.stderr.flush()
        os._exit(status)
    if sys.argv[1:] != ["--worker"]:
        raise SystemExit("Use tools/workspace.py for native runtime verification")
    raise SystemExit(_worker_main())
