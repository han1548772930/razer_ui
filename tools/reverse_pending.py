"""Resume retained native and frontend evidence in one bounded static queue."""
from __future__ import annotations

from collections import Counter
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor, as_completed, wait, FIRST_COMPLETED
from datetime import datetime, timezone
import base64
import ctypes
import gzip
import hashlib
import importlib.metadata
import json
import multiprocessing
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time
import tempfile
from types import SimpleNamespace
import urllib.parse

import analysis as a

REPORT = ".analysis/results/reverse-pending.json"
UNRESOLVED_REPORT = ".analysis/results/reverse-pending/unresolved-current.json"
DOC = "docs/re/native-bulk-reverse.md"
BEGIN = "<!-- reverse-pending:begin -->"
END = "<!-- reverse-pending:end -->"
INPUT_PREFIXES = (".analysis/inputs/official-native-current/",
                  ".analysis/inputs/host-current/", ".analysis/inputs/installed-current/")

_process_batch = None


def initialize_process_worker(root, ida_root, verify_only, native_only, resume, tool_sha256, recheck_mode=False):
    """Give static workers their own parser state and no publishing authority."""
    global _process_batch
    a.ROOT = Path(root)
    batch = Batch.__new__(Batch)
    batch.ida_root, batch.verify_only = ida_root, verify_only
    batch.native_only, batch.resume = native_only, resume
    batch.frontend_locks, batch.lock = {}, threading.Lock()
    batch.previous_native = {}
    batch.recheck_mode = recheck_mode
    batch.worker_tool_sha256 = tool_sha256
    _process_batch = batch


def execute_process_task(task, previous=None):
    if _process_batch is None:
        raise RuntimeError("Static worker was not initialized")
    if a.sha(Path(__file__)) != _process_batch.worker_tool_sha256:
        raise ValueError("Static queue worker source changed after initialization")
    _process_batch.previous_native = {task["sha256"]: previous} if previous else {}
    return {**_process_batch.execute(task), "worker_pid": os.getpid()}


def retained_task_artifact_valid(record, task, field=None):
    if valid(record):
        return True
    name = record.get("path", "")
    digest = task.get("sha256")
    if (field != "source_database" or re.fullmatch(r"[0-9a-f]{64}", str(digest)) is None
            or not name.startswith(".work/native/" + digest + "/")
            or Path(name).suffix.lower() not in {".i64", ".idb"}):
        return False
    try:
        path = a.resolve(name, ".work/native/" + digest)
        return path.is_file() and path.stat().st_size == record["bytes"] and a.sha(path) == record["sha256"]
    except (OSError, KeyError, ValueError, TypeError):
        return False


def check_retained_task(task):
    """Recheck every retained nested byte identity before resuming a task list."""
    def check_records(value, field=None):
        if isinstance(value, dict):
            if {"path", "bytes", "sha256"}.issubset(value) and not retained_task_artifact_valid(value, task, field):
                raise ValueError("Retained queue artifact changed: " + value["path"])
            for name, child in value.items():
                check_records(child, name)
        elif isinstance(value, list):
            for child in value:
                check_records(child)

    for key in ("resolution", "receipt"):
        if key in task:
            record = task[key]
            if not valid(record):
                raise ValueError("Retained completed task result changed")
            check_records(json.loads(a.resolve(record["path"], ".analysis/results").read_bytes()))
    return task["id"]


def inspect_retained_completion(task):
    try:
        check_retained_task(task)
        return {"task_id": task["id"], "verified": True}
    except (OSError, ValueError, KeyError, TypeError) as error:
        return {"task_id": task["id"], "verified": False, "error": str(error)}


