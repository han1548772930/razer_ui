"""Compact retained analysis files without changing their logical bytes or paths."""
from concurrent.futures import ThreadPoolExecutor, as_completed
from contextlib import contextmanager
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


def native_dataset(name):
    if not isinstance(name, str):
        raise ValueError("Compaction target must be a project-relative dataset path")
    parts = Path(name).parts
    native = (len(parts) == 5 and parts[:3] == (".analysis", "results", "native")
              and re.fullmatch(r"[0-9a-f]{64}", parts[3]) and parts[4] in DATASETS)
    review = (len(parts) == 5 and parts[:4] == (".analysis", "results", "reverse-pending", "review-sources")
              and re.fullmatch(r"[0-9a-f]{64}\.json", parts[4]))
    if (Path(name).is_absolute() or not (native or review) or Path(name).as_posix() != name):
        raise ValueError("Compaction target is outside the retained native dataset scope")
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


def expected_output(path):
    if path.parent.name == "review-sources":
        # Immutable review filenames bind the complete logical byte stream.
        expected = {"path": path.name, "bytes": path.stat().st_size, "sha256": path.stem}
        return expected, {"path": a.rel(path), "bytes": expected["bytes"], "sha256": path.stem,
                          "capture_complete": None, "scope": "immutable_native_static_review"}
    capture_path = path.parent / "capture.json"
    # An unfinished capture is still retained evidence. Compression changes no
    # logical bytes and must not promote its acquisition or semantic status.
    with open_dataset(capture_path) as file:
        info = file.info()
        raw = file.read_bytes()
        verify_snapshot(file.info(), info)
    capture = json.loads(raw)
    if capture.get("schema_version") != 1 or capture.get("input_sha256") != path.parent.name:
        raise ValueError("Compaction requires the native capture for this exact original")
    matches = [row for row in capture.get("outputs", []) if row.get("path") == path.name]
    if len(matches) > 1 or (capture.get("capture_complete") is True and len(matches) != 1):
        raise ValueError("Dataset is not uniquely declared by its native capture")
    expected = matches[0] if matches else None
    if expected is not None and (not re.fullmatch(r"[0-9a-f]{64}", str(expected.get("sha256", "")))
            or type(expected.get("bytes")) is not int or expected["bytes"] < 0):
        raise ValueError("Retained capture output byte identity is malformed")
    return expected, {"path": a.rel(capture_path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(),
                      "capture_complete": capture.get("capture_complete") is True}


def plan_file(name):
    path = native_dataset(name)
    expected, capture = expected_output(path)
    with open_dataset(path) as file:
        info = file.info()
        digest = file.sha256()
        verify_snapshot(file.info(), info)
    if expected is not None and (digest != expected["sha256"] or info["bytes"] != expected["bytes"]):
        raise ValueError("Retained dataset differs from its complete capture")
    return {"path": name, **info, "sha256": digest, "capture": capture,
            "capture_output_identity_available": expected is not None}


def apply_file(row):
    path = native_dataset(row["path"])
    expected, capture = expected_output(path)
    if (capture != row["capture"] or (expected is not None and
            (expected["sha256"] != row["sha256"] or expected["bytes"] != row["bytes"]))
            or (expected is not None) != row["capture_output_identity_available"]):
        raise ValueError("Native capture differs from the reviewed compaction plan")
    with open_dataset(path, writable=True) as file:
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
    if args.apply and reviews:
        raise ValueError("Compaction apply takes its complete scope only from the unchanged reviewed plan")
    if len(reviews) != len(set(reviews)):
        raise ValueError("Compaction review receipt list contains duplicates")
    for name in reviews:
        target = native_dataset(name)
        if target.parent.name != "review-sources":
            raise ValueError("Explicit review compaction requires a SHA-named immutable native review")
    plan_path = report_path(args.plan)
    with a.index_lock(a.ROOT / ".analysis/storage-compaction.lock"):
        if args.apply:
            raw = plan_path.read_bytes()
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
            report = {"schema_version": 1, "mode": "plan", "recorded_at_utc": a.stamp(),
                "minimum_mib": args.minimum_mib, "workers": args.workers, "files": [], "failures": [],
                "scope": "NTFS transparent compression of retained root native datasets and explicitly selected SHA-named static reviews; logical bytes, paths and capture completion status retained",
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
