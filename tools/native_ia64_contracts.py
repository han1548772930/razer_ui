"""Verify retained IA64 manual contracts without manufacturing decompiler results."""
from __future__ import annotations

from collections import Counter
import copy
import hashlib
import json
from pathlib import Path
import re
import struct

import native_adjudication as adjudication
from host import inspect_pe, native_exception_region_coverage


def require(condition, message):
    if not condition:
        raise ValueError("IA64 static contract: " + message)


def integer(value):
    require(isinstance(value, str) and re.fullmatch(r"0x(?:0|[1-9a-f][0-9a-f]*)", value),
            "address is not a canonical hexadecimal integer")
    return int(value, 16)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def exact(actual, expected, message):
    def same(left, right):
        if type(left) is not type(right):
            return False
        if isinstance(left, dict):
            return left.keys() == right.keys() and all(same(left[key], right[key]) for key in left)
        if isinstance(left, (list, tuple)):
            return len(left) == len(right) and all(same(first, second) for first, second in zip(left, right))
        return left == right
    require(same(actual, expected), message)


class Evidence:
    def __init__(self, a):
        self.a, self.records, self.content = a, {}, {}

    def read(self, record, boundary=".analysis"):
        require(isinstance(record, dict) and set(record) == {"path", "bytes", "sha256"},
                "evidence requires one complete exact file identity")
        name = record["path"]
        if name in self.records:
            exact(record, self.records[name], "one evidence path has conflicting identities")
            return self.content[name]
        raw = adjudication.checked(self.a, record, boundary)
        self.records[name], self.content[name] = dict(record), raw
        return raw

    def json(self, record, boundary=".analysis/results"):
        return json.loads(self.read(record, boundary))

    def indexed(self, directory, row):
        path = self.a.resolve(self.a.rel(directory / row["path"]), self.a.rel(directory))
        record = adjudication.identity(self.a, path)
        require(record["sha256"] == row.get("sha256")
                and ("bytes" not in row or record["bytes"] == row["bytes"]),
                "indexed artifact differs from its retained byte identity")
        return self.json(record), record

    def recheck(self):
        for name, record in self.records.items():
            exact(adjudication.identity(self.a, self.a.resolve(name, ".work/native" if name.startswith(".work/")
                                                              else ".analysis")), record,
                  "evidence changed while checking its complete contracts")


class Image:
    """Strict file-backed PE reads; GlobalPtr remains metadata, not a raw span."""
    def __init__(self, raw):
        self.raw, self.pe = raw, inspect_pe(raw)
        require(self.pe.get("architecture") == "ia64" and self.pe.get("format") == "pe32+"
                and self.pe.get("unresolved") == [], "input is not a completely parsed IA64 PE32+ image")
        self.base = integer(self.pe["image_base"])
        coff = struct.unpack_from("<I", raw, 0x3c)[0]
        optional_size = struct.unpack_from("<H", raw, coff + 20)[0]
        optional = coff + 24
        self.entry = struct.unpack_from("<I", raw, optional + 16)[0]
        self.header_bytes = struct.unpack_from("<I", raw, optional + 60)[0]
        count = struct.unpack_from("<I", raw, optional + 108)[0]
        self.directories = [struct.unpack_from("<II", raw, optional + 112 + i * 8) for i in range(count)]
        self.sections = [{**row, "flags": struct.unpack_from("<I", raw,
                         optional + optional_size + i * 40 + 36)[0]}
                         for i, row in enumerate(self.pe["sections"])]

    def read(self, rva, size, *, executable=False):
        require(type(rva) is int and type(size) is int and rva >= 0 and size > 0,
                "invalid or empty original PE range")
        spans = [(0, self.header_bytes, 0, False)] + [
            (row["rva"], row["rva"] + row["bytes"], row["file_offset"], bool(row["flags"] & 0x20000000))
            for row in self.sections if row["bytes"]]
        found = [span for span in spans if span[0] < rva + size and rva < span[1]]
        require(len(found) == 1 and found[0][0] <= rva and rva + size <= found[0][1]
                and (not executable or found[0][3]), "range is not uniquely backed by original PE bytes")
        offset = found[0][2] + rva - found[0][0]
        value = self.raw[offset:offset + size]
        require(len(value) == size, "original PE bytes are truncated")
        return value

    def directory(self, index):
        return self.directories[index] if index < len(self.directories) else (0, 0)

    def string(self, rva):
        value = bytearray()
        for offset in range(len(self.raw)):
            byte = self.read(rva + offset, 1)[0]
            if not byte:
                return bytes(value).decode("ascii")
            value.append(byte)
        raise ValueError("IA64 static contract: unterminated PE string")


def slot_rva(begin, slot):
    require(type(begin) is int and begin >= 0 and begin % 16 == 0
            and type(slot) is int and slot >= 0, "invalid original IA64 slot coordinate")
    return begin + slot // 3 * 16 + (0, 6, 12)[slot % 3]


def instruction_rows(image, assembly, spans):
    require(isinstance(assembly, list) and assembly, "missing complete original instruction rows")
    previous, result = -1, {}
    for row in assembly:
        rva, address = integer(row["rva"]), integer(row["address"])
        code = bytes.fromhex(row["bytes"])
        require(address == image.base + rva and rva >= previous
                and (rva % 16, len(code)) in {(0, 6), (6, 6), (12, 4), (0, 12)}
                and any(first <= rva < rva + len(code) <= stop for first, stop in spans)
                and image.read(rva, len(code), executable=True) == code
                and ("size" not in row or row["size"] == len(code)),
                "instruction slot, byte sequence or containing range changed")
        require(isinstance(row.get("xrefs_from"), list) and isinstance(row.get("xrefs_to"), list),
                "instruction lost its original incoming or outgoing references")
        for field, direction in (("xrefs_from", "from"), ("xrefs_to", "to")):
            for edge in row[field]:
                require(integer(edge[direction]) == address and type(edge.get("type")) is int
                        and type(edge.get("code")) is bool, "xref belongs to another original instruction")
        result[rva] = row
        previous = rva + len(code)
    return result