def identity(path):
    raw = path.read_bytes()
    return {"path": a.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def retain_review_json(value=None, *, raw=None):
    """Atomically retain complete review bytes without replacing earlier receipts."""
    if raw is None:
        raw = (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    json.loads(raw)
    digest = hashlib.sha256(raw).hexdigest()
    destination = a.ROOT / ".analysis/results/reverse-pending/review-sources" / (digest + ".json")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(prefix=".review-", suffix=".tmp", dir=destination.parent, delete=False) as file:
            temporary = Path(file.name)
            file.write(raw)
            file.flush()
            os.fsync(file.fileno())
        try:
            os.link(temporary, destination)
        except FileExistsError:
            if destination.read_bytes() != raw:
                raise ValueError("Retained static review bytes changed")
        return {"path": a.rel(destination), "bytes": len(raw), "sha256": digest}
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def frozen_result_reference(record):
    """Locate an exact historical result after its mutable query owner advances."""
    if valid(record):
        return record
    retained = {**record, "path": ".analysis/results/reverse-pending/review-sources/" + record["sha256"] + ".json"}
    if not valid(retained):
        raise ValueError("Frozen task result bytes are unavailable: " + record["path"])
    return retained


def valid(row):
    try:
        path = a.resolve(row["path"], ".analysis")
        return path.is_file() and path.stat().st_size == row["bytes"] and a.sha(path) == row["sha256"]
    except (OSError, KeyError, ValueError, TypeError):
        return False


def read_retained_receipt(reference, *, materialize=True):
    """Verify physical gzip bytes and the complete uncompressed receipt."""
    if (re.fullmatch(r"[0-9a-f]{64}", str(reference.get("sha256"))) is None
            or type(reference.get("bytes")) is not int or reference["bytes"] < 0):
        raise ValueError("Retained queue receipt logical identity is malformed")
    stored = reference["artifact"]
    expected_path = ".analysis/results/reverse-pending/review-sources/" + reference["sha256"] + ".json.gz"
    if reference.get("encoding") != "gzip" or stored.get("path") != expected_path or not valid(stored):
        raise ValueError("Retained queue receipt physical identity differs")
    digest, count, chunks = hashlib.sha256(), 0, [] if materialize else None
    with gzip.open(a.resolve(stored["path"], ".analysis/results"), "rb") as stream:
        while block := stream.read(4 * 1024 * 1024):
            digest.update(block)
            count += len(block)
            if chunks is not None:
                chunks.append(block)
    if count != reference["bytes"] or digest.hexdigest() != reference["sha256"]:
        raise ValueError("Retained queue receipt logical identity differs")
    return b"".join(chunks) if chunks is not None else None


def retain_receipt_chunks(chunks, expected_sha256, expected_bytes=None):
    """Publish one lossless content-addressed predecessor without embedding it."""
    if re.fullmatch(r"[0-9a-f]{64}", str(expected_sha256)) is None:
        raise ValueError("Retained queue receipt needs a valid logical SHA-256")
    destination = a.ROOT / ".analysis/results/reverse-pending/review-sources" / (expected_sha256 + ".json.gz")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        digest, count = hashlib.sha256(), 0
        with tempfile.NamedTemporaryFile(prefix=".receipt-", suffix=".tmp", dir=destination.parent, delete=False) as file:
            temporary = Path(file.name)
            with gzip.GzipFile(filename="", mode="wb", fileobj=file, mtime=0, compresslevel=6) as compressed:
                for block in chunks:
                    digest.update(block)
                    count += len(block)
                    compressed.write(block)
        if digest.hexdigest() != expected_sha256 or expected_bytes is not None and count != expected_bytes:
            raise ValueError("Queue receipt source identity differs before retention")
        reference = {"sha256": expected_sha256, "bytes": count, "encoding": "gzip",
                     "artifact": {"path": a.rel(destination), "bytes": temporary.stat().st_size,
                                  "sha256": a.sha(temporary)}}
        with gzip.open(temporary, "rb") as stream:
            verified, verified_bytes = hashlib.sha256(), 0
            while block := stream.read(4 * 1024 * 1024):
                verified.update(block)
                verified_bytes += len(block)
        if verified.hexdigest() != expected_sha256 or verified_bytes != count:
            raise ValueError("Compressed queue receipt did not preserve complete source bytes")
        try:
            os.link(temporary, destination)
        except FileExistsError:
            reference["artifact"] = identity(destination)
        read_retained_receipt(reference, materialize=False)
        return reference
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def retain_receipt_bytes(raw):
    view = memoryview(raw)
    return retain_receipt_chunks((view[start:start + 4 * 1024 * 1024]
                                 for start in range(0, len(view), 4 * 1024 * 1024)),
                                hashlib.sha256(raw).hexdigest(), len(raw))


def externalize_report_history(report):
    """Retain every legacy history byte while removing recursive inline copies."""
    for name in ("previous_static_run", "interrupted_static_run"):
        record = report.get(name)
        if not isinstance(record, dict):
            continue
        if "raw_base64" in record:
            encoded = record["raw_base64"]
            if not isinstance(encoded, str) or len(encoded) % 4:
                raise ValueError("Legacy queue receipt history has invalid base64")
            chunks = (base64.b64decode(encoded[start:start + 4 * 1024 * 1024], validate=True)
                      for start in range(0, len(encoded), 4 * 1024 * 1024))
            reference = retain_receipt_chunks(chunks, record["sha256"])
            record["raw_receipt"] = reference
            del record["raw_base64"]
        elif "raw_receipt" in record:
            read_retained_receipt(record["raw_receipt"], materialize=False)
    return report


def classify_non_javascript_source_set(acquired, deployment):
    """Verify JSON installer metadata independently of JavaScript recovery."""
    result = {"kind": "non_javascript", "source_format": "installer_manifest_json",
              "javascript_recovery": "not_applicable", "verification": "failed",
              "originals": [], "declared_dependencies": [], "unavailable_probes": [], "failures": []}
    failures = result["failures"]
    rows = {row.get("url"): row for row in acquired.get("files", [])}
    entry = acquired.get("entry_url")
    probes = {row.get("url") for row in acquired.get("probes", []) if row.get("declared") is False}
    result["unavailable_probes"] = [row for row in acquired.get("files", [])
                                     if not row.get("path") and row.get("url") in probes]
    if deployment.get("entry_is_html") is not False or acquired.get("source_current_during_acquisition") is not True:
        failures.append({"kind": "non_javascript_entry_or_stability_unverified"})
    if deployment.get("failures"):
        failures.append({"kind": "non_javascript_deployment_failed", "failures": deployment["failures"]})
    entry_row = rows.get(entry, {})
    stable = any(row.get("url") == entry and row.get("unchanged") is True
                 and row.get("before_sha256") == entry_row.get("sha256")
                 and row.get("after_sha256") == entry_row.get("sha256")
                 for row in acquired.get("stability", []))
    if not stable:
        failures.append({"kind": "non_javascript_entry_stability_identity_unverified"})
    for row in rows.values():
        if row.get("path") and (not row["path"].startswith(".analysis/inputs/") or not valid(row)):
            failures.append({"kind": "non_javascript_original_identity_unverified", "url": row.get("url")})

    def document(url):
        row = rows.get(url, {})
        if not row.get("path") or not row["path"].startswith(".analysis/inputs/") or not valid(row):
            raise ValueError("Declared JSON original is unavailable: " + str(url))
        value = json.loads(a.resolve(row["path"], ".analysis/inputs").read_bytes())
        if not isinstance(value, dict):
            raise ValueError("Installer JSON original is not an object: " + str(url))
        result["originals"].append({"url": url, **{key: row[key] for key in ("path", "bytes", "sha256")}})
        return value

    retained_resources = None

    def resource_original(resource):
        nonlocal retained_resources
        url, size, digest = resource.get("url"), resource.get("size"), resource.get("sha256")
        if (not isinstance(url, str) or not url.startswith("https://") or type(size) is not int or size < 0
                or not isinstance(digest, str) or re.fullmatch(r"[0-9a-fA-F]{64}", digest) is None):
            raise ValueError("Declared installer resource lacks a valid URL, size or SHA-256")
        digest = digest.lower()
        candidates = [(rows[url], None)] if url in rows else []
        if retained_resources is None:
            retained_resources = []
            for name, field in (("host-current", "files"), ("official-native-current", "resources")):
                receipt = a.ROOT / ".analysis/inputs" / name / "acquisition.json"
                if receipt.is_file():
                    data = json.loads(receipt.read_bytes())
                    receipt_identity = identity(receipt)
                    retained_resources.extend((row, receipt_identity) for row in data.get(field, [])
                                              if row.get("path") and row.get("url", "").startswith("https://"))
        candidates.extend(retained_resources)
        match = next(((row, owner) for row, owner in candidates
                      if row.get("bytes") == size and str(row.get("sha256", "")).lower() == digest
                      and row.get("path", "").startswith(".analysis/inputs/") and valid(row)), None)
        if match is None:
            raise ValueError("Declared installer resource bytes are unavailable: " + url)
        row, owner = match
        result["declared_dependencies"].append({"declared_url": url, "expected_bytes": size,
            "expected_sha256": digest, "retained_original": {key: row[key] for key in ("path", "bytes", "sha256")},
            "acquired_url": row.get("url"), "source_acquisition": owner,
            "verification": "exact_manifest_size_and_sha256_match", "target_execution": False})

    try:
        root = document(entry)
        if (not isinstance(root.get("baseURL"), str) or not root["baseURL"]
                or not isinstance(root.get("latest"), dict) or not isinstance(root.get("archive"), list)):
            raise ValueError("JSON-only input has no verified installer-manifest schema")
        base = a.artifact_url("https://" + urllib.parse.urlsplit(entry).netloc + "/", root["baseURL"].rstrip("/") + "/")
        if base is None:
            raise ValueError("Installer manifest base URL is invalid")
        for release in [root["latest"], *root["archive"]]:
            if (not isinstance(release, dict) or not isinstance(release.get("url"), str)
                    or not isinstance(release.get("version"), str) or not release["version"]):
                raise ValueError("Installer manifest release identity is invalid")
            target = a.artifact_url(base, release["url"])
            manifest = document(target)
            resources = manifest.get("resources")
            if not isinstance(resources, list) or not resources:
                raise ValueError("Installer resource declarations are incomplete")
            result["declared_dependencies"].append({"declared_url": target,
                "declared_version": release["version"], "original": result["originals"][-1]})
            for resource in resources:
                if not isinstance(resource, dict) or resource.get("resourceVersion") != release["version"]:
                    raise ValueError("Installer resource version differs from its declared release")
                resource_original(resource)
        referenced = {row["declared_url"] for row in result["declared_dependencies"]}
        for edge in acquired.get("edges", []):
            if edge.get("from") in {row["url"] for row in result["originals"]} and edge.get("to") not in referenced:
                raise ValueError("Additional installer dependency requires verification: " + str(edge.get("to")))
        if acquired.get("unresolved"):
            failures.append({"kind": "non_javascript_acquisition_references_unresolved",
                             "references": acquired["unresolved"]})
    except (OSError, ValueError, KeyError, TypeError) as error:
        failures.append({"kind": "non_javascript_schema_or_dependency_failed", "error": str(error)})
    if not failures:
        result["verification"] = "passed"
    return result


def process_alive(pid):
    """Query process metadata only; never signal or attach a debugger."""
    if type(pid) is not int or pid <= 0:
        return None
    if os.name != "nt":
        try:
            os.kill(pid, 0)
            return True
        except ProcessLookupError:
            return False
        except PermissionError:
            return None
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    kernel.GetExitCodeProcess.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.OpenProcess(0x1000, False, pid)
    if not handle:
        return False if ctypes.get_last_error() == 87 else None
    try:
        code = wintypes.DWORD()
        return code.value == 259 if kernel.GetExitCodeProcess(handle, ctypes.byref(code)) else None
    finally:
        kernel.CloseHandle(handle)


def process_creation_time(pid):
    """Read the OS process creation identity, including protected-process metadata."""
    if type(pid) is not int or pid <= 0 or os.name != "nt":
        return None
    from ctypes import wintypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.GetProcessTimes.argtypes = [wintypes.HANDLE, *(ctypes.POINTER(wintypes.FILETIME),) * 4]
    kernel.GetProcessTimes.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.OpenProcess(0x1000, False, pid)
    if handle:
        try:
            times = [wintypes.FILETIME() for _ in range(4)]
            if kernel.GetProcessTimes(handle, *(ctypes.byref(value) for value in times)):
                return {"filetime_100ns": times[0].dwHighDateTime << 32 | times[0].dwLowDateTime,
                        "precision_100ns": 1, "source": "GetProcessTimes"}
        finally:
            kernel.CloseHandle(handle)
    script = ("$item = Get-CimInstance Win32_Process -Filter 'ProcessId = " + str(pid) +
              "' -ErrorAction Stop; if ($null -ne $item -and $null -ne $item.CreationDate) "
              "{ $item.CreationDate.ToUniversalTime().ToFileTimeUtc() }")
    try:
        completed = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script],
            capture_output=True, text=True, encoding="utf-8", timeout=15, check=False,
            creationflags=subprocess.CREATE_NO_WINDOW)
        value = completed.stdout.strip()
        if completed.returncode == 0 and re.fullmatch(r"[0-9]{15,19}", value):
            return {"filetime_100ns": int(value), "precision_100ns": 10, "source": "Win32_Process.CreationDate"}
    except (OSError, subprocess.TimeoutExpired):
        pass
    return None


