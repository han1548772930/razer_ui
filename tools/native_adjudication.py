"""Explicit, byte-bound adjudication of complete native capture alternatives."""
from __future__ import annotations

import ast
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
                or {key: value for key, value in old.items() if key not in {"text", "comment"}}
                != {key: value for key, value in new.items() if key not in {"text", "comment"}}):
            raise ValueError("Native adjudication cannot replace different original machine semantics")
        if old.get("text") != new.get("text"):
            if not isinstance(old.get("text"), str) or not isinstance(new.get("text"), str):
                raise ValueError("Native adjudication display change lacks complete original text")
            changes.append({"address": old["address"], "previous_text": old["text"], "selected_text": new["text"]})
    return changes


def assembly_comment_differences(previous, selected):
    """Retain exact per-instruction annotations separately from display text."""
    assembly_rendering_differences(previous, selected)
    changes = []
    for old, new in zip(previous["assembly"], selected["assembly"]):
        if any(row.get("comment") is not None and not isinstance(row["comment"], str) for row in (old, new)):
            raise ValueError("Native instruction annotation must be complete text or null")
        if old.get("comment") != new.get("comment") or ("comment" in old) != ("comment" in new):
            changes.append({"address": old["address"], "previous_comment": old.get("comment"),
                "selected_comment": new.get("comment"), "previous_present": "comment" in old,
                "selected_present": "comment" in new})
    return changes


def _virtual_local_proof(metadata, local, location, conversion):
    proof = local.get("virtual_register_proof", {})
    if (metadata.get("schema_version") != 2 or location.get("kind") != "register"
            or not isinstance(proof, dict) or proof.get("complete") is not True
            or proof.get("status") != "api_returned"
            or type(proof.get("processor_register")) is not int or proof["processor_register"] != -1
            or proof.get("kernel_api") != "ida_hexrays.is_kreg"
            or proof.get("processor_api") != "ida_hexrays.mreg2reg"
            or proof.get("name_api") != "ida_hexrays.get_mreg_name"
            or proof.get("register_namespace") != "hexrays_microregisters"
            or type(proof.get("register")) is not int or proof["register"] < 0
            or proof["register"] != location.get("register")
            or type(proof.get("register_offset")) is not int or proof["register_offset"] < 0
            or proof["register_offset"] != location.get("register_offset")
            or type(proof.get("width")) is not int or proof["width"] != local["width"]
            or not isinstance(proof.get("name"), str) or not proof["name"].strip()):
        raise ValueError("Adjudicated virtual local lacks exact live SDK register identity")
    reason = conversion.get("reason")
    if reason == "sdk_kernel_register_has_no_processor_location":
        valid = proof.get("is_kernel_register") is True
    elif reason == "sdk_temporary_register_has_no_processor_location":
        valid = (proof.get("is_kernel_register") is False and proof.get("is_codegen_temporary") is True
                 and proof["register_offset"] == 0
                 and proof.get("temporary_api") == "ida_hexrays.get_temp_regs"
                 and proof.get("membership_api") == "mlist_t.has_all")
    elif reason == "sdk_bit_register_has_no_processor_location":
        valid = (proof.get("is_kernel_register") is False and proof.get("is_codegen_temporary") is False
                 and proof.get("temporary_api") == "ida_hexrays.get_temp_regs"
                 and proof.get("membership_api") == "mlist_t.has_all"
                 and proof.get("operand_api") == "ida_hexrays.mop_t.make_reg"
                 and type(proof.get("operand_register")) is int and proof["operand_register"] == proof["register"]
                 and type(proof.get("operand_width")) is int and proof["operand_width"] == local["width"] == 1
                 and proof["register_offset"] == 0 and proof.get("operand_is_exact_register") is True
                 and proof.get("bit_api") == "ida_hexrays.mop_t.is_bit_reg"
                 and proof.get("condition_api") == "ida_hexrays.mop_t.is_cc"
                 and type(proof.get("is_condition_code")) is bool and proof.get("is_bit_register") is True)
    else:
        valid = False
    if not valid:
        raise ValueError("Adjudicated virtual local lacks exact SDK register class proof")


def _physical_tbyte_local_proof(metadata, local, location, conversion):
    proof = local.get("physical_register_proof", {})
    apis = {"type_float": "tinfo_t.is_float", "type_floating": "tinfo_t.is_floating",
        "type_tbyte": "tinfo_t.is_tbyte", "type_size": "tinfo_t.get_size",
        "operand": "ida_hexrays.mop_t.make_reg", "operand_exact": "mop_t.is_reg",
        "bit": "mop_t.is_bit_reg", "condition": "mop_t.is_cc",
        "operand_kernel": "mop_t.is_kreg", "kernel": "ida_hexrays.is_kreg",
        "temporary": "ida_hexrays.get_temp_regs; mlist_t.has_all",
        "processor": "ida_hexrays.mreg2reg", "identity_roundtrip": "ida_hexrays.reg2mreg",
        "processor_name": "ida_idp.get_reg_name", "processor_info": "ida_idp.parse_reg_name; reg_info_t.reg/size",
        "tbyte_size": "ida_idp.ph_get_tbyte_size"}
    if (metadata.get("schema_version") != 2 or location.get("kind") != "register"
            or not isinstance(proof, dict) or proof.get("status") != "api_returned"
            or proof.get("complete") is not True or proof.get("apis") != apis
            or proof.get("register_namespace") != "hexrays_microregisters"
            or type(proof.get("register")) is not int or proof["register"] < 0
            or type(location.get("register")) is not int or proof["register"] != location["register"]
            or type(proof.get("register_offset")) is not int or proof["register_offset"] != 0
            or type(location.get("register_offset")) is not int or location["register_offset"] != 0
            or any(type(proof.get(key)) is not int or proof[key] != expected for key, expected in {
                "width": 10, "type_bytes": 10, "operand_register": proof["register"], "operand_width": 10,
                "processor_register_at_original_width": -1, "identity_width": 8,
                "identity_roundtrip_microregister": proof["register"], "processor_name_width": 10,
                "processor_capacity_bytes": 10, "processor_tbyte_bytes": 10}.items())
            or local["width"] != 10 or proof.get("type_is_float") is not False
            or proof.get("type_is_floating") is not True or proof.get("type_is_tbyte") is not True
            or proof.get("operand_is_exact_register") is not True or proof.get("identity_mapping_only") is not True
            or any(proof.get(key) is not False for key in ("is_kernel_register", "is_codegen_temporary",
                "operand_is_kernel_register", "is_bit_register", "is_condition_code"))
            or type(proof.get("identity_processor_register")) is not int or proof["identity_processor_register"] < 0
            or not isinstance(proof.get("processor_name"), str) or not proof["processor_name"].strip()
            or proof.get("processor_parsed") is not True or type(proof.get("processor_parsed_register")) is not int
            or proof["processor_parsed_register"] != proof["identity_processor_register"]
            or "virtual_register_proof" in local
            or conversion.get("reason") != "sdk_x87_tbyte_capacity_and_identity_roundtrip"
            or "roundtrip_matches" in conversion):
        raise ValueError("Adjudicated extended local lacks exact independent physical SDK register proof")
    original = conversion.get("original_conversion", {})
    if (not isinstance(original, dict) or original.get("status") != "api_returned"
            or original.get("complete") is not False or original.get("roundtrip_matches") is not False
            or original.get("conversion_api") != "mba_t.vd2idaloc" or original.get("roundtrip_api") != "mba_t.idaloc2vd"):
        raise ValueError("Adjudicated extended local omitted its failed original SDK location conversion")
    ida, back = original.get("location", {}), original.get("roundtrip_location", {})
    if (not isinstance(ida, dict) or not isinstance(back, dict) or ida.get("complete") is not True
            or ida.get("status") != "observed" or ida.get("register_namespace") != "ida_processor_registers"
            or ida.get("kind") != "scattered" or ida.get("part_count") != 2
            or not isinstance(ida.get("parts"), list) or len(ida["parts"]) != 2
            or back.get("complete") is not True or back.get("status") != "observed"
            or back.get("register_namespace") != "hexrays_microregisters"
            or back.get("kind") != "scattered" or back.get("part_count") != 1
            or not isinstance(back.get("parts"), list) or len(back["parts"]) != 1
            or back == {key: value for key, value in location.items() if key != "text"}):
        raise ValueError("Adjudicated extended local changed its original scattered conversion observations")
    for record, namespace, sizes in ((ida, "ida_processor_registers", (8, 2)),
                                     (back, "hexrays_microregisters", (10,))):
        offset = 0
        for part, size in zip(record["parts"], sizes):
            if (not isinstance(part, dict) or part.get("complete") is not True or part.get("status") != "observed"
                    or part.get("register_namespace") != namespace or part.get("kind") != "register"
                    or type(part.get("register")) is not int or part["register"] < 0
                    or any(type(part.get(key)) is not int or part[key] != expected for key, expected in {
                        "register_offset": 0, "argument_offset": offset, "size": size}.items())):
                raise ValueError("Adjudicated extended local lacks its exact original 8+2 byte SDK observations")
            offset += size
    if ida["parts"][0]["register"] != proof["identity_processor_register"]:
        raise ValueError("Adjudicated extended local maps a different physical processor register")
    if back["parts"][0]["register"] != proof["register"]:
        raise ValueError("Adjudicated extended local omitted its actual original microregister mapping")


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
            _virtual_local_proof(metadata, local, location, conversion)
        elif conversion.get("status") == "physical_register_verified":
            _physical_tbyte_local_proof(metadata, local, location, conversion)
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
            comment_changes = assembly_comment_differences(old, body)
            comment_reviews = comparison.get("assembly_comment_differences", [])
            if (not isinstance(comment_reviews, list)
                    or any(not {"address", "previous_comment", "selected_comment", "previous_present",
                                "selected_present"}.issubset(row)
                           or type(row.get("previous_present")) is not bool
                           or type(row.get("selected_present")) is not bool for row in comment_reviews)
                    or [{key: row.get(key) for key in ("address", "previous_comment", "selected_comment",
                                                     "previous_present", "selected_present")}
                        for row in comment_reviews] != comment_changes
                    or any(not isinstance(row.get("reason"), str) or not row["reason"].strip()
                           for row in comment_reviews)):
                raise ValueError("Native adjudication omitted exact instruction annotation changes or their reasons")
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


