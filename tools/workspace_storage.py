"""Workspace size accounting and explicit Cargo profile cleanup plans."""
from concurrent.futures import ThreadPoolExecutor
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess

import analysis as a


def _windows_path(path):
    value = str(Path(path).resolve())
    return "\\\\?\\" + value if os.name == "nt" and not value.startswith("\\\\") else value


def measure(name):
    """Report logical and allocated bytes separately, excluding reparse points."""
    root = a.ROOT / name
    groups, failures, links, seen = {}, [], [], set()
    allocated_size = None
    if os.name == "nt":
        allocated_size = ctypes.WinDLL("kernel32", use_last_error=True).GetCompressedFileSizeW
        allocated_size.argtypes = [wintypes.LPCWSTR, ctypes.POINTER(wintypes.DWORD)]
        allocated_size.restype = wintypes.DWORD
    pending = [(_windows_path(root), "")]
    while pending:
        directory, prefix = pending.pop()
        try:
            with os.scandir(directory) as entries:
                for entry in entries:
                    relative = prefix + entry.name
                    try:
                        st = entry.stat(follow_symlinks=False)
                        # Windows DirEntry's cached stat omits file identity fields.
                        if os.name == "nt" and not st.st_ino:
                            st = os.stat(entry.path, follow_symlinks=False)
                        if getattr(st, "st_file_attributes", 0) & 0x400 or entry.is_symlink():
                            links.append(relative)
                        elif entry.is_dir(follow_symlinks=False):
                            pending.append((entry.path, relative + "/"))
                        else:
                            group = relative.split("/", 1)[0]
                            row = groups.setdefault(group, {"files": 0, "logical_bytes": 0, "allocated_bytes": 0})
                            row["files"] += 1
                            row["logical_bytes"] += st.st_size
                            file_id = (st.st_dev, st.st_ino)
                            if st.st_ino and file_id in seen:
                                continue
                            if st.st_ino:
                                seen.add(file_id)
                            if allocated_size:
                                high = wintypes.DWORD()
                                ctypes.set_last_error(0)
                                low = allocated_size(entry.path, ctypes.byref(high))
                                if low == 0xffffffff and ctypes.get_last_error():
                                    raise ctypes.WinError(ctypes.get_last_error())
                                row["allocated_bytes"] += (high.value << 32) | low
                            else:
                                row["allocated_bytes"] += getattr(st, "st_blocks", 0) * 512
                    except OSError as error:
                        failures.append({"path": relative, "error": str(error)})
        except OSError as error:
            failures.append({"path": prefix, "error": str(error)})
    return {"path": name, "groups": groups, "failures": failures, "reparse_points_excluded": links,
            **{key: sum(row[key] for row in groups.values()) for key in ("files", "logical_bytes", "allocated_bytes")}}


def _build_boundary():
    root = a.ROOT.resolve()
    for name in ("target", "target/debug"):
        path = root / name
        if path.exists() and (path.is_symlink() or path.is_junction() or path.resolve() != path):
            raise ValueError("Cargo cleanup boundary is linked or escaped")
    return root / "target"


def _no_build_process():
    import workspace
    processes, errors = workspace._scratch_processes()
    if errors:
        raise ValueError("Process inventory failed: " + str(errors))
    active = [row for row in processes if Path(row.get("executable", "")).stem.lower() in {"cargo", "rustc", "rustdoc"}
              or any(token in row.get("command_line", "").lower() for token in ("cargo.exe", "rustc.exe", "rustdoc.exe"))]
    if active:
        raise ValueError("Build processes still active: " + str([row["pid"] for row in active]))


def _save_new(path, value):
    with path.open("x", encoding="utf-8") as output:
        json.dump(value, output, ensure_ascii=False, indent=2)
        output.write("\n")


def _receipt(name):
    path = a.ROOT / name
    if path.parent.resolve() != (a.ROOT / ".analysis/results").resolve() or path.suffix != ".json":
        raise ValueError("Storage receipt must be a direct results JSON")
    return path


