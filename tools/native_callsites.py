"""Byte-bound callsite types and switch metadata for private static IDBs."""
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
        if isinstance(review, dict) and review.get("review_kind") == "native_x86_stack_forwarder_callsite_type":
            value = _select_stack_forwarder(a, raw, capture, function_rvas, review, review_source, retained)
            if value["callsite_rva"] in observed:
                raise ValueError("Callsite type recovery repeats an original owner instruction")
            observed[value["callsite_rva"]] = value
            continue
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


def _select_stack_forwarder(a, raw, capture, function_rvas, review, review_source, retained):
    """Keep a polymorphic helper global type while proving one two-word call."""
    import native_boundaries
    from host import inspect_pe
    required = {"schema_version", "review_kind", "source_input", "base_capture", "owner_rva", "owner_function",
        "callsite_rva", "callee_rva", "callee_function", "callee_original_type", "declaration", "callers",
        "callee_database_type", "database_type_observation", "assembly_evidence", "target_execution", "raw_artifacts_modified"}
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    if (set(review) != required or review["schema_version"] != 1 or inspect_pe(raw)["architecture"] != "x86"
            or review["source_input"] != evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
            or review["base_capture"] != evidence.identity(a, result / "capture.json")
            or review["target_execution"] is not False or review["raw_artifacts_modified"] is not False
            or any(not isinstance(review[key], str) or not re.fullmatch(r"0x[0-9a-f]+", review[key])
                   or hex(int(review[key], 16)) != review[key] for key in ("owner_rva", "callsite_rva", "callee_rva"))
            or int(review["owner_rva"], 16) not in function_rvas
            or review["declaration"] != "void __stdcall recovered(void *pthis, void *pmfn);"
            or not isinstance(review["assembly_evidence"], str) or not review["assembly_evidence"].strip()):
        raise ValueError("Stack-forwarder review lacks exact original identities or the two-word callsite contract")
    base, read = native_boundaries._instruction_image(raw)
    owner, owner_id = retained(review["owner_rva"])
    callee, callee_id = retained(review["callee_rva"])
    helper = int(review["callee_rva"], 16)
    helper_code = bytes.fromhex("5859870424ffe0")
    expected_parts = ["58", "59", "870424", "ffe0"]
    if (owner_id != review["owner_function"] or callee_id != review["callee_function"]
            or callee.get("type") != review["callee_original_type"]
            or review["callee_original_type"] != "void __stdcall(void *pthis, void *pmfn, void *pthat, int val2)"
            or callee["chunks"] != [{**callee["chunks"][0], "start": hex(base + helper),
                                      "end": hex(base + helper + len(helper_code)), "raw_hex": helper_code.hex()}]
            or read(helper, len(helper_code)) != helper_code
            or [row["bytes"] for row in callee["assembly"]] != expected_parts
            or [int(row["address"], 16) for row in callee["assembly"]]
                != [base + helper + offset for offset in (0, 1, 2, 5)]):
        raise ValueError("Stack-forwarder is not the exact original return/this/member-pointer stack transformation")
    observation = json.loads(evidence.checked(a, review["database_type_observation"]))
    database = a.ROOT / ".work/native" / capture["input_sha256"] / "batch" / (Path(capture["input"]).name + ".i64")
    expected_details = {"cc": 80, "return_type": "void", "arguments": [
        {"name": name, "type": "void *"} for name in ("pthis", "pmfn", "pthat", "val2")]}
    observed = observation.get("observation", {})
    observer = observation.get("observer_source", {})
    if (observation.get("schema_version") != 1
            or observation.get("review_kind") != "native_private_static_type_observation"
            or observation.get("source_input") != review["source_input"]
            or observation.get("source_database") != evidence.identity(a, database)
            or observation.get("target_execution") is not False or observation.get("shared_tools_modified") is not False
            or observation.get("temporary_directory_absent") is not True
            or observation.get("source_database_unchanged") is not True or observation.get("analyzer_exit_code") != 0
            or not isinstance(observer.get("source"), str) or not observer["source"].strip()
            or hashlib.sha256(observer["source"].encode()).hexdigest() != observer.get("sha256")
            or review["callee_database_type"] != "void __stdcall(void *pthis, void *pmfn, void *pthat, void *val2)"
            or observed.get("idc_type") != review["callee_database_type"]
            or observed.get("tinfo_text") != review["callee_database_type"]
            or observed.get("global_present") is not True or observed.get("details_present") is not True
            or observed.get("cc") != expected_details["cc"] or observed.get("return_type") != expected_details["return_type"]
            or not isinstance(observed.get("arguments"), list)
            or [{key: arg.get(key) for key in ("name", "type")} for arg in observed["arguments"]] != expected_details["arguments"]
            or observed.get("helper_hex") != helper_code.hex()
            or observed.get("callsite_hex") != read(int(review["callsite_rva"], 16) - 4, 9).hex()
            or observed.get("target_execution") is not False):
        raise ValueError("Stack-forwarder source-IDB type observation omits its exact database, four arguments or unchanged source")
    # Every executable-section E8 byte candidate is accounted for independently
    # of IDA's existing xrefs. No ignored caller or global arity change is allowed.
    import struct
    pe = struct.unpack_from("<I", raw, 0x3c)[0]
    count, optional = struct.unpack_from("<H", raw, pe + 6)[0], struct.unpack_from("<H", raw, pe + 20)[0]
    physical = set()
    for index in range(count):
        at = pe + 24 + optional + 40 * index
        rva, size, offset, flags = (struct.unpack_from("<I", raw, at + field)[0] for field in (12, 16, 20, 36))
        if flags & 0x20000000:
            for cursor in range(size - 4):
                code = raw[offset + cursor:offset + cursor + 5]
                if _direct_target(code, base + rva + cursor, "x86") == base + helper:
                    physical.add(hex(rva + cursor))
    refs = [row for row in callee.get("xrefs_to_entry", []) if row.get("code") is True]
    if (not refs or any(row.get("type") != 17 or row.get("to") != hex(base + helper) for row in refs)
            or len({row["from"] for row in refs}) != len(refs)
            or {hex(int(row["from"], 16) - base) for row in refs} != physical
            or [ref for ref in callee["assembly"][0].get("xrefs_to", []) if ref.get("code") is True] != refs):
        raise ValueError("Stack-forwarder proof does not account for every original physical direct caller")
    for previous, instruction in zip(callee["assembly"], callee["assembly"][1:]):
        incoming = [ref for ref in instruction.get("xrefs_to", []) if ref.get("code") is True]
        if (len(incoming) != 1 or incoming[0].get("from") != previous["address"]
                or incoming[0].get("to") != instruction["address"] or incoming[0].get("type") != 21
                or incoming[0] not in previous.get("xrefs_from", [])):
            raise ValueError("Stack-forwarder helper has an incoming edge bypassing its stack transformation")
    callers = review["callers"]
    if (not isinstance(callers, list) or not 2 <= len(callers) <= 64
            or any(not isinstance(row, dict) or set(row) != {"owner_rva", "owner_function", "callsite_rva", "argument_words", "assembly_evidence"}
                   or type(row["argument_words"]) is not int or row["argument_words"] not in {2, 3, 4}
                   or not isinstance(row["assembly_evidence"], str) or not row["assembly_evidence"].strip() for row in callers)
            or {row["callsite_rva"] for row in callers} != physical or len(callers) != len(physical)
            or {row["argument_words"] for row in callers} != {2, 3, 4}):
        raise ValueError("Stack-forwarder review omits its complete differing-arity caller lifecycle")
    for caller in callers:
        body, identity = retained(caller["owner_rva"])
        rows = [row for row in body["assembly"] if row["rva"] == caller["callsite_rva"]]
        if (identity != caller["owner_function"] or len(rows) != 1
                or _direct_target(bytes.fromhex(rows[0]["bytes"]), int(rows[0]["address"], 16), "x86") != base + helper
                or not any(ref.get("from") == rows[0]["address"] and ref.get("to") == hex(base + helper)
                           and ref.get("type") == 17 and ref.get("code") is True for ref in rows[0].get("xrefs_from", []))):
            raise ValueError("Stack-forwarder caller is not an exact same-input complete original function")
    heads = {int(row["rva"], 16): row for row in owner["assembly"]}
    call = int(review["callsite_rva"], 16)
    setup = [heads.get(call - 4), heads.get(call - 1), heads.get(call)]
    if (any(row is None for row in setup) or [row["bytes"] for row in setup[:2]] != ["ff7204", "51"]
            or _direct_target(bytes.fromhex(setup[2]["bytes"]), base + call, "x86") != base + helper
            or not any(row["owner_rva"] == review["owner_rva"] and row["callsite_rva"] == review["callsite_rva"]
                       and row["argument_words"] == 2 for row in callers)):
        raise ValueError("Stack-forwarder callsite is not the exact original member-pointer/this two-push setup")
    for previous, instruction in zip(setup, setup[1:]):
        incoming = [ref for ref in instruction.get("xrefs_to", []) if ref.get("code") is True]
        if (len(incoming) != 1 or incoming[0].get("type") != 21
                or incoming[0].get("from") != previous["address"] or incoming[0].get("to") != instruction["address"]
                or incoming[0] not in previous.get("xrefs_from", [])):
            raise ValueError("Stack-forwarder callsite has an incoming edge bypassing argument setup")
    return {"review_source": review_source, "callsite_kind": "x86_stack_forwarder_two_words",
        **{key: review[key] for key in ("owner_rva", "owner_function", "callsite_rva", "callee_rva", "callee_function",
            "callee_original_type", "callee_database_type", "database_type_observation", "declaration", "callers", "assembly_evidence")},
        "source_database": observation["source_database"], "callee_database_details": expected_details,
        "instruction": setup[2], "setup_instructions": setup[:2], "callee_raw_hex": helper_code.hex(), "target_execution": False}


