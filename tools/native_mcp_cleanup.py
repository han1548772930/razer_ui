"""Release a completed MCP-only private copy through the workspace cleaner.

The semantic owner, original, ended process and all retained selected queries
must remain independently verifiable. No whole-library capture is required.
"""
from collections import defaultdict
from datetime import datetime, timezone
import json
from pathlib import Path
import re


def candidates(w, selectors):
    import code_identity as ci
    import code_identity_native
    import host
    import reverse_pending
    result = []
    for selector in selectors:
        match = re.fullmatch(r"(\.analysis/results/frontend/[A-Za-z0-9_-]+\.json)#/([A-Za-z0-9_]+)", selector)
        if not match:
            raise ValueError("MCP cleanup owner must be a formal frontend semantic section")
        owner_name, section_name = match.groups()
        owner_path = w.safe(w.ROOT / owner_name)
        report = next((value for key, value in ci.stream_sections(owner_path) if key == section_name), None)
        if (not isinstance(report, dict) or report.get("private_analysis_database_required") is not False
                or any(report.get(key) is not True for key in ("source_slice_complete", "bounded_observation_complete", "snapshot_stable"))
                or report.get("failures") != [] or report.get("changed_during_query") != []
                or report.get("target_execution") is not False):
            raise ValueError("MCP semantic scope has not released its private database")
        original = report["binary_scope"]["original"]
        sha = original["sha256"]
        if not re.fullmatch(r"[a-f0-9]{64}", sha):
            raise ValueError("MCP original has an invalid byte identity")
        name = ".work/native/" + sha + "/mcp"
        temporary = report["temporary_resources_created"]
        if (not isinstance(temporary, list) or len(temporary) != 1 or not isinstance(temporary[0], dict)
                or temporary[0].get("path") != name or temporary[0].get("required_after_publication") is not False):
            raise ValueError("MCP semantic owner has no exact released directory")
        target = w._clean_path(name)
        if not target.exists():
            continue
        base = ".analysis/results/native/" + sha
        identities = {}

        def retained(reference, prefix):
            path = reference["path"]
            if (not isinstance(path, str) or not path.startswith(prefix) or ".." in Path(path).parts
                    or Path(path).is_absolute() or "\\" in path):
                raise ValueError("MCP retained evidence escaped its declared boundary")
            actual = w.safe(w.ROOT / path)
            if any(w._scratch_link(p) for p in [actual, *actual.parents] if p.exists()):
                raise ValueError("MCP retained evidence is linked")
            identity = w._clean_file_identity(actual)
            if any(identity[k] != reference[k] for k in ("sha256", "bytes") if k in reference):
                raise ValueError("MCP retained evidence changed: " + path)
            identities[path] = identity
            return actual

        retained({"path": owner_name}, ".analysis/results/frontend/")
        raw_path = retained(original, ".analysis/inputs/")
        source = raw_path.read_bytes()
        sessions = report["new_analysis_sessions"]
        if len(sessions) != 1 or sessions[0]["path"] != base + "/mcp-session.json":
            raise ValueError("MCP owner requires its one exact analysis session")
        session_path = retained(sessions[0], base + "/")
        plan_ref = report["analysis_plan"]
        if plan_ref["path"] != base + "/plan.json":
            raise ValueError("MCP owner references another analysis plan")
        plan_path = retained(plan_ref, base + "/")
        session, plan = json.loads(session_path.read_bytes()), json.loads(plan_path.read_bytes())
        shutdown = session.get("shutdown", {})
        private = name + "/" + raw_path.name
        if (session.get("mode") != "mcp" or session.get("input") != private
                or session.get("target_execution") is not False or session.get("real_device_operations") is not False
                or session.get("private_analysis_database_required") is not False
                or session.get("database_resume") is not None or plan.get("database_resume") is not None
                or plan.get("database_rebuild") is not None or plan.get("input") != original["path"]
                or plan.get("sha256") != sha or plan.get("output") != base
                or shutdown.get("process_instance_confirmed_absent") is not True
                or shutdown.get("process_instance", {}).get("pid") != session.get("pid")
                or shutdown.get("target_execution") is not False or report.get("analyzer_shutdown") != shutdown):
            raise ValueError("MCP source/session/termination ownership differs")
        if (type(session.get("pid")) is not int or session["pid"] <= 0
                or reverse_pending.coordinator_instance_absence({"pid": session["pid"], "started_at_utc": session["started_at_utc"]}).get("confirmed_absent") is not True):
            raise ValueError("MCP process instance has not ended")
        times = [datetime.fromisoformat(value) for value in (session["started_at_utc"], shutdown["requested_at_utc"],
                 shutdown["observed_at_utc"], session["finished_at_utc"], report["recorded_at_utc"])]
        if any(t.tzinfo is None for t in times) or times != sorted(times) or times[-1] > datetime.now(timezone.utc):
            raise ValueError("MCP termination/publication ordering is invalid")
        if w.digest(w._clean_path(private)) != sha:
            raise ValueError("MCP private input changed")
        pe = host.inspect_pe(source)
        image_base = int(pe["image_base"], 0)

        def read(rva, size):
            matches = [s for s in pe["sections"] if s["rva"] <= rva and rva + size <= s["rva"] + s["bytes"]]
            if size <= 0 or len(matches) != 1:
                raise ValueError("MCP selected bytes are outside an original PE section")
            offset = matches[0]["file_offset"] + rva - matches[0]["rva"]
            return source[offset:offset + size]

        selected = defaultdict(list)
        for item in report["native_evidence"]["owners"]:
            selected[item["capture"]["path"]].append(item)
        if not selected:
            raise ValueError("MCP release has no selected retained native owners")
        query_refs = report["query_receipts"]
        query_names = {r["path"] for r in query_refs}
        if len(query_names) != len(query_refs) or not set(selected) <= query_names:
            raise ValueError("MCP selected owners lost their query inventory")
        actual_queries = {w.relative(p) for p in (w.ROOT / base / "queries").glob("*.json")}
        if actual_queries != query_names:
            raise ValueError("MCP query inventory is incomplete or has unreviewed additions")
        verified = 0
        failures = []
        for reference in query_refs:
            query_path = retained(reference, base + "/queries/")
            if query_path.stem != reference["sha256"]:
                raise ValueError("MCP query is not content-addressed")
            query = json.loads(query_path.read_bytes())
            if query.get("target_execution") is not False or query.get("input_sha256", query.get("source_sha256")) != sha:
                raise ValueError("MCP query has a different source or execution scope")
            failures.extend({"query": reference["path"], "failure": f} for f in query.get("failures", []))
            for item in selected.get(reference["path"], []):
                if item["capture"] != reference or not re.fullmatch(r"/functions/(0|[1-9][0-9]*)", item["pointer"]):
                    raise ValueError("MCP selected owner has an invalid query pointer")
                body = query["functions"][int(item["pointer"].rsplit("/", 1)[1])]
                if ci.digest(body) != item["canonical_sha256"] or not body.get("pseudocode"):
                    raise ValueError("MCP selected function body or pseudocode differs")
                actual = code_identity_native.function_identity(body, image_base, sha, pe["architecture"], read)
                if actual != item["function_identity"]:
                    raise ValueError("MCP selected function no longer matches original instructions")
                verified += 1
        allowed = {raw_path.name, "ida.log", "process.log", *(raw_path.name + suffix for suffix in (".i64", ".idb", ".id0", ".id1", ".id2", ".nam", ".til"))}
        if any(not p.is_file() or w._scratch_link(p) or p.name not in allowed for p in target.iterdir()):
            raise ValueError("MCP directory contains unknown or linked resources")
        result.append({"path": name, "kind": "ended_native_mcp_private_copy", "native_mcp_owner": selector,
                       "analyzer_pid": session["pid"], "owner_receipts": [owner_name, w.relative(session_path), w.relative(plan_path), *sorted(query_names)],
                       "basis": list(identities.values()), "selected_functions_verified": verified,
                       "retained_query_failures": failures, "observed_exit_code": session.get("exit_code")})
    if len({r["path"] for r in result}) != len(result):
        raise ValueError("MCP release selects one directory more than once")
    return result
