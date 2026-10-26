"""Recover webpack modules and source positions without executing target code."""
from __future__ import annotations

from bisect import bisect_right
import concurrent.futures
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from types import SimpleNamespace
import urllib.parse
import urllib.error
import urllib.request


BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"


def canonical_frontend_entry(url):
    """Normalize only directory spellings of an official deployment entry."""
    if not isinstance(url, str):
        raise ValueError("Frontend entry has no official URL")
    parsed = urllib.parse.urlsplit(url)
    if (parsed.scheme != "https" or parsed.netloc != "apps.razer.com"
            or parsed.fragment or ".." in parsed.path.split("/")):
        raise ValueError("Frontend entry is outside the verified official origin")
    path = parsed.path
    if path.endswith("/index.html"):
        path = path[:-len("index.html")]
    elif not Path(path).suffix:
        path = path.rstrip("/") + "/"
    return urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, path, parsed.query, ""))


def current_source_selection(entry_url, audit, *, candidates):
    """Select the newest retained stable acquisition for one actual route.

    Candidate receipts are explicit, bounded evidence. A newer stable claim
    must pass its own byte checks; a failure never selects an older deployment.
    Capture validation remains separate from deployment-source selection.
    """
    import analysis as a
    import evidence
    route = canonical_frontend_entry(entry_url)
    boundary = (a.ROOT / ".analysis/inputs/frontend").resolve()
    decisions, eligible, seen = [], [], set()
    for candidate in candidates:
        path = Path(candidate)
        path = (a.ROOT / path).resolve() if not path.is_absolute() else path.resolve()
        if (path.name != "acquisition.json" or path.parent.parent != boundary
                or path in seen):
            raise ValueError("Frontend source candidate boundary is invalid or repeated")
        seen.add(path)
        raw = audit.artifact(path)
        acquired = json.loads(raw)
        decision = {"name": path.parent.name, "acquisition": audit.artifacts[a.rel(path)]}
        decisions.append(decision)
        try:
            same_route = canonical_frontend_entry(acquired.get("entry_url")) == route
        except ValueError:
            same_route = False
        if not same_route:
            decision["status"] = "different_or_unverified_deployment_route"
            continue
        if acquired.get("target_execution") is not False:
            raise ValueError("Frontend candidate does not establish static-only acquisition")
        if acquired.get("source_current_during_acquisition") is not True:
            decision["status"] = "not_a_stable_current_acquisition"
            continue
        recorded = datetime.fromisoformat(acquired["recorded_at_utc"])
        if recorded.tzinfo is None:
            raise ValueError("Frontend source acquisition time has no timezone")
        decision["recorded_at_utc"] = acquired["recorded_at_utc"]
        eligible.append((recorded, path, acquired, decision))
    if not eligible:
        raise ValueError("No stable official frontend acquisition covers the requested route")
    latest = max(row[0] for row in eligible)
    newest = [row for row in eligible if row[0] == latest]
    entry_identities, url_identities = set(), {}
    for _, _, acquired, _ in newest:
        entries = [row for row in acquired["files"] if row.get("url") == acquired["entry_url"]]
        if len(entries) != 1 or not entries[0].get("path"):
            raise ValueError("Newest frontend acquisition has no unique retained entry")
        entry_identities.add((entries[0].get("bytes"), entries[0].get("sha256")))
        for original in acquired["files"]:
            if not original.get("path"):
                continue
            url = original.get("url")
            identity = original.get("bytes"), original.get("sha256")
            if url in url_identities and url_identities[url] != identity:
                raise ValueError("Same-time frontend candidates have conflicting URL bytes")
            url_identities[url] = identity
    if len(entry_identities) != 1:
        raise ValueError("Same-time frontend candidates have conflicting entry bytes")
    _, path, acquired, decision = min(newest, key=lambda row: row[1].parent.name)
    deployment = evidence.frontend_deployment_sources(acquired)
    if deployment["failures"]:
        raise ValueError("Newest frontend deployment failed strict verification: " +
                         json.dumps(deployment["failures"], sort_keys=True))
    claims = [row for row in acquired.get("stability", []) if row.get("url") == acquired["entry_url"]]
    entry = deployment["entry_original"]
    if (entry is None or len(claims) != 1 or claims[0].get("unchanged") is not True
            or any(claims[0].get(key) != entry["sha256"] for key in ("before_sha256", "after_sha256"))):
        raise ValueError("Newest frontend entry has no exact two-round stability identity")
    for original in acquired["files"]:
        if original.get("path"):
            audit.read(original)
    originals = [row["original"] for row in deployment["sources"] if row["identity_verified"]]
    capture_path = a.ROOT / ".analysis/results/frontend" / path.parent.name / "analysis.json"
    capture = audit.frontend_capture(path.parent.name, originals) if capture_path.is_file() else None
    for old_time, old_path, old_acquired, old_decision in eligible:
        old_decision["status"] = ("selected_latest_stable_acquisition" if old_path == path else
                                  "same_time_identical_entry" if old_time == latest else
                                  "superseded_stable_acquisition")
    return {"name": path.parent.name, "requested_entry_url": entry_url, "canonical_entry_url": route,
            "acquisition": decision["acquisition"], "deployment": deployment, "originals": originals,
            "capture": capture, "capture_failures": capture["capture_failures"] if capture else ["complete_capture_unavailable"],
            "candidate_decisions": decisions,
            "failed_acquisition": [row for row in acquired["files"] if not row.get("path")],
            "unresolved_acquisition": acquired.get("unresolved", []),
            "source_semantics_complete": False, "target_execution": False}


def _frozen_frontend_artifact(expected, audit):
    """Locate exact frozen bytes even when their mutable receipt was updated."""
    import analysis as a
    if (not re.fullmatch(r"[a-f0-9]{64}", expected.get("sha256", ""))
            or type(expected.get("bytes")) is not int or expected["bytes"] < 0):
        raise ValueError("Frozen frontend artifact has an invalid identity")
    candidates = [a.resolve(expected["path"], ".analysis"),
                  a.ROOT / ".analysis/results/reverse-pending/review-sources" / (expected["sha256"] + ".json")]
    for path in candidates:
        if not path.is_file():
            continue
        raw = path.read_bytes()
        if len(raw) == expected["bytes"] and hashlib.sha256(raw).hexdigest() == expected["sha256"]:
            return json.loads(audit.artifact(path, expected)), audit.artifacts[a.rel(path)]
    raise ValueError("Exact frozen frontend bytes are unavailable: " + expected["path"])


def _recheck_frontend_originals(input_dir, audit, receipt=None):
    """Read originals, direct provider captures and complete ASTs without writes."""
    import analysis as a
    import evidence
    acquired_path = input_dir / "acquisition.json"
    acquired = json.loads(audit.artifact(acquired_path))
    if acquired.get("target_execution") is not False:
        raise ValueError("Frontend acquisition has no static-only execution boundary")
    files, providers = {}, {}
    for row in acquired["files"]:
        url = row["url"]
        if url in files:
            raise ValueError("Frontend acquisition repeats a URL: " + url)
        files[url] = row
        if not row.get("path"):
            continue
        audit.read(row)
        dependency = row.get("retained_dependency")
        if dependency is None:
            continue
        key = dependency["acquisition"]["path"]
        if key not in providers:
            provider = json.loads(audit.artifact(a.ROOT / key, dependency["acquisition"]))
            provider_dir = a.resolve(key, ".analysis/inputs/frontend").parent
            summary = audit.frontend_capture(provider_dir.name, [])
            if summary["manifest"] != dependency["capture"] or provider.get("source_current_during_acquisition") is not True:
                raise ValueError("Retained frontend provider identity or current scope differs")
            providers[key] = {"acquisition": dependency["acquisition"], "capture": summary,
                              "files": provider["files"], "originals": []}
        provider = providers[key]
        if provider["capture"]["manifest"] != dependency["capture"]:
            raise ValueError("Retained frontend URL claims conflicting provider captures")
        matches = [item for item in provider["files"] if item.get("url") == url]
        if len(matches) != 1 or any(matches[0].get(k) != row.get(k) for k in ("path", "bytes", "sha256")):
            raise ValueError("Retained frontend dependency has no exact URL and byte identity")
        provider["originals"].append({key: row[key] for key in ("url", "path", "bytes", "sha256")})
    deployment = evidence.frontend_deployment_sources(acquired)
    relevant = [row["original"] for row in deployment["sources"] if row["identity_verified"]]
    capture_path = a.ROOT / ".analysis/results/frontend" / input_dir.name / "analysis.json"
    capture, recovery = None, None
    if capture_path.is_file():
        capture = audit.frontend_capture(input_dir.name, relevant)
        captured = json.loads(audit.artifact(capture_path, capture["manifest"]))
        if captured.get("target_execution") is not False or captured.get("input_bytes_unchanged") is not True:
            raise ValueError("Frontend full capture has no verified static original identity")
        executable = a.ROOT / "tools/analyzers" / ("wakaru-" + captured["tool"]["version"]) / "wakaru.exe"
        for borrowed in captured.get("borrowed_captures", []):
            verify_borrowed_capture(borrowed, executable, audit)
        recovered = captured.get("module_recovery")
        if recovered is not None:
            audit.artifact(a.ROOT / recovered["recipe"]["path"], recovered["recipe"])
            recovery = {key: recovered[key] for key in ("registered_factories", "recovered_modules", "missing_factories", "scan_failures")}
    expected_trees = {row["input"]["sha256"]: row["tree"] for row in (receipt or {}).get("bundles", [])}
    syntax, current_failures = [], list(deployment["failures"])
    for row in files.values():
        if not row.get("path") or Path(urllib.parse.urlsplit(row["url"]).path).suffix.lower() not in {".js", ".mjs"}:
            continue
        raw = audit.read(row)
        actual = evidence.ui_tree(raw)
        tree_path = a.ROOT / ".analysis/results/frontend/ui-trees" / (row["sha256"] + ".json")
        tree_identity = None
        if tree_path.is_file():
            stored = json.loads(audit.artifact(tree_path, expected_trees.get(row["sha256"])))
            if (stored.get("input_sha256") != row["sha256"] or stored.get("input_bytes") != len(raw)
                    or stored.get("target_execution") is not False
                    or any(stored.get(key) != actual[key] for key in ("parse_passed", "owners", "elements", "actions", "failures"))):
                raise ValueError("Retained frontend AST differs from complete original syntax: " + row["path"])
            tree_identity = audit.artifacts[a.rel(tree_path)]
        elif row["sha256"] in expected_trees:
            raise ValueError("Retained frontend AST artifact is absent: " + row["path"])
        syntax.append({"original": {key: row[key] for key in ("path", "bytes", "sha256")},
                       "tree": tree_identity, "parse_passed": actual["parse_passed"],
                       "owners": len(actual["owners"]), "elements": len(actual["elements"]),
                       "complete_syntax_recomputed": True})
        if not actual["parse_passed"]:
            current_failures.append({"kind": "javascript_parse_failed", "input": row["path"]})
    return {"acquisition": audit.artifacts[a.rel(acquired_path)], "acquired": acquired,
            "deployment": deployment, "files": files, "capture": capture, "recovery": recovery,
            "providers": [{key: value for key, value in item.items() if key != "files"}
                          for item in providers.values()],
            "syntax": syntax, "current_failures": current_failures}


