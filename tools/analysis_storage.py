"""Compact retained analysis files without changing their logical bytes or paths."""
from concurrent.futures import ThreadPoolExecutor, as_completed
from contextlib import ExitStack, contextmanager
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import re

import analysis as a


PLAN = ".analysis/results/storage-compaction.json"
DATASETS = {"segments.json", "imports.json", "strings.json", "entrypoints.json",
            "names.json", "local_types.json", "structures.json", "all-functions.c", "all-functions.asm"}
FUNCTION_INDEXES = {"functions": "functions", "partition-helpers": "partition_helpers",
                    "shared-tail-helpers": "shared_tail_helpers", "boundary-helpers": "boundary_helpers",
                    "entry-partition-helpers": "entry_partition_helpers", "context-helpers": "context_helpers",
                    "preanalysis": "decompiler_prepasses"}


def native_function_parts(parts):
    if (len(parts) not in (6, 8) or parts[:3] != (".analysis", "results", "native")
            or not re.fullmatch(r"[0-9a-f]{64}", parts[3])
            or parts[-2] not in FUNCTION_INDEXES
            or not re.fullmatch(r"(?:0|[1-9a-f][0-9a-f]{0,15})\.json", parts[-1])):
        return False
    return (len(parts) == 6 and parts[-2] == "functions"
            or len(parts) == 8 and parts[4] == "supplements" and a.native_supplement_attempt_name(parts[5]))


def native_supplement_manifest_parts(parts):
    return (len(parts) == 7 and parts[:3] == (".analysis", "results", "native")
            and re.fullmatch(r"[0-9a-f]{64}", parts[3]) is not None
            and parts[4] == "supplements" and a.native_supplement_attempt_name(parts[5])
            and parts[6] == "capture.json")


def fixed_evidence_path(name):
    parts = Path(name).parts
    if (Path(name).is_absolute() or Path(name).as_posix() != name
            or parts[:2] != (".analysis", "results") or ".." in parts):
        raise ValueError("Compaction evidence is outside its fixed results path")
    root = a.ROOT.resolve(strict=True)
    path = root.joinpath(*parts)
    current = root
    for part in parts:
        current /= part
        if current.is_symlink() or (hasattr(current, "is_junction") and current.is_junction()):
            raise ValueError("Compaction target contains a filesystem link")
    if a._resolved_path_identity(path.resolve(strict=True)) != a._resolved_path_identity(path):
        raise ValueError("Compaction target resolved outside its fixed dataset path")
    return path


def native_dataset(name):
    if not isinstance(name, str):
        raise ValueError("Compaction target must be a project-relative dataset path")
    parts = Path(name).parts
    native = (len(parts) == 5 and parts[:3] == (".analysis", "results", "native")
              and re.fullmatch(r"[0-9a-f]{64}", parts[3]) and parts[4] in DATASETS)
    review = (len(parts) == 5 and parts[:4] == (".analysis", "results", "reverse-pending", "review-sources")
              and re.fullmatch(r"[0-9a-f]{64}\.json", parts[4]))
    if (Path(name).is_absolute() or not (native or review or native_function_parts(parts)
            or native_supplement_manifest_parts(parts))
            or Path(name).as_posix() != name):
        raise ValueError("Compaction target is outside the retained native dataset scope")
    return fixed_evidence_path(name)


class FileInfo(ctypes.Structure):
    _fields_ = [("attributes", wintypes.DWORD), ("creation", wintypes.FILETIME),
                ("access", wintypes.FILETIME), ("write", wintypes.FILETIME),
                ("volume", wintypes.DWORD), ("size_high", wintypes.DWORD),
                ("size_low", wintypes.DWORD), ("links", wintypes.DWORD),
                ("index_high", wintypes.DWORD), ("index_low", wintypes.DWORD)]


class StandardInfo(ctypes.Structure):
    _fields_ = [("allocated", ctypes.c_longlong), ("size", ctypes.c_longlong),
                ("links", wintypes.DWORD), ("delete_pending", ctypes.c_ubyte),
                ("directory", ctypes.c_ubyte)]


class CompressionInfo(ctypes.Structure):
    _fields_ = [("allocated", ctypes.c_longlong), ("format", wintypes.WORD),
                ("unit_shift", ctypes.c_ubyte), ("chunk_shift", ctypes.c_ubyte),
                ("cluster_shift", ctypes.c_ubyte), ("reserved", ctypes.c_ubyte * 3)]