def plan(path):
    _build_boundary()
    with ThreadPoolExecutor(max_workers=3) as pool:
        before = list(pool.map(measure, (".analysis", "target", ".work")))
    dedup = {"scope": "current byte-verified first-tier frontend originals in code-identity.sqlite3",
             "status": "planned_not_applied", "groups": 0, "duplicate_paths": 0, "logical_duplicate_bytes": 0}
    database = a.ROOT / ".analysis/results/code-identity.sqlite3"
    if database.is_file():
        with sqlite3.connect(database) as db:
            row = db.execute("""SELECT COUNT(*),COALESCE(SUM(n-1),0),COALESCE(SUM((n-1)*bytes),0)
                FROM (SELECT s.sha256,s.bytes,COUNT(DISTINCT s.path) n FROM sources s
                JOIN deployment_sources ds ON ds.path=s.path GROUP BY s.sha256 HAVING n>1)""").fetchone()
            dedup.update(zip(("groups", "duplicate_paths", "logical_duplicate_bytes"), row))
    report = {"schema_version": 2, "kind": "workspace_storage_plan", "recorded_at_utc": a.stamp(),
        "allocation_method": "GetCompressedFileSizeW, hardlinks deduplicated with os.stat file identity",
        "target_execution": False, "before": before,
        "cargo_cleanup": {"profile": "dev", "target": "target/debug", "preserve": ["target/verification-temp", "target/flycheck0", "target/tmp"],
                          "argv": ["cargo", "clean", "--profile", "dev", "--target-dir", str(_build_boundary())]},
        "analysis_deduplication": dedup,
        "migration_order": [
            "Use verified byte SHA as a blob key and keep acquisition URL, version and stability provenance.",
            "Rebind current source consumers to blob identities atomically; verify every former path reference before deleting copies.",
            "Keep historical evidence resolvable; never edit referenced canonical receipts to improve counts.",
            "Compact large immutable reviews with the existing compact-analysis/review retention workflow, verifying logical SHA.",
            "Execute original deletion only through a separately reviewed dependency and identity bound plan."],
        "scratch_cleanup": "Use workspace.py clean plan/apply; retained dependencies and active databases remain protected."}
    _save_new(path, report)
    return {"plan": a.rel(path), "sha256": a.sha(path), "before": [{k:v for k,v in row.items() if k != "groups"} for row in before],
            "analysis_deduplication": dedup}


def apply(path, expected_sha):
    if a.sha(path) != expected_sha:
        raise ValueError("Storage plan SHA changed")
    report = json.loads(path.read_bytes())
    if report.get("kind") != "workspace_storage_plan" or report.get("schema_version") != 2:
        raise ValueError("Wrong storage plan kind")
    expected = ["cargo", "clean", "--profile", "dev", "--target-dir", str(_build_boundary())]
    if report["cargo_cleanup"]["argv"] != expected:
        raise ValueError("Cargo cleanup scope changed")
    _no_build_process()
    before = measure("target/debug")
    if before["failures"] or before["reparse_points_excluded"]:
        raise ValueError("Build output inventory is incomplete or linked")
    _build_boundary()
    _no_build_process()
    destination = path.with_name(path.stem + "-applied.json")
    result = {"schema_version": 1, "kind": "workspace_storage_apply", "plan": a.rel(path),
              "plan_sha256": expected_sha, "status": "started", "target_execution": False, "before_profile": before}
    _save_new(destination, result)
    try:
        completed = subprocess.run(expected, cwd=a.ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
        result.update(exit_code=completed.returncode, stdout=completed.stdout, stderr=completed.stderr,
                      status="completed" if completed.returncode == 0 else "failed")
    except BaseException as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        # Cargo owns deletion; a remaining partial profile remains visible in the receipt.
        after = measure("target/debug") if (a.ROOT / "target/debug").exists() else {"logical_bytes": 0, "allocated_bytes": 0, "files": 0, "failures": []}
        result.update(after_profile=after, finished_at_utc=a.stamp(),
                      logical_bytes_removed=before["logical_bytes"] - after["logical_bytes"],
                      allocated_bytes_reclaimed=before["allocated_bytes"] - after["allocated_bytes"])
        a.save(destination, result)
    return {key: value for key, value in result.items() if key not in {"before_profile", "after_profile"}}


def run(args):
    if args.finish:
        applied_path = _receipt(args.finish)
        applied = json.loads(applied_path.read_bytes())
        source = _receipt(applied["plan"])
        if applied.get("kind") != "workspace_storage_apply" or a.sha(source) != applied["plan_sha256"]:
            raise ValueError("Storage completion has no unchanged owning plan")
        planned = json.loads(source.read_bytes())
        with ThreadPoolExecutor(max_workers=3) as pool:
            after = list(pool.map(measure, (".analysis", "target", ".work")))
        result = {"schema_version": 2, "kind": "workspace_storage_completion", "recorded_at_utc": a.stamp(),
                  "apply_receipt": {"path": a.rel(applied_path), "sha256": a.sha(applied_path)},
                  "status": applied["status"], "target_execution": False,
                  "before": planned["before"], "after": after,
                  "cargo_allocated_bytes_reclaimed": applied["allocated_bytes_reclaimed"],
                  "analysis_deduplication": planned["analysis_deduplication"]}
        destination = applied_path.with_name(applied_path.stem + "-summary.json")
        _save_new(destination, result)
        return {"receipt": a.rel(destination), "sha256": a.sha(destination),
                "after": [{k:v for k,v in row.items() if k != "groups"} for row in after],
                "cargo_allocated_bytes_reclaimed": applied["allocated_bytes_reclaimed"]}
    if args.apply:
        if not args.plan_sha256:
            raise ValueError("Apply requires the retained plan SHA")
        return apply(_receipt(args.apply), args.plan_sha256)
    return plan(_receipt(args.plan))