def _literal_chunk_expression(node, parameter=None, chunk=None):
    """Interpret only the observed literal filename grammar, never target code."""
    node = unparenthesized(node)
    if node.type == "string":
        text = node.text.decode()
        if not text.startswith('"'):
            raise ValueError("Chunk filename string is outside the verified JSON literal grammar")
        return json.loads(text)
    if node.type == "number":
        return int(node.text)
    if node.type == "identifier" and parameter is not None and node.text.decode() == parameter:
        return chunk
    if node.type == "object":
        result = {}
        for pair in node.named_children:
            if pair.type != "pair":
                raise ValueError("Chunk filename object contains a nonliteral member")
            key = str(_literal_chunk_expression(pair.child_by_field_name("key")))
            if key in result:
                raise ValueError("Chunk filename object repeats a key")
            result[key] = _literal_chunk_expression(pair.child_by_field_name("value"), parameter, chunk)
        return result
    if node.type == "subscript_expression":
        values = _literal_chunk_expression(node.child_by_field_name("object"), parameter, chunk)
        key = _literal_chunk_expression(node.child_by_field_name("index"), parameter, chunk)
        if not isinstance(values, dict):
            raise ValueError("Chunk filename subscript is not a literal table")
        return values.get(str(key))
    if node.type == "binary_expression":
        operator = node.child_by_field_name("operator").text
        if operator not in {b"+", b"||"}:
            raise ValueError("Chunk filename operator is outside the verified literal grammar")
        left = _literal_chunk_expression(node.child_by_field_name("left"), parameter, chunk)
        right = _literal_chunk_expression(node.child_by_field_name("right"), parameter, chunk)
        if operator == b"||":
            return left or right
        if left is None or right is None or isinstance(left, dict) or isinstance(right, dict):
            raise ValueError("Chunk filename table has a missing or invalid literal value")
        return str(left) + str(right) if isinstance(left, str) or isinstance(right, str) else left + right
    raise ValueError("Chunk filename syntax is outside the verified literal grammar: " + node.type)


def _frontend_runtime_closure(source, audit):
    """Verify complete runtime filename owners and each concrete dependency URL."""
    acquired = source["acquired"]
    files = source["files"]
    deployed = {row["url"]: row for row in source.get("deployment", {}).get("sources", [])}
    runtimes, failures, script_status = [], [], {}
    for row in files.values():
        if not row.get("path") or Path(urllib.parse.urlsplit(row["url"]).path).suffix.lower() not in {".js", ".mjs"}:
            continue
        raw = audit.read(row)
        tree = audit.tree(row)
        grouped, nonloaders = {}, []
        for node in nodes(tree.root_node):
            if node.type != "assignment_expression":
                continue
            left = node.child_by_field_name("left")
            if left is None or left.type != "member_expression":
                continue
            receiver, prop = left.child_by_field_name("object"), left.child_by_field_name("property")
            if receiver is None or receiver.type != "identifier" or prop is None or prop.text not in {b"u", b"miniCssF", b"p"}:
                continue
            right = node.child_by_field_name("right")
            if prop.text in {b"u", b"miniCssF"} and right.type not in {"arrow_function", "function_expression"}:
                nonloaders.append({"property": prop.text.decode(), "right_syntax": right.type,
                    "start_byte": node.start_byte, "end_byte": node.end_byte,
                    "sha256": hashlib.sha256(raw[node.start_byte:node.end_byte]).hexdigest()})
                continue
            binding = audit.binding("frontend_runtime_receiver", row, receiver.text.decode(), receiver.start_byte)
            if binding["resolution"] != "declaration":
                if prop.text != b"p":
                    failures.append({"kind": "runtime_receiver_binding_unresolved", "original": row["path"],
                                     "start_byte": node.start_byte})
                continue
            key = (binding["start_byte"], binding["end_byte"])
            group = grouped.setdefault(key, {"binding": binding, "members": {}})
            group["members"].setdefault(prop.text.decode(), []).append(node)
        count = 0
        for group in grouped.values():
            members = group["members"]
            if "u" not in members and "miniCssF" not in members:
                continue
            count += 1
            owners, assets = [], []
            try:
                if any(len(value) != 1 for value in members.values()) or len(members.get("p", [])) != 1:
                    raise ValueError("Runtime filename members or public path are absent or ambiguous")
                public = _literal_chunk_expression(members["p"][0].child_by_field_name("right"))
                if not isinstance(public, str):
                    raise ValueError("Runtime public path is not a literal string")
                for prop, values in members.items():
                    node = values[0]
                    owners.append({"property": prop, "start_byte": node.start_byte, "end_byte": node.end_byte,
                                   "syntax": node.type, "body": raw[node.start_byte:node.end_byte].decode(),
                                   "sha256": hashlib.sha256(raw[node.start_byte:node.end_byte]).hexdigest()})
                    if prop == "p":
                        continue
                    function = node.child_by_field_name("right")
                    parameter = function.child_by_field_name("parameter")
                    if function.type != "arrow_function" or parameter is None or parameter.type != "identifier":
                        raise ValueError("Runtime filename owner has an unverified parameter boundary")
                    body = function.child_by_field_name("body")
                    keys = set()
                    for value in nodes(body):
                        if value.type == "object":
                            table = _literal_chunk_expression(value)
                            keys.update(table)
                    if not keys or any(not key.isdecimal() for key in keys):
                        raise ValueError("Runtime filename owner has no finite numeric literal table")
                    for chunk in sorted(keys, key=int):
                        relative = _literal_chunk_expression(body, parameter.text.decode(), int(chunk))
                        if not isinstance(relative, str):
                            raise ValueError("Runtime filename did not produce a complete literal path")
                        url = urllib.parse.urljoin(urllib.parse.urljoin(acquired["entry_url"], public), relative)
                        original = files.get(url)
                        if original is None or not original.get("path"):
                            failures.append({"kind": "runtime_literal_dependency_missing", "url": url,
                                             "original": row["path"], "property": prop, "chunk": chunk})
                            continue
                        audit.read(original)
                        assets.append({"property": prop, "chunk": chunk, "url": url,
                                       "original": {key: original[key] for key in ("path", "bytes", "sha256")}})
                runtimes.append({"original": {key: row[key] for key in ("path", "bytes", "sha256")},
                                 "current_deployment_reference": row["url"] in deployed,
                                 "deployment_role": deployed.get(row["url"], {}).get("deployment_role", "retained_without_current_reference"),
                                 "runtime_binding": group["binding"], "public_path": public,
                                 "owners": owners, "assets": assets})
            except ValueError as error:
                failures.append({"kind": "runtime_literal_grammar_unresolved", "original": row["path"], "error": str(error)})
        script_status[row["url"]] = {"original": {key: row[key] for key in ("path", "bytes", "sha256")},
                                    "runtime_filename_owners": count, "nonloader_assignments": nonloaders,
                                    "complete_ast_verified": True}
    dispositions = []
    for failure in acquired.get("unresolved", []):
        if failure.get("kind") != "chunk_enumeration_unavailable" or failure.get("url") not in script_status:
            failures.append(failure)
            continue
        dispositions.append({"historical_failure": failure, **script_status[failure["url"]],
            "disposition": "literal_runtime_table_verified" if script_status[failure["url"]]["runtime_filename_owners"]
                           else "complete_ast_has_no_callable_chunk_filename_assignment"})
    missing = [row for row in acquired["files"] if not row.get("path")]
    optional = {row["url"] for row in acquired.get("probes", []) if row.get("declared") is False}
    declared = {row.get("to") for row in acquired.get("edges", [])}
    missing_sources = [row for row in missing if not urllib.parse.urlsplit(row["url"]).path.endswith(".map")
                       and (row["url"] not in optional or row["url"] in declared)]
    failures.extend({"kind": "current_nonsourcemap_original_missing", "url": row["url"]} for row in missing_sources)
    return {"status": "unresolved" if failures else "verified", "runtimes": runtimes,
            "scope": "Complete literal webpack filename tables and referenced originals; dynamic application semantics remain separate",
            "enumeration_dispositions": dispositions, "failures": failures,
            "unavailable_source_maps": sum(urllib.parse.urlsplit(row["url"]).path.endswith(".map") for row in missing),
            "unavailable_undeclared_probes": [row for row in missing if row["url"] in optional and row["url"] not in declared],
            "source_maps_recovered": False, "target_execution": False}


def _version_metadata(original, audit):
    values = {}
    for statement in audit.tree(original).root_node.named_children:
        if statement.type != "export_statement":
            raise ValueError("Changed cache metadata contains nonliteral version logic")
        declarations = [node for node in nodes(statement) if node.type == "variable_declarator"]
        if len(declarations) != 1:
            raise ValueError("Changed cache metadata has an ambiguous export")
        declaration = declarations[0]
        name = declaration.child_by_field_name("name")
        value = declaration.child_by_field_name("value")
        if name.type != "identifier" or value.type != "string" or name.text.decode() in values:
            raise ValueError("Changed cache metadata is not a unique literal export")
        text = value.text.decode()
        if text[:1] not in {'"', "'"} or text[-1:] != text[:1] or "\\" in text:
            raise ValueError("Changed cache metadata string is outside the verified literal grammar")
        values[name.text.decode()] = text[1:-1]
    if set(values) != {"APP_NAME", "APP_VERSION"} or not re.fullmatch(r"[0-9.]+", values["APP_VERSION"]):
        raise ValueError("Changed cache metadata is not the complete application version record")
    return values