def coordinator_instance_absence(session):
    """Distinguish an exited coordinator from another process reusing its PID."""
    pid = session.get("pid")
    state = process_alive(pid)
    if state is False:
        return {"confirmed_absent": True, "reason": "pid_not_running", "pid": pid}
    observed = process_creation_time(pid)
    result = {"confirmed_absent": False, "reason": "process_identity_unverified", "pid": pid,
              "observed_creation": observed, "process_state": state}
    if observed is None:
        return result
    retained = session.get("process_creation")
    if isinstance(retained, dict) and type(retained.get("filetime_100ns")) is int:
        left, right = retained["filetime_100ns"], observed["filetime_100ns"]
        precision = max(retained.get("precision_100ns", 1), observed.get("precision_100ns", 1))
        if left // precision != right // precision:
            result.update(confirmed_absent=True, reason="pid_reused_different_os_creation", retained_creation=retained)
        else:
            result["reason"] = "same_os_process_instance_still_present"
        return result
    try:
        started = datetime.fromisoformat(session["started_at_utc"])
        if started.tzinfo is None:
            return result
        delta = started.astimezone(timezone.utc) - datetime(1601, 1, 1, tzinfo=timezone.utc)
        boundary = (delta.days * 86400 + delta.seconds) * 10_000_000 + delta.microseconds * 10
        if observed["filetime_100ns"] >= boundary + observed.get("precision_100ns", 1):
            result.update(confirmed_absent=True, reason="pid_reused_creation_after_recorded_queue_start",
                          retained_queue_started_at_utc=session["started_at_utc"])
    except (KeyError, TypeError, ValueError):
        pass
    return result


def task_id(kind, inputs, options):
    return hashlib.sha256(json.dumps({"kind": kind, "inputs": inputs, "options": options},
                                    sort_keys=True).encode()).hexdigest()[:24]


def active_native_sessions(directory):
    rows = []
    paths = [directory / "batch-session.json", *directory.glob("supplements/*/session.json")]
    for path in paths:
        if not path.is_file():
            continue
        data = json.loads(path.read_bytes())
        if not data.get("finished_at_utc"):
            alive = process_alive(data.get("pid"))
            if alive is not False:
                rows.append({"session": identity(path), "pid": data.get("pid"),
                             "process_state": "running" if alive else "unknown"})
    return rows


def native_partition_specifications(partitions):
    """Serialize the reviewed local helper ranges through one CLI owner."""
    try:
        if not isinstance(partitions, list):
            raise ValueError("Native function partitions must be a list")
        specifications = []
        for row in partitions:
            owner, ranges = row["owner_rva"], row["helper_ranges"]
            if not isinstance(owner, str) or not isinstance(ranges, list) or not ranges:
                raise ValueError("Native function partitions require an owner and explicit helper ranges")
            values = []
            for helper in ranges:
                start, stop = helper["start_rva"], helper["end_rva_exclusive"]
                if not isinstance(start, str) or not isinstance(stop, str):
                    raise ValueError("Native partition helper boundaries must be explicit RVA strings")
                values.append(start + ":" + stop)
            specifications.append(owner + "=" + ",".join(values))
        return specifications
    except (KeyError, TypeError) as error:
        raise ValueError("Native function partition schema is incomplete") from error


def native_shared_tail_specifications(rows):
    """Validate raw reviewed shared-tail fields before serializing the CLI."""
    if not isinstance(rows, list):
        raise ValueError("Native shared function tails must be a list")
    required = {"primary_owner_rva", "tail_start_rva", "tail_end_rva", "owner_rvas",
                "inline_owners", "assembly_evidence"}
    for row in rows:
        if (not isinstance(row, dict) or set(row) != required
                or any(not isinstance(row[key], str) or not row[key].strip()
                       for key in ("primary_owner_rva", "tail_start_rva", "tail_end_rva", "assembly_evidence"))
                or type(row["inline_owners"]) is not bool
                or not isinstance(row["owner_rvas"], list) or len(row["owner_rvas"]) < 2
                or any(not isinstance(value, str) or not value.strip() for value in row["owner_rvas"])):
            raise ValueError("Native shared function tail schema is incomplete")
    return a.native_shared_tail_specifications(rows)