def _stack_forwarder_global(requested, base, sdk):
    tif = sdk.types.tinfo_t()
    details = sdk.types.func_type_data_t()
    ea = base + int(requested["callee_rva"], 16)
    if (not sdk.nalt.get_tinfo(tif, ea) or str(tif) != requested["callee_database_type"]
            or not tif.get_func_details(details)):
        raise ValueError("Private IDB stack-forwarder global type differs from its exact source-database observation")
    actual = {"cc": int(details.cc), "return_type": str(details.rettype),
              "arguments": [{"name": arg.name, "type": str(arg.type)} for arg in details]}
    if actual != requested["callee_database_details"]:
        raise ValueError("Private IDB stack-forwarder global ABI does not preserve all four observed arguments")
    return {"type": str(tif), "details": actual}


def apply(requested, base, sdk):
    """Apply only reviewed operand types; SDK calls operate on the private IDB."""
    ea = base + int(requested["callsite_rva"], 16)
    forwarder = requested.get("callsite_kind") == "x86_stack_forwarder_two_words"
    if forwarder:
        callee_ea = base + int(requested["callee_rva"], 16)
        global_before = _stack_forwarder_global(requested, base, sdk)
        if (sdk.bytes.get_bytes(callee_ea, len(bytes.fromhex(requested["callee_raw_hex"]))) != bytes.fromhex(requested["callee_raw_hex"])
                or any(sdk.bytes.get_bytes(int(ins["address"], 16), len(bytes.fromhex(ins["bytes"]))) != bytes.fromhex(ins["bytes"])
                       for ins in [*requested["setup_instructions"], requested["instruction"]])):
            raise ValueError("Private IDB stack-forwarder global type or original helper/callsite bytes differ")
    else:
        table = requested["vtable"]
    if not forwarder and (sdk.bytes.get_bytes(ea, len(bytes.fromhex(requested["instruction"]["bytes"])))
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
    global_fields = {}
    if forwarder:
        global_after = _stack_forwarder_global(requested, base, sdk)
        if global_after != global_before:
            raise ValueError("SDK callsite override changed the polymorphic helper global type")
        global_fields = {"callee_global_type_before": global_before, "callee_global_type_after": global_after,
                         "callee_global_type_unchanged": True}
    return {**requested, "previous_operand_type_present": had_previous,
        "previous_operand_type": str(previous) if had_previous else None,
        "parsed_type": str(parsed), "applied_type": str(observed),
        "operand_type_readback_matches": True, "callee_type_applied": True, "base_database_modified": False, **global_fields}


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
    for expected, row in zip(requested, applied):
        if expected.get("callsite_kind") == "x86_stack_forwarder_two_words" and (
                row.get("callee_global_type_before") != {"type": expected["callee_database_type"], "details": expected["callee_database_details"]}
                or row.get("callee_global_type_after") != row.get("callee_global_type_before")
                or row.get("callee_global_type_unchanged") is not True):
            raise ValueError("Native supplement did not preserve the polymorphic helper global type")


def _direct_target(code, address, architecture):
    if architecture in {"x86", "x64"} and len(code) == 5 and code[0] == 0xe8:
        return address + 5 + int.from_bytes(code[1:], "little", signed=True)
    if architecture == "arm64" and len(code) == 4 and address % 4 == 0:
        word = int.from_bytes(code, "little")
        if word & 0xfc000000 == 0x94000000:
            immediate = word & 0x03ffffff
            if immediate & 0x02000000:
                immediate -= 0x04000000
            return address + (immediate << 2)
    return None


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
    from host import inspect_pe
    architecture = inspect_pe(raw).get("architecture")
    if architecture not in {"x86", "x64", "arm64"}:
        raise ValueError("Direct call propagation requires a verified supported PE architecture")
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
            target = _direct_target(code, int(instruction["address"], 16), architecture)
            if target is None:
                continue
            callee = hex(target - base)
            if callee not in types:
                continue
            callsite = instruction["rva"]
            mnemonic = instruction.get("text", "").split()
            if (callsite in seen or int(callsite, 16) + base != int(instruction["address"], 16)
                    or not mnemonic or mnemonic[0].lower() != ("bl" if architecture == "arm64" else "call")):
                raise ValueError("Direct call propagation does not name a unique original near CALL")
            seen.add(callsite)
            _, callee_identity = original(callee)
            values.append({"owner_rva": rva, "owner_function": identity,
                "callsite_rva": callsite, "callee_rva": callee, "callee_function": callee_identity,
                "instruction": instruction, "declaration": types[callee]["declaration"],
                "assembly_evidence": types[callee]["assembly_evidence"], "target_execution": False,
                **({"instruction_architecture": "arm64"} if architecture == "arm64" else {})})
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
    architecture = requested.get("instruction_architecture", "x64")
    if (architecture not in {"x64", "arm64"} or sdk.bytes.get_bytes(ea, len(code)) != code
            or _direct_target(code, ea, architecture) != base + int(requested["callee_rva"], 16)):
        raise ValueError("Direct call propagation target differs from its original direct instruction")
    parsed, callee = sdk.types.tinfo_t(), sdk.types.tinfo_t()
    if (not sdk.types.parse_decl(parsed, None, requested["declaration"], sdk.types.PT_SIL)
            or not parsed.is_func() or not sdk.nalt.get_tinfo(callee, base + int(requested["callee_rva"], 16))
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
        protected_global = (_stack_forwarder_global(recovery, base, sdk)
                            if recovery.get("callsite_kind") == "x86_stack_forwarder_two_words" else None)
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
        protected_after = _stack_forwarder_global(recovery, base, sdk) if protected_global is not None else None
        if protected_after != protected_global:
            raise ValueError("Automatic owner reanalysis changed the polymorphic helper global ABI")
        values.append({"owner_rva": rva, "owner_function": source, "before": before, "after": after,
                       "analysis_invalidation": invalidation,
                       "reanalyze_function_requested": True, "auto_wait_completed": True,
                       "manual_stack_points_modified": False, "base_database_modified": False,
                       "target_execution": False,
                       **({"callee_global_before_reanalysis": protected_global,
                           "callee_global_after_reanalysis": protected_after,
                           "callee_global_unchanged_after_reanalysis": True} if protected_global is not None else {})})
    return values


def verify_reanalysis(a, requested, applied):
    owners = {row["owner_rva"]: row for row in requested}
    if (not isinstance(applied, list) or len(applied) != len(owners)
            or [row.get("owner_rva") for row in applied if isinstance(row, dict)] != list(owners)):
        raise ValueError("Automatic callsite reanalysis omitted or changed an owner")
    for value in applied:
        recovery = owners[value["owner_rva"]]
        source = recovery["owner_function"]
        body = json.loads(evidence.checked(a, source, ".analysis/results/native"))
        if (value.get("owner_function") != source or value.get("reanalyze_function_requested") is not True
                or value.get("auto_wait_completed") is not True
                or value.get("manual_stack_points_modified") is not False
                or value.get("base_database_modified") is not False or value.get("target_execution") is not False):
            raise ValueError("Automatic callsite reanalysis omitted SDK completion or changed its scope")
        if recovery.get("callsite_kind") == "x86_stack_forwarder_two_words" and (
                value.get("callee_global_before_reanalysis") != {"type": recovery["callee_database_type"], "details": recovery["callee_database_details"]}
                or value.get("callee_global_after_reanalysis") != value.get("callee_global_before_reanalysis")
                or value.get("callee_global_unchanged_after_reanalysis") is not True):
            raise ValueError("Automatic callsite reanalysis omitted unchanged four-argument helper ABI readback")
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


def select_switches(a, raw, capture, selected_rvas, specifications):
    """Resolve every entry of an explicitly reviewed masked relative jump table."""
    import native_boundaries
    from host import inspect_pe
    if (not isinstance(specifications, (tuple, list)) or not 1 <= len(specifications) <= 64
            or len(set(specifications)) != len(specifications) or capture.get("capture_complete") is not True
            or hashlib.sha256(raw).hexdigest() != capture.get("input_sha256")
            or len(raw) != capture.get("input_bytes") or inspect_pe(raw)["architecture"] not in {"x86", "x64"}):
        raise ValueError("Switch recovery requires exact x86/x64 original bytes and unique bounded reviews")
    architecture = inspect_pe(raw)["architecture"]
    base, read = native_boundaries._instruction_image(raw)
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    records = {row["rva"]: row for row in capture["functions"]}
    source = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    base_identity = evidence.identity(a, result / "capture.json")
    values, seen = [], set()
    required = {"schema_version", "review_kind", "source_input", "base_capture", "owner_rva",
        "owner_function", "dispatch_start_rva", "switch_rva", "table_rva", "mask", "case_targets",
        "default_policy", "assembly_evidence", "target_execution", "raw_artifacts_modified"}
    for specification in specifications:
        review, review_source = _review(a, specification)
        if architecture == "x86":
            value = (_select_x86_fsm_switch(a, raw, capture, selected_rvas, review, review_source)
                     if isinstance(review, dict) and review.get("review_kind") == "native_x86_printf_fsm_switch_complete_bytes"
                     else _select_x86_switch(a, raw, capture, selected_rvas, review, review_source))
            if value["switch_rva"] in seen:
                raise ValueError("Switch review repeats an indirect jump")
            seen.add(value["switch_rva"])
            values.append(value)
            continue
        if (not isinstance(review, dict) or set(review) != required or review["schema_version"] != 1
                or review["review_kind"] != "native_x64_masked_rel32_switch_complete_bytes"
                or review["source_input"] != source or review["base_capture"] != base_identity
                or review["target_execution"] is not False or review["raw_artifacts_modified"] is not False
                or review["default_policy"] != "all_masked_values_have_explicit_entries"
                or not isinstance(review["assembly_evidence"], str) or not review["assembly_evidence"].strip()
                or any(not isinstance(review[key], str) or not re.fullmatch(r"0x[0-9a-f]+", review[key])
                       or hex(int(review[key], 16)) != review[key]
                       for key in ("owner_rva", "dispatch_start_rva", "switch_rva", "table_rva"))):
            raise ValueError("Switch review lacks complete original identities, range or exhaustive default evidence")
        owner = review["owner_rva"]
        if int(owner, 16) not in selected_rvas or owner not in records or review["switch_rva"] in seen:
            raise ValueError("Switch review selects an unrequested owner or repeated indirect jump")
        record = records[owner]
        path = a.resolve(a.rel(result / record["path"]), a.rel(result))
        identity = evidence.identity(a, path)
        if (record.get("assembly_captured") is not True or identity != review["owner_function"]
                or identity["sha256"] != record["sha256"]
                or ("bytes" in record and identity["bytes"] != record["bytes"])):
            raise ValueError("Switch owner is not the exact complete retained function")
        body = json.loads(path.read_bytes())
        if body.get("rva") != owner or body.get("address") != hex(base + int(owner, 16)):
            raise ValueError("Switch owner identity differs from its complete original function")
        evidence.original_machine(raw, body)
        start, switch, table = (int(review[key], 16)
                                for key in ("dispatch_start_rva", "switch_rva", "table_rva"))
        code = read(start, 21)
        mask = review["mask"]
        if (code[:4] != bytes.fromhex("89c183e1") or type(mask) is not int or not 1 <= mask <= 127
                or mask & (mask + 1) or code[4] != mask or code[5:8] != bytes.fromhex("488d15")
                or code[12:] != bytes.fromhex("48630c8a4801d1ffe1") or switch != start + 19
                or table != start + 12 + int.from_bytes(code[8:12], "little", signed=True)):
            raise ValueError("Switch is not the complete reviewed exhaustive ECX-mask relative32 dispatch")
        assembly = body["assembly"]
        instructions = [ins for ins in assembly if start <= int(ins["rva"], 16) < start + 21]
        offsets, sizes = (0, 2, 5, 12, 16, 19), (2, 3, 7, 4, 3, 2)
        if (len(instructions) != 6 or any(ins.get("rva") != hex(start + offset)
                or ins.get("bytes") != code[offset:offset + size].hex()
                for ins, offset, size in zip(instructions, offsets, sizes))):
            raise ValueError("Switch dispatch has missing or different complete original instruction boundaries")
        for previous, ins in zip(instructions, instructions[1:]):
            if any(ref.get("code") is True and (ref.get("from") != previous["address"] or ref.get("type") != 21)
                   for ref in ins.get("xrefs_to", [])):
                raise ValueError("Switch dispatch has an incoming edge that bypasses its exhaustive mask")
        table_bytes = read(table, (mask + 1) * 4)
        heads = {ins["rva"]: ins for ins in assembly}
        cases = []
        for index in range(mask + 1):
            offset = int.from_bytes(table_bytes[index * 4:index * 4 + 4], "little", signed=True)
            target = table + offset
            instruction = heads.get(hex(target))
            if instruction is None or read(target, len(bytes.fromhex(instruction["bytes"]))).hex() != instruction["bytes"]:
                raise ValueError("Switch target is not an exact original instruction in the same complete owner")
            cases.append({"value": index, "target_rva": hex(target)})
        if review["case_targets"] != cases:
            raise ValueError("Switch review omits or changes an exhaustive masked case target")
        seen.add(review["switch_rva"])
        values.append({"review_source": review_source, "owner_rva": owner, "owner_function": identity,
            "dispatch_start_rva": review["dispatch_start_rva"], "switch_rva": review["switch_rva"],
            "dispatch_raw_hex": code.hex(), "dispatch_instructions": instructions, "mask": mask,
            "table": {"rva": review["table_rva"], "bytes": len(table_bytes), "raw_hex": table_bytes.hex(),
                      "sha256": hashlib.sha256(table_bytes).hexdigest()}, "case_targets": cases,
            "default_policy": review["default_policy"], "assembly_evidence": review["assembly_evidence"],
            "target_execution": False})
    return values


def _select_x86_switch(a, raw, capture, selected_rvas, review, review_source):
    """Prove guarded unsigned32 index domains from complete original x86 control flow."""
    import native_boundaries
    base, read = native_boundaries._instruction_image(raw)
    document = json.loads(evidence.checked(a, {key: review_source[key] for key in ("path", "bytes", "sha256")}))
    if (not re.fullmatch(r"/tables/(0|[1-9][0-9]*)", review_source["pointer"])
            or document.get("schema_version") != 1
            or document.get("review_kind") != "complete_x86_memmove_guarded_absolute_table_static_review"
            or document.get("source_input") != evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
            or document.get("base_capture") != evidence.identity(a, a.ROOT / ".analysis/results/native" / capture["input_sha256"] / "capture.json")
            or document.get("target_execution") is not False or document.get("machine_bytes_modified") is not False
            or not isinstance(review, dict) or set(review) != {"jump_rva", "original_table_base_rva", "entry_bytes",
                "index_low", "index_count", "index_width", "pointer_width", "address_arithmetic", "guard_proof", "entries"}
            or any(review.get(key) != value for key, value in (("entry_bytes", 4), ("index_width", 4),
                ("pointer_width", 4), ("address_arithmetic", "modulo_u32")))
            or not isinstance(review["guard_proof"], str) or not review["guard_proof"].strip()):
        raise ValueError("Absolute switch lacks its complete reviewed x86 input, table or arithmetic identity")
    identity = document.get("function")
    body = json.loads(evidence.checked(a, identity))
    owner = body["rva"]
    records = [row for row in capture["functions"] if row["rva"] == owner]
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    if (int(owner, 16) not in selected_rvas or len(records) != 1 or records[0].get("assembly_captured") is not True
            or evidence.identity(a, result / records[0]["path"]) != identity
            or records[0]["sha256"] != identity["sha256"] or body["address"] != hex(base + int(owner, 16))):
        raise ValueError("Absolute switch owner is not the exact selected complete original function")
    evidence.original_machine(raw, body)
    heads = {int(ins["rva"], 16): ins for ins in body["assembly"]}

    def instruction(at, code=None):
        ins = heads.get(at)
        if ins is None or code is not None and ins.get("bytes") != code.hex():
            raise ValueError("Absolute switch guard differs from an exact original instruction boundary")
        return ins

    def incoming(at):
        ins = instruction(at)
        refs = [ref for ref in ins.get("xrefs_to", []) if ref.get("code") is True]
        found = []
        for ref in refs:
            source = int(ref["from"], 16) - base
            previous = instruction(source)
            if (ref.get("to") != ins["address"] or ref not in previous.get("xrefs_from", [])
                    or source == at or any(row[0] == source for row in found)):
                raise ValueError("Absolute switch guard has an incomplete or contradictory original incoming edge")
            found.append((source, ref["type"]))
        if not found:
            raise ValueError("Absolute switch guard has no complete original incoming-edge proof")
        return found

    def chain(start, codes):
        previous = None
        used = []
        for code in codes:
            ins = instruction(start, code)
            if previous is not None and incoming(start) != [(previous, 21)]:
                raise ValueError("Absolute switch has an incoming edge bypassing its original domain guard")
            used.append(ins)
            previous, start = start, start + len(code)
        return used

    def branch(at, target, condition):
        ins = instruction(at)
        code = bytes.fromhex(ins["bytes"])
        if (not (len(code) == 2 and code[0] == 0x70 + condition
                 or len(code) == 6 and code[:2] == bytes((0x0f, 0x80 + condition)))
                or native_boundaries._branch_target(ins) != base + target):
            raise ValueError("Absolute switch guard is not the exact original conditional branch")
        return ins

    jump, table_base = int(review["jump_rva"], 16), int(review["original_table_base_rva"], 16)
    code = read(jump, 7)
    if (code[:3] not in (b"\xff\x24\x85", b"\xff\x24\x8d")
            or int.from_bytes(code[3:], "little") != base + table_base):
        raise ValueError("Absolute switch is not the exact unsigned32 indexed original jump")
    instruction(jump, code)
    low, count = review["index_low"], review["index_count"]
    if type(low) is not int or type(count) is not int:
        raise ValueError("Absolute switch domain must use exact integer indices")
    guards = []
    if code[2] == 0x85 and (low, count) == (1, 3):
        start = jump - 17
        prefix = read(start, 17)
        if (prefix[:7] != bytes.fromhex("8bc7ba03000000") or prefix[7:10] not in (bytes.fromhex("83e904"), bytes.fromhex("83f904"))
                or prefix[10] != 0x72 or prefix[12:15] != bytes.fromhex("83e003")
                or prefix[15:17] != (bytes.fromhex("03c8") if prefix[8] == 0xe9 else bytes.fromhex("2bc8"))):
            raise ValueError("Absolute alignment switch lacks its complete guarded original EDI mask sequence")
        guarded = incoming(start)
        if len(guarded) != 1 or guarded[0][1] != 19:
            raise ValueError("Absolute alignment switch has a bypass of the nonzero EDI guard")
        source = guarded[0][0]
        test = source - 6
        guards += chain(test, [bytes.fromhex("f7c703000000"), bytes.fromhex(branch(source, start, 5)["bytes"])])
        guards += chain(start, [prefix[:2], prefix[2:7], prefix[7:10], prefix[10:12], prefix[12:15], prefix[15:17], code])
        expression = "eax"
    elif code[2] == 0x8d and (low, count) == (-4, 4):
        edges = incoming(jump)
        if len(edges) != 1 or edges[0][1] != 19:
            raise ValueError("Absolute tiny-count switch has a bypass of its borrow guard")
        source = edges[0][0]
        guards += chain(source - 3, [bytes.fromhex("83e904"), bytes.fromhex(branch(source, jump, 2)["bytes"])])
        start, expression = source - 3, "ecx"
    elif code[2] == 0x8d and (low, count) == (-7, 8):
        start, expression = jump - 2, "ecx"
        guards += chain(start, [bytes.fromhex("f7d9"), code])
        for source, kind in incoming(start):
            if kind != 19:
                raise ValueError("Absolute negative-count switch has an unguarded incoming edge")
            guards += chain(source - 3, [bytes.fromhex("83f908"), bytes.fromhex(branch(source, start, 2)["bytes"])])
    else:
        raise ValueError("Absolute switch domain is not an exhaustively proven original guard form")
    entries = review["entries"]
    if not isinstance(entries, list) or len(entries) != count:
        raise ValueError("Absolute switch review omits a reachable original index")
    table = (table_base + 4 * low) & 0xffffffff
    table_bytes = read(table, count * 4)
    cases = []
    for index, entry in zip(range(low, low + count), entries):
        slot = (table_base + 4 * (index & 0xffffffff)) & 0xffffffff
        raw_entry = read(slot, 4)
        target = int.from_bytes(raw_entry, "little") - base
        target_instruction = instruction(target)
        if (not isinstance(entry, dict) or set(entry) != {"index", "index_u32", "entry_rva", "raw_hex", "target_rva", "target_instruction"}
                or entry != {"index": index, "index_u32": index & 0xffffffff, "entry_rva": hex(slot),
                    "raw_hex": raw_entry.hex(), "target_rva": hex(target), "target_instruction": target_instruction}):
            raise ValueError("Absolute switch entry changes original modulo32 address, bytes or complete target instruction")
        cases.append({"value": index, "target_rva": hex(target)})
    return {"review_source": review_source, "owner_rva": owner, "owner_function": identity,
        "dispatch_kind": "x86_guarded_absolute32", "dispatch_start_rva": hex(jump), "switch_rva": hex(jump),
        "dispatch_raw_hex": code.hex(), "guard_instructions": guards, "index_low": low, "index_count": count,
        "expression_start_rva": hex(start), "expression_rva": hex(jump), "expression_register": expression,
        "table": {"rva": hex(table), "original_base_rva": hex(table_base), "bytes": len(table_bytes),
                  "raw_hex": table_bytes.hex(), "sha256": hashlib.sha256(table_bytes).hexdigest()},
        "case_targets": cases, "default_policy": "all_guarded_values_have_explicit_entries",
        "assembly_evidence": review["guard_proof"], "target_execution": False}


def _fsm_graph(body, switch, case_targets):
    """Check reciprocal original CFG edges and every ordinary branch byte."""
    import native_boundaries
    heads = {int(row["address"], 16): row for row in body["assembly"]}
    if len(heads) != len(body["assembly"]):
        raise ValueError("FSM proof repeats an original instruction boundary")
    entry = int(body["address"], 16)
    graph, parents = {}, {at: set() for at in heads}
    for at, row in heads.items():
        code = bytes.fromhex(row["bytes"])
        edges = [ref for ref in row.get("xrefs_from", []) if ref.get("code") is True and ref.get("type") != 17]
        actual = {(int(ref["to"], 16), ref["type"]) for ref in edges}
        if len(actual) != len(edges) or any(ref.get("from") != row["address"] for ref in edges):
            raise ValueError("FSM graph has duplicate or contradictory original outgoing edges")
        if at == switch:
            expected = {(target, 19) for target in case_targets}
        elif code[0] in {0xc2, 0xc3}:
            expected = set()
        elif (len(code) == 2 and (0x70 <= code[0] <= 0x7f or code[0] == 0xeb)
                or len(code) == 5 and code[0] == 0xe9
                or len(code) == 6 and code[0] == 0x0f and 0x80 <= code[1] <= 0x8f):
            expected = {(native_boundaries._branch_target(row), 19)}
            if code[0] not in {0xe9, 0xeb}:
                expected.add((at + len(code), 21))
        elif (code[0] == 0xff and len(code) >= 2 and (code[1] >> 3) & 7 in {4, 5}
                or code[0] in {0x9a, 0xca, 0xcb, 0xcc, 0xcd, 0xcf, 0xea, 0xf4}
                or 0xe0 <= code[0] <= 0xe3):
            raise ValueError("FSM graph has an unreviewed control transfer")
        else:
            expected = {(at + len(code), 21)}
        if actual != expected:
            raise ValueError("FSM graph does not retain every original branch and fallthrough")
        graph[at] = {target for target, _ in actual}
        for ref in edges:
            target = int(ref["to"], 16)
            if target not in heads or ref not in heads[target].get("xrefs_to", []):
                raise ValueError("FSM graph omits an exact reciprocal original incoming edge")
            parents[target].add(at)
    for at, row in heads.items():
        for ref in row.get("xrefs_to", []):
            if ref.get("code") is not True:
                continue
            source = int(ref["from"], 16)
            if source not in heads:
                if at != entry:
                    raise ValueError("FSM graph has an external incoming edge bypassing initialization")
            elif ref not in heads[source].get("xrefs_from", []) or source not in parents[at]:
                raise ValueError("FSM graph contains an unmatched original incoming edge")
    return heads, graph, parents


def _fsm_zero_source(heads, parents, start, register):
    """Prove all backwards paths preserve a register back to a self-XOR."""
    pending, visited, zeroes = list(parents[start]), set(), set()
    if not pending:
        raise ValueError("FSM initialization has no original zero-source path")
    while pending:
        at = pending.pop()
        if at in visited:
            continue
        visited.add(at)
        code = bytes.fromhex(heads[at]["bytes"])
        opcode = code[0]
        if code == bytes((0x33, 0xc0 + register * 9)) or code == bytes((0x31, 0xc0 + register * 9)):
            zeroes.add(at)
            continue
        safe = False
        if (opcode in {0x39, 0x3b, 0x3c, 0x3d, 0x84, 0x85, 0xe9, 0xeb}
                or len(code) == 2 and 0x70 <= opcode <= 0x7f
                or len(code) == 6 and code[:1] == b"\x0f" and 0x80 <= code[1] <= 0x8f):
            safe = True
        elif opcode in {0x88, 0x89, 0x31, 0x33, 0x8b, 0x8a, 0x8d, 0x83} and len(code) >= 2:
            mod, middle, low = code[1] >> 6, (code[1] >> 3) & 7, code[1] & 7
            if opcode == 0x88:
                safe = mod != 3 or low % 4 != register
            elif opcode in {0x89, 0x31}:
                safe = mod != 3 or low != register
            elif opcode in {0x33, 0x8b, 0x8d}:
                safe = middle != register
            elif opcode == 0x8a:
                safe = middle % 4 != register
            else:
                safe = mod != 3 or middle == 7 or low != register
        elif 0x40 <= opcode <= 0x47:
            safe = opcode - 0x40 != register
        if not safe or not parents[at]:
            raise ValueError("FSM state initialization does not have a complete unchanged zero-source chain at "
                             + hex(at) + ": " + code.hex())
        pending.extend(parents[at])
    if not zeroes:
        raise ValueError("FSM initialization has no proven zero definition")
    return [heads[at] for at in sorted(visited)]


def _select_x86_fsm_switch(a, raw, capture, selected_rvas, review, review_source):
    """Prove the full nine-state printf transition domain before EBX dispatch."""
    import native_boundaries
    required = {"schema_version", "review_kind", "source_input", "base_capture", "owner_rva", "owner_function",
        "classification_start_rva", "transition_table_rva", "character_class_table_rva", "character_class_raw_hex",
        "transition_raw_hex", "state_references", "switch_rva", "table_rva", "case_targets", "assembly_evidence",
        "target_execution", "raw_artifacts_modified"}
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    base, read = native_boundaries._instruction_image(raw)
    if (set(review) != required or review["schema_version"] != 1
            or review["source_input"] != evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
            or review["base_capture"] != evidence.identity(a, result / "capture.json")
            or review["target_execution"] is not False or review["raw_artifacts_modified"] is not False
            or any(not isinstance(review[key], str) or not re.fullmatch(r"0x[0-9a-f]+", review[key])
                   or hex(int(review[key], 16)) != review[key]
                   for key in ("owner_rva", "classification_start_rva", "transition_table_rva", "character_class_table_rva", "switch_rva", "table_rva"))
            or int(review["owner_rva"], 16) not in selected_rvas
            or not isinstance(review["assembly_evidence"], str) or not review["assembly_evidence"].strip()):
        raise ValueError("FSM switch review lacks exact complete input, function, table or domain identities")
    identity = review["owner_function"]
    body = json.loads(evidence.checked(a, identity))
    records = [row for row in capture["functions"] if row["rva"] == review["owner_rva"]]
    if (len(records) != 1 or records[0].get("assembly_captured") is not True
            or evidence.identity(a, result / records[0]["path"]) != identity
            or records[0]["sha256"] != identity["sha256"] or body["rva"] != review["owner_rva"]
            or body["address"] != hex(base + int(review["owner_rva"], 16))):
        raise ValueError("FSM switch owner is not the selected complete original function")
    evidence.original_machine(raw, body)
    switch, table, classify = (int(review[key], 16) for key in ("switch_rva", "table_rva", "classification_start_rva"))
    class_table, transitions = (int(review[key], 16) for key in ("character_class_table_rva", "transition_table_rva"))
    class_bytes, transition_bytes, table_bytes = read(class_table, 89), read(transitions, 81), read(table, 32)
    if (class_bytes.hex() != review["character_class_raw_hex"] or transition_bytes.hex() != review["transition_raw_hex"]
            or {value & 15 for value in class_bytes} != set(range(9))
            or {value >> 4 for value in transition_bytes} != set(range(9))):
        raise ValueError("FSM switch does not retain the complete 89-character and 81-transition byte domains")
    expected_cases = [{"value": index, "target_rva": hex(int.from_bytes(table_bytes[index * 4:index * 4 + 4], "little") - base)}
                      for index in range(8)]
    if review["case_targets"] != expected_cases:
        raise ValueError("FSM switch review omits or changes an original state 0..7 target")
    jump_code = bytes.fromhex("ff249d") + (base + table).to_bytes(4, "little")
    if read(switch, 7) != jump_code:
        raise ValueError("FSM switch is not the exact original EBX-indexed absolute dispatch")
    heads, graph, parents = _fsm_graph(body, base + switch, {base + int(row["target_rva"], 16) for row in expected_cases})

    def exact(at, code):
        address = base + at
        if address not in heads or heads[address]["bytes"] != code.hex():
            raise ValueError("FSM proof differs from an exact original instruction boundary")
        return heads[address]

    # Each join is checked against all original incoming edges; classes outside
    # ASCII 0x20..0x78 explicitly use zero, never an unchecked table offset.
    prefix = [bytes.fromhex("8d41e0"), bytes.fromhex("3c58"), bytes.fromhex("770f"), bytes.fromhex("0fbec1"),
        bytes.fromhex("0fb680") + (base + class_table - 0x20).to_bytes(4, "little"), bytes.fromhex("83e00f"),
        bytes.fromhex("eb02"), bytes.fromhex("33c0"), bytes.fromhex("8b8d60f7ffff"), bytes.fromhex("6bc009"),
        bytes.fromhex("0fb68c08") + (base + transitions).to_bytes(4, "little"), bytes.fromhex("c1e904"),
        bytes.fromhex("898d60f7ffff"), bytes.fromhex("83f901"), bytes.fromhex("0f85e3000000")]
    at, proof, offsets = classify, [], []
    for code in prefix:
        proof.append(exact(at, code)); offsets.append(at); at += len(code)
    for index in range(1, len(offsets)):
        expected = {base + offsets[index - 1]}
        if index == 7: expected = {base + offsets[2]}
        elif index == 8: expected = {base + offsets[6], base + offsets[7]}
        if parents[base + offsets[index]] != expected:
            raise ValueError("FSM classification or transition has an incoming edge bypassing its full domain proof")
    store = offsets[12]
    one_branch = offsets[14]
    eight_compare = one_branch + 6 + 0xe3
    exact(eight_compare, bytes.fromhex("83f908"))
    eight_branch = eight_compare + 3
    branch_code = bytes.fromhex(exact(eight_branch, read(eight_branch, 6))["bytes"])
    if branch_code[:2] != bytes.fromhex("0f84") or parents[base + eight_compare] != {base + one_branch}:
        raise ValueError("FSM invalid state eight is not rejected on the complete non-state-one path")
    error = native_boundaries._branch_target(heads[base + eight_branch])
    if error in {base + switch, base + switch - 6} or parents[base + eight_branch] != {base + eight_compare}:
        raise ValueError("FSM invalid state path bypasses its original reject branch")
    reload = switch - 6
    exact(reload, bytes.fromhex("8b9d60f7ffff"))
    exact(switch, jump_code)
    if parents[base + switch] != {base + reload}:
        raise ValueError("FSM switch can bypass the exact original state reload")
    references = [row for row in body["assembly"]
                  if any(((-0x8a0 + delta) & 0xffffffff).to_bytes(4, "little") in bytes.fromhex(row["bytes"]) for delta in range(4))]
    if review["state_references"] != references or len(references) != 6:
        raise ValueError("FSM proof omits a state write, read, partial overlap or address escape")
    pattern = {"898d60f7ffff", "899560f7ffff", "898560f7ffff", "8b8d60f7ffff", "8b9d60f7ffff"}
    if any(row["bytes"] not in pattern for row in references):
        raise ValueError("FSM state is accessed through an unreviewed operand")
    stores = [row for row in references if row["bytes"].startswith("89")]
    if len(stores) != 3 or sum(row["rva"] == hex(store) for row in stores) != 1:
        raise ValueError("FSM proof omits the initial, transition or recovery state write")
    zero_stores = {int(row["address"], 16) for row in stores if row["rva"] != hex(store)}
    for address in sorted(zero_stores):
        code = bytes.fromhex(heads[address]["bytes"])
        proof.extend(_fsm_zero_source(heads, parents, address, (code[1] >> 3) & 7))
    # Finite data flow retains uninitialized paths. All 81 transitions are
    # evaluated for every reachable old state and all nine character classes.
    entry, state_at = int(body["address"], 16), {address: set() for address in heads}
    state_at[entry] = {None}
    pending, queued = [entry], {entry}
    while pending:
        address = pending.pop(); queued.remove(address)
        values = state_at[address]
        if address in zero_stores:
            values = {0}
        elif address == base + store:
            if None in values or not values <= set(range(9)):
                raise ValueError("FSM transition reads an uninitialized or out-of-domain previous state")
            values = {transition_bytes[category * 9 + state] >> 4 for state in values for category in range(9)}
        for target in graph[address]:
            propagated = values
            if address == base + one_branch:
                propagated = {value for value in values if (value != 1) == (target == base + eight_compare)}
            elif address == base + eight_branch:
                propagated = {value for value in values if (value == 8) == (target == error)}
            if not propagated <= state_at[target]:
                state_at[target].update(propagated)
                if target not in queued: pending.append(target); queued.add(target)
    if state_at[base + switch] != set(range(8)):
        raise ValueError("FSM switch index lacks the complete initialized state 0..7 domain")
    domain = {"state_domain": list(range(9)), "dispatch_domain": sorted(state_at[base + switch]),
        "state_one_branch_rva": hex(one_branch), "state_eight_branch_rva": hex(eight_branch),
        "state_eight_error_rva": hex(error - base), "zero_store_rvas": [hex(at - base) for at in sorted(zero_stores)],
        "complete_graph_instructions": len(heads), "complete_graph_edges": sum(map(len, graph.values()))}
    # Keep full graph, initialization and state references in the request so
    # the private IDB must still have all original proof bytes at application.
    return {"review_source": review_source, "owner_rva": body["rva"], "owner_function": identity,
        "dispatch_kind": "x86_printf_fsm_absolute32", "dispatch_start_rva": hex(switch), "switch_rva": hex(switch),
        "dispatch_raw_hex": jump_code.hex(), "guard_instructions": body["assembly"], "index_low": 0, "index_count": 8,
        "expression_start_rva": hex(reload), "expression_rva": hex(switch), "expression_register": "ebx",
        "table": {"rva": hex(table), "bytes": 32, "raw_hex": table_bytes.hex(), "sha256": hashlib.sha256(table_bytes).hexdigest()},
        "lookup_tables": [{"rva": hex(at), "bytes": len(code), "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest()}
                          for at, code in ((class_table, class_bytes), (transitions, transition_bytes))],
        "state_references": references, "domain_proof": domain, "case_targets": expected_cases,
        "default_policy": "all_initialized_fsm_dispatch_values_have_explicit_entries",
        "assembly_evidence": review["assembly_evidence"], "target_execution": False}


def _switch_state(info):
    if info is None:
        return None
    return {**{key: int(getattr(info, key)) for key in ("flags", "ncases", "jumps", "lowcase", "defjump",
        "startea", "elbase", "regnum", "regdtype", "expr_ea", "jcases", "ind_lowcase")},
        "element_bytes": int(info.get_jtable_element_size()), "shift": int(info.get_shift()),
        "table_size": int(info.get_jtable_size()), "has_default": bool(info.has_default()),
        "marks": [int(value) for value in info.marks]}


def _switch_cases(sdk, address, info, base):
    observed = sdk.xref.calc_switch_cases(address, info)
    if observed is None or len(observed.cases) != len(observed.targets):
        raise ValueError("SDK did not return complete switch cases and targets")
    return sorted(({"value": int(value), "target_rva": hex(int(target) - base)}
                   for group, target in zip(observed.cases, observed.targets) for value in group),
                  key=lambda row: row["value"])


def _x86_switch_cases(raw_cases, signed):
    result = []
    for row in raw_cases:
        value = row["value"]
        if (type(value) is not int or not -(1 << 31) <= value <= 0xffffffffffffffff
                or value > 0xffffffff and value < 0xffffffff80000000):
            raise ValueError("SDK x86 case value is not an exact zero/sign extension of its 32-bit expression: " + repr(value))
        value &= 0xffffffff
        if signed and value & 0x80000000:
            value -= 1 << 32
        result.append({"value": value, "target_rva": row["target_rva"]})
    return sorted(result, key=lambda row: row["value"])


def apply_switch(requested, base, sdk):
    """Repair only copied-IDB switch metadata using every original table entry."""
    import native_boundaries
    raw = (sdk.root / requested["owner_function"]["path"]).read_bytes()
    if len(raw) != requested["owner_function"]["bytes"] or hashlib.sha256(raw).hexdigest() != requested["owner_function"]["sha256"]:
        raise ValueError("Switch metadata repair original owner changed")
    body = json.loads(raw)
    absolute = requested.get("dispatch_kind") in {"x86_guarded_absolute32", "x86_printf_fsm_absolute32"}
    start, ea = (base + int(requested[key], 16) for key in ("dispatch_start_rva", "switch_rva"))
    table = requested["table"]
    table_ea = base + int(table["rva"], 16)
    if (sdk.bytes.get_bytes(start, len(bytes.fromhex(requested["dispatch_raw_hex"]))) != bytes.fromhex(requested["dispatch_raw_hex"])
            or sdk.bytes.get_bytes(table_ea, table["bytes"]) != bytes.fromhex(table["raw_hex"])):
        raise ValueError("Private IDB switch dispatch or complete table differs from original bytes")
    if absolute and any(sdk.bytes.get_bytes(int(ins["address"], 16), len(bytes.fromhex(ins["bytes"])))
                        != bytes.fromhex(ins["bytes"]) for ins in requested["guard_instructions"]):
        raise ValueError("Private IDB absolute switch guard differs from the complete original proof")
    if any(sdk.bytes.get_bytes(base + int(row["rva"], 16), row["bytes"]) != bytes.fromhex(row["raw_hex"])
           for row in requested.get("lookup_tables", [])):
        raise ValueError("Private IDB FSM lookup table differs from the complete original byte domain")
    before = native_boundaries._snapshot(body, sdk)
    previous = _switch_state(sdk.nalt.get_switch_info(ea))
    info = sdk.nalt.switch_info_t()
    constants = {key: int(getattr(sdk.nalt, key))
                 for key in ("SWI_USER", "SWI_SIGNED", "SWI_ELBASE", "SWI_J32", "SWI_VERSION")}
    constructor_flags = int(info.flags)
    if constructor_flags & ~constants["SWI_VERSION"]:
        raise ValueError("SDK new switch contains unreviewed non-version flags")
    info.flags = constants["SWI_USER"] | constants["SWI_VERSION"] | (0 if absolute else constants["SWI_SIGNED"])
    info.set_jtable_element_size(4)
    info.set_elbase(0 if absolute else table_ea)
    info.ncases = len(requested["case_targets"])
    info.jumps, info.lowcase = table_ea, requested.get("index_low", 0) & 0xffffffffffffffff
    info.defjump = sdk.api.BADADDR
    info.startea = base + int(requested["expression_start_rva"], 16) if absolute else start + 2
    info.set_expr(sdk.idp.str2reg(requested.get("expression_register", "ecx")), sdk.ua.dt_dword)
    info.expr_ea = base + int(requested["expression_rva"], 16) if absolute else start + 12
    expected = _switch_state(info)
    if expected["regnum"] < 0 or expected["has_default"]:
        raise ValueError("SDK cannot represent the exhaustive original masked expression")
    sdk.nalt.set_switch_info(ea, info)
    stored = sdk.nalt.get_switch_info(ea)
    observed = _switch_state(stored)
    if observed != expected:
        raise ValueError("SDK switch metadata readback differs from its exact reviewed request: "
                         + json.dumps({"expected": expected, "observed": observed,
                                       "constructor_flags": constructor_flags}, sort_keys=True))
    raw_calculated = _switch_cases(sdk, ea, stored, base)
    calculated = _x86_switch_cases(raw_calculated, requested["index_low"] < 0) if absolute else raw_calculated
    if calculated != requested["case_targets"]:
        raise ValueError("SDK switch case computation omits or changes an original masked target")
    if not sdk.xref.create_switch_xrefs(ea, stored):
        raise ValueError("SDK could not create the complete reviewed switch target cross-references")
    actual_targets = sorted(set(int(ref.to) - base for ref in sdk.utils.XrefsFrom(ea)
                                if ref.iscode))
    if actual_targets != sorted({int(row["target_rva"], 16) for row in requested["case_targets"]}):
        raise ValueError("SDK switch target cross-references omit or add an original branch")
    sdk.hexrays.mark_cfunc_dirty(base + int(requested["owner_rva"], 16), False)
    after = native_boundaries._snapshot(body, sdk)
    if before != after or sdk.bytes.get_bytes(table_ea, table["bytes"]) != bytes.fromhex(table["raw_hex"]):
        raise ValueError("Switch metadata repair changed original bytes, boundaries, frame or stack analysis")
    return {**requested, "previous_switch": previous, "sdk_switch_constants": constants,
        "constructor_switch_flags": constructor_flags, "expected_switch": expected,
        "applied_switch": _switch_state(sdk.nalt.get_switch_info(ea)), "sdk_case_targets": calculated,
        **({"sdk_case_targets_raw": raw_calculated} if absolute else {}),
        "sdk_xref_targets": [hex(value) for value in actual_targets], "before": before, "after": after,
        "switch_metadata_readback_matches": True, "switch_xrefs_created": True,
        "manual_stack_points_modified": False, "manual_frames_modified": False,
        "instructions_patched": False, "base_database_modified": False}


def verify_switches(a, requested, applied):
    import native_boundaries
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("Switch metadata repair omitted an exact reviewed request")
    for expected, actual in zip(requested, applied):
        if (not isinstance(actual, dict) or any(actual.get(key) != value for key, value in expected.items())
                or actual.get("switch_metadata_readback_matches") is not True
                or actual.get("switch_xrefs_created") is not True
                or any(actual.get(key) is not False for key in ("manual_stack_points_modified", "manual_frames_modified",
                                                              "instructions_patched", "base_database_modified"))
                or actual.get("before") != actual.get("after") or actual.get("sdk_case_targets") != expected["case_targets"]
                or actual.get("sdk_xref_targets") != sorted({row["target_rva"] for row in expected["case_targets"]}, key=lambda r: int(r, 16))
                or actual.get("applied_switch") != actual.get("expected_switch")):
            raise ValueError("Switch metadata repair changed scope, original machine state or exhaustive targets")
        body = json.loads(evidence.checked(a, expected["owner_function"]))
        native_boundaries._verify_snapshot(body, actual.get("before"))
        state = actual.get("applied_switch", {})
        constants = actual.get("sdk_switch_constants", {})
        absolute = expected.get("dispatch_kind") in {"x86_guarded_absolute32", "x86_printf_fsm_absolute32"}
        if (not isinstance(state, dict) or not isinstance(constants, dict)
                or set(constants) != {"SWI_USER", "SWI_SIGNED", "SWI_ELBASE", "SWI_J32", "SWI_VERSION"}
                or any(type(value) is not int or value <= 0 or value & (value - 1) for value in constants.values())
                or len(set(constants.values())) != 5
                or state.get("flags") != sum(constants.values()) - (constants["SWI_SIGNED"] if absolute else 0)
                or type(actual.get("constructor_switch_flags")) is not int
                or actual["constructor_switch_flags"] & ~constants["SWI_VERSION"]):
            raise ValueError("Switch metadata repair omitted its exact signed32 SDK flags")
        base = int(body["address"], 16) - int(body["rva"], 16)
        start = base + int(expected["dispatch_start_rva"], 16)
        if absolute:
            if _x86_switch_cases(actual.get("sdk_case_targets_raw", []), expected["index_low"] < 0) != expected["case_targets"]:
                raise ValueError("SDK x86 raw case indices do not preserve the exact original 32-bit domain")
        if (state.get("ncases") != len(expected["case_targets"]) or state.get("table_size") != len(expected["case_targets"])
                or state.get("jumps") != base + int(expected["table"]["rva"], 16)
                or state.get("elbase") != (0 if absolute else state.get("jumps")) or state.get("element_bytes") != 4
                or state.get("lowcase") != (expected.get("index_low", 0) & 0xffffffffffffffff)
                or state.get("shift") != 0 or state.get("has_default") is not False
                or state.get("defjump") != 0xffffffffffffffff
                or state.get("startea") != (base + int(expected["expression_start_rva"], 16) if absolute else start + 2)
                or state.get("expr_ea") != (base + int(expected["expression_rva"], 16) if absolute else start + 12)
                or type(state.get("regnum")) is not int or state["regnum"] < 0):
            raise ValueError("Switch metadata repair lacks exact signed32 table or masked-expression readback")