def decode_unwind(raw):
    """Decode the reviewed R1/R3/P3/P5/P7/B1/B2 subset, rejecting unknown forms.

    The acquired runtime architecture guide, Appendix B, defines these fields.
    Windows header layout is checked separately as <HHI>; portable headers are
    deliberately not used to parse the retained Windows version-3 stream.
    """
    offset, slots, body, current_end, records = 0, 0, None, None, []

    def byte():
        nonlocal offset
        require(offset < len(raw), "truncated unwind descriptor")
        value = raw[offset]
        offset += 1
        return value

    def uleb():
        value, shift = 0, 0
        while True:
            b = byte()
            require(shift < 64 and (shift < 63 or b & 0x7f <= 1), "unwind ULEB128 exceeds 64 bits")
            value |= (b & 0x7f) << shift
            if not b & 0x80:
                return value
            shift += 7

    while offset < len(raw):
        start, opcode = offset, byte()
        row = {"offset": start}
        if opcode < 0x40 or opcode in (0x60, 0x61):
            body = bool(opcode & (0x20 if opcode < 0x40 else 1))
            length = opcode & 0x1f if opcode < 0x40 else uleb()
            row.update(format="R1" if opcode < 0x40 else "R3", record="body" if body else "prologue",
                       instruction_slots=length, start_slot=slots, end_slot=slots + length)
            if opcode == 0 and start > 0 and not any(raw[start:]):
                row["zero_padding_noop"] = True
            slots += length
            current_end = slots
        elif body is False and opcode & 0xf8 == 0xb0:
            extra = byte()
            kind = (opcode & 7) * 2 + (extra >> 7)
            names = {1: "rp_gr", 2: "pfs_gr", 3: "preds_gr", 5: "lc_gr"}
            require(kind in names, "unreviewed P3 unwind register form")
            row.update(format="P3", record=names[kind], register=extra & 0x7f)
        elif body is False and opcode == 0xb9:
            first, second, third = byte(), byte(), byte()
            row.update(format="P5", record="frgr_mem", grmask=first >> 4,
                       frmask=((first & 15) << 16) | second << 8 | third)
        elif body is False and opcode in (0xe0, 0xe2, 0xe4, 0xe6, 0xe8, 0xea):
            value = uleb()
            name = {0xe0: "mem_stack_f", 0xe2: "spill_base", 0xe4: "rp_when",
                    0xe6: "pfs_when", 0xe8: "preds_when", 0xea: "lc_when"}[opcode]
            row.update(format="P7", record=name)
            if opcode == 0xe2:
                row.update(pspoff=value, address_expression="psp+16-" + str(4 * value))
            else:
                row["t"] = value
                if opcode == 0xe0:
                    units = uleb()
                    row.update(stack_16byte_units=units, stack_bytes=units * 16)
        elif body is True and opcode & 0xc0 == 0x80:
            row.update(format="B1", record="copy_state" if opcode & 0x20 else "label_state", label=opcode & 31)
        elif body is True and opcode & 0xe0 == 0xc0:
            row.update(format="B2", record="epilogue", additional_pop_count=opcode & 31, t_from_region_end=uleb())
            require(row["t_from_region_end"] < current_end, "epilogue lies outside its original region")
        else:
            raise ValueError("IA64 static contract: unknown or unreviewed unwind opcode " + hex(opcode))
        row["raw_hex"] = raw[start:offset].hex()
        records.append(row)
    return records, slots