def _cache_current_replacement(source, comparison, audit):
    import analysis as a
    supplement_path = a.ROOT / ".analysis/inputs/current-frontend-dependencies/acquisition.json"
    if not supplement_path.is_file():
        return {"status": "unresolved", "failures": [{"kind": "official_cache_dependency_supplement_unavailable"}]}
    supplement = json.loads(audit.artifact(supplement_path))
    if supplement.get("target_execution") is not False or supplement.get("kind") != "official_frontend_dependency_supplement":
        raise ValueError("Cache replacement supplement has an unverified source or execution scope")
    byurl = {row["url"]: row for row in supplement["comparisons"]}
    observed = {row["url"]: row for row in supplement["files"]}
    if len(byurl) != len(supplement["comparisons"]) or len(observed) != len(supplement["files"]):
        raise ValueError("Cache replacement supplement repeats URL identities")
    extra = [row for row in source["acquired"]["files"] if row["url"] not in comparison["files"]]
    verified, failures = [], []
    for row in extra:
        entry = byurl.get(row["url"])
        claims = [] if entry is None else [claim for claim in entry["cache_references"]
            if claim["cache_acquisition"] == source["acquisition"]
            and all(claim["original"][key] == row[key] for key in ("path", "bytes", "sha256"))]
        if len(claims) != 1:
            failures.append({"kind": "cache_extra_url_has_no_exact_supplement_reference", "url": row["url"]})
            continue
        official = entry["retained_official"]
        if official is None:
            current = observed.get(row["url"])
            if current is None or current.get("stable_exact_url") is not True:
                failures.append({"kind": "cache_extra_current_original_unverified", "url": row["url"]})
                continue
            pair = current["http_observations"]
            if len(pair) != 2 or [item.get("phase") for item in pair] != ["before", "after"]:
                raise ValueError("Cache replacement dependency has no two official observation rounds")
            for observation in pair:
                if (observation.get("error") or observation.get("http_status") != 200
                        or observation.get("final_url") != row["url"]
                        or any(observation.get(key) != current.get(key) for key in ("url", "path", "bytes", "sha256"))):
                    raise ValueError("Cache replacement HTTP observations differ from current original bytes")
            provenance = {"acquisition": audit.artifacts[a.rel(supplement_path)], "method": "two_official_exact_url_http_rounds"}
        else:
            provider = json.loads(audit.artifact(a.ROOT / official["acquisition"]["path"], official["acquisition"]))
            if provider.get("target_execution") is not False or not (provider.get("source_current_during_acquisition") is True
                    or provider.get("declared_host_resources_verified") is True):
                raise ValueError("Cache replacement retained provider has no verified official current scope")
            matches = [item for item in provider["files"] if item.get("url") == row["url"]
                       and item.get("path") == official["original"]["path"]]
            if len(matches) != 1 or any(matches[0].get(key) != official["original"][key] for key in ("bytes", "sha256")):
                raise ValueError("Cache replacement retained original has no exact official URL identity")
            current = official["original"]
            provenance = {"acquisition": official["acquisition"], "method": "existing_official_same_url_original"}
        audit.read(current)
        same = all(current[key] == row[key] for key in ("bytes", "sha256"))
        version = None
        if not same:
            if row["url"] != "https://apps.razer.com/synapse/assets/src/version.js":
                failures.append({"kind": "cache_extra_url_changed_semantics_unresolved", "url": row["url"]})
                continue
            previous, actual = _version_metadata(row, audit), _version_metadata(current, audit)
            if previous["APP_NAME"] != actual["APP_NAME"]:
                raise ValueError("Current asset version identity changed its application name")
            version = {"previous": previous, "current": actual, "same_bytes": False,
                       "scope": "Complete two-export asset version metadata; old cache bytes are retained"}
        verified.append({"url": row["url"], "cached_original": {key: row[key] for key in ("path", "bytes", "sha256")},
                         "current_original": {key: current[key] for key in ("path", "bytes", "sha256")},
                         "same_bytes": same, "version_difference": version, "provenance": provenance})
    return {"status": "unresolved" if failures else "verified", "supplement": audit.artifacts[a.rel(supplement_path)],
            "cached_extra_urls": len(extra), "verified_extra_urls": len(verified),
            "same_bytes": sum(row["same_bytes"] for row in verified), "comparisons": verified, "failures": failures}


def recheck_frozen_frontend(task):
    """Recheck one frozen task; scope corrections never erase historical failures."""
    import analysis as a
    import evidence
    from host import HostLocalAudit
    if task.get("kind") != "frontend":
        raise ValueError("Frozen frontend recheck received another task kind")
    input_dir = a.resolve(task["input"], ".analysis/inputs/frontend")
    if input_dir.parent != a.resolve(".analysis/inputs/frontend", ".analysis/inputs"):
        raise ValueError("Frozen frontend task is outside a source-set root")
    audit = HostLocalAudit()
    historical, historical_identity = _frozen_frontend_artifact(task["receipt"], audit)
    old_acquired, old_acquisition_identity = _frozen_frontend_artifact(task["acquisition"], audit)
    if historical.get("acquisition") != task["acquisition"] or historical.get("target_execution") is not False:
        raise ValueError("Frozen frontend task and receipt have different acquisition identities")
    current_path = a.resolve(task["receipt"]["path"], ".analysis/results/reverse-pending")
    current = json.loads(audit.artifact(current_path)) if current_path.is_file() else None
    current_identity = audit.artifacts[a.rel(current_path)] if current is not None else None
    checked = _recheck_frontend_originals(input_dir, audit, current)
    if current is not None and current.get("acquisition") != checked["acquisition"]:
        raise ValueError("Current frontend receipt does not cover its actual acquisition")
    if checked["acquired"].get("entry_url") != old_acquired.get("entry_url"):
        raise ValueError("Frozen frontend recheck cannot replace the original deployment route")
    for row in old_acquired["files"]:
        if row.get("path"):
            replacement = checked["files"].get(row["url"], {})
            if any(replacement.get(key) != row[key] for key in ("bytes", "sha256")):
                raise ValueError("Frozen frontend recheck cannot replace or remove original URL bytes")
    old_deployment = historical["deployment"]
    for original in [old_deployment.get("entry_original"),
                     *(row.get("original") for row in old_deployment.get("manifests", []))]:
        if original is not None:
            audit.read(original)
    classification = (current or {}).get("source_classification") or {}
    scope_report, selected_sources = None, []
    if checked["acquired"].get("source") == ".analysis/inputs/installed-current/acquisition.json":
        official = classification.get("official_source_set")
        if not isinstance(official, str) or not re.fullmatch(r"[a-z0-9-]+", official):
            raise ValueError("Installed frontend cache has no explicit official comparison source")
        scope_report = evidence.installed_frontend_provenance(
            input_dir, a.ROOT / ".analysis/inputs/frontend" / official, audit)
        selected_sources.append(_recheck_frontend_originals(a.ROOT / ".analysis/inputs/frontend" / official, audit))
    elif input_dir.name == "synapse-root-current":
        route = evidence.host_frontend_entry_provenance(audit)
        scope_report = {key: value for key, value in route.items() if key != "owners"}
        scope_report["status"] = "failed_root_probe_is_not_host_entry"
        selected_sources.append(_recheck_frontend_originals(
            a.ROOT / ".analysis/inputs/frontend/synapse-dashboard-current", audit))
    elif input_dir.name == "synapse-chroma-studio-entry-2252a16187ab-current":
        selected = classification.get("current_source_selection") or {}
        candidates = [a.ROOT / row["acquisition"]["path"] for row in selected.get("candidate_decisions", [])]
        if not candidates:
            raise ValueError("Changed Studio acquisition has no explicit current source candidates")
        selection = current_source_selection("https://apps.razer.com/synapse/chroma-studio/", audit, candidates=candidates)
        scope_report = {key: selection[key] for key in ("name", "acquisition", "candidate_decisions", "canonical_entry_url")}
        scope_report["status"] = "historical_acquisition_changed_mid_fetch"
        selected_sources.append(_recheck_frontend_originals(a.ROOT / ".analysis/inputs/frontend" / selection["name"], audit))
    dispositions, remaining = [], []
    for index, failure in enumerate(historical.get("failures", [])):
        kind = failure["kind"]
        url = failure.get("url") or failure.get("source", {}).get("url")
        row = checked["files"].get(url)
        actual_source = next((item for item in checked["deployment"]["sources"] if item["url"] == url), None)
        resolved = (kind in {"deployment_original_unavailable", "source_identity_unverified"}
                    and url is not None and row is not None and row.get("path")
                    and actual_source is not None and actual_source["identity_verified"]
                    and not any(item["kind"] == "javascript_parse_failed" and item.get("input") == row["path"]
                                for item in checked["current_failures"]))
        disposition = {"index": index, "failure": failure,
                       "disposition": "resolved_exact_url_original_verified" if resolved else "historical_failure_preserved",
                       "resolved": bool(resolved)}
        if resolved:
            disposition["original"] = {key: row[key] for key in ("path", "bytes", "sha256")}
            disposition["retained_dependency"] = row.get("retained_dependency")
        elif checked["acquired"].get("source") == ".analysis/inputs/installed-current/acquisition.json":
            disposition["disposition"] = ("deployment_check_does_not_apply_to_cache_snapshot" if url is None
                                           and kind != "current_deployment_stability_unverified"
                                           else "cache_current_deployment_identity_remains_unverified")
            disposition["basis"] = scope_report["cache_acquisition"]
        elif scope_report is not None:
            disposition["basis"] = {key: scope_report[key] for key in ("status", "entry_url", "name", "acquisition") if key in scope_report}
        if not resolved:
            remaining.append(failure)
        dispositions.append(disposition)
    boundaries = [{"kind": "semantics_rust_ui_and_runtime_acceptance_remain_separate"}]
    summaries = []
    for source in [checked, *selected_sources]:
        acquired = source["acquired"]
        summaries.append({key: source[key] for key in ("acquisition", "capture", "recovery", "providers", "syntax", "current_failures")})
        for capture in [source["capture"], *(item["capture"] for item in source["providers"])]:
            if capture is None:
                boundaries.append({"kind": "complete_capture_unavailable", "acquisition": source["acquisition"]})
            elif capture["capture_failures"]:
                boundaries.append({"kind": "historical_capture_stage_failures_preserved",
                                   "capture": capture["manifest"], "failures": capture["capture_failures"]})
        if acquired.get("unresolved") or any(not row.get("path") for row in acquired["files"]):
            boundaries.append({"kind": "source_dependency_closure_remains_unverified", "acquisition": source["acquisition"],
                               "unresolved": len(acquired.get("unresolved", [])),
                               "failed_originals": sum(not row.get("path") for row in acquired["files"])})
    replacement = None
    if selected_sources:
        closures = []
        replacement_failures = []
        for source in selected_sources:
            closure = _frontend_runtime_closure(source, audit)
            closures.append({"acquisition": source["acquisition"], "runtime_closure": closure,
                             "complete_originals_and_ast": not source["current_failures"],
                             "capture": source["capture"], "recovery": source["recovery"]})
            replacement_failures.extend(closure["failures"])
            replacement_failures.extend(source["current_failures"])
            if source["capture"] is None or source["recovery"] is None:
                replacement_failures.append({"kind": "replacement_complete_capture_or_factory_recovery_unavailable",
                                             "acquisition": source["acquisition"]})
            elif source["recovery"]["missing_factories"] or source["recovery"]["scan_failures"]:
                replacement_failures.append({"kind": "replacement_factory_recovery_incomplete", "acquisition": source["acquisition"]})
        cache_coverage = None
        if checked["acquired"].get("source") == ".analysis/inputs/installed-current/acquisition.json":
            cache_coverage = _cache_current_replacement(checked, selected_sources[0], audit)
            replacement_failures.extend(cache_coverage["failures"])
        replacement = {"status": "unresolved" if replacement_failures else "verified",
                       "resolution_method": "verified_current_official_source_replacement",
                       "historical_failures_preserved": True, "original_task_passed": not remaining,
                       "current_source_closures": closures, "cache_extra_coverage": cache_coverage,
                       "failures": replacement_failures, "source_maps_recovered": False,
                       "semantics_complete": False, "runtime_acceptance": "unverified"}
    changed = audit.final_snapshot()
    if changed:
        raise ValueError("Frontend evidence changed during frozen recheck: " + ", ".join(changed))
    return {"status": "unresolved" if remaining else "passed", "task_id": task["id"],
            "historical_receipt": historical_identity, "historical_acquisition": old_acquisition_identity,
            "current_receipt": current_identity, "current_acquisition": checked["acquisition"],
            "acquisition_changed_since_frozen_task": checked["acquisition"]["sha256"] != task["acquisition"]["sha256"],
            "source_scope": scope_report, "source_verification": summaries,
            "failure_dispositions": dispositions, "failures": remaining,
            "replacement_resolution": replacement,
            "current_failures": checked["current_failures"], "remaining_boundaries": boundaries,
            "verified_artifacts": len(audit.artifacts), "changed_during_query": changed,
            "verification_only": True, "target_execution": False, "semantics_complete": False,
            "mount_tree_complete": False, "rust_implementation_changed": False,
            "scratch_cleanup": {"temporary_paths_created": [], "deleted_paths": [], "released_bytes": 0}}