def recursive_context_coverage(a, original_raw, base, manifest, directory):
    """Bind a recursive entry to a typed capture of its complete original root context."""
    import native_boundaries

    requests = manifest.get("function_selection", {}).get("recursive_fallthrough_contexts", [])
    recoveries = manifest.get("recursive_context_recoveries", [])
    if not requests:
        if recoveries or manifest.get("context_helpers"):
            raise ValueError("Native recursive context has unselected captures")
        return {}
    if not isinstance(requests, list) or not isinstance(recoveries, list) or len(requests) != len(recoveries):
        raise ValueError("Native recursive context omitted a requested recovery")
    native_boundaries.verify_recursive_contexts(a, requests, recoveries)
    if (manifest.get("target_execution") is not False or manifest.get("failures") != []
            or manifest.get("capture_complete") is not True or manifest.get("all_pseudocode_captured") is not True
            or manifest.get("input_sha256") != hashlib.sha256(original_raw).hexdigest()
            or base.get("input_sha256") != manifest["input_sha256"]):
        raise ValueError("Native recursive context is not a complete static capture of the original image")
    base_rows = base.get("functions", [])
    base_directory = a.ROOT / ".analysis/results/native" / base["input_sha256"]
    index = manifest.get("context_helpers", [])
    if not isinstance(index, list) or len(index) != len(requests):
        raise ValueError("Native recursive context omitted complete physical core captures")
    roots, cores, result = set(), set(), {}

    def captured(record, rva, group, pseudocode):
        expected = directory / group / (rva[2:] + ".json")
        if not isinstance(record, dict) or record.get("path") != a.rel(expected):
            raise ValueError("Native recursive context artifact is outside its canonical capture index")
        body = json.loads(checked(a, record, a.rel(directory)))
        rows = manifest.get("functions", []) if pseudocode else index
        matches = [row for row in rows if row.get("rva") == rva]
        if (len(matches) != 1 or matches[0].get("path") != group + "/" + rva[2:] + ".json"
                or matches[0].get("sha256") != record["sha256"]
                or ("bytes" in matches[0] and matches[0]["bytes"] != record["bytes"])
                or matches[0].get("address") != body.get("address")
                or matches[0].get("assembly_captured") is not True
                or matches[0].get("pseudocode_captured") is not pseudocode
                or body.get("rva") != rva):
            raise ValueError("Native recursive context artifact differs from its unique physical capture index")
        original_machine(original_raw, body)
        return body

    def original(record, rva):
        expected = base_directory / "functions" / (rva[2:] + ".json")
        if not isinstance(record, dict) or record.get("path") != a.rel(expected):
            raise ValueError("Native recursive context original is outside the base capture")
        body = json.loads(checked(a, record, a.rel(base_directory)))
        matches = [row for row in base_rows if row.get("rva") == rva]
        if (len(matches) != 1 or matches[0].get("path") != "functions/" + rva[2:] + ".json"
                or matches[0].get("sha256") != record["sha256"] or body.get("rva") != rva):
            raise ValueError("Native recursive context original differs from its complete base index")
        original_machine(original_raw, body)
        return body

    for request, recovery in zip(requests, recoveries):
        root_rva, core_rva = request.get("root_rva"), request.get("core_rva")
        if (not isinstance(root_rva, str) or not re.fullmatch(r"0x[0-9a-f]+", root_rva)
                or not isinstance(core_rva, str) or not re.fullmatch(r"0x[0-9a-f]+", core_rva)
                or root_rva in roots or core_rva in cores or root_rva == core_rva
                or root_rva in cores or core_rva in roots or recovery.get("request") != request):
            raise ValueError("Native recursive context has duplicate, overlapping or different owners")
        roots.add(root_rva)
        cores.add(core_rva)
        old_root = original(request.get("root_function"), root_rva)
        old_core = original(request.get("core_function"), core_rva)
        root = captured(recovery.get("root_capture"), root_rva, "functions", True)
        core = captured(recovery.get("core_capture"), core_rva, "context-helpers", False)
        if (not _same_machine(old_root, root) or not _same_machine(old_core, core)
                or core.get("pseudocode") is not None):
            raise ValueError("Native recursive context changed or relabeled a physical original owner")
        _typed_function(root)
        pseudo = root["pseudocode"]
        if (not isinstance(pseudo.get("lines"), list) or not pseudo["lines"]
                or pseudo.get("entry") != root["address"] or recovery.get("warnings") != []
                or ("hexrays_diagnostics" in root and (
                    root["hexrays_diagnostics"].get("diagnostics_complete") is not True
                    or root["hexrays_diagnostics"].get("diagnostic_error_count") != 0
                    or root["hexrays_diagnostics"].get("decompile_outcome") != "returned_cfunc"))):
            raise ValueError("Native recursive context lacks complete root pseudocode or diagnostics")
        proof = request.get("proof", {})
        union = proof.get("original_machine_union", {})
        chunks = sorted([*old_root["chunks"], *old_core["chunks"]], key=lambda row: int(row["start"], 16))
        if (len(chunks) != 2 or old_root["end"] != old_core["address"]
                or chunks[0]["start"] != old_root["address"] or chunks[0]["end"] != old_core["address"]
                or chunks[1]["start"] != old_core["address"] or chunks[1]["end"] != old_core["end"]):
            raise ValueError("Native recursive context does not preserve its contiguous complete original owners")
        union_raw = b"".join(bytes.fromhex(chunk["raw_hex"]) for chunk in chunks)
        if (any(union.get(key) != value for key, value in {
                "start": old_root["address"], "end": old_core["end"], "bytes": len(union_raw),
                "sha256": hashlib.sha256(union_raw).hexdigest(),
                "instructions": len(old_root["assembly"]) + len(old_core["assembly"])}.items())
                or proof.get("original_fallthrough") != old_root["assembly"][-1]):
            raise ValueError("Native recursive context omitted or changed the original machine union")
        fallthrough = old_root["assembly"][-1]
        if (int(fallthrough["address"], 16) + len(bytes.fromhex(fallthrough["bytes"]))
                != int(old_core["address"], 16)
                or not any(edge.get("from") == fallthrough["address"] and edge.get("to") == old_core["address"]
                    and edge.get("type") == 21 and edge.get("code") is True
                    for edge in fallthrough.get("xrefs_from", []))):
            raise ValueError("Native recursive context lacks the original physical fallthrough")
        calls = [row for row in old_core["assembly"] if bytes.fromhex(row["bytes"])[:1] == b"\xe8"]
        returns = [row for body in (old_root, old_core) for row in body["assembly"]
                   if bytes.fromhex(row["bytes"])[:1] == b"\xc3"]
        if (proof.get("recursive_call_instructions") != calls
                or proof.get("all_original_return_instructions") != returns or not calls or not returns):
            raise ValueError("Native recursive context omitted original recursive calls or returns")
        base_address = int(base["image_base"], 16)
        for call in calls:
            code = bytes.fromhex(call["bytes"])
            if len(code) != 5 or int(call["address"], 16) + 5 + int.from_bytes(code[1:], "little", signed=True) != base_address + int(core_rva, 16):
                raise ValueError("Native recursive context CALL does not target its original separate core")
        result[core_rva] = {"coverage_kind": "verified_original_root_recursive_context",
            "original_function": request["core_function"], "complete_root": recovery["root_capture"],
            "complete_core_assembly": recovery["core_capture"], "request": request,
            "all_original_bytes_preserved": True, "all_original_calls_preserved": True,
            "numerical_semantics_complete": False, "target_execution": False}
    return result


