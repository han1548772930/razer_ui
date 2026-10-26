"""Explicit, byte-bound adjudication of complete native capture alternatives."""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import re


def identity(a, path):
    raw = path.read_bytes()
    return {"path": a.rel(path), "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def retain_diagnostic(a, source):
    """Retain exact ended-analyzer diagnostics independently of disposable scratch."""
    source = a.resolve(a.rel(source), ".work/native")
    raw = source.read_bytes()
    if not raw or b"fatal failure" not in raw or b"Traceback" not in raw:
        raise ValueError("Analyzer diagnostic does not retain the original fatal failure")
    digest = hashlib.sha256(raw).hexdigest()
    target = a.ROOT / ".analysis/results/reverse-pending/review-sources" / (digest + ".log")
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        if target.read_bytes() != raw:
            raise ValueError("Retained analyzer diagnostic bytes conflict")
    else:
        with target.open("xb") as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
    return identity(a, target)


def checked(a, record, boundary=".analysis/results"):
    if (not isinstance(record, dict) or set(record) != {"path", "bytes", "sha256"}
            or type(record.get("bytes")) is not int or record["bytes"] <= 0
            or not isinstance(record.get("path"), str)
            or not isinstance(record.get("sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", record["sha256"])):
        raise ValueError("Adjudication requires a complete exact byte identity")
    path = a.resolve(record["path"], boundary)
    if identity(a, path) != record:
        raise ValueError("Adjudication evidence bytes changed: " + record["path"])
    return path.read_bytes()


def value_identity(value, present=True):
    raw = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return {"present": present, "sha256": hashlib.sha256(raw).hexdigest() if present else None}


def differences(previous, selected, pointer=""):
    """Enumerate every changed JSON subtree without normalizing any raw field."""
    if type(previous) is type(selected) and previous == selected:
        return []
    if isinstance(previous, dict) and isinstance(selected, dict):
        found = []
        for key in sorted(previous.keys() | selected.keys()):
            child = pointer + "/" + key.replace("~", "~0").replace("/", "~1")
            if key in previous and key in selected:
                found.extend(differences(previous[key], selected[key], child))
            else:
                found.append({"pointer": child,
                    "previous": value_identity(previous.get(key), key in previous),
                    "selected": value_identity(selected.get(key), key in selected)})
        return found
    return [{"pointer": pointer, "previous": value_identity(previous), "selected": value_identity(selected)}]


def _evidence(a, records):
    if not isinstance(records, list) or not records:
        raise ValueError("Adjudication needs explicit retained review evidence")
    for record in records:
        checked(a, record)


def assembly_rendering_differences(previous, selected):
    """Separate reviewed display text from otherwise identical instruction records."""
    before, after = previous.get("assembly"), selected.get("assembly")
    if not isinstance(before, list) or not isinstance(after, list) or len(before) != len(after):
        raise ValueError("Native adjudication cannot replace different original machine semantics")
    changes = []
    for old, new in zip(before, after):
        if (not isinstance(old, dict) or not isinstance(new, dict)
                or {key: value for key, value in old.items() if key != "text"}
                != {key: value for key, value in new.items() if key != "text"}):
            raise ValueError("Native adjudication cannot replace different original machine semantics")
        if old.get("text") != new.get("text"):
            if not isinstance(old.get("text"), str) or not isinstance(new.get("text"), str):
                raise ValueError("Native adjudication display change lacks complete original text")
            changes.append({"address": old["address"], "previous_text": old["text"], "selected_text": new["text"]})
    return changes


def _typed_function(body):
    pseudo = body.get("pseudocode", {})
    if not isinstance(pseudo, dict) or not isinstance(pseudo.get("locals"), list):
        raise ValueError("Adjudicated function lacks complete live typed local coordinates")
    metadata = pseudo.get("locals_capture", {})
    if (not isinstance(metadata, dict) or metadata.get("complete") is not True
            or metadata.get("mba_entry_ea") != body.get("address")):
        raise ValueError("Adjudicated function lacks complete live typed local coordinates")
    for local in pseudo.get("locals", []):
        if not isinstance(local, dict):
            raise ValueError("Adjudicated function has incomplete typed local coordinates")
        location = local.get("location", {})
        conversion = local.get("ida_location_conversion", {})
        if (not isinstance(location, dict) or not isinstance(conversion, dict)
                or local.get("complete") is not True or type(local.get("width")) is not int
                or local["width"] <= 0 or location.get("complete") is not True
                or location.get("status") != "observed"
                or location.get("register_namespace") != "hexrays_microregisters"
                or conversion.get("complete") is not True):
            raise ValueError("Adjudicated function has incomplete typed local coordinates")
        if conversion.get("status") == "not_applicable":
            proof = local.get("virtual_register_proof", {})
            if (metadata.get("schema_version") != 2 or location.get("kind") != "register"
                    or conversion.get("reason") != "sdk_kernel_register_has_no_processor_location"
                    or not isinstance(proof, dict) or proof.get("complete") is not True
                    or proof.get("status") != "api_returned" or proof.get("is_kernel_register") is not True
                    or proof.get("processor_register") != -1
                    or proof.get("kernel_api") != "ida_hexrays.is_kreg"
                    or proof.get("processor_api") != "ida_hexrays.mreg2reg"
                    or proof.get("name_api") != "ida_hexrays.get_mreg_name"
                    or proof.get("register_namespace") != "hexrays_microregisters"
                    or proof.get("register") != location.get("register")
                    or proof.get("register_offset") != location.get("register_offset")
                    or proof.get("width") != local["width"]
                    or not isinstance(proof.get("name"), str) or not proof["name"].strip()):
                raise ValueError("Adjudicated virtual local lacks exact SDK kernel register proof")
        elif conversion.get("roundtrip_matches") is not True:
            raise ValueError("Adjudicated function has incomplete typed local coordinates")
        if location.get("kind") == "stack":
            stack = local.get("stack_coordinate_conversion", {})
            if (not isinstance(stack, dict) or stack.get("complete") is not True
                    or stack.get("roundtrip_matches") is not True):
                raise ValueError("Adjudicated stack local lacks a complete coordinate roundtrip")
            if metadata.get("schema_version") == 2:
                delta = metadata.get("stack_offset_delta")
                offset = location.get("stack_offset")
                if (type(delta) is not int or type(offset) is not int
                        or stack.get("coordinate_namespace") != "ida_frame_offsets"
                        or stack.get("conversion_api") != "cfunc_t.get_stkoff_delta"
                        or stack.get("rule") != "ida_stkoff = v.location.stkoff() - cfunc.get_stkoff_delta()"
                        or stack.get("cfunc_stack_offset_delta") != delta
                        or stack.get("decompiler_offset") != offset or stack.get("ida_offset") != offset - delta
                        or stack.get("roundtrip_decompiler_offset") != offset):
                    raise ValueError("Adjudicated stack local differs from documented cfunc frame coordinates")
    if (not body.get("type_before_decompile") or not body.get("type_after_decompile")
            or body.get("type") != body["type_before_decompile"]
            or body.get("assembly_after_decompile") != body.get("assembly")):
        raise ValueError("Adjudicated function changed type or original assembly during decompilation")


def original_machine(raw, body):
    """Check every retained chunk and instruction against uniquely file-backed PE bytes."""
    from host import inspect_pe
    pe = inspect_pe(raw)
    base = int(pe["image_base"], 16)

    def bytes_at(start, end):
        sections = [row for row in pe["sections"] if 0 <= start - base - row["rva"]
                    and end - base - row["rva"] <= row["bytes"]]
        if end <= start or len(sections) != 1:
            raise ValueError("Adjudicated original machine range is not uniquely file-backed")
        offset = sections[0]["file_offset"] + start - base - sections[0]["rva"]
        return raw[offset:offset + end - start]

    chunks = body.get("chunks", [])
    if not chunks or not body.get("assembly"):
        raise ValueError("Adjudicated function omitted its complete machine chunks or instructions")
    for chunk in chunks:
        code = bytes_at(int(chunk["start"], 16), int(chunk["end"], 16))
        if code.hex() != chunk["raw_hex"] or ("sha256" in chunk and hashlib.sha256(code).hexdigest() != chunk["sha256"]):
            raise ValueError("Adjudicated function chunk differs from original PE bytes")
    for instruction in body["assembly"]:
        at, code = int(instruction["address"], 16), bytes.fromhex(instruction["bytes"])
        if (not code or bytes_at(at, at + len(code)) != code
                or not any(int(chunk["start"], 16) <= at < at + len(code) <= int(chunk["end"], 16) for chunk in chunks)):
            raise ValueError("Adjudicated instruction differs from its complete original PE chunks")


def _apply_review(a, result, resolution, candidates, review_record):
    review = json.loads(checked(a, review_record))
    expected_input = identity(a, a.resolve(json.loads((result / "capture.json").read_bytes())["input"], ".analysis/inputs"))
    if (review.get("schema_version") != 1 or review.get("review_kind") != "native_complete_artifact_adjudication"
            or review.get("source_input") != expected_input
            or review.get("base_capture") != resolution["base_capture"]
            or review.get("target_execution") is not False
            or review.get("raw_artifacts_modified") is not False
            or not isinstance(review.get("functions"), list) or not review["functions"]):
        raise ValueError("Native adjudication scope differs from the exact retained original and base capture")
    _evidence(a, review.get("evidence"))
    original_raw = checked(a, expected_input, ".analysis/inputs")
    adjudicated = {}
    for entry in review["functions"]:
        rva = entry.get("rva")
        if rva in adjudicated or rva not in resolution["conflicting_functions"]:
            raise ValueError("Native adjudication has a duplicate or absent complete-function conflict")
        observed = sorted([{ "function": identity(a, a.ROOT / item["capture"]["path"]),
                             "supplement": item["supplement"]} for item in candidates[rva]],
                          key=lambda item: (item["supplement"]["path"], item["function"]["path"]))
        if entry.get("candidates") != observed:
            raise ValueError("Native adjudication candidate set changed or omitted a complete artifact")
        selected = entry.get("selected")
        matches = [item for item in candidates[rva] if identity(a, a.ROOT / item["capture"]["path"]) == selected]
        if len(matches) != 1:
            raise ValueError("Native adjudication selected artifact is not a unique strictly verified candidate")
        body = matches[0]["body"]
        _typed_function(body)
        original_machine(original_raw, body)
        stage_changes = differences({"type": body["type_before_decompile"]}, {"type": body["type_after_decompile"]})
        stage_reviews = entry.get("stage_differences", [])
        if ([{key: item.get(key) for key in ("pointer", "previous", "selected")} for item in stage_reviews] != stage_changes
                or any(item.get("classification") != "observed_type_refinement"
                       or not isinstance(item.get("reason"), str) or not item["reason"].strip() for item in stage_reviews)):
            raise ValueError("Native adjudication omitted the exact decompiler-stage type refinement")
        _evidence(a, entry.get("evidence"))
        if not isinstance(entry.get("reason"), str) or not entry["reason"].strip():
            raise ValueError("Native adjudication lacks its semantic selection reason")
        reviews = entry.get("comparisons")
        if not isinstance(reviews, list) or len(reviews) != len(observed) - 1:
            raise ValueError("Native adjudication omitted complete alternative comparisons")
        alternatives = {item["function"]["path"]: item["function"] for item in observed if item["function"] != selected}
        reviewed = set()
        for comparison in reviews:
            previous = comparison.get("previous")
            if (not isinstance(previous, dict) or previous.get("path") in reviewed
                    or alternatives.get(previous.get("path")) != previous):
                raise ValueError("Native adjudication compared a duplicate or different alternative")
            reviewed.add(previous["path"])
            old = json.loads(checked(a, previous))
            machine_fields = ("address", "rva", "end", "chunks", "basic_blocks", "xrefs_to_entry")
            if any(old.get(key) != body.get(key) for key in machine_fields):
                raise ValueError("Native adjudication cannot replace different original machine semantics")
            render_changes = assembly_rendering_differences(old, body)
            render_reviews = comparison.get("assembly_rendering_differences", [])
            if (not isinstance(render_reviews, list)
                    or [{key: row.get(key) for key in ("address", "previous_text", "selected_text")}
                        for row in render_reviews] != render_changes
                    or any(not isinstance(row.get("reason"), str) or not row["reason"].strip()
                           for row in render_reviews)):
                raise ValueError("Native adjudication omitted exact instruction display changes or their reasons")
            expected = differences(old, body)
            changes = comparison.get("differences")
            if (not isinstance(changes, list)
                    or [{k: row.get(k) for k in ("pointer", "previous", "selected")} for row in changes] != expected):
                raise ValueError("Native adjudication omitted or changed an exact field difference")
            for change in changes:
                if (change.get("classification") not in {"observed_type_refinement", "pseudocode_type_propagation",
                                                        "capture_metadata_replacement"}
                        or not isinstance(change.get("reason"), str) or not change["reason"].strip()):
                    raise ValueError("Native adjudication lacks a reviewed reason for a changed field")
        adjudicated[rva] = {"selected": selected, "supplement": matches[0]["supplement"],
                            "review": review_record, "reason": entry["reason"], "body": body,
                            "capture": matches[0]["capture"]}
    return adjudicated


def apply(a, result, resolution, candidates, review_record=None):
    """Apply explicit adjudications only after all ordinary strict checks pass."""
    binding_path = result / "reanalysis-adjudication.json"
    if review_record is None and not binding_path.is_file():
        return {}
    try:
        if review_record is None:
            binding = json.loads(binding_path.read_bytes())
            if (binding.get("schema_version") != 1 or binding.get("base_capture") != resolution["base_capture"]
                    or binding.get("target_execution") is not False):
                raise ValueError("Native adjudication binding differs from its retained base capture")
            review_record = binding["review"]
        adjudicated = _apply_review(a, result, resolution, candidates, review_record)
    except (OSError, ValueError, KeyError, TypeError) as error:
        resolution["verification_failures"].append({"adjudication": a.rel(binding_path), "error": str(error)})
        return {}
    resolution["historical_conflicting_functions"] = copy.deepcopy(resolution["conflicting_functions"])
    resolution["historical_verification_failures"] = []
    resolution["adjudicated_functions"] = {}
    for rva, item in adjudicated.items():
        resolution["complete_functions"][rva] = {"capture": item["capture"], **item["body"]}
        del resolution["conflicting_functions"][rva]
        resolution["adjudicated_functions"][rva] = {k: v for k, v in item.items() if k not in ("body", "capture")}
    current = []
    for failure in resolution["verification_failures"]:
        (resolution["historical_verification_failures"] if failure.get("function_rva") in adjudicated else current).append(failure)
    resolution["verification_failures"] = current
    return {rva: item["supplement"] for rva, item in adjudicated.items()}


def _same_machine(first, second):
    return (all(first.get(key) == second.get(key)
                for key in ("address", "rva", "end", "chunks", "xrefs_to_entry"))
            and [(item["address"], item["bytes"]) for item in first.get("assembly", [])]
            == [(item["address"], item["bytes"]) for item in second.get("assembly", [])])


def merged_attempt_coverage(a, original_raw, base, previous, replacement, manifest):
    """Prove an old head is preserved by an independently reviewed complete merge."""
    from host import inspect_pe

    selection = manifest.get("function_selection", {})
    merges = [row for row in selection.get("function_merges", [])
              if row.get("owner_rva") == previous.get("rva")]
    if len(merges) != 1:
        raise ValueError("Different native boundaries lack a unique reviewed complete function merge")
    merge = merges[0]
    specification = merge["owner_rva"] + "=" + ",".join(merge["member_rvas"])
    recomputed = a.native_function_merges(original_raw, base,
        [int(row["rva"], 16) for row in selection.get("targets", [])],
        [specification], merge["assembly_evidence"], inspect_pe(original_raw))
    if recomputed != [merge]:
        raise ValueError("Native attempt merge differs from its complete original-byte review")
    source = merge["members"][0]
    source_record = {key: source[key] for key in ("path", "bytes", "sha256")}
    original_head = json.loads(checked(a, source_record, ".analysis/results/native"))
    if not _same_machine(previous, original_head):
        raise ValueError("Failed native attempt is not the exact original head of the reviewed merge")
    complete_union = a.native_function_merge_body(merge)
    if not _same_machine(replacement, complete_union):
        raise ValueError("Native attempt replacement omitted or changed the reviewed complete merge union")
    original_machine(original_raw, previous)
    original_machine(original_raw, replacement)
    return {"coverage_kind": "reviewed_complete_function_merge", "original_head": source_record,
            "complete_merge": merge}


def partitioned_attempt_coverage(a, original_raw, base, previous, replacement, manifest, directory):
    """Bind an old complete body to its verified owner and all local helpers."""
    from host import inspect_pe

    selection = manifest.get("function_selection", {})
    partitions = [row for row in selection.get("function_partitions", [])
                  if row.get("owner_rva") == previous.get("rva")]
    if len(partitions) != 1:
        raise ValueError("Different native boundaries lack a unique reviewed complete function partition")
    partition = partitions[0]
    pe = inspect_pe(original_raw)
    image_base = int(pe["image_base"], 16)
    specification = partition["owner_rva"] + "=" + ",".join(
        helper["rva"] + ":" + hex(int(helper["expected_end"], 16) - image_base)
        for helper in partition["helpers"])
    recomputed = a.native_function_partitions(original_raw, base,
        [int(row["rva"], 16) for row in selection.get("targets", [])],
        [specification], partition["assembly_evidence"], pe)
    if recomputed != [partition]:
        raise ValueError("Native attempt partition differs from its exact original-byte review")
    original = json.loads(checked(a, partition["source_function"], ".analysis/results/native"))
    expected = a.native_function_partition_bodies(partition)
    if not _same_machine(previous, original) or not _same_machine(replacement, expected["owner"]):
        raise ValueError("Native attempt original or replacement differs from the complete partition evidence")
    helpers = []
    for rva, body in expected["helpers"].items():
        records = [row for row in manifest.get("partition_helpers", []) if row.get("rva") == rva]
        if len(records) != 1:
            raise ValueError("Native attempt partition omitted a complete original local helper")
        record = records[0]
        path = a.resolve(a.rel(directory / record["path"]), a.rel(directory))
        observed = json.loads(path.read_bytes())
        if (a.sha(path) != record["sha256"] or record.get("assembly_captured") is not True
                or record.get("pseudocode_captured") is not False or observed.get("pseudocode") is not None
                or not _same_machine(observed, body)):
            raise ValueError("Native attempt local helper omitted or changed its complete original machine")
        original_machine(original_raw, observed)
        helpers.append(identity(a, path))
    original_machine(original_raw, previous)
    original_machine(original_raw, replacement)
    return {"coverage_kind": "reviewed_complete_function_partition", "original_function": partition["source_function"],
            "complete_partition": partition, "complete_helper_assemblies": helpers,
            "helper_semantics_or_abi_completion": False}


def attempt_coverage(a, result, resolution, failure, failure_diagnostic=None):
    """Prove that every target of an ended failed attempt has a complete replacement."""
    capture_path = a.resolve(failure["supplement"], a.rel(result / "supplements"))
    manifest = json.loads(capture_path.read_bytes())
    session_path, plan_path = capture_path.parent / "session.json", capture_path.parent / "plan.json"
    session, plan = json.loads(session_path.read_bytes()), json.loads(plan_path.read_bytes())
    base = json.loads((result / "capture.json").read_bytes())
    original = a.resolve(base["input"], ".analysis/inputs")
    original_raw = original.read_bytes()
    if hashlib.sha256(original_raw).hexdigest() != base["input_sha256"] or len(original_raw) != base["input_bytes"]:
        raise ValueError("Native attempt original PE bytes changed")
    base_identity = {"path": resolution["base_capture"]["path"], "sha256": resolution["base_capture"]["sha256"]}
    selection = manifest.get("function_selection", {})
    failures = manifest.get("failures")
    if (manifest.get("all_pseudocode_captured") is not False
            or manifest.get("input_sha256") != base["input_sha256"]
            or manifest.get("base_capture") != base_identity
            or manifest.get("target_execution") is not False
            or not isinstance(failures, list)
            or not session.get("finished_at_utc") or session.get("target_execution") is not False
            or session.get("capture_tool") != manifest.get("capture_tool")
            or plan.get("base_capture") != base_identity or plan.get("function_selection") != selection
            or plan.get("source_database") != manifest.get("source_database")
            or plan.get("sha256") != base["input_sha256"] or plan.get("input_bytes") != base["input_bytes"]
            or plan.get("input") != base["input"]
            or plan.get("target_execution") is not False):
        raise ValueError("Failed native attempt is not an exact ended capture with only outstanding decompilation results")
    targets = [row["rva"] for row in selection.get("targets", [])]
    records = manifest.get("functions", [])
    before_export = manifest.get("capture_complete") is False
    if not targets or len(set(targets)) != len(targets):
        raise ValueError("Failed native attempt lacks unique exact selected targets")
    if before_export:
        if (records or failures or session.get("passed") is not False or session.get("exit_code") != 1
                or any(selection.get(key) for key in ("static_observation", "function_merges", "function_outlines",
                    "function_partitions", "function_shared_tails", "function_fresh_inlines"))
                or failure_diagnostic is None):
            raise ValueError("Ended pre-export failure lacks complete diagnostic and simple target coverage")
        diagnostic = checked(a, failure_diagnostic)
        if b"fatal failure" not in diagnostic or b"Traceback" not in diagnostic:
            raise ValueError("Ended pre-export failure diagnostic does not retain the analyzer exception")
        source_database = plan.get("source_database", {})
        source_path = a.resolve(source_database.get("path", ""), ".work/native")
        if a.sha(source_path) != source_database.get("sha256"):
            raise ValueError("Ended pre-export failure original source database changed")
        records = [row for row in base["functions"] if row["rva"] in targets]
        if len(records) != len(targets):
            raise ValueError("Ended pre-export failure plan is not covered by exact original function boundaries")
        raw = a.resolve(base["input"], ".analysis/inputs").read_bytes()
        mapped = a.native_function_selection(raw, base, [int(rva, 16) for rva in targets])
        if mapped["targets"] != selection["targets"]:
            raise ValueError("Ended pre-export failure target addresses differ from original PE mapping")
    elif (manifest.get("capture_complete") is not True
            or manifest.get("database_integrity", {}).get("verified") is not True
            or manifest.get("base_capture_unchanged") is not True
            or manifest.get("base_database_unchanged") is not True
            or session.get("passed") is not True or session.get("exit_code") != 0
            or not failures or len(records) != len(targets) or {row["rva"] for row in records} != set(targets)
            or any(row.get("stage") != "hexrays" or row.get("rva") not in targets for row in failures)
            or {row["rva"] for row in failures} != {row["rva"] for row in records if row.get("pseudocode_captured") is not True}):
        raise ValueError("Failed native attempt omitted selected functions or has failures outside Hex-Rays targets")
    functions, base_verified = [], None
    base_records = {item["rva"]: item for item in base["functions"]}
    for row in records:
        rva = row["rva"]
        selected = resolution["complete_functions"].get(rva)
        if selected is None:
            base_record = base_records.get(rva)
            if (rva in resolution["conflicting_functions"] or not base_record
                    or base_record.get("pseudocode_captured") is not True
                    or any(item.get("stage", "").lower().startswith(rva[2:].lower() + ":") for item in base["failures"])):
                raise ValueError("Failed native attempt target still lacks a strictly complete unconflicted replacement: " + rva)
            if base_verified is None:
                base_verified, _ = a.verify_native_capture(result, {"sha256": base["input_sha256"], "bytes": base["input_bytes"]})
            source = a.resolve(a.rel(result / base_record["path"]), a.rel(result))
            selected = {"capture": {**base_record, "path": a.rel(source)}, **json.loads(source.read_bytes())}
            pseudo = selected.get("pseudocode", {})
            if not isinstance(pseudo, dict) or not pseudo.get("lines") or not isinstance(pseudo.get("locals"), list):
                raise ValueError("Native attempt original complete target lacks full pseudocode and locals")
            original_machine(original_raw, selected)
        old_owner = result if before_export else capture_path.parent
        old_path = a.resolve(a.rel(old_owner / row["path"]), a.rel(old_owner))
        if a.sha(old_path) != row["sha256"] or (not before_export and row.get("assembly_captured") is not True):
            raise ValueError("Failed native attempt output changed or omitted original assembly")
        old = json.loads(old_path.read_bytes())
        replacement = identity(a, a.ROOT / selected["capture"]["path"])
        replacement_manifest_path = (a.ROOT / selected["capture"]["path"]).parent.parent / "capture.json"
        replacement_manifest = identity(a, replacement_manifest_path)
        if replacement_manifest != resolution["base_capture"] and replacement_manifest not in [item["receipt"] for item in resolution["supplements"]]:
            raise ValueError("Native attempt replacement does not belong to a strictly verified complete supplement")
        union = None
        if not _same_machine(old, selected):
            manifest = json.loads(replacement_manifest_path.read_bytes())
            if any(row.get("owner_rva") == rva for row in manifest.get("function_selection", {}).get("function_partitions", [])):
                union = partitioned_attempt_coverage(a, original_raw, base, old, selected, manifest,
                                                     replacement_manifest_path.parent)
            else:
                union = merged_attempt_coverage(a, original_raw, base, old, selected, manifest)
        functions.append({"rva": rva, "previous_function": identity(a, old_path),
                          "replacement_function": replacement, "replacement_supplement": replacement_manifest,
                          **({("partitioned_original_coverage" if union["coverage_kind"] == "reviewed_complete_function_partition"
                               else "merged_original_coverage"): union} if union is not None else {})})
    return {"capture": identity(a, capture_path), "session": identity(a, session_path),
            "plan": identity(a, plan_path), "verification_failure": failure,
            "original_failures": failures, "functions": functions,
            "capture_outcome": "failed_before_export" if before_export else "ended_with_hexrays_failures",
            **({"failure_diagnostic": failure_diagnostic, "source_database": identity(a, source_path)}
               if before_export else {})}


def apply_attempts(a, result, resolution, review_record=None):
    binding_path = result / "reanalysis-attempt-adjudication.json"
    if review_record is None and not binding_path.is_file():
        return
    try:
        if review_record is None:
            binding = json.loads(binding_path.read_bytes())
            if (binding.get("schema_version") != 1 or binding.get("base_capture") != resolution["base_capture"]
                    or binding.get("target_execution") is not False):
                raise ValueError("Native attempt adjudication binding changed")
            review_record = binding["review"]
        review = json.loads(checked(a, review_record))
        base = json.loads((result / "capture.json").read_bytes())
        if (review.get("schema_version") != 1 or review.get("review_kind") != "native_superseded_attempt_adjudication"
                or review.get("source_input") != identity(a, a.resolve(base["input"], ".analysis/inputs"))
                or review.get("base_capture") != resolution["base_capture"]
                or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
                or not isinstance(review.get("attempts"), list) or not review["attempts"]):
            raise ValueError("Native attempt adjudication scope differs from the exact retained original")
        _evidence(a, review.get("evidence"))
        failures = {item["supplement"]: item for item in resolution["verification_failures"] if "supplement" in item
                    and "function_rva" not in item}
        proved = {}
        for entry in review["attempts"]:
            name = entry.get("capture", {}).get("path")
            if name in proved or name not in failures:
                raise ValueError("Native attempt review has a duplicate or absent failed selection")
            expected = attempt_coverage(a, result, resolution, failures[name], entry.get("failure_diagnostic"))
            if {key: entry.get(key) for key in expected} != expected:
                raise ValueError("Native attempt review omitted a target, changed bytes or a complete replacement")
            if not isinstance(entry.get("reason"), str) or not entry["reason"].strip():
                raise ValueError("Native attempt review lacks an explicit coverage reason")
            proved[name] = {**expected, "review": review_record, "reason": entry["reason"]}
    except (OSError, ValueError, KeyError, TypeError) as error:
        resolution["verification_failures"].append({"adjudication": a.rel(binding_path), "error": str(error)})
        return
    current = []
    historical = resolution.setdefault("historical_verification_failures", [])
    for failure in resolution["verification_failures"]:
        (historical if failure.get("supplement") in proved and "function_rva" not in failure else current).append(failure)
    resolution["verification_failures"] = current
    resolution["superseded_attempts"] = proved


def run(a, args):
    review_path = a.resolve(args.review, ".analysis/results")
    if a.sha(review_path) != args.review_sha256:
        raise ValueError("Native adjudication review SHA-256 differs from the explicitly reviewed bytes")
    record = identity(a, review_path)
    review = json.loads(checked(a, record))
    original = a.resolve(args.input, ".analysis/inputs")
    if identity(a, original) != review.get("source_input"):
        raise ValueError("Native adjudication command input differs from reviewed original")
    result = a.ROOT / ".analysis/results/native" / review["source_input"]["sha256"]
    attempts = review.get("review_kind") == "native_superseded_attempt_adjudication"
    with a.index_lock(a.ROOT / ".analysis/reverse-pending.lock"):
        resolved = a.native_reanalysis_resolution(result, **(
            {"attempt_adjudication_record": record} if attempts else {"adjudication_record": record}))
        expected = ({entry["capture"]["path"] for entry in review["attempts"]} if attempts
                    else {entry["rva"] for entry in review["functions"]})
        observed = resolved.get("superseded_attempts" if attempts else "adjudicated_functions", {})
        if set(observed) != expected:
            raise ValueError("Native adjudication failed: " + json.dumps(resolved["verification_failures"], ensure_ascii=False))
        binding = {"schema_version": 1, "base_capture": resolved["base_capture"],
                   "review": record, "target_execution": False,
                   **({"previous_binding_snapshot": review["previous_binding_snapshot"]}
                      if "previous_binding_snapshot" in review else {})}
        destination = result / ("reanalysis-attempt-adjudication.json" if attempts else "reanalysis-adjudication.json")
        if destination.exists():
            if json.loads(destination.read_bytes()) != binding:
                previous_raw = destination.read_bytes()
                if (review.get("previous_binding") != identity(a, destination)
                        or checked(a, review.get("previous_binding_snapshot")) != previous_raw):
                    raise ValueError("A different native adjudication binding already exists")
                a.save(destination, binding)
        else:
            raw = (json.dumps(binding, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
            with destination.open("xb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
        output = {"schema_version": 1, "recorded_at_utc": a.stamp(), "review": record,
                  "binding": identity(a, destination), "adjudicated_attempts" if attempts else "adjudicated_functions": sorted(expected),
                  "remaining_original_failures": resolved["remaining_failures"],
                  "remaining_complete_artifact_conflicts": resolved["conflicting_function_rvas"],
                  "historical_verification_failures": resolved.get("historical_verification_failures", []),
                  "current_verification_failures": resolved["verification_failures"],
                  "target_execution": False, "semantic_completion": False,
                  "official_dll_comparison": "unverified"}
        output["tool_sources"] = []
        for source in (Path(__file__), Path(a.__file__)):
            source_raw = source.read_bytes()
            source_digest = hashlib.sha256(source_raw).hexdigest()
            retained = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (source_digest + ".py")
            retained.parent.mkdir(parents=True, exist_ok=True)
            if not retained.exists():
                with retained.open("xb") as stream:
                    stream.write(source_raw)
                    stream.flush()
                    os.fsync(stream.fileno())
            elif retained.read_bytes() != source_raw:
                raise ValueError("Native adjudication tool source receipt conflicts")
            output["tool_sources"].append(identity(a, retained))
        raw = (json.dumps(output, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
        receipt = a.ROOT / ".analysis/results/reverse-pending/review-sources" / (hashlib.sha256(raw).hexdigest() + ".json")
        receipt.parent.mkdir(parents=True, exist_ok=True)
        if not receipt.exists():
            with receipt.open("xb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
        elif receipt.read_bytes() != raw:
            raise ValueError("Native adjudication operation receipt conflicts")
        print(json.dumps({"receipt": identity(a, receipt), "adjudicated": len(expected),
                          "remaining_failures": len(output["remaining_original_failures"]),
                          "remaining_conflicts": len(output["remaining_complete_artifact_conflicts"]),
                          "current_verification_failures": len(output["current_verification_failures"]),
                          "target_execution": False}), flush=True)