def _frozen_frontend_cache_plan():
    """Join installed-cache extras to existing official URL identities."""
    import analysis as a
    master_path = a.ROOT / ".analysis/results/reverse-pending.json"
    master_raw = master_path.read_bytes()
    master = json.loads(master_raw)
    references, source_receipts = {}, {}
    for task in master["tasks"]:
        if task["kind"] != "frontend" or not Path(task["input"]).name.startswith("installed-"):
            continue
        acquired_path = a.resolve(task["input"], ".analysis/inputs/frontend") / "acquisition.json"
        cache_raw = acquired_path.read_bytes()
        cached = json.loads(cache_raw)
        if cached.get("source") != ".analysis/inputs/installed-current/acquisition.json":
            continue
        receipt_path = a.resolve(task["receipt"]["path"], ".analysis/results/reverse-pending")
        receipt_raw = receipt_path.read_bytes()
        classification = json.loads(receipt_raw).get("source_classification") or {}
        official = classification.get("official_source_set")
        if not isinstance(official, str) or not re.fullmatch(r"[a-z0-9-]+", official):
            raise ValueError("Frozen installed source has no explicit official comparison")
        counterpart_path = a.ROOT / ".analysis/inputs/frontend" / official / "acquisition.json"
        counterpart_raw = counterpart_path.read_bytes()
        counterpart = json.loads(counterpart_raw)
        known = {row["url"] for row in counterpart["files"]}
        for path, raw in ((acquired_path, cache_raw), (receipt_path, receipt_raw), (counterpart_path, counterpart_raw)):
            source_receipts[a.rel(path)] = {"path": a.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        for row in cached["files"]:
            if row["url"] in known:
                continue
            parsed = urllib.parse.urlsplit(row["url"])
            if parsed.scheme != "https" or parsed.netloc != "apps.razer.com" or parsed.query or parsed.fragment:
                raise ValueError("Frozen cached dependency is outside the exact official URL boundary")
            references.setdefault(row["url"], []).append({"task_id": task["id"],
                "cache_acquisition": source_receipts[a.rel(acquired_path)],
                "original": {key: row[key] for key in ("path", "bytes", "sha256")}})
    matches = {url: [] for url in references}
    for path in sorted((a.ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")):
        raw = path.read_bytes()
        acquired = json.loads(raw)
        if acquired.get("source_current_during_acquisition") is not True or acquired.get("target_execution") is not False:
            continue
        candidates = [row for row in acquired.get("files", []) if row.get("url") in references and row.get("path")]
        if not candidates:
            continue
        identity = {"path": a.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        for row in candidates:
            matches[row["url"]].append({"acquisition": identity, "recorded_at_utc": acquired.get("recorded_at_utc"),
                "original": {key: row[key] for key in ("path", "bytes", "sha256")}})
    host_path = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    host_raw = host_path.read_bytes()
    host = json.loads(host_raw)
    if host.get("target_execution") is False and host.get("declared_host_resources_verified") is True:
        for row in host["files"]:
            if row.get("url") in references and row.get("path"):
                matches[row["url"]].append({"acquisition": {"path": a.rel(host_path), "bytes": len(host_raw),
                    "sha256": hashlib.sha256(host_raw).hexdigest()}, "recorded_at_utc": host.get("finished_at_utc"),
                    "original": {key: row[key] for key in ("path", "bytes", "sha256")}})
    comparisons = []
    for url, rows in sorted(references.items()):
        candidates = matches[url]
        same = [item for item in candidates if any(all(item["original"][key] == row["original"][key]
                for key in ("bytes", "sha256")) for row in rows)]
        chosen = max(same, key=lambda item: (item["recorded_at_utc"] or "", item["acquisition"]["path"])) if same else None
        comparisons.append({"url": url, "cache_references": rows, "retained_official": chosen,
                            "other_official_identities": candidates if not same else []})
    return {"frozen_tasks": {"path": a.rel(master_path), "bytes": len(master_raw), "sha256": hashlib.sha256(master_raw).hexdigest()},
            "source_receipts": list(source_receipts.values()), "comparisons": comparisons}


def acquire_frozen_frontend_sources(args):
    """Acquire only unmatched exact official URLs; keep failed and changed bytes."""
    import analysis as a
    from host import HostLocalAudit
    workers = getattr(args, "workers", 8)
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Frozen frontend acquisition workers must be between two and sixteen")
    plan = _frozen_frontend_cache_plan()
    audit = HostLocalAudit()
    for row in [plan["frozen_tasks"], *plan["source_receipts"]]:
        audit.artifact(a.ROOT / row["path"], row)
    for item in plan["comparisons"]:
        for reference in item["cache_references"]:
            audit.read(reference["original"])
        current = item["retained_official"]
        if current:
            audit.artifact(a.ROOT / current["acquisition"]["path"], current["acquisition"])
            audit.read(current["original"])
    output = a.ROOT / ".analysis/inputs/current-frontend-dependencies"
    receipt_path = output / "acquisition.json"
    previous = json.loads(receipt_path.read_bytes()) if receipt_path.is_file() else None
    previous_rows = {row["url"]: row for row in previous.get("files", [])} if previous else {}
    wanted = [item for item in plan["comparisons"] if item["retained_official"] is None]
    retained = {row["original"]["sha256"]: row["original"] for item in plan["comparisons"] for row in item["cache_references"]}
    headers = {"User-Agent": "razer-ui-static-evidence", "Accept-Encoding": "identity",
               "Cache-Control": "no-cache", "Pragma": "no-cache"}

    class OfficialRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, request, response, code, message, response_headers, new_url):
            parsed = urllib.parse.urlsplit(new_url)
            if parsed.scheme != "https" or parsed.netloc != "apps.razer.com":
                raise ValueError("Frontend dependency redirected outside the official origin")
            return super().redirect_request(request, response, code, message, response_headers, new_url)

    def fetch(url, phase):
        observation = {"url": url, "phase": phase, "request_headers": headers, "started_at_utc": a.stamp()}
        try:
            opener = urllib.request.build_opener(OfficialRedirect)
            with opener.open(urllib.request.Request(url, headers=headers), timeout=35) as response:
                raw = response.read()
                observation.update(final_url=response.geturl(), http_status=response.status, headers=dict(response.headers),
                    content_type=response.headers.get("Content-Type"), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
            if observation["http_status"] != 200 or observation["final_url"] != url:
                raise ValueError("Exact frontend dependency URL did not return an unchanged official 200 endpoint")
            known = retained.get(observation["sha256"])
            if known is not None and known["bytes"] == len(raw):
                if audit.read(known) != raw:
                    raise ValueError("Official response differs from retained bytes with the same identity")
                observation["path"] = known["path"]
                observation["retained_original_reused"] = True
            else:
                filename = Path(urllib.parse.urlsplit(url).path).name
                if not filename or re.search(r'[\\:*?"<>|]', filename):
                    raise ValueError("Frontend dependency has no safe original filename")
                path = output / "blobs" / observation["sha256"] / filename
                path.parent.mkdir(parents=True, exist_ok=True)
                if path.is_file() and path.read_bytes() != raw:
                    raise ValueError("Frontend dependency content-addressed original differs")
                if not path.exists():
                    path.write_bytes(raw)
                observation["path"] = a.rel(path)
                observation["retained_original_reused"] = False
        except Exception as error:
            observation.update(error=str(error).replace(str(a.ROOT), "<project>"), error_type=type(error).__name__)
            if isinstance(error, urllib.error.HTTPError):
                observation.update(http_status=error.code, final_url=error.geturl(), headers=dict(error.headers or {}))
                error.close()
        observation["finished_at_utc"] = a.stamp()
        return observation

    files, active = [], []
    for item in wanted:
        prior = previous_rows.get(item["url"])
        if prior and prior.get("stable_exact_url") is True:
            for observation in prior["http_observations"]:
                if any(observation.get(key) != prior.get(key) for key in ("url", "path", "bytes", "sha256")):
                    raise ValueError("Previous frontend dependency HTTP identity differs from its original")
            audit.read(prior)
            files.append(prior)
        else:
            active.append(item)
    observations = {}
    for phase in ("before", "after"):
        with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
            for row in pool.map(lambda item: fetch(item["url"], phase), active):
                observations.setdefault(row["url"], []).append(row)
        print(json.dumps({"phase": phase, "completed": len(active), "workers": workers}), flush=True)
    for item in active:
        pair = observations[item["url"]]
        successes = [row for row in pair if "path" in row and "error" not in row]
        stable = len(successes) == 2 and all(successes[0][key] == successes[1][key] for key in ("url", "path", "bytes", "sha256"))
        chosen = successes[-1] if successes else pair[-1]
        row = {key: value for key, value in chosen.items() if key not in {"phase", "started_at_utc", "finished_at_utc"}}
        row.update(http_observations=pair, stable_exact_url=stable)
        if item["url"] in previous_rows:
            row["previous_http_observations"] = previous_rows[item["url"]].get("previous_http_observations", []) + previous_rows[item["url"]]["http_observations"]
        files.append(row)
    changed = audit.final_snapshot()
    if changed:
        raise ValueError("Frontend source identities changed during dependency acquisition: " + ", ".join(changed))
    output.mkdir(parents=True, exist_ok=True)
    previous_identity = retain_capture(receipt_path) if previous else None
    report = {"schema_version": 1, "kind": "official_frontend_dependency_supplement", "recorded_at_utc": a.stamp(),
              **plan, "files": sorted(files, key=lambda row: row["url"]),
              "previous_receipt": previous_identity, "target_execution": False,
              "scope": "Exact installed-cache extra URLs; independent official observations, not a live app deployment",
              "semantics_complete": False, "runtime_acceptance": "unverified",
              "cleanup": {"temporary_paths_created": [], "duplicate_captures_created": [], "deleted_paths": [], "released_bytes": 0}}
    a.save(receipt_path, report)
    indexed = {row["path"]: {key: row[key] for key in ("path", "bytes", "sha256", "url")}
               for row in files if row.get("path", "").startswith(a.rel(output) + "/")}
    a.index_files([{**row, "kind": "official_frontend_dependency_original"} for row in indexed.values()] + [
        {"path": a.rel(receipt_path), "bytes": receipt_path.stat().st_size, "sha256": a.sha(receipt_path),
         "kind": "official_frontend_dependency_acquisition"}])
    print(json.dumps({"receipt": a.rel(receipt_path), "unique_urls": len(plan["comparisons"]),
        "existing_official_reused": len(plan["comparisons"]) - len(wanted), "requested_urls": len(wanted),
        "stable_exact_urls": sum(row["stable_exact_url"] for row in files),
        "failed_or_unstable": sum(not row["stable_exact_url"] for row in files)}, ensure_ascii=False), flush=True)
    return report


def input_root(input_dir, inputs):
    """Find a retained input root for ordinary and cross-product source sets."""
    import analysis as a
    boundary = (a.ROOT / ".analysis/inputs/frontend").resolve()
    paths = [path.resolve() for path in inputs]
    if not paths or any(not path.is_file() or not path.is_relative_to(boundary) for path in paths):
        raise ValueError("Frontend inputs are missing or outside retained frontend originals")
    preferred = (input_dir / "raw").resolve()
    if preferred.is_dir() and all(path.is_relative_to(preferred) for path in paths):
        return preferred
    common = Path(os.path.commonpath([path.parent for path in paths]))
    if not common.is_dir() or not common.is_relative_to(boundary):
        raise ValueError("Frontend input common directory escaped retained originals")
    return common


def inline_script_equivalence(left, right, parser):
    """Compare complete syntax trees, including one safe typeof rewrite."""
    trees = [parser.parse(raw) for raw in (left, right)]
    if any(tree.root_node.has_error for tree in trees):
        return {"equivalent": False, "reason": "unresolved_javascript_syntax"}
    rewrites = [0, 0]

    def literal_typeof_comparison(node):
        if node.type != "binary_expression":
            return False
        operands = [unparenthesized(node.child_by_field_name(name)) for name in ("left", "right")]
        return any(value is not None and value.type == "unary_expression"
                   and value.child_by_field_name("operator") is not None
                   and value.child_by_field_name("operator").text == b"typeof"
                   and other is not None and other.type == "string"
                   for value, other in (operands, operands[::-1]))

    def signature(node, index):
        if not node.children:
            return node.type, node.text
        children = []
        safe = literal_typeof_comparison(node)
        for child_index, child in enumerate(node.children):
            if child.type == "comment" or node.type == "expression_statement" and child.type == ";":
                continue
            field = node.field_name_for_child(child_index)
            if safe and field == "operator" and child.text in {b"==", b"===", b"!=", b"!=="}:
                token = b"==" if child.text in {b"==", b"==="} else b"!="
                children.append((field, ("typeof_string_equality", token)))
                rewrites[index] += 1
            else:
                children.append((field, signature(child, index)))
        return node.type, tuple(children)

    equivalent = signature(trees[0].root_node, 0) == signature(trees[1].root_node, 1)
    return {"equivalent": equivalent,
        "comparison": "complete_ordered_javascript_syntax_tree",
        "allowed_normalizations": ["comments", "expression_statement_semicolon", "typeof_string_equality_strictness"],
        "typeof_string_comparisons": rewrites,
        "reason": "same_complete_syntax" if equivalent else "different_complete_syntax"}


def syntax_identity(root):
    """Hash the complete ordered syntax, retaining operators and literal bytes."""
    digests, pending = {}, [(root, False)]
    while pending:
        node, visited = pending.pop()
        if not visited:
            pending.append((node, True))
            pending.extend((child, False) for child in reversed(node.children) if child.type != "comment")
            continue
        digest = hashlib.sha256(node.type.encode() + b"\0")
        if node.children:
            for index, child in enumerate(node.children):
                if child.type == "comment":
                    continue
                digest.update((node.field_name_for_child(index) or "").encode() + b"\0")
                digest.update(digests[(child.start_byte, child.end_byte, child.type)])
        else:
            digest.update(node.text)
        digests[(node.start_byte, node.end_byte, node.type)] = digest.digest()
    return digests[(root.start_byte, root.end_byte, root.type)].hex()


def readable_lexical_script(raw, parser):
    """Add only line breaks and prove the complete original syntax is retained."""
    tree = parser.parse(raw)
    if tree.root_node.has_error:
        raise ValueError("Lexical context original has unresolved JavaScript syntax")
    offsets = set()
    for node in nodes(tree.root_node):
        if node.type == "statement_block":
            offsets.update((node.start_byte + 1, node.end_byte - 1))
        elif node.type.endswith("_statement") or node.type in {"variable_declaration", "lexical_declaration"}:
            offsets.add(node.end_byte)
    offsets = sorted(offset for offset in offsets if 0 < offset < len(raw)
                     and raw[offset:offset + 1] not in {b"\r", b"\n"})
    pieces, original_start = [], 0
    for end in [*offsets, len(raw)]:
        piece = raw[original_start:end]
        pieces.append(piece)
        if end != len(raw):
            pieces.append(b"\n")
        original_start = end
    readable = b"".join(pieces)
    parsed = parser.parse(readable)
    identity = syntax_identity(tree.root_node)
    if parsed.root_node.has_error or syntax_identity(parsed.root_node) != identity:
        raise ValueError("Readable lexical context changed the complete original syntax")
    return readable, {"comparison": "complete_ordered_javascript_syntax_tree", "syntax_sha256": identity,
                      "only_added_bytes": "LF", "inserted_lf_original_offsets": offsets}


def lexical_context_findings(captured, provenance, validation, input_dir, audit):
    """Bind each failed derived module to its complete original lexical owner."""
    import analysis as a
    originals = {row["path"]: row for row in captured["original_inputs"]}
    grouped = {}
    for finding in validation["findings"]:
        filename = finding["filename"]
        if (".." in Path(filename).parts or Path(filename).is_absolute()
                or finding.get("kind") not in {"esm_commonjs_residual", "unresolved_reference"}):
            raise ValueError("Unsupported derived-module validation finding requires analysis")
        owner = provenance["modules"][filename]
        source_path = (input_dir / "raw" / owner["input"].replace("\\", "/")).resolve()
        source = originals.get(a.rel(source_path))
        if source is None:
            raise ValueError("Failed derived module has no verified original owner")
        raw = audit.read(source)
        ranges = owner["ranges"]
        if (not ranges or any(not isinstance(span, list) or len(span) != 2
                or not 0 <= span[0] < span[1] <= len(raw) for span in ranges)):
            raise ValueError("Failed derived module has invalid original ranges")
        target = a.ROOT / ".analysis/results/frontend" / input_dir.name / "modules" / filename
        output = next((row for row in captured["outputs"] if row["path"] == a.rel(target)), None)
        if output is None:
            raise ValueError("Failed derived module is absent from captured output identities")
        derived = audit.artifact(target, output).decode("utf-8")
        lines = re.split(r"\r\n|[\r\n\u2028\u2029]", derived)
        line, column = finding["line"], finding["column"]
        if not 1 <= line <= len(lines) or not 1 <= column <= len(lines[line - 1].encode("utf-16-le")) // 2 + 1:
            raise ValueError("Validation finding position is outside its captured module")
        syntax = audit.tree(source).root_node
        contexts = []
        for start, end in ranges:
            ancestor = syntax.descendant_for_byte_range(start, end - 1)
            while ancestor is not None and ancestor.type not in {"function_expression", "arrow_function", "program"}:
                ancestor = ancestor.parent
            if ancestor is None:
                raise ValueError("Failed derived module lost its complete lexical owner")
            parameters = ancestor.child_by_field_name("parameters")
            contexts.append({"range": [ancestor.start_byte, ancestor.end_byte], "syntax": ancestor.type,
                             "parameters": [node.text.decode("utf-8") for node in parameters.named_children]
                             if parameters is not None else []})
        record = grouped.setdefault(source["path"], {"original": source, "failed_modules": {}})
        module = record["failed_modules"].setdefault(filename, {"derived_original_ranges": ranges,
            "complete_lexical_owners": contexts, "diagnostics": []})
        module["diagnostics"].append(finding)
    return list(grouped.values())


def recover_validation_context(input_dir, audit):
    """Recover readable whole-script owners while retaining the failed ESM result."""
    import analysis as a
    output = a.ROOT / ".analysis/results/frontend" / input_dir.name
    receipt_path = output / "analysis.json"
    captured = json.loads(audit.artifact(receipt_path))
    for row in captured["original_inputs"]:
        audit.read(row)
    previous = captured.get("lexical_context_recovery")
    if previous is not None:
        for row in captured["outputs"]:
            audit.artifact(a.ROOT / row["path"], row)
        compacted = False
        for script in previous["scripts"]:
            raw = audit.read(script["original"])
            expected, proof = readable_lexical_script(raw, audit.parser)
            readable = audit.artifact(a.ROOT / script["readable_script"]["path"], script["readable_script"])
            if readable != expected or script["proof"]["syntax_sha256"] != proof["syntax_sha256"]:
                raise ValueError("Retained readable lexical context differs from its complete original")
            if "byte_segments" in script["proof"]:
                restored = b"".join(readable[row["readable_range"][0]:row["readable_range"][1]]
                                    for row in script["proof"]["byte_segments"])
                if restored != raw:
                    raise ValueError("Retained lexical byte-segment proof does not reconstruct the original")
                script["proof"] = proof
                compacted = True
            elif script["proof"] != proof:
                raise ValueError("Retained lexical context LF offsets differ from the verified syntax")
        if audit.final_snapshot():
            raise ValueError("Frontend evidence changed while verifying lexical contexts")
        if compacted:
            previous["previous_receipt"] = retain_capture(receipt_path)
            recipe_raw = Path(__file__).read_bytes()
            recipe_path = output / "lexical-contexts" / (hashlib.sha256(recipe_raw).hexdigest() + ".py")
            if recipe_path.exists() and recipe_path.read_bytes() != recipe_raw:
                raise ValueError("Lexical recovery recipe identity collision")
            recipe_path.write_bytes(recipe_raw)
            recipe = {"path": a.rel(recipe_path), "bytes": len(recipe_raw), "sha256": hashlib.sha256(recipe_raw).hexdigest()}
            previous["recipe"] = recipe
            captured["outputs"].append(recipe)
            captured["recorded_at_utc"] = a.stamp()
            a.save(receipt_path, captured)
        return {"status": "reused_verified_lexical_contexts", "diagnostics_covered": previous["diagnostics_covered"],
                "originals_recovered": len(previous["scripts"]), "proof_compacted_losslessly": compacted}
    failed = [row for row in captured["commands"] if row.get("exit_code")]
    if not failed:
        return {"status": "no_failed_derived_validation"}
    if any(row["stage"] != "validate" for row in failed):
        raise ValueError("Lexical recovery cannot resolve failed capture commands")
    for row in captured["outputs"]:
        audit.artifact(a.ROOT / row["path"], row)
    provenance_path = output / "modules/provenance.json"
    provenance = json.loads(audit.artifact(provenance_path))
    scripts, validations = [], []
    for command in failed:
        validation_path = a.ROOT / command["stdout"]
        validation = json.loads(audit.artifact(validation_path))
        if not validation["findings"]:
            raise ValueError("Failed derived validation has no retained diagnostic findings")
        validations.append(audit.artifacts[a.rel(validation_path)])
        scripts.extend(lexical_context_findings(captured, provenance, validation, input_dir, audit))
    if len({row["original"]["path"] for row in scripts}) != len(scripts):
        raise ValueError("Repeated validation owners require an explicit unified diagnostic set")
    generated = []
    for script in scripts:
        raw = audit.read(script["original"])
        readable, proof = readable_lexical_script(raw, audit.parser)
        target = output / "lexical-contexts" / (script["original"]["sha256"] + ".js")
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists() and target.read_bytes() != readable:
            raise ValueError("Lexical-context output identity collision")
        target.write_bytes(readable)
        generated.append({"path": a.rel(target), "bytes": len(readable), "sha256": hashlib.sha256(readable).hexdigest()})
        script.update(readable_script=generated[-1], proof=proof,
                      runtime_and_outer_bindings_preserved=True,
                      scope="static_semantic_analysis_only; failed_ESM_is_not_executable")
    if audit.final_snapshot():
        raise ValueError("Frontend evidence changed during lexical context recovery")
    predecessor = retain_capture(receipt_path)
    recipe_raw = Path(__file__).read_bytes()
    recipe_path = output / "lexical-contexts" / (hashlib.sha256(recipe_raw).hexdigest() + ".py")
    recipe_path.parent.mkdir(parents=True, exist_ok=True)
    if recipe_path.exists() and recipe_path.read_bytes() != recipe_raw:
        raise ValueError("Lexical recovery recipe identity collision")
    recipe_path.write_bytes(recipe_raw)
    recipe = {"path": a.rel(recipe_path), "bytes": len(recipe_raw), "sha256": hashlib.sha256(recipe_raw).hexdigest()}
    captured["lexical_context_recovery"] = {"strategy": "whole_original_script_with_complete_syntax_identity",
        "previous_receipt": predecessor, "validation_results": validations, "recipe": recipe,
        "scripts": scripts, "diagnostics_covered": sum(len(module["diagnostics"]) for script in scripts
            for module in script["failed_modules"].values()),
        "derived_esm_validation_passed": False, "target_execution": False}
    captured["outputs"].extend([*generated, recipe])
    captured["recorded_at_utc"] = a.stamp()
    a.save(receipt_path, captured)
    return {"status": "recovered_complete_lexical_contexts", "originals_recovered": len(scripts),
            "diagnostics_covered": captured["lexical_context_recovery"]["diagnostics_covered"],
            "derived_esm_validation_passed": False}


def retain_capture(receipt_path):
    """Retain the exact predecessor of a repaired, still referenced capture."""
    import analysis as a
    raw = receipt_path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    retained = a.ROOT / ".analysis/results/reverse-pending/review-sources" / (digest + ".json")
    retained.parent.mkdir(parents=True, exist_ok=True)
    if retained.exists():
        if retained.read_bytes() != raw:
            raise ValueError("Retained frontend capture identity collision")
    else:
        retained.write_bytes(raw)
    return {"path": a.rel(retained), "bytes": len(raw), "sha256": digest}


def verify_borrowed_capture(record, tool, audit):
    """Verify a direct complete provider, without following provider chains."""
    import analysis as a
    provider = record["capture"]
    acquired = json.loads(audit.artifact(a.ROOT / record["acquisition"]["path"], record["acquisition"]))
    captured = json.loads(audit.artifact(a.ROOT / provider["path"], provider))
    if (not provider["path"].startswith(".analysis/results/frontend/")
            or not record["acquisition"]["path"].startswith(".analysis/inputs/frontend/")):
        raise ValueError("Borrowed frontend capture is outside retained frontend evidence")
    source_dir = (a.ROOT / record["acquisition"]["path"]).parent
    expected_capture = a.ROOT / ".analysis/results/frontend" / source_dir.name / "analysis.json"
    if ((a.ROOT / provider["path"]).resolve() != expected_capture.resolve()
            or captured.get("input") != a.rel(source_dir)):
        raise ValueError("Borrowed frontend acquisition and capture have different owners")
    if (acquired.get("source_current_during_acquisition") is not True
            or captured.get("input_bytes_unchanged") is not True
            or captured.get("tool", {}).get("sha256") != a.sha(tool)):
        raise ValueError("Borrowed frontend capture has an unverified current scope or analyzer")
    if captured.get("borrowed_captures"):
        raise ValueError("Nested or cyclic borrowed frontend captures are not allowed")
    recovery = captured.get("module_recovery")
    if not isinstance(recovery, dict) or recovery.get("missing_factories") != [] or recovery.get("scan_failures") != []:
        raise ValueError("Borrowed frontend provider has incomplete factory recovery")
    originals = {row["path"]: {key: row[key] for key in ("path", "bytes", "sha256")}
                 for row in acquired["files"] if row.get("path")}
    if originals != {row["path"]: row for row in captured["original_inputs"]}:
        raise ValueError("Borrowed frontend capture does not cover its current acquisition")
    for original in record["originals"]:
        if originals.get(original["path"]) != original or original["path"] not in captured["original_js_inputs"]:
            raise ValueError("Borrowed frontend script is not covered by the provider capture")
    for original in originals.values():
        audit.read(original)
    for artifact in captured["outputs"]:
        audit.artifact(a.ROOT / artifact["path"], artifact)
    audit.artifact(a.ROOT / recovery["recipe"]["path"], recovery["recipe"])
    return {row["path"] for row in record["originals"]}


def borrow_script_captures(input_dir, scripts, tool, audit):
    """Reuse an original's complete capture in its actual source-set owner."""
    import analysis as a
    grouped, direct = {}, []
    for row in scripts:
        parts = Path(row["path"]).parts
        if len(parts) < 5 or parts[:3] != (".analysis", "inputs", "frontend") or parts[3] == input_dir.name:
            direct.append(row)
            continue
        source_dir = a.ROOT / ".analysis/inputs/frontend" / parts[3]
        capture = a.ROOT / ".analysis/results/frontend" / parts[3] / "analysis.json"
        acquisition = source_dir / "acquisition.json"
        if not capture.is_file() or not acquisition.is_file():
            direct.append(row)
            continue
        grouped.setdefault(parts[3], {"capture": {"path": a.rel(capture), "bytes": capture.stat().st_size, "sha256": a.sha(capture)},
            "acquisition": {"path": a.rel(acquisition), "bytes": acquisition.stat().st_size, "sha256": a.sha(acquisition)},
            "originals": []})["originals"].append(row)
    records = [grouped[name] for name in sorted(grouped)]
    for record in records:
        verify_borrowed_capture(record, tool, audit)
    return direct, records


def extend_capture(input_dir, tool, audit):
    """Capture only added originals; every reused output must still match."""
    import analysis as a
    output = a.ROOT / ".analysis/results/frontend" / input_dir.name
    receipt_path = output / "analysis.json"
    acquired = json.loads(audit.artifact(input_dir / "acquisition.json"))
    captured = json.loads(receipt_path.read_bytes())
    originals = {}
    for row in acquired["files"]:
        if not row.get("path"):
            continue
        identity = {key: row[key] for key in ("path", "bytes", "sha256")}
        if row["path"] in originals and originals[row["path"]] != identity:
            raise ValueError("Conflicting acquired frontend original identities")
        originals[row["path"]] = identity
    previous = {row["path"]: row for row in captured["original_inputs"]}
    if any(originals.get(name) != row for name, row in previous.items()):
        raise ValueError("Incremental recovery cannot replace or remove original input bytes")
    additions = [originals[name] for name in sorted(originals.keys() - previous.keys())]
    known_scripts = set(captured["original_js_inputs"])
    scripts = [originals[name] for name in sorted(originals)
               if Path(name).suffix.lower() in {".js", ".mjs"} and name not in known_scripts]
    if not additions and not scripts:
        return False
    if captured["tool"]["sha256"] != a.sha(tool):
        raise ValueError("Incremental recovery analyzer differs from the complete capture")
    for row in originals.values():
        audit.read(row)
    for row in captured["outputs"]:
        audit.artifact(a.ROOT / row["path"], row)
    for record in captured.get("borrowed_captures", []):
        verify_borrowed_capture(record, tool, audit)
    predecessor = retain_capture(receipt_path)
    new_script_inputs = {row["path"] for row in scripts}
    scripts, borrowed = borrow_script_captures(input_dir, scripts, tool, audit)
    commands, generated = [], set()
    provenance_path = output / "modules/provenance.json"
    if scripts:
        provenance = json.loads(provenance_path.read_bytes())
        signature = hashlib.sha256(json.dumps(scripts, sort_keys=True).encode()).hexdigest()
        increment = output / "modules/increments" / signature
        if increment.exists():
            raise ValueError("Uncommitted frontend increment already exists; inspect retained diagnostics")
        increment.parent.mkdir(parents=True, exist_ok=True)
        cwd = input_root(input_dir, [a.ROOT / row["path"] for row in scripts])
        argv = [str(tool), *(os.path.relpath(a.ROOT / row["path"], cwd) for row in scripts),
                "--unpack", "--level", "minimal", "--provenance", "--emit-source-map",
                "--diagnostics", "--json", "-o", str(increment)]
        stdout, stderr = output / ("increment-" + signature + ".stdout.txt"), output / ("increment-" + signature + ".stderr.txt")
        with stdout.open("wb") as out, stderr.open("wb") as err:
            completed = subprocess.run(argv, cwd=cwd, stdout=out, stderr=err, check=False)
        commands.append({"stage": "increment-" + signature,
            "argv": [a.rel(Path(arg)) if Path(arg).is_absolute() and Path(arg).is_relative_to(a.ROOT) else arg for arg in argv],
            "cwd": a.rel(cwd), "exit_code": completed.returncode, "stdout": a.rel(stdout), "stderr": a.rel(stderr)})
        if completed.returncode or not (increment / "provenance.json").is_file():
            raise ValueError("Added frontend scripts were not completely captured; diagnostics retained")
        extra = json.loads((increment / "provenance.json").read_bytes())
        covered = set()
        for name, owner in extra["modules"].items():
            if not owner.get("input"):
                raise ValueError("Incremental module has no original owner")
            source = (cwd / owner["input"].replace("\\", "/")).resolve()
            if source not in {a.ROOT / row["path"] for row in scripts}:
                raise ValueError("Incremental module points outside the added originals")
            covered.add(source)
            key = "increments/" + signature + "/" + name
            if key in provenance["modules"]:
                raise ValueError("Incremental frontend module identity collision")
            provenance["modules"][key] = {**owner, "input": os.path.relpath(source, input_dir / "raw").replace("\\", "/")}
        if covered != {a.ROOT / row["path"] for row in scripts}:
            raise ValueError("Incremental analyzer omitted an added frontend script")
        if audit.final_snapshot():
            raise ValueError("Original frontend capture changed during incremental analysis")
        a.save(provenance_path, provenance)
        generated.update(path for path in increment.rglob("*") if path.is_file())
        generated.update((stdout, stderr, provenance_path))
        if captured.get("module_recovery") is not None:
            captured["pending_previous_module_recovery"] = captured.pop("module_recovery")
    changed = set(audit.final_snapshot()) - ({a.rel(provenance_path)} if scripts else set())
    if changed:
        raise ValueError("Original frontend capture changed before incremental publication: " + ", ".join(sorted(changed)))
    captured["original_inputs"] = [originals[name] for name in sorted(originals)]
    captured["original_js_inputs"] = sorted(set(captured["original_js_inputs"]) | new_script_inputs)
    for record in borrowed:
        existing = next((item for item in captured.get("borrowed_captures", []) if item["capture"] == record["capture"]), None)
        if existing is None:
            captured.setdefault("borrowed_captures", []).append(record)
        else:
            existing["originals"].extend(record["originals"])
    captured["commands"].extend(commands)
    captured.setdefault("incremental_captures", []).append({"previous_receipt": predecessor,
        "added_originals": additions, "analyzed_scripts": [row["path"] for row in scripts],
        "borrowed_scripts": [row for record in borrowed for row in record["originals"]],
        "original_acquisition_sha256": a.sha(input_dir / "acquisition.json"), "target_execution": False})
    outputs = {row["path"]: row for row in captured["outputs"]}
    for path in generated:
        outputs[a.rel(path)] = {"path": a.rel(path), "bytes": path.stat().st_size, "sha256": a.sha(path)}
    captured["outputs"] = [outputs[name] for name in sorted(outputs)]
    captured["recorded_at_utc"] = a.stamp()
    a.save(receipt_path, captured)
    return True


def decode_vlq(segment):
    values, value, shift = [], 0, 0
    for char in segment:
        digit = BASE64.find(char)
        if digit < 0 or shift > 60:
            raise ValueError("Invalid source-map VLQ")
        value |= (digit & 31) << shift
        if digit & 32:
            shift += 5
        else:
            values.append(-(value >> 1) if value & 1 else value >> 1)
            value, shift = 0, 0
    if shift:
        raise ValueError("Truncated source-map VLQ")
    return values


def encode_vlq(values):
    result = []
    for value in values:
        value = (-value << 1 | 1) if value < 0 else value << 1
        while value >= 32:
            result.append(BASE64[(value & 31) | 32])
            value >>= 5
        result.append(BASE64[value])
    return "".join(result)


def source_position(raw, byte_offset):
    """Source-map lines/columns are zero-based UTF-16, not UTF-8 bytes."""
    if not 0 <= byte_offset <= len(raw):
        raise ValueError("Source-map offset is outside the original")
    prefix = raw[:byte_offset].decode("utf-8")
    lines = re.split(r"\r\n|[\r\n\u2028\u2029]", prefix)
    return len(lines) - 1, len(lines[-1].encode("utf-16-le")) // 2






def nodes(root):
    pending = [root]
    while pending:
        node = pending.pop()
        yield node
        pending.extend(reversed(node.named_children))


def unparenthesized(node):
    while node is not None and node.type == "parenthesized_expression" and len(node.named_children) == 1:
        node = node.named_children[0]
    return node


def module_table(node):
    if node is None or node.type != "object" or not node.named_children:
        return None
    factories = []
    for pair in node.named_children:
        if pair.type == "comment":
            continue
        if pair.type == "pair":
            key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
        elif pair.type == "method_definition":
            key, value = pair.child_by_field_name("name"), pair
        else:
            return None
        if key is None or not re.fullmatch(rb"\d+|[\"']\d+[\"']", key.text):
            return None
        value = unparenthesized(value)
        if value is None or value.type not in {"function_expression", "arrow_function", "method_definition"}:
            return None
        body = value.child_by_field_name("body")
        parameters = value.child_by_field_name("parameters")
        if body is None or body.type != "statement_block":
            return None
        if parameters is None:
            parameter = value.child_by_field_name("parameter")
            parameter_names = [parameter.text.decode()] if parameter is not None else []
        else:
            parameter_names = [part.text.decode() for part in parameters.named_children
                               if part.type != "comment"]
        factories.append({"module_id": int(key.text.strip(b"\"'")),
                          "factory_range": [value.start_byte, value.end_byte],
                          "body_range": [body.start_byte + 1, body.end_byte - 1],
                          "parameters": parameter_names, "syntax": value.type})
    if not factories:
        return None
    if len({row["module_id"] for row in factories}) != len(factories):
        raise ValueError("Webpack registration contains duplicate module identities")
    return factories


def webpack_receiver(node):
    node = unparenthesized(node)
    if node is not None and node.type == "assignment_expression":
        node = unparenthesized(node.child_by_field_name("left"))
    if node is None or node.type not in {"member_expression", "subscript_expression"}:
        return False
    key = node.child_by_field_name("property") or node.child_by_field_name("index")
    return key is not None and key.text.strip(b"\"'").startswith(b"webpackChunk")


def recovery_segments(root, *, include_standalone=False):
    """Select complete webpack expressions hidden by top-level comma sequences."""
    leaves = []
    for statement in root.named_children:
        if statement.type != "expression_statement" or not statement.named_children:
            continue
        pending = [statement.named_children[0]]
        while pending:
            expression = unparenthesized(pending.pop())
            if expression.type == "sequence_expression":
                pending.extend(reversed(expression.named_children))
            else:
                leaves.append(expression)
    result = []
    for expression in leaves:
        # A full standalone expression was already submitted to the analyzer.
        statement = expression.parent
        if statement.type == "expression_statement" and not include_standalone:
            continue
        callee = expression.child_by_field_name("function") if expression.type == "call_expression" else None
        if callee is not None and callee.type == "member_expression":
            prop = callee.child_by_field_name("property")
            receiver = callee.child_by_field_name("object")
            arguments = expression.child_by_field_name("arguments")
            values = arguments.named_children if arguments is not None else []
            if prop is not None and prop.text == b"push" and webpack_receiver(receiver) and len(values) == 1:
                array = values[0]
                parts = array.named_children if array.type == "array" else []
                factories = module_table(parts[1]) if len(parts) >= 2 else None
                if factories:
                    result.append({"kind": "webpack_chunk_registration",
                                   "range": [expression.start_byte, expression.end_byte],
                                   "factories": factories})
                    continue
        candidate = expression
        if candidate.type == "unary_expression":
            candidate = candidate.child_by_field_name("argument")
        if candidate is None or candidate.type != "call_expression":
            continue
        function = unparenthesized(candidate.child_by_field_name("function"))
        if function is None or function.type not in {"function_expression", "arrow_function"}:
            continue
        descendants = list(nodes(function))
        if not any(webpack_receiver(node) for node in descendants
                   if node.type in {"member_expression", "subscript_expression"}):
            continue
        tables = [table for node in descendants if node.type == "variable_declarator"
                  if (table := module_table(node.child_by_field_name("value")))]
        if tables:
            factories = max(tables, key=len)
            result.append({"kind": "webpack_bootstrap",
                           "range": [expression.start_byte, expression.end_byte], "factories": factories})
    return result


def factory_source_maps(document, raw, factories, sources):
    """Slice an existing readable bundle map, keeping complete factory bindings."""
    if document.get("version") != 3 or "sections" in document:
        raise ValueError("Factory slicing requires a flat version-3 source map")
    if len(sources) != len(document["sources"]):
        raise ValueError("Factory source-map identities are incomplete")
    line_starts = [0, *(match.end() for match in re.finditer(rb"\r\n|[\r\n]|\xe2\x80[\xa8\xa9]", raw))]
    def position(offset):
        line = bisect_right(line_starts, offset) - 1
        return line, len(raw[line_starts[line]:offset].decode().encode("utf-16-le")) // 2
    spans = []
    for factory in factories:
        start, end = map(position, factory["factory_range"])
        prefix = 2 if factory["syntax"] == "method_definition" else 1
        spans.append({"factory": factory, "start": start, "end": end, "prefix": prefix,
                      "lines": [[] for _ in range(end[0] - start[0] + 1)],
                      "last_source": 0, "last_line": 0, "last_column": 0, "last_name": 0,
                      "last_generated": {}, "mapped_segments": 0})
    spans.sort(key=lambda row: row["start"])
    if any(left["end"] > right["start"] for left, right in zip(spans, spans[1:])):
        raise ValueError("Readable webpack factories overlap")
    source = original_line = original_column = name = index = 0
    for generated_line, line in enumerate(document["mappings"].split(";")):
        generated_column = 0
        for segment in line.split(",") if line else []:
            values = decode_vlq(segment)
            if len(values) not in (1, 4, 5):
                raise ValueError("Invalid readable source-map segment width")
            generated_column += values[0]
            if len(values) > 1:
                source += values[1]
                original_line += values[2]
                original_column += values[3]
                if not 0 <= source < len(sources) or min(original_line, original_column) < 0:
                    raise ValueError("Readable source-map coordinate is invalid")
                if len(values) == 5:
                    name += values[4]
                    if not 0 <= name < len(document.get("names", [])):
                        raise ValueError("Readable source-map name is invalid")
            point = generated_line, generated_column
            while index < len(spans) and point >= spans[index]["end"]:
                index += 1
            if index == len(spans) or point < spans[index]["start"]:
                continue
            owner = spans[index]
            line_index = generated_line - owner["start"][0]
            column = generated_column - (owner["start"][1] if line_index == 0 else 0)
            column += owner["prefix"] if line_index == 0 else 0
            encoded = [column - owner["last_generated"].get(line_index, 0)]
            owner["last_generated"][line_index] = column
            if len(values) > 1:
                encoded.extend((source - owner["last_source"], original_line - owner["last_line"],
                                original_column - owner["last_column"]))
                owner.update(last_source=source, last_line=original_line, last_column=original_column)
                owner["mapped_segments"] += 1
                if len(values) == 5:
                    encoded.append(name - owner["last_name"])
                    owner["last_name"] = name
            owner["lines"][line_index].append(encode_vlq(encoded))
    result = {}
    for owner in spans:
        if not owner["mapped_segments"]:
            raise ValueError("Readable factory has no verified original source-map positions")
        mapping = {key: value for key, value in document.items()
                   if key not in {"sourcesContent", "sourceRoot", "mappings", "file"}}
        owner["lines"][0].insert(0, "A")  # The synthetic opening wrapper has no original position.
        mapping.update(sources=sources, mappings=";".join(",".join(line) for line in owner["lines"]),
                       x_recovery_kind="complete_lexical_factory_from_verified_readable_bundle")
        result[owner["factory"]["module_id"]] = mapping
    return result




def recover_one(input_dir, tool, audit):
    import analysis as a
    output = a.ROOT / ".analysis/results/frontend" / input_dir.name
    receipt_path = output / "analysis.json"
    acquisition_path = input_dir / "acquisition.json"
    acquired = json.loads(acquisition_path.read_bytes())
    captured = json.loads(receipt_path.read_bytes())
    originals = {row["path"]: {key: row[key] for key in ("path", "bytes", "sha256")}
                 for row in acquired["files"] if row.get("path")}
    if originals != {row["path"]: row for row in captured["original_inputs"]}:
        raise ValueError("Recovery capture does not cover the current acquisition bytes")
    if captured["tool"]["sha256"] != a.sha(tool):
        raise ValueError("Recovery analyzer differs from the verified complete capture")
    for row in originals.values():
        audit.read(row)
    for row in captured["outputs"]:
        audit.artifact(a.ROOT / row["path"], row)
    borrowed_inputs = set()
    for record in captured.get("borrowed_captures", []):
        borrowed_inputs.update(verify_borrowed_capture(record, tool, audit))
    previous = captured.get("module_recovery")
    if previous is not None:
        audit.artifact(a.ROOT / previous["recipe"]["path"], previous["recipe"])
        if audit.final_snapshot():
            raise ValueError("Recovery input or output changed during verification")
        return {"input": a.rel(input_dir), "status": "reused_verified_recovery",
                "registered_factories": previous["registered_factories"],
                "recovered_modules": previous["recovered_modules"],
                "borrowed_scripts": len(borrowed_inputs), "borrowed_providers": len(captured.get("borrowed_captures", [])),
                "missing_factories": len(previous["missing_factories"]),
                "scan_failures": len(previous["scan_failures"])}
    recipe_raw = Path(__file__).read_bytes()
    recipe_path = a.ROOT / ".analysis/results/frontend/recovery-tools" / (hashlib.sha256(recipe_raw).hexdigest() + ".py")
    recipe_path.parent.mkdir(parents=True, exist_ok=True)
    if recipe_path.exists() and recipe_path.read_bytes() != recipe_raw:
        raise ValueError("Frontend recovery recipe identity collision")
    recipe_path.write_bytes(recipe_raw)
    generated = {recipe_path}
    recipe = {"path": a.rel(recipe_path), "sha256": a.sha(recipe_path),
              "parser_versions": audit.parser_versions}
    provenance_path = output / "modules/provenance.json"
    provenance = json.loads(provenance_path.read_bytes())
    indexed = {}
    for name, owner in provenance["modules"].items():
        if owner["input"]:
            source = (input_dir / "raw" / owner["input"].replace("\\", "/")).resolve()
            indexed.setdefault(source, []).append((name, owner))
    selected, recovered, commands, scan_failures, readable_scripts = [], [], [], [], []
    def command(label, argv):
        stdout, stderr = output / (label + ".stdout.txt"), output / (label + ".stderr.txt")
        with stdout.open("wb") as out, stderr.open("wb") as err:
            completed = subprocess.run(argv, cwd=a.ROOT, stdout=out, stderr=err, check=False)
        row = {"stage": label, "argv": [a.rel(Path(arg)) if Path(arg).is_absolute() and Path(arg).is_relative_to(a.ROOT) else arg for arg in argv],
               "cwd": ".", "exit_code": completed.returncode, "stdout": a.rel(stdout), "stderr": a.rel(stderr)}
        commands.append(row)
        generated.update((stdout, stderr))
        return row
    for source in originals.values():
        if source["path"] in borrowed_inputs:
            continue
        if source["path"] not in captured["original_js_inputs"]:
            continue
        raw = audit.read(source)
        if b"webpackChunk" not in raw:
            continue
        try:
            segments = recovery_segments(audit.tree(source).root_node)
        except ValueError as error:
            scan_failures.append({"original": source, "error": str(error)})
            continue
        linked = indexed.get((a.ROOT / source["path"]).resolve(), [])
        pending = []
        for segment in segments:
            factories = [factory for factory in segment["factories"] if not any(
                any(factory["factory_range"][0] <= start < end <= factory["factory_range"][1]
                    for start, end in owner["ranges"]) for _, owner in linked)]
            if factories:
                pending.append({**segment, "factories": factories})
        if not pending:
            audit.parsed.pop(source["path"], None)
            continue
        full = [(name, owner) for name, owner in linked
                if owner["ranges"] == [[0, len(raw)]]]
        if len(full) > 1:
            raise ValueError("More than one complete readable script claims the original")
        if full:
            readable = output / "modules" / full[0][0]
        else:
            readable = output / "readable-scripts" / (source["sha256"] + ".js")
            readable.parent.mkdir(parents=True, exist_ok=True)
            completed = command("recover-format-" + source["sha256"][:16],
                [str(tool), str(a.ROOT / source["path"]), "--level", "minimal",
                 "--emit-source-map", "--diagnostics", "--json", "-o", str(readable)])
            if completed["exit_code"]:
                raise ValueError("Whole-script static formatting failed; diagnostics retained")
            generated.update((readable, readable.with_name(readable.name + ".map")))
        readable_raw = readable.read_bytes()
        tree = audit.parser.parse(readable_raw)
        if tree.root_node.has_error:
            raise ValueError("Verified readable script has unresolved syntax")
        readable_factories = {}
        for segment in recovery_segments(tree.root_node, include_standalone=True):
            for factory in segment["factories"]:
                if factory["module_id"] in readable_factories:
                    raise ValueError("Readable script has ambiguous webpack module identities")
                readable_factories[factory["module_id"]] = factory
        needed = []
        for segment in pending:
            for factory in segment["factories"]:
                readable_factory = readable_factories.get(factory["module_id"])
                if readable_factory is None or len(readable_factory["parameters"]) != len(factory["parameters"]):
                    raise ValueError("Readable factory lost its complete original parameter bindings")
                needed.append(readable_factory)
        map_path = readable.with_name(readable.name + ".map")
        document = json.loads(map_path.read_bytes())
        for literal in document["sources"]:
            if (map_path.parent / document.get("sourceRoot", "") / literal).resolve() != (a.ROOT / source["path"]).resolve():
                raise ValueError("Readable source map points outside its verified original")
        directory = output / "modules/factories" / source["sha256"]
        directory.mkdir(parents=True, exist_ok=True)
        sources = [os.path.relpath(a.ROOT / source["path"], directory).replace("\\", "/")
                   for _ in document["sources"]]
        mappings = factory_source_maps(document, readable_raw, needed, sources)
        for segment in pending:
            selected.append({"original": source, **segment})
            for factory in segment["factories"]:
                readable_factory = readable_factories[factory["module_id"]]
                start, end = readable_factory["factory_range"]
                value = readable_raw[start:end]
                wrapped = b"({" + value + b"});" if readable_factory["syntax"] == "method_definition" else b"(" + value + b");"
                if audit.parser.parse(wrapped).root_node.has_error:
                    raise ValueError("Complete recovered factory has invalid syntax")
                target = directory / ("module-" + str(factory["module_id"]) + ".factory.js")
                target.write_bytes(wrapped + b"\n")
                mapping_path = target.with_name(target.name + ".map")
                mapping = mappings[factory["module_id"]]
                mapping["file"] = target.name
                a.save(mapping_path, mapping)
                generated.update((target, mapping_path))
                key = target.relative_to(output / "modules").as_posix()
                if key in provenance["modules"]:
                    raise ValueError("Factory recovery would overwrite an existing module identity")
                provenance["modules"][key] = {
                    "input": os.path.relpath(a.ROOT / source["path"], input_dir / "raw").replace("\\", "/"),
                    "ranges": [factory["factory_range"]], "extraction": "complete_lexical_factory",
                    "factory_parameters": factory["parameters"], "module_id": factory["module_id"],
                    "readable_script": a.rel(readable), "readable_range": readable_factory["factory_range"]}
                recovered.append({"output": key, "original": source["path"], **factory})
        readable_scripts.append({"original": source, "readable_script": a.rel(readable),
                                 "runtime_and_non_factory_code_preserved": True})
        audit.parsed.pop(source["path"], None)
        del tree, readable_raw, document, mappings
    a.save(provenance_path, provenance)
    generated.add(provenance_path)
    changed = [name for name in audit.final_snapshot()
               if name not in {a.rel(provenance_path), a.rel(recipe_path)}]
    if changed:
        raise ValueError("Evidence changed during recovery: " + ", ".join(changed))
    captured["commands"].extend(commands)
    inherited = captured.pop("pending_previous_module_recovery", {})
    captured["module_recovery"] = {
        "strategy": "complete_lexical_factory_slices", "recipe": recipe, "target_execution": False,
        "registered_factories": inherited.get("registered_factories", 0) + sum(len(row["factories"]) for row in selected),
        "recovered_modules": inherited.get("recovered_modules", 0) + len(recovered),
        "registrations": inherited.get("registrations", []) + selected,
        "factories": inherited.get("factories", []) + recovered,
        "readable_scripts": inherited.get("readable_scripts", []) + readable_scripts,
        "missing_factories": inherited.get("missing_factories", []),
        "scan_failures": inherited.get("scan_failures", []) + scan_failures,
        "original_acquisition_sha256": a.sha(acquisition_path),
        "source_map_kind": "composed_from_verified_readable_bundle",
        "official_original_names_recovered": False}
    outputs = {row["path"]: row for row in captured["outputs"]}
    for path in generated:
        outputs[a.rel(path)] = {"path": a.rel(path), "bytes": path.stat().st_size, "sha256": a.sha(path)}
    captured["outputs"] = [outputs[key] for key in sorted(outputs)]
    captured["recorded_at_utc"] = a.stamp()
    a.save(receipt_path, captured)
    all_failures = captured["module_recovery"]["scan_failures"]
    return {"input": a.rel(input_dir), "status": "recovered_with_diagnostics" if all_failures else "recovered",
            "registrations": len(selected), "registered_factories": captured["module_recovery"]["registered_factories"],
            "recovered_modules": captured["module_recovery"]["recovered_modules"],
            "borrowed_scripts": len(borrowed_inputs), "borrowed_providers": len(captured.get("borrowed_captures", [])),
            "missing_factories": len(captured["module_recovery"]["missing_factories"]), "scan_failures": len(all_failures),
            "failed_stages": sum(bool(row["exit_code"]) for row in commands)}


def recover_source_set(path, tool):
    import analysis as a
    from host import HostLocalAudit
    audit = HostLocalAudit()
    try:
        acquired = json.loads(path.read_bytes())
        if acquired.get("source_current_during_acquisition") is not True:
            return {"input": a.rel(path.parent), "status": "outside_current_deployment_scope"}
        if not any(Path(row.get("path", "")).suffix.lower() in {".js", ".mjs"} for row in acquired["files"]):
            return {"input": a.rel(path.parent), "status": "no_javascript_originals"}
        capture = a.ROOT / ".analysis/results/frontend" / path.parent.name / "analysis.json"
        if not capture.is_file():
            a.analyze_frontend(SimpleNamespace(input=a.rel(path.parent), no_refresh_gaps=True), refresh_ledger=False)
            restored = json.loads(capture.read_bytes())["module_recovery"]
            return {"input": a.rel(path.parent), "status": "captured_and_recovered",
                    "registered_factories": restored["registered_factories"], "recovered_modules": restored["recovered_modules"],
                    "lexical_contexts": recover_validation_context(path.parent, HostLocalAudit())}
        if extend_capture(path.parent, tool, audit):
            audit = HostLocalAudit()
        result = recover_one(path.parent, tool, audit)
        result["lexical_contexts"] = recover_validation_context(path.parent, HostLocalAudit())
        return result
    except Exception as error:
        return {"input": a.rel(path.parent), "status": "failed", "error": str(error).replace(str(a.ROOT), "<project>")}


def recover_frontend(args):
    import analysis as a
    # Validate the scheduler before resolving inputs or loading external tools;
    # callers receive the concurrency contract error deterministically.
    workers = getattr(args, "workers", 2)
    if type(workers) is not int or not 2 <= workers <= 8:
        raise ValueError("Frontend recovery workers must be between two and eight")
    inputs = sorted((a.ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")) if getattr(args, "all_acquired", False) else [a.resolve(args.input, ".analysis/inputs") / "acquisition.json"]
    tool, rows = a.wakaru(), []
    # Keep the single-source entry parallel too.  Recovery spends most of its
    # time waiting on external static analyzers and filesystem I/O, so a
    # bounded thread pool gives the same scheduling and failure isolation as
    # the multi-source batch.  One-worker runs are rejected so this entry
    # cannot silently become a serial reverse-engineering pass.
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(recover_source_set, path, tool): path for path in inputs}
        for future in concurrent.futures.as_completed(futures):
            row = future.result()
            rows.append(row)
            print(json.dumps({"source_sets_done": len(rows), "source_sets_total": len(inputs), **row}), flush=True)
            if getattr(args, "all_acquired", False):
                a.save(a.ROOT / ".analysis/results/frontend/module-recovery.json",
                       {"recorded_at_utc": a.stamp(), "target_execution": False, "source_sets": rows})
    if getattr(args, "all_acquired", False):
        a.save(a.ROOT / ".analysis/results/frontend/module-recovery.json",
               {"recorded_at_utc": a.stamp(), "target_execution": False, "source_sets": rows})
    if not getattr(args, "no_refresh_gaps", False):
        a.refresh_gaps()
    if any(row["status"] == "failed" for row in rows):
        raise ValueError("Frontend recovery has failed source sets; inspect the retained diagnostics")
    return rows