def finalization_coverage(a, result, capture_path, diagnostic_record):
    """Independently check complete exports after a serializer identity guard failed."""
    from reverse_pending import process_alive

    capture_path = a.resolve(a.rel(capture_path), a.rel(result / "supplements"))
    manifest = json.loads(capture_path.read_bytes())
    output = capture_path.parent
    session_path, plan_path = output / "session.json", output / "plan.json"
    session, plan = json.loads(session_path.read_bytes()), json.loads(plan_path.read_bytes())
    base_path = result / "capture.json"
    base = json.loads(base_path.read_bytes())
    source = a.resolve(base["input"], ".analysis/inputs")
    source_record = identity(a, source)
    selection = manifest.get("function_selection", {})
    targets = selection.get("targets", [])
    records = manifest.get("functions", [])
    diagnostic = checked(a, diagnostic_record)
    terminal = b"ValueError: Typed capture serializer changed after it was imported"
    if (diagnostic.count(b"[STATIC-SUPPLEMENT] fatal failure:") != 1
            or terminal not in diagnostic or diagnostic.count(b"Traceback (most recent call last):") != 1
            or b"\nValueError:" in diagnostic.split(terminal)[1]
            or b"\nTraceback" in diagnostic.split(terminal)[1]):
        raise ValueError("Export finalization requires the complete exact serializer identity failure")
    if (manifest.get("capture_complete") is not False or manifest.get("all_pseudocode_captured") is not False
            or manifest.get("failures") != [] or session.get("passed") is not False
            or type(session.get("exit_code")) is not int or session["exit_code"] != 1
            or not session.get("finished_at_utc") or process_alive(session.get("pid")) is not False
            or session.get("capture_tool") != manifest.get("capture_tool")
            or not isinstance(manifest.get("capture_tool", {}).get("sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", manifest["capture_tool"]["sha256"])
            or manifest.get("database_integrity", {}).get("verified") is not True
            or manifest.get("database_integrity", {}).get("records_with_issues") != 0
            or any(record.get("target_execution") is not False for record in (manifest, session, plan))
            or any(record.get("real_device_operations") is not False for record in (manifest, session, plan))):
        raise ValueError("Export finalization requires an ended static session and its unchanged failure outcome")
    expected_base = {"path": a.rel(base_path), "sha256": a.sha(base_path)}
    if (manifest.get("input_sha256") != source_record["sha256"] or base.get("input_sha256") != source_record["sha256"]
            or base.get("input_bytes") != source_record["bytes"] or plan.get("input_bytes") != source_record["bytes"]
            or plan.get("input") != source_record["path"] or plan.get("sha256") != source_record["sha256"]
            or manifest.get("base_capture") != expected_base or plan.get("base_capture") != expected_base
            or plan.get("function_selection") != selection or manifest.get("database") != plan.get("database")
            or session.get("database") != plan.get("database")
            or manifest.get("source_database") != plan.get("source_database")
            or selection.get("mode") != "retained_function_reanalysis"
            or any(selection.get(key) for key in ("static_observation", "function_merges", "function_partitions",
                "function_shared_tails", "function_fresh_inlines", "function_boundary_recoveries",
                "instruction_boundary_recoveries", "decompiler_prepasses", "callsite_type_recoveries",
                "type_owner_reanalysis", "import_type_recoveries"))):
        raise ValueError("Export finalization changed its exact simple retained-function plan or original input")
    source_database = plan.get("source_database", {})
    source_db_path = a.resolve(source_database.get("path", ""), ".work/native")
    if a.sha(source_db_path) != source_database.get("sha256"):
        raise ValueError("Export finalization original source database changed")
    if not a.verify_native_typed_serializer(manifest):
        raise ValueError("Export finalization lacks its exact retained typed serializer source")
    requested = [row["rva"] for row in targets]
    if (not requested or len(requested) != len(set(requested)) or len(records) != len(requested)
            or {row.get("rva") for row in records} != set(requested)
            or any(row.get("assembly_captured") is not True or row.get("pseudocode_captured") is not True
                   for row in records)):
        raise ValueError("Export finalization omitted an exact requested full function")
    chains = manifest.get("chains", [])
    if (len(chains) != len(requested) or [row.get("requested_rva") for row in chains] != requested
            or any(row.get("complete") is not True or row.get("entries") != [{
                "address": target["address"], "rva": target["rva"], "direct_jump_target_rva": None}]
                   for row, target in zip(chains, targets))):
        raise ValueError("Export finalization omitted its complete retained target chains")
    raw = source.read_bytes()
    base_records = {row["rva"]: row for row in base["functions"]}
    observed = []
    for row in records:
        path = a.resolve(a.rel(output / row["path"]), a.rel(output))
        current = identity(a, path)
        if current["sha256"] != row.get("sha256"):
            raise ValueError("Export finalization function bytes changed")
        body = json.loads(path.read_bytes())
        original = base_records.get(row["rva"], {})
        original_path = a.resolve(a.rel(result / original.get("path", "")), a.rel(result))
        if a.sha(original_path) != original.get("sha256"):
            raise ValueError("Export finalization original full function bytes changed")
        previous = json.loads(original_path.read_bytes())
        if not _same_machine(previous, body):
            raise ValueError("Export finalization changed complete original function boundaries or instructions")
        original_machine(raw, body)
        pseudo = body.get("pseudocode", {})
        if (not isinstance(pseudo, dict) or not pseudo.get("lines") or not isinstance(pseudo.get("locals"), list)
                or not isinstance(pseudo.get("locals_capture"), dict)
                or pseudo["locals_capture"].get("schema_version") != 2):
            raise ValueError("Export finalization lacks complete live schema 2 pseudocode observations")
        try:
            _typed_function(body)
            typed = {"complete": True}
        except (ValueError, KeyError, TypeError) as error:
            typed = {"complete": False, "error": str(error)}
        observed.append({"rva": row["rva"], "function": current,
                         "original_function": identity(a, original_path), "typed_validation": typed})
    return {"capture": identity(a, capture_path), "session": identity(a, session_path), "plan": identity(a, plan_path),
            "failure_diagnostic": diagnostic_record, "source_input": source_record,
            "base_capture": identity(a, base_path), "source_database": identity(a, source_db_path),
            "typed_serializer_source": a.verify_native_typed_serializer(manifest), "functions": observed,
            "original_session_exit_code": 1, "original_session_passed": False,
            "capture_tool": manifest["capture_tool"], "capture_tool_source_verified": False,
            "recovery_basis": "independent_original_bytes_and_complete_export_observations",
            "target_execution": False, "semantic_completion": False}


def finalized_export(a, result, capture_path):
    binding_path = capture_path.parent / "finalization-review.json"
    if not binding_path.is_file():
        return None
    binding = json.loads(binding_path.read_bytes())
    review_record = binding.get("review")
    review = json.loads(checked(a, review_record))
    if (binding.get("schema_version") != 1 or binding.get("target_execution") is not False
            or review.get("schema_version") != 1 or review.get("review_kind") != "native_completed_export_finalization_review"
            or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
            or review.get("historical_session_outcome_preserved") is not True
            or not isinstance(review.get("reason"), str) or not review["reason"].strip()):
        raise ValueError("Export finalization review does not preserve the failed original session")
    _evidence(a, review.get("evidence"))
    expected = finalization_coverage(a, result, capture_path, review["finalization"]["failure_diagnostic"])
    if review.get("finalization") != expected:
        raise ValueError("Independent export finalization evidence or per-function validation changed")
    return {"review": review_record, "binding": identity(a, binding_path), **expected}


def finalize(a, args):
    review_path = a.resolve(args.review, ".analysis/results")
    if a.sha(review_path) != args.review_sha256:
        raise ValueError("Export finalization review SHA differs from the explicit reviewed bytes")
    review = json.loads(review_path.read_bytes())
    capture_record = review.get("finalization", {}).get("capture")
    capture_path = a.resolve(capture_record["path"], ".analysis/results/native")
    result = capture_path.parents[2]
    source = a.resolve(args.input, ".analysis/inputs")
    if identity(a, source) != review.get("finalization", {}).get("source_input"):
        raise ValueError("Export finalization command input differs from its exact original")
    binding = {"schema_version": 1, "review": identity(a, review_path), "target_execution": False}
    destination = capture_path.parent / "finalization-review.json"
    with a.index_lock(a.ROOT / ".analysis/reverse-pending.lock"):
        # Validate the same immutable review before publishing its sole binding.
        expected = finalization_coverage(a, result, capture_path, review["finalization"]["failure_diagnostic"])
        if (review.get("schema_version") != 1 or review.get("review_kind") != "native_completed_export_finalization_review"
                or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
                or review.get("historical_session_outcome_preserved") is not True
                or not isinstance(review.get("reason"), str) or not review["reason"].strip()
                or review.get("finalization") != expected):
            raise ValueError("Independent export finalization review is incomplete or changed")
        _evidence(a, review.get("evidence"))
        if destination.exists():
            if json.loads(destination.read_bytes()) != binding:
                raise ValueError("A different immutable export finalization binding already exists")
        else:
            a.save(destination, binding)
    print(json.dumps({"binding": identity(a, destination), "functions": len(expected["functions"]),
        "typed_complete": sum(row["typed_validation"]["complete"] for row in expected["functions"]),
        "original_session_passed": False, "target_execution": False}), flush=True)


def recursive_context_attempt_coverage(a, original_raw, base, previous, replacement, manifest, directory):
    """Cover an old physical core or full merged union with its real root context."""
    contexts = recursive_context_coverage(a, original_raw, base, manifest, directory)
    rva = previous.get("rva")
    matches = [context for core, context in contexts.items()
               if rva in {core, context["request"]["root_rva"]}]
    if len(matches) != 1:
        raise ValueError("Failed native attempt lacks a unique verified recursive root context")
    context = matches[0]
    request = context["request"]
    root = json.loads(checked(a, request["root_function"], ".analysis/results/native"))
    core = json.loads(checked(a, request["core_function"], ".analysis/results/native"))
    captured_root = json.loads(checked(a, context["complete_root"], a.rel(directory)))
    if not _same_machine(replacement, captured_root):
        raise ValueError("Failed native attempt replacement is not the exact verified root capture")
    _typed_function(replacement)
    if rva == request["core_rva"]:
        if not _same_machine(previous, core):
            raise ValueError("Failed native attempt core differs from its original physical owner")
        kind = "original_physical_core"
    else:
        raw = b"".join(bytes.fromhex(row["raw_hex"]) for body in (root, core) for row in body["chunks"])
        merged = {**root, "end": core["end"],
            "chunks": [{"start": root["address"], "end": core["end"], "raw_hex": raw.hex(),
                        "sha256": hashlib.sha256(raw).hexdigest()}],
            "assembly": [*root["assembly"], *core["assembly"]]}
        if not _same_machine(previous, merged):
            raise ValueError("Failed native attempt merged owner differs from the complete original root/core union")
        kind = "complete_original_root_core_union"
    original_machine(original_raw, previous)
    original_machine(original_raw, replacement)
    return {"coverage_kind": "verified_original_root_recursive_context", "previous_machine_kind": kind,
            "complete_context": context, "all_original_bytes_preserved": True,
            "all_original_calls_preserved": True, "core_relabelled_as_root": False,
            "numerical_semantics_complete": False, "target_execution": False}


def shared_tail_attempt_coverage(a, original_raw, base, previous, replacement, manifest, directory):
    """Bind an old owner to its complete byte- and edge-preserving shared tail."""
    from host import inspect_pe

    selection = manifest.get("function_selection", {})
    shared_tails = selection.get("function_shared_tails", [])
    targets = [row["rva"] for row in selection.get("targets", [])]
    matches = [row for row in shared_tails if previous.get("rva") in row.get("owner_rvas", [])]
    if (len(matches) != 1 or previous.get("rva") not in targets or len(set(targets)) != len(targets)
            or manifest.get("target_execution") is not False or manifest.get("capture_complete") is not True
            or manifest.get("all_pseudocode_captured") is not True or manifest.get("failures") != []
            or manifest.get("input_sha256") != hashlib.sha256(original_raw).hexdigest()):
        raise ValueError("Failed native attempt lacks a unique complete static shared-tail owner")
    recomputed = a.native_function_shared_tails(original_raw, base, [int(rva, 16) for rva in targets],
        shared_tails, None, inspect_pe(original_raw))
    if recomputed != shared_tails:
        raise ValueError("Native attempt shared tail differs from its complete original-byte and edge review")
    applied = manifest.get("function_shared_tail_recoveries", [])
    if not isinstance(applied, list) or len(applied) != len(shared_tails):
        raise ValueError("Native attempt shared tail omitted an exact ownership recovery")

    def same_edges(left, right):
        return [(ins["address"], ins.get("xrefs_from"), ins.get("xrefs_to")) for ins in left["assembly"]] == [
            (ins["address"], ins.get("xrefs_from"), ins.get("xrefs_to")) for ins in right["assembly"]]

    originals, baselines, helpers = {}, {}, {}
    for shared, recovery in zip(shared_tails, applied):
        if (not isinstance(recovery, dict) or any(recovery.get(key) != value for key, value in shared.items())
                or recovery.get("base_database_modified") is not False
                or recovery.get("applied_tail") != shared["expected_tail"]
                or recovery.get("applied_primary_owner_rva") != shared["primary_owner_rva"]
                or recovery.get("applied_owner_rvas") != shared["owner_rvas"]
                or not isinstance(recovery.get("applied_owners"), list)
                or len(recovery["applied_owners"]) != len(shared["owners"])):
            raise ValueError("Native attempt shared tail changed its reviewed ownership or original bytes")
        for owner in shared["owners"]:
            source = json.loads(checked(a, owner["source_function"], ".analysis/results/native"))
            original_machine(original_raw, source)
            originals[owner["rva"]] = source
        primary = originals[shared["primary_owner_rva"]]
        tail = shared["expected_tail"]
        tail_assembly = [ins for ins in primary["assembly"]
                         if int(tail["start"], 16) <= int(ins["address"], 16) < int(tail["end"], 16)]
        for owner, actual in zip(shared["owners"], recovery["applied_owners"]):
            outline_flag = actual.get("outline_flag")
            if type(outline_flag) is not int or outline_flag <= 0 or outline_flag & (outline_flag - 1):
                raise ValueError("Native attempt shared-tail outline flag is not one exact SDK flag")
            flags = owner["expected_flags"] | outline_flag if shared["inline_owners"] else owner["expected_flags"]
            if (actual.get("rva") != owner["rva"] or actual.get("end") != owner["expected_end"]
                    or actual.get("chunks") != owner["expected_chunks"]
                    or actual.get("base_database_modified") is not False
                    or type(actual.get("previous_flags")) is not int
                    or actual["previous_flags"] != owner["expected_flags"]
                    or type(actual.get("applied_flags")) is not int or actual["applied_flags"] != flags):
                raise ValueError("Native attempt shared-tail owner changed complete chunks or original flags")
            body = copy.deepcopy(originals[owner["rva"]])
            body.update(end=owner["expected_end"], chunks=owner["expected_chunks"], flags=flags)
            if owner["rva"] != shared["primary_owner_rva"]:
                addresses = {ins["address"] for ins in body["assembly"]}
                body["assembly"] = sorted([*body["assembly"],
                    *(ins for ins in tail_assembly if ins["address"] not in addresses)],
                    key=lambda ins: int(ins["address"], 16))
            baselines[owner["rva"]] = body
            if owner["rva"] not in targets:
                helpers[owner["rva"]] = body

    original, expected = originals[previous["rva"]], baselines[previous["rva"]]
    if (not _same_machine(previous, original) or not same_edges(previous, original)
            or not _same_machine(replacement, expected) or not same_edges(replacement, expected)
            or replacement.get("flags") != expected["flags"]):
        raise ValueError("Native attempt shared-tail replacement changed original owner bytes or complete edges")
    _typed_function(replacement)
    helper_records = manifest.get("shared_tail_helpers", [])
    if (not isinstance(helper_records, list) or len(helper_records) != len(helpers)
            or {row.get("rva") for row in helper_records} != set(helpers)):
        raise ValueError("Native attempt shared tail omitted complete unselected owner assemblies")
    verified_helpers = []
    for row in helper_records:
        rva = row["rva"]
        expected_path = "shared-tail-helpers/" + rva[2:] + ".json"
        if row.get("path") != expected_path:
            raise ValueError("Native attempt shared-tail helper is outside its canonical capture index")
        path = a.resolve(a.rel(directory / expected_path), a.rel(directory))
        record = identity(a, path)
        body = json.loads(checked(a, record, a.rel(directory)))
        if (record["sha256"] != row.get("sha256") or ("bytes" in row and record["bytes"] != row["bytes"])
                or row.get("address") != body.get("address")
                or row.get("assembly_captured") is not True or row.get("pseudocode_captured") is not False
                or body.get("pseudocode") is not None or not _same_machine(body, helpers[rva])
                or not same_edges(body, helpers[rva]) or body.get("flags") != helpers[rva]["flags"]):
            raise ValueError("Native attempt shared-tail helper changed complete original instructions or edges")
        original_machine(original_raw, body)
        verified_helpers.append(record)
    original_machine(original_raw, previous)
    original_machine(original_raw, replacement)
    shared = matches[0]
    owner = next(row for row in shared["owners"] if row["rva"] == previous["rva"])
    return {"coverage_kind": "reviewed_complete_shared_tail_owner", "original_function": owner["source_function"],
            "complete_shared_tail": shared, "complete_helper_assemblies": verified_helpers,
            "all_original_bytes_preserved": True, "all_original_edges_preserved": True,
            "helper_semantics_or_abi_completion": False, "target_execution": False}


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


def entry_partition_attempt_coverage(a, original_raw, base, previous, replacement, manifest, directory):
    """Retain every original x86 entry byte, edge and complete typed helper."""
    import native_boundaries

    selection = manifest.get("function_selection", {})
    plans = selection.get("x86_entry_partitions", [])
    rows = [row for row in plans if row.get("owner_rva") == previous.get("rva")]
    if len(rows) != 1:
        raise ValueError("Native attempt lacks a unique exact complete x86 entry partition")
    specifications = [row["review_source"]["path"] + "#" + row["review_source"]["pointer"] for row in plans]
    recomputed = native_boundaries.select_entry_partitions(a, original_raw, base,
        [int(row["rva"], 16) for row in selection.get("targets", [])], specifications,
        [row["rva"] for row in selection.get("function_outlines", [])])
    if recomputed != plans:
        raise ValueError("Native attempt entry partition differs from its complete original-byte and incoming-edge review")
    native_boundaries.verify_entry_partitions(a, plans, manifest.get("x86_entry_partition_recoveries"))
    verified = native_boundaries.verify_entry_helpers(a, original_raw, plans,
        manifest.get("entry_partition_helpers"), directory)
    row = rows[0]
    original = json.loads(checked(a, row["owner_function"], ".analysis/results/native"))
    expected = native_boundaries.entry_partition_bodies(a, row)

    def same_edges(left, right):
        return [(ins["address"], ins.get("xrefs_to"), ins.get("xrefs_from")) for ins in left["assembly"]] == [
            (ins["address"], ins.get("xrefs_to"), ins.get("xrefs_from")) for ins in right["assembly"]]

    if (not _same_machine(previous, original) or not _same_machine(replacement, expected[0])
            or not same_edges(previous, original) or not same_edges(replacement, expected[0])):
        raise ValueError("Native attempt original or replacement differs from its complete x86 entry partition machine or edges")
    helpers, observed_bodies = [], [replacement]
    for body in expected[1:]:
        records = [record for record in verified if record["rva"] == body["rva"]]
        if len(records) != 1:
            raise ValueError("Native attempt entry partition omitted a complete typed machine entry")
        record = {key: records[0][key] for key in ("path", "bytes", "sha256")}
        observed = json.loads(checked(a, record, ".analysis/results/native"))
        if not _same_machine(observed, body) or not same_edges(observed, body):
            raise ValueError("Native attempt typed entry helper changed original machine bytes or incoming edges")
        helpers.append(record)
        observed_bodies.append(observed)
    old_bytes = {int(chunk["start"], 16) + index: value for chunk in previous["chunks"]
                 for index, value in enumerate(bytes.fromhex(chunk["raw_hex"]))}
    kept_bytes = {int(chunk["start"], 16) + index: value for body in observed_bodies
                  for chunk in body["chunks"] for index, value in enumerate(bytes.fromhex(chunk["raw_hex"]))}
    if old_bytes != kept_bytes:
        raise ValueError("Native attempt entry partition lost or changed the complete original byte union")
    for body in [previous, *observed_bodies]:
        original_machine(original_raw, body)
    return {"coverage_kind": "reviewed_complete_x86_entry_partition", "original_function": row["owner_function"],
            "complete_entry_partition": row, "complete_typed_entry_helpers": helpers,
            "all_original_bytes_preserved": True, "all_original_edges_preserved": True,
            "helper_semantics_or_abi_completion": False}


def boundary_attempt_coverage(a, original_raw, base, previous, replacement, manifest, directory):
    """Prove all old owner bytes survive in corrected owners and a complete helper."""
    import native_boundaries
    selections = manifest.get("function_selection", {})
    rows = [row for row in selections.get("function_boundary_recoveries", [])
            if any(owner["owner_rva"] == previous.get("rva") for owner in row.get("shared_owners", []))]
    if len(rows) != 1:
        raise ValueError("Different native boundaries lack one exact exception-backed helper recovery")
    selected = [int(row["rva"], 16) for row in selections.get("targets", [])]
    plans = selections["function_boundary_recoveries"]
    recomputed = native_boundaries.select(a, original_raw, base, selected,
        [row["review_source"]["path"] + "#" + row["review_source"]["pointer"] for row in plans])
    if recomputed != plans:
        raise ValueError("Native attempt boundary recovery differs from original PE exception evidence")
    native_boundaries.verify_applied(a, plans, manifest.get("function_boundary_recoveries"))
    row = rows[0]
    owners = [owner for owner in row["shared_owners"] if owner["owner_rva"] == previous["rva"]]
    if len(owners) != 1:
        raise ValueError("Native attempt boundary recovery omitted the exact original owner")
    original = json.loads(checked(a, owners[0]["owner_function"], ".analysis/results/native"))
    expected = native_boundaries.owner_body(a, row, previous["rva"])
    if not _same_machine(previous, original) or not _same_machine(replacement, expected):
        raise ValueError("Native attempt original or replacement differs from its complete boundary proof")
    helper = native_boundaries.helper_body(row)
    records = [record for record in manifest.get("boundary_helpers", []) if record.get("rva") == helper["rva"]]
    if len(records) != 1:
        raise ValueError("Native attempt boundary recovery omitted its complete helper assembly")
    record = records[0]
    path = a.resolve(a.rel(directory / record["path"]), a.rel(directory))
    observed = json.loads(path.read_bytes())
    if (a.sha(path) != record["sha256"] or record.get("assembly_captured") is not True
            or record.get("pseudocode_captured") is not False or observed.get("pseudocode") is not None
            or any(observed.get(key) != helper.get(key) for key in ("address", "rva", "end", "chunks"))
            or [(ins["address"], ins["bytes"]) for ins in observed.get("assembly", [])]
            != [(ins["address"], ins["bytes"]) for ins in helper["assembly"]]):
        raise ValueError("Native attempt recovered helper differs from its complete original instructions")
    original_machine(original_raw, previous)
    original_machine(original_raw, replacement)
    original_machine(original_raw, observed)
    old_bytes = {int(chunk["start"], 16) + index: value for chunk in previous["chunks"]
                 for index, value in enumerate(bytes.fromhex(chunk["raw_hex"]))}
    kept_bytes = {int(chunk["start"], 16) + index: value for body in (replacement, observed)
                  for chunk in body["chunks"] for index, value in enumerate(bytes.fromhex(chunk["raw_hex"]))}
    if any(kept_bytes.get(address) != value for address, value in old_bytes.items()):
        raise ValueError("Native attempt boundary recovery lost an original owner byte")
    return {"coverage_kind": "reviewed_exception_chain_boundary_recovery",
            "original_function": owners[0]["owner_function"], "complete_boundary_recovery": row,
            "complete_helper_assembly": identity(a, path), "all_original_bytes_preserved": True,
            "helper_semantics_or_abi_completion": False}


def instruction_attempt_coverage(a, original_raw, base, previous, replacement, manifest):
    """Bind displaced instruction heads to an independently verified CPU recovery."""
    import native_boundaries
    selection = manifest.get("function_selection", {})
    rows = [row for row in selection.get("instruction_boundary_recoveries", [])
            if row.get("owner_rva") == previous.get("rva")]
    if len(rows) != 1:
        raise ValueError("Native attempt lacks a unique exact instruction recovery")
    row = rows[0]
    review = row["review_source"]
    recomputed = native_boundaries.select_instructions(a, original_raw, base,
        [int(item["rva"], 16) for item in selection.get("targets", [])],
        [review["path"] + "#" + review["pointer"]])
    if recomputed != rows:
        raise ValueError("Native attempt instruction recovery differs from its original-byte proof")
    native_boundaries.verify_instructions(a, selection["instruction_boundary_recoveries"],
                                         manifest.get("instruction_boundary_recoveries"))
    original = json.loads(checked(a, row["owner_function"], ".analysis/results/native"))
    recovered = native_boundaries.instruction_body(a, row)
    if not _same_machine(previous, original) or not _same_machine(replacement, recovered):
        raise ValueError("Native attempt instruction recovery changed an unrelated original machine range")
    original_machine(original_raw, previous)
    original_machine(original_raw, replacement)
    return {"coverage_kind": "reviewed_exact_instruction_boundary_recovery",
            "original_function": row["owner_function"], "complete_instruction_recovery": row,
            "all_original_bytes_preserved": True, "original_decode_failure_preserved": True}


def preexport_instruction_scope(a, original_raw, base, selection, resolution):
    """Verify every intended instruction recovery of an ended pre-export failure."""
    import native_boundaries
    rows = selection.get("instruction_boundary_recoveries", [])
    targets = [row["rva"] for row in selection.get("targets", [])]
    if (not rows or {row.get("owner_rva") for row in rows} != set(targets)
            or len(rows) != len(targets)
            or any(selection.get(key) for key in ("function_merges", "function_outlines",
                "function_partitions", "function_shared_tails", "function_fresh_inlines"))):
        raise ValueError("Pre-export instruction failure lacks complete exact reviewed target scope")
    specifications = [row["review_source"]["path"] + "#" + row["review_source"]["pointer"] for row in rows]
    if native_boundaries.select_instructions(a, original_raw, base, [int(rva, 16) for rva in targets], specifications) != rows:
        raise ValueError("Pre-export instruction failure changed its original-byte review")
    observation = selection.get("static_observation")
    if observation:
        source = observation["review_source"]
        if a.native_observation_evidence(source["path"] + "#" + source["pointer"]) != observation:
            raise ValueError("Pre-export instruction observer differs from its retained exact source")
    prepasses = selection.get("decompiler_prepasses", [])
    if prepasses:
        reasons = {row.get("reason") for row in prepasses}
        if len(reasons) != 1:
            raise ValueError("Pre-export instruction prepasses have ambiguous review reasons")
        if native_boundaries.select_prepasses(a, original_raw, base, [int(rva, 16) for rva in targets],
                [int(row["rva"], 16) for row in prepasses], next(iter(reasons)), instructions=rows) != prepasses:
            raise ValueError("Pre-export instruction prepasses differ from their exact reviewed owners")
    proved = []
    for row in rows:
        selected = resolution["complete_functions"].get(row["owner_rva"])
        if selected is None:
            raise ValueError("Pre-export instruction failure target lacks a complete replacement")
        manifest_path = (a.ROOT / selected["capture"]["path"]).parent.parent / "capture.json"
        manifest = json.loads(manifest_path.read_bytes())
        if identity(a, manifest_path) not in [item["receipt"] for item in resolution["supplements"]]:
            raise ValueError("Pre-export instruction replacement is not a strictly verified supplement")
        intended = native_boundaries.instruction_body(a, row)
        if not _same_machine(selected, intended):
            raise ValueError("Pre-export instruction replacement differs from the original reviewed machine")
        original = json.loads(checked(a, row["owner_function"], ".analysis/results/native"))
        proof = instruction_attempt_coverage(a, original_raw, base, original, selected, manifest)
        proved.append({"failed_instruction_review": row, "complete_replacement_proof": proof})
    return proved


def preexport_frame_scope(a, original_raw, base, manifest, resolution, diagnostic_record):
    """Preserve an exact failed processor binding and prove its completed frame replacement."""
    import native_boundaries

    selection = manifest.get("function_selection", {})
    allowed = {"mode", "image_base", "architecture", "targets", "type_recoveries", "static_observation",
               "decompiler_limits", "type_owner_reanalysis", "direct_call_type_propagations",
               "frame_rebuilds", "decompiler_prepasses"}
    rows, stages = selection.get("frame_rebuilds", []), manifest.get("frame_rebuilds", [])
    targets = [row["rva"] for row in selection.get("targets", [])]
    observation = selection.get("static_observation")
    if (set(selection) - allowed or len(rows) != 1 or len(stages) != 1
            or rows[0].get("owner_rva") not in targets or not isinstance(observation, dict)
            or manifest.get("real_device_operations") is not False
            or manifest.get("database_integrity", {}).get("verified") is not True):
        raise ValueError("Pre-export frame failure lacks one exact unchanged reviewed reconstruction scope")
    notes = {row["assembly_evidence"] for row in selection.get("type_recoveries", [])}
    notes.update(row["reason"] for row in selection.get("decompiler_prepasses", []))
    if len(notes) > 1:
        raise ValueError("Pre-export frame failure has ambiguous type or prepass evidence")
    source = observation["review_source"]
    recomputed = a.native_function_selection(original_raw, base, [int(rva, 16) for rva in targets],
        [row["rva"] + "=" + row["declaration"] for row in selection.get("type_recoveries", [])],
        next(iter(notes)) if notes else None,
        selection.get("decompiler_limits", {}).get("max_function_kib"),
        observation_evidence=source["path"] + "#" + source["pointer"],
        reanalyze_type_owners=bool(selection.get("type_owner_reanalysis")),
        propagate_direct_calls="direct_call_type_propagations" in selection,
        frame_reviews=[row["review_source"]["path"] + "#" + row["review_source"]["pointer"] for row in rows],
        decompiler_prepass_rvas=[row["rva"] for row in selection.get("decompiler_prepasses", [])])
    if recomputed != selection:
        raise ValueError("Pre-export frame failure selection differs from its complete original review")
    if manifest.get("capture_tool", {}).get("sha256") != observation["observer_source"]["sha256"]:
        raise ValueError("Pre-export frame failure did not use its reviewed exact observer source")
    diagnostic = checked(a, diagnostic_record)
    error = b"AttributeError: type object 'processor_t' has no attribute 'create_func_frame'"
    statement = b"sdk.idp.processor_t.create_func_frame(function)"
    if (b"fatal failure" not in diagnostic or b"Traceback" not in diagnostic or error not in diagnostic
            or b"in apply_frame_rebuild" not in diagnostic or statement not in diagnostic):
        raise ValueError("Pre-export frame diagnostic does not retain the exact failed processor binding")
    validator = native_boundaries.retain_source(a, manifest.get("boundary_validator"))
    if statement not in checked(a, validator):
        raise ValueError("Pre-export frame validator lacks the actual failed processor binding")
    request, stage = rows[0], stages[0]
    stage_keys = {"before", "frame_before", "operation", "completed", "corrupt_frame_deleted",
                  "base_database_modified", "target_execution", "manual_stack_points_modified",
                  "manual_frame_values_applied", "instructions_patched"}
    if (set(stage) != set(request) | stage_keys
            or any(stage.get(key) != value for key, value in request.items())
            or stage.get("operation") != "processor_t.create_func_frame"
            or stage.get("completed") is not False or stage.get("corrupt_frame_deleted") is not True
            or stage.get("frame_before") != request["original_frame"]
            or any(stage.get(key) is not False for key in ("base_database_modified", "target_execution",
                    "manual_stack_points_modified", "manual_frame_values_applied", "instructions_patched"))):
        raise ValueError("Pre-export frame failure omitted its exact incomplete destructive-stage checkpoint")
    owner = json.loads(checked(a, request["owner_function"], ".analysis/results/native"))
    native_boundaries._verify_snapshot(owner, stage.get("before"))
    native = request["original_frame"]["native_function"]
    if (stage["before"]["flags"] != native["flags"]
            or stage["before"]["native_frame"] != {key: native[key] for key in stage["before"]["native_frame"]}):
        raise ValueError("Pre-export frame failure snapshot differs from its complete original frame")
    proved, completed_frame = [], None
    originals = {row["rva"]: row for row in base["functions"]}
    result = a.ROOT / ".analysis/results/native" / base["input_sha256"]
    for rva in targets:
        selected = resolution["complete_functions"].get(rva)
        if selected is None:
            raise ValueError("Pre-export frame failure target lacks a strictly complete replacement")
        function_path = a.resolve(selected["capture"]["path"], a.rel(result / "supplements"))
        selected_record = identity(a, function_path)
        manifest_path = function_path.parent.parent / "capture.json"
        manifest_record = identity(a, manifest_path)
        if (selected_record["sha256"] != selected["capture"]["sha256"]
                or manifest_record not in [item["receipt"] for item in resolution["supplements"]]):
            raise ValueError("Pre-export frame replacement is not a strictly verified supplement")
        original_path = a.resolve(a.rel(result / originals[rva]["path"]), a.rel(result))
        previous = json.loads(original_path.read_bytes())
        if (a.sha(original_path) != originals[rva]["sha256"] or not _same_machine(previous, selected)
                or not _same_machine(selected, json.loads(function_path.read_bytes()))):
            raise ValueError("Pre-export frame replacement changed the complete original machine")
        original_machine(original_raw, previous)
        original_machine(original_raw, selected)
        replacement = json.loads(manifest_path.read_bytes())
        replacement_scope = replacement.get("function_selection", {})
        if rva == request["owner_rva"]:
            if (replacement_scope.get("frame_rebuilds") != rows
                    or replacement_scope.get("decompiler_prepasses", []) != selection.get("decompiler_prepasses", [])):
                raise ValueError("Pre-export frame replacement differs from its exact frame and prepass reviews")
            native_boundaries.verify_frame_rebuilds(a, rows, replacement.get("frame_rebuilds"))
            completed_frame = {"replacement_supplement": manifest_record,
                               "completed_processor_reconstruction": replacement["frame_rebuilds"][0]}
        proved.append({"rva": rva, "original_function": identity(a, original_path),
                       "replacement_function": selected_record, "replacement_supplement": manifest_record,
                       "all_original_bytes_preserved": True})
    return {"failed_frame_review": request, "failed_processor_stage": stage,
            "failed_boundary_validator": validator, "complete_replacement_proof": completed_frame,
            "functions": proved, "original_binding_failure_preserved": True,
            "original_failed_frame_restored_or_completed": False}


def _preexport_switch_prepass_order(a, manifest, validator_source):
    """Prove the failed observer reaches switch validation before any prepass."""
    tool = manifest.get("capture_tool", {})
    if (not isinstance(tool, dict) or set(tool) != {"path", "sha256"}
            or tool.get("path") != "tools/ida/supplement.py"
            or not isinstance(tool.get("sha256"), str) or not re.fullmatch(r"[0-9a-f]{64}", tool["sha256"])):
        raise ValueError("Pre-export signed case failure lacks the exact original observer")
    source = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (tool["sha256"] + ".py")
    observer = identity(a, source)
    if observer["sha256"] != tool["sha256"] or validator_source is None:
        raise ValueError("Pre-export signed case failure needs both retained analyzer sources")

    def calls(node, name):
        return [child for child in ast.walk(node) if isinstance(child, ast.Call)
                and ((isinstance(child.func, ast.Name) and child.func.id == name)
                     or isinstance(child.func, ast.Attribute) and child.func.attr == name)]

    def selected_get(node, key):
        return (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
                and isinstance(node.func.value, ast.Name) and node.func.value.id == "selection"
                and node.func.attr == "get" and node.args and isinstance(node.args[0], ast.Constant)
                and node.args[0].value == key)

    tree = ast.parse(source.read_bytes())
    captures = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "capture"]
    if len(captures) != 1:
        raise ValueError("Pre-export observer lacks a unique complete capture owner")
    capture = captures[0]
    switches = [node for node in capture.body if isinstance(node, ast.If)
                and selected_get(node.test, "switch_metadata_recoveries") and calls(node, "apply_switch")]
    prepasses = [node for node in capture.body if isinstance(node, ast.For)
                 and isinstance(node.target, ast.Name) and node.target.id == "prepass"
                 and selected_get(node.iter, "decompiler_prepasses")]
    exports = [node for node in calls(capture, "export")
               if any(keyword.arg == "prepass" for keyword in node.keywords)]
    if (len(switches) != 1 or len(prepasses) != 1 or len(exports) != 1
            or switches[0].end_lineno >= prepasses[0].lineno
            or exports[0] not in list(ast.walk(prepasses[0]))
            or not any(keyword.arg == "prepass" and isinstance(keyword.value, ast.Name)
                       and keyword.value.id == "prepass" for keyword in exports[0].keywords)):
        raise ValueError("Failed switch observer does not prove prepasses were still unexecuted")
    old = ast.parse(checked(a, validator_source))
    functions = {node.name: node for node in old.body if isinstance(node, ast.FunctionDef)}
    normalize, apply = functions.get("_x86_switch_cases"), functions.get("apply_switch")
    message = "SDK x86 case value is not an exact zero/sign extension of its 32-bit expression"
    if normalize is None or apply is None or not any(
            isinstance(node, ast.Raise) and isinstance(node.exc, ast.Call)
            and isinstance(node.exc.func, ast.Name) and node.exc.func.id == "ValueError"
            and node.exc.args and isinstance(node.exc.args[0], ast.Constant)
            and node.exc.args[0].value == message for node in ast.walk(normalize)):
        raise ValueError("Retained validator lacks its exact original signed case rejection")
    stages = [calls(apply, name) for name in ("_switch_cases", "_x86_switch_cases", "create_switch_xrefs")]
    if (any(len(stage) != 1 for stage in stages)
            or not stages[0][0].lineno < stages[1][0].lineno < stages[2][0].lineno):
        raise ValueError("Retained validator does not prove the failed case stage precedes xrefs")
    return {"observer_source": observer, "validator_source": validator_source,
            "switch_statement_line": switches[0].lineno, "prepass_statement_line": prepasses[0].lineno,
            "case_validation_before_xrefs": True, "selected_prepasses_executed": False}