def load_native_reviews(specs):
    """Load explicit reviewed plans, rechecking every retained evidence identity."""
    plans = {}
    for spec in specs:
        name, marker, pointer = spec.partition("#")
        if Path(name).is_absolute():
            raise ValueError("Native review paths must be project-relative")
        path = a.resolve(name, ".analysis/results")
        raw = path.read_bytes()
        reference = {"path": a.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        retained = a.ROOT / ".analysis/results/reverse-pending/review-sources" / (reference["sha256"] + ".json")
        with a.index_lock(a.ROOT / ".work/reverse-pending/locks" / ("review-source-" + reference["sha256"] + ".lock")):
            if retained.is_file():
                if retained.read_bytes() != raw:
                    raise ValueError("Retained native review source bytes changed")
            else:
                retained.parent.mkdir(parents=True, exist_ok=True)
                retained.write_bytes(raw)
        value = json.loads(raw)
        if marker and pointer:
            if not pointer.startswith("/"):
                raise ValueError("Native review requires an RFC 6901 JSON pointer")
            for token in pointer[1:].split("/"):
                token = token.replace("~1", "/").replace("~0", "~")
                try:
                    value = value[int(token)] if isinstance(value, list) else value[token]
                except (KeyError, IndexError, TypeError, ValueError) as error:
                    raise ValueError("Native review JSON pointer does not resolve") from error
        for plan_index, plan in enumerate(value if isinstance(value, list) else [value]):
            if plan.get("schema_version") != 1:
                raise ValueError("Unsupported native review plan schema")
            for record in [plan["source_input"], plan["source_capture"], *plan["evidence"]]:
                if not valid(record):
                    raise ValueError("Native review evidence identity differs: " + record["path"])
            if not plan["evidence"]:
                raise ValueError("Native review must retain complete assembly evidence")
            original = plan["source_input"]
            if (not original["path"].startswith(INPUT_PREFIXES)
                    or "$PLUGINSDIR/" in original["path"]
                    or Path(original["path"]).suffix.lower() not in {".dll", ".node", ".sys"}):
                raise ValueError("Native review input is outside the native product scope")
            capture = json.loads(a.resolve(plan["source_capture"]["path"], ".analysis/results/native").read_bytes())
            if capture["input_sha256"] != original["sha256"] or not capture.get("capture_complete"):
                raise ValueError("Native review lacks its complete matching base capture")
            selected = [int(rva, 0) for rva in plan["function_rvas"]]
            if not selected or len(selected) != len(set(selected)):
                raise ValueError("Native review requires unique explicit function RVAs")
            types = plan["type_recoveries"]
            imports = plan.get("import_type_recoveries", [])
            merges = plan.get("function_merges", [])
            outlines = plan.get("function_outlines", [])
            partitions = plan.get("function_partitions", [])
            partition_specs = native_partition_specifications(partitions)
            shared_tails = plan.get("function_shared_tails", [])
            shared_specs = native_shared_tail_specifications(shared_tails)
            fresh_inlines = plan.get("function_fresh_inlines", [])
            if (not isinstance(fresh_inlines, list) or len(fresh_inlines) > 256
                    or any(not isinstance(row, dict) or not isinstance(row.get("rva"), str)
                           or not row.get("assembly_evidence", "").strip() for row in fresh_inlines)
                    or len({int(row["rva"], 0) for row in fresh_inlines}) != len(fresh_inlines)):
                raise ValueError("Native fresh inline review requires unique helpers and explicit evidence")
            for recoveries in (types, imports):
                if len({int(row["rva"], 0) for row in recoveries}) != len(recoveries):
                    raise ValueError("Native review repeats a recovered type")
                if any(not row.get("declaration", "").strip() or not row.get("assembly_evidence", "").strip() for row in recoveries):
                    raise ValueError("Native review types require declarations and assembly evidence")
            if any(not row.get("assembly_evidence", "").strip() for row in [*merges, *outlines, *partitions, *shared_tails]):
                raise ValueError("Native function boundary repairs require complete assembly evidence")
            note = "\n\n".join(dict.fromkeys(row["assembly_evidence"] for row in [*types, *imports, *merges, *outlines, *partitions, *shared_tails, *fresh_inlines]))
            merge_options = {"merge_specs": [row["owner_rva"] + "=" + ",".join(row["member_rvas"]) for row in merges]} if merges else {}
            if outlines:
                merge_options["outline_rvas"] = [row["rva"] for row in outlines]
            if partitions:
                merge_options["partition_specs"] = partition_specs
            if shared_tails:
                merge_options["shared_tail_specs"] = shared_specs
            if fresh_inlines:
                merge_options["fresh_inline_rvas"] = [row["rva"] for row in fresh_inlines]
            if "reanalyze_type_owners" in plan:
                merge_options["reanalyze_type_owners"] = plan["reanalyze_type_owners"]
            if "propagate_direct_calls" in plan:
                merge_options["propagate_direct_calls"] = plan["propagate_direct_calls"]
            observation_options = {}
            if "static_observation" in plan:
                plan_pointer = pointer + ("/" + str(plan_index) if isinstance(value, list) else "")
                observation_spec = a.rel(retained) + "#" + plan_pointer + "/static_observation"
                observation = a.native_observation_evidence(observation_spec)
                if observation["observer_source"] not in plan["evidence"]:
                    raise ValueError("Static observation source must also be retained as plan evidence")
                observation_options["observation_evidence"] = observation_spec
            a.native_function_selection(a.resolve(original["path"], ".analysis/inputs").read_bytes(), capture,
                selected, [row["rva"] + "=" + row["declaration"] for row in types], note or None, 1024,
                [row["rva"] + "=" + row["declaration"] for row in imports], **merge_options, **observation_options)
            digest = original["sha256"]
            if digest in plans:
                raise ValueError("Native review repeats an input SHA; merge its reviewed plan explicitly")
            plans[digest] = {**plan, "review_reference": {**reference, "pointer": pointer},
                "type_evidence": note + "\n\nReviewed plan receipt: " + name + "#" + pointer + "; SHA-256 " + reference["sha256"],
                **({"observation_evidence_spec": observation_options["observation_evidence"]} if observation_options else {})}
    return plans


class Batch:
    def __init__(self, workers, ida_root, verify_only=False, native_only=False, review_paths=(), resume=False,
                 process_workers=False, resume_reason=None):
        if type(workers) is not int or not 2 <= workers <= min(32, os.cpu_count() or 2):
            raise ValueError("Workers must be an integer from two to the available CPU limit (maximum 32)")
        if resume and (verify_only or native_only or review_paths):
            raise ValueError("Resume inherits the retained scope and reviews; do not combine it with scope options")
        if resume_reason is not None and (not resume or not isinstance(resume_reason, str) or not resume_reason.strip()):
            raise ValueError("A nonempty resume reason requires resume mode")
        self.workers, self.ida_root = workers, ida_root
        if type(process_workers) is not bool:
            raise ValueError("Process-worker mode must be a boolean")
        self.process_workers = process_workers
        self.recheck_mode = False
        self.resume = resume
        self.resume_reason = resume_reason
        self.verify_only = verify_only
        self.native_only, self.review_paths = native_only, tuple(review_paths)
        self.reviews = load_native_reviews(self.review_paths)
        self.previous_native = {}
        self.tool = identity(Path(__file__))
        self.frontend_locks, self.lock = {}, threading.Lock()
        self.report = {"schema_version": 1, "started_at_utc": a.stamp(), "workers": workers,
                       "tool": self.tool, "tasks": [], "excluded": [], "target_execution": False,
                       "rust_implementation_changed": False, "semantics_complete": False,
                       "official_dll_comparison": "not_executed", "real_device_acceptance": "not_executed"}
        self.report["verification_only"] = verify_only
        self.report["native_only"] = native_only
        self.report["native_reviews"] = [row["review_reference"] for row in self.reviews.values()]
        self.last_document = 0

    def process_pool(self):
        return ProcessPoolExecutor(max_workers=self.workers, mp_context=multiprocessing.get_context("spawn"),
            initializer=initialize_process_worker,
            initargs=(str(a.ROOT), self.ida_root, self.verify_only, self.native_only, self.resume, self.tool["sha256"], self.recheck_mode),
            max_tasks_per_child=8)

    def resume_tasks(self):
        """Continue the frozen queue under the caller's exclusive queue lock."""
        previous_raw = (a.ROOT / REPORT).read_bytes()
        previous = json.loads(previous_raw)
        if previous.get("schema_version") != 1 or previous.get("finished_at_utc"):
            raise ValueError("Resume requires a supported unfinished static queue")
        session = previous.get("queue_session")
        coordinator_guard = coordinator_instance_absence(session) if session else None
        if session and not coordinator_guard["confirmed_absent"]:
            raise ValueError("Previous queue process is still running or cannot be verified absent")
        workflow_tool = previous.get("task_identity_tool", previous["tool"])
        snapshot = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (workflow_tool["sha256"] + ".py")
        if not snapshot.is_file() or a.sha(snapshot) != workflow_tool["sha256"]:
            raise ValueError("Retained queue workflow source changed or is unavailable")
        references = previous.get("native_reviews", [])
        for reference in references:
            if not valid(reference):
                raise ValueError("Retained native review identity changed: " + reference["path"])
        specs = [row["path"] + "#" + row.get("pointer", "") for row in references]
        reviews = load_native_reviews(specs)

        tasks, identifiers, scheduled, completed = previous["tasks"], set(), [], []
        for task in tasks:
            keys = ("kind", "input", "sha256", "base_capture", "review") if task["kind"] == "native" else ("kind", "input", "acquisition")
            inputs = {key: task[key] for key in keys if key in task}
            expected = task_id(task["kind"], inputs, {"workflow": workflow_tool["sha256"],
                "ida_root": self.ida_root, "max_function_kib": 1024})
            if task["id"] != expected or task["id"] in identifiers:
                raise ValueError("Retained queue task identity or IDA root differs")
            identifiers.add(task["id"])
            if task["kind"] == "native":
                source = a.resolve(task["input"], ".analysis/inputs")
                if a.sha(source) != task["sha256"] or (task.get("base_capture") and not valid(task["base_capture"])):
                    raise ValueError("Retained native original or base capture changed")
                if task.get("review") != reviews.get(task["sha256"]):
                    raise ValueError("Retained native task differs from its exact reviewed plan")
            elif task["kind"] == "frontend":
                if not valid(task["acquisition"]):
                    raise ValueError("Retained frontend acquisition changed")
            else:
                raise ValueError("Unknown retained queue task kind")
            if task["status"] in {"pending", "waiting_external"}:
                if task.get("finished_at_utc"):
                    raise ValueError("Retained pending task has a contradictory completion timestamp")
                scheduled.append(task)
            elif task["status"] in {"passed", "unresolved", "failed"} and task.get("finished_at_utc"):
                completed.append(task)
            else:
                raise ValueError("Retained queue task has an unsupported status or missing completion timestamp")
        if self.process_workers and completed:
            with self.process_pool() as pool:
                completion_checks = list(pool.map(inspect_retained_completion, completed))
        else:
            completion_checks = [inspect_retained_completion(task) for task in completed]
        completion_by_id = {task["id"]: task for task in completed}
        invalidated = []
        for checked in completion_checks:
            if checked["verified"]:
                continue
            task = completion_by_id[checked["task_id"]]
            invalidated.append({**checked, "previous_completion": dict(task),
                "action": "only_this_task_requeued_for_strict_verification", "target_execution": False})
            frozen_keys = {"id", "kind", "input", "sha256", "base_capture", "review", "acquisition"}
            frozen = {key: value for key, value in task.items() if key in frozen_keys}
            task.clear()
            task.update(frozen, status="pending")
            scheduled.append(task)
        retained_source = self.report["tool_source"]
        retained_receipt = retain_receipt_bytes(previous_raw)
        self.report = externalize_report_history(previous)
        if invalidated:
            self.report.setdefault("resume_revalidation_failures", []).extend(invalidated)
        self.report["interrupted_static_run"] = {
            "sha256": hashlib.sha256(previous_raw).hexdigest(),
            "raw_receipt": retained_receipt,
            "observed_at_utc": a.stamp(), "exit_code": None,
            "coordinator_process_confirmed_absent": True if session else None,
            "coordinator_instance_verification": coordinator_guard,
            "legacy_coordinator_guard": None if session else "exclusive_queue_lock; no legacy coordinator PID was recorded",
            "completed_task_ids": [task["id"] for task in tasks if task not in scheduled],
            "scheduled_task_ids": [task["id"] for task in scheduled]}
        if self.resume_reason is not None:
            self.report["interrupted_static_run"]["reason"] = self.resume_reason
        self.report.update({"tool": self.tool, "tool_source": retained_source,
                            "task_identity_tool": workflow_tool, "workers": self.workers})
        self.verify_only = previous.get("verification_only", False)
        self.native_only = previous.get("native_only", False)
        self.reviews, self.review_paths = reviews, tuple(specs)
        self.previous_native = {task["sha256"]: task for task in tasks if task["kind"] == "native"}
        return scheduled

    def completed_review_attempt(self, source, capture, review):
        """Adopt an exact ended supplement whose coordinator result was not published."""
        selection = a.native_function_selection(source.read_bytes(), capture,
            [int(rva, 0) for rva in review["function_rvas"]],
            [row["rva"] + "=" + row["declaration"] for row in review["type_recoveries"]],
            review["type_evidence"] or None, 1024,
            [row["rva"] + "=" + row["declaration"] for row in review.get("import_type_recoveries", [])],
            [row["owner_rva"] + "=" + ",".join(row["member_rvas"]) for row in review.get("function_merges", [])],
            [row["rva"] for row in review.get("function_outlines", [])],
            **({"partition_specs": native_partition_specifications(review["function_partitions"])}
               if review.get("function_partitions") else {}),
            **({"shared_tail_specs": native_shared_tail_specifications(review["function_shared_tails"])}
               if review.get("function_shared_tails") else {}),
            **({"fresh_inline_rvas": [row["rva"] for row in review["function_fresh_inlines"]]}
               if review.get("function_fresh_inlines") else {}),
            observation_evidence=review.get("observation_evidence_spec"),
            reanalyze_type_owners=review.get("reanalyze_type_owners", False),
            propagate_direct_calls=review.get("propagate_direct_calls", False))
        selection_id = hashlib.sha256(json.dumps(selection, sort_keys=True).encode()).hexdigest()[:16]
        output = a.ROOT / ".analysis/results/native" / capture["input_sha256"] / "supplements" / selection_id
        session_path, plan_path = output / "session.json", output / "plan.json"
        if not session_path.is_file():
            return None
        session = json.loads(session_path.read_bytes())
        if not session.get("finished_at_utc"):
            return None
        plan = json.loads(plan_path.read_bytes())
        if (type(session.get("exit_code")) is not int or plan.get("function_selection") != selection
                or plan.get("input") != a.rel(source) or plan.get("sha256") != capture["input_sha256"]
                or plan.get("base_capture") != {"path": review["source_capture"]["path"], "sha256": review["source_capture"]["sha256"]}
                or session.get("database") != plan.get("database")):
            raise ValueError("Completed supplemental attempt differs from its exact retained plan")
        database = plan["source_database"]
        if a.sha(a.resolve(database["path"], ".work/native")) != database["sha256"]:
            raise ValueError("Retained source database changed before completed-attempt adoption")
        record = {"session": identity(session_path), "plan": identity(plan_path), "exit_code": session["exit_code"]}
        capture_path = output / "capture.json"
        if capture_path.is_file():
            record["capture"] = identity(capture_path)
        return record

    def discover(self):
        tasks, native = [], {}
        for path in sorted((a.ROOT / ".analysis/results/native").glob("*/capture.json")):
            data = json.loads(path.read_bytes())
            source = data.get("input", "")
            if (not source.startswith(INPUT_PREFIXES) or "$PLUGINSDIR/" in source
                    or Path(source).suffix.lower() not in {".dll", ".node", ".sys"}):
                continue
            if data.get("failures") or not data.get("capture_complete"):
                if self.native_only and data.get("capture_complete"):
                    retained = a.ROOT / ".analysis/results/reverse-pending/verified-native" / (data["input_sha256"] + ".json")
                    if retained.is_file():
                        verified = json.loads(retained.read_bytes())
                        if (verified.get("verified_base_capture") == identity(path)
                                and not verified.get("remaining_failures") and not verified.get("verification_failures")
                                and data["input_sha256"] not in self.reviews):
                            continue
                native[data["input_sha256"]] = {"kind": "native", "input": source,
                    "sha256": data["input_sha256"], "base_capture": identity(path)}
        corpus = a.ROOT / ".analysis/results/corpus.json"
        if corpus.is_file():
            inventory = json.loads(corpus.read_bytes())
            for row in inventory.get("native_inputs", []) + inventory.get("mixed_inputs", []):
                aliases = [name for name in row.get("verified_aliases", row.get("aliases", []))
                           if name.startswith(INPUT_PREFIXES) and "$PLUGINSDIR/" not in name
                           and Path(name).suffix.lower() in {".dll", ".node", ".sys"}]
                capture = a.ROOT / ".analysis/results/native" / row["sha256"] / "capture.json"
                if aliases and not capture.is_file():
                    native[row["sha256"]] = {"kind": "native", "input": sorted(aliases)[0],
                                              "sha256": row["sha256"], "base_capture": None}
        tasks.extend(native[key] for key in sorted(native))
        for path in ([] if self.native_only else sorted((a.ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json"))):
            tasks.append({"kind": "frontend", "input": a.rel(path.parent), "acquisition": identity(path)})
        for task in tasks:
            if task.get("sha256") in self.reviews:
                task["review"] = self.reviews[task["sha256"]]
            task["id"] = task_id(task["kind"], task, {"workflow": self.tool["sha256"],
                                                       "ida_root": self.ida_root, "max_function_kib": 1024})
            task["status"] = "pending"
        tasks.sort(key=lambda task: (0 if "review" in task else 1, task["id"]))
        self.report["tasks"] = tasks
        self.publish()
        return tasks

    def publish(self):
        counts = Counter(row["status"] for row in self.report["tasks"])
        kinds = Counter(row["kind"] for row in self.report["tasks"])
        self.report["summary"] = {"tasks": len(self.report["tasks"]), "by_status": dict(counts),
                                  "by_kind": dict(kinds), "queue_finished": bool(self.report.get("finished_at_utc")),
                                  "semantics_complete": False}
        if "native_scope" in self.report:
            for task in self.report["tasks"]:
                if task["kind"] == "native" and "remaining" in task:
                    self.report["native_scope"][task["sha256"]] = {
                        "resolved": task["resolved"], "remaining": task["remaining"],
                        "verification_errors": task["verification_errors"]}
            self.report["summary"]["native_functions"] = {
                "resolved": sum(row["resolved"] for row in self.report["native_scope"].values()),
                "remaining": sum(row["remaining"] for row in self.report["native_scope"].values())}
        a.save(a.ROOT / REPORT, self.report)
        if time.monotonic() - self.last_document >= 30 or self.report.get("finished_at_utc"):
            self.document()
            self.last_document = time.monotonic()

    def execute(self, task):
        with a.index_lock(a.ROOT / ".work/reverse-pending/locks" / (task["id"] + ".lock")):
            try:
                if self.recheck_mode and task["kind"] == "frontend":
                    import frontend
                    review = frontend.recheck_frozen_frontend(task)
                    if review.get("status") not in {"passed", "unresolved"} or review.get("semantics_complete") is not False:
                        raise ValueError("Frontend recheck must preserve its explicit static scope")
                    result = {"status": review["status"], "receipt": retain_review_json(review),
                              "semantic_review": "required"}
                else:
                    result = self.native(task) if task["kind"] == "native" else self.frontend(task)
            except Exception as error:
                result = {"status": "failed", "error": str(error).replace(str(a.ROOT), "<project>")}
            return result

    def command(self, task, arguments):
        log = a.ROOT / ".work/reverse-pending" / (task["id"] + ".log")
        log.parent.mkdir(parents=True, exist_ok=True)
        command = [sys.executable, "-X", "utf8", str(a.ROOT / "tools/workspace.py"), *arguments]
        options = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}
        with log.open("ab") as stream:
            stream.write((json.dumps({"argv": command[3:]}, ensure_ascii=False) + "\n").encode("utf-8"))
            process = subprocess.Popen(command, cwd=a.ROOT, stdout=stream, stderr=subprocess.STDOUT, **options)
            exit_code = process.wait()
        return {"exit_code": exit_code, "log": identity(log)}

    def native(self, task):
        source = a.resolve(task["input"], ".analysis/inputs")
        original = identity(source)
        if original["sha256"] != task["sha256"]:
            raise ValueError("Native original identity changed")

        def unchanged():
            if identity(source) != original:
                raise ValueError("Native original identity changed during analysis")

        directory = a.ROOT / ".analysis/results/native" / task["sha256"]
        with a.index_lock(a.ROOT / ".work/reverse-pending/locks" / (task["sha256"] + ".lock")):
            active = active_native_sessions(directory)
            if active:
                return {"status": "waiting_external", "external_sessions": active}
            capture_path = directory / "capture.json"
            commands = []
            previous = self.previous_native.get(task["sha256"])
            if (self.native_only and "review" not in task and previous and previous.get("status") == "unresolved"
                    and previous.get("resolution") and valid(previous["resolution"]) and capture_path.is_file()):
                retained = json.loads(a.resolve(previous["resolution"]["path"], ".analysis/results").read_bytes())
                if (retained.get("input") == original and retained.get("verified_base_capture") == identity(capture_path)):
                    unchanged()
                    return {"status": "unresolved", "resolution": previous["resolution"], "commands": [],
                        "resolved": len(retained["resolved_failures"]), "remaining": len(retained["remaining_failures"]),
                        "verification_errors": len(retained["verification_failures"]),
                        "verification_reused": True, "unchanged_retry_skipped": True,
                        "verification_scope": "Retained unresolved result reused after original/base/result identity checks; no new function-artifact audit"}
            if not capture_path.is_file() or not json.loads(capture_path.read_bytes()).get("capture_complete"):
                if self.verify_only:
                    return {"status": "unresolved", "commands": [], "error": "No complete native capture to verify"}
                arguments = ["capture-native", "--input", task["input"], "--ida-root", self.ida_root, "--no-refresh-gaps"]
                if (directory / "plan.json").is_file():
                    arguments.append("--resume")
                commands.append(self.command(task, arguments))
                unchanged()
                if not capture_path.is_file():
                    return {"status": "failed", "commands": commands, "error": "No native capture was produced"}
            capture = json.loads(capture_path.read_bytes())
            if not capture.get("capture_complete"):
                return {"status": "unresolved", "commands": commands, "capture": identity(capture_path),
                        "remaining": capture.get("failures", []), "error": "Full native capture remains incomplete"}
            capture, capture_identity = a.verify_native_capture(directory, original)
            unchanged()
            resolution = a.native_reanalysis_resolution(directory, capture)
            boundaries = {row["rva"] for row in capture["functions"]}
            rvas = sorted({hex(int(row["stage"].split(":")[0], 16)) for row in resolution["remaining_failures"]
                           if row.get("stage", "").endswith(":hexrays")}, key=lambda rva: int(rva, 16))
            if not set(rvas).issubset(boundaries):
                raise ValueError("Failed function has no exact retained function boundary")
            review = task.get("review")
            if review:
                if (not valid(review["review_reference"]) or review["source_input"] != original
                        or review["source_capture"] != capture_identity):
                    raise ValueError("Reviewed plan, input or base capture changed before analysis")
            selected = review["function_rvas"] if review else rvas
            completed_attempt = self.completed_review_attempt(source, capture, review) if self.resume and review else None
            if selected and not self.verify_only and (review or not self.native_only) and not completed_attempt:
                arguments = ["supplement-native", "--input", task["input"], "--ida-root", self.ida_root,
                             "--no-refresh-gaps", "--recover-interrupted", "--max-function-kib", "1024"]
                for rva in selected:
                    arguments.extend(["--function-rva", rva])
                if review:
                    if any(not valid(row) for row in [review["source_input"], review["source_capture"], *review["evidence"]]):
                        raise ValueError("Reviewed evidence changed before IDA invocation")
                    for row in review["type_recoveries"]:
                        arguments.extend(["--function-type", row["rva"] + "=" + row["declaration"]])
                    for row in review.get("import_type_recoveries", []):
                        arguments.extend(["--import-type", row["rva"] + "=" + row["declaration"]])
                    if review.get("reanalyze_type_owners"):
                        arguments.append("--reanalyze-type-owners")
                    if review.get("propagate_direct_calls"):
                        arguments.append("--propagate-direct-calls")
                    for row in review.get("function_merges", []):
                        arguments.extend(["--merge-functions", row["owner_rva"] + "=" + ",".join(row["member_rvas"])])
                    for row in review.get("function_outlines", []):
                        arguments.extend(["--outline-function-rva", row["rva"]])
                    for specification in native_partition_specifications(review.get("function_partitions", [])):
                        arguments.extend(["--partition-functions", specification])
                    for specification in native_shared_tail_specifications(review.get("function_shared_tails", [])):
                        arguments.extend(["--share-function-tail", specification])
                    for row in review.get("function_fresh_inlines", []):
                        arguments.extend(["--fresh-inline-function-rva", row["rva"]])
                    if review["type_evidence"]:
                        arguments.extend(["--type-evidence", review["type_evidence"]])
                    if review.get("observation_evidence_spec"):
                        arguments.extend(["--observation-evidence", review["observation_evidence_spec"]])
                commands.append(self.command(task, arguments))
                unchanged()
                resolution = a.native_reanalysis_resolution(directory)
            unchanged()
            if identity(capture_path) != capture_identity:
                raise ValueError("Native base capture changed during analysis")
            folder = "verified-native" if self.verify_only else "reviewed-native" if self.native_only else "native"
            receipt = a.ROOT / ".analysis/results/reverse-pending" / folder / (task["sha256"] + ".json")
            compact = {key: value for key, value in resolution.items() if key != "complete_functions"}
            compact.update({"input": original, "verified_base_capture": capture_identity,
                            "original_identity_rechecked": True, "verification_only": self.verify_only,
                            "review": review, "unchanged_retry_skipped": bool(self.native_only and not review),
                            "target_execution": False, "semantics_complete": False})
            if completed_attempt:
                compact["completed_attempt_reused"] = completed_attempt
            if self.recheck_mode:
                result_reference = retain_review_json(compact)
            else:
                a.save(receipt, compact)
                result_reference = identity(receipt)
            return {"status": "unresolved" if resolution["remaining_failures"] or resolution["verification_failures"] else "passed",
                    "resolution": result_reference, "commands": commands,
                    "selected_function_rvas": selected, "original_failed_function_rvas": rvas,
                    "resolved": len(resolution["resolved_failures"]),
                    "remaining": len(resolution["remaining_failures"]), "verification_errors": len(resolution["verification_failures"]),
                    **({"completed_attempt_reused": completed_attempt} if completed_attempt else {})}

    def frontend(self, task):
        import evidence
        import frontend
        import families
        path = a.resolve(task["input"], ".analysis/inputs") / "acquisition.json"
        acquired = json.loads(path.read_bytes())
        relevant = {".js", ".mjs", ".css", ".html"}
        missing = [row for row in acquired.get("files", [])
                   if Path(urllib.parse.urlsplit(row.get("url", "")).path).suffix.lower() in relevant
                   and ("error" in row or ("path" in row and not families.valid(row)))]
        repairs = []
        if not self.verify_only and (missing or acquired.get("source_current_during_acquisition") is not True):
            try:
                a.fetch_frontend(SimpleNamespace(url=acquired["entry_url"], output=task["input"],
                                                repair=True, no_refresh_gaps=True), refresh_ledger=False)
            except Exception as error:
                repairs.append({"error": str(error).replace(str(a.ROOT), "<project>")})
            acquired = json.loads(path.read_bytes())
        recovery = frontend.recover_source_set(path, a.wakaru())
        deployment = evidence.frontend_deployment_sources(acquired)
        classification = None
        if recovery["status"] == "no_javascript_originals":
            classification = classify_non_javascript_source_set(acquired, deployment)
            if classification["verification"] == "passed":
                recovery = {**recovery, "original_status": recovery["status"], "status": "not_applicable",
                            "source_kind": "non_javascript"}
        parsers = {name: importlib.metadata.version(name) for name in ("tree-sitter", "tree-sitter-javascript")}
        tool = identity(Path(evidence.__file__))
        bundles, failures = [], list(deployment["failures"])
        if classification is not None:
            failures.extend(classification["failures"])
        for source in deployment["sources"]:
            if not source["identity_verified"]:
                failures.append({"kind": "source_identity_unverified", "source": source})
                continue
            original = source["original"]
            if Path(urllib.parse.urlsplit(source["url"]).path).suffix.lower() not in {".js", ".mjs"}:
                continue
            digest = original["sha256"]
            with self.lock:
                lock = self.frontend_locks.setdefault(digest, threading.Lock())
            with lock, a.index_lock(a.ROOT / ".work/reverse-pending/locks" / ("frontend-" + digest + ".lock")):
                destination = a.ROOT / ".analysis/results/frontend/ui-trees" / (digest + ".json")
                parsed = json.loads(destination.read_bytes()) if destination.is_file() else None
                if (parsed is None or parsed.get("tool") != tool or parsed.get("parser") != parsers
                        or parsed.get("input_sha256") != digest):
                    raw = a.resolve(original["path"], ".analysis/inputs").read_bytes()
                    if hashlib.sha256(raw).hexdigest() != digest:
                        raise ValueError("Frontend source changed before AST analysis")
                    parsed = {"schema_version": 2, "input": original["path"], "input_sha256": digest,
                              "input_bytes": len(raw), "tool": tool, "parser": parsers,
                              **evidence.ui_tree(raw), "target_execution": False}
                    a.save(destination, parsed)
            bundles.append({"input": original, "tree": identity(destination), "parse_passed": parsed["parse_passed"],
                            "lexical_owners": len(parsed["owners"]), "elements": len(parsed["elements"])})
            if not parsed["parse_passed"]:
                failures.append({"kind": "javascript_parse_failed", "input": original["path"]})
        folder = "verified-frontend" if self.verify_only else "frontend"
        receipt = a.ROOT / ".analysis/results/reverse-pending" / folder / (path.parent.name + ".json")
        result = {"acquisition": identity(path), "recovery": recovery, "deployment": deployment,
                  "source_classification": classification,
                  "bundles": bundles, "repair_failures": repairs, "failures": failures,
                  "verification_only": self.verify_only,
                  "target_execution": False, "semantics_complete": False, "mount_tree_complete": False}
        a.save(receipt, result)
        return {"status": "unresolved" if failures or repairs or recovery["status"] in {
                    "failed", "outside_current_deployment_scope", "no_javascript_originals"} else "passed",
                "receipt": identity(receipt), "bundles": len(bundles), "failures": len(failures) + len(repairs),
                "semantic_review": "required"}

    def document(self):
        path = a.ROOT / DOC
        current = path.read_text(encoding="utf-8")
        summary = self.report["summary"]
        command = f"python -X utf8 tools/workspace.py reverse-pending --workers {self.workers}"
        if self.process_workers:
            command += " --process-workers"
        if self.resume:
            command += " --resume"
        elif self.verify_only:
            command += " --verify-only"
        if self.native_only and not self.resume:
            command += " --native-only"
        for spec in (() if self.resume else self.review_paths):
            command += ' --native-review "' + spec + '"'
        pool_kind = "进程池" if self.process_workers else "线程池"
        workflow = (f"本次以有界{pool_kind}复核原件身份、完整捕获及产物、补查冲突和前端解析；不启动 IDA 或网络修复。"
                    if self.verify_only else
                    f"本次以同一个有界{pool_kind}批量执行原生审阅计划，复用身份未变的未决结果，不处理前端。"
                    if self.native_only else
                    f"一个协调者使用同一个有界{pool_kind}处理原生缺失／失败函数、前端原件修复、模块恢复和页面 AST；")
        text = (BEGIN + "\n## 统一剩余静态任务队列\n\n"
                f"运行入口：`{command}`。"
                + workflow +
                "不执行厂商代码，不修改 Rust，不重新启动仍存活的 IDA 会话。\n\n"
                f"本次队列登记 {summary['tasks']} 项；状态：`{json.dumps(summary['by_status'], ensure_ascii=False)}`。"
                + (f"原生基础失败现已恢复 {summary['native_functions']['resolved']} 条、仍保留 {summary['native_functions']['remaining']} 条。"
                   if "native_functions" in summary else "") +
                "完整任务身份、实际命令、输出和未决原因见[统一收据](../../.analysis/results/reverse-pending.json)。"
                "passed 只表示该任务的静态检查通过；队列结束、伪代码恢复、完整语义、Rust 实现和实际设备验收分别记账，"
                "不由脚本自动提升为全功能完成。\n" + END)
        if BEGIN in current:
            start, end = current.index(BEGIN), current.index(END) + len(END)
            current = current[:start] + text + current[end:]
        else:
            current = current.rstrip() + "\n\n" + text + "\n"
        path.write_text(current, encoding="utf-8")

    def run(self):
        tool_raw = Path(__file__).read_bytes()
        if hashlib.sha256(tool_raw).hexdigest() != self.tool["sha256"]:
            raise ValueError("Analyzer source changed before queue start")
        tool_source = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (self.tool["sha256"] + ".py")
        tool_source.parent.mkdir(parents=True, exist_ok=True)
        if tool_source.is_file():
            if tool_source.read_bytes() != tool_raw:
                raise ValueError("Retained analyzer source changed")
        else:
            tool_source.write_bytes(tool_raw)
        self.report["tool_source"] = identity(tool_source)
        if self.resume:
            tasks = self.resume_tasks()
        elif self.verify_only or self.native_only:
            previous_path = a.ROOT / REPORT
            previous_raw = previous_path.read_bytes()
            previous = json.loads(previous_raw)
            if not previous.get("finished_at_utc"):
                raise ValueError("Previous static queue must finish before strict verification")
            self.report["previous_static_run"] = {
                "sha256": hashlib.sha256(previous_raw).hexdigest(),
                "raw_receipt": retain_receipt_bytes(previous_raw),
                "tool": previous["tool"], "summary": previous["summary"]}
            snapshot = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (previous["tool"]["sha256"] + ".py")
            if snapshot.is_file():
                retained = identity(snapshot)
                if retained["sha256"] != previous["tool"]["sha256"]:
                    raise ValueError("Retained previous analyzer source changed")
                self.report["previous_static_run"]["tool_source"] = retained
            if "scratch_cleanup" in previous:
                self.report["scratch_cleanup"] = previous["scratch_cleanup"]
            if self.native_only:
                self.previous_native = {row["sha256"]: row for row in previous["tasks"] if row["kind"] == "native"}
                self.report["native_scope"] = previous.get("native_scope") or {
                    row["sha256"]: {key: row[key] for key in ("resolved", "remaining", "verification_errors")}
                    for row in previous["tasks"] if row["kind"] == "native" and "remaining" in row}
                self.report["baseline_native"] = {
                    "resolved": sum(row["resolved"] for row in self.report["native_scope"].values()),
                    "remaining": sum(row["remaining"] for row in self.report["native_scope"].values())}
        self.report["configuration"] = {"ida_root": self.ida_root, "max_function_kib": 1024,
            "worker_isolation": "spawned_process" if self.process_workers else "thread"}
        self.report["queue_session"] = {"pid": os.getpid(), "started_at_utc": a.stamp(),
                                        "process_creation": process_creation_time(os.getpid())}
        if self.resume:
            self.publish()
        else:
            tasks = self.discover()
        print(json.dumps({"queue": REPORT, "workers": self.workers, "tasks": len(self.report["tasks"]),
                          "scheduled_tasks": len(tasks),
                          "by_kind": self.report["summary"]["by_kind"]}), flush=True)
        deferred = []
        with (self.process_pool() if self.process_workers else
              ThreadPoolExecutor(max_workers=self.workers, thread_name_prefix="reverse-pending")) as pool:
            def submit(task):
                if self.process_workers:
                    return pool.submit(execute_process_task, task, self.previous_native.get(task.get("sha256")))
                return pool.submit(self.execute, task)
            futures = {submit(task): task for task in tasks}
            while futures or deferred:
                if not futures:
                    time.sleep(5)
                    pending, deferred = deferred, []
                    for task in pending:
                        futures[submit(task)] = task
                done, _ = wait(futures, timeout=5, return_when=FIRST_COMPLETED)
                for future in done:
                    task = futures.pop(future)
                    result = future.result()
                    task.update(result)
                    if result["status"] == "waiting_external":
                        deferred.append(task)
                    else:
                        task["finished_at_utc"] = a.stamp()
                    self.publish()
                    print(json.dumps({"job": task["id"], "kind": task["kind"], "input": task["input"],
                                      "status": task["status"], "summary": self.report["summary"]["by_status"]},
                                     ensure_ascii=False), flush=True)
        self.report["finished_at_utc"] = a.stamp()
        self.report["queue_session"]["finished_at_utc"] = self.report["finished_at_utc"]
        self.publish()
        self.publish_gaps()
        return self.report

    def publish_gaps(self):
        path = a.ROOT / ".analysis/results/gaps.json"
        with a.index_lock(a.ROOT / ".analysis/results/gaps.lock"):
            ledger = json.loads(path.read_bytes()) if path.is_file() else {"schema_version": 1, "owners": {}}
            rows = [{"kind": "reverse_pending_task_unresolved", "task_id": task["id"],
                     "input": task["input"], "status": task["status"], "evidence": REPORT,
                     "result": {key: task[key] for key in ("resolution", "receipt", "error", "remaining", "verification_errors") if key in task}}
                    for task in self.report["tasks"] if task["status"] != "passed"]
            rows.append({"kind": "semantic_and_runtime_acceptance_pending", "evidence": REPORT,
                         "scope": "Static queue does not establish complete product semantics, Rust, UI wiring or actual DLL/device comparison"})
            ledger.setdefault("owners", {})["tools/reverse_pending.py"] = rows
            a.save(path, ledger)


def recheck_unresolved(args):
    """Recheck only explicit frozen failures, publishing a separate current increment."""
    if any(getattr(args, name, False) for name in ("resume", "resume_reason", "native_review")):
        raise ValueError("Unresolved recheck is a static increment, separate from resumed analyzer work")
    batch = Batch(args.workers, args.ida_root, verify_only=True,
                  process_workers=getattr(args, "process_workers", False))
    batch.recheck_mode = True
    baseline_path = a.ROOT / REPORT
    baseline_raw = baseline_path.read_bytes()
    baseline = json.loads(baseline_raw)
    if baseline.get("schema_version") != 1 or not baseline.get("finished_at_utc"):
        raise ValueError("Unresolved recheck requires a completed frozen baseline")
    candidates = [task for task in baseline["tasks"] if task.get("status") in {"unresolved", "failed"}]
    available = {task["id"]: task for task in candidates}
    if len(available) != len(candidates) or not available:
        raise ValueError("Frozen failures must have unique task identities")
    identifiers = getattr(args, "task_id", None)
    if identifiers is not None:
        if not identifiers or len(set(identifiers)) != len(identifiers) or set(identifiers) - available.keys():
            raise ValueError("Recheck task IDs must uniquely select existing frozen failures")
        candidates = [available[key] for key in identifiers]
    if getattr(args, "native_only", False):
        candidates = [task for task in candidates if task["kind"] == "native"]
    if not candidates:
        raise ValueError("Unresolved recheck selection is empty")
    for task in candidates:
        if task["kind"] == "native":
            source = a.resolve(task["input"], ".analysis/inputs")
            if a.sha(source) != task["sha256"] or not valid(task["base_capture"]):
                raise ValueError("Frozen native input or base capture changed")
        elif task["kind"] != "frontend":
            raise ValueError("Unknown frozen failure kind")
        record = task.get("resolution", task.get("receipt"))
        if record:
            frozen_result_reference(record)
    baseline_reference = retain_review_json(raw=baseline_raw)
    tool_path = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (batch.tool["sha256"] + ".py")
    tool_raw = Path(__file__).read_bytes()
    tool_path.parent.mkdir(parents=True, exist_ok=True)
    if tool_path.is_file():
        if tool_path.read_bytes() != tool_raw:
            raise ValueError("Retained recheck tool source changed")
    else:
        with tool_path.open("xb") as stream:
            stream.write(tool_raw)
    tool_source = identity(tool_path)
    destination = a.ROOT / UNRESOLVED_REPORT
    previous = json.loads(destination.read_bytes()) if destination.is_file() else None
    if previous and previous.get("baseline") != baseline_reference:
        raise ValueError("Current unresolved increment belongs to a different frozen baseline")
    rows = {row["baseline_task_id"]: row for row in previous.get("tasks", [])} if previous else {}
    if set(rows) - available.keys():
        raise ValueError("Current unresolved increment contains a foreign task")
    report = {"schema_version": 1, "kind": "frozen_unresolved_static_increment", "baseline": baseline_reference,
              "baseline_failure_count": len(available), "selected_task_ids": [task["id"] for task in candidates],
              "started_at_utc": a.stamp(), "tool": batch.tool, "tool_reference": tool_source,
              "target_execution": False, "semantics_complete": False,
              "official_dll_comparison": "not_executed", "real_device_acceptance": "not_executed"}

    def publish():
        if baseline_path.read_bytes() != baseline_raw or identity(Path(__file__)) != batch.tool:
            raise ValueError("Frozen baseline or recheck source changed during verification")
        report["tasks"] = [rows[key] for key in sorted(rows)]
        counts = Counter(row["status"] for row in report["tasks"])
        report["summary"] = {"baseline_failures": len(available), "rechecked": len(rows),
                             "by_status": dict(counts), "not_rechecked": len(available) - len(rows),
                             "semantics_complete": False}
        reference = retain_review_json(report)
        a.save(destination, {**report, "source_review": reference})

    with batch.process_pool() if batch.process_workers else ThreadPoolExecutor(max_workers=batch.workers) as pool:
        futures = {pool.submit(execute_process_task if batch.process_workers else batch.execute, task): task
                   for task in candidates}
        for future in as_completed(futures):
            task = futures[future]
            result = future.result()
            record = task.get("resolution", task.get("receipt"))
            rows[task["id"]] = {"baseline_task_id": task["id"], "kind": task["kind"], "input": task["input"],
                **({"sha256": task["sha256"]} if "sha256" in task else {}),
                "historical_result": frozen_result_reference(record) if record else None,
                **result, "finished_at_utc": a.stamp()}
            print(json.dumps({"task": task["id"], "kind": task["kind"], "status": result["status"]}), flush=True)
    report["finished_at_utc"] = a.stamp()
    publish()
    print(json.dumps({"receipt": UNRESOLVED_REPORT, **report["summary"]}), flush=True)
    return report


def run(args):
    with a.index_lock(a.ROOT / ".analysis/reverse-pending.lock"):
        if getattr(args, "recheck_unresolved", False):
            report = recheck_unresolved(args)
            if any(task["status"] != "passed" for task in report["tasks"]):
                raise SystemExit(1)
            return report
        if getattr(args, "task_id", None):
            raise ValueError("Task selection requires --recheck-unresolved")
        options = {}
        if getattr(args, "native_only", False) or getattr(args, "native_review", None):
            options = {"native_only": getattr(args, "native_only", False),
                       "review_paths": getattr(args, "native_review", None) or ()}
        if getattr(args, "resume", False):
            options["resume"] = True
        if getattr(args, "process_workers", False):
            options["process_workers"] = True
        if getattr(args, "resume_reason", None) is not None:
            options["resume_reason"] = args.resume_reason
        report = Batch(args.workers, args.ida_root, getattr(args, "verify_only", False), **options).run()
        if any(task["status"] != "passed" for task in report["tasks"]):
            raise SystemExit(1)
        return report
