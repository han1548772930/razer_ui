"""Resume retained native and frontend evidence in one bounded static queue."""
from __future__ import annotations

from collections import Counter
from concurrent.futures import ThreadPoolExecutor, wait, FIRST_COMPLETED
import base64
import ctypes
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time
from types import SimpleNamespace
import urllib.parse

import analysis as a

REPORT = ".analysis/results/reverse-pending.json"
DOC = "docs/re/native-bulk-reverse.md"
BEGIN = "<!-- reverse-pending:begin -->"
END = "<!-- reverse-pending:end -->"
INPUT_PREFIXES = (".analysis/inputs/official-native-current/",
                  ".analysis/inputs/host-current/", ".analysis/inputs/installed-current/")


def identity(path):
    raw = path.read_bytes()
    return {"path": a.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def valid(row):
    try:
        path = a.resolve(row["path"], ".analysis")
        return path.is_file() and path.stat().st_size == row["bytes"] and a.sha(path) == row["sha256"]
    except (OSError, KeyError, ValueError, TypeError):
        return False


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
            for recoveries in (types, imports):
                if len({int(row["rva"], 0) for row in recoveries}) != len(recoveries):
                    raise ValueError("Native review repeats a recovered type")
                if any(not row.get("declaration", "").strip() or not row.get("assembly_evidence", "").strip() for row in recoveries):
                    raise ValueError("Native review types require declarations and assembly evidence")
            if any(not row.get("assembly_evidence", "").strip() for row in [*merges, *outlines, *partitions, *shared_tails]):
                raise ValueError("Native function boundary repairs require complete assembly evidence")
            note = "\n\n".join(dict.fromkeys(row["assembly_evidence"] for row in [*types, *imports, *merges, *outlines, *partitions, *shared_tails]))
            merge_options = {"merge_specs": [row["owner_rva"] + "=" + ",".join(row["member_rvas"]) for row in merges]} if merges else {}
            if outlines:
                merge_options["outline_rvas"] = [row["rva"] for row in outlines]
            if partitions:
                merge_options["partition_specs"] = partition_specs
            if shared_tails:
                merge_options["shared_tail_specs"] = shared_specs
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
    def __init__(self, workers, ida_root, verify_only=False, native_only=False, review_paths=(), resume=False):
        if type(workers) is not int or not 2 <= workers <= min(32, os.cpu_count() or 2):
            raise ValueError("Workers must be an integer from two to the available CPU limit (maximum 32)")
        if resume and (verify_only or native_only or review_paths):
            raise ValueError("Resume inherits the retained scope and reviews; do not combine it with scope options")
        self.workers, self.ida_root = workers, ida_root
        self.resume = resume
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

    def resume_tasks(self):
        """Continue the frozen queue under the caller's exclusive queue lock."""
        previous_raw = (a.ROOT / REPORT).read_bytes()
        previous = json.loads(previous_raw)
        if previous.get("schema_version") != 1 or previous.get("finished_at_utc"):
            raise ValueError("Resume requires a supported unfinished static queue")
        session = previous.get("queue_session")
        if session and process_alive(session.get("pid")) is not False:
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

        def check_records(value):
            if isinstance(value, dict):
                if {"path", "bytes", "sha256"}.issubset(value) and not valid(value):
                    raise ValueError("Retained queue artifact changed: " + value["path"])
                for child in value.values():
                    check_records(child)
            elif isinstance(value, list):
                for child in value:
                    check_records(child)

        tasks, identifiers, scheduled = previous["tasks"], set(), []
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
                for key in ("resolution", "receipt"):
                    if key in task:
                        record = task[key]
                        if not valid(record):
                            raise ValueError("Retained completed task result changed")
                        check_records(json.loads(a.resolve(record["path"], ".analysis/results").read_bytes()))
            else:
                raise ValueError("Retained queue task has an unsupported status or missing completion timestamp")
        retained_source = self.report["tool_source"]
        self.report = previous
        self.report["interrupted_static_run"] = {
            "sha256": hashlib.sha256(previous_raw).hexdigest(),
            "raw_base64": base64.b64encode(previous_raw).decode(),
            "observed_at_utc": a.stamp(), "exit_code": None,
            "coordinator_process_confirmed_absent": True if session else None,
            "legacy_coordinator_guard": None if session else "exclusive_queue_lock; no legacy coordinator PID was recorded",
            "completed_task_ids": [task["id"] for task in tasks if task not in scheduled],
            "scheduled_task_ids": [task["id"] for task in scheduled]}
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
            observation_evidence=review.get("observation_evidence_spec"))
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
                    for row in review.get("function_merges", []):
                        arguments.extend(["--merge-functions", row["owner_rva"] + "=" + ",".join(row["member_rvas"])])
                    for row in review.get("function_outlines", []):
                        arguments.extend(["--outline-function-rva", row["rva"]])
                    for specification in native_partition_specifications(review.get("function_partitions", [])):
                        arguments.extend(["--partition-functions", specification])
                    for specification in native_shared_tail_specifications(review.get("function_shared_tails", [])):
                        arguments.extend(["--share-function-tail", specification])
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
            a.save(receipt, compact)
            return {"status": "unresolved" if resolution["remaining_failures"] or resolution["verification_failures"] else "passed",
                    "resolution": identity(receipt), "commands": commands,
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
        relevant = {".js", ".css", ".html"}
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
        parsers = {name: importlib.metadata.version(name) for name in ("tree-sitter", "tree-sitter-javascript")}
        tool = identity(Path(evidence.__file__))
        bundles, failures = [], list(deployment["failures"])
        for source in deployment["sources"]:
            if not source["identity_verified"]:
                failures.append({"kind": "source_identity_unverified", "source": source})
                continue
            original = source["original"]
            if Path(urllib.parse.urlsplit(source["url"]).path).suffix.lower() != ".js":
                continue
            digest = original["sha256"]
            with self.lock:
                lock = self.frontend_locks.setdefault(digest, threading.Lock())
            with lock:
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
        if self.resume:
            command += " --resume"
        elif self.verify_only:
            command += " --verify-only"
        if self.native_only and not self.resume:
            command += " --native-only"
        for spec in (() if self.resume else self.review_paths):
            command += ' --native-review "' + spec + '"'
        workflow = ("本次只复核原件身份、完整捕获及产物、补查冲突和前端解析；不启动 IDA 或网络修复。"
                    if self.verify_only else
                    "本次以同一个有界线程池批量执行原生审阅计划，复用身份未变的未决结果，不处理前端。"
                    if self.native_only else
                    "一个协调者使用同一个有界线程池处理原生缺失／失败函数、前端原件修复、模块恢复和页面 AST；")
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
                "raw_base64": base64.b64encode(previous_raw).decode(),
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
        self.report["configuration"] = {"ida_root": self.ida_root, "max_function_kib": 1024}
        self.report["queue_session"] = {"pid": os.getpid(), "started_at_utc": a.stamp()}
        if self.resume:
            self.publish()
        else:
            tasks = self.discover()
        print(json.dumps({"queue": REPORT, "workers": self.workers, "tasks": len(self.report["tasks"]),
                          "scheduled_tasks": len(tasks),
                          "by_kind": self.report["summary"]["by_kind"]}), flush=True)
        deferred = []
        with ThreadPoolExecutor(max_workers=self.workers, thread_name_prefix="reverse-pending") as pool:
            futures = {pool.submit(self.execute, task): task for task in tasks}
            while futures or deferred:
                if not futures:
                    time.sleep(5)
                    pending, deferred = deferred, []
                    for task in pending:
                        futures[pool.submit(self.execute, task)] = task
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


def run(args):
    with a.index_lock(a.ROOT / ".analysis/reverse-pending.lock"):
        options = {}
        if getattr(args, "native_only", False) or getattr(args, "native_review", None):
            options = {"native_only": getattr(args, "native_only", False),
                       "review_paths": getattr(args, "native_review", None) or ()}
        if getattr(args, "resume", False):
            options["resume"] = True
        report = Batch(args.workers, args.ida_root, getattr(args, "verify_only", False), **options).run()
        if any(task["status"] != "passed" for task in report["tasks"]):
            raise SystemExit(1)
        return report