def preexport_switch_scope(a, original_raw, base, manifest, resolution, diagnostic_record):
    """Prove a failed metadata readback against complete new switch cases and cross-references."""
    import native_callsites

    selection = manifest.get("function_selection", {})
    diagnostic = checked(a, diagnostic_record)
    signed_case_failure = (b"in _x86_switch_cases" in diagnostic
        and b"ValueError: SDK x86 case value is not an exact zero/sign extension of its 32-bit expression" in diagnostic)
    allowed = {"mode", "image_base", "architecture", "targets", "type_recoveries",
               "decompiler_limits", "switch_metadata_recoveries"}
    if signed_case_failure:
        allowed.add("decompiler_prepasses")
    rows = selection.get("switch_metadata_recoveries", [])
    targets = [row["rva"] for row in selection.get("targets", [])]
    if (set(selection) - allowed or not rows or {row.get("owner_rva") for row in rows} != set(targets)
            or manifest.get("switch_metadata_recoveries") != []
            or manifest.get("real_device_operations") is not False):
        raise ValueError("Pre-export switch failure lacks complete exact original target scope")
    specifications = [row["review_source"]["path"] + "#" + row["review_source"]["pointer"] for row in rows]
    prepasses = selection.get("decompiler_prepasses", [])
    if signed_case_failure and (selection.get("architecture") != "x86" or len(targets) != 1
            or selection.get("type_recoveries") or not isinstance(prepasses, list) or len(prepasses) != 1
            or prepasses[0].get("rva") != targets[0] or manifest.get("decompiler_prepasses")
            or any(row.get("dispatch_kind") != "x86_guarded_absolute32" for row in rows)
            or not any(row.get("index_low", 0) < 0 for row in rows)):
        raise ValueError("Signed switch case failure lacks its exact unchanged owner-only pending prepass")
    recomputed = a.native_function_selection(original_raw, base, [int(rva, 16) for rva in targets],
        max_function_kib=selection.get("decompiler_limits", {}).get("max_function_kib"),
        switch_reviews=specifications, decompiler_prepass_rvas=[int(row["rva"], 16) for row in prepasses],
        type_note=prepasses[0]["reason"] if prepasses else None)
    if recomputed != selection:
        raise ValueError("Pre-export switch failure differs from its exact original dispatch and complete table review")
    if (b"fatal failure" not in diagnostic or b"Traceback" not in diagnostic
            or b"in apply_switch" not in diagnostic or b"native_callsites.apply_switch(row, base, sdk)" not in diagnostic
            or not signed_case_failure
            and b"ValueError: SDK switch metadata readback differs from its exact reviewed request" not in diagnostic):
        raise ValueError("Pre-export switch diagnostic does not retain the actual metadata readback failure")
    validator = manifest.get("callsite_type_validator")
    if (not isinstance(validator, dict) or set(validator) != {"path", "bytes", "sha256"}
            or validator.get("path") != "tools/native_callsites.py"
            or type(validator.get("bytes")) is not int or validator["bytes"] <= 0
            or not isinstance(validator.get("sha256"), str) or not re.fullmatch(r"[0-9a-f]{64}", validator["sha256"])):
        raise ValueError("Pre-export switch failure lacks its recorded analyzer identity")
    retained = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (validator["sha256"] + ".py")
    old_source = None
    if retained.is_file():
        old_source = identity(a, retained)
        if old_source["bytes"] != validator["bytes"] or old_source["sha256"] != validator["sha256"]:
            raise ValueError("Pre-export switch retained analyzer source differs from its recorded identity")
    stage_proof = _preexport_switch_prepass_order(a, manifest, old_source) if signed_case_failure else None
    result = a.ROOT / ".analysis/results/native" / base["input_sha256"]
    proved = []
    for rva in targets:
        selected = resolution["complete_functions"].get(rva)
        if selected is None:
            raise ValueError("Pre-export switch failure target lacks a strictly complete replacement")
        function_path = a.resolve(selected["capture"]["path"], a.rel(result / "supplements"))
        function_record = identity(a, function_path)
        manifest_path = function_path.parent.parent / "capture.json"
        manifest_record = identity(a, manifest_path)
        if (function_record["sha256"] != selected["capture"]["sha256"]
                or manifest_record not in [item["receipt"] for item in resolution["supplements"]]):
            raise ValueError("Pre-export switch replacement is not a strictly verified complete supplement")
        replacement = json.loads(manifest_path.read_bytes())
        if signed_case_failure:
            import native_boundaries
            current_prepasses = replacement.get("function_selection", {}).get("decompiler_prepasses")
            if (not isinstance(current_prepasses, list)
                    or [{key: value for key, value in row.items() if key != "reason"} for row in current_prepasses]
                    != [{key: value for key, value in row.items() if key != "reason"} for row in prepasses]
                    or any(not isinstance(row.get("reason"), str) or not row["reason"].strip()
                           for row in current_prepasses)):
                raise ValueError("Signed switch replacement changed the exact observed prepass scope")
            native_boundaries.verify_prepasses(a, current_prepasses, replacement.get("decompiler_prepasses"), manifest_path.parent)
        replacements = replacement.get("function_selection", {}).get("switch_metadata_recoveries", [])
        current = [row for row in replacements if row.get("owner_rva") == rva]
        previous = [row for row in rows if row["owner_rva"] == rva]
        if (len(current) != len(previous)
                or [{key: value for key, value in row.items() if key not in {"review_source", "assembly_evidence"}} for row in current]
                != [{key: value for key, value in row.items() if key not in {"review_source", "assembly_evidence"}} for row in previous]):
            raise ValueError("Pre-export switch replacement changed an original table, dispatch, masked case or default proof")
        reviews = [row["review_source"]["path"] + "#" + row["review_source"]["pointer"] for row in replacements]
        actual = native_callsites.select_switches(a, original_raw, base,
            [int(row["rva"], 16) for row in replacement["function_selection"]["targets"]], reviews)
        if actual != replacements:
            raise ValueError("Pre-export switch replacement review differs from the complete original machine")
        native_callsites.verify_switches(a, replacements, replacement.get("switch_metadata_recoveries"))
        original = json.loads(checked(a, previous[0]["owner_function"], ".analysis/results/native"))
        if not _same_machine(original, selected) or not _same_machine(selected, json.loads(function_path.read_bytes())):
            raise ValueError("Pre-export switch replacement changed the complete original function bytes or boundaries")
        original_machine(original_raw, original)
        original_machine(original_raw, selected)
        proved.append({"owner_rva": rva, "failed_switch_reviews": previous,
            "replacement_switch_reviews": current, "replacement_function": function_record,
            "replacement_supplement": manifest_record,
            "complete_sdk_switch_results": [row for row in replacement["switch_metadata_recoveries"] if row["owner_rva"] == rva],
            **({"failed_pending_prepasses": prepasses, "replacement_prepasses": current_prepasses,
                "complete_replacement_prepasses": replacement["decompiler_prepasses"]} if signed_case_failure else {}),
            "all_original_bytes_preserved": True})
    return {"functions": proved, "original_metadata_readback_failure_preserved": not signed_case_failure,
            "failed_switch_results": "not_returned", "failed_switch_readback_values": "unobserved",
            "failed_validator_record": validator, "failed_validator_source": old_source,
            "failed_validator_source_bytes_verified": old_source is not None,
            **({"original_signed_case_failure_preserved": True,
                "failed_switch_failure_stage": "signed32_case_validation", "prepass_stage_proof": stage_proof}
               if signed_case_failure else {})}