def imports(image):
    start, size = image.directory(1)
    require(bool(start) == bool(size) and size % 20 == 0, "invalid original import directory")
    result, modules, occupied = [], [], set()
    for index in range(size // 20):
        at = start + index * 20
        lookup, stamp, chain, name_rva, iat = struct.unpack("<IIIII", image.read(at, 20))
        if not any((lookup, stamp, chain, name_rva, iat)):
            require(image.read(at, start + size - at) == b"\0" * (start + size - at),
                    "nonzero import descriptors follow the terminator")
            break
        require(lookup and name_rva and iat, "unreviewed bound or incomplete import descriptor")
        module, count = image.string(name_rva), 0
        for slot in range(len(image.raw) // 8):
            value = struct.unpack("<Q", image.read(lookup + slot * 8, 8))[0]
            original_iat = struct.unpack("<Q", image.read(iat + slot * 8, 8))[0]
            require(original_iat == value, "original IAT and lookup entry differ")
            if not value:
                break
            require(not value >> 63 and iat + slot * 8 not in occupied,
                    "unreviewed ordinal import or duplicate IAT slot")
            result.append({"module": module, "name": image.string(value + 2), "iat_rva": hex(iat + slot * 8),
                           "lookup_entry_rva": hex(lookup + slot * 8), "hint_name_rva": hex(value),
                           "hint": struct.unpack("<H", image.read(value, 2))[0]})
            occupied.add(iat + slot * 8)
            count += 1
        else:
            raise ValueError("IA64 static contract: unterminated import lookup table")
        modules.append({"name": module, "descriptor_rva": hex(at), "imports": count})
    else:
        require(size == 0, "unterminated PE import descriptors")
    return result, modules


def imm64(bundle):
    require(len(bundle) == 16, "IMM64 relocation requires one complete original bundle")
    words = struct.unpack("<IIII", bundle)
    # EXT_IMM64 / EMARCH_ENC_I17_* in the acquired winnt.h definition.
    fields = ((3, 7, 4, 0), (3, 9, 18, 7), (3, 5, 13, 16), (3, 1, 12, 21),
              (1, 10, 14, 22), (1, 8, 24, 32), (2, 23, 0, 40), (3, 1, 27, 63))
    return sum(((words[word] >> position) & ((1 << size) - 1)) << value
               for word, size, position, value in fields)


def relocations(image):
    start, size = image.directory(5)
    require(bool(start) == bool(size), "incomplete relocation directory")
    cursor, rows, counts, seen = 0, [], Counter(), set()
    while cursor < size:
        page, length = struct.unpack("<II", image.read(start + cursor, 8))
        require(length >= 8 and length % 2 == 0 and cursor + length <= size and page % 4096 == 0,
                "partial or malformed relocation block")
        entries = image.read(start + cursor + 8, length - 8) if length > 8 else b""
        for (entry,) in struct.iter_unpack("<H", entries):
            kind, target = entry >> 12, page + (entry & 0xfff)
            counts[hex(kind)] += 1
            require(kind in (0, 9, 10), "unreviewed IA64 relocation kind")
            if kind == 0:
                continue
            require(target not in seen, "duplicate original relocation address")
            seen.add(target)
            value = imm64(image.read(target & ~15, 16, executable=True)) if kind == 9 else struct.unpack(
                "<Q", image.read(target, 8))[0]
            rows.append({"kind": kind, "fixup_rva": hex(target), "original_value": hex(value)})
        cursor += length
    return rows, dict(counts)


def merge_ranges(spans):
    result = []
    for begin, end in sorted(spans):
        require(type(begin) is int and type(end) is int and end > begin, "invalid coverage interval")
        if result and begin <= result[-1][1]:
            result[-1] = (result[-1][0], max(end, result[-1][1]))
        else:
            result.append((begin, end))
    return result


def uncovered(begin, end, spans):
    cursor, result = begin, []
    for first, stop in merge_ranges(spans):
        if stop <= cursor or first >= end:
            continue
        if first > cursor:
            result.append((cursor, min(first, end)))
        cursor = max(cursor, min(stop, end))
    if cursor < end:
        result.append((cursor, end))
    return result


def _original_functions(evidence, image, review, base, directory):
    rows, functions = review["functions"], base["functions"]
    rvas = [row.get("rva") for row in rows]
    require(len(rvas) == len(set(rvas)) and set(rvas) == {row["rva"] for row in functions}
            and len(functions) == len(set(row["rva"] for row in functions)),
            "manual review omitted, duplicated or invented an original function")
    result, chunks, instructions = {}, [], {}
    for row in rows:
        rva = row["rva"]
        matches = [item for item in functions if item["rva"] == rva]
        index = matches[0]
        require(index.get("path") == "functions/" + rva[2:] + ".json"
                and index.get("assembly_captured") is True and index.get("pseudocode_captured") is False,
                "original IA64 function index was relabelled as decompiled")
        body, source = evidence.indexed(directory, index)
        exact(row["evidence"], source, "manual function evidence identity differs")
        require(body.get("rva") == rva and integer(body["address"]) == image.base + integer(rva)
                and body.get("pseudocode") is None and row.get("hexrays_pseudocode_available") is False
                and row.get("provider_execution_verified") is False and body.get("name") == row.get("name")
                and isinstance(row.get("role"), str) and row["role"].strip()
                and isinstance(row.get("normal_path_contract"), str) and row["normal_path_contract"].strip(),
                "function lacks its original owner or explicit manual contract boundary")
        local, expected_chunks = [], []
        for chunk in body["chunks"]:
            begin, end = integer(chunk["start"]) - image.base, integer(chunk["end"]) - image.base
            code = image.read(begin, end - begin, executable=True)
            require(begin % 16 == end % 16 == 0 and chunk.get("raw_hex") == code.hex()
                    and chunk.get("sha256") == digest(code), "function chunk differs from original PE bytes")
            local.append((begin, end))
            chunks.append((begin, end, rva))
            expected_chunks.append({"begin_rva": hex(begin), "end_rva": hex(end), "sha256": digest(code)})
        require(len(merge_ranges(local)) == len(local) and any(begin == integer(rva) for begin, _ in local),
                "function chunks overlap or omit their retained entry")
        exact(row["chunks"], expected_chunks, "manual function omitted or changed complete original chunks")
        found = instruction_rows(image, body["assembly"], local)
        exact(row["captured_instruction_rows"], len(found), "manual function instruction count changed")
        calls = [{"rva": ins["rva"], "instruction": ins["text"],
                  "direct_target_rvas": [hex(integer(edge["to"]) - image.base) for edge in ins["xrefs_from"]
                                         if edge.get("code") is True and edge.get("type") in (16, 17, 18, 19)]}
                 for ins in body["assembly"] if "br.call" in ins["text"]
                 or re.fullmatch(r"br\.few b[0-7](?:;;)?", ins["text"]) is not None]
        exact(row.get("calls_and_import_tails"), calls, "manual review omitted a call or changed an original import tail")
        for at, ins in found.items():
            if at in instructions:
                exact(ins["bytes"], instructions[at]["bytes"], "overlapping owners disagree on original instruction bytes")
            instructions[at] = ins
        result[rva] = {"body": body, "source": source, "review": row}
    return result, chunks, instructions


def _original_regions(evidence, image, review, base, functions, chunks, instructions):
    sources = {record["path"]: record for record in review["sources"]}
    captures = [record for name, record in sources.items() if "/exception-regions/" in name and name.endswith("/capture.json")]
    require(len(captures) == 1, "manual review needs one exact original exception capture")
    cap_ref = captures[0]
    directory = evidence.a.ROOT / cap_ref["path"]
    directory = directory.parent
    cap = evidence.json(cap_ref)
    plan = evidence.json(sources[evidence.a.rel(directory / "plan.json")])
    session = evidence.json(sources[evidence.a.rel(directory / "session.json")])
    require(cap.get("capture_complete") is True and cap.get("function_chunks_unchanged") is True
            and cap.get("source_database_unchanged") is True and cap.get("base_capture_unchanged") is True
            and cap.get("functions_created") is False and cap.get("decompilation_attempted") is False
            and cap.get("function_discovery_complete") is False and cap.get("failures") == []
            and cap.get("automatic_analysis_enabled") is False
            and all(row.get("target_execution") is False and row.get("real_device_operations") is False
                    for row in (cap, plan, session)) and session.get("passed") is True
            and session.get("exit_code") == 0 and session.get("finished_at_utc"),
            "exception capture lost its static outcome, failure or ownership boundary")
    base_path = evidence.a.rel(evidence.a.ROOT / ".analysis/results/native" / digest(image.raw) / "capture.json")
    exact(cap["base_capture"], sources.get(base_path), "exception capture base identity differs")
    require(cap.get("input") == base.get("input") and cap.get("input_sha256") == digest(image.raw)
            and cap.get("image_base") == hex(image.base), "exception capture uses a different original input")
    snapshot_rows = [{"address": item["body"]["address"],
                      "chunks": [[chunk["start"], chunk["end"]] for chunk in item["body"]["chunks"]]}
                     for item in functions.values()]
    snapshot_hash = digest(json.dumps(snapshot_rows, sort_keys=True).encode())
    exact(cap["function_chunks_snapshot"], {"functions": len(functions), "chunks": len(chunks),
          "sha256_before": snapshot_hash, "sha256_after": snapshot_hash, "rows": snapshot_rows},
          "exception snapshot changed original function ownership or chunk boundaries")
    exception = image.pe["exception_directory"]
    exact(cap["exception_regions"]["directory"], exception, "original exception table or range bytes changed")
    coverage = native_exception_region_coverage(exception, [row["body"] for row in functions.values()], hex(image.base))
    exact(cap["exception_regions"]["captured_chunk_coverage"], coverage, "original chunk ownership coverage changed")
    for record in (cap["source_database"], review["cleanup"]["source_database"]):
        evidence.read(record, ".work/native")
    exact(cap["source_database"], review["cleanup"]["source_database"], "review source database differs from capture")
    for name in ("base_capture", "source_database", "exception_regions", "tool_sources"):
        exact(cap[name], plan[name], "exception plan differs from its actual capture")
    exact(cap["tool_sources"], session["tool_sources"], "exception session uses different retained tools")

    def tool(value):
        if isinstance(value, dict) and "snapshot" in value:
            record = value["snapshot"]
            require(record.get("bytes") == value.get("bytes") and record.get("sha256") == value.get("sha256"),
                    "analyzer source snapshot identity differs")
            evidence.read(record)
        elif isinstance(value, dict):
            for item in value.values():
                tool(item)
    tool(cap["tool_sources"])
    rows = review["exception_region_ownership"]
    require(len(rows) == len(exception["regions"]) == len(cap["regions"])
            and len({row["begin_rva"] for row in rows}) == len(rows), "exception review omitted or duplicated original ranges")
    regions, decoded, alignment, outside, no_entry = {}, 0, 0, [], []
    for table, reviewed, index in zip(exception["regions"], rows, cap["regions"]):
        begin, end = integer(table["begin_rva"]), integer(table["end_rva"])
        require(index.get("path") == "regions/" + table["begin_rva"][2:] + ".json", "exception index path differs")
        region, identity = evidence.indexed(directory, index)
        exact(reviewed["evidence"], identity, "manual exception range evidence differs")
        for key, value in table.items():
            exact(region.get(key), value, "exception record no longer matches original table field " + key)
        code = image.read(begin, end - begin, executable=True)
        exact(region["raw_hex"], code.hex(), "exception full raw body differs from original PE")
        found = instruction_rows(image, region["assembly"], [(begin, end)])
        spans = [(at, at + len(bytes.fromhex(row["bytes"]))) for at, row in found.items()]
        align_bytes = 0
        for item in region["alignment_items"]:
            first, stop = integer(item["start_rva"]), integer(item["end_rva"])
            original = image.read(first, stop - first, executable=True)
            require(begin <= first < stop <= end and first % 16 == stop % 16 == 0
                    and item.get("bytes") == stop - first and item.get("raw_hex") == original.hex()
                    and item.get("sha256") == digest(original)
                    and item.get("classification") == "existing_idb_alignment_item"
                    and item.get("instruction_semantics_inferred") is False,
                    "alignment bytes were omitted, changed or relabelled as decoded instructions")
            spans.append((first, stop))
            align_bytes += stop - first
        require(not region["undecoded_ranges"] and region.get("undecoded_bytes") == 0
                and not uncovered(begin, end, spans) and sum(stop - first for first, stop in spans) == end - begin,
                "exception body has an overlap or unaccounted original byte")
        count = sum(len(bytes.fromhex(row["bytes"])) for row in found.values())
        for key, value in {"decoded_bytes": count, "alignment_bytes": align_bytes, "undecoded_bytes": 0,
                           "decode_complete": align_bytes == 0, "coverage_complete": True}.items():
            exact(region.get(key), value, "exception decode/alignment completion state changed")
            exact(index.get(key), value, "exception manifest changed original coverage status")
        owners = [{"function_rva": rva, "begin_rva": hex(max(begin, first)), "end_rva": hex(min(end, stop))}
                  for first, stop, rva in chunks if first < end and begin < stop]
        unowned = [at for at in found if not any(first <= at < stop for first, stop, _ in chunks)]
        expected = {"begin_rva": hex(begin), "end_rva": hex(end), "table_index": table["index"],
            "existing_chunk_owners": owners, "unowned_instruction_rows": len(unowned), "decoded_bytes": count,
            "alignment_bytes": align_bytes, "undecoded_bytes": 0, "coverage_complete": True,
            "unwind_rva": table["unwind_rva"], "unwind_header_hex": table["unwind_header_hex"],
            "original_capture_unwind_semantics_verified": False, "static_unwind_descriptor_semantics_verified": True}
        for key, value in expected.items():
            exact(reviewed.get(key), value, "manual exception ownership or coverage differs: " + key)
        for at, ins in found.items():
            if at in instructions:
                exact(ins["bytes"], instructions[at]["bytes"], "original and exception instruction bytes disagree")
            else:
                instructions[at] = ins
        for first, stop in uncovered(begin, end, [(first, stop) for first, stop, _ in chunks]):
            instruction_count = sum(first <= at < stop for at in found)
            if instruction_count:
                outside.append((begin, first, stop, instruction_count, identity))
        decoded += count
        alignment += align_bytes
        regions[hex(begin)] = region
    exact(cap["instruction_decode_complete"], alignment == 0, "aggregate instruction decode state changed")
    reviewed_outside = review["regions_outside_existing_chunks"]
    exact([(integer(row["region_begin_rva"]), integer(row["begin_rva"]), integer(row["end_rva"]),
            row["instruction_rows"], row["evidence"]) for row in reviewed_outside], outside,
          "manual review lost additional original code outside existing function chunks")
    require(all(row.get("no_function_ownership_changed") is True and isinstance(row.get("classification"), str)
                and row["classification"] for row in reviewed_outside), "unowned code lost its explicit classification")
    no_entry = [rva for rva in functions if rva not in regions]
    return regions, {"decoded_exception_bytes": decoded, "retained_original_alignment_bytes": alignment,
                     "no_independent_exception_entry": no_entry}, cap_ref


def instruction_text(row):
    return row["text"] if "text" in row else row["idb_disassembly"]


def _unwind(evidence, image, review, regions, instructions):
    manual = review["unwind_review"]
    evidence.read(manual["portable_descriptor_spec"], ".analysis/inputs")
    require(manual.get("unknown_opcode_count") == 0 and manual.get("static_descriptor_semantics_complete") is True
            and manual.get("matching_windows_ia64_unwind_execution_verified") is False,
            "unwind review invented opcode coverage or matching-platform execution")
    grouped = {}
    for begin, region in regions.items():
        grouped.setdefault(region["unwind_rva"], []).append(begin)
    streams = manual["unique_streams"]
    exact([row["unwind_rva"] for row in streams], list(grouped), "unwind review omitted or duplicated original streams")
    counts, parsed, frames, saved, epilogues = Counter(), {}, {}, [], []
    for stream in streams:
        at = integer(stream["unwind_rva"])
        header = image.read(at, 8)
        version, flags, doublewords = struct.unpack("<HHI", header)
        require(version == 3 and flags in (0, 3) and doublewords > 0,
                "unreviewed Windows unwind header version or flags")
        raw = image.read(at + 8, doublewords * 8)
        records, slots = decode_unwind(raw)
        expected = {"header_raw_hex": header.hex(), "version": version, "flags": flags,
                    "descriptor_doublewords": doublewords, "descriptor_bytes": len(raw),
                    "descriptor_sha256": digest(raw), "region_begins": grouped[stream["unwind_rva"]],
                    "records": records, "interpreted_slots": slots, "all_opcodes_decoded": True}
        for key, value in expected.items():
            exact(stream.get(key), value, "unwind stream original bytes or decoded field changed: " + key)
        counts.update(row["format"] for row in records)
        parsed[stream["unwind_rva"]] = records
        for begin_name in stream["region_begins"]:
            begin, region = integer(begin_name), regions[begin_name]
            exact(slots, (integer(region["end_rva"]) - begin) // 16 * 3,
                  "unwind slots do not cover their complete original runtime range")
            groups = []
            for record in records:
                if record["format"] in ("R1", "R3"):
                    groups.append((record, []))
                else:
                    require(bool(groups), "unwind descriptor precedes a region header")
                    groups[-1][1].append(record)
            first_records = groups[0][1]
            frame = {}
            for record in first_records:
                if record["record"] in ("pfs_gr", "rp_gr", "preds_gr", "lc_gr"):
                    frame[record["record"][:-3]] = {"general_register": record["register"]}
                elif record["record"] == "mem_stack_f":
                    frame["memory_stack_bytes"] = record["stack_bytes"]
            frames[begin_name] = frame
            for region_header, descriptors in groups:
                for record in descriptors:
                    if "t" in record:
                        if region_header["record"] == "prologue" and region_header["instruction_slots"] == 0:
                            # Recovery fragments describe an already established
                            # parent frame; they execute no new save/allocation.
                            require(region_header["start_slot"] == 0,
                                    "zero-length recovery prologue is not the original entry state")
                            continue
                        require(region_header["record"] == "prologue"
                                and record["t"] < region_header["instruction_slots"],
                                "save action time lies outside its original prologue")
                        rva = slot_rva(begin, region_header["start_slot"] + record["t"])
                        ins = instructions.get(rva)
                        require(ins is not None, "unwind save action has no original instruction slot")
                        text = instruction_text(ins)
                        saved.append({"region_begin_rva": begin_name, "descriptor_offset": record["offset"],
                                      "record": record["record"], "instruction_rva": hex(rva), "instruction": text})
                        normalized = text.split("//", 1)[0].replace(";;", "").strip()
                        if record["record"] == "mem_stack_f":
                            require(normalized == "adds sp = -" + str(record["stack_bytes"]) + ", sp",
                                    "fixed frame descriptor differs from original SP allocation")
                        else:
                            category = record["record"][:-5]
                            registers = [item["register"] for item in descriptors if item["record"] == category + "_gr"]
                            require(len(registers) == 1, "save action has no unique original register descriptor")
                            register = str(registers[0])
                            patterns = {"pfs": r"alloc r" + register + r" = ar\.pfs,.*",
                                        "rp": r"mov r" + register + r" = b0",
                                        "preds": r"mov r" + register + r" = pr",
                                        "lc": r"mov(?:\.i)? r" + register + r" = ar\.lc"}
                            require(re.fullmatch(patterns[category], normalized) is not None,
                                    "unwind register save differs from original instruction")
                    elif record["record"] == "epilogue":
                        require(record["t_from_region_end"] < region_header["instruction_slots"],
                                "epilogue time lies outside its body")
                        rva = slot_rva(begin, region_header["end_slot"] - 1 - record["t_from_region_end"])
                        ins = instructions.get(rva)
                        require(ins is not None and re.search(r"(?:adds|mov) sp = ", instruction_text(ins)),
                                "epilogue has no original stack restoration instruction")
                        epilogues.append({"region_begin_rva": begin_name, "descriptor_offset": record["offset"],
                                          "instruction_rva": hex(rva), "instruction": instruction_text(ins),
                                          "popped_prologues": record["additional_pop_count"] + 1})
    exact(dict(counts), manual["opcode_counts_unique_streams"], "unwind opcode counts changed")
    # Checks are ordered by runtime range in the manual review, not by shared stream.
    order = {begin: i for i, begin in enumerate(regions)}
    saved.sort(key=lambda row: (order[row["region_begin_rva"]], row["descriptor_offset"]))
    epilogues.sort(key=lambda row: (order[row["region_begin_rva"]], row["descriptor_offset"]))
    exact(saved, manual["saved_action_instruction_checks"], "manual review omitted or changed an original save action")
    exact(epilogues, manual["epilogue_sp_instruction_checks"], "manual review omitted or changed an original epilogue")
    inherited = []
    for begin_name, region in regions.items():
        records = parsed[region["unwind_rva"]]
        if records[0]["instruction_slots"] != 0:
            continue
        owners = [row["function_rva"] for row in review["exception_region_ownership"]
                  if row["begin_rva"] == begin_name for row in row["existing_chunk_owners"]]
        require(len(set(owners)) == 1 and owners[0] in frames, "recovery range lacks one original owning frame")
        exact(frames[begin_name], frames[owners[0]], "recovery descriptor differs from its original parent frame")
        inherited.append({"region_begin_rva": begin_name, "owning_function_rva": owners[0],
                          "inherited_machine_frame": frames[begin_name], "matches_owning_function_frame": True})
    exact(inherited, manual["inherited_recovery_frames"], "manual review lost an inherited recovery frame")
    for row in manual["floating_spill_instruction_checks"]:
        ins = instructions.get(integer(row["rva"]))
        require(ins is not None and instruction_text(ins) == row["instruction"], "floating spill check differs from original code")
    cookies = []
    descriptors = {row["descriptor_rva"]: row for row in review["descriptors"]}
    for stream in streams:
        if stream["flags"] != 3:
            continue
        at = integer(stream["unwind_rva"]) + 8 + stream["descriptor_bytes"]
        handler, encoded = struct.unpack("<II", image.read(at, 8))
        descriptor = descriptors.get(hex(handler))
        require(descriptor is not None, "cookie handler has no reviewed original function descriptor")
        for begin in stream["region_begins"]:
            cookies.append({"function_rva": begin, "unwind_rva": stream["unwind_rva"],
                "handler_field_rva": hex(at), "handler_descriptor_rva": hex(handler),
                "handler_code_rva": descriptor["code_rva"], "cookie_metadata_rva": hex(at + 4),
                "encoded_cookie_offset": encoded, "masked_frame_offset": encoded & ~7,
                "first_prologue_instruction_slots": stream["records"][0]["instruction_slots"],
                "cookie_checked_after_prologue": True})
    exact(cookies, manual["cookie_handlers"], "cookie metadata differs from complete original handler data")
    for key, value in {"runtime_ranges": len(regions), "unique_stream_count": len(streams),
                       "all_range_slot_lengths_match": True}.items():
        exact(manual.get(key), value, "manual unwind summary changed")
    return {"unwind_range_slot_checks": len(regions), "unwind_streams": len(streams),
            "unwind_save_instruction_checks": len(saved), "unwind_epilogue_checks": len(epilogues),
            "inherited_recovery_frame_checks": len(inherited), "unknown_unwind_opcodes": 0}


def _pe_contracts(evidence, image, review, instructions, chunks, regions):
    architecture, manual = review["architecture"], review["independent_pe_review"]
    gp, gp_size = image.directory(8)
    require(gp_size == 0 and gp != 0, "original GlobalPtr metadata is invalid")
    expected_architecture = {"machine": "0x200", "format": "PE32+", "image_base": hex(image.base),
                             "gp": hex(image.base + gp), "entrypoint_descriptor_rva": hex(image.entry)}
    for key, value in expected_architecture.items():
        exact(architecture.get(key), value, "architecture or GlobalPtr metadata differs: " + key)
    evidence.read(architecture["instruction_semantics_reference"], ".analysis/inputs")
    descriptor_rvas = []
    for row in review["descriptors"]:
        at = integer(row["descriptor_rva"])
        require(at not in descriptor_rvas, "duplicate original function descriptor")
        descriptor_rvas.append(at)
        raw = image.read(at, 16)
        code, actual_gp = struct.unpack("<QQ", raw)
        require(row["raw_hex"] == raw.hex() and row["code_rva"] == hex(code - image.base)
                and row["gp"] == hex(actual_gp) == architecture["gp"]
                and code - image.base in instructions, "function descriptor does not identify original code and GP")
    discovered_descriptors = []
    for section in image.sections:
        if section["flags"] & 0x20000000:
            continue
        for at in range((section["rva"] + 7) & ~7, section["rva"] + section["bytes"] - 15, 8):
            code, actual_gp = struct.unpack("<QQ", image.read(at, 16))
            if actual_gp == image.base + gp and code - image.base in instructions:
                discovered_descriptors.append(at)
    exact(descriptor_rvas, discovered_descriptors,
          "manual review omitted or invented a file-backed code/GP function descriptor")
    require(image.entry in descriptor_rvas, "review omitted the actual entrypoint descriptor")
    entry_code = struct.unpack("<Q", image.read(image.entry, 8))[0] - image.base
    exact(architecture["entry_code_rva"], hex(entry_code), "entry descriptor code differs")
    actual_imports, modules = imports(image)
    exact(manual["imports"], actual_imports, "manual PE import inventory omitted or changed a slot")
    exact(manual["import_modules"], modules, "manual PE import module inventory differs")
    actual_relocations, kinds = relocations(image)
    exact(manual["all_relocations"], actual_relocations, "manual relocation inventory omitted or changed a fixup")
    exact(manual["relocation_kind_counts"], kinds, "relocation kind counts differ")
    evidence.read(manual["relocation_decode_reference"], ".analysis/inputs")
    import_sources = [row for row in review["sources"] if row["path"].endswith("/imports.json")]
    require(len(import_sources) == 1, "review needs one original imports dataset")
    imported = evidence.json(import_sources[0])
    exact([(row["rva"], row["name"], row["module"].lower()) for row in imported],
          [(row["iat_rva"], row["name"], Path(row["module"]).stem.lower()) for row in actual_imports],
          "retained import names or IAT identities differ from the original PE")
    tables = review["fixed_tables"]
    require(all(len(rows) == len({row["row_rva"] for row in rows}) for rows in tables.values()),
            "fixed-table review duplicated an original row")
    for row in tables["access_rights"]:
        raw = image.read(integer(row["row_rva"]), 16)
        pointer, length, mask = struct.unpack("<QII", raw)
        token = image.read(pointer - image.base, (length + 1) * 2).decode("utf-16le")
        require(row["raw_hex"] == raw.hex() and row["utf16_units"] == length
                and token == row["token"] + "\0" and row["mask"] == hex(mask),
                "access-right fixed table differs from original pointer, token or mask")
    for row in tables["sid_aliases"]:
        raw = image.read(integer(row["row_rva"]), 24)
        field, flag = struct.unpack_from("<QI", raw)
        units = struct.unpack_from("<I", raw, 20)[0]
        token = raw[12:12 + units * 2].decode("utf-16le")
        require(row["raw_hex"] == raw.hex() and row["utf16_units"] == units and units <= 3
                and row["token"] == token and row["se_exports_field_offset"] == hex(field)
                and type(row["requires_wdm_1_20"]) is bool and row["requires_wdm_1_20"] == bool(flag)
                and flag in (0, 1), "SID fixed table differs from original fields")
    metadata = []
    for row in manual["literal_ranges"]:
        begin, length = integer(row["begin_rva"]), row["bytes"]
        code = image.read(begin, length)
        exact(code, (row["text"] + "\0").encode("utf-16le"), "original UTF16 literal changed")
        exact(row["sha256"], digest(code), "literal identity differs")
        metadata.append((begin, begin + length))
    start, size = image.directory(1)
    if size:
        metadata.append((start, start + size))
    for module in modules:
        at = integer(module["descriptor_rva"])
        lookup, _, _, name, iat = struct.unpack("<IIIII", image.read(at, 20))
        metadata.extend(((name, name + len(module["name"]) + 1),
                         (lookup, lookup + (module["imports"] + 1) * 8), (iat, iat + (module["imports"] + 1) * 8)))
    for row in actual_imports:
        at = integer(row["hint_name_rva"])
        metadata.append((at, at + 2 + len(row["name"]) + 1))
    coverage = [(begin, end) for begin, end, _ in chunks] + [
        (integer(row["begin_rva"]), integer(row["end_rva"])) for row in regions.values()]
    outside = []
    for section in image.sections:
        if not section["flags"] & 0x20000000:
            continue
        for begin, end in uncovered(section["rva"], section["rva"] + section["virtual_bytes"], coverage):
            raw = image.read(begin, end - begin, executable=True)
            nonzero = sum(byte != 0 for byte in raw)
            unclassified = sum(byte != 0 and not any(first <= begin + offset < stop for first, stop in metadata)
                               for offset, byte in enumerate(raw))
            require(unclassified == 0, "original executable virtual bytes lack reviewed code, literal or import ownership")
            outside.append({"section": section["name"], "begin_rva": hex(begin), "end_rva": hex(end),
                "bytes": len(raw), "nonzero_bytes": nonzero, "sha256": digest(raw),
                "classification": "referenced_UTF16_literals_and_PE_import_metadata" if nonzero else "zero_fill",
                "unclassified_nonzero_bytes": unclassified})
    exact(outside, manual["executable_section_ranges_outside_captures"], "manual executable-byte accounting differs")
    require(manual.get("all_executable_virtual_bytes_accounted_for") is True
            and manual.get("unclassified_nonzero_bytes") == 0, "manual executable coverage is not complete")
    return {"descriptors_verified": len(descriptor_rvas), "fixed_access_rows_verified": len(tables["access_rights"]),
            "fixed_sid_rows_verified": len(tables["sid_aliases"]), "independent_imports_verified": len(actual_imports),
            "independent_relocations_verified": len(actual_relocations)}, actual_relocations


def _copy_body(evidence, image, review, functions, regions, instructions, fixes):
    body = review["additional_forward_copy_contract"]
    begin, end = integer(body["begin_rva"]), integer(body["end_rva"])
    region = evidence.json(body["evidence"])
    require(region is regions.get(region["begin_rva"]) or region == regions.get(region["begin_rva"]),
            "forward-copy evidence is not one verified original region")
    require(begin < end and hex(begin) not in functions and body.get("normal_entry_bypasses") is True
            and isinstance(body.get("contract"), str) and body["contract"].strip(),
            "forward-copy body lost its distinct manual contract")
    rows = [row for row in region["assembly"] if begin <= integer(row["rva"]) < end]
    require(rows and not uncovered(begin, end, [(integer(row["rva"]), integer(row["rva"]) + row["size"]) for row in rows]),
            "forward-copy body omitted original instructions")
    branch = instructions.get(integer(body["entry_branch_rva"]))
    require(branch is not None and any(edge.get("type") == 19 and edge.get("code") is True
                and integer(edge["to"]) == image.base + integer(body["entry_branch_target"])
                for edge in branch["xrefs_from"]) and not instruction_text(branch).lstrip().startswith("("),
            "normal copy entry does not retain its original unconditional bypass")
    external = [edge for row in instructions.values() for edge in row["xrefs_from"]
                if edge.get("code") is True and integer(edge["to"]) == image.base + begin
                and not begin <= integer(edge["from"]) - image.base < end]
    require(not external and not any(integer(row["original_value"]) == image.base + begin for row in fixes)
            and image.directory(0) == (0, 0) and image.directory(9) == (0, 0)
            and struct.pack("<Q", image.base + begin) not in image.raw,
            "reviewed forward-copy entry boundary has an unaccounted original entry source")
    boundary = {"entry_rva": hex(begin), "range_end_rva": hex(end),
                "unique_captured_instruction_addresses_scanned": len(instructions),
                "external_inbound_captured_xrefs": external,
                "all36_imm64_and22_dir64_relocations_checked": True,
                "pe_export_directory_absent": True, "pe_tls_directory_absent": True,
                "literal_pointer_to_entry_present": False}
    for actual in (body["entry_boundary"], review["independent_pe_review"]["forward_copy_entry_boundary"]):
        for key, value in boundary.items():
            exact(actual.get(key), value, "forward-copy entry audit differs from original references")
    return {"evidence": body["evidence"], "begin_rva": hex(begin), "end_rva": hex(end),
            "instruction_rows": len(rows), "manual_contract": body["contract"], "independent_callable_entry": False}


def verify(a, review_record):
    """Recheck a complete manual review and retain all independent failure dimensions."""
    evidence = Evidence(a)
    review = evidence.json(review_record)
    require(review.get("schema_version") == 1 and review.get("kind") == "ia64_asrdrv_full_static_function_contract_review",
            "unknown manual contract review schema")
    scope = review["scope"]
    require(all(scope.get(name) is False for name in ("source_or_idb_mutated", "target_execution", "real_device_operations",
                                                    "semantics_complete"))
            and all(scope.get(name) is True for name in ("static_function_contracts_complete",
                "static_unwind_descriptor_semantics_complete", "all_executable_virtual_bytes_accounted_for")),
            "manual static contracts must remain separate from execution and full semantics")
    raw = evidence.read(review["input"], ".analysis/inputs")
    image = Image(raw)
    container = review["container"]
    parent = evidence.read({key: container[key] for key in ("path", "bytes", "sha256")}, ".analysis/inputs")
    offset, length = container["embedded_offset"], container["embedded_bytes"]
    require(type(offset) is int and offset >= 0 and type(length) is int and length == len(raw)
            and parent[offset:offset + length] == raw and container.get("embedded_bytes_independently_matched") is True,
            "embedded IA64 original is not the exact reviewed parent slice")
    for record in review["sources"]:
        evidence.read(record)
    base_directory = a.ROOT / ".analysis/results/native" / digest(raw)
    sources = [record for record in review["sources"] if record["path"] == a.rel(base_directory / "capture.json")]
    require(len(sources) == 1, "manual review omitted its canonical original capture")
    base_ref, base = sources[0], evidence.json(sources[0])
    require(base.get("input") == review["input"]["path"] and base.get("input_sha256") == digest(raw)
            and base.get("input_bytes") == len(raw) and base.get("image_base") == hex(image.base)
            and base.get("capture_complete") is True and base.get("hexrays_available") is False
            and base.get("all_pseudocode_captured") is False and base.get("target_execution") is False
            and base.get("real_device_operations") is False, "base capture outcome or original identity changed")
    functions, chunks, instructions = _original_functions(evidence, image, review, base, base_directory)
    expected_failures = {rva[2:] + ":hexrays" for rva in functions}
    require(len(base["failures"]) == len(expected_failures)
            and {row["stage"] for row in base["failures"]} == expected_failures,
            "original IA64 decompiler failures were removed or changed in scope")
    regions, metrics, exception_ref = _original_regions(evidence, image, review, base, functions, chunks, instructions)
    metrics.update(_unwind(evidence, image, review, regions, instructions))
    pe_metrics, fixes = _pe_contracts(evidence, image, review, instructions, chunks, regions)
    metrics.update(pe_metrics)
    extra = _copy_body(evidence, image, review, functions, regions, instructions, fixes)
    for key, value in {"existing_functions": len(functions), "existing_chunks": len(chunks),
                       "function_contracts_reviewed": len(functions), "exception_ranges": len(regions),
                       "additional_forward_copy_body": True}.items():
        exact(scope.get(key), value, "declared manual scope differs from the complete original inventory")
    validation = review["validation"]
    require(validation.get("failures") == [] and validation.get("matching_windows_ia64_execution_verified") is False
            and validation.get("exception_capture_instruction_decode_complete") is False,
            "review dropped failures or promoted original alignment/platform boundaries")
    metrics.update(function_chunks=len(chunks), function_instruction_rows_verified=sum(
        len(row["body"]["assembly"]) for row in functions.values()))
    for key, value in metrics.items():
        if key not in ("unwind_streams", "independent_relocations_verified"):
            exact(validation.get(key), value, "manual validation count or original decode state differs: " + key)
    evidence.recheck()
    common = {"coverage_kind": "reviewed_ia64_static_contract", "semantic_source": "manual_review",
              "source_input": review["input"], "base_capture": base_ref, "review": review_record,
              "hexrays_pseudocode_available": False, "provider_execution_verified": False,
              "rust_implementation_verified": False, "ui_backend_wiring_verified": False,
              "runtime_acceptance_verified": False, "target_execution": False}
    return {"schema_version": 1, **common, "complete_static_contracts": {
        rva: {**common, "rva": rva, "function": item["source"], "role": item["review"]["role"],
              "manual_contract": item["review"]["normal_path_contract"]} for rva, item in functions.items()},
        "exception_capture": exception_ref, "validated_original_artifacts": list(evidence.records.values()),
        "validation": metrics, "additional_forward_copy_contract": extra,
        "original_decompiler_failures": copy.deepcopy(base["failures"]),
        "unverified_boundaries": copy.deepcopy(review["unverified_boundaries"]),
        "function_discovery_complete": False, "semantic_completion": False,
        "official_dll_comparison": "unverified", "all_original_artifacts_unchanged": True}
