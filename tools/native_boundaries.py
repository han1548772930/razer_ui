"""Byte-bound native boundary recovery and audited static reanalysis."""
from __future__ import annotations

import copy
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import re
import struct

import native_adjudication as evidence


_SOURCE = Path(__file__).read_bytes()


def source_identity(root):
    source = Path(__file__).resolve()
    if source.read_bytes() != _SOURCE:
        raise ValueError("Boundary validator changed after import")
    return {"path": source.relative_to(root).as_posix(), "bytes": len(_SOURCE),
            "sha256": hashlib.sha256(_SOURCE).hexdigest()}


def retain_source(a, record):
    if (not isinstance(record, dict) or set(record) != {"path", "bytes", "sha256"}
            or record.get("path") != "tools/native_boundaries.py"
            or type(record.get("bytes")) is not int or record["bytes"] <= 0
            or not isinstance(record.get("sha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", record["sha256"])):
        raise ValueError("Boundary validator source identity is incomplete")
    target = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (record["sha256"] + ".py")
    with a.index_lock(a.ROOT / ".analysis/native-boundary-source.lock"):
        path = target if target.is_file() else a.resolve(record["path"], "tools")
        raw = path.read_bytes()
        if len(raw) != record["bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
            raise ValueError("Boundary validator source bytes changed")
        if not target.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open("xb") as stream:
                stream.write(raw)
    return evidence.identity(a, target)


def _review(a, specification):
    name, separator, pointer = specification.partition("#")
    if separator and pointer and not pointer.startswith("/"):
        raise ValueError("Boundary review requires JSON or JSON#pointer")
    path = a.resolve(name, ".analysis/results")
    value = json.loads(path.read_bytes())
    if pointer:
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
            raise ValueError("Boundary review pointer does not resolve") from error
    return value, {**evidence.identity(a, path), "pointer": pointer}


def _pe(raw):
    from host import inspect_pe
    pe = inspect_pe(raw)
    if pe["architecture"] != "x64" or pe["format"] != "pe32+":
        raise ValueError("Chained runtime helper recovery requires an x64 PE")
    offset = struct.unpack_from("<I", raw, 0x3c)[0]
    optional = offset + 24
    if struct.unpack_from("<I", raw, optional + 108)[0] <= 3:
        raise ValueError("Chained runtime helper has no exception directory")
    rva, size = struct.unpack_from("<II", raw, optional + 112 + 3 * 8)
    section_table = optional + struct.unpack_from("<H", raw, offset + 20)[0]

    def read(start, length, executable=False):
        matches = [(index, section) for index, section in enumerate(pe["sections"])
                   if section["rva"] < start + length and start < section["rva"] + section["bytes"]]
        if (type(start) is not int or type(length) is not int or length <= 0 or len(matches) != 1
                or start < matches[0][1]["rva"]
                or start + length > matches[0][1]["rva"] + matches[0][1]["bytes"]
                or executable and not struct.unpack_from("<I", raw, section_table + matches[0][0] * 40 + 36)[0] & 0x20000000):
            raise ValueError("Chained runtime range is not uniquely backed by original PE bytes")
        section = matches[0][1]
        at = section["file_offset"] + start - section["rva"]
        return raw[at:at + length]

    if not rva or not size or size % 12 or size > 12_000_000:
        raise ValueError("Chained runtime exception directory is empty, partial or unbounded")
    table = read(rva, size)
    entries, previous_end = {}, 0
    for index in range(size // 12):
        begin, end, unwind = struct.unpack_from("<III", table, index * 12)
        if not begin or end <= begin or begin < previous_end or not unwind or unwind % 4:
            raise ValueError("Chained runtime table has an empty, unordered, overlapping or unaligned record")
        read(begin, end - begin, True)
        read(unwind, 4)
        entries[begin] = {"table_entry_rva": hex(rva + index * 12),
            "raw_hex": table[index * 12:(index + 1) * 12].hex(), "begin_rva": hex(begin),
            "end_rva": hex(end), "unwind_rva": hex(unwind)}
        previous_end = end
    return pe, read, {"rva": hex(rva), "bytes": size,
                      "raw_sha256": hashlib.sha256(table).hexdigest()}, entries


def _chain(read, entries, start):
    chain, seen = [], set()
    while True:
        if start in seen or len(seen) >= 64 or start not in entries:
            raise ValueError("Runtime helper has a cyclic, unbounded or missing chained record")
        seen.add(start)
        entry = entries[start]
        unwind = int(entry["unwind_rva"], 16)
        header = read(unwind, 4)
        version, flags, slots = header[0] & 7, header[0] >> 3, header[2]
        if version != 1 or flags not in (0, 4):
            raise ValueError("Runtime helper has an unsupported unwind version or handler")
        length = 4 + 2 * ((slots + 1) & ~1) + (12 if flags == 4 else 0)
        raw = read(unwind, length)
        item = {"runtime_entry": copy.deepcopy(entry), "unwind_rva": hex(unwind),
                "version": version, "flags": flags, "prolog_bytes": header[1],
                "code_slots": slots, "frame_register": header[3] & 15,
                "frame_offset_units": header[3] >> 4, "raw_hex": raw.hex()}
        chain.append(item)
        if flags == 0:
            return chain
        begin, end, next_unwind = struct.unpack_from("<III", raw, length - 12)
        following = entries.get(begin)
        if (begin != int(entry["end_rva"], 16) or following is None
                or following["end_rva"] != hex(end) or following["unwind_rva"] != hex(next_unwind)):
            raise ValueError("Runtime helper chain does not prove an exact contiguous following region")
        item["chain_triple"] = [hex(begin), hex(end), hex(next_unwind)]
        start = begin


def _linear(assembly, start, code, base):
    if not isinstance(assembly, list) or not 1 <= len(assembly) <= 262144:
        raise ValueError("Runtime helper lacks complete bounded instruction evidence")
    current = start
    for instruction in assembly:
        if not isinstance(instruction, dict):
            raise ValueError("Runtime helper instruction is invalid")
        raw = bytes.fromhex(instruction.get("bytes", ""))
        if (not 1 <= len(raw) <= 15 or instruction.get("rva") != hex(current)
                or instruction.get("address") != hex(base + current)
                or not isinstance(instruction.get("text"), str) or not instruction["text"].strip()
                or raw != code[current - start:current - start + len(raw)]):
            raise ValueError("Runtime helper instructions omit, overlap or differ from original bytes")
        current += len(raw)
    if current != start + len(code) or assembly[-1]["bytes"] != "c3":
        raise ValueError("Runtime helper instructions do not cover its full exact RET boundary")


def select(a, raw, capture, selected_rvas, specifications):
    if (not isinstance(specifications, (tuple, list)) or not 1 <= len(specifications) <= 64
            or len(set(specifications)) != len(specifications) or capture.get("capture_complete") is not True
            or hashlib.sha256(raw).hexdigest() != capture.get("input_sha256")
            or len(raw) != capture.get("input_bytes")):
        raise ValueError("Boundary recovery requires unique bounded reviews and complete original bytes")
    pe, read, table, entries = _pe(raw)
    base = int(pe["image_base"], 16)
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    base_identity = evidence.identity(a, result / "capture.json")
    source = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    if source["bytes"] != len(raw) or source["sha256"] != hashlib.sha256(raw).hexdigest():
        raise ValueError("Boundary recovery source bytes changed")
    retained = {}
    for record in capture["functions"]:
        path = a.resolve(a.rel(result / record["path"]), a.rel(result))
        identity = evidence.identity(a, path)
        if identity["sha256"] != record["sha256"] or ("bytes" in record and identity["bytes"] != record["bytes"]):
            raise ValueError("Boundary recovery original function bytes changed")
        body = json.loads(path.read_bytes())
        if body.get("rva") != record["rva"] or body.get("address") != record["address"] or record["rva"] in retained:
            raise ValueError("Boundary recovery original function identity differs or repeats")
        retained[record["rva"]] = (body, identity, record)
    values, used_owners, used_helpers = [], set(), []
    for specification in specifications:
        review, review_source = _review(a, specification)
        if (not isinstance(review, dict) or review.get("schema_version") != 1
                or review.get("review_kind") != "native_x64_chained_runtime_helper_complete_bytes"
                or review.get("source_input") != source or review.get("base_capture") != base_identity
                or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
                or not isinstance(review.get("owner_function"), dict) or not isinstance(review.get("helper"), dict)):
            raise ValueError("Boundary review lacks its exact current input, base capture or read-only scope")
        owners = [(rva, body, identity, record) for rva, (body, identity, record) in retained.items()
                  if identity == review["owner_function"]]
        if len(owners) != 1:
            raise ValueError("Boundary review does not name one complete original owner")
        owner_rva, owner, owner_identity, record = owners[0]
        if int(owner_rva, 16) not in selected_rvas or owner_rva in used_owners or record.get("assembly_captured") is not True:
            raise ValueError("Boundary review owner is unselected, repeated or incomplete")
        evidence.original_machine(raw, owner)
        helper = review["helper"]
        try:
            start, end = int(helper["start_rva"], 16), int(helper["end_rva"], 16)
        except (KeyError, TypeError, ValueError) as error:
            raise ValueError("Boundary helper range is incomplete") from error
        if helper["start_rva"] != hex(start) or helper["end_rva"] != hex(end):
            raise ValueError("Boundary helper range is not canonical")
        chain = _chain(read, entries, start)
        if len(chain) < 2 or end != int(chain[-1]["runtime_entry"]["end_rva"], 16) or end - start > 1024 * 1024:
            raise ValueError("Boundary helper is not a complete bounded chained runtime range")
        code = read(start, end - start, True)
        if (helper.get("bytes") != len(code) or helper.get("raw_hex") != code.hex()
                or helper.get("sha256") != hashlib.sha256(code).hexdigest()
                or helper.get("complete_linear_instruction_bytes_verified") is not True
                or helper.get("ret_rva") != hex(end - 1)):
            raise ValueError("Boundary helper full bytes or terminal RET differ from the original PE")
        _linear(helper.get("assembly"), start, code, base)
        head = entries.get(int(owner_rva, 16))
        chunks = owner.get("chunks")
        first_end = int(entries[start]["end_rva"], 16)
        if (head is None or not isinstance(chunks, list) or len(chunks) != 2
                or (chunks[0]["start"], chunks[0]["end"]) != (owner["address"], hex(base + int(head["end_rva"], 16)))
                or chunks[1]["start"] != hex(base + start) or chunks[1]["end"] != hex(base + first_end)
                or owner["end"] != chunks[0]["end"] or start < int(head["end_rva"], 16)):
            raise ValueError("Boundary helper is not the exact separate original owner tail fragment")
        if any(max(start, lo) < min(end, hi) for lo, hi in used_helpers):
            raise ValueError("Boundary helper ranges overlap")
        shared = []
        for rva, (other, identity, other_record) in retained.items():
            overlaps = [chunk for chunk in other.get("chunks", [])
                        if int(chunk["start"], 16) < base + end and base + start < int(chunk["end"], 16)]
            if not overlaps:
                continue
            other_head = entries.get(int(rva, 16))
            head_regions = [entry for begin, entry in entries.items()
                            if int(rva, 16) <= begin < int(other["end"], 16) - base]
            covered = int(rva, 16)
            for region in head_regions:
                if int(region["begin_rva"], 16) != covered:
                    raise ValueError("Shared owner main runtime regions contain a gap")
                covered = int(region["end_rva"], 16)
            if (int(rva, 16) not in selected_rvas or rva in used_owners
                    or other_record.get("assembly_captured") is not True or other_head is None
                    or len(other["chunks"]) != 2 or overlaps != [chunks[1]]
                    or other["chunks"][0]["start"] != other["address"]
                    or other["chunks"][0]["end"] != hex(base + covered)
                    or other["end"] != other["chunks"][0]["end"]
                    or int(other_head["end_rva"], 16) > start):
                raise ValueError("Boundary helper has an unselected, changed or unrelated shared original owner")
            evidence.original_machine(raw, other)
            shared.append({"owner_rva": rva, "owner_function": identity,
                           "original_chunks": copy.deepcopy(other["chunks"]),
                           "owner_chunks": copy.deepcopy(other["chunks"][:1]),
                           "head_runtime_entry": copy.deepcopy(other_head),
                           "head_runtime_entries": copy.deepcopy(head_regions)})
        callsite = review.get("callsite")
        calls = [instruction for instruction in owner["assembly"] if instruction == callsite]
        callcode = bytes.fromhex(callsite.get("bytes", "")) if isinstance(callsite, dict) else b""
        if (len(calls) != 1 or len(callcode) != 5 or callcode[0] != 0xe8
                or int(callsite["rva"], 16) + 5 + int.from_bytes(callcode[1:], "little", signed=True) != start
                or not int(owner_rva, 16) <= int(callsite["rva"], 16) < int(head["end_rva"], 16)):
            raise ValueError("Boundary helper lacks its exact original owner E8 call target")
        prefix = [instruction for instruction in helper["assembly"] if int(instruction["rva"], 16) < first_end]
        original_prefix = [instruction for instruction in owner["assembly"] if int(instruction["rva"], 16) >= start]
        if [(row["address"], row["bytes"]) for row in prefix] != [(row["address"], row["bytes"]) for row in original_prefix]:
            raise ValueError("Boundary helper changes an original tail instruction boundary")
        expected_table = {**table, "entries": [copy.deepcopy(head)] + [copy.deepcopy(item["runtime_entry"]) for item in chain]}
        expected_chain = {key: value for key, value in chain[0].items() if key != "runtime_entry"}
        expected_chain["terminal_unwind_raw_hex"] = chain[-1]["raw_hex"]
        missing = read(first_end, end - first_end, True)
        if (review.get("exception_directory") != expected_table or helper.get("unwind_chain") != expected_chain
                or review.get("missing_retained_range") != {"start_rva": hex(first_end), "end_rva": hex(end),
                    "bytes": len(missing), "raw_hex": missing.hex(), "sha256": hashlib.sha256(missing).hexdigest(),
                    "base_function_owners": []}):
            raise ValueError("Boundary review exception chain or unowned continuation differs from the original PE")
        values.append({"review_source": review_source, "source_input": source, "base_capture": base_identity,
            "owner_rva": owner_rva, "owner_function": owner_identity, "original_chunks": copy.deepcopy(chunks),
            "owner_chunks": copy.deepcopy(chunks[:1]), "shared_owners": shared, "callsite": copy.deepcopy(callsite),
            "helper": {"rva": hex(start), "address": hex(base + start), "end": hex(base + end),
                "bytes": len(code), "raw_hex": code.hex(), "sha256": hashlib.sha256(code).hexdigest(),
                "assembly": copy.deepcopy(helper["assembly"])},
            "exception_directory": {**table, "entries": [entry for item in shared for entry in item["head_runtime_entries"]]
                                    + [copy.deepcopy(item["runtime_entry"]) for item in chain]},
            "unwind_chain": chain, "target_execution": False})
        used_owners.update(item["owner_rva"] for item in shared)
        used_helpers.append((start, end))
    return values


def owner_body(a, row, owner_rva=None):
    matches = [item for item in row["shared_owners"] if item["owner_rva"] == (owner_rva or row["owner_rva"])]
    if len(matches) != 1:
        raise ValueError("Boundary baseline requires one exact shared original owner")
    owner = matches[0]
    body = json.loads(evidence.checked(a, owner["owner_function"], ".analysis/results/native"))
    body["chunks"] = copy.deepcopy(owner["owner_chunks"])
    body["assembly"] = [item for item in body["assembly"] if any(
        int(chunk["start"], 16) <= int(item["address"], 16) < int(chunk["end"], 16) for chunk in body["chunks"])]
    return body


def helper_body(row):
    helper = row["helper"]
    return {"address": helper["address"], "rva": helper["rva"], "end": helper["end"],
            "chunks": [{"start": helper["address"], "end": helper["end"],
                        "raw_hex": helper["raw_hex"], "sha256": helper["sha256"]}],
            "assembly": copy.deepcopy(helper["assembly"])}


def _entry_piece_bodies(original, row):
    pieces = row.get("pieces")
    if (original.get("rva") != row.get("owner_rva") or original.get("chunks") != row.get("original_chunks")
            or len(original["chunks"]) != 1 or not isinstance(pieces, list) or len(pieces) != 3
            or [piece.get("role") for piece in pieces] != ["ci", "stack", "context"]
            or any(not isinstance(piece.get("chunks"), list) or len(piece["chunks"]) != 1 for piece in pieces)):
        raise ValueError("Entry partition pieces differ from their complete original owner")
    chunks = [piece["chunks"][0] for piece in pieces]
    if (chunks[0]["start"] != original["address"] or chunks[-1]["end"] != original["end"]
            or any(left["end"] != right["start"] for left, right in zip(chunks, chunks[1:]))
            or "".join(chunk["raw_hex"] for chunk in chunks) != original["chunks"][0]["raw_hex"]
            or any(hashlib.sha256(bytes.fromhex(chunk["raw_hex"])).hexdigest() != chunk["sha256"] for chunk in chunks)):
        raise ValueError("Entry partition pieces omit or change original complete bytes")
    values = []
    base = int(original["address"], 16) - int(original["rva"], 16)
    for piece, chunk in zip(pieces, chunks):
        if (piece["address"] != chunk["start"] or piece["end"] != chunk["end"]
                or hex(base + int(piece["rva"], 16)) != piece["address"]):
            raise ValueError("Entry partition piece coordinates differ from its complete original bytes")
        body = copy.deepcopy(original)
        first, end = int(piece["address"], 16), int(piece["end"], 16)
        body.update(address=piece["address"], rva=piece["rva"], end=piece["end"],
            chunks=copy.deepcopy(piece["chunks"]),
            assembly=[ins for ins in original["assembly"] if first <= int(ins["address"], 16) < end])
        body["xrefs_to_entry"] = copy.deepcopy(body["assembly"][0].get("xrefs_to", []))
        if piece["role"] == "ci":
            body["xrefs_to_entry"] = copy.deepcopy(original.get("xrefs_to_entry", []))
        values.append(body)
    return values


def entry_partition_bodies(a, row):
    """Keep the complete original byte union while exposing its three real entries."""
    original = json.loads(evidence.checked(a, row["owner_function"], ".analysis/results/native"))
    return _entry_piece_bodies(original, row)


def _entry_pattern(body, points):
    first, stack, context, end = points
    code = bytes.fromhex(body["chunks"][0]["raw_hex"])
    relative = lambda at: at + 5 + struct.unpack_from("<i", code, at - first + 1)[0]
    if (stack - first == 20 and context - stack == 9
            and code[:6] == bytes.fromhex("83ec0cdd1424") and code[6] == 0xe8 and code[11] == 0xe8
            and code[16:20] == bytes.fromhex("83c40cc3")
            and code[20:24] == bytes.fromhex("8d542404") and code[24] == 0xe8
            and code[29:34] == bytes.fromhex("529bd93c24") and relative(first + 11) == context):
        return {"kind": "x87_ci_stack_context", "context_call_rva": hex(first + 11),
                "check_helper_rva": hex(relative(first + 6)), "load_helper_rva": hex(relative(stack + 4))}
    if (stack - first == 24 and context - stack == 6
            and code[:17] == bytes.fromhex("558bec83ec0883e4f0dd1c24f30f7e0424")
            and code[17] == 0xe8 and code[22:24] == bytes.fromhex("c9c3")
            and code[24:30] == bytes.fromhex("660f12442404")
            and code[30:35] == bytes.fromhex("660fc5c003") and relative(first + 17) == context):
        return {"kind": "sse_ci_stack_context", "context_call_rva": hex(first + 17)}
    raise ValueError("Entry partition does not prove the exact CI, stack and context machine entries")


def select_entry_partitions(a, raw, capture, selected_rvas, specifications, outline_rvas):
    """Prove the x86 CI/stack/context shape without supplying ABI or stack values."""
    from host import inspect_pe
    pe = inspect_pe(raw)
    if (pe["architecture"] != "x86" or pe["format"] != "pe32"
            or capture.get("capture_complete") is not True
            or capture.get("input_sha256") != hashlib.sha256(raw).hexdigest()
            or capture.get("input_bytes") != len(raw)
            or not isinstance(specifications, (list, tuple)) or not 1 <= len(specifications) <= 64
            or len(set(specifications)) != len(specifications)):
        raise ValueError("Entry partitions require unique bounded reviews and a complete exact x86 PE")
    base = int(pe["image_base"], 16)
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    source = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    base_capture = evidence.identity(a, result / "capture.json")
    if source["bytes"] != len(raw) or source["sha256"] != hashlib.sha256(raw).hexdigest():
        raise ValueError("Entry partition original input changed")
    records = {row["rva"]: row for row in capture["functions"]}
    if len(records) != len(capture["functions"]):
        raise ValueError("Entry partition base capture repeats a boundary")
    retained = {}

    def load(identity):
        if identity["path"] not in retained:
            body = json.loads(evidence.checked(a, identity, ".analysis/results/native"))
            record = records.get(body.get("rva"))
            if (record is None or identity["path"] != a.rel(result / record["path"])
                    or identity["sha256"] != record["sha256"]
                    or "bytes" in record and identity["bytes"] != record["bytes"]
                    or body.get("address") != record["address"] or record.get("assembly_captured") is not True):
                raise ValueError("Entry partition original function identity or complete assembly differs")
            evidence.original_machine(raw, body)
            retained[identity["path"]] = (body, identity)
        if retained[identity["path"]][1] != identity:
            raise ValueError("Entry partition repeats a changed original artifact")
        return retained[identity["path"]][0]

    values, used, ranges = [], set(), []
    expected_outline = [hex(int(value, 0) if isinstance(value, str) else value) for value in outline_rvas]
    for specification in specifications:
        review, review_source = _review(a, specification)
        if (not isinstance(review, dict) or set(review) != {"schema_version", "review_kind", "source_input",
                "base_capture", "owner_function", "boundaries", "external_incoming", "outline_helper",
                "reason", "target_execution", "raw_artifacts_modified"}
                or review.get("schema_version") != 1
                or review.get("review_kind") != "x86_ci_stack_context_complete_entry_partition"
                or review.get("source_input") != source or review.get("base_capture") != base_capture
                or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
                or not isinstance(review.get("reason"), str) or not review["reason"].strip()
                or len(review["reason"]) > 65536):
            raise ValueError("Entry partition review lacks its exact original identities and read-only scope")
        owner = load(review["owner_function"])
        rva = owner["rva"]
        points = review["boundaries"]
        if (not isinstance(points, list) or len(points) != 4
                or any(not isinstance(value, str) or not re.fullmatch("0x[0-9a-f]+", value) for value in points)):
            raise ValueError("Entry partition requires four canonical machine boundaries")
        points = [int(value, 16) for value in points]
        first, stack, context, end = points
        if (any(hex(value) != text for value, text in zip(points, review["boundaries"]))
                or not first < stack < context < end or end - first > 1024 * 1024
                or rva != hex(first) or first not in selected_rvas or rva in used
                or len(owner["chunks"]) != 1 or owner["address"] != hex(base + first)
                or owner["end"] != hex(base + end) or owner["chunks"][0]["start"] != owner["address"]
                or owner["chunks"][0]["end"] != owner["end"]
                or any(max(first, lo) < min(end, hi) for lo, hi in ranges)):
            raise ValueError("Entry partition owner is unselected, repeated, overlapping or not one complete range")
        pattern = _entry_pattern(owner, points)
        helper = load(review["outline_helper"])
        if expected_outline != [helper["rva"]] or int(helper["rva"], 16) in selected_rvas:
            raise ValueError("Entry partition must bind its one exact separately reviewed outline helper")
        helper_first, helper_end = int(helper["address"], 16), int(helper["end"], 16)
        if (len(helper["chunks"]) != 1 or helper["chunks"][0]["start"] != helper["address"]
                or helper["chunks"][0]["end"] != helper["end"]
                or helper_first < base + end and base + first < helper_end):
            raise ValueError("Entry partition outline helper overlaps its owner or lacks complete bytes")
        assembly, cursor = owner["assembly"], base + first
        for ins in assembly:
            at, code = int(ins["address"], 16), bytes.fromhex(ins["bytes"])
            if (at != cursor or ins.get("rva") != hex(at - base) or not 1 <= len(code) <= 15
                    or any(at < base + point < at + len(code) for point in points)):
                raise ValueError("Entry partition instructions contain gaps, overlap or split a machine boundary")
            cursor += len(code)
        padding = bytes.fromhex(owner["chunks"][0]["raw_hex"])[cursor - base - first:]
        if (cursor > base + end or len(padding) > 15 or any(value != 0x90 for value in padding)
                or not {hex(base + point) for point in points[:-1]}.issubset({ins["address"] for ins in assembly})):
            raise ValueError("Entry partition omits machine instructions beyond explicit terminal NOP padding")
        calls = [ins for ins in assembly if len(bytes.fromhex(ins["bytes"])) == 5
            and bytes.fromhex(ins["bytes"])[0] == 0xe8
            and int(ins["rva"], 16) + 5 + struct.unpack("<i", bytes.fromhex(ins["bytes"])[1:])[0]
                == int(helper["rva"], 16)]
        if pattern["kind"] == "x87_ci_stack_context" and not calls:
            raise ValueError("Entry partition lacks an original direct call to its outline helper")
        incoming = []
        for ins in assembly:
            for edge in ins.get("xrefs_to", []):
                source_ea, target_ea = int(edge["from"], 16), int(edge["to"], 16)
                if edge.get("code") is not True or base + first <= source_ea < base + end:
                    continue
                if edge["to"] != ins["address"] or target_ea - base not in points[:-1]:
                    raise ValueError("Entry partition has an external incoming edge inside a proposed machine entry")
                incoming.append({"target_rva": hex(target_ea - base), "source_rva": hex(source_ea - base),
                                 "xref": copy.deepcopy(edge)})
        supplied = review["external_incoming"]
        if (not isinstance(supplied, list) or len(supplied) != len(incoming)
                or [{key: row.get(key) for key in ("target_rva", "source_rva", "xref")} for row in supplied] != incoming
                or any(set(row) != {"target_rva", "source_rva", "xref", "source_function", "source_instruction"}
                       for row in supplied)):
            raise ValueError("Entry partition review omits or changes an original external incoming edge")
        for edge in supplied:
            caller = load(edge["source_function"])
            ins = edge["source_instruction"]
            if (ins not in caller["assembly"] or ins["rva"] != edge["source_rva"]
                    or edge["xref"] not in ins.get("xrefs_from", [])):
                raise ValueError("Entry partition external entry lacks its exact reciprocal original caller instruction")
        pieces, original_code = [], bytes.fromhex(owner["chunks"][0]["raw_hex"])
        for role, lo, hi in zip(("ci", "stack", "context"), points, points[1:]):
            code = original_code[lo - first:hi - first]
            chunks = [{"start": hex(base + lo), "end": hex(base + hi), "raw_hex": code.hex(),
                       "sha256": hashlib.sha256(code).hexdigest()}]
            pieces.append({"role": role, "rva": hex(lo), "address": hex(base + lo),
                           "end": hex(base + hi), "chunks": chunks})
        values.append({"review_source": review_source, "source_input": source, "base_capture": base_capture,
            "owner_rva": rva, "owner_function": review["owner_function"],
            "original_chunks": copy.deepcopy(owner["chunks"]), "original_flags": owner["flags"],
            "pattern": pattern, "pieces": pieces, "external_incoming": copy.deepcopy(supplied),
            "outline_helper": review["outline_helper"], "outline_helper_rva": helper["rva"],
            "outline_helper_calls": copy.deepcopy(calls),
            "unserialized_padding": {"start": hex(cursor), "end": owner["end"], "raw_hex": padding.hex()},
            "reason": review["reason"], "target_execution": False, "semantics_complete": False})
        used.add(rva)
        ranges.append((first, end))
    if not any(row["outline_helper_calls"] for row in values):
        raise ValueError("Entry partition selection does not use its separately reviewed outline helper")
    return values


def entry_partition_prepasses(a, raw, capture, selected_rvas, partitions):
    """Retain the real first derivation for every new entry and unchanged output."""
    covered, values = set(), []
    for row in partitions:
        for body in entry_partition_bodies(a, row):
            values.append({"rva": body["rva"], "source_function": row["owner_function"],
                "original_chunks": copy.deepcopy(row["original_chunks"]), "entry_partition": copy.deepcopy(row),
                "reason": row["reason"]})
        covered.add(int(row["owner_rva"], 16))
    rest = [rva for rva in selected_rvas if rva not in covered]
    if rest:
        values.extend(select_prepasses(a, raw, capture, selected_rvas, rest, partitions[0]["reason"]))
    return values


def apply_entry_partition(request, base, sdk, checkpoint):
    """Recreate three exact private boundaries; the source database stays untouched."""
    original = json.loads(_read_identity(sdk.root, request["owner_function"]))
    before = _snapshot(original, sdk)
    if before["flags"] != request["original_flags"]:
        raise ValueError("Entry partition original private function flags differ")
    row = {"request": copy.deepcopy(request), "before": before, "after": [], "deleted_original": False,
        "created_entries": [], "auto_wait_completed": False, "manual_stack_points_modified": False,
        "manual_frames_modified": False, "instructions_patched": False,
        "base_database_modified": False, "target_execution": False}
    checkpoint(row)
    if not sdk.funcs.del_func(int(original["address"], 16)):
        raise ValueError("SDK rejected deleting the exact private entry-partition owner")
    row["deleted_original"] = True
    checkpoint(row)
    for piece in request["pieces"]:
        if piece["address"] != hex(base + int(piece["rva"], 16)):
            raise ValueError("Entry partition image base differs")
        if not sdk.funcs.add_func(int(piece["address"], 16), int(piece["end"], 16)):
            raise ValueError("SDK rejected an exact private machine entry boundary")
        row["created_entries"].append(piece["rva"])
        checkpoint(row)
    if not sdk.auto.auto_wait():
        raise ValueError("Entry partition automatic analysis was canceled")
    row["auto_wait_completed"] = True
    for body in _entry_piece_bodies(original, request):
        row["after"].append(_snapshot(body, sdk))
    checkpoint(row)
    return row


def verify_entry_partitions(a, requested, applied):
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("Entry partition omitted an exact original owner")
    for request, row in zip(requested, applied):
        if (not isinstance(row, dict) or row.get("request") != request
                or row.get("deleted_original") is not True or row.get("auto_wait_completed") is not True
                or row.get("created_entries") != [piece["rva"] for piece in request["pieces"]]
                or any(row.get(key) is not False for key in ("manual_stack_points_modified", "manual_frames_modified",
                                                            "instructions_patched", "base_database_modified", "target_execution"))
                or not isinstance(row.get("after"), list) or len(row["after"]) != 3):
            raise ValueError("Entry partition omitted SDK completion or changed its exact original scope")
        original = json.loads(evidence.checked(a, request["owner_function"], ".analysis/results/native"))
        _verify_snapshot(original, row.get("before"))
        if row["before"]["flags"] != request["original_flags"]:
            raise ValueError("Entry partition private original flags changed")
        for body, snapshot in zip(entry_partition_bodies(a, request), row["after"]):
            _verify_snapshot(body, snapshot)


def verify_entry_helpers(a, raw, requested, records, output):
    helpers = {body["rva"]: body for row in requested for body in entry_partition_bodies(a, row)[1:]}
    if (not isinstance(records, list) or len(records) != len(helpers)
            or {row.get("rva") for row in records} != set(helpers)):
        raise ValueError("Entry partition omitted a complete typed stack or context helper")
    verified = []
    for row in records:
        name = "entry-partition-helpers/" + row["rva"][2:] + ".json"
        if row.get("path") != name or row.get("assembly_captured") is not True or row.get("pseudocode_captured") is not True:
            raise ValueError("Entry partition helper lacks its exact complete typed capture")
        absolute = a.resolve(a.rel(output / name), ".analysis/results/native")
        identity = evidence.identity(a, absolute)
        if row.get("sha256") != identity["sha256"] or "bytes" in row and row["bytes"] != identity["bytes"]:
            raise ValueError("Entry partition typed helper capture bytes changed")
        record = {**row, **identity}
        body = json.loads(evidence.checked(a, identity, ".analysis/results/native"))
        baseline = helpers[row["rva"]]
        if (any(body.get(key) != baseline.get(key) for key in ("address", "rva", "end", "chunks", "xrefs_to_entry"))
                or [(ins["address"], ins["bytes"]) for ins in body.get("assembly", [])]
                    != [(ins["address"], ins["bytes"]) for ins in baseline["assembly"]]
                or not isinstance(body.get("pseudocode"), dict) or not body["pseudocode"].get("lines")):
            raise ValueError("Entry partition helper omits original bytes or complete pseudocode")
        evidence.original_machine(raw, body)
        evidence._typed_function(body)
        verified.append(record)
    return verified


def select_optimizer_properties(a, raw, capture, selected_rvas, specifications, outlines, shared_tails=(), type_recoveries=()):
    """Bind public optimizer barriers to exact original owners and outlined helpers."""
    from host import inspect_pe
    pe = inspect_pe(raw)
    if (pe.get("architecture") != "x64" or pe.get("format") != "pe32+"
            or capture.get("capture_complete") is not True
            or capture.get("input_sha256") != hashlib.sha256(raw).hexdigest()
            or capture.get("input_bytes") != len(raw) or len(specifications) != 1):
        raise ValueError("Optimizer properties require one complete exact x64 original review")
    review, source = _review(a, specifications[0])
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    original = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    cap = evidence.identity(a, result / "capture.json")
    if (set(review) != {"schema_version", "review_kind", "source_input", "base_capture", "owners", "helpers",
                        "reason", "target_execution", "raw_artifacts_modified"}
            or review.get("schema_version") != 1 or review.get("review_kind") != "exact_fict_ext_MULX_optimizer_properties"
            or review.get("source_input") != original or review.get("base_capture") != cap
            or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
            or not isinstance(review.get("reason"), str) or not review["reason"].strip()
            or len(review["reason"]) > 65536):
        raise ValueError("Optimizer property review changed its exact original scope")
    records = {row["rva"]: row for row in capture["functions"]}
    if len(records) != len(capture["functions"]):
        raise ValueError("Optimizer property base capture repeats a function")
    owners, helpers, used = [], [], set()
    for name, target in (("owners", owners), ("helpers", helpers)):
        supplied = review.get(name)
        if not isinstance(supplied, list) or not supplied:
            raise ValueError("Optimizer property review omitted complete owners or helpers")
        for identity in supplied:
            body = json.loads(evidence.checked(a, identity, ".analysis/results/native"))
            record = records.get(body.get("rva"))
            if (record is None or identity["path"] != a.rel(result / record["path"])
                    or identity["sha256"] != record["sha256"] or record.get("assembly_captured") is not True
                    or body["rva"] in used or body["address"] != record["address"]):
                raise ValueError("Optimizer property original function is changed, duplicated or unretained")
            evidence.original_machine(raw, body)
            used.add(body["rva"])
            target.append({"rva": body["rva"], "address": body["address"], "end": body["end"], "source_function": identity,
                           "chunks": body["chunks"], "instructions": copy.deepcopy(body["assembly"])})
    helper_plans = [*outlines, *(owner for tail in shared_tails if tail["inline_owners"] for owner in tail["owners"])]
    if ([row["rva"] for row in owners] != [hex(rva) for rva in selected_rvas]
            or [row["rva"] for row in helpers] != [row["rva"] for row in helper_plans]
            or any(helper["source_function"] != plan["source_function"]
                   or helper["chunks"] != plan.get("original_chunks", plan["expected_chunks"])
                   for helper, plan in zip(helpers, helper_plans))
            or any(owner["rva"] not in used for tail in shared_tails for owner in tail["owners"])):
        raise ValueError("Optimizer property owners or complete helpers differ from explicit selected boundaries")
    machine = {ins["address"]: ins for helper in helpers for ins in helper["instructions"]}
    ext = [ea for ea, ins in machine.items() if re.fullmatch(r"(?:66|f3)(?:4[0-9a-f])?0f38f6[0-9a-f]+", ins["bytes"])]
    mulx = [ea for ea, ins in machine.items() if re.fullmatch(r"(?:67)*c4[02468ace]2[0-9a-f][3b]f6[0-9a-f]+", ins["bytes"])]
    if not ext or not mulx:
        raise ValueError("Optimizer property helpers lack complete original ADCX/ADOX and MULX instructions")
    request = {"review_source": source, "source_input": original, "base_capture": cap,
             "owners": owners, "helpers": helpers, "ext_machine_eas": ext, "mulx_machine_eas": mulx,
             "shared_tails": list(shared_tails), "type_recoveries": list(type_recoveries),
             "property": "IPROP_DONT_PROP", "public_api": "minsn_t.clr_propagatable",
             "optimizer_properties_changed": True, "machine_bytes_modified": False,
             "opcode_ea_operands_modified": False, "manual_stack_or_frame_modified": False,
             "target_execution": False, "reason": review["reason"], "semantics_complete": False}
    for row in [*owners, *helpers]:
        evidence.original_machine(raw, optimizer_context_body(request, row))
    return [request]


def optimizer_context_body(request, row):
    """Reuse independently byte-validated shared-tail plans without replacing their original evidence."""
    body = {"address": row["address"], "rva": row["rva"], "end": row["end"],
            "chunks": row["chunks"], "assembly": row["instructions"]}
    originals = {item["rva"]: item for item in [*request["owners"], *request["helpers"]]}
    for tail in request.get("shared_tails", []):
        owner = next((item for item in tail["owners"] if item["rva"] == row["rva"]), None)
        if owner is None:
            continue
        if (owner["source_function"] != row["source_function"] or owner["original_chunks"] != row["chunks"]
                or owner["original_end"] != row["end"]):
            raise ValueError("Optimizer shared-tail context changed its original complete owner")
        primary = originals[tail["primary_owner_rva"]]
        start, end = int(tail["expected_tail"]["start"], 16), int(tail["expected_tail"]["end"], 16)
        instructions = row["instructions"]
        if row["rva"] != primary["rva"]:
            present = {ins["address"] for ins in instructions}
            instructions = sorted([*instructions, *(ins for ins in primary["instructions"]
                if start <= int(ins["address"], 16) < end and ins["address"] not in present)],
                key=lambda ins: int(ins["address"], 16))
        body.update(end=owner["expected_end"], chunks=owner["expected_chunks"], assembly=instructions)
    return body


def optimizer_instruction_rows(snapshot):
    """Enumerate every original top-level and nested minsn without dropping operands."""
    found = []
    def visit_operand(operand, block, top, path):
        if "instruction" in operand:
            visit(operand["instruction"], block, top, path)
        if "address_operand" in operand:
            visit_operand(operand["address_operand"], block, top, path + ["address_operand"])
    def visit(ins, block, top, path):
        found.append((block, top, path, ins))
        for slot in ("l", "r", "d"):
            visit_operand(ins["operands"][slot], block, top, path + [slot])
    for block in snapshot["blocks"]:
        for top, ins in enumerate(block["instructions"]):
            visit(ins, block["serial"], top, [])
    return found


def optimizer_expected_changes(request, snapshot):
    """Recompute exact ext and def-use-backed MULX closures from the full SDK snapshot."""
    owners = {row["rva"] for row in request["owners"]}
    if (snapshot.get("entry_rva") not in owners or snapshot.get("maturity") != 1
            or snapshot.get("complete") is not True or not isinstance(snapshot.get("blocks"), list)
            or [block.get("serial") for block in snapshot["blocks"]] != list(range(len(snapshot["blocks"])) )):
        raise ValueError("Optimizer property snapshot omitted its complete PREOPT target")
    ext_eas, mulx_eas = set(request["ext_machine_eas"]), set(request["mulx_machine_eas"])
    rows = optimizer_instruction_rows(snapshot)
    selected, witnesses, scopes = [], [], set()
    for block, top, path, ins in rows:
        if ins["opcode"] != 62 or ins["ea"] == ins["mapped_ea"] or ins["mapped_ea"] == "0xffffffffffffffff":
            continue
        if path or ins["mapped_ea"] not in ext_eas:
            raise ValueError("Optimizer ext is nested or lacks its exact original ADCX/ADOX instruction")
        selected.append((block, top, path))
        prior = snapshot["blocks"][block]["instructions"][:top]
        for slot in ("l", "r"):
            operand = ins["operands"][slot]
            if operand["kind"] != 1:
                continue
            register, width = operand["register"], operand["size"]
            if type(register) is not int or type(width) is not int or register < 0 or not 1 <= width <= 16:
                raise ValueError("Optimizer source register lacks its exact SDK location")
            wanted = set(range(register, register + width))
            witness = {"use": {"block": block, "top": top, "path": path}, "operand": slot,
                       "register": register, "width": width, "examined": [], "status": "no_same_block_definition"}
            for index in range(len(prior) - 1, -1, -1):
                definition = prior[index]
                may, must = definition["may_def_registers"], definition["must_def_registers"]
                if (not isinstance(may, list) or not isinstance(must, list)
                        or may != sorted(set(may)) or must != sorted(set(must)) or not set(must).issubset(may)):
                    raise ValueError("Optimizer def-use evidence omits exact SDK may/must register sets")
                witness["examined"].append(index)
                if not wanted.intersection(may):
                    continue
                witness["definition_top"] = index
                destination = definition["operands"]["d"]
                if not wanted.issubset(must):
                    witness["status"] = "nearest_definition_does_not_fully_cover"
                elif (destination["kind"] != 1 or destination["register"] != register or destination["size"] != width):
                    witness["status"] = "nearest_definition_is_not_exact_direct_register"
                elif definition["mapped_ea"] not in mulx_eas:
                    witness["status"] = "nearest_definition_not_original_MULX"
                else:
                    witness["status"] = "exact_nearest_full_direct_MULX_definition"
                    scopes.add((block, definition["ea"]))
                break
            witnesses.append(witness)
    if not selected or not scopes:
        raise ValueError("Optimizer properties lack real fict ext and complete proven MULX inputs")
    changes = list(selected)
    for block, top, path, ins in rows:
        if (block, ins["ea"]) in scopes:
            if ins["mapped_ea"] not in mulx_eas or ins["opcode"] not in (2, 4, 9, 12, 14):
                raise ValueError("Optimizer MULX generated closure changed its exact original mapping or opcode")
            changes.append((block, top, path))
    if len({(b, t, tuple(p)) for b, t, p in changes}) != len(changes):
        raise ValueError("Optimizer property instruction scope is duplicated")
    return {"changes": [{"block": b, "top": t, "path": p} for b, t, p in changes],
            "ext_count": len(selected), "mulx_scopes": [{"block": b, "ea": ea} for b, ea in sorted(scopes)],
            "def_use_witnesses": witnesses}


def verify_optimizer_property_pass(request, row):
    """Allow only the documented single optimizer bit on the independently recomputed closure."""
    before, after = row.get("before"), row.get("after")
    if not isinstance(before, dict) or not isinstance(after, dict):
        raise ValueError("Optimizer property proof omitted a complete before/after SDK snapshot")
    expected = optimizer_expected_changes(request, before)
    if row.get("scope") != expected or row.get("iprop_dont_prop") != 131072:
        raise ValueError("Optimizer property proof changed or omitted its exact def-use closure")
    old_rows, new_rows = optimizer_instruction_rows(before), optimizer_instruction_rows(after)
    if len(old_rows) != len(new_rows):
        raise ValueError("Optimizer property change omitted an original microinstruction")
    changed = {(item["block"], item["top"], tuple(item["path"])) for item in expected["changes"]}
    normalized = copy.deepcopy(after)
    normalized_rows = optimizer_instruction_rows(normalized)
    observed_changed = 0
    for (b, t, p, old), (nb, nt, np, new), (_, _, _, reset) in zip(old_rows, new_rows, normalized_rows):
        if (b, t, p) != (nb, nt, np):
            raise ValueError("Optimizer property change moved an original microinstruction")
        wanted = old["iprops"] | (131072 if (b, t, tuple(p)) in changed else 0)
        if type(old["iprops"]) is not int or new.get("iprops") != wanted:
            raise ValueError("Optimizer property change altered an unreviewed property or missed an exact instruction")
        observed_changed += new["iprops"] != old["iprops"]
        reset["iprops"] = old["iprops"]
    if normalized != before or not observed_changed:
        raise ValueError("Optimizer property change altered original EA, opcode, operands, CFG, stack or frame")
    return expected


def optimizer_original_states(request, sdk):
    states = []
    for row in [*request["owners"], *request["helpers"]]:
        original = json.loads(_read_identity(sdk.root, row["source_function"]))
        if any(original.get(key) != row[key] for key in ("address", "rva", "end", "chunks")) or original["assembly"] != row["instructions"]:
            raise ValueError("Optimizer state omitted complete immutable original function evidence")
        states.append(_snapshot(optimizer_context_body(request, row), sdk))
    return states


def optimizer_snapshot(mba, base, sdk, definitions=False):
    """Freeze the complete public microinstruction tree and actual SDK def sets."""
    h = sdk.hexrays
    def operand(value):
        kind = int(value.t)
        result = {"kind": kind, "size": int(value.size), "oprops": int(value.oprops),
                  "valnum": int(value.valnum), "text": str(value.dstr())}
        if kind == h.mop_r:
            result["register"] = int(value.r)
        elif kind == h.mop_n:
            result.update(unsigned_value=int(value.unsigned_value()), signed_value=int(value.signed_value()))
        elif kind == h.mop_d:
            result["instruction"] = instruction(value.d)
        elif kind == h.mop_a:
            result["address_operand"] = operand(value.a)
        elif kind == h.mop_S:
            result.update(stack_offset=int(value.s.off), source_mba_entry=hex(int(value.s.mba.entry_ea)))
        elif kind == h.mop_v:
            result["global_ea"] = hex(int(value.g))
        elif kind == h.mop_b:
            result["target_block"] = int(value.b)
        return result
    def instruction(ins):
        return {"ea": hex(int(ins.ea)), "mapped_ea": hex(int(mba.map_fict_ea(int(ins.ea)))),
                "opcode": int(ins.opcode), "iprops": int(ins.iprops), "text": str(ins.dstr()),
                "operands": {slot: operand(getattr(ins, slot)) for slot in ("l", "r", "d")}}
    def registers(value):
        last = int(value.reg.last())
        if not value.reg.count():
            return []
        if not 0 <= last <= 65536:
            raise ValueError("SDK definition register namespace is outside its bounded exact representation")
        found = [bit for bit in range(last + 1) if value.reg.has(bit)]
        if len(found) != int(value.reg.count()):
            raise ValueError("SDK definition register serialization omitted a location")
        return found
    row = {"entry_rva": hex(int(mba.entry_ea) - base), "maturity": int(mba.maturity), "complete": True,
           "frame": {name: int(getattr(mba, name)) for name in ("cc", "npurged", "last_prolog_ea", "first_epilog_ea",
               "frsize", "frregs", "fpd", "fullsize", "stacksize", "inargoff", "minstkref", "minstkref_ea",
               "minargref", "tmpstk_size", "retsize", "shadow_args", "spd_adjust", "pfn_flags")},
           "mba_flags": int(mba.get_mba_flags()), "mba_flags2": int(mba.get_mba_flags2()), "blocks": []}
    for serial in range(int(mba.qty)):
        block = mba.get_mblock(serial)
        current = {"serial": int(block.serial), "flags": int(block.flags), "type": int(block.type),
                   "start": hex(int(block.start)), "end": hex(int(block.end)), "maxbsp": int(block.maxbsp),
                   "minbstkref": int(block.minbstkref), "minbargref": int(block.minbargref),
                   "predecessors": [int(block.pred(index)) for index in range(int(block.npred()))],
                   "successors": [int(block.succ(index)) for index in range(int(block.nsucc()))], "instructions": []}
        ins = block.head
        while ins is not None:
            frozen = instruction(ins)
            if definitions:
                frozen["may_def_registers"] = registers(block.build_def_list(ins, h.MAY_ACCESS))
                frozen["must_def_registers"] = registers(block.build_def_list(ins, h.MUST_ACCESS))
            current["instructions"].append(frozen)
            ins = ins.next
        row["blocks"].append(current)
    return row


def apply_optimizer_properties(request, mba, base, sdk, checkpoint):
    before = optimizer_snapshot(mba, base, sdk, definitions=True)
    scope = optimizer_expected_changes(request, before)
    row = {"before": before, "scope": scope, "iprop_dont_prop": int(sdk.hexrays.IPROP_DONT_PROP)}
    checkpoint(row)
    wanted = {(item["block"], item["top"], tuple(item["path"])) for item in scope["changes"]}
    visited = set()
    def change_operand(value, block, top, path):
        if int(value.t) == sdk.hexrays.mop_d:
            change(value.d, block, top, path)
        elif int(value.t) == sdk.hexrays.mop_a:
            change_operand(value.a, block, top, path + ["address_operand"])
    def change(ins, block, top, path):
        key = (block, top, tuple(path))
        if key in wanted:
            ins.clr_propagatable()
            visited.add(key)
        for slot in ("l", "r", "d"):
            change_operand(getattr(ins, slot), block, top, path + [slot])
    for serial in range(int(mba.qty)):
        block, top = mba.get_mblock(serial), 0
        ins = block.head
        while ins is not None:
            change(ins, serial, top, [])
            ins, top = ins.next, top + 1
    if visited != wanted:
        raise ValueError("SDK optimizer property application omitted a complete generated instruction")
    row["after"] = optimizer_snapshot(mba, base, sdk, definitions=True)
    checkpoint(row)
    verify_optimizer_property_pass(request, row)
    return row


def verify_optimizer_stages(audit, rva):
    """Require every real SDK stage for this exact target, including final local allocation."""
    stages = audit.get("stages")
    expected = [("generated", 0), ("locopt", 2), ("calls_done", 3), ("glbopt", 6), ("final_mba", 8)]
    if not isinstance(stages, list) or len(stages) != len(expected):
        raise ValueError("Optimizer property capture omitted a complete SDK stage")
    for row, (phase, maturity) in zip(stages, expected):
        snapshot = row.get("snapshot", {})
        blocks = snapshot.get("blocks")
        if (row.get("phase") != phase or snapshot.get("maturity") != maturity
                or snapshot.get("entry_rva") != rva or snapshot.get("complete") is not True
                or not isinstance(blocks, list) or not blocks
                or [block.get("serial") for block in blocks] != list(range(len(blocks)))):
            raise ValueError("Optimizer property capture changed its exact SDK stage or target")
        optimizer_instruction_rows(snapshot)
    passes = audit.get("passes", [])
    if (len(passes) != 1 or passes[0].get("before", {}).get("entry_rva") != rva
            or passes[0].get("after", {}).get("entry_rva") != rva):
        raise ValueError("Optimizer property pass belongs to a different selected owner")


def verify_optimizer_properties(requests, body):
    if not requests:
        if "optimizer_property_audit" in body:
            raise ValueError("Native capture contains unreviewed optimizer property changes")
        return
    if len(requests) != 1 or body.get("rva") not in {owner["rva"] for owner in requests[0]["owners"]}:
        raise ValueError("Optimizer properties selected a different exact target")
    audit = body.get("optimizer_property_audit", {})
    if (audit.get("request") != requests[0] or audit.get("complete") is not True
            or audit.get("hook_registered") is not True or audit.get("hook_unregistered") is not True
            or audit.get("errors") != [] or audit.get("optimizer_properties_changed") is not True
            or any(audit.get(key) is not False for key in ("target_execution", "base_database_modified",
                "machine_bytes_modified", "opcode_ea_operands_modified", "manual_stack_or_frame_modified"))
            or not isinstance(audit.get("passes"), list) or len(audit["passes"]) != 1
            or not isinstance(audit.get("stages"), list) or not audit["stages"]
            or audit.get("original_state_before") != audit.get("original_state_after")
            or not isinstance(audit.get("original_state_before"), list) or not audit["original_state_before"]):
        raise ValueError("Optimizer property capture omitted exact SDK completion, original state or cleanup")
    request = requests[0]
    verify_optimizer_stages(audit, body["rva"])
    originals = [*request["owners"], *request["helpers"]]
    if len(audit["original_state_before"]) != len(originals):
        raise ValueError("Optimizer property capture omitted original helper or owner state")
    for original, observed in zip(originals, audit["original_state_before"]):
        _verify_snapshot(optimizer_context_body(request, original), observed)
    verify_optimizer_property_pass(request, audit["passes"][0])
    evidence._typed_function(body)


def _read_identity(root, record):
    path = (root / record["path"]).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError("Boundary recovery evidence escaped the workspace")
    raw = path.read_bytes()
    if len(raw) != record["bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
        raise ValueError("Boundary recovery evidence bytes changed")
    return raw


def _snapshot(body, sdk):
    ea = int(body["address"], 16)
    function = sdk.funcs.get_func(ea)
    if function is None or int(function.start_ea) != ea or hex(int(function.end_ea)) != body["end"]:
        raise ValueError("Private boundary recovery lost its exact function boundary")
    chunks = []
    for start, end in sdk.utils.Chunks(ea):
        raw = sdk.bytes.get_bytes(start, end - start)
        if raw is None:
            raise ValueError("Private boundary recovery function bytes are unavailable")
        chunks.append({"start": hex(start), "end": hex(end), "raw_hex": raw.hex(),
                       "sha256": hashlib.sha256(raw).hexdigest()})
    if ([(c["start"], c["end"], c["raw_hex"]) for c in chunks]
            != [(c["start"], c["end"], c["raw_hex"]) for c in body["chunks"]]
            or any(sdk.bytes.get_bytes(int(ins["address"], 16), len(bytes.fromhex(ins["bytes"])))
                   != bytes.fromhex(ins["bytes"]) for ins in body["assembly"])):
        raise ValueError("Private boundary recovery changed complete original function bytes or chunks")
    tif = sdk.types.tinfo_t()
    present = bool(sdk.nalt.get_tinfo(tif, ea))
    return {"address": body["address"], "end": body["end"], "chunks": chunks,
        "flags": int(function.flags), "type_present": present, "type": str(tif) if present else None,
        "native_frame": {key: int(getattr(function, key)) for key in ("frame", "frsize", "frregs", "argsize", "fpd")},
        "stack_points": [{"ea": ins["address"], "spd": int(sdk.frame.get_spd(function, int(ins["address"], 16))),
                          "delta": int(sdk.frame.get_sp_delta(function, int(ins["address"], 16)))} for ins in body["assembly"]]}


def apply(requested, base, sdk):
    """Split only the reviewed original tail and CPU-decode its proven continuation."""
    raw = _read_identity(sdk.root, requested["source_input"])
    _read_identity(sdk.root, {key: value for key, value in requested["review_source"].items() if key != "pointer"})
    original_owners = []
    for item in requested["shared_owners"]:
        body = json.loads(_read_identity(sdk.root, item["owner_function"]))
        evidence.original_machine(raw, body)
        if (int(body["address"], 16) != base + int(item["owner_rva"], 16)
                or body["chunks"] != item["original_chunks"]):
            raise ValueError("Private shared boundary owner input or image base differs")
        original_owners.append(body)
    helper = helper_body(requested)
    start, end = int(helper["address"], 16), int(helper["end"], 16)
    if (start != base + int(helper["rva"], 16)
            or sdk.bytes.get_bytes(start, end - start) != bytes.fromhex(requested["helper"]["raw_hex"])):
        raise ValueError("Private boundary recovery input, image base or complete helper bytes differ")
    before = [{"owner_rva": item["owner_rva"], "snapshot": _snapshot(body, sdk)}
              for item, body in zip(requested["shared_owners"], original_owners)]
    parent_iterator = sdk.funcs.func_parent_iterator_t()
    tail = sdk.funcs.get_fchunk(start)
    if tail is None or not parent_iterator.set(tail):
        raise ValueError("Private boundary helper has no exact shared tail ownership")
    parents = list(parent_iterator)
    # The SDK iterator owns a range lock. Release it before changing any
    # parent membership, including the final owner that destroys the tail.
    del parent_iterator, tail
    if sorted(parents) != sorted(int(body["address"], 16) for body in original_owners):
        raise ValueError("Private boundary helper parent list differs from all reviewed shared owners")
    old_end = int(requested["original_chunks"][1]["end"], 16)
    for address in range(start, end):
        function = sdk.funcs.get_func(address)
        if ((address < old_end and (function is None or int(function.start_ea) not in parents))
                or (address >= old_end and function is not None)):
            raise ValueError("Private boundary helper has another owner or a newly occupied continuation")
    decoded = []
    for ins in helper["assembly"]:
        address, code = int(ins["address"], 16), bytes.fromhex(ins["bytes"])
        instruction = sdk.ua.insn_t()
        if sdk.ua.decode_insn(instruction, address) != len(code) or int(instruction.size) != len(code):
            raise ValueError("Processor fresh decode differs from the full reviewed helper instruction boundary")
        decoded.append({"address": hex(address), "rva": hex(address - base), "bytes": code.hex(),
                        "size": len(code), "itype": int(instruction.itype)})
    removed = []
    for item, body in zip(requested["shared_owners"], original_owners):
        owner = sdk.funcs.get_func(int(body["address"], 16))
        if not sdk.funcs.remove_func_tail(owner, start):
            raise ValueError("SDK rejected removal of an exact proven shared helper tail from " + item["owner_rva"])
        removed.append(item["owner_rva"])
    for ins in decoded:
        if sdk.ua.create_insn(int(ins["address"], 16)) != ins["size"]:
            raise ValueError("SDK could not create an exact CPU-decoded original helper instruction")
    if not sdk.funcs.add_func(start, end):
        raise ValueError("SDK rejected the complete proven helper boundary")
    retained_owners = []
    for item, original in zip(requested["shared_owners"], original_owners):
        owner = copy.deepcopy(original)
        owner["chunks"] = copy.deepcopy(item["owner_chunks"])
        owner["assembly"] = [ins for ins in original["assembly"] if any(
            int(chunk["start"], 16) <= int(ins["address"], 16) < int(chunk["end"], 16) for chunk in owner["chunks"])]
        retained_owners.append(owner)

    def snapshots():
        return {"owners": [{"owner_rva": item["owner_rva"], "snapshot": _snapshot(body, sdk)}
                           for item, body in zip(requested["shared_owners"], retained_owners)],
                "helper": _snapshot(helper, sdk)}

    split = snapshots()
    invalidations = []
    flag = sdk.funcs.FUNC_SP_READY
    if type(flag) is not int or flag <= 0 or flag & (flag - 1):
        raise ValueError("SDK automatic stack-analysis readiness flag is invalid")
    for body in [*retained_owners, helper]:
        function = sdk.funcs.get_func(int(body["address"], 16))
        previous = int(function.flags)
        function.flags = previous & ~flag
        if not sdk.funcs.update_func(function):
            raise ValueError("SDK rejected automatic boundary stack-analysis invalidation")
        observed = sdk.funcs.get_func(int(body["address"], 16))
        if observed is None or int(observed.start_ea) != int(body["address"], 16) or int(observed.flags) != previous & ~flag:
            raise ValueError("SDK boundary stack-analysis invalidation readback differs")
        invalidations.append({"address": body["address"], "readiness_flag": flag,
            "previous_flags": previous, "invalidated_flags": previous & ~flag,
            "update_func_completed": True, "flag_readback_matches": True})
        sdk.funcs.reanalyze_function(observed)
    if not sdk.auto.auto_wait():
        raise ValueError("Automatic boundary recovery analysis was canceled")
    after = snapshots()
    return {**requested, "before": before, "after_split": split, "after": after,
        "cpu_decoded_instructions": decoded, "tail_removed": True, "helper_created": True,
        "removed_owner_rvas": removed, "original_shared_tail_parents": [hex(ea) for ea in parents],
        "automatic_reanalysis": invalidations, "auto_wait_completed": True,
        "manual_stack_points_modified": False, "manual_frames_modified": False,
        "instructions_patched": False, "base_database_modified": False}


def _verify_snapshot(body, observed):
    if not isinstance(observed, dict):
        raise ValueError("Boundary SDK snapshot is missing")
    chunks = [{"start": c["start"], "end": c["end"], "raw_hex": c["raw_hex"],
               "sha256": hashlib.sha256(bytes.fromhex(c["raw_hex"])).hexdigest()} for c in body["chunks"]]
    points, frame = observed.get("stack_points"), observed.get("native_frame")
    if (observed.get("address") != body["address"] or observed.get("end") != body["end"]
            or observed.get("chunks") != chunks or type(observed.get("flags")) is not int
            or type(observed.get("type_present")) is not bool
            or (observed["type_present"] and (not isinstance(observed.get("type"), str) or not observed["type"].strip()))
            or (not observed["type_present"] and observed.get("type") is not None)
            or not isinstance(frame, dict) or set(frame) != {"frame", "frsize", "frregs", "argsize", "fpd"}
            or any(type(value) is not int for value in frame.values())
            or not isinstance(points, list) or len(points) != len(body["assembly"])
            or any(not isinstance(point, dict) or set(point) != {"ea", "spd", "delta"}
                   or point["ea"] != ins["address"] or type(point["spd"]) is not int or type(point["delta"]) is not int
                   for point, ins in zip(points, body["assembly"]))):
        raise ValueError("Boundary SDK snapshot omits exact boundaries, bytes, frame or stack observations")


def verify_applied(a, requested, applied):
    if (not isinstance(applied, list) or len(applied) != len(requested)
            or any(not isinstance(value, dict) for value in applied)):
        raise ValueError("Boundary recovery omitted an exact SDK operation")
    for row, expected in zip(applied, requested):
        if (any(row.get(key) != value for key, value in expected.items())
                or any(row.get(key) is not True for key in ("tail_removed", "helper_created", "auto_wait_completed"))
                or any(row.get(key) is not False for key in ("manual_stack_points_modified", "manual_frames_modified",
                                                            "instructions_patched", "base_database_modified"))):
            raise ValueError("Boundary recovery changed its exact reviewed scope or omitted SDK completion")
        shared = expected["shared_owners"]
        originals = [json.loads(evidence.checked(a, item["owner_function"], ".analysis/results/native")) for item in shared]
        owners = [owner_body(a, expected, item["owner_rva"]) for item in shared]
        helper = helper_body(expected)
        if (row.get("removed_owner_rvas") != [item["owner_rva"] for item in shared]
                or not isinstance(row.get("original_shared_tail_parents"), list)
                or sorted(row["original_shared_tail_parents"]) != sorted(body["address"] for body in originals)):
            raise ValueError("Boundary recovery omitted or changed shared tail ownership")

        def verify_owners(bodies, observed):
            if (not isinstance(observed, list) or len(observed) != len(shared)
                    or any(not isinstance(item, dict) or set(item) != {"owner_rva", "snapshot"}
                           or item["owner_rva"] != selected["owner_rva"] for item, selected in zip(observed, shared))):
                raise ValueError("Boundary recovery omitted an exact shared owner snapshot")
            for body, item in zip(bodies, observed):
                _verify_snapshot(body, item["snapshot"])

        verify_owners(originals, row.get("before"))
        for stage in ("after_split", "after"):
            observed = row.get(stage)
            if not isinstance(observed, dict) or set(observed) != {"owners", "helper"}:
                raise ValueError("Boundary recovery omitted an owner or helper readback")
            verify_owners(owners, observed["owners"])
            _verify_snapshot(helper, observed["helper"])
        decoded = row.get("cpu_decoded_instructions")
        if (not isinstance(decoded, list) or len(decoded) != len(helper["assembly"])
                or any(not isinstance(item, dict) or set(item) != {"address", "rva", "bytes", "size", "itype"}
                       or any(item.get(key) != ins[key] for key in ("address", "rva", "bytes"))
                       or type(item.get("size")) is not int or item["size"] != len(bytes.fromhex(ins["bytes"]))
                       or type(item.get("itype")) is not int or item["itype"] < 0
                       for item, ins in zip(decoded, helper["assembly"]))):
            raise ValueError("Boundary recovery omitted complete fresh processor instruction decoding")
        invalidations = row.get("automatic_reanalysis")
        if not isinstance(invalidations, list) or len(invalidations) != len(owners) + 1:
            raise ValueError("Boundary recovery omitted automatic owner and helper reanalysis")
        snapshots = [item["snapshot"] for item in row["after_split"]["owners"]] + [row["after_split"]["helper"]]
        for item, body, snapshot in zip(invalidations, [*owners, helper], snapshots):
            flag = item.get("readiness_flag") if isinstance(item, dict) else None
            if (type(flag) is not int or flag <= 0 or flag & (flag - 1)
                    or item.get("address") != body["address"]
                    or type(item.get("previous_flags")) is not int
                    or item["previous_flags"] != snapshot["flags"]
                    or type(item.get("invalidated_flags")) is not int
                    or item["invalidated_flags"] != item["previous_flags"] & ~flag
                    or item.get("update_func_completed") is not True or item.get("flag_readback_matches") is not True):
                raise ValueError("Boundary recovery changed flags or omitted automatic analysis readback")


def _instruction_image(raw):
    from host import inspect_pe
    pe = inspect_pe(raw)
    if pe["architecture"] not in {"x86", "x64"}:
        raise ValueError("Exact E8 instruction recovery requires an x86 or x64 PE")
    base = int(pe["image_base"], 16)

    def read(rva, length):
        matches = [section for section in pe["sections"] if section["rva"] < rva + length
                   and rva < section["rva"] + section["bytes"]]
        if (length <= 0 or len(matches) != 1 or rva < matches[0]["rva"]
                or rva + length > matches[0]["rva"] + matches[0]["bytes"]):
            raise ValueError("Exact E8 instruction is not uniquely backed by original PE bytes")
        at = matches[0]["file_offset"] + rva - matches[0]["rva"]
        return raw[at:at + length]

    return base, read


def _branch_target(instruction):
    code = bytes.fromhex(instruction["bytes"])
    address = int(instruction["address"], 16)
    if len(code) == 6 and code[0] == 0x0f and 0x80 <= code[1] <= 0x8f:
        return address + 6 + int.from_bytes(code[2:], "little", signed=True)
    if len(code) == 5 and code[0] == 0xe9:
        return address + 5 + int.from_bytes(code[1:], "little", signed=True)
    if len(code) == 2 and (0x70 <= code[0] <= 0x7f or code[0] == 0xeb):
        return address + 2 + int.from_bytes(code[1:], "little", signed=True)
    raise ValueError("Exact E8 instruction proof lacks an ordinary original direct branch")


def select_instructions(a, raw, capture, selected_rvas, specifications):
    if (not isinstance(specifications, (tuple, list)) or not 1 <= len(specifications) <= 64
            or len(set(specifications)) != len(specifications) or capture.get("capture_complete") is not True
            or hashlib.sha256(raw).hexdigest() != capture.get("input_sha256")
            or len(raw) != capture.get("input_bytes")):
        raise ValueError("Instruction recovery requires bounded unique reviews and exact complete original bytes")
    base, read = _instruction_image(raw)
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    source = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    base_capture = evidence.identity(a, result / "capture.json")
    if source["bytes"] != len(raw) or source["sha256"] != hashlib.sha256(raw).hexdigest():
        raise ValueError("Instruction recovery original input changed")
    records = {row["rva"]: row for row in capture["functions"]}

    def retained(rva):
        record = records.get(rva)
        if record is None or record.get("assembly_captured") is not True:
            raise ValueError("Instruction recovery lacks a complete retained original function")
        path = a.resolve(a.rel(result / record["path"]), a.rel(result))
        identity = evidence.identity(a, path)
        if identity["sha256"] != record["sha256"] or ("bytes" in record and identity["bytes"] != record["bytes"]):
            raise ValueError("Instruction recovery original function changed")
        body = json.loads(path.read_bytes())
        if body.get("rva") != rva or body.get("address") != hex(base + int(rva, 16)):
            raise ValueError("Instruction recovery original function identity differs")
        evidence.original_machine(raw, body)
        return body, identity

    values, used = [], set()
    keys = {"schema_version", "review_kind", "source_input", "base_capture", "owner_rva", "owner_function",
            "instruction_rva", "raw_hex", "callee_rva", "callee_function", "incoming_branches",
            "assembly_evidence", "target_execution", "raw_artifacts_modified"}
    for specification in specifications:
        review, review_source = _review(a, specification)
        if (not isinstance(review, dict) or set(review) not in (keys, keys | {"clear_stale_far_return"})
                or ("clear_stale_far_return" in review and review["clear_stale_far_return"] is not True)
                or review["schema_version"] != 1
                or review["review_kind"] != "native_exact_e8_instruction_boundary"
                or review["source_input"] != source or review["base_capture"] != base_capture
                or review["target_execution"] is not False or review["raw_artifacts_modified"] is not False
                or any(not isinstance(review[key], str) or not re.fullmatch(r"0x[0-9a-f]+", review[key])
                       or hex(int(review[key], 16)) != review[key] for key in ("owner_rva", "instruction_rva", "callee_rva"))
                or int(review["owner_rva"], 16) not in selected_rvas
                or review["owner_rva"] in used
                or not isinstance(review["assembly_evidence"], str) or not review["assembly_evidence"].strip()
                or len(review["assembly_evidence"]) > 16384):
            raise ValueError("Instruction review lacks its exact retained scope, source or original branch proof")
        owner, owner_identity = retained(review["owner_rva"])
        _, callee_identity = retained(review["callee_rva"])
        start = int(review["instruction_rva"], 16)
        code = read(start, 5)
        address = base + start
        if (review["owner_function"] != owner_identity or review["callee_function"] != callee_identity
                or code[0] != 0xe8 or code.hex() != review["raw_hex"]
                or start + 5 + int.from_bytes(code[1:], "little", signed=True) != int(review["callee_rva"], 16)
                or not int(owner["address"], 16) <= address < address + 5 <= int(owner["end"], 16)
                or owner["chunks"][0]["start"] != owner["address"]
                or owner["chunks"][0]["end"] != owner["end"]):
            raise ValueError("Instruction recovery is not an exact E8 call inside its retained main owner chunk")
        displaced = [ins for ins in owner["assembly"]
                     if int(ins["address"], 16) < address + 5
                     and address < int(ins["address"], 16) + len(bytes.fromhex(ins["bytes"]))]
        if (not displaced or any(not address <= int(ins["address"], 16)
                                 < int(ins["address"], 16) + len(bytes.fromhex(ins["bytes"])) <= address + 5 for ins in displaced)
                or any(ins["address"] == hex(address) and ins["bytes"] == code.hex() for ins in displaced)):
            raise ValueError("Instruction recovery would change an overlapping original boundary or repeat a correct call")
        branches = review["incoming_branches"]
        if (not isinstance(branches, list) or not 1 <= len(branches) <= 64
                or len({ins.get("address") for ins in branches if isinstance(ins, dict)}) != len(branches)):
            raise ValueError("Instruction recovery requires bounded distinct incoming branch evidence")
        for instruction in branches:
            if (instruction not in owner["assembly"] or _branch_target(instruction) != address
                    or address <= int(instruction["address"], 16) < address + 5
                    or not any(ref.get("from") == instruction["address"] and ref.get("to") == hex(address)
                               and ref.get("code") is True for ref in instruction.get("xrefs_from", []))):
                raise ValueError("Instruction recovery incoming branch does not prove the exact original E8 head")
        for instruction in displaced:
            if any(ref.get("code") is True and not address <= int(ref["from"], 16) < address + 5
                   for ref in instruction.get("xrefs_to", [])):
                raise ValueError("Instruction recovery would discard an external original code edge to an interior head")
        value = {"review_source": review_source, "source_input": source, "base_capture": base_capture,
            "owner_rva": review["owner_rva"], "owner_function": owner_identity,
            "callee_rva": review["callee_rva"], "callee_function": callee_identity,
            "original_chunks": copy.deepcopy(owner["chunks"]),
            "instruction": {"address": hex(address), "rva": hex(start), "bytes": code.hex(),
                            "text": "call " + hex(base + int(review["callee_rva"], 16))},
            "displaced_instructions": copy.deepcopy(displaced), "incoming_branches": copy.deepcopy(branches),
            "assembly_evidence": review["assembly_evidence"], "target_execution": False}
        if review.get("clear_stale_far_return"):
            value["near_return_recovery"] = _near_return_proof(raw, owner, displaced, value["instruction"])
        values.append(value)
        used.add(review["owner_rva"])
    return values


def _near_return_proof(raw, original, displaced, instruction):
    from host import inspect_pe
    if (inspect_pe(raw)["architecture"] != "x64" or type(original.get("flags")) is not int
            or original["flags"] & 0x22 != 0x2
            or not any(ins["bytes"] in {"cb"} or re.fullmatch(r"ca[0-9a-f]{4}", ins["bytes"])
                       for ins in displaced)):
        raise ValueError("Stale far-return recovery requires an automatic x64 FAR flag and displaced far RET")
    removed = {ins["address"] for ins in displaced}
    assembly = sorted([ins for ins in original["assembly"] if ins["address"] not in removed]
                      + [instruction], key=lambda ins: int(ins["address"], 16))
    returns, padding = [], []
    for chunk in original["chunks"]:
        start, end, code = int(chunk["start"], 16), int(chunk["end"], 16), bytes.fromhex(chunk["raw_hex"])
        cursor = start
        for ins in [row for row in assembly if start <= int(row["address"], 16) < end]:
            address, value = int(ins["address"], 16), bytes.fromhex(ins["bytes"])
            if address < cursor:
                raise ValueError("Stale far-return proof has overlapping original instruction boundaries")
            if address > cursor:
                gap = code[cursor - start:address - start]
                if gap not in (b"\x90", b"\xcc", b"\x0f\x1f\x44\0\0"):
                    raise ValueError("Stale far-return proof has unreviewed original code or data gaps")
                padding.append({"address": hex(cursor), "bytes": gap.hex()})
            if value[:1] in (b"\xca", b"\xcb") or re.match(r"\s*(?:retf|iret)\b", ins["text"], re.I):
                raise ValueError("Stale far-return proof still contains a real far or interrupt return")
            if value == b"\xc3" or len(value) == 3 and value[0] == 0xc2:
                returns.append(copy.deepcopy(ins))
            elif re.match(r"\s*ret\w*\b", ins["text"], re.I):
                raise ValueError("Stale far-return proof contains an unrecognized return encoding")
            cursor = address + len(value)
        if cursor < end:
            gap = code[cursor - start:]
            if gap not in (b"\x90", b"\xcc", b"\x0f\x1f\x44\0\0"):
                raise ValueError("Stale far-return proof has an unreviewed trailing code or data gap")
            padding.append({"address": hex(cursor), "bytes": gap.hex()})
    if not returns:
        raise ValueError("Stale far-return proof has no actual near return")
    return {"kind": "near_return_after_displaced_far_return", "original_flags": original["flags"],
            "far_flag": 2, "user_far_flag": 32, "near_returns": returns, "padding": padding}


def instruction_body(a, row):
    body = json.loads(evidence.checked(a, row["owner_function"], ".analysis/results/native"))
    displaced = {ins["address"] for ins in row["displaced_instructions"]}
    body["assembly"] = sorted([ins for ins in body["assembly"] if ins["address"] not in displaced]
                              + [copy.deepcopy(row["instruction"])], key=lambda ins: int(ins["address"], 16))
    return body


def _annotation(sdk, address):
    def ref(x):
        return {"from": hex(int(x.frm)), "to": hex(int(x.to)), "type": int(x.type),
                "code": bool(x.iscode), "user": bool(x.user)}
    return {"address": hex(address), "flags": int(sdk.bytes.get_full_flags(address)),
            "item_head": hex(int(sdk.bytes.get_item_head(address))),
            "item_end": hex(int(sdk.bytes.get_item_end(address))),
            "name": sdk.idc.get_name(address), "comment": sdk.idc.get_cmt(address, False),
            "repeatable_comment": sdk.idc.get_cmt(address, True),
            "xrefs_from": [ref(x) for x in sdk.utils.XrefsFrom(address, 0)],
            "xrefs_to": [ref(x) for x in sdk.utils.XrefsTo(address, 0)]}


def apply_instructions(requested, base, sdk):
    original = json.loads(_read_identity(sdk.root, requested["owner_function"]))
    callee = json.loads(_read_identity(sdk.root, requested["callee_function"]))
    raw = _read_identity(sdk.root, requested["source_input"])
    _read_identity(sdk.root, {key: value for key, value in requested["review_source"].items() if key != "pointer"})
    evidence.original_machine(raw, original)
    evidence.original_machine(raw, callee)
    start = int(requested["instruction"]["address"], 16)
    code = bytes.fromhex(requested["instruction"]["bytes"])
    target = base + int(requested["callee_rva"], 16)
    if (int(original["address"], 16) != base + int(requested["owner_rva"], 16)
            or start != base + int(requested["instruction"]["rva"], 16)
            or len(code) != 5 or code[0] != 0xe8 or start + 5 + int.from_bytes(code[1:], "little", signed=True) != target
            or sdk.bytes.get_bytes(start, 5) != code):
        raise ValueError("Private E8 recovery image base, bytes or callee target differ")
    before = _snapshot(original, sdk)
    _snapshot(callee, sdk)
    annotations_before = [_annotation(sdk, address) for address in range(start, start + 5)]
    heads = {ins["address"]: ins for ins in requested["displaced_instructions"]}
    for item in annotations_before:
        if int(item["item_head"], 16) < start or int(item["item_end"], 16) > start + 5:
            raise ValueError("Private E8 recovery overlaps a larger original IDA item")
        original_ins = heads.get(item["address"])
        if original_ins is not None and any(item[key] != original_ins.get(key) for key in (
                "comment", "repeatable_comment", "xrefs_from", "xrefs_to")):
            raise ValueError("Private E8 recovery original annotations or cross-references changed")
        if any(ref["code"] and start < int(ref["to"], 16) < start + 5
               and not start <= int(ref["from"], 16) < start + 5 for ref in item["xrefs_to"]):
            raise ValueError("Private E8 recovery has an external code edge to an interior byte")
    decoded = sdk.ua.insn_t()
    if (sdk.ua.decode_insn(decoded, start) != 5 or int(decoded.size) != 5
            or int(decoded.ops[0].type) != int(sdk.ua.o_near) or int(decoded.ops[0].addr) != target):
        raise ValueError("Processor fresh E8 decode differs from the exact original call target")
    if not sdk.bytes.del_items(start, sdk.bytes.DELIT_SIMPLE, 5):
        raise ValueError("SDK rejected undefining exactly the reviewed five original E8 bytes")
    created = sdk.ua.insn_t()
    if (sdk.ua.create_insn(start, created) != 5 or int(created.size) != 5
            or int(created.ops[0].type) != int(sdk.ua.o_near) or int(created.ops[0].addr) != target):
        raise ValueError("SDK could not create the exact processor-decoded E8 instruction")
    body = copy.deepcopy(original)
    displaced = set(heads)
    body["assembly"] = sorted([ins for ins in body["assembly"] if ins["address"] not in displaced]
                              + [requested["instruction"]], key=lambda ins: int(ins["address"], 16))
    after_decode = _snapshot(body, sdk)
    function = sdk.funcs.get_func(int(body["address"], 16))
    ready = sdk.funcs.FUNC_SP_READY
    if type(ready) is not int or ready <= 0 or ready & (ready - 1):
        raise ValueError("SDK E8 automatic stack-analysis readiness flag is invalid")
    previous = int(function.flags)
    far, return_decodings = 0, []
    if "near_return_recovery" in requested:
        proof = _near_return_proof(raw, original, requested["displaced_instructions"], requested["instruction"])
        if proof != requested["near_return_recovery"]:
            raise ValueError("Private stale far-return proof differs from the complete original bytes")
        far, user_far = sdk.funcs.FUNC_FAR, sdk.funcs.FUNC_USERFAR
        if (far != proof["far_flag"] or user_far != proof["user_far_flag"]
                or previous & (far | user_far) != far):
            raise ValueError("SDK far-return flags differ or function far-ness was explicitly user-specified")
        for ins in [*proof["near_returns"], *proof["padding"]]:
            address, value = int(ins["address"], 16), bytes.fromhex(ins["bytes"])
            decoded_return = sdk.ua.insn_t()
            size = sdk.ua.decode_insn(decoded_return, address)
            mnemonic = decoded_return.get_canon_mnem().lower()
            expected = ({"ret", "retn"} if ins in proof["near_returns"] else
                        {"int", "int3"} if value == b"\xcc" else {"nop"})
            if (size != len(value) or int(decoded_return.size) != size or mnemonic not in expected
                    or sdk.bytes.get_bytes(address, size) != value):
                raise ValueError("SDK fresh decode does not prove the exact original near-return convention: "
                                 + hex(address) + " bytes=" + value.hex() + " size=" + str(size)
                                 + " mnemonic=" + repr(mnemonic))
            return_decodings.append({"address": ins["address"], "bytes": ins["bytes"], "size": size,
                                     "itype": int(decoded_return.itype), "mnemonic": mnemonic})
    function.flags = previous & ~(ready | far)
    if not sdk.funcs.update_func(function):
        raise ValueError("SDK rejected E8 automatic analysis invalidation")
    function = sdk.funcs.get_func(int(body["address"], 16))
    if function is None or int(function.flags) != previous & ~(ready | far):
        raise ValueError("SDK E8 automatic analysis invalidation readback differs")
    sdk.funcs.reanalyze_function(function)
    if not sdk.auto.auto_wait():
        raise ValueError("Automatic E8 instruction reanalysis was canceled")
    after = _snapshot(body, sdk)
    decoded_after = sdk.ua.insn_t()
    if (sdk.ua.decode_insn(decoded_after, start) != 5 or int(decoded_after.ops[0].type) != int(sdk.ua.o_near)
            or int(decoded_after.ops[0].addr) != target or sdk.bytes.get_bytes(start, 5) != code):
        raise ValueError("Automatic analysis changed the exact recovered original E8 call")
    annotations_after = [_annotation(sdk, address) for address in range(start, start + 5)]
    if (annotations_after[0]["item_head"] != hex(start) or annotations_after[0]["item_end"] != hex(start + 5)
            or any(item["item_head"] != hex(start) or item["item_end"] != hex(start + 5) for item in annotations_after)
            or not any(ref["code"] and ref["from"] == hex(start) and ref["to"] == hex(target)
                       for ref in annotations_after[0]["xrefs_from"])):
        raise ValueError("SDK E8 item boundary or actual callee cross-reference readback differs")
    return {**requested, "before": before, "after_decode": after_decode, "after": after,
        "annotations_before": annotations_before, "annotations_after": annotations_after,
        "cpu_decoding": {"size": 5, "itype": int(decoded.itype), "operand_kind": "o_near",
                         "target": hex(target), "readback_target": hex(int(decoded_after.ops[0].addr))},
        "automatic_reanalysis": {"readiness_flag": ready, "previous_flags": previous,
            "invalidated_flags": previous & ~(ready | far), "update_func_completed": True, "flag_readback_matches": True},
        **({"near_return_flag_reanalysis": {"far_flag": far, "user_far_flag": user_far,
            "cpu_decodings": return_decodings, "automatic_far_flag_cleared": True}}
           if "near_return_recovery" in requested else {}),
        "exact_five_bytes_undefined": True, "exact_e8_created": True, "auto_wait_completed": True,
        "manual_stack_points_modified": False, "manual_frames_modified": False,
        "instructions_patched": False, "base_database_modified": False}


def _verify_annotations(annotations, start):
    if not isinstance(annotations, list) or len(annotations) != 5:
        raise ValueError("E8 recovery omitted exact original byte annotations")
    for address, item in zip(range(start, start + 5), annotations):
        if (not isinstance(item, dict) or set(item) != {"address", "flags", "item_head", "item_end", "name",
                                                      "comment", "repeatable_comment", "xrefs_from", "xrefs_to"}
                or item.get("address") != hex(address) or type(item.get("flags")) is not int
                or any(not isinstance(item.get(key), str) or not re.fullmatch(r"0x[0-9a-f]+", item[key])
                       for key in ("item_head", "item_end"))
                or not isinstance(item.get("name"), str)
                or any(item.get(key) is not None and not isinstance(item[key], str) for key in ("comment", "repeatable_comment"))):
            raise ValueError("E8 recovery original byte annotations are incomplete")
        for key in ("xrefs_from", "xrefs_to"):
            refs = item.get(key)
            if (not isinstance(refs, list) or any(not isinstance(ref, dict) or set(ref) != {"from", "to", "type", "code", "user"}
                    or any(not isinstance(ref.get(field), str) or not re.fullmatch(r"0x[0-9a-f]+", ref[field]) for field in ("from", "to"))
                    or type(ref.get("type")) is not int or type(ref.get("code")) is not bool or type(ref.get("user")) is not bool
                    for ref in refs)):
                raise ValueError("E8 recovery cross-reference observations are incomplete")


def verify_instructions(a, requested, applied):
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("E8 recovery omitted an exact requested SDK operation")
    for row, expected in zip(applied, requested):
        if (not isinstance(row, dict) or any(row.get(key) != value for key, value in expected.items())
                or any(row.get(key) is not True for key in ("exact_five_bytes_undefined", "exact_e8_created", "auto_wait_completed"))
                or any(row.get(key) is not False for key in ("manual_stack_points_modified", "manual_frames_modified",
                                                            "instructions_patched", "base_database_modified"))):
            raise ValueError("E8 recovery changed original scope or omitted exact SDK completion")
        original = json.loads(evidence.checked(a, expected["owner_function"], ".analysis/results/native"))
        recovered = instruction_body(a, expected)
        _verify_snapshot(original, row.get("before"))
        _verify_snapshot(recovered, row.get("after_decode"))
        _verify_snapshot(recovered, row.get("after"))
        start = int(expected["instruction"]["address"], 16)
        target = start + 5 + int.from_bytes(bytes.fromhex(expected["instruction"]["bytes"])[1:], "little", signed=True)
        for key in ("annotations_before", "annotations_after"):
            _verify_annotations(row.get(key), start)
        before = {item["address"]: item for item in row["annotations_before"]}
        for instruction in expected["displaced_instructions"]:
            if any(before[instruction["address"]][key] != instruction.get(key) for key in (
                    "comment", "repeatable_comment", "xrefs_from", "xrefs_to")):
                raise ValueError("E8 recovery discarded original instruction annotations")
        if (any(item["item_head"] != hex(start) or item["item_end"] != hex(start + 5) for item in row["annotations_after"])
                or not any(ref["code"] and ref["from"] == hex(start) and ref["to"] == hex(target)
                           for ref in row["annotations_after"][0]["xrefs_from"])):
            raise ValueError("E8 recovery omitted its actual item and callee cross-reference readback")
        decoded = row.get("cpu_decoding")
        if (not isinstance(decoded, dict) or set(decoded) != {"size", "itype", "operand_kind", "target", "readback_target"}
                or type(decoded.get("size")) is not int or decoded["size"] != 5
                or type(decoded.get("itype")) is not int or decoded["itype"] < 0
                or decoded.get("operand_kind") != "o_near" or decoded.get("target") != hex(target)
                or decoded.get("readback_target") != hex(target)):
            raise ValueError("E8 recovery omitted exact CPU target decoding")
        invalidation = row.get("automatic_reanalysis")
        flag = invalidation.get("readiness_flag") if isinstance(invalidation, dict) else None
        far = 0
        if "near_return_recovery" in expected:
            raw = evidence.checked(a, expected["source_input"], ".analysis/inputs")
            proof = _near_return_proof(raw, original, expected["displaced_instructions"], expected["instruction"])
            audit = row.get("near_return_flag_reanalysis")
            if (proof != expected["near_return_recovery"] or not isinstance(audit, dict)
                    or set(audit) != {"far_flag", "user_far_flag", "cpu_decodings", "automatic_far_flag_cleared"}
                    or type(audit.get("far_flag")) is not int or audit["far_flag"] != proof["far_flag"]
                    or type(audit.get("user_far_flag")) is not int or audit["user_far_flag"] != proof["user_far_flag"]
                    or audit.get("automatic_far_flag_cleared") is not True
                    or row["after"]["flags"] & proof["far_flag"]):
                raise ValueError("Stale far-return recovery lacks exact SDK flag correction and readback")
            far = proof["far_flag"]
            decoded, originals = audit.get("cpu_decodings"), [*proof["near_returns"], *proof["padding"]]
            if not isinstance(decoded, list) or len(decoded) != len(originals):
                raise ValueError("Stale far-return recovery omitted fresh return or padding CPU decoding")
            for value, ins in zip(decoded, originals):
                allowed = ({"ret", "retn"} if ins in proof["near_returns"] else
                           {"int", "int3"} if ins["bytes"] == "cc" else {"nop"})
                if (not isinstance(value, dict) or set(value) != {"address", "bytes", "size", "itype", "mnemonic"}
                        or value["address"] != ins["address"] or value["bytes"] != ins["bytes"]
                        or type(value["size"]) is not int or value["size"] != len(bytes.fromhex(ins["bytes"]))
                        or type(value["itype"]) is not int or value["itype"] < 0 or value["mnemonic"] not in allowed):
                    raise ValueError("Stale far-return recovery has an invalid exact CPU return observation")
        elif "near_return_flag_reanalysis" in row:
            raise ValueError("E8 recovery contains unreviewed far-return flag changes")
        if (type(flag) is not int or flag <= 0 or flag & (flag - 1)
                or type(invalidation.get("previous_flags")) is not int
                or invalidation["previous_flags"] != row["after_decode"]["flags"]
                or type(invalidation.get("invalidated_flags")) is not int
                or invalidation["invalidated_flags"] != invalidation["previous_flags"] & ~(flag | far)
                or invalidation.get("update_func_completed") is not True or invalidation.get("flag_readback_matches") is not True):
            raise ValueError("E8 recovery omitted automatic analysis invalidation and readback")


def _merged_body(requested, root):
    bodies = [json.loads(_read_identity(root, member)) for member in requested["members"]]
    body = copy.deepcopy(bodies[0])
    body["end"] = requested["expected_end"]
    body["chunks"] = copy.deepcopy(requested["expected_chunks"])
    body["assembly"] = [ins for member in bodies for ins in member["assembly"]]
    return body


def reanalyze_merges(requested, base, sdk):
    """Refresh only processor-derived stack analysis of complete merged owners."""
    values = []
    for merge in requested:
        body = _merged_body(merge, sdk.root)
        if body["address"] != hex(base + int(merge["owner_rva"], 16)):
            raise ValueError("Merged owner automatic reanalysis has another image base")
        before = _snapshot(body, sdk)
        flag = sdk.funcs.FUNC_SP_READY
        if type(flag) is not int or flag <= 0 or flag & (flag - 1):
            raise ValueError("SDK merged stack-analysis readiness flag is invalid")
        function = sdk.funcs.get_func(int(body["address"], 16))
        invalidated = int(function.flags) & ~flag
        function.flags = invalidated
        if not sdk.funcs.update_func(function):
            raise ValueError("SDK rejected merged automatic stack-analysis invalidation")
        function = sdk.funcs.get_func(int(body["address"], 16))
        if function is None or int(function.start_ea) != int(body["address"], 16) or int(function.flags) != invalidated:
            raise ValueError("SDK merged automatic analysis invalidation readback differs")
        sdk.funcs.reanalyze_function(function)
        if not sdk.auto.auto_wait():
            raise ValueError("Merged automatic stack analysis was canceled")
        after = _snapshot(body, sdk)
        values.append({"owner_rva": merge["owner_rva"], "members": copy.deepcopy(merge["members"]),
            "before": before, "after": after,
            "analysis_invalidation": {"readiness_flag": flag, "previous_flags": before["flags"],
                "invalidated_flags": invalidated, "update_func_completed": True, "flag_readback_matches": True},
            "reanalyze_function_requested": True, "auto_wait_completed": True,
            "manual_stack_points_modified": False, "manual_frames_modified": False,
            "instructions_patched": False, "base_database_modified": False, "target_execution": False})
    return values


def verify_merge_reanalysis(a, requested, applied):
    if (not isinstance(applied, list) or len(applied) != len(requested)
            or any(not isinstance(row, dict) for row in applied)):
        raise ValueError("Merged automatic reanalysis omitted an exact owner")
    for row, merge in zip(applied, requested):
        body = a.native_function_merge_body(merge)
        if (row.get("owner_rva") != merge["owner_rva"] or row.get("members") != merge["members"]
                or any(row.get(key) is not True for key in ("reanalyze_function_requested", "auto_wait_completed"))
                or any(row.get(key) is not False for key in ("manual_stack_points_modified", "manual_frames_modified",
                                                            "instructions_patched", "base_database_modified", "target_execution"))):
            raise ValueError("Merged automatic reanalysis changed its original union or omitted SDK completion")
        _verify_snapshot(body, row.get("before"))
        _verify_snapshot(body, row.get("after"))
        invalidation = row.get("analysis_invalidation")
        flag = invalidation.get("readiness_flag") if isinstance(invalidation, dict) else None
        if (type(flag) is not int or flag <= 0 or flag & (flag - 1)
                or type(invalidation.get("previous_flags")) is not int
                or invalidation["previous_flags"] != row["before"]["flags"]
                or type(invalidation.get("invalidated_flags")) is not int
                or invalidation["invalidated_flags"] != invalidation["previous_flags"] & ~flag
                or invalidation.get("update_func_completed") is not True or invalidation.get("flag_readback_matches") is not True):
            raise ValueError("Merged automatic reanalysis changed flags or omitted readiness readback")


def select_prepasses(a, raw, capture, selected_rvas, requested_rvas, reason, partitions=(), instructions=()):
    """Select explicit unchanged owners for one retained SDK derivation pass."""
    if (not isinstance(requested_rvas, (list, tuple)) or not 1 <= len(requested_rvas) <= 256
            or any(type(rva) is not int or rva < 0 for rva in requested_rvas)
            or len(set(requested_rvas)) != len(requested_rvas)
            or not set(requested_rvas).issubset(selected_rvas)
            or not isinstance(reason, str) or not reason.strip() or len(reason) > 65536
            or capture.get("capture_complete") is not True
            or capture.get("input_sha256") != hashlib.sha256(raw).hexdigest()
            or capture.get("input_bytes") != len(raw)):
        raise ValueError("Decompiler prepass requires exact selected owners, original bytes and explicit evidence")
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    records = {row["rva"]: row for row in capture["functions"]}
    values = []
    for rva in requested_rvas:
        record = records.get(hex(rva))
        if record is None:
            raise ValueError("Decompiler prepass owner is not a retained complete function")
        path = a.resolve(a.rel(result / record["path"]), a.rel(result))
        source = evidence.identity(a, path)
        if source["sha256"] != record["sha256"] or ("bytes" in record and source["bytes"] != record["bytes"]):
            raise ValueError("Decompiler prepass original function bytes changed")
        body = json.loads(path.read_bytes())
        if body.get("rva") != hex(rva) or body.get("address") != record["address"]:
            raise ValueError("Decompiler prepass original function identity differs")
        evidence.original_machine(raw, body)
        value = {"rva": hex(rva), "source_function": source,
                 "original_chunks": copy.deepcopy(body["chunks"]), "reason": reason}
        matches = [row for row in partitions if row["owner_rva"] == hex(rva)]
        if matches:
            if len(matches) != 1:
                raise ValueError("Decompiler prepass has duplicate original partition owners")
            value["partition_boundary"] = copy.deepcopy(matches[0])
            _prepass_body(value, body)
        matches = [row for row in instructions if row["owner_rva"] == hex(rva)]
        if matches:
            if len(matches) != 1 or "partition_boundary" in value:
                raise ValueError("Decompiler prepass has conflicting original boundary repairs")
            value["instruction_boundary"] = copy.deepcopy(matches[0])
            _prepass_body(value, body)
        values.append(value)
    return values


def _prepass_body(requested, original):
    if original.get("rva") != requested["rva"] or original.get("chunks") != requested["original_chunks"]:
        if "entry_partition" not in requested or original.get("rva") != requested["entry_partition"]["owner_rva"]:
            raise ValueError("Decompiler prepass original owner changed")
    entry = requested.get("entry_partition")
    if entry is not None:
        if (entry.get("owner_function") != requested["source_function"]
                or entry.get("original_chunks") != original["chunks"]
                or requested["original_chunks"] != original["chunks"]
                or "partition_boundary" in requested or "instruction_boundary" in requested):
            raise ValueError("Decompiler prepass entry partition differs from its complete original owner")
        matches = [body for body in _entry_piece_bodies(original, entry) if body["rva"] == requested["rva"]]
        if len(matches) != 1:
            raise ValueError("Decompiler prepass does not name one exact machine entry")
        return matches[0]
    partition = requested.get("partition_boundary")
    instruction = requested.get("instruction_boundary")
    if instruction is not None:
        if (partition is not None or instruction.get("owner_rva") != requested["rva"]
                or instruction.get("owner_function") != requested["source_function"]
                or instruction.get("original_chunks") != original["chunks"]
                or not instruction.get("displaced_instructions")
                or any(ins not in original["assembly"] for ins in instruction["displaced_instructions"])):
            raise ValueError("Decompiler prepass instruction recovery differs from its complete original owner")
        replacement = instruction.get("instruction", {})
        address, code = int(replacement["address"], 16), bytes.fromhex(replacement["bytes"])
        containers = [chunk for chunk in original["chunks"]
                      if int(chunk["start"], 16) <= address < address + len(code) <= int(chunk["end"], 16)]
        if (len(code) != 5 or code[0] != 0xe8 or len(containers) != 1
                or bytes.fromhex(containers[0]["raw_hex"])[address - int(containers[0]["start"], 16):
                    address - int(containers[0]["start"], 16) + 5] != code
                or any(not address <= int(ins["address"], 16)
                       < int(ins["address"], 16) + len(bytes.fromhex(ins["bytes"])) <= address + 5
                       for ins in instruction["displaced_instructions"])):
            raise ValueError("Decompiler prepass instruction boundary changes original bytes or scope")
        body = copy.deepcopy(original)
        removed = {ins["address"] for ins in instruction["displaced_instructions"]}
        body["assembly"] = sorted([ins for ins in body["assembly"] if ins["address"] not in removed]
            + [copy.deepcopy(instruction["instruction"])], key=lambda ins: int(ins["address"], 16))
        return body
    if partition is None:
        return original
    if (partition.get("owner_rva") != requested["rva"] or partition.get("source_function") != requested["source_function"]
            or partition.get("original_chunks") != original["chunks"] or len(original["chunks"]) != 1
            or not partition.get("owner_chunks") or not partition.get("helpers")):
        raise ValueError("Decompiler prepass partition differs from its complete original owner")
    chunks = partition["owner_chunks"]
    pieces = sorted(chunks + [chunk for helper in partition["helpers"] for chunk in helper["expected_chunks"]],
                    key=lambda chunk: int(chunk["start"], 16))
    source = original["chunks"][0]
    if (pieces[0]["start"] != source["start"] or pieces[-1]["end"] != source["end"]
            or any(left["end"] != right["start"] for left, right in zip(pieces, pieces[1:]))
            or "".join(chunk["raw_hex"] for chunk in pieces) != source["raw_hex"]
            or any(hashlib.sha256(bytes.fromhex(chunk["raw_hex"])).hexdigest() != chunk["sha256"] for chunk in pieces)
            or chunks[0]["start"] != original["address"]):
        raise ValueError("Decompiler prepass partition omits or changes original bytes")
    body = copy.deepcopy(original)
    body.update(chunks=copy.deepcopy(chunks), end=chunks[0]["end"],
        assembly=[ins for ins in original["assembly"] if any(
            int(chunk["start"], 16) <= int(ins["address"], 16) < int(chunk["end"], 16) for chunk in chunks)])
    return body


def prepass_snapshot(requested, sdk):
    """Read the complete original frame/SP state without changing the IDB."""
    body = _prepass_body(requested, json.loads(_read_identity(sdk.root, requested["source_function"])))
    return _snapshot(body, sdk)


def verify_prepass(a, requested, body):
    """Retain the first derivation, including failures and all changed display text."""
    original = _prepass_body(requested,
        json.loads(evidence.checked(a, requested["source_function"], ".analysis/results/native")))
    audit = body.get("decompiler_prepass")
    if (not isinstance(audit, dict) or any(audit.get(key) != value for key, value in requested.items())
            or any(body.get(key) != original.get(key) for key in ("address", "rva", "end", "chunks"))
            or any(audit.get(key) is not False for key in ("manual_stack_points_modified", "manual_frames_modified",
                                                          "instructions_patched", "base_database_modified", "target_execution"))
            or not isinstance(body.get("assembly"), list)
            or not isinstance(body.get("assembly_after_decompile"), list)):
        raise ValueError("Decompiler prepass changed its exact original owner or omitted its audit")
    machine = [(ins["address"], ins["rva"], ins["bytes"]) for ins in original["assembly"]]
    for key in ("assembly", "assembly_after_decompile"):
        if [(ins["address"], ins["rva"], ins["bytes"]) for ins in body[key]] != machine:
            raise ValueError("Decompiler prepass changed original instruction bytes, order or boundaries")
    for key in ("before", "after"):
        _verify_snapshot(original, audit.get(key))


def verify_prepasses(a, requested, applied, output):
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("Decompiler prepass omitted an explicitly selected owner")
    for expected, row in zip(requested, applied):
        if (not isinstance(row, dict) or set(row) != set(expected) | {"capture"}
                or any(row.get(key) != value for key, value in expected.items())):
            raise ValueError("Decompiler prepass changed its requested scope")
        record = row["capture"]
        name = "preanalysis/" + expected["rva"][2:] + ".json"
        if not isinstance(record, dict) or record.get("path") != name:
            raise ValueError("Decompiler prepass capture path is not exact")
        absolute = a.resolve(a.rel(output / name), a.rel(output))
        identity = {**record, "path": a.rel(absolute)}
        body = json.loads(evidence.checked(a, identity, ".analysis/results/native"))
        verify_prepass(a, expected, body)


def _frame_layout(body):
    layout = body.get("native_frame_layout_before_decompile")
    if (not isinstance(layout, dict) or layout.get("status") != "api_returned"
            or layout.get("references_complete") is not True or layout.get("diagnostic_errors")
            or layout.get("target_execution") is not False or layout.get("frame_modified") is not False
            or not isinstance(layout.get("members"), list) or not layout["members"]
            or len(layout["members"]) != layout.get("member_count")
            or not isinstance(layout.get("member_references"), list)
            or len(layout["member_references"]) != len(layout["members"])
            or not isinstance(layout.get("memory_operands"), list)):
        raise ValueError("Frame recovery lacks complete original SDK member and machine-reference observations")
    for index, (member, references) in enumerate(zip(layout["members"], layout["member_references"])):
        if (member.get("index") != index or references.get("member_index") != index
                or references.get("offset_bits") != member.get("offset_bits")
                or references.get("size_bits") != member.get("size_bits")
                or not isinstance(references.get("references"), list)):
            raise ValueError("Frame recovery member/reference identities differ")
    assembly = {row["address"]: row for row in body["assembly"]}
    for operand in layout["memory_operands"]:
        original = assembly.get(operand.get("address"))
        if (original is None or operand.get("instruction_bytes") != original["bytes"]
                or type(operand.get("operand")) is not int or not 0 <= operand["operand"] < 8
                or type(operand.get("dtype_bytes")) is not int or operand["dtype_bytes"] <= 0
                or type(operand.get("stack_operand")) is not bool
                or operand["stack_operand"] and type(operand.get("frame_offset")) is not int):
            raise ValueError("Frame recovery memory operand is not an exact original instruction observation")
    return layout


def select_frame_rebuilds(a, raw, capture, selected_rvas, specifications):
    """Bind explicit CPU frame reconstruction to the full corrupt frame and its references."""
    from host import inspect_pe
    if (not isinstance(specifications, (tuple, list)) or not 1 <= len(specifications) <= 64
            or len(set(specifications)) != len(specifications)
            or capture.get("capture_complete") is not True
            or hashlib.sha256(raw).hexdigest() != capture.get("input_sha256")
            or len(raw) != capture.get("input_bytes")):
        raise ValueError("CPU frame recovery requires bounded unique reviews and complete original bytes")
    pe = inspect_pe(raw)
    if pe["architecture"] != "x64":
        raise ValueError("CPU frame reconstruction currently requires an x64 original")
    base = int(pe["image_base"], 16)
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    original_input = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    base_identity = evidence.identity(a, result / "capture.json")
    records = {row["rva"]: row for row in capture["functions"]}
    values, used = [], set()
    for specification in specifications:
        review, review_source = _review(a, specification)
        rva = review.get("owner_rva")
        if (review.get("schema_version") != 1 or review.get("review_kind") != "native_cpu_frame_reconstruction"
                or review.get("source_input") != original_input or review.get("base_capture") != base_identity
                or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
                or rva not in records or int(rva, 16) not in selected_rvas or rva in used
                or review.get("discard_corrupt_frame") is not True
                or not isinstance(review.get("reason"), str) or not review["reason"].strip()):
            raise ValueError("CPU frame review lacks its exact selected original and explicit reconstruction scope")
        path = a.resolve(a.rel(result / records[rva]["path"]), a.rel(result))
        source_identity = evidence.identity(a, path)
        if source_identity != review.get("owner_function") or source_identity["sha256"] != records[rva]["sha256"]:
            raise ValueError("CPU frame recovery original function changed")
        owner = json.loads(path.read_bytes())
        observed = json.loads(evidence.checked(a, review.get("frame_evidence")))
        evidence.original_machine(raw, owner)
        evidence.original_machine(raw, observed)
        if (any(owner.get(key) != observed.get(key) for key in ("address", "rva", "end", "chunks", "xrefs_to_entry"))
                or [(row["address"], row["bytes"]) for row in owner["assembly"]]
                   != [(row["address"], row["bytes"]) for row in observed["assembly"]]
                or hex(base + int(rva, 16)) != owner["address"]):
            raise ValueError("CPU frame evidence differs from the complete original machine function")
        layout = _frame_layout(observed)
        if review.get("original_frame") != layout:
            raise ValueError("CPU frame review omitted original members, comments, types or full reference observations")
        native, parts = layout.get("native_function", {}), layout.get("parts", {})
        ret, count = parts.get("retaddr"), layout.get("return_address_bytes")
        if (type(ret) is not int or type(count) is not int or count <= 0
                or native.get("start_ea") != int(owner["address"], 16)
                or native.get("end_ea") != int(owner["end"], 16)
                or any(row.get("return_address") is True for row in layout["members"])
                or not any(row["offset_bits"] < (ret + count) * 8
                           and ret * 8 < row["offset_bits"] + row["size_bits"]
                           and row.get("special_member") is False for row in layout["members"])
                or layout.get("layout", {}).get("total_size", 0) <= layout.get("full_frame_bytes", 0)):
            raise ValueError("CPU frame recovery is not supported by an observed corrupt return-address layout")
        decisions = review.get("member_reviews")
        invalid = [row for row in layout["members"] if row["offset_bits"] >= ret * 8
                   and row.get("special_member") is False]
        if (not isinstance(decisions, list) or len(decisions) != len(invalid)
                or any(not isinstance(row, dict) or row.get("member") != member
                       or not isinstance(row.get("reason"), str) or not row["reason"].strip()
                       or not isinstance(row.get("machine_evidence"), list) or not row["machine_evidence"]
                       or any(instruction not in owner["assembly"] for instruction in row["machine_evidence"])
                       for row, member in zip(decisions, invalid))):
            raise ValueError("CPU frame recovery omitted complete per-member machine and lifetime evidence")
        values.append({"review_source": review_source, "owner_rva": rva,
                       "owner_function": source_identity, "frame_evidence": review["frame_evidence"],
                       "original_frame": copy.deepcopy(layout), "member_reviews": copy.deepcopy(decisions),
                       "discard_corrupt_frame": True, "reason": review["reason"]})
        used.add(rva)
    return values


def _rebuilt_frame(body, layout):
    _frame_layout({**body, "native_frame_layout_before_decompile": layout})
    native, parts = layout.get("native_function", {}), layout.get("parts", {})
    ret, width = parts.get("retaddr"), layout.get("return_address_bytes")
    markers = [member for member in layout["members"] if member.get("return_address") is True]
    if (native.get("start_ea") != int(body["address"], 16) or native.get("end_ea") != int(body["end"], 16)
            or type(ret) is not int or type(width) is not int or width <= 0 or len(markers) != 1
            or markers[0].get("offset_bits") != ret * 8 or markers[0].get("size_bits") != width * 8
            or markers[0].get("special_member") is not True
            or any(member["offset_bits"] < (ret + width) * 8
                   and ret * 8 < member["offset_bits"] + member["size_bits"]
                   for member in layout["members"] if member is not markers[0])):
        raise ValueError("CPU frame reconstruction lacks an exact nonoverlapped special return-address member")


def apply_frame_rebuild(requested, base, sdk, checkpoint):
    """Let the processor reconstruct one corrupt frame, retaining every destructive-stage observation."""
    body = json.loads(_read_identity(sdk.root, requested["owner_function"]))
    if body["address"] != hex(base + int(requested["owner_rva"], 16)):
        raise ValueError("CPU frame reconstruction image base differs")
    before = _snapshot(body, sdk)
    function = sdk.funcs.get_func(int(body["address"], 16))
    old_layout = sdk.frame_layout(function)
    if old_layout != requested["original_frame"]:
        raise ValueError("CPU frame reconstruction original layout changed before the reviewed operation")
    processor = sdk.idp.get_ph()
    create_frame = getattr(processor, "create_func_frame", None)
    if not callable(create_frame):
        raise ValueError("Installed processor object does not expose the verified frame reconstruction API")
    row = {**copy.deepcopy(requested), "before": before, "frame_before": old_layout,
           "operation": "ida_idp.get_ph().create_func_frame", "completed": False,
           "base_database_modified": False, "target_execution": False,
           "manual_stack_points_modified": False, "manual_frame_values_applied": False,
           "instructions_patched": False}
    checkpoint(row)
    if not sdk.frame.del_frame(function):
        raise ValueError("SDK refused to release the explicitly reviewed corrupt private frame")
    row["corrupt_frame_deleted"] = True
    checkpoint(row)
    result = int(create_frame(function))
    row["processor_result"] = result
    function = sdk.funcs.get_func(int(body["address"], 16))
    row["frame_after_processor"] = sdk.frame_layout(function)
    checkpoint(row)
    if result != 1:
        raise ValueError("Processor did not reconstruct the exact original function frame")
    sdk.funcs.reanalyze_function(function)
    if not sdk.auto.auto_wait():
        raise ValueError("CPU frame reconstruction automatic analysis was canceled")
    row["auto_wait_completed"] = True
    row["after"] = _snapshot(body, sdk)
    row["frame_after"] = sdk.frame_layout(sdk.funcs.get_func(int(body["address"], 16)))
    checkpoint(row)
    if before["type_present"] != row["after"]["type_present"] or before["type"] != row["after"]["type"]:
        raise ValueError("CPU frame reconstruction changed the original function declaration")
    if row["frame_after"].get("status") != "api_returned" or row["frame_after"].get("references_complete") is not True:
        raise ValueError("CPU reconstructed frame has incomplete SDK observations")
    _rebuilt_frame(body, row["frame_after"])
    row["completed"] = True
    checkpoint(row)
    return row


def verify_frame_rebuilds(a, requested, applied):
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("CPU frame reconstruction omitted a reviewed owner")
    for request, row in zip(requested, applied):
        body = json.loads(evidence.checked(a, request["owner_function"]))
        if (not isinstance(row, dict) or any(row.get(key) != value for key, value in request.items())
                or row.get("frame_before") != request["original_frame"]
                or row.get("operation") != "ida_idp.get_ph().create_func_frame"
                or type(row.get("processor_result")) is not int or row["processor_result"] != 1
                or any(row.get(key) is not True for key in ("completed", "corrupt_frame_deleted", "auto_wait_completed"))
                or any(row.get(key) is not False for key in ("base_database_modified", "target_execution",
                    "manual_stack_points_modified", "manual_frame_values_applied", "instructions_patched"))):
            raise ValueError("CPU frame reconstruction lacks exact scope, automatic SDK completion or retained original layout")
        _verify_snapshot(body, row.get("before"))
        _verify_snapshot(body, row.get("after"))
        if any(row["before"][key] != row["after"][key] for key in ("type_present", "type")):
            raise ValueError("CPU frame reconstruction changed its original function declaration")
        for key in ("frame_after_processor", "frame_after"):
            layout = row.get(key, {})
            _frame_layout({**body, "native_frame_layout_before_decompile": layout})
            if (layout["native_function"]["start_ea"] != int(body["address"], 16)
                    or layout["native_function"]["end_ea"] != int(body["end"], 16)):
                raise ValueError("CPU frame reconstruction lost the original function boundary")
        _rebuilt_frame(body, row["frame_after"])


def arm64_probe_stack_proof(raw, owner, probe):
    """Derive a missing automatic SP point from a complete linear allocation and probe."""
    from host import inspect_pe
    if inspect_pe(raw)["architecture"] != "arm64":
        raise ValueError("Derived probe stack analysis requires an ARM64 PE")
    for body in (owner, probe):
        evidence.original_machine(raw, body)
        assembly = body.get("assembly", [])
        if (len(body.get("chunks", [])) != 1 or not assembly
                or body["chunks"][0]["start"] != body["address"]
                or body["chunks"][0]["end"] != body["end"]
                or [int(row["address"], 16) for row in assembly]
                   != list(range(int(body["address"], 16), int(body["end"], 16), 4))
                or any(len(bytes.fromhex(row["bytes"])) != 4 for row in assembly)):
            raise ValueError("Derived stack proof requires complete contiguous four-byte instructions")
    p = [int.from_bytes(bytes.fromhex(row["bytes"]), "little") for row in probe["assembly"]]
    # Both loop edges, both returns, the TEB limit read and all clobbered
    # registers are part of this complete processor probe template.
    if p != [0xf9400a51, 0xeb2f73f0, 0x9a9033f0, 0xeb11021f,
             0x54000043, 0xd65f03c0, 0x9274ce10, 0xd1400631,
             0xf940023f, 0xeb10023f, 0x54ffffa1, 0xd65f03c0]:
        raise ValueError("Complete stack probe does not prove preservation of X15 and SP")
    assembly = owner["assembly"]
    words = [int.from_bytes(bytes.fromhex(row["bytes"]), "little") for row in assembly]
    if (type(owner.get("flags")) is not int or owner["flags"] & 1 == 0
            or len(words) < 7 or len(words) > 128 or words[:3] != [0xd503237f, 0xa9bf7bfd, 0x910003fd]
            or words[3] & 0xffe0001f != 0xd280000f
            or words[4] & 0xfc000000 != 0x94000000 or words[5] != 0xcb2f73ff):
        raise ValueError("Allocation lacks the exact PAC/frame/MOVZ/probe/SUB definition chain")
    immediate = words[4] & 0x3ffffff
    target = int(assembly[4]["address"], 16) + ((immediate - (1 << 26) if immediate & (1 << 25) else immediate) << 2)
    if target != int(probe["address"], 16):
        raise ValueError("Original BL does not target the fully reviewed probe")
    units = (words[3] >> 5) & 0xffff
    shift = (words[5] >> 10) & 7
    delta = -(units << shift)
    if units == 0 or not -(16 << 20) <= delta < 0 or shift != 4:
        raise ValueError("Derived allocation is empty, unbounded or not the observed UXTX shift")
    for instruction in assembly[1:]:
        address = int(instruction["address"], 16)
        if any(edge.get("code") is True and int(edge["from"], 16) != address - 4
               for edge in instruction.get("xrefs_to", [])):
            raise ValueError("Derived stack allocation has an alternate internal entry")
    # The remainder may address this frame and make ordinary ABI calls. It
    # cannot change SP, branch around the point, or have an additional entry.
    for index, word in enumerate(words[6:], 6):
        address = int(assembly[index]["address"], 16)
        if word & 0xfc000000 == 0x94000000:
            pass
        elif word & 0x9f000000 == 0x90000000:  # ADRP Xd
            if word & 31 == 31:
                raise ValueError("Derived stack owner has a noncanonical ADRP destination")
        elif word & 0xff000000 == 0x91000000:  # ADD Xd,Xn,#imm
            if word & 31 == 31:
                raise ValueError("Derived stack owner changes SP after allocation")
        elif word & 0xffc003e0 == 0xf90003e0:  # STR Xt,[SP,#uimm12*8]
            pass
        elif word & 0xffe0ffe0 == 0xaa0003e0:  # MOV Xd,Xm
            if word & 31 == 31:
                raise ValueError("Derived stack owner has an unsupported register destination")
        elif word & 0x7f800000 == 0x52800000:  # MOVZ Wd/Xd
            if word & 31 == 31:
                raise ValueError("Derived stack owner has an unsupported immediate destination")
        else:
            raise ValueError("Derived stack owner has an unproved control-flow or stack instruction")
    if words[-1] & 0xfc000000 != 0x94000000:
        raise ValueError("Derived stack template requires its complete terminal nonreturn call")
    return {"definition": copy.deepcopy(assembly[3]), "probe_call": copy.deepcopy(assembly[4]),
        "allocation": copy.deepcopy(assembly[5]), "point_ea": assembly[6]["address"],
        "point_rva": assembly[6]["rva"], "units": units, "extension": "UXTX", "shift": shift,
        "delta": delta, "entry_prefix_delta": -16, "probe_preserves": ["SP", "X15"],
        "probe_clobbers": ["X16", "X17", "NZCV"], "point_semantics": "after_allocation_instruction"}


def select_stack_points(a, raw, capture, selected_rvas, specifications):
    result = a.ROOT / ".analysis/results/native" / hashlib.sha256(raw).hexdigest()
    source = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    base_capture = evidence.identity(a, result / "capture.json")
    if source["sha256"] != hashlib.sha256(raw).hexdigest() or source["sha256"] != capture["input_sha256"]:
        raise ValueError("Derived stack review input differs from the original captured PE")
    values, used = [], set()
    for specification in specifications:
        review, reference = _review(a, specification)
        if (not isinstance(review, dict) or review.get("schema_version") != 1
                or review.get("review_kind") != "arm64_complete_probe_derived_stack_point"
                or review.get("source_input") != source or review.get("base_capture") != base_capture
                or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
                or not isinstance(review.get("reason"), str) or not review["reason"].strip()):
            raise ValueError("Derived stack review lacks exact original input and static scope")
        bodies = []
        for key in ("owner_function", "probe_function"):
            body = json.loads(evidence.checked(a, review[key], ".analysis/results/native"))
            records = [row for row in capture["functions"] if row["rva"] == body["rva"]]
            if (len(records) != 1 or records[0].get("assembly_captured") is not True
                    or records[0]["sha256"] != review[key]["sha256"]
                    or a.rel(result / records[0]["path"]) != review[key]["path"]):
                raise ValueError("Derived stack review is not bound to a complete original captured function")
            bodies.append(body)
        owner, probe = bodies
        if int(owner["rva"], 16) not in selected_rvas or owner["rva"] in used:
            raise ValueError("Derived stack owner is unselected or repeated")
        proof = arm64_probe_stack_proof(raw, owner, probe)
        if review.get("proof") != proof:
            raise ValueError("Derived stack review differs from the complete original machine proof")
        values.append({"review_source": reference, "source_input": source, "base_capture": base_capture,
            "owner_rva": owner["rva"], "owner_function": review["owner_function"],
            "probe_function": review["probe_function"], "proof": proof, "reason": review["reason"]})
        used.add(owner["rva"])
    return values


def _verify_stack_point_snapshots(body, proof, before, after):
    _verify_snapshot(body, before)
    _verify_snapshot(body, after)
    if any(before[key] != after[key] for key in ("type_present", "type", "flags", "native_frame")):
        raise ValueError("Derived automatic stack point changed a type, frame or function flag")
    point = int(proof["point_ea"], 16)
    for index, (old, new) in enumerate(zip(before["stack_points"], after["stack_points"])):
        address = int(old["ea"], 16)
        prefix = 0 if index < 2 else proof["entry_prefix_delta"]
        original_delta = proof["entry_prefix_delta"] if index == 2 else 0
        if old["spd"] != prefix or old["delta"] != original_delta:
            raise ValueError("Original SDK stack points do not have the reviewed missing allocation")
        if (new["ea"] != old["ea"] or new["spd"] != prefix + (proof["delta"] if address >= point else 0)
                or new["delta"] != original_delta + (proof["delta"] if address == point else 0)):
            raise ValueError("Automatic stack point readback differs from the exact machine derivation")


def apply_stack_point(request, base, sdk, checkpoint):
    raw = _read_identity(sdk.root, request["source_input"])
    body = json.loads(_read_identity(sdk.root, request["owner_function"]))
    probe = json.loads(_read_identity(sdk.root, request["probe_function"]))
    proof = arm64_probe_stack_proof(raw, body, probe)
    if proof != request["proof"] or int(body["address"], 16) != base + int(request["owner_rva"], 16):
        raise ValueError("Private automatic stack proof or image base changed")
    before = _snapshot(body, sdk)
    # Verify the entire old state before making even an automatic point.
    expected = copy.deepcopy(before)
    for row in expected["stack_points"]:
        ea = int(row["ea"], 16)
        row["spd"] += proof["delta"] if ea >= int(proof["point_ea"], 16) else 0
        row["delta"] += proof["delta"] if ea == int(proof["point_ea"], 16) else 0
    _verify_stack_point_snapshots(body, proof, before, expected)
    row = {**copy.deepcopy(request), "before": before, "completed": False,
        "operation": "ida_frame.add_auto_stkpnt", "base_database_modified": False, "target_execution": False,
        "manual_stack_point_api_used": False, "manual_frame_values_applied": False, "instructions_patched": False}
    checkpoint(row)
    function = sdk.funcs.get_func(int(body["address"], 16))
    row["automatic_point_added"] = bool(sdk.frame.add_auto_stkpnt(function, int(proof["point_ea"], 16), proof["delta"]))
    row["after"] = _snapshot(body, sdk)
    checkpoint(row)
    if row["automatic_point_added"] is not True:
        raise ValueError("SDK rejected the completely derived automatic stack point")
    _verify_stack_point_snapshots(body, proof, before, row["after"])
    row["completed"] = True
    checkpoint(row)
    return row


def verify_stack_points(a, requested, applied):
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("Derived automatic stack observations omitted a reviewed owner")
    for request, row in zip(requested, applied):
        body = json.loads(evidence.checked(a, request["owner_function"]))
        probe = json.loads(evidence.checked(a, request["probe_function"]))
        raw = evidence.checked(a, request["source_input"], ".analysis/inputs")
        if arm64_probe_stack_proof(raw, body, probe) != request["proof"]:
            raise ValueError("Derived automatic stack request differs from its original machine proof")
        if (any(row.get(key) != value for key, value in request.items())
                or row.get("operation") != "ida_frame.add_auto_stkpnt"
                or row.get("automatic_point_added") is not True or row.get("completed") is not True
                or any(row.get(key) is not False for key in ("base_database_modified", "target_execution",
                    "manual_stack_point_api_used", "manual_frame_values_applied", "instructions_patched"))):
            raise ValueError("Derived automatic stack operation lacks its full proof and exact SDK result")
        _verify_stack_point_snapshots(body, request["proof"], row.get("before"), row.get("after"))


def shared_tail_context_proof(raw, owner, callee):
    """Prove the exact EBP cleanup entry and its real one-word thiscall consumer."""
    from host import inspect_pe
    pe = inspect_pe(raw)
    if pe["architecture"] != "x86":
        raise ValueError("Shared-tail frame context requires an original x86 image")
    evidence.original_machine(raw, owner)
    evidence.original_machine(raw, callee)
    base = int(pe["image_base"], 16)
    if any(int(row["rva"], 16) != int(row["address"], 16) - base
           for body in (owner, callee) for row in body["assembly"]):
        raise ValueError("Shared-tail cleanup instruction RVAs differ from the original image base")
    start = int(owner["address"], 16)
    heads = [chunk for chunk in owner["chunks"] if int(chunk["start"], 16) == start]
    if len(heads) != 1 or len(owner["chunks"]) != 2:
        raise ValueError("Shared-tail cleanup must retain one complete entry and one existing tail")
    code = bytes.fromhex(heads[0]["raw_hex"])
    if len(code) != 8 or code[:2] != b"\x8d\x4d" or code[3] != 0xe9:
        raise ValueError("Shared-tail cleanup does not prove its exact parent-EBP input")
    tail_start = start + 8 + struct.unpack_from("<i", code, 4)[0]
    tails = [chunk for chunk in owner["chunks"] if int(chunk["start"], 16) == tail_start]
    if len(tails) != 1:
        raise ValueError("Shared-tail cleanup jump does not reach its complete existing tail")
    tail = tails[0]
    tail_code = bytes.fromhex(tail["raw_hex"])
    if (len(tail_code) != 13 or tail_code[:8] != b"\x0f\xb6\x01\x83\xc1\x08\x50\xe8"
            or tail_code[-1:] != b"\xc3"
            or tail_start + 12 + struct.unpack_from("<i", tail_code, 8)[0] != int(callee["address"], 16)):
        raise ValueError("Shared-tail cleanup omits the original tagged-byte, payload or direct call")
    expected = [(start, code[:3]), (start + 3, code[3:]), (tail_start, tail_code[:3]),
                (tail_start + 3, tail_code[3:6]), (tail_start + 6, tail_code[6:7]),
                (tail_start + 7, tail_code[7:12]), (tail_start + 12, tail_code[12:])]
    if [(int(row["address"], 16), bytes.fromhex(row["bytes"])) for row in owner["assembly"]] != expected:
        raise ValueError("Shared-tail cleanup instructions do not cover both exact original chunks")
    normal = [chunk for chunk in callee["chunks"] if chunk["start"] == callee["address"]]
    if len(normal) != 1:
        raise ValueError("Shared-tail cleanup callee lacks its complete normal entry")
    normal_code = bytes.fromhex(normal[0]["raw_hex"])
    reads = [row for row in callee["assembly"]
             if row["bytes"] in ("8a4508", "0fb64508")
             and int(normal[0]["start"], 16) <= int(row["address"], 16) < int(normal[0]["end"], 16)]
    if (normal_code[:3] != b"\x55\x8b\xec" or b"\x8b\xd9" not in normal_code
            or normal_code[-6:] != b"\x8b\xe5\x5d\xc2\x04\x00" or not reads):
        raise ValueError("Shared-tail cleanup callee does not prove ECX, low-byte input and RET4")
    return {"owner_rva": owner["rva"], "callee_rva": callee["rva"], "tail": copy.deepcopy(tail),
        "parent_frame_register": "ebp", "parent_value_offset": struct.unpack_from("<b", code, 2)[0],
        "payload_offset_in_value": 8, "callsite_rva": hex(tail_start + 7 - base),
        "callee_kind_reads": copy.deepcopy(reads), "callee_pops_stack_bytes": 4,
        "owner_declaration": "void __usercall recovered(void *parent_frame@<ebp>);",
        "callee_declaration": "void __thiscall recovered(void *payload, unsigned char value_kind);"}


def select_shared_tail_contexts(a, raw, capture, selected_rvas, specifications, type_recoveries):
    """Bind an existing tail's complete parent set without changing any boundary."""
    if (not isinstance(specifications, (tuple, list)) or not 1 <= len(specifications) <= 64
            or len(set(specifications)) != len(specifications) or capture.get("capture_complete") is not True
            or capture.get("input_sha256") != hashlib.sha256(raw).hexdigest()
            or capture.get("input_bytes") != len(raw)):
        raise ValueError("Shared-tail context requires bounded explicit reviews and exact original bytes")
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    records = {row["rva"]: row for row in capture["functions"]}
    source = a.resolve(capture["input"], ".analysis/inputs")
    database = a.ROOT / ".work/native" / capture["input_sha256"] / "batch" / (source.name + ".i64")
    values, seen = [], set()
    for specification in specifications:
        review, reference = _review(a, specification)
        fields = {"schema_version", "review_kind", "source_input", "base_capture", "source_database",
                  "owner_function", "callee_function", "tail", "parent_functions", "original_primary_owner_rva",
                  "proof_operation", "reason", "target_execution", "raw_artifacts_modified"}
        if (not isinstance(review, dict) or set(review) != fields or review.get("schema_version") != 1
                or review.get("review_kind") != "x86_existing_shared_tail_primary_context"
                or review.get("source_input") != evidence.identity(a, source)
                or review.get("base_capture") != evidence.identity(a, result / "capture.json")
                or review.get("source_database") != evidence.identity(a, database)
                or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
                or not isinstance(review.get("reason"), str) or not review["reason"].strip()):
            raise ValueError("Shared-tail context review changed its exact sources or supplied unreviewed state")
        evidence.checked(a, review["proof_operation"], ".analysis/results")
        bodies = [json.loads(evidence.checked(a, review[key], a.rel(result)))
                  for key in ("owner_function", "callee_function")]
        owner, callee = bodies
        proof = shared_tail_context_proof(raw, owner, callee)
        if (owner["rva"] in seen or int(owner["rva"], 16) not in selected_rvas
                or int(callee["rva"], 16) not in selected_rvas
                or review["tail"] != proof["tail"]):
            raise ValueError("Shared-tail context does not select its exact owner, callee and complete tail")
        for key, body in zip(("owner_function", "callee_function"), bodies):
            record = records.get(body["rva"])
            if (record is None or record.get("sha256") != review[key]["sha256"]
                    or a.rel(result / record["path"]) != review[key]["path"]):
                raise ValueError("Shared-tail context function is not its exact retained original")
            declarations = [row["declaration"] for row in type_recoveries if row["rva"] == body["rva"]]
            expected = proof["owner_declaration" if key == "owner_function" else "callee_declaration"]
            if declarations != [expected]:
                raise ValueError("Shared-tail context requires its machine-proven parent and thiscall effect types")
        parents, parent_bodies = [], []
        for record in capture["functions"]:
            path = a.resolve(a.rel(result / record["path"]), a.rel(result))
            body = json.loads(path.read_bytes())
            if proof["tail"] not in body.get("chunks", []):
                continue
            identity = evidence.identity(a, path)
            if identity["sha256"] != record["sha256"]:
                raise ValueError("Shared-tail context retained parent changed")
            evidence.original_machine(raw, body)
            parents.append({"owner_rva": body["rva"], "owner_function": identity})
            parent_bodies.append(body)
        parents.sort(key=lambda row: int(row["owner_rva"], 16))
        primary = review["original_primary_owner_rva"]
        if (review["parent_functions"] != parents or not 2 <= len(parents) <= 4096
                or primary == owner["rva"] or primary not in {row["owner_rva"] for row in parents}):
            raise ValueError("Shared-tail context omits a real parent or its distinct original primary")
        tail_addresses = {row["address"] for row in owner["assembly"] if row["address"] != owner["address"]}
        all_addresses = {row["address"] for body in parent_bodies for row in body["assembly"]}
        tail_rows = owner["assembly"][2:]
        for body in parent_bodies:
            actual = [row for row in body["assembly"] if int(proof["tail"]["start"], 16)
                      <= int(row["address"], 16) < int(proof["tail"]["end"], 16)]
            if actual != tail_rows:
                raise ValueError("Shared-tail context parents disagree on complete original instructions or xrefs")
        for row in tail_rows:
            for edge in row.get("xrefs_to", []):
                if edge.get("code") and (edge["from"] not in all_addresses
                        or row["address"] != proof["tail"]["start"] and edge["from"] not in tail_addresses):
                    raise ValueError("Shared-tail context has an unaccounted or external interior incoming edge")
        values.append({"owner_rva": owner["rva"], "owner_function": review["owner_function"],
            "callee_function": review["callee_function"], "original_primary_owner_rva": primary,
            "parent_functions": parents, "proof": proof, "review_source": reference,
            "source_database": review["source_database"], "assembly_evidence": review["reason"]})
        seen.add(owner["rva"])
    return values


def _tail_context_snapshot(request, sdk):
    tail = sdk.funcs.get_fchunk(int(request["proof"]["tail"]["start"], 16))
    expected = request["proof"]["tail"]
    if (tail is None or not tail.flags & sdk.funcs.FUNC_TAIL
            or (hex(tail.start_ea), hex(tail.end_ea)) != (expected["start"], expected["end"])):
        raise ValueError("Shared-tail context lost its original complete tail")
    parents = sorted(sdk.funcs.func_parent_iterator_t(tail))
    wanted = sorted(int(json.loads(_read_identity(sdk.root, row["owner_function"]))["address"], 16)
                    for row in request["parent_functions"])
    if parents != wanted or tail.refqty != len(wanted):
        raise ValueError("Shared-tail context changed its complete original parent set")
    return {"tail": copy.deepcopy(expected), "flags": int(tail.flags), "primary_owner": hex(tail.owner),
            "parents": [hex(value) for value in parents], "functions": [
                _snapshot(json.loads(_read_identity(sdk.root, row["owner_function"])), sdk)
                for row in request["parent_functions"]]}


@contextmanager
def shared_tail_context(request, base, sdk, audit):
    """Keep the reviewed owner's CPU context through capture, then restore it."""
    before = _tail_context_snapshot(request, sdk)
    original = base + int(request["original_primary_owner_rva"], 16)
    owner = base + int(request["owner_rva"], 16)
    if before["primary_owner"] != hex(original):
        raise ValueError("Shared-tail context original primary owner differs from its review")
    audit.update(request=copy.deepcopy(request), before=before, restored=False,
        manual_stack_points_modified=False, manual_frames_modified=False,
        instructions_patched=False, base_database_modified=False, target_execution=False)
    try:
        tail = sdk.funcs.get_fchunk(int(request["proof"]["tail"]["start"], 16))
        if not sdk.funcs.set_tail_owner(tail, owner):
            raise ValueError("SDK rejected the reviewed existing-tail primary owner")
        active = _tail_context_snapshot(request, sdk)
        if active["primary_owner"] != hex(owner):
            raise ValueError("SDK shared-tail primary-owner readback differs")
        audit["selected"] = active
        function = sdk.funcs.get_func(owner)
        flag = sdk.funcs.FUNC_SP_READY
        if type(flag) is not int or flag <= 0 or flag & (flag - 1):
            raise ValueError("SDK shared-tail stack-analysis readiness flag is invalid")
        previous_flags = int(function.flags)
        function.flags = previous_flags & ~flag
        if not sdk.funcs.update_func(function) or sdk.funcs.get_func(owner).flags != previous_flags & ~flag:
            raise ValueError("SDK rejected shared-tail derived stack-analysis invalidation")
        audit["analysis_invalidation"] = {"readiness_flag": flag, "previous_flags": previous_flags,
            "invalidated_flags": previous_flags & ~flag, "update_func_completed": True,
            "flag_readback_matches": True}
        sdk.funcs.reanalyze_function(sdk.funcs.get_func(owner))
        if not sdk.auto.auto_wait():
            raise ValueError("Shared-tail processor reanalysis was canceled")
        audit.update(reanalyze_function_requested=True, auto_wait_completed=True,
                     analyzed=_tail_context_snapshot(request, sdk))
        sdk.hexrays.clear_cached_cfuncs()
        yield audit
        audit["captured"] = _tail_context_snapshot(request, sdk)
    except BaseException as error:
        audit["operation_error"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        tail = sdk.funcs.get_fchunk(int(request["proof"]["tail"]["start"], 16))
        if tail is None or not sdk.funcs.set_tail_owner(tail, original):
            audit["restore_error"] = "SDK rejected original existing-tail primary restoration"
            raise ValueError(audit["restore_error"])
        sdk.hexrays.clear_cached_cfuncs()
        audit["after"] = _tail_context_snapshot(request, sdk)
        audit["restored"] = audit["after"]["primary_owner"] == hex(original)
        if not audit["restored"]:
            raise ValueError("SDK existing-tail primary restoration readback differs")


def verify_shared_tail_contexts(a, requested, applied):
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("Shared-tail context omitted an exact selected owner")
    for request, audit in zip(requested, applied):
        if (not isinstance(audit, dict) or audit.get("request") != request
                or audit.get("restored") is not True or "operation_error" in audit or "restore_error" in audit
                or any(audit.get(key) is not True for key in ("reanalyze_function_requested", "auto_wait_completed"))
                or any(audit.get(key) is not False for key in ("manual_stack_points_modified", "manual_frames_modified",
                    "instructions_patched", "base_database_modified", "target_execution"))):
            raise ValueError("Shared-tail context omitted processor completion or original-owner restoration")
        bodies = [json.loads(evidence.checked(a, row["owner_function"], ".analysis/results/native"))
                  for row in request["parent_functions"]]
        base = int(bodies[0]["address"], 16) - int(bodies[0]["rva"], 16)
        original, owner = (hex(base + int(request[key], 16))
                           for key in ("original_primary_owner_rva", "owner_rva"))
        invalidation = audit.get("analysis_invalidation", {})
        flag = invalidation.get("readiness_flag")
        if type(flag) is not int or flag != 0x400:
            raise ValueError("Shared-tail context changed the SDK stack-analysis readiness flag")
        previous = None
        for stage in ("before", "selected", "analyzed", "captured", "after"):
            observed = audit.get(stage)
            if (not isinstance(observed, dict) or observed.get("tail") != request["proof"]["tail"]
                    or observed.get("primary_owner") != (original if stage in ("before", "after") else owner)
                    or observed.get("parents") != [body["address"] for body in bodies]
                    or type(observed.get("flags")) is not int or not isinstance(observed.get("functions"), list)
                    or len(observed["functions"]) != len(bodies)):
                raise ValueError("Shared-tail context changed the complete original tail or parent set")
            for body, snapshot in zip(bodies, observed["functions"]):
                _verify_snapshot(body, snapshot)
            if previous is not None:
                if observed["flags"] != audit["before"]["flags"]:
                    raise ValueError("Shared-tail context changed existing tail flags")
                for body, before, after in zip(bodies, audit["before"]["functions"], observed["functions"]):
                    if any(before[key] != after[key] for key in ("native_frame", "type_present", "type")):
                        raise ValueError("Shared-tail context changed an original frame or reviewed type")
                    if before["flags"] & ~flag != after["flags"] & ~flag:
                        raise ValueError("Shared-tail context changed a non-readiness function flag")
                    if stage == "selected" and before != after:
                        raise ValueError("Shared-tail owner selection changed state before processor reanalysis")
                    for left, right in zip(before["stack_points"], after["stack_points"]):
                        if not int(request["proof"]["tail"]["start"], 16) <= int(left["ea"], 16) <= int(request["proof"]["tail"]["end"], 16) and left != right:
                            raise ValueError("Shared-tail context changed a stack point outside its original tail")
            previous = observed
        selected = next(row for row in audit["selected"]["functions"] if row["address"] == owner)
        if (type(flag) is not int or flag <= 0 or flag & (flag - 1)
                or invalidation.get("previous_flags") != selected["flags"]
                or invalidation.get("invalidated_flags") != selected["flags"] & ~flag
                or invalidation.get("update_func_completed") is not True or invalidation.get("flag_readback_matches") is not True):
            raise ValueError("Shared-tail context did not preserve exact readiness invalidation")

_RECURSIVE_ROOT_DECLARATION = ("unsigned __int64 __fastcall root(unsigned __int64 state, "
    "unsigned __int64 destination, unsigned __int64 source);")
_RECURSIVE_CORE_DECLARATION = ("unsigned __int128 __usercall __spoils<rax,rbx,rdi,"
    + ",".join("ymm" + str(index) for index in range(16))
    + "> recursive@<r11:rdx>(void *destination@<rdx>,const void *source@<r8>,"
    "const void *permutation@<r11>,__int64 subtree_size@<r10>,__int64 root_size@<rsi>);")


def recursive_context_proof(raw, root, core):
    """Bind the real root fallthrough and complete separate recursive machine body."""
    from host import inspect_pe
    pe = inspect_pe(raw)
    if pe["architecture"] != "x64":
        raise ValueError("Recursive fallthrough context requires original x64 machine evidence")
    for body in (root, core):
        evidence.original_machine(raw, body)
    base = int(pe["image_base"], 16)
    chunks = [*root["chunks"], *core["chunks"]]
    if (len(root["chunks"]) != 1 or len(core["chunks"]) != 1
            or root["end"] != core["address"]
            or chunks[0]["start"] != root["address"] or chunks[0]["end"] != core["address"]
            or chunks[1]["start"] != core["address"] or chunks[1]["end"] != core["end"]):
        raise ValueError("Recursive context omits a complete contiguous physical root or core")
    start, middle, stop = (int(value, 16) for value in
                            (root["address"], core["address"], core["end"]))
    code = b"".join(bytes.fromhex(chunk["raw_hex"]) for chunk in chunks)
    root_code = bytes.fromhex(root["chunks"][0]["raw_hex"])
    if (len(root_code) != 112
            or not root_code.startswith(bytes.fromhex("575653554889e54883e4e04881ecb0010000"))
            or not root_code.endswith(bytes.fromhex("4c63114c8b59084c89d66690"))):
        raise ValueError("Recursive context lacks its original aligned Win64 root frame and inputs")
    instructions = [*root["assembly"], *core["assembly"]]
    cursor, alignment = start, []
    for instruction in instructions:
        address, data = int(instruction["address"], 16), bytes.fromhex(instruction["bytes"])
        if (address < cursor or address + len(data) > stop
                or instruction["rva"] != hex(address - base)
                or data != code[address - start:address - start + len(data)]):
            raise ValueError("Recursive context instructions overlap or differ from their complete union")
        if address > cursor:
            gap = code[cursor - start:address - start]
            position = 0
            nops = (bytes.fromhex(value) for value in
                    ("0f1f840000000000", "0f1f8000000000", "0f1f440000", "0f1f4000", "0f1f00", "6690", "90"))
            nops = tuple(nops)
            while position < len(gap):
                match = next((nop for nop in nops if gap.startswith(nop, position)), None)
                if match is None:
                    raise ValueError("Recursive context has unaccounted machine bytes between instructions")
                position += len(match)
            alignment.append({"start": hex(cursor), "end": hex(address), "raw_hex": gap.hex()})
        cursor = address + len(data)
    if cursor != stop:
        raise ValueError("Recursive context omitted its complete final machine boundary")
    last = root["assembly"][-1]
    if (last["bytes"] != "6690" or int(last["address"], 16) + 2 != middle
            or not any(edge.get("from") == last["address"] and edge.get("to") == core["address"]
                       and edge.get("type") == 21 and edge.get("code") is True
                       for edge in last.get("xrefs_from", []))):
        raise ValueError("Recursive context has no original physical root fallthrough")
    calls = [row for row in core["assembly"] if bytes.fromhex(row["bytes"])[:1] == b"\xe8"]
    returns = [row for row in instructions if bytes.fromhex(row["bytes"]) == b"\xc3"]
    if len(calls) != 22 or len(returns) != 14:
        raise ValueError("Recursive context omitted an original recursive CALL or RET")
    for call in calls:
        data = bytes.fromhex(call["bytes"])
        if len(data) != 5 or int(call["address"], 16) + 5 + struct.unpack("<i", data[1:])[0] != middle:
            raise ValueError("Recursive context direct CALL targets a different physical function")
    incoming = [edge for edge in core.get("xrefs_to_entry", []) if edge.get("code")]
    expected = [(last["address"], 21)] + [(row["address"], 17) for row in calls]
    if sorted((edge["from"], edge["type"]) for edge in incoming) != sorted(expected):
        raise ValueError("Recursive core has an unaccounted original incoming entry")
    pushes = [row for row in core["assembly"] if row["bytes"] == "4152"]
    pops = [row for row in core["assembly"] if row["bytes"] == "415a"]
    if len(pushes) != 11 or len(pops) != 11:
        raise ValueError("Recursive context omitted an original R10 save or restore")
    pairs = []
    for push, pop in zip(pushes, pops):
        first, last_ea = int(push["address"], 16), int(pop["address"], 16)
        inside = [row for row in calls if first < int(row["address"], 16) < last_ea]
        if len(inside) != 2:
            raise ValueError("Recursive R10 save does not enclose both original child calls")
        pairs.append({"push_ea": push["address"], "pop_ea": pop["address"]})
    return {"original_machine_union": {"start": root["address"], "end": core["end"],
        "bytes": len(code), "sha256": hashlib.sha256(code).hexdigest(),
        "instructions": len(instructions), "retained_alignment": alignment},
        "original_fallthrough": copy.deepcopy(root["assembly"][-1]),
        "recursive_call_instructions": copy.deepcopy(calls),
        "all_original_return_instructions": copy.deepcopy(returns),
        "saved_r10_instructions": pairs, "recursive_abi_declaration": _RECURSIVE_CORE_DECLARATION,
        "root_abi_declaration": _RECURSIVE_ROOT_DECLARATION,
        "recursive_stack_argument_bytes": 0, "root_home_area_bytes": 32,
        "flag_names": ["INLINE_DONTCOPY"]}


def _recursive_receipt(a, record):
    import gzip
    identity = {key: record[key] for key in ("path", "bytes", "sha256")}
    raw = evidence.checked(a, identity)
    if record["path"].endswith(".gz"):
        raw = gzip.decompress(raw)
        if len(raw) != record.get("logical_bytes") or hashlib.sha256(raw).hexdigest() != record.get("logical_sha256"):
            raise ValueError("Recursive context compressed proof changed its complete logical bytes")
    value = json.loads(raw)
    pointer = record.get("pointer")
    if not isinstance(pointer, str) or not re.fullmatch(r"/(?:result/)?functions/[0-9]+", pointer):
        raise ValueError("Recursive context proof lacks an exact function pointer")
    for token in pointer[1:].split("/"):
        value = value[int(token)] if isinstance(value, list) else value[token]
    return value


def select_recursive_contexts(a, raw, capture, selected_rvas, specifications):
    """Select exact complete root contexts while retaining physical core ownership."""
    if (not isinstance(specifications, (tuple, list)) or not 1 <= len(specifications) <= 64
            or len(set(specifications)) != len(specifications)
            or capture.get("capture_complete") is not True
            or capture.get("input_sha256") != hashlib.sha256(raw).hexdigest()
            or capture.get("input_bytes") != len(raw)):
        raise ValueError("Recursive contexts require unique exact reviewed original captures")
    result = a.ROOT / ".analysis/results/native" / capture["input_sha256"]
    records = {row["rva"]: row for row in capture["functions"]}
    source = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    base_capture = evidence.identity(a, result / "capture.json")
    requests, seen = [], set()
    for specification in specifications:
        review, reference = _review(a, specification)
        fields = {"schema_version", "review_kind", "source_input", "base_capture", "root_function",
            "core_function", "root_rva", "core_rva", "root_end", "core_end", "native_owners",
            "original_machine_union", "original_fallthrough", "core_incoming_code_references",
            "recursive_call_instructions", "all_original_return_instructions", "recursive_abi_declaration",
            "recursive_stack_argument_bytes", "root_home_area_bytes", "sdk_inline_flag_names",
            "control_semantics", "private_sdk_capture", "all_11_r10_save_restore_pairs", "target_execution",
            "raw_artifacts_modified", "stack_point_edits", "frame_edits", "microcode_edits", "scope"}
        if (not isinstance(review, dict) or set(review) != fields or review.get("schema_version") != 1
                or review.get("review_kind") != "native_x64_recursive_fallthrough_context_complete_bytes"
                or review.get("source_input") != source or review.get("base_capture") != base_capture
                or any(review.get(key) is not False for key in ("target_execution", "raw_artifacts_modified",
                    "stack_point_edits", "frame_edits", "microcode_edits"))):
            raise ValueError("Recursive context review changed its original identity or allowed mutations")
        bodies = [json.loads(evidence.checked(a, review[key], a.rel(result)))
                  for key in ("root_function", "core_function")]
        root, core = bodies
        for key, body in zip(("root_function", "core_function"), bodies):
            record = records.get(body["rva"])
            if (record is None or record.get("assembly_captured") is not True
                    or record.get("sha256") != review[key]["sha256"]
                    or a.rel(result / record["path"]) != review[key]["path"]):
                raise ValueError("Recursive context owner differs from its complete original capture")
        proof = recursive_context_proof(raw, root, core)
        semantics = _recursive_receipt(a, review["control_semantics"])
        probe = _recursive_receipt(a, review["private_sdk_capture"])
        if (root["rva"] in seen or core["rva"] in seen
                or int(root["rva"], 16) not in selected_rvas
                or review.get("root_rva") != root["rva"] or review.get("core_rva") != core["rva"]
                or review.get("root_end") != root["end"] or review.get("core_end") != core["end"]
                or review.get("native_owners") != [{key: body[key] for key in ("rva", "address", "end", "chunks")}
                                                   for body in bodies]
                or review.get("core_incoming_code_references") != [edge for edge in core["xrefs_to_entry"] if edge.get("code")]
                or review.get("original_machine_union") != proof["original_machine_union"]
                or review.get("original_fallthrough") != proof["original_fallthrough"]
                or review.get("recursive_call_instructions") != proof["recursive_call_instructions"]
                or [item.get("instruction") for item in review.get("all_original_return_instructions", [])]
                    != proof["all_original_return_instructions"]
                or review.get("recursive_abi_declaration") != proof["recursive_abi_declaration"]
                or review.get("recursive_stack_argument_bytes") != 0
                or review.get("root_home_area_bytes") != 32
                or review.get("sdk_inline_flag_names") != proof["flag_names"]):
            raise ValueError("Recursive context review omits or changes complete original machine evidence")
        for row in (semantics, probe):
            if row.get("root") != review["root_function"] or row.get("core") != review["core_function"]:
                raise ValueError("Recursive context semantic or SDK proof names different original owners")
        domains = semantics.get("recursive_call_domains", [])
        expected_domains = [{"rva": row["rva"], "subtree_sizes": [1 << (5 + index // 2)]}
                           for index, row in enumerate(proof["recursive_call_instructions"])]
        sizes = semantics.get("internal_size_summaries", [])
        roots = semantics.get("root_cases_conditioned_on_size_contract", [])
        if (semantics.get("all_original_bytes_exact") is not True
                or semantics.get("all_22_original_call_instructions_covered") is not True
                or domains != expected_domains or [row.get("size") for row in sizes] != [1 << n for n in range(5, 16)]
                or [(row.get("size"), row.get("root_stack_mod32")) for row in roots]
                    != [(1 << n, alignment) for n in range(6, 18) for alignment in (8, 24)]
                or any(row.get("ordinary_return") is not True for row in sizes)
                or any(row.get("ordinary_return") is not False for row in roots)
                or any(row.get("sp_before_return_delta") != 0
                       or not all(row.get("preserved_expected_registers", {}).values())
                       for row in [*sizes, *roots])
                or semantics.get("recursive_gp_preserved") != ["RCX", "R8", "R9", "R10", "RSI", "RBP", "RSP before RET"]
                or semantics.get("recursive_gp_may_write") != ["RAX", "RBX", "RDI", "RDX", "R11"]):
            raise ValueError("Recursive context lacks its byte-bound complete control, return and register proof")
        decompiled = probe.get("decompilation", {})
        if (probe.get("strict_sdk_capture_passed") is not True or probe.get("observer_errors") != []
                or probe.get("observer_hook_unregistered") is not True
                or decompiled.get("returned_function") is not True or decompiled.get("failure_code") != 0
                or decompiled.get("warnings") != [] or decompiled.get("typed_locals_capture", {}).get("complete") is not True
                or probe.get("boundaries_and_native_frames_unchanged") is not True
                or probe.get("all_original_machine_bytes_unchanged") is not True):
            raise ValueError("Recursive context lacks its complete strict private SDK proof")
        requests.append({"root_rva": root["rva"], "core_rva": core["rva"],
            "root_function": review["root_function"], "core_function": review["core_function"],
            "review_source": reference, "proof": proof})
        seen.update((root["rva"], core["rva"]))
    if {int(row["root_rva"], 16) for row in requests} != set(selected_rvas):
        raise ValueError("Recursive context selection must contain precisely its reviewed roots")
    return requests


def verify_recursive_contexts(a, requested, applied):
    """Check every SDK scope and actual saved stack value without accepting cfunc alone."""
    if not isinstance(applied, list) or len(applied) != len(requested):
        raise ValueError("Recursive context omitted an exact requested root")
    for request, audit in zip(requested, applied):
        if (not isinstance(audit, dict) or audit.get("request") != request
                or any(audit.get(key) is not True for key in ("complete", "hook_registered",
                    "hook_unregistered", "abi_hook_registered", "abi_hook_unregistered",
                    "type_cache_computed_under_abi_hook"))
                or any(audit.get(key) is not False for key in ("target_execution", "manual_stack_points_modified",
                    "manual_frames_modified", "instructions_patched", "microcode_modified"))
                or audit.get("warnings") != [] or audit.get("errors") != []):
            raise ValueError("Recursive context lacks complete SDK callbacks, warning-free capture or cleanup")
        bodies = [json.loads(evidence.checked(a, request[key], ".analysis/results/native"))
                  for key in ("root_function", "core_function")]
        for stage in ("before", "after"):
            snapshots = audit.get(stage)
            if not isinstance(snapshots, list) or len(snapshots) != 2:
                raise ValueError("Recursive context omits a physical owner snapshot")
            for body, snapshot in zip(bodies, snapshots):
                _verify_snapshot(body, snapshot)
        for before, after in zip(audit["before"], audit["after"]):
            if any(before[key] != after[key] for key in ("address", "end", "chunks", "native_frame", "stack_points", "flags")):
                raise ValueError("Recursive context changed native bytes, frame, boundary or stack points")
        root, core = bodies
        inline = audit.get("sdk_inline", {})
        interval = inline.get("inserted_block_interval")
        if (inline.get("root_ea") != root["address"] or inline.get("core_ea") != core["address"]
                or inline.get("flag_names") != ["INLINE_DONTCOPY"] or inline.get("return_code") != 0
                or inline.get("edge_ea") != hex(int(core["address"], 16) - 1)
                or not isinstance(interval, list) or len(interval) != 2
                or any(type(value) is not int for value in interval) or not 0 <= interval[0] < interval[1]):
            raise ValueError("Recursive context omitted its exact shared-frame fallthrough SDK inline")
        typ = audit.get("core_type_readback", {})
        arguments = typ.get("arguments", [])
        if (typ.get("cc") != 240 or typ.get("stack_argument_bytes") != 0
                or [row.get("name") for row in arguments] != ["destination", "source", "permutation", "subtree_size", "root_size"]
                or [row.get("location", {}).get("register") for row in arguments] != [2, 8, 11, 10, 6]
                or any(row.get("location", {}).get("kind") != "register"
                       or row["location"].get("complete") is not True for row in arguments)
                or typ.get("return_location", {}).get("kind") != "register_pair"
                or typ["return_location"].get("register") != 2
                or typ["return_location"].get("second_register") != 11):
            raise ValueError("Recursive context lost its byte-proven internal register and stack ABI")
        calls = audit.get("recursive_calls", [])
        if (not isinstance(calls, list)
                or [row.get("ea") for row in calls] != [row["address"] for row in request["proof"]["recursive_call_instructions"]]
                or any(row.get("callee") != core["address"] or row.get("cc") != 240
                    or row.get("stkargs_top") != 0 or row.get("root_shadow_size") != 32
                    or row.get("return_registers") != "rdx.8,r11.8"
                    or [arg.get("location", {}).get("register") for arg in row.get("arguments", [])] != [2, 8, 11, 10, 6]
                    or any("ymm" + str(index) + ".32" not in row.get("spoiled", "") for index in range(16))
                    for row in calls)):
            raise ValueError("Recursive context omitted or changed a real recursive SDK CALL")
        pairs = audit.get("saved_r10_pairs", [])
        if (not isinstance(pairs, list) or len(pairs) != 11
                or [{"push_ea": row.get("push_ea"), "pop_ea": row.get("pop_ea")} for row in pairs]
                    != request["proof"]["saved_r10_instructions"]):
            raise ValueError("Recursive context omitted a real R10 save or restore")
        for pair in pairs:
            store, load = pair.get("store", {}), pair.get("load", {})
            if (store.get("ea") != pair["push_ea"] or load.get("ea") != pair["pop_ea"]
                    or store.get("opcode") != 4 or load.get("opcode") != 4
                    or store.get("source_register") != "r10" or load.get("destination_register") != "r10"
                    or type(store.get("stack_offset")) is not int or store.get("stack_offset") != load.get("stack_offset")
                    or store.get("size") != 8 or load.get("size") != 8):
                raise ValueError("Recursive context did not retain the actual SDK stack write and read")