def attempt_coverage(a, result, resolution, failure, failure_diagnostic=None, *, alias_abi_review=None):
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
    if before_export and records:
        import native_attempt_history
        return native_attempt_history.reviewed_partial_attempt_coverage(a, result, resolution, failure, failure_diagnostic)
    preexport_scope, preexport_kind = None, "preexport_instruction_scope"
    if not targets or len(set(targets)) != len(targets):
        raise ValueError("Failed native attempt lacks unique exact selected targets")
    if before_export:
        if (records or failures or session.get("passed") is not False or session.get("exit_code") != 1
                or failure_diagnostic is None):
            raise ValueError("Ended pre-export failure lacks complete diagnostic and simple target coverage")
        if selection.get("frame_rebuilds"):
            preexport_scope = preexport_frame_scope(a, original_raw, base, manifest, resolution, failure_diagnostic)
            preexport_kind = "preexport_frame_scope"
        elif selection.get("switch_metadata_recoveries"):
            preexport_scope = preexport_switch_scope(a, original_raw, base, manifest, resolution, failure_diagnostic)
            preexport_kind = "preexport_switch_scope"
        elif selection.get("instruction_boundary_recoveries"):
            preexport_scope = preexport_instruction_scope(a, original_raw, base, selection, resolution)
        elif any(selection.get(key) for key in ("static_observation", "function_merges", "function_outlines",
                "function_partitions", "function_shared_tails", "function_fresh_inlines", "decompiler_prepasses")):
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
        forwarder = resolution.get("complete_forwarders", {}).get(rva)
        contract = resolution.get("complete_static_contracts", {}).get(rva)
        if forwarder is not None or contract is not None:
            old_owner = result if before_export else capture_path.parent
            old_path = a.resolve(a.rel(old_owner / row["path"]), a.rel(old_owner))
            previous_function = identity(a, old_path)
            if previous_function["sha256"] != row["sha256"]:
                raise ValueError("Failed native attempt original static function identity changed")
            if forwarder is not None:
                if alias_abi_review is not None:
                    import native_attempt_history
                    coverage = native_attempt_history.previous_alias_forwarder_coverage(
                        a, result, {**forwarder, "alias_abi_review": alias_abi_review}, previous_function)
                else:
                    import native_import_forwarders
                    coverage = native_import_forwarders.previous_function_coverage(a, result, forwarder, previous_function)
            else:
                import native_static_reviews
                coverage = native_static_reviews.previous_function_coverage(a, result, contract, previous_function)
            functions.append({"rva": rva, "previous_function": previous_function,
                              "replacement_static_contract": coverage})
            continue
        selected = resolution["complete_functions"].get(rva)
        selected_context = resolution.get("complete_contexts", {}).get(rva)
        if selected is None and selected_context is not None:
            if (rva in resolution["conflicting_functions"]
                    or selected_context.get("coverage_kind") != "verified_original_root_recursive_context"
                    or selected_context.get("request", {}).get("core_rva") != rva):
                raise ValueError("Failed native attempt core lacks an unconflicted verified root context")
            context_capture = selected_context["complete_root"]
            selected = {"capture": context_capture, **json.loads(checked(a, context_capture))}
        elif selected is None:
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
            if any(rva in {row.get("root_rva"), row.get("core_rva")}
                   for row in manifest.get("function_selection", {}).get("recursive_fallthrough_contexts", [])):
                union = recursive_context_attempt_coverage(a, original_raw, base, old, selected, manifest,
                                                           replacement_manifest_path.parent)
                expected_context = union["complete_context"]
                if selected_context is not None and selected_context != {**expected_context, "supplement": replacement_manifest}:
                    raise ValueError("Failed native attempt resolution context differs from its complete verified capture")
            elif any(row.get("owner_rva") == rva for row in manifest.get("function_selection", {}).get("instruction_boundary_recoveries", [])):
                union = instruction_attempt_coverage(a, original_raw, base, old, selected, manifest)
            elif any(any(owner["owner_rva"] == rva for owner in row.get("shared_owners", []))
                   for row in manifest.get("function_selection", {}).get("function_boundary_recoveries", [])):
                union = boundary_attempt_coverage(a, original_raw, base, old, selected, manifest,
                                                 replacement_manifest_path.parent)
            elif any(row.get("owner_rva") == rva for row in manifest.get("function_selection", {}).get("function_partitions", [])):
                union = partitioned_attempt_coverage(a, original_raw, base, old, selected, manifest,
                                                     replacement_manifest_path.parent)
            elif any(row.get("owner_rva") == rva for row in manifest.get("function_selection", {}).get("x86_entry_partitions", [])):
                union = entry_partition_attempt_coverage(a, original_raw, base, old, selected, manifest,
                                                        replacement_manifest_path.parent)
            elif any(rva in row.get("owner_rvas", []) for row in manifest.get("function_selection", {}).get("function_shared_tails", [])):
                union = shared_tail_attempt_coverage(a, original_raw, base, old, selected, manifest,
                                                    replacement_manifest_path.parent)
            else:
                union = merged_attempt_coverage(a, original_raw, base, old, selected, manifest)
        functions.append({"rva": rva, "previous_function": identity(a, old_path),
                          "replacement_function": replacement, "replacement_supplement": replacement_manifest,
                          **({({"reviewed_complete_function_partition": "partitioned_original_coverage",
                                "reviewed_complete_x86_entry_partition": "entry_partition_original_coverage",
                                "reviewed_exact_instruction_boundary_recovery": "instruction_original_coverage",
                                "verified_original_root_recursive_context": "recursive_context_original_coverage",
                                "reviewed_complete_shared_tail_owner": "shared_tail_original_coverage",
                                "reviewed_exception_chain_boundary_recovery": "boundary_original_coverage"}
                               .get(union["coverage_kind"], "merged_original_coverage")): union}
                             if union is not None else {})})
    return {"capture": identity(a, capture_path), "session": identity(a, session_path),
            "plan": identity(a, plan_path), "verification_failure": failure,
            "original_failures": failures, "functions": functions,
            "capture_outcome": "failed_before_export" if before_export else "ended_with_hexrays_failures",
            **({preexport_kind: preexport_scope} if preexport_scope is not None else {}),
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
            expected = attempt_coverage(a, result, resolution, failures[name], entry.get("failure_diagnostic"),
                                        alias_abi_review=entry.get("alias_abi_review"))
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


def preserve_binding_scope(a, previous_raw, review, attempts):
    """A replacement binding must carry every previously published decision forward."""
    previous_binding = json.loads(previous_raw)
    previous = json.loads(checked(a, previous_binding.get("review")))
    if (previous.get("review_kind") != review.get("review_kind")
            or previous.get("source_input") != review.get("source_input")
            or previous.get("base_capture") != review.get("base_capture")):
        raise ValueError("Native binding replacement changed its previous review scope")
    name = "attempts" if attempts else "functions"
    old, new = previous.get(name), review.get(name)
    if not isinstance(old, list) or not isinstance(new, list):
        raise ValueError("Native binding replacement omitted its previous decisions")
    def key(row):
        return row.get("capture", {}).get("path") if attempts else row.get("rva")
    old_keys, new_keys = [key(row) for row in old], [key(row) for row in new]
    if (None in old_keys or len(set(old_keys)) != len(old_keys)
            or None in new_keys or len(set(new_keys)) != len(new_keys)
            or not set(old_keys).issubset(new_keys)):
        raise ValueError("Native binding replacement dropped or duplicated a previously published decision")


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
                preserve_binding_scope(a, previous_raw, review, attempts)
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
