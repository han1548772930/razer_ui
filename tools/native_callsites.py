"""Byte-bound vtable callsite types for private static analysis databases."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re

import native_adjudication as evidence


_SOURCE = Path(__file__).read_bytes()


def source_identity(root):
    source = Path(__file__).resolve()
    if source.read_bytes() != _SOURCE:
        raise ValueError("Callsite type validator changed after import")
    return {"path": source.relative_to(root).as_posix(), "bytes": len(_SOURCE),
            "sha256": hashlib.sha256(_SOURCE).hexdigest()}


def retain_source(a, record):
    if (not isinstance(record, dict) or set(record) != {"path", "bytes", "sha256"}
            or record.get("path") != "tools/native_callsites.py"
            or type(record.get("bytes")) is not int or record["bytes"] <= 0
            or not isinstance(record.get("sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", record["sha256"])):
        raise ValueError("Callsite type validator source identity is incomplete")
    target = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (record["sha256"] + ".py")
    with a.index_lock(a.ROOT / ".analysis/native-callsite-source.lock"):
        path = target if target.is_file() else a.resolve(record["path"], "tools")
        raw = path.read_bytes()
        if len(raw) != record["bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
            raise ValueError("Callsite type validator source bytes changed")
        if not target.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open("xb") as stream:
                stream.write(raw)
    return evidence.identity(a, target)


def _review(a, specification):
    name, separator, pointer = specification.partition("#")
    if not separator or not pointer.startswith("/"):
        raise ValueError("Callsite type review requires retained JSON#pointer")
    path = a.resolve(name, ".analysis/results")
    value = json.loads(path.read_bytes())
    try:
        for token in pointer[1:].split("/"):
            if re.search(r"~(?![01])", token):
                raise ValueError("Invalid JSON pointer escape")
            token = token.replace("~1", "/").replace("~0", "~")
            if isinstance(value, list):
                if not re.fullmatch(r"0|[1-9][0-9]*", token):
                    raise ValueError("Invalid JSON pointer index")
                value = value[int(token)]
            else:
                value = value[token]
    except (KeyError, IndexError, TypeError, ValueError) as error:
        raise ValueError("Callsite type review pointer does not resolve") from error
    return value, {**evidence.identity(a, path), "pointer": pointer}


def select(a, raw, capture, function_rvas, specifications):
    if (not specifications or len(specifications) > 64
            or len(set(specifications)) != len(specifications)
            or capture.get("capture_complete") is not True
            or hashlib.sha256(raw).hexdigest() != capture.get("input_sha256")
            or len(raw) != capture.get("input_bytes")):
        raise ValueError("Callsite type recovery requires bounded unique reviews and exact complete original bytes")
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    base = evidence.identity(a, result / "capture.json")
    original = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    records = {row["rva"]: row for row in capture["functions"]}
    observed = {}

    def retained(rva):
        record = records.get(rva)
        if record is None or record.get("assembly_captured") is not True:
            raise ValueError("Callsite type proof lacks a complete original function")
        path = a.resolve(a.rel(result / record["path"]), a.rel(result))
        identity = evidence.identity(a, path)
        if identity["sha256"] != record["sha256"] or ("bytes" in record and identity["bytes"] != record["bytes"]):
            raise ValueError("Callsite type proof original function changed")
        body = json.loads(path.read_bytes())
        if body.get("rva") != rva or body.get("address") != record["address"]:
            raise ValueError("Callsite type proof original function identity differs")
        evidence.original_machine(raw, body)
        return body, identity

    required = {"schema_version", "review_kind", "source_input", "base_capture", "owner_rva",
                "callsite_rva", "callee_rva", "vtable_rva", "vtable_count", "vtable_index",
                "declaration", "assembly_evidence", "object_binding_evidence", "target_execution",
                "raw_artifacts_modified"}
    for specification in specifications:
        review, review_source = _review(a, specification)
        if (not isinstance(review, dict) or set(review) != required or review["schema_version"] != 1
                or review["review_kind"] != "native_resolved_vtable_callsite_type"
                or review["source_input"] != original or review["base_capture"] != base
                or review["target_execution"] is not False or review["raw_artifacts_modified"] is not False
                or any(not isinstance(review[key], str) or not re.fullmatch(r"0x[0-9a-f]+", review[key])
                       or hex(int(review[key], 16)) != review[key]
                       for key in ("owner_rva", "callsite_rva", "callee_rva", "vtable_rva"))
                or int(review["owner_rva"], 16) not in function_rvas
                or any(not isinstance(review[key], str) or not review[key].strip() or len(review[key]) > 16384
                       for key in ("declaration", "assembly_evidence"))
                or type(review["vtable_count"]) is not int or not 1 <= review["vtable_count"] <= 64
                or type(review["vtable_index"]) is not int
                or not 0 <= review["vtable_index"] < review["vtable_count"]):
            raise ValueError("Callsite type review omits exact source, resolved target, ABI or bounded original scope")
        owner, owner_identity = retained(review["owner_rva"])
        _, callee_identity = retained(review["callee_rva"])
        instructions = [row for row in owner["assembly"] if row.get("rva") == review["callsite_rva"]]
        if len(instructions) != 1 or review["callsite_rva"] in observed:
            raise ValueError("Callsite type recovery requires one unique original owner instruction")
        instruction = instructions[0]
        code = bytes.fromhex(instruction["bytes"])
        table = a.native_pointer_targets(raw, int(review["vtable_rva"], 16), review["vtable_count"])
        # Only ordinary near indirect CALLs are accepted. No jump, far call,
        # immediate target, prefix reinterpretation or new instruction is made.
        opcode = 1 if table["architecture"] == "x64" and len(code) >= 3 and 0x40 <= code[0] <= 0x4f else 0
        if (len(code) < opcode + 2 or code[opcode] != 0xff or ((code[opcode + 1] >> 3) & 7) != 2
                or instruction.get("text", "").strip().split(" ", 1)[0].lower() != "call"):
            raise ValueError("Callsite type review does not name an original near indirect CALL")
        if table["targets"][review["vtable_index"]]["rva"] != review["callee_rva"]:
            raise ValueError("Callsite type review target differs from its original vtable slot")
        bindings = review["object_binding_evidence"]
        if (not isinstance(bindings, list) or len(bindings) < 2 or len(bindings) > 64
                or any(not isinstance(row, dict) or set(row) != {"path", "bytes", "sha256"} for row in bindings)
                or len({row["path"] for row in bindings}) != len(bindings)):
            raise ValueError("Callsite type review lacks distinct constructor and lifetime binding evidence")
        for binding in bindings:
            matches = [rva for rva, row in records.items() if a.rel(result / row["path"]) == binding["path"]]
            if len(matches) != 1 or retained(matches[0])[1] != binding:
                raise ValueError("Callsite object binding is not an exact same-input original function")
        observed[review["callsite_rva"]] = {"review_source": review_source,
            **{key: review[key] for key in ("owner_rva", "callsite_rva", "callee_rva", "declaration",
                "assembly_evidence", "vtable_index", "object_binding_evidence")},
            "owner_function": owner_identity, "callee_function": callee_identity,
            "instruction": instruction, "vtable": table, "target_execution": False}
    return list(observed.values())


def apply(requested, base, sdk):
    """Apply only reviewed operand types; SDK calls operate on the private IDB."""
    ea = base + int(requested["callsite_rva"], 16)
    table = requested["vtable"]
    if (sdk.bytes.get_bytes(ea, len(bytes.fromhex(requested["instruction"]["bytes"])))
            != bytes.fromhex(requested["instruction"]["bytes"])
            or sdk.bytes.get_bytes(base + int(table["rva"], 16), table["bytes"]) != bytes.fromhex(table["raw_hex"])):
        raise ValueError("Private IDB callsite or resolved vtable differs from original bytes")
    parsed = sdk.types.tinfo_t()
    if not sdk.types.parse_decl(parsed, None, requested["declaration"], sdk.types.PT_SIL) or not parsed.is_func():
        raise ValueError("SDK rejected the exact reviewed callsite function declaration")
    previous = sdk.types.tinfo_t()
    had_previous = bool(sdk.nalt.get_op_tinfo(previous, ea, 0))
    if not sdk.nalt.set_op_tinfo(ea, 0, parsed):
        raise ValueError("SDK rejected the exact reviewed indirect call operand type")
    if not sdk.types.apply_callee_tinfo(ea, parsed):
        raise ValueError("SDK rejected the exact reviewed callee argument type propagation")
    observed = sdk.types.tinfo_t()
    if not sdk.nalt.get_op_tinfo(observed, ea, 0) or observed != parsed:
        raise ValueError("SDK callsite operand type readback differs from the reviewed declaration")
    return {**requested, "previous_operand_type_present": had_previous,
        "previous_operand_type": str(previous) if had_previous else None,
        "parsed_type": str(parsed), "applied_type": str(observed),
        "operand_type_readback_matches": True, "callee_type_applied": True, "base_database_modified": False}


def verify_applied(requested, applied):
    if (not isinstance(applied, list) or len(applied) != len(requested)
            or any(not isinstance(row, dict) for row in applied)
            or any(any(row.get(key) != value for key, value in expected.items())
                or type(row.get("previous_operand_type_present")) is not bool
                or (not row["previous_operand_type_present"] and row.get("previous_operand_type") is not None)
                or (row["previous_operand_type_present"] and not isinstance(row.get("previous_operand_type"), str))
                or not isinstance(row.get("parsed_type"), str) or not row["parsed_type"].strip()
                or row.get("applied_type") != row["parsed_type"]
                or row.get("operand_type_readback_matches") is not True
                or row.get("callee_type_applied") is not True
                or row.get("base_database_modified") is not False
                for row, expected in zip(applied, requested))):
        raise ValueError("Native supplement did not retain its exact resolved callsite types and SDK readback")


def select_direct(a, raw, capture, requested_owners, recoveries):
    """Bind reviewed callee types to original direct CALLs in selected owners."""
    if (capture.get("capture_complete") is not True
            or hashlib.sha256(raw).hexdigest() != capture.get("input_sha256")
            or len(raw) != capture.get("input_bytes")
            or not isinstance(requested_owners, list) or not requested_owners
            or len(requested_owners) > 4096 or not isinstance(recoveries, list)
            or len(recoveries) > 4096):
        raise ValueError("Direct call propagation requires exact original bytes and bounded selected owners")
    owners = {row["owner_rva"]: row for row in requested_owners}
    if len(owners) != len(requested_owners):
        raise ValueError("Direct call propagation repeats a selected owner")
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    records = {row["rva"]: row for row in capture["functions"]}
    base = int(capture["image_base"], 16)
    retained = {}

    def original(rva):
        if rva in retained:
            return retained[rva]
        record = records.get(rva)
        if record is None or record.get("assembly_captured") is not True:
            raise ValueError("Direct call propagation lacks a complete original function")
        path = a.resolve(a.rel(result / record["path"]), a.rel(result))
        identity = evidence.identity(a, path)
        if identity["sha256"] != record["sha256"] or ("bytes" in record and identity["bytes"] != record["bytes"]):
            raise ValueError("Direct call propagation original function changed")
        body = json.loads(path.read_bytes())
        if body.get("rva") != rva or body.get("address") != hex(base + int(rva, 16)):
            raise ValueError("Direct call propagation original function identity differs")
        evidence.original_machine(raw, body)
        retained[rva] = body, identity
        return retained[rva]

    types = {}
    for row in recoveries:
        rva = row.get("rva")
        if (not isinstance(rva, str) or not re.fullmatch(r"0x[0-9a-f]+", rva)
                or hex(int(rva, 16)) != rva or rva not in owners or rva in types
                or any(not isinstance(row.get(key), str) or not row[key].strip()
                       or len(row[key]) > 16384 for key in ("declaration", "assembly_evidence"))):
            raise ValueError("Direct call propagation requires unique explicitly reviewed selected callee types")
        types[rva] = row
        original(rva)
    values = []
    for rva, owner in owners.items():
        body, identity = original(rva)
        if owner.get("owner_function") != identity:
            raise ValueError("Direct call propagation selected owner identity differs")
        seen = set()
        for instruction in body["assembly"]:
            code = bytes.fromhex(instruction["bytes"])
            if len(code) != 5 or code[0] != 0xe8:
                continue
            target = int(instruction["address"], 16) + 5 + int.from_bytes(code[1:], "little", signed=True)
            callee = hex(target - base)
            if callee not in types:
                continue
            callsite = instruction["rva"]
            if (callsite in seen or int(callsite, 16) + base != int(instruction["address"], 16)
                    or instruction.get("text", "").strip().split(" ", 1)[0].lower() != "call"):
                raise ValueError("Direct call propagation does not name a unique original near CALL")
            seen.add(callsite)
            _, callee_identity = original(callee)
            values.append({"owner_rva": rva, "owner_function": identity,
                "callsite_rva": callsite, "callee_rva": callee, "callee_function": callee_identity,
                "instruction": instruction, "declaration": types[callee]["declaration"],
                "assembly_evidence": types[callee]["assembly_evidence"], "target_execution": False})
            if len(values) > 16384:
                raise ValueError("Direct call propagation exceeds its reviewed callsite bound")
    return values


def apply_direct(requested, base, sdk):
    """Propagate the actual applied callee type, retaining both operand states."""
    for key in ("owner_function", "callee_function"):
        source = requested[key]
        raw = (sdk.root / source["path"]).read_bytes()
        if len(raw) != source["bytes"] or hashlib.sha256(raw).hexdigest() != source["sha256"]:
            raise ValueError("Direct call propagation retained original function changed")
        body = json.loads(raw)
        function = sdk.funcs.get_func(base + int(body["rva"], 16))
        if (function is None or int(function.start_ea) != base + int(body["rva"], 16)
                or hex(int(function.end_ea)) != body["end"]
                or [(hex(start), hex(stop), sdk.bytes.get_bytes(start, stop - start))
                    for start, stop in sdk.utils.Chunks(function.start_ea)]
                != [(row["start"], row["end"], bytes.fromhex(row["raw_hex"])) for row in body["chunks"]]
                or any(sdk.bytes.get_bytes(int(row["address"], 16), len(bytes.fromhex(row["bytes"])))
                       != bytes.fromhex(row["bytes"]) for row in body["assembly"])):
            raise ValueError("Direct call propagation private original function boundaries or bytes differ")
    ea = base + int(requested["callsite_rva"], 16)
    code = bytes.fromhex(requested["instruction"]["bytes"])
    if (len(code) != 5 or code[0] != 0xe8 or sdk.bytes.get_bytes(ea, 5) != code
            or ea + 5 + int.from_bytes(code[1:], "little", signed=True) != base + int(requested["callee_rva"], 16)):
        raise ValueError("Direct call propagation target differs from its original E8 instruction")
    parsed, callee = sdk.types.tinfo_t(), sdk.types.tinfo_t()
    if (not sdk.types.parse_decl(parsed, None, requested["declaration"], sdk.types.PT_SIL)
            or not parsed.is_func() or not sdk.types.get_tinfo(callee, base + int(requested["callee_rva"], 16))
            or callee != parsed):
        raise ValueError("Direct call propagation actual applied callee differs from its reviewed declaration")
    previous = sdk.types.tinfo_t()
    had_previous = bool(sdk.nalt.get_op_tinfo(previous, ea, 0))
    if not sdk.nalt.set_op_tinfo(ea, 0, callee):
        raise ValueError("SDK rejected the exact reviewed direct call operand type")
    if not sdk.types.apply_callee_tinfo(ea, callee):
        raise ValueError("SDK rejected the exact direct callee argument propagation")
    observed = sdk.types.tinfo_t()
    if not sdk.nalt.get_op_tinfo(observed, ea, 0) or observed != callee:
        raise ValueError("SDK direct call operand type readback differs from its actual callee")
    return {**requested, "previous_operand_type_present": had_previous,
        "previous_operand_type": str(previous) if had_previous else None,
        "parsed_type": str(parsed), "actual_callee_type": str(callee), "applied_type": str(observed),
        "actual_callee_type_matches": True, "operand_type_readback_matches": True,
        "callee_type_applied": True, "base_database_modified": False}


def verify_direct_applied(requested, applied):
    verify_applied(requested, applied)
    if any(row.get("actual_callee_type") != row["parsed_type"]
           or row.get("actual_callee_type_matches") is not True for row in applied):
        raise ValueError("Direct call propagation omitted the actual applied callee type")


def reanalyze_owners(requested, base, sdk):
    """Let the processor refresh its own analysis after exact call type changes."""
    values = []
    owners = {row["owner_rva"]: row for row in requested}
    for rva, recovery in owners.items():
        raw = (sdk.root / recovery["owner_function"]["path"]).read_bytes()
        source = recovery["owner_function"]
        if len(raw) != source["bytes"] or hashlib.sha256(raw).hexdigest() != source["sha256"]:
            raise ValueError("Automatic callsite reanalysis original owner changed")
        body = json.loads(raw)
        ea = base + int(rva, 16)
        function = sdk.funcs.get_func(ea)
        if function is None or int(function.start_ea) != ea:
            raise ValueError("Automatic callsite reanalysis lost its exact retained owner")

        def snapshot():
            current = sdk.funcs.get_func(ea)
            if current is None or int(current.start_ea) != ea:
                raise ValueError("Automatic callsite reanalysis changed its retained owner")
            chunks = []
            for start, stop in sdk.utils.Chunks(ea):
                code = sdk.bytes.get_bytes(start, stop - start)
                if code is None:
                    raise ValueError("Automatic callsite reanalysis bytes are unavailable")
                chunks.append({"start": hex(start), "end": hex(stop), "raw_hex": code.hex(),
                               "sha256": hashlib.sha256(code).hexdigest()})
            if (hex(int(current.end_ea)) != body["end"] or chunks != body["chunks"]
                    or any(sdk.bytes.get_bytes(int(insn["address"], 16), len(bytes.fromhex(insn["bytes"])))
                           != bytes.fromhex(insn["bytes"]) for insn in body["assembly"])):
                raise ValueError("Automatic callsite reanalysis changed original boundaries or machine bytes")
            return {"flags": int(current.flags), "chunks": chunks,
                    "native_frame": {key: int(getattr(current, key))
                                     for key in ("frame", "frsize", "frregs", "argsize", "fpd")},
                    "stack_points": [
                {"ea": insn["address"], "spd": int(sdk.frame.get_spd(current, int(insn["address"], 16))),
                 "delta": int(sdk.frame.get_sp_delta(current, int(insn["address"], 16)))}
                for insn in body["assembly"]]}

        before = snapshot()
        # A completed SP analysis prevents the processor from updating its
        # derived change points after an indirect call type is corrected.
        # Invalidate only that readiness bit; never supply a replacement delta.
        ready_flag = sdk.funcs.FUNC_SP_READY
        if type(ready_flag) is not int or ready_flag <= 0 or ready_flag & (ready_flag - 1):
            raise ValueError("SDK stack-analysis readiness flag is invalid")
        invalidated_flags = int(function.flags) & ~ready_flag
        function.flags = invalidated_flags
        if not sdk.funcs.update_func(function):
            raise ValueError("SDK could not invalidate completed automatic stack analysis")
        function = sdk.funcs.get_func(ea)
        if function is None or int(function.start_ea) != ea or int(function.flags) != invalidated_flags:
            raise ValueError("SDK stack-analysis invalidation readback differs")
        invalidation = {"operation": "clear_completed_automatic_stack_analysis_readiness",
                        "readiness_flag": ready_flag, "previous_flags": before["flags"],
                        "invalidated_flags": invalidated_flags, "update_func_completed": True,
                        "flag_readback_matches": True, "manual_stack_points_modified": False}
        sdk.funcs.reanalyze_function(function)
        waited = bool(sdk.auto.auto_wait())
        after = snapshot()
        if not waited:
            raise ValueError("Automatic callsite owner reanalysis was canceled")
        values.append({"owner_rva": rva, "owner_function": source, "before": before, "after": after,
                       "analysis_invalidation": invalidation,
                       "reanalyze_function_requested": True, "auto_wait_completed": True,
                       "manual_stack_points_modified": False, "base_database_modified": False,
                       "target_execution": False})
    return values


def verify_reanalysis(a, requested, applied):
    owners = {row["owner_rva"]: row for row in requested}
    if (not isinstance(applied, list) or len(applied) != len(owners)
            or [row.get("owner_rva") for row in applied if isinstance(row, dict)] != list(owners)):
        raise ValueError("Automatic callsite reanalysis omitted or changed an owner")
    for value in applied:
        source = owners[value["owner_rva"]]["owner_function"]
        body = json.loads(evidence.checked(a, source, ".analysis/results/native"))
        if (value.get("owner_function") != source or value.get("reanalyze_function_requested") is not True
                or value.get("auto_wait_completed") is not True
                or value.get("manual_stack_points_modified") is not False
                or value.get("base_database_modified") is not False or value.get("target_execution") is not False):
            raise ValueError("Automatic callsite reanalysis omitted SDK completion or changed its scope")
        invalidation = value.get("analysis_invalidation")
        if invalidation is not None:
            flag = invalidation.get("readiness_flag")
            if (type(flag) is not int or flag <= 0 or flag & (flag - 1)
                    or invalidation.get("operation") != "clear_completed_automatic_stack_analysis_readiness"
                    or type(invalidation.get("previous_flags")) is not int
                    or invalidation.get("previous_flags") != value["before"].get("flags")
                    or type(invalidation.get("invalidated_flags")) is not int
                    or invalidation["invalidated_flags"] != (invalidation["previous_flags"] & ~flag)
                    or invalidation.get("update_func_completed") is not True
                    or invalidation.get("flag_readback_matches") is not True
                    or invalidation.get("manual_stack_points_modified") is not False):
                raise ValueError("Automatic stack-analysis invalidation changed flags or omitted SDK readback")
        for stage in ("before", "after"):
            observed = value.get(stage, {})
            points = observed.get("stack_points")
            native_frame = observed.get("native_frame")
            if native_frame is not None and (not isinstance(native_frame, dict)
                    or set(native_frame) != {"frame", "frsize", "frregs", "argsize", "fpd"}
                    or any(type(item) is not int for item in native_frame.values())):
                raise ValueError("Automatic callsite reanalysis native frame observations are incomplete")
            if (observed.get("chunks") != body["chunks"] or type(observed.get("flags")) is not int
                    or not isinstance(points, list) or len(points) != len(body["assembly"])
                    or any(not isinstance(point, dict) or point.get("ea") != instruction["address"]
                           or type(point.get("spd")) is not int or type(point.get("delta")) is not int
                           for point, instruction in zip(points, body["assembly"]))):
                raise ValueError("Automatic callsite reanalysis omitted original chunks or raw stack observations")
