"""Independent machine proofs for exact PE32 variadic import jumps; no cfunc synthesis."""
from __future__ import annotations

from dataclasses import asdict, dataclass, replace
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import tempfile

import native_adjudication as evidence


_SOURCE = Path(__file__).read_bytes()


def source_identity(root):
    path = Path(__file__).resolve()
    if path.read_bytes() != _SOURCE:
        raise ValueError("Import-forwarder validator changed after import")
    return {"path": path.relative_to(root).as_posix(), "bytes": len(_SOURCE),
            "sha256": hashlib.sha256(_SOURCE).hexdigest()}


def _u32(value):
    if type(value) is not int or not 0 <= value <= 0xffffffff:
        raise ValueError("x86 coordinate is not an exact unsigned 32-bit value")
    return value


def _address(value):
    if not isinstance(value, str) or not re.fullmatch(r"0x[0-9a-f]+", value):
        raise ValueError("Import-forwarder address is not canonical hexadecimal")
    result = _u32(int(value, 16))
    if hex(result) != value:
        raise ValueError("Import-forwarder address is not canonical hexadecimal")
    return result


@dataclass(frozen=True)
class Cpu32:
    eax: int
    ecx: int
    edx: int
    ebx: int
    esp: int
    ebp: int
    esi: int
    edi: int
    eflags: int
    eip: int
    stack_bytes: bytes = b""

    def __post_init__(self):
        for key, value in asdict(self).items():
            if key != "stack_bytes":
                _u32(value)
        if not isinstance(self.stack_bytes, bytes):
            raise ValueError("x86 stack witness must preserve its exact immutable bytes")


@dataclass(frozen=True)
class ImportJump32:
    entry_virtual_address: int
    iat_virtual_address: int

    def __post_init__(self):
        _u32(self.entry_virtual_address)
        _u32(self.iat_virtual_address)

    def normal_step(self, before: Cpu32, loaded_iat_value: int) -> Cpu32:
        """Conditional normal architectural step after valid instruction/IAT fetch and target checks."""
        if not isinstance(before, Cpu32) or before.eip != self.entry_virtual_address:
            raise ValueError("Normal import-jump step starts at a different original instruction")
        return replace(before, eip=_u32(loaded_iat_value))