class NativeFile:
    def __init__(self, path, writable=False):
        if os.name != "nt":
            raise ValueError("Transparent analysis compaction requires Windows NTFS")
        self.path = Path(os.path.abspath(path))
        expected_path = a._resolved_path_identity(self.path)
        self.api = ctypes.WinDLL("kernel32", use_last_error=True)
        self.api.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
            ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
        self.api.CreateFileW.restype = wintypes.HANDLE
        self.api.CloseHandle.argtypes = [wintypes.HANDLE]
        self.api.CloseHandle.restype = wintypes.BOOL
        self.api.GetFileInformationByHandle.argtypes = [wintypes.HANDLE, ctypes.POINTER(FileInfo)]
        self.api.GetFileInformationByHandle.restype = wintypes.BOOL
        self.api.GetFileInformationByHandleEx.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
        self.api.GetFileInformationByHandleEx.restype = wintypes.BOOL
        self.api.GetFinalPathNameByHandleW.argtypes = [wintypes.HANDLE, wintypes.LPWSTR, wintypes.DWORD, wintypes.DWORD]
        self.api.GetFinalPathNameByHandleW.restype = wintypes.DWORD
        self.api.ReadFile.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD,
                                     ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
        self.api.ReadFile.restype = wintypes.BOOL
        self.api.SetFilePointerEx.argtypes = [wintypes.HANDLE, ctypes.c_longlong,
                                             ctypes.POINTER(ctypes.c_longlong), wintypes.DWORD]
        self.api.SetFilePointerEx.restype = wintypes.BOOL
        self.api.DeviceIoControl.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.c_void_p,
            wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
        self.api.DeviceIoControl.restype = wintypes.BOOL
        self.handle = self.api.CreateFileW(str(self.path), 0xC0000000 if writable else 0x80000000,
            1, None, 3, 0x08200000, None)
        if self.handle == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            buffer = ctypes.create_unicode_buffer(32768)
            length = self.api.GetFinalPathNameByHandleW(self.handle, buffer, len(buffer), 0)
            if not length or length >= len(buffer):
                raise ctypes.WinError(ctypes.get_last_error())
            actual = a._resolved_path_identity(Path(buffer.value))
            if actual != expected_path:
                raise ValueError("Compaction handle resolved to a different file")
            self.info()
        except Exception:
            self.close()
            raise

    def close(self):
        if self.handle is not None:
            self.api.CloseHandle(self.handle)
            self.handle = None

    def info(self):
        info, standard, compressed = FileInfo(), StandardInfo(), CompressionInfo()
        if not self.api.GetFileInformationByHandle(self.handle, ctypes.byref(info)):
            raise ctypes.WinError(ctypes.get_last_error())
        if not self.api.GetFileInformationByHandleEx(self.handle, 1, ctypes.byref(standard), ctypes.sizeof(standard)):
            raise ctypes.WinError(ctypes.get_last_error())
        if not self.api.GetFileInformationByHandleEx(self.handle, 8, ctypes.byref(compressed), ctypes.sizeof(compressed)):
            raise ctypes.WinError(ctypes.get_last_error())
        if (info.attributes & (0x400 | 0x10 | 0x4000) or info.links != 1
                or standard.directory or standard.delete_pending):
            raise ValueError("Compaction target is linked, encrypted, a directory or pending deletion")
        return {"bytes": (info.size_high << 32) | info.size_low,
            "file_id": [info.volume, info.index_high, info.index_low],
            "write_time": (info.write.dwHighDateTime << 32) | info.write.dwLowDateTime,
            "allocated_bytes": compressed.allocated, "compressed": bool(info.attributes & 0x800)}

    def chunks(self):
        if not self.api.SetFilePointerEx(self.handle, 0, None, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        buffer, count = ctypes.create_string_buffer(4 * 1024 * 1024), wintypes.DWORD()
        while True:
            if not self.api.ReadFile(self.handle, buffer, len(buffer), ctypes.byref(count), None):
                raise ctypes.WinError(ctypes.get_last_error())
            if not count.value:
                return
            yield buffer.raw[:count.value]

    def read_bytes(self):
        return b"".join(self.chunks())

    def sha256(self):
        digest = hashlib.sha256()
        for block in self.chunks():
            digest.update(block)
        return digest.hexdigest()

    def set_compressed(self):
        value, returned = ctypes.c_ushort(1), wintypes.DWORD()
        if not self.api.DeviceIoControl(self.handle, 0x9C040, ctypes.byref(value), ctypes.sizeof(value),
                                       None, 0, ctypes.byref(returned), None):
            raise ctypes.WinError(ctypes.get_last_error())


@contextmanager
def open_dataset(path, writable=False):
    file = NativeFile(path, writable)
    try:
        yield file
    finally:
        file.close()


def verify_snapshot(actual, expected):
    if any(actual[key] != expected[key] for key in ("bytes", "file_id", "write_time")):
        raise ValueError("Retained dataset changed after the compaction plan")


def receipt_json(path, stack):
    path = fixed_evidence_path(a.rel(path))
    file = stack.enter_context(open_dataset(path))
    before = file.info()
    raw = file.read_bytes()
    verify_snapshot(file.info(), before)
    return json.loads(raw), {"path": a.rel(path), "bytes": len(raw),
                             "sha256": hashlib.sha256(raw).hexdigest()}


def function_output(path, stack, owned_manifest=None):
    """Bind one explicit function to its retained index and ended analyzer session."""
    from reverse_pending import process_alive

    parts = Path(a.rel(path)).parts
    manifest_file = native_supplement_manifest_parts(parts)
    output = path.parent if manifest_file else path.parent.parent
    result = a.ROOT.joinpath(*parts[:4])
    if owned_manifest is not None:
        if not manifest_file or owned_manifest.path != path:
            raise ValueError("Compaction manifest handle belongs to another target")
        before = owned_manifest.info()
        raw = owned_manifest.read_bytes()
        verify_snapshot(owned_manifest.info(), before)
        capture = json.loads(raw)
        record = {"path": a.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
    else:
        capture, record = receipt_json(output / "capture.json", stack)
    session_name = "batch-session.json" if len(parts) == 6 else "session.json"
    session, session_record = receipt_json(output / session_name, stack)
    plan, plan_record = receipt_json(output / "plan.json", stack)
    if (capture.get("schema_version") != 1 or capture.get("input_sha256") != parts[3]
            or plan.get("sha256") != parts[3] or plan.get("output") != a.rel(output)
            or capture.get("input") != plan.get("input")
            or any(row.get("target_execution") is not False for row in (capture, session))
            or ("target_execution" in plan and plan["target_execution"] is not False)
            or any(row.get("real_device_operations") is not False for row in (capture, session))):
        raise ValueError("Function compaction capture, plan or session has a different static original")
    if (not isinstance(session.get("finished_at_utc"), str) or not session["finished_at_utc"]
            or not isinstance(session.get("started_at_utc"), str) or not session["started_at_utc"]
            or type(session.get("exit_code")) is not int or type(session.get("passed")) is not bool
            or type(session.get("pid")) is not int or session["pid"] <= 0
            or process_alive(session["pid"]) is not False
            or session.get("state", "finished") not in ("finished", "failed")
            or ("process_released" in session and session["process_released"] is not True)):
        raise ValueError("Function compaction requires a terminal released analyzer session")
    if session["passed"] and (session["exit_code"] != 0 or capture.get("capture_complete") is not True):
        raise ValueError("Function compaction session contradicts its capture outcome")
    base_record = None
    if len(parts) == 8 or manifest_file:
        if plan.get("target_execution") is not False or plan.get("real_device_operations") is not False:
            raise ValueError("Function supplement plan is not explicitly static")
        if "-" in parts[5] and plan.get("analysis_request") is None:
            raise ValueError("Version-1 supplement attempt requires its immutable source-bound request")
        a.verify_native_supplement_request(plan, session)
        if (plan.get("attempt_id", parts[5]) != parts[5]
                or plan.get("function_selection") != capture.get("function_selection")):
            raise ValueError("Function supplement differs from its exact attempt plan")
        base, base_record = receipt_json(result / "capture.json", stack)
        expected_base = {key: base_record[key] for key in ("path", "sha256")}
        if (base.get("input_sha256") != parts[3] or base.get("capture_complete") is not True
                or capture.get("base_capture") != expected_base or plan.get("base_capture") != expected_base):
            raise ValueError("Function supplement base capture identity changed")
    if manifest_file:
        return {"path": "capture.json", "bytes": record["bytes"], "sha256": record["sha256"]}, {
            **record, "scope": "explicit_native_supplement_manifest", "artifact_group": "capture-manifest",
            "capture_complete": capture.get("capture_complete") is True,
            "session": session_record, "plan": plan_record, "base_capture": base_record,
            "session_passed": session["passed"], "session_exit_code": session["exit_code"]}
    group = path.parent.name
    index = FUNCTION_INDEXES[group]
    records = capture.get(index)
    if not isinstance(records, list):
        raise ValueError("Function compaction requires an explicit native capture function index")
    if group == "preanalysis":
        records = [{"rva": row.get("rva"), **{key: row.get("capture", {}).get(key)
                    for key in ("path", "bytes", "sha256")}}
                   for row in records if isinstance(row, dict) and isinstance(row.get("capture"), dict)]
    relative = group + "/" + path.name
    rva = "0x" + path.stem
    matches = [row for row in records if isinstance(row, dict)
               and (row.get("path") == relative or row.get("rva") == rva)]
    if (len(matches) != 1 or matches[0].get("path") != relative or matches[0].get("rva") != rva
            or not re.fullmatch(r"[0-9a-f]{64}", str(matches[0].get("sha256", "")))
            or ("bytes" in matches[0] and (type(matches[0]["bytes"]) is not int or matches[0]["bytes"] < 0))):
        raise ValueError("Function is not uniquely SHA-bound by its exact native capture index")
    return matches[0], {**record, "scope": "explicit_native_function",
        "artifact_group": group, "capture_index": index,
        "capture_complete": capture.get("capture_complete") is True,
        "session": session_record, "plan": plan_record,
        "session_passed": session["passed"], "session_exit_code": session["exit_code"],
        **({"base_capture": base_record} if base_record else {})}


def expected_output(path, stack, owned_manifest=None):
    if (native_function_parts(Path(a.rel(path)).parts)
            or native_supplement_manifest_parts(Path(a.rel(path)).parts)):
        return function_output(path, stack, owned_manifest)
    if path.parent.name == "review-sources":
        # Immutable review filenames bind the complete logical byte stream.
        expected = {"path": path.name, "bytes": path.stat().st_size, "sha256": path.stem}
        return expected, {"path": a.rel(path), "bytes": expected["bytes"], "sha256": path.stem,
                          "capture_complete": None, "scope": "immutable_native_static_review"}
    capture_path = path.parent / "capture.json"
    # An unfinished capture is still retained evidence. Compression changes no
    # logical bytes and must not promote its acquisition or semantic status.
    capture, record = receipt_json(capture_path, stack)
    if capture.get("schema_version") != 1 or capture.get("input_sha256") != path.parent.name:
        raise ValueError("Compaction requires the native capture for this exact original")
    matches = [row for row in capture.get("outputs", []) if row.get("path") == path.name]
    if len(matches) > 1 or (capture.get("capture_complete") is True and len(matches) != 1):
        raise ValueError("Dataset is not uniquely declared by its native capture")
    expected = matches[0] if matches else None
    if expected is not None and (not re.fullmatch(r"[0-9a-f]{64}", str(expected.get("sha256", "")))
            or type(expected.get("bytes")) is not int or expected["bytes"] < 0):
        raise ValueError("Retained capture output byte identity is malformed")
    return expected, {**record, "capture_complete": capture.get("capture_complete") is True}


def plan_file(name):
    path = native_dataset(name)
    with ExitStack() as stack:
        expected, capture = expected_output(path, stack)
        with open_dataset(path) as file:
            info = file.info()
            digest = file.sha256()
            verify_snapshot(file.info(), info)
    if expected is not None and (digest != expected["sha256"]
            or ("bytes" in expected and info["bytes"] != expected["bytes"])):
        raise ValueError("Retained dataset differs from its complete capture")
    return {"path": name, **info, "sha256": digest, "capture": capture,
            "capture_output_identity_available": expected is not None}


def apply_file(row):
    path = native_dataset(row["path"])
    with ExitStack() as stack:
        manifest_file = native_supplement_manifest_parts(Path(a.rel(path)).parts)
        file = stack.enter_context(open_dataset(path, writable=True)) if manifest_file else None
        expected, capture = expected_output(path, stack, file)
        if (capture != row["capture"] or (expected is not None and
                (expected["sha256"] != row["sha256"]
                 or ("bytes" in expected and expected["bytes"] != row["bytes"])))
                or (expected is not None) != row["capture_output_identity_available"]):
            raise ValueError("Native capture differs from the reviewed compaction plan")
        if file is None:
            file = stack.enter_context(open_dataset(path, writable=True))
        before = file.info()
        verify_snapshot(before, row)
        if file.sha256() != row["sha256"]:
            raise ValueError("Dataset bytes changed before compaction")
        verify_snapshot(file.info(), before)
        file.set_compressed()
        after = file.info()
        verify_snapshot(after, before)
        if file.sha256() != row["sha256"]:
            raise ValueError("Dataset logical bytes changed during compaction")
        verify_snapshot(file.info(), after)
        if not after["compressed"]:
            raise ValueError("Filesystem did not confirm transparent compression")
    return {"path": row["path"], "bytes": row["bytes"], "sha256": row["sha256"],
        "allocated_before": before["allocated_bytes"], "allocated_after": after["allocated_bytes"],
        "bytes_reclaimed": before["allocated_bytes"] - after["allocated_bytes"],
        "logical_bytes_unchanged": True}


def reviewed_plan(raw, digest):
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest) or hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("Apply requires the exact reviewed compaction plan SHA-256")
    report = json.loads(raw)
    if (report.get("schema_version") != 1 or report.get("mode") != "plan" or report.get("failures") != []
            or not report.get("finished_at_utc") or report.get("target_execution") is not False
            or report.get("deletes_files") is not False or not isinstance(report.get("files"), list)):
        raise ValueError("Compaction plan is incomplete or already applied")
    names = []
    for row in report["files"]:
        native_dataset(row["path"])
        if (type(row.get("bytes")) is not int or row["bytes"] < 0
                or type(row.get("write_time")) is not int or row["write_time"] < 0
                or not isinstance(row.get("file_id"), list) or len(row["file_id"]) != 3
                or any(type(value) is not int or value < 0 for value in row["file_id"])
                or not re.fullmatch(r"[0-9a-f]{64}", str(row.get("sha256", "")))
                or type(row.get("capture_output_identity_available")) is not bool
                or not isinstance(row.get("capture"), dict)):
            raise ValueError("Compaction plan dataset byte identity is incomplete")
        names.append(row["path"])
    if len(names) != len(set(names)):
        raise ValueError("Compaction plan duplicates a dataset")
    summary = {"files": len(names), "logical_bytes": sum(row["bytes"] for row in report["files"]),
               "failures": 0, "bytes_reclaimed": 0}
    if report.get("summary") != summary:
        raise ValueError("Compaction plan summary does not cover its complete dataset list")
    return report


def report_path(name):
    path = Path(name)
    if (path.parts[:2] != (".analysis", "results") or len(path.parts) != 3
            or path.as_posix() != name
            or not re.fullmatch(r"storage-compaction(?:-[a-z0-9]+)*\.json", path.name)):
        raise ValueError("Compaction receipt must use its reserved top-level result name")
    expected = a.ROOT.resolve(strict=True) / path
    actual = a.resolve(name, ".analysis/results")
    if a._resolved_path_identity(actual) != a._resolved_path_identity(expected):
        raise ValueError("Compaction receipt cannot follow filesystem links")
    current = a.ROOT.resolve(strict=True)
    for part in path.parts:
        current /= part
        if current.is_symlink() or (hasattr(current, "is_junction") and current.is_junction()):
            raise ValueError("Compaction receipt cannot follow filesystem links")
    return expected


def reserve_receipt(path, report):
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = (json.dumps(report, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    with path.open("xb") as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


def run(args):
    if os.name != "nt":
        raise ValueError("Transparent analysis compaction requires Windows NTFS")
    if type(args.workers) is not int or not 2 <= args.workers <= min(16, os.cpu_count() or 2):
        raise ValueError("Compaction workers exceed the available bounded CPU count")
    if type(args.minimum_mib) is not int or args.minimum_mib < 1:
        raise ValueError("Compaction minimum file size must be positive")
    reviews = getattr(args, "review_receipt", None) or []
    functions = getattr(args, "function_receipt", None) or []
    if args.apply and (reviews or functions):
        raise ValueError("Compaction apply takes its complete scope only from the unchanged reviewed plan")
    if len(reviews + functions) != len(set(reviews + functions)):
        raise ValueError("Compaction explicit receipt list contains duplicates")
    for name in reviews:
        target = native_dataset(name)
        if target.parent.name != "review-sources":
            raise ValueError("Explicit review compaction requires a SHA-named immutable native review")
    for name in functions:
        target = native_dataset(name)
        if (not native_function_parts(Path(a.rel(target)).parts)
                and not native_supplement_manifest_parts(Path(a.rel(target)).parts)):
            raise ValueError("Explicit function compaction requires a canonical indexed native function or supplement manifest")
    plan_path = report_path(args.plan)
    with a.index_lock(a.ROOT / ".analysis/storage-compaction.lock"), ExitStack() as ownership:
        if args.apply:
            plan_file_handle = ownership.enter_context(open_dataset(plan_path))
            plan_snapshot = plan_file_handle.info()
            raw = plan_file_handle.read_bytes()
            verify_snapshot(plan_file_handle.info(), plan_snapshot)
            report = reviewed_plan(raw, args.plan_sha256)
            jobs = report["files"]
            output = report_path(a.rel(plan_path.with_name(plan_path.stem + "-apply.json")))
            if output.exists():
                raise ValueError("Compaction apply receipt already exists; retain it and use a new reviewed plan")
            report = {**report, "mode": "apply", "plan": {"path": a.rel(plan_path), "bytes": len(raw),
                        "sha256": args.plan_sha256},
                      "applied_at_utc": a.stamp(), "apply_workers": args.workers, "results": [], "failures": []}
            report.pop("summary", None)
            report.pop("finished_at_utc", None)
            # Keep the exact reviewed plan available for independent verification.
            plan_path = output
            operation = apply_file
        else:
            if args.plan_sha256:
                raise ValueError("Plan identity is only accepted with --apply")
            if plan_path.exists() or plan_path.stem.endswith("-apply"):
                raise ValueError("Retain existing compaction receipts and use a new reserved plan name")
            jobs = []
            for directory in (a.ROOT / ".analysis/results/native").iterdir():
                if not directory.is_dir() or not re.fullmatch(r"[0-9a-f]{64}", directory.name):
                    continue
                for path in directory.iterdir():
                    if path.is_file() and path.name in DATASETS:
                        status = path.stat()
                        if (status.st_size >= args.minimum_mib * 1024 * 1024
                                and not getattr(status, "st_file_attributes", 0) & 0x800):
                            jobs.append(a.rel(path))
            jobs.extend(name for name in reviews if name not in jobs)
            jobs.extend(name for name in functions if name not in jobs)
            report = {"schema_version": 1, "mode": "plan", "recorded_at_utc": a.stamp(),
                "minimum_mib": args.minimum_mib, "workers": args.workers, "files": [], "failures": [],
                "scope": "NTFS transparent compression of retained root native datasets, explicitly selected SHA-named static reviews and indexed native functions from terminal analyzer sessions; logical bytes, paths and capture completion status retained",
                "target_execution": False, "deletes_files": False}
            operation = plan_file
        # Persist the full operation scope before any filesystem metadata change.
        # An interrupted apply keeps its plan identity and unfinished targets.
        reserve_receipt(plan_path, report)
        with ThreadPoolExecutor(max_workers=args.workers) as pool:
            futures = {pool.submit(operation, job): job for job in jobs}
            for future in as_completed(futures):
                job = futures[future]
                try:
                    row = future.result()
                    report["results" if args.apply else "files"].append(row)
                    print(json.dumps({"path": row["path"], "status": "compacted" if args.apply else "verified",
                        "logical_bytes": row["bytes"], "bytes_reclaimed": row.get("bytes_reclaimed")}), flush=True)
                except Exception as error:
                    report["failures"].append({"path": job["path"] if args.apply else job,
                        "error": str(error).replace(str(a.ROOT), "<project>")})
                a.save(plan_path, report)
        rows = report["results" if args.apply else "files"]
        report["summary"] = {"files": len(rows), "logical_bytes": sum(row["bytes"] for row in rows),
            "failures": len(report["failures"]), "bytes_reclaimed": sum(row.get("bytes_reclaimed", 0) for row in rows)}
        report["finished_at_utc"] = a.stamp()
        a.save(plan_path, report)
        print(json.dumps({"receipt": a.rel(plan_path), "plan_sha256": a.sha(plan_path), **report["summary"]}), flush=True)
        if report["failures"]:
            raise ValueError("Analysis compaction has failed items; inspect retained receipt: " + a.rel(plan_path))
        return report