class Pe32:
    """Read only original file bytes with unique section mappings and exact import identities."""
    def __init__(self, raw):
        self.raw = raw
        if raw[:2] != b"MZ":
            raise ValueError("Import jump requires an original PE32 image")
        self.pe = self.unpack("<I", 0x3c)[0]
        if raw[self.pe:self.pe + 4] != b"PE\0\0":
            raise ValueError("Import jump original PE signature is invalid")
        machine, count, _, _, _, optional_size, flags = self.unpack("<HHIIIHH", self.pe + 4)
        self.optional = self.pe + 24
        if (machine != 0x14c or self.unpack("<H", self.optional)[0] != 0x10b
                or not flags & 0x100 or not 1 <= count <= 96 or optional_size < 112):
            raise ValueError("Import jump requires machine 0x14c and PE32 default 32-bit code")
        self.base = self.unpack("<I", self.optional + 28)[0]
        directories = self.unpack("<I", self.optional + 92)[0]
        if directories < 2 or 96 + directories * 8 > optional_size:
            raise ValueError("Import jump PE32 directories are incomplete")
        self.import_rva, self.import_bytes = self.unpack("<II", self.optional + 104)
        self.sections = []
        for index in range(count):
            at = self.optional + optional_size + index * 40
            _, _, rva, size, offset, _, _, _, _, characteristics = self.unpack("<8sIIIIIIHHI", at)
            if offset + size > len(raw) or rva + size > 0x100000000:
                raise ValueError("Import jump section exceeds original bytes or address space")
            self.sections.append((rva, rva + size, offset, bool(characteristics & 0x20000000)))

    def unpack(self, fmt, at):
        size = struct.calcsize(fmt)
        if at < 0 or at + size > len(self.raw):
            raise ValueError("Import jump PE structure is truncated")
        return struct.unpack_from(fmt, self.raw, at)

    def at(self, rva, size, executable=False):
        _u32(rva)
        if type(size) is not int or size <= 0 or rva + size > 0x100000000:
            raise ValueError("Import jump PE range is invalid")
        overlapping = [s for s in self.sections if s[0] < rva + size and rva < s[1]]
        if (len(overlapping) != 1 or overlapping[0][0] > rva or overlapping[0][1] < rva + size
                or executable and not overlapping[0][3]):
            raise ValueError("Import jump range is not uniquely file-backed executable code" if executable
                             else "Import jump range is not uniquely file-backed")
        return overlapping[0][2] + rva - overlapping[0][0]

    def bytes(self, rva, size, executable=False):
        at = self.at(rva, size, executable)
        return self.raw[at:at + size]

    def string(self, rva):
        at = self.at(rva, 1)
        end = self.raw.find(b"\0", at)
        if end < 0:
            raise ValueError("Import jump name is unterminated")
        raw = self.bytes(rva, end - at + 1)
        try:
            name = raw[:-1].decode("ascii")
        except UnicodeDecodeError as error:
            raise ValueError("Import jump name is not ASCII") from error
        if not name:
            raise ValueError("Import jump name is empty")
        return name, raw.hex()

    def import_slot(self, iat_rva):
        if not self.import_rva or self.import_bytes < 40 or self.import_bytes % 20:
            raise ValueError("Import jump descriptor directory is incomplete")
        self.at(self.import_rva, self.import_bytes)
        found, terminated = [], False
        for relative in range(0, self.import_bytes, 20):
            descriptor_rva = self.import_rva + relative
            descriptor = self.bytes(descriptor_rva, 20)
            lookup, timestamp, chain, library_rva, first_iat = struct.unpack("<IIIII", descriptor)
            if not any(descriptor):
                terminated = True
                break
            if not lookup or not first_iat or not library_rva:
                raise ValueError("Import jump requires an explicit original lookup, IAT and library name")
            library, library_hex = self.string(library_rva)
            table_terminated = False
            for index in range(len(self.raw) // 4):
                lookup_rva, slot_rva = lookup + index * 4, first_iat + index * 4
                lookup_bytes, iat_bytes = self.bytes(lookup_rva, 4), self.bytes(slot_rva, 4)
                value = struct.unpack("<I", lookup_bytes)[0]
                if value == 0:
                    if iat_bytes != b"\0" * 4:
                        raise ValueError("Import jump lookup and IAT terminators disagree")
                    table_terminated = True
                    break
                if slot_rva != iat_rva:
                    continue
                if value & 0x80000000 or iat_bytes != lookup_bytes or timestamp or chain:
                    raise ValueError("Import jump is not an exact unbound named original import")
                hint = struct.unpack("<H", self.bytes(value, 2))[0]
                name, name_hex = self.string(value + 2)
                found.append({"library": library, "name": name, "descriptor_rva": hex(descriptor_rva),
                    "descriptor_raw_hex": descriptor.hex(), "lookup_rva": hex(lookup_rva),
                    "lookup_raw_hex": lookup_bytes.hex(), "iat_rva": hex(slot_rva), "iat_raw_hex": iat_bytes.hex(),
                    "lookup_table_rva": hex(lookup), "iat_table_rva": hex(first_iat), "slot_index": index,
                    "library_name_rva": hex(library_rva), "library_name_raw_hex": library_hex,
                    "hint_name_rva": hex(value), "hint": hint, "name_rva": hex(value + 2), "name_raw_hex": name_hex})
            if not table_terminated:
                raise ValueError("Import jump lookup table has no complete terminator")
        if not terminated or len(found) != 1:
            raise ValueError("Import jump lacks one unique terminated original IAT binding")
        return found[0]


def _complete_machine(pe, body):
    chunks, instructions = body.get("chunks"), body.get("assembly")
    entry, end = _address(body.get("address")), _address(body.get("end"))
    if not isinstance(chunks, list) or not chunks or not isinstance(instructions, list) or not instructions:
        raise ValueError("Import-forwarder caller omitted complete original code")
    if _address(body.get("rva")) + pe.base != entry:
        raise ValueError("Import-forwarder caller entry or RVA differs from the original image")
    ranges, rows = [], {}
    for chunk in chunks:
        first, stop = _address(chunk.get("start")), _address(chunk.get("end"))
        code = pe.bytes(first - pe.base, stop - first, executable=True)
        if chunk.get("raw_hex") != code.hex() or chunk.get("sha256") != hashlib.sha256(code).hexdigest():
            raise ValueError("Import-forwarder caller chunk differs from original executable bytes")
        ranges.append((first, stop))
    if (len(set(ranges)) != len(ranges) or any(a[1] > b[0] for a, b in zip(sorted(ranges), sorted(ranges)[1:]))
            or (entry, end) not in ranges):
        raise ValueError("Import-forwarder caller chunks overlap or omit its exact entry boundary")
    for ins in instructions:
        at, rva = _address(ins.get("address")), _address(ins.get("rva"))
        try:
            code = bytes.fromhex(ins["bytes"])
        except (ValueError, TypeError, KeyError) as error:
            raise ValueError("Import-forwarder caller instruction bytes are invalid") from error
        if (at in rows or at != pe.base + rva or not 1 <= len(code) <= 15
                or not any(first <= at < at + len(code) <= stop for first, stop in ranges)
                or pe.bytes(rva, len(code), executable=True) != code):
            raise ValueError("Import-forwarder caller instruction identity, extent or bytes differ")
        rows[at] = ins
    visited = set()
    for first, stop in ranges:
        at = first
        while at < stop:
            if at not in rows:
                raise ValueError("Import-forwarder caller instruction coverage is truncated")
            visited.add(at)
            at += len(bytes.fromhex(rows[at]["bytes"]))
        if at != stop:
            raise ValueError("Import-forwarder caller instruction extends beyond its complete chunk")
    if visited != set(rows):
        raise ValueError("Import-forwarder caller contains overlapping original instructions")
    return rows


def _xref(source, target, kind, code):
    return {"from": hex(source), "to": hex(target), "type": kind, "code": code, "user": False}


def decode_import_jump(pe, body):
    """Decode the sole original x86 instruction and its named unbound IAT operand."""
    rows = _complete_machine(pe, body)
    entry = _address(body["address"])
    if (len(body["chunks"]) != 1 or len(rows) != 1 or body["end"] != hex(entry + 6)
            or body.get("pseudocode") is not None or not isinstance(body.get("type"), str)
            or body["type"].count("...") != 1):
        raise ValueError("Import-forwarder proof requires a complete six-byte variadic thunk without a cfunc")
    instruction = rows[entry]
    code = bytes.fromhex(instruction["bytes"])
    if len(code) != 6 or code[:2] != b"\xff\x25":
        raise ValueError("Import-forwarder instruction is not exact unprefixed FF /4 ModRM 00:100:101")
    iat_va = struct.unpack("<I", code[2:])[0]
    binding = pe.import_slot(iat_va - pe.base)
    if body.get("name") != binding["name"] or binding["library"].lower() != "ntoskrnl.exe":
        raise ValueError("Import-forwarder original entry or system import identity differs")
    if instruction.get("xrefs_from") != [_xref(entry, iat_va, 3, False)]:
        raise ValueError("Import-forwarder original IAT operand lacks its exact reciprocal data reference")
    return ImportJump32(entry, iat_va), binding


def _record(a, path, row):
    identity = evidence.identity(a, path)
    if identity["sha256"] != row.get("sha256") or "bytes" in row and row["bytes"] != identity["bytes"]:
        raise ValueError("Import-forwarder captured function identity changed")
    body = json.loads(path.read_bytes())
    if body.get("address") != row.get("address") or body.get("rva") != row.get("rva"):
        raise ValueError("Import-forwarder capture index differs from its original function")
    return identity, body


def _sdk_evidence(a, raw_review, abi_review, observation, body, source, capture):
    matrix_id = raw_review.get("source_matrix")
    matrix = json.loads(evidence.checked(a, matrix_id))
    if (matrix.get("review_kind") != "x86_variadic_import_tail_sdk_capability_matrix"
            or matrix.get("target_execution") is not False or abi_review.get("target_execution") is not False
            or abi_review.get("review_kind") != "variadic_import_tail_exact_explicit_stack_abi"):
        raise ValueError("Import-forwarder SDK evidence changed its static boundary")
    observer = matrix.get("observer_source", {})
    code = observer.get("source", "").encode("utf-8")
    if len(code) != observer.get("bytes") or hashlib.sha256(code).hexdigest() != observer.get("sha256"):
        raise ValueError("Import-forwarder SDK matrix omitted its complete observer source")
    evidence.checked(a, abi_review.get("probe_source"))
    matches = [row for row in matrix.get("observations", []) if row.get("rva") == body["rva"]
               and row.get("source_input") == source and row.get("base_capture") == capture]
    if len(matches) != 1:
        raise ValueError("Import-forwarder SDK matrix target is absent or ambiguous")
    row, target = matches[0], body["address"]
    observed = row.get("observation", {})
    if row.get("target_execution") is not False or row.get("analyzer_exit_code") != 0 or observed.get("target_execution") is not False:
        raise ValueError("Import-forwarder SDK matrix is not a successful static observation")
    before, after = observed.get("before", {}), observed.get("after", {})
    if (any(before.get(key) != after.get(key) for key in ("address", "end", "flags", "type", "chunks", "assembly", "frame", "stack_points"))
            or before.get("address") != target or before.get("end") != body["end"] or before.get("type") != body["type"]
            or before.get("flags") != body.get("flags")
            or [(c.get("start"), c.get("end"), c.get("raw_hex")) for c in before.get("chunks", [])]
               != [(c["start"], c["end"], c["raw_hex"]) for c in body["chunks"]]
            or before.get("entry_type", {}).get("serialized") != after.get("entry_type", {}).get("serialized")
            or before.get("iat_type", {}).get("serialized") != after.get("iat_type", {}).get("serialized")):
        raise ValueError("Import-forwarder SDK observations changed original machine, frame, SP or serialized types")
    frame, points, instructions = before.get("frame"), before.get("stack_points"), before.get("assembly")
    if (not isinstance(frame, dict) or set(frame) != {"frame", "frsize", "frregs", "argsize", "fpd"}
            or any(type(value) is not int for value in frame.values())
            or not isinstance(points, list) or len(points) != 1 or points[0].get("address") != target
            or type(points[0].get("spd")) is not int or type(points[0].get("delta")) is not int
            or not isinstance(instructions, list) or len(instructions) != 1
            or instructions[0].get("address") != target or instructions[0].get("bytes") != body["assembly"][0]["bytes"]):
        raise ValueError("Import-forwarder SDK evidence omitted complete original instruction, frame or SP observations")
    for key in ("entry_type", "iat_type"):
        left, right = before.get(key, {}), after.get(key, {})
        if (left.get("observed") is not True or right.get("observed") is not True
                or not left.get("serialized") or left.get("declaration") != right.get("declaration")
                or not isinstance(left.get("declaration"), str) or left["declaration"].count("...") != 1):
            raise ValueError("Import-forwarder SDK evidence omitted complete original variadic type bytes")
    experiments = observed.get("experiments", [])
    modes = [row.get("mode") for row in experiments[:2]]
    if modes != ["original_function_no_cache", "original_function_outline_no_cache"] or len(experiments) != 12:
        raise ValueError("Import-forwarder SDK matrix omits original full-function or maturity attempts")
    expected = [(snippet, maturity) for snippet in (False, True) for maturity in
                ("MMAT_GENERATED", "MMAT_PREOPTIMIZED", "MMAT_LOCOPT", "MMAT_CALLS", "MMAT_GLBOPT3")]
    if [(row.get("snippet"), row.get("maturity")) for row in experiments[2:]] != expected:
        raise ValueError("Import-forwarder SDK matrix changed its complete maturity set")
    for index, experiment in enumerate(experiments):
        success = index >= 2 and experiment.get("maturity") in {"MMAT_GENERATED", "MMAT_PREOPTIMIZED", "MMAT_LOCOPT"}
        failure = experiment.get("failure", {})
        if (experiment.get("success") is not success or failure.get("code") != (0 if success else -12)
                or not success and failure.get("address") != target
                or success and not isinstance(experiment.get("warnings"), list)):
            raise ValueError("Import-forwarder SDK matrix changed a preserved success, failure or warning observation")
    actual_warnings = [warning for experiment in experiments if experiment["success"] for warning in experiment["warnings"]]
    trials = [row for row in abi_review.get("trials", []) if row.get("rva") == body["rva"]
              and row.get("source_database", {}).get("path", "").startswith(".work/native/" + source["sha256"] + "/")]
    if [row.get("mode") for row in trials] != ["explicit_thunk_abi", "explicit_thunk_and_iat_abi"]:
        raise ValueError("Import-forwarder SDK ABI trials omitted a separately retained original failure")
    for trial in trials:
        arguments = trial.get("argument_locations", [])
        if (trial.get("errors") != [] or trial.get("parsed") is not True or trial.get("ellipsis_preserved") is not True
                or trial.get("returned_cfunc") is not False or trial.get("temporary_removed") is not True
                or trial.get("source_database_unchanged") is not True or trial.get("source_database_after") != trial.get("source_database")
                or trial.get("declaration", "").count("...") != 1 or trial.get("readback", "").count("...") != 1
                or trial.get("failure", {}).get("code") != -12 or trial["failure"].get("ea") != target
                or not arguments or trial.get("stack_argument_size") != len(arguments) * 4
                or [arg.get("stack_offset") for arg in arguments] != list(range(0, len(arguments) * 4, 4))
                or any(arg.get("kind") != 1 for arg in arguments)):
            raise ValueError("Import-forwarder SDK ABI evidence removed ellipsis, original failure or cleanup")
    return {"matrix": matrix_id, "actual_successful_stage_warnings": actual_warnings,
            "actual_warning_id_3_emitted": any(warning.get("id") == 3 for warning in actual_warnings),
            "sdk_documented_warning_is_not_an_observed_warning": True,
            "failure_internal_cause_established": False, "explicit_abi_trials": trials,
            "original_matrix_metadata_equality_flag": observed.get("original_metadata_unchanged"),
            "original_capability_result": observation.get("capability_result")}


def prove_forwarder(a, result, rva, machine_evidence, abi_evidence):
    """Recompute one complete machine-only forwarder and every original direct caller."""
    result = a.resolve(a.rel(result), ".analysis/results/native")
    capture_id = evidence.identity(a, result / "capture.json")
    capture = json.loads(evidence.checked(a, capture_id))
    source_path = a.resolve(capture.get("input", ""), ".analysis/inputs")
    source_id = evidence.identity(a, source_path)
    raw = evidence.checked(a, source_id, ".analysis/inputs")
    if (result.name != source_id["sha256"] or capture.get("input_sha256") != source_id["sha256"]
            or capture.get("input_bytes") != len(raw) or capture.get("capture_complete") is not True
            or capture.get("functions_complete") is not True or capture.get("target_execution") is not False):
        raise ValueError("Import-forwarder proof requires an exact complete original static capture")
    pe = Pe32(raw)
    if capture.get("image_base") != hex(pe.base):
        raise ValueError("Import-forwarder capture image base differs from its original PE32 image")
    rva = hex(_address(rva))
    records = capture.get("functions", [])
    if (not records or len({row.get("rva") for row in records}) != len(records)
            or any(capture.get(key) != len(records) for key in ("functions_total", "functions_captured", "settled_functions_total"))):
        raise ValueError("Import-forwarder capture omitted its complete unique function inventory")
    originals, scan, target_identity, target = {}, [], None, None
    for record in records:
        if record.get("assembly_captured") is not True:
            raise ValueError("Import-forwarder caller scan omitted an original complete assembly")
        path = a.resolve(a.rel(result / record["path"]), a.rel(result / "functions"))
        identity, body = _record(a, path, record)
        if body["rva"] == rva:
            target_identity, target = identity, body
        scan.append(identity)
        originals[body["rva"]] = (identity, body)
    if target is None:
        raise ValueError("Import-forwarder proof target is absent from its exact original capture")
    model, binding = decode_import_jump(pe, target)
    entry, iat_va = model.entry_virtual_address, model.iat_virtual_address
    instruction = target["assembly"][0]
    code = bytes.fromhex(instruction["bytes"])
    callers, owners = [], {}
    for identity, body in originals.values():
        for call in body.get("assembly", []):
            machine = bytes.fromhex(call["bytes"])
            if len(machine) != 5 or machine[0] != 0xe8:
                continue
            at = _address(call["address"])
            if (at + 5 + struct.unpack("<i", machine[1:])[0]) & 0xffffffff != entry:
                continue
            _complete_machine(pe, body)
            edge = _xref(at, entry, 17, True)
            if call.get("xrefs_from", []).count(edge) != 1:
                raise ValueError("Import-forwarder direct CALL lacks its reciprocal original target reference")
            if any(x.get("code") is True and x != edge and x != _xref(at, at + 5, 21, True)
                   for x in call["xrefs_from"]):
                raise ValueError("Import-forwarder direct CALL has an unrelated original control-flow reference")
            callers.append({"owner_function": identity, "call_rva": hex(at - pe.base), "raw_hex": machine.hex(),
                            "return_rva": hex(at + 5 - pe.base), "reciprocal_xref": edge})
            owners[identity["path"]] = {"owner_function": identity, "rva": body["rva"],
                "complete_original_instruction_count": len(body["assembly"]),
                "complete_original_chunk_count": len(body["chunks"]),
                "complete_original_machine_bytes": sum(len(bytes.fromhex(c["raw_hex"])) for c in body["chunks"])}
    callers.sort(key=lambda row: int(row["call_rva"], 16))
    edges = [row["reciprocal_xref"] for row in callers]
    if (not callers or len({row["call_rva"] for row in callers}) != len(callers)
            or target.get("xrefs_to_entry") != edges or instruction.get("xrefs_to") != edges):
        raise ValueError("Import-forwarder complete caller inventory and reciprocal entry references differ")
    machine_review = json.loads(evidence.checked(a, machine_evidence))
    abi_review = json.loads(evidence.checked(a, abi_evidence))
    if (machine_review.get("review_kind") != "x86_variadic_import_tail_exact_machine_and_sdk_boundary"
            or machine_review.get("target_execution") is not False or machine_review.get("raw_artifacts_modified") is not False):
        raise ValueError("Import-forwarder retained machine evidence changed its exact static scope")
    matches = [row for row in machine_review.get("observations", []) if row.get("rva") == rva
               and row.get("source_input") == source_id and row.get("base_capture") == capture_id]
    if len(matches) != 1:
        raise ValueError("Import-forwarder retained observation is absent or ambiguous")
    observation = matches[0]
    if (observation.get("function") != target_identity or observation.get("original_complete_bytes") != code.hex()
            or observation.get("incoming_calls") != callers or observation.get("incoming_call_count") != len(callers)
            or observation.get("caller_owner_count") != len(owners)
            or any(binding.get(key) != value for key, value in observation.get("original_import", {}).items())
            or set(observation.get("original_import", {})) != {"library", "name", "iat_rva", "lookup_rva",
                "lookup_raw_hex", "iat_raw_hex", "descriptor_raw_hex"}
            or any(observation.get(key) is not False for key in ("provider_semantics_complete", "runtime_iat_target_observed",
                "provider_executed", "rust_implementation_acceptance", "pseudocode_recovered", "target_execution"))):
        raise ValueError("Import-forwarder retained evidence omitted exact IAT, callers, original bytes or limitations")
    sdk = _sdk_evidence(a, machine_review, abi_review, observation, target, source_id, capture_id)
    original_failures = [row for row in capture.get("failures", []) if row.get("stage") == rva[2:] + ":hexrays"]
    if not original_failures or any("code -12" not in row.get("error", "") for row in original_failures):
        raise ValueError("Import-forwarder proof omitted the preserved original Hex-Rays failure")
    return {"coverage_kind": "exact_original_x86_variadic_import_jump", "source_input": source_id,
        "base_capture": capture_id, "rva": rva, "function": target_identity, "original_complete_bytes": code.hex(),
        "original_type_with_ellipsis": target["type"], "machine": {"format": "pe32", "machine": 0x14c,
            "default_operand_bits": 32, "opcode": 0xff, "modrm": 0x25, "group_operation": 4,
            "mod": 0, "rm": 5, "prefix_bytes": [], "instruction_bytes": 6, **asdict(model)},
        "original_import": binding, "incoming_calls": callers,
        "caller_owners": sorted(owners.values(), key=lambda row: _address(row["rva"])),
        "caller_inventory_scan": {"complete_function_count": len(scan), "all_function_identities_verified": True,
            "ordered_function_identities_sha256": hashlib.sha256(json.dumps(scan, sort_keys=True).encode()).hexdigest(),
            "retained_complete_index": capture_id},
        "normal_transfer": {"precondition": "instruction and IAT fetch succeed; loaded near target passes architectural checks",
            "eip_after": "memory32[iat_virtual_address]", "esp_after": "esp_before",
            "preserved_registers": ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi", "eflags"],
            "memory_writes": [], "return_address_pushes": 0, "stack_adjustment_bytes": 0,
            "existing_call_return_address": "unchanged", "all_fixed_and_variadic_argument_bytes": "unchanged",
            "fault_boundary": "instruction fetch, IAT read or invalid near target may fault; provider behavior is unobserved"},
        "original_hexrays_failures": original_failures,
        "sdk_evidence": sdk, "machine_evidence": machine_evidence, "abi_evidence": abi_evidence,
        "machine_transfer_complete": True, "pseudocode_recovered": False, "provider_semantics_complete": False,
        "runtime_iat_target_observed": False, "provider_executed": False, "rust_implementation_acceptance": False,
        "target_execution": False, "raw_artifacts_modified": False}


def build_review(a, cases, machine_evidence, abi_evidence):
    """Build a concrete review; publication and queue policy belong to the unified owner."""
    proofs = [prove_forwarder(a, result, rva, machine_evidence, abi_evidence) for result, rva in cases]
    if not proofs or len({(p["source_input"]["sha256"], p["rva"]) for p in proofs}) != len(proofs):
        raise ValueError("Import-forwarder review requires unique complete original cases")
    return {"schema_version": 1, "review_kind": "exact_x86_variadic_import_forwarders_machine_semantics",
            "validator_source": source_identity(a.ROOT), "proofs": proofs, "target_execution": False,
            "raw_artifacts_modified": False, "queue_status_modified": False, "pseudocode_synthesized": False,
            "provider_semantics_complete": False, "runtime_acceptance": False}


def verify_review(a, review_identity):
    review = json.loads(evidence.checked(a, review_identity))
    if (set(review) != {"schema_version", "review_kind", "validator_source", "proofs", "target_execution",
            "raw_artifacts_modified", "queue_status_modified", "pseudocode_synthesized", "provider_semantics_complete", "runtime_acceptance"}
            or review.get("schema_version") != 1
            or review.get("review_kind") != "exact_x86_variadic_import_forwarders_machine_semantics"
            or review.get("validator_source") != source_identity(a.ROOT)
            or any(review.get(key) is not False for key in ("target_execution", "raw_artifacts_modified",
                "queue_status_modified", "pseudocode_synthesized", "provider_semantics_complete", "runtime_acceptance"))
            or not isinstance(review.get("proofs"), list) or not review["proofs"]):
        raise ValueError("Import-forwarder review changed its exact machine-only completion boundary")
    complete = {}
    for proof in review["proofs"]:
        capture = a.resolve(proof["base_capture"]["path"], ".analysis/results/native")
        expected = prove_forwarder(a, capture.parent, proof["rva"], proof["machine_evidence"], proof["abi_evidence"])
        if proof != expected or proof["rva"] in complete.setdefault(proof["source_input"]["sha256"], {}):
            raise ValueError("Import-forwarder review omitted or changed its independently recomputed original proof")
        if (not proof["original_hexrays_failures"] or any("code -12" not in row.get("error", "")
                for row in proof["original_hexrays_failures"])):
            raise ValueError("Import-forwarder proof omitted the preserved original Hex-Rays failure")
        complete[proof["source_input"]["sha256"]][proof["rva"]] = {"review": review_identity, **proof}
    return {"complete_forwarders": complete, "pseudocode_synthesized": False,
            "provider_semantics_complete": False, "runtime_acceptance": False, "target_execution": False}


def _binding(a, result, binding, capture, capture_identity):
    required = {"schema_version", "review", "source_input", "base_capture", "function_rvas",
                "target_execution", "pseudocode_synthesized", "provider_semantics_complete", "runtime_acceptance"}
    if (not isinstance(binding, dict) or set(binding) != required or binding.get("schema_version") != 1
            or binding.get("base_capture") != capture_identity
            or any(binding.get(key) is not False for key in ("target_execution", "pseudocode_synthesized",
                "provider_semantics_complete", "runtime_acceptance"))
            or not isinstance(binding.get("function_rvas"), list) or not binding["function_rvas"]
            or binding["function_rvas"] != sorted(set(binding["function_rvas"]), key=_address)):
        raise ValueError("Import-forwarder binding changed its exact original scope or completion boundary")
    if (json.loads(evidence.checked(a, capture_identity)) != capture
            or result.name != capture.get("input_sha256")):
        raise ValueError("Import-forwarder resolution did not retain the exact current base capture")
    source = evidence.identity(a, a.resolve(capture.get("input", ""), ".analysis/inputs"))
    if (binding.get("source_input") != source or source["sha256"] != capture.get("input_sha256")
            or source["bytes"] != capture.get("input_bytes")):
        raise ValueError("Import-forwarder binding belongs to a different original input")
    verified = verify_review(a, binding["review"])
    proofs = verified["complete_forwarders"].get(source["sha256"], {})
    if (sorted(proofs, key=_address) != binding["function_rvas"]
            or any(proof["source_input"] != source or proof["base_capture"] != capture_identity
                   or proof["rva"] != rva for rva, proof in proofs.items())):
        raise ValueError("Import-forwarder binding omitted or changed an exact reviewed input, base or target")
    return proofs


def publish_binding(a, args):
    """Publish only an explicitly SHA-selected, fully reverified machine-proof binding."""
    if not isinstance(args.review_sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", args.review_sha256):
        raise ValueError("Import-forwarder publication requires an exact explicitly selected review SHA-256")
    review_path = a.resolve(args.review, ".analysis/results")
    review_identity = evidence.identity(a, review_path)
    if review_identity["sha256"] != args.review_sha256:
        raise ValueError("Import-forwarder publication review bytes differ from the explicitly selected SHA-256")
    source = evidence.identity(a, a.resolve(args.input, ".analysis/inputs"))
    result = a.resolve(a.rel(a.ROOT / ".analysis/results/native" / source["sha256"]), ".analysis/results/native")
    destination = result / "import-forwarder-review.json"
    with a.index_lock(a.ROOT / ".analysis/reverse-pending.lock"):
        capture_identity = evidence.identity(a, result / "capture.json")
        capture = json.loads(evidence.checked(a, capture_identity))
        proofs = verify_review(a, review_identity)["complete_forwarders"].get(source["sha256"], {})
        if not proofs:
            raise ValueError("Import-forwarder publication input has no exact proof in the selected review")
        binding = {"schema_version": 1, "review": review_identity, "source_input": source,
            "base_capture": capture_identity, "function_rvas": sorted(proofs, key=_address),
            "target_execution": False, "pseudocode_synthesized": False,
            "provider_semantics_complete": False, "runtime_acceptance": False}
        _binding(a, result, binding, capture, capture_identity)
        if destination.is_symlink():
            raise ValueError("Import-forwarder binding cannot replace or follow a link")
        serialized = (json.dumps(binding, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
        temporary = None
        try:
            if destination.exists():
                if json.loads(destination.read_bytes()) != binding:
                    raise ValueError("A different import-forwarder binding already exists; its decisions remain intact")
            else:
                with tempfile.NamedTemporaryFile(prefix=".import-forwarder-", suffix=".tmp", dir=result, delete=False) as stream:
                    temporary = Path(stream.name)
                    stream.write(serialized)
                    stream.flush()
                    os.fsync(stream.fileno())
                try:
                    os.link(temporary, destination)
                except FileExistsError:
                    if destination.is_symlink() or destination.read_bytes() != serialized:
                        raise ValueError("Concurrent import-forwarder publication changed the selected binding")
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
        observed = json.loads(destination.read_bytes())
        if observed != binding:
            raise ValueError("Published import-forwarder binding differs from its complete selected proof")
        _binding(a, result, observed, capture, capture_identity)
        return {"binding": evidence.identity(a, destination), "review": review_identity,
            "source_input": source, "base_capture": capture_identity, "complete_forwarder_rvas": binding["function_rvas"],
            "target_execution": False, "pseudocode_synthesized": False,
            "provider_semantics_complete": False, "runtime_acceptance": False, "queue_status_modified": False}


def apply_resolution(a, result, resolution, capture):
    """Apply only an explicit binding; preserve every original failure and unrelated validation error."""
    result = a.resolve(a.rel(result), ".analysis/results/native")
    destination = result / "import-forwarder-review.json"
    if not destination.exists() and not destination.is_symlink():
        return {}
    try:
        if destination.is_symlink():
            raise ValueError("Import-forwarder resolution cannot follow a binding link")
        capture_identity = evidence.identity(a, result / "capture.json")
        if resolution.get("base_capture") != capture_identity or resolution.get("target_execution") is not False:
            raise ValueError("Import-forwarder resolution changed its exact base capture or static boundary")
        binding_identity = evidence.identity(a, destination)
        binding = json.loads(evidence.checked(a, binding_identity))
        verified = _binding(a, result, binding, capture, capture_identity)
        if (not isinstance(resolution.get("resolved_failures"), list)
                or not isinstance(resolution.get("remaining_failures"), list)
                or not isinstance(resolution.get("verification_failures"), list)
                or not isinstance(resolution.get("complete_forwarders", {}), dict)):
            raise ValueError("Import-forwarder resolution lacks separate complete-failure and verification inventories")
        complete = copy.deepcopy(resolution.get("complete_forwarders", {}))
        selected = {rva: {**proof, "binding": binding_identity} for rva, proof in verified.items()}
        for rva, proof in selected.items():
            if rva in complete and complete[rva] != proof:
                raise ValueError("Import-forwarder resolution conflicts with a previously selected machine proof")
            complete[rva] = proof
        resolved, remaining = list(resolution["resolved_failures"]), []
        for failure in resolution["remaining_failures"]:
            match = re.fullmatch(r"([0-9a-fA-F]+):hexrays", failure.get("stage", ""))
            rva = hex(int(match[1], 16)) if match else None
            if rva not in selected:
                remaining.append(failure)
                continue
            proof = selected[rva]
            if failure not in proof["original_hexrays_failures"]:
                raise ValueError("Import-forwarder resolution changed an original Hex-Rays failure")
            row = {"original_failure": copy.deepcopy(failure), "function_rva": rva,
                   "resolved_by": "exact_original_import_jump", "complete_forwarder": proof}
            if row in resolved:
                raise ValueError("Import-forwarder resolution repeats an already resolved original failure")
            resolved.append(row)
        resolution.update(complete_forwarders=complete, resolved_failures=resolved, remaining_failures=remaining)
        return selected
    except (OSError, ValueError, KeyError, TypeError) as error:
        resolution.setdefault("verification_failures", []).append({"import_forwarder_review": a.rel(destination),
                                                                    "error": str(error)})
        return {}


def previous_function_coverage(a, result, complete_forwarder, previous_function):
    """Prove one old exact machine artifact; this does not retire its whole analyzer attempt."""
    result = a.resolve(a.rel(result), ".analysis/results/native")
    current = prove_forwarder(a, result, complete_forwarder["rva"],
        complete_forwarder["machine_evidence"], complete_forwarder["abi_evidence"])
    if (any(complete_forwarder.get(key) != value for key, value in current.items())
            or set(complete_forwarder) - set(current) - {"review", "binding"}):
        raise ValueError("Previous import-forwarder coverage changed its complete independently verified machine proof")
    path = a.resolve(previous_function["path"], a.rel(result))
    previous = json.loads(evidence.checked(a, previous_function, a.rel(result)))
    if evidence.identity(a, path) != previous_function:
        raise ValueError("Previous import-forwarder function identity changed")
    raw = evidence.checked(a, current["source_input"], ".analysis/inputs")
    model, binding = decode_import_jump(Pe32(raw), previous)
    original = json.loads(evidence.checked(a, current["function"], a.rel(result)))
    if (previous.get("rva") != current["rva"] or previous.get("type") != current["original_type_with_ellipsis"]
            or previous.get("flags") != original.get("flags") or binding != current["original_import"]
            or asdict(model) != {key: current["machine"][key] for key in ("entry_virtual_address", "iat_virtual_address")}
            or any(previous.get(key) != original.get(key) for key in ("address", "end", "chunks", "xrefs_to_entry"))
            or [(ins.get("address"), ins.get("rva"), ins.get("bytes"), ins.get("xrefs_to"), ins.get("xrefs_from"))
                for ins in previous["assembly"]]
               != [(ins.get("address"), ins.get("rva"), ins.get("bytes"), ins.get("xrefs_to"), ins.get("xrefs_from"))
                   for ins in original["assembly"]]
            or any(key in previous and previous[key] != previous["type"] for key in
                   ("type_before_decompile", "type_after_decompile"))
            or "assembly_after_decompile" in previous and previous["assembly_after_decompile"] != previous["assembly"]):
        raise ValueError("Previous import-forwarder bytes, complete type, IAT or reciprocal caller scope differ")
    return {"coverage_kind": "exact_previous_x86_variadic_import_jump_machine_coverage",
        "source_input": current["source_input"], "base_capture": current["base_capture"],
        "rva": current["rva"], "previous_function": previous_function,
        "complete_forwarder_function": current["function"], "original_type_with_ellipsis": previous["type"],
        "original_complete_bytes": current["original_complete_bytes"], "original_import": binding,
        "incoming_calls": current["incoming_calls"], "machine_transfer_complete": True,
        "original_hexrays_failure_reinterpreted_as_success": False, "whole_attempt_retired": False,
        "pseudocode_recovered": False, "provider_semantics_complete": False, "runtime_acceptance": False,
        "target_execution": False}
