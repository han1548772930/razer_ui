"""Acquire official host evidence and statically inspect containers; never launch targets."""
from __future__ import annotations

import base64
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import struct
import sys
import threading
import types
import urllib.parse
import urllib.request
import io
import lzma
import zipfile
import zlib


OFFICIAL = {"www.razer.com", "rzr.to", "dl.razerzone.com", "apps.razer.com", "app-assets.razer.com",
            "discovery3.razerapi.com", "manifest3.razerapi.com", "manifest-assets.razersynapse.com"}
STABLE_DOWNLOAD_PAGE = "https://www.razer.com/synapse-4"
STABLE_BOOTSTRAP = "https://rzr.to/synapse-4-pc-download"

IA64_EXCEPTION_SDK_MODULES = (
    "ida_auto", "ida_bytes", "ida_diskio", "ida_funcs", "ida_ida", "ida_idp",
    "ida_kernwin", "ida_lines", "ida_loader", "ida_nalt", "ida_ua",
    "idaapi", "idautils", "idc",
)
IA64_EXCEPTION_ANALYZER_FILES = {
    "kernel": "ida.dll", "processor": "procs/ia.dll", "python_plugin": "plugins/idapython3.dll",
}


def inspect_pe(raw):
    """Read PE metadata as bytes; never load the image or resolve imports in the OS."""
    if raw[:2] != b"MZ":
        return {"format": "elf" if raw[:4] == b"\x7fELF" else "non_pe", "kind": "other"}

    def read(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(raw):
            raise ValueError("PE structure exceeds input bytes")
        return struct.unpack_from(fmt, raw, offset)

    pe, = read("<I", 0x3c)
    if raw[pe:pe + 4] != b"PE\0\0":
        raise ValueError("MZ image has no valid PE signature")
    machine, section_count, timestamp, _, _, optional_size, characteristics = read("<HHIIIHH", pe + 4)
    optional = pe + 24
    magic, = read("<H", optional)
    if magic not in (0x10b, 0x20b):
        raise ValueError("Unsupported PE optional header magic: " + hex(magic))
    directory_offset = 112 if magic == 0x20b else 96
    if optional_size < directory_offset:
        raise ValueError("Truncated PE optional header")
    pointer_size = 8 if magic == 0x20b else 4
    image_base, = read("<Q" if pointer_size == 8 else "<I", optional + (24 if pointer_size == 8 else 28))
    directory_count, = read("<I", optional + directory_offset - 4)
    if directory_count * 8 > optional_size - directory_offset:
        raise ValueError("PE directories exceed optional header")
    directories = [read("<II", optional + directory_offset + i * 8) for i in range(directory_count)]
    sections = []
    for i in range(section_count):
        offset = optional + optional_size + i * 40
        name, virtual_size, rva, size, file_offset = read("<8sIIII", offset)
        if size and file_offset + size > len(raw):
            raise ValueError("PE section exceeds input bytes")
        sections.append({"name": name.rstrip(b"\0").decode("ascii", errors="replace"), "rva": rva,
                         "virtual_bytes": virtual_size, "file_offset": file_offset, "bytes": size})
    header_size, = read("<I", optional + 60)

    def at_rva(rva, length=1):
        if 0 <= rva < min(header_size, len(raw)) and rva + length <= min(header_size, len(raw)):
            return rva
        for section in sections:
            delta = rva - section["rva"]
            if 0 <= delta and delta + length <= section["bytes"]:
                return section["file_offset"] + delta
        raise ValueError("PE RVA is not backed by input bytes: " + hex(rva))

    def directory(index):
        return directories[index] if index < len(directories) else (0, 0)

    def string(rva):
        offset = at_rva(rva)
        end = raw.find(b"\0", offset)
        if end < 0:
            raise ValueError("PE ASCII string is unterminated")
        at_rva(rva, end - offset + 1)
        return raw[offset:end].decode("ascii", errors="replace")

    def thunks(rva):
        symbols = []
        mask = 1 << (pointer_size * 8 - 1)
        for i in range(len(raw) // pointer_size):
            value, = read("<Q" if pointer_size == 8 else "<I", at_rva(rva + i * pointer_size, pointer_size))
            if not value:
                return symbols
            symbols.append({"ordinal": value & 0xffff} if value & mask else {"name": string(value + 2)})
        raise ValueError("Unterminated PE import thunk table")

    result = {"format": "pe32+" if pointer_size == 8 else "pe32", "kind": "native",
              "machine": hex(machine), "architecture": {0x14c: "x86", 0x200: "ia64", 0x8664: "x64", 0xaa64: "arm64"}.get(machine, "other"),
              "image_base": hex(image_base), "coff_timestamp": timestamp, "characteristics": hex(characteristics),
              "sections": sections, "imports": [], "delay_imports": [], "exports": [], "version_resources": [], "unresolved": []}
    if machine == 0x200:
        exception_rva, exception_size = directory(3)
        exception = {"rva": hex(exception_rva), "bytes": exception_size,
            "record_bytes": 12, "format": "IMAGE_IA64_RUNTIME_FUNCTION_ENTRY",
            "regions": [], "validated": False,
            "boundary": "Exception regions may describe function fragments; entries do not establish independent callable functions or a complete function inventory"}
        result["exception_directory"] = exception
        try:
            if bool(exception_rva) != bool(exception_size):
                raise ValueError("IA64 exception directory has an incomplete address/size pair")
            if exception_size:
                if pointer_size != 8 or exception_size % 12:
                    raise ValueError("IA64 exception directory has a partial 12-byte record or invalid optional header")
                mappings = [(0, min(header_size, len(raw)), 0, False)]
                for index, section in enumerate(sections):
                    flags, = read("<I", optional + optional_size + index * 40 + 36)
                    if section["bytes"]:
                        mappings.append((section["rva"], section["rva"] + section["bytes"],
                                         section["file_offset"], bool(flags & 0x20000000)))

                def exception_at_rva(rva, length, *, executable=False):
                    overlaps = [row for row in mappings if row[0] < rva + length and rva < row[1]]
                    if (len(overlaps) != 1 or overlaps[0][0] > rva or rva + length > overlaps[0][1]
                            or executable and not overlaps[0][3]):
                        raise ValueError("IA64 exception RVA is not uniquely backed by "
                                         + ("executable " if executable else "") + "input bytes: " + hex(rva))
                    return overlaps[0][2] + rva - overlaps[0][0]

                table = exception_at_rva(exception_rva, exception_size)
                regions, previous_end = [], 0
                for index in range(exception_size // 12):
                    offset = table + index * 12
                    begin, end, unwind = read("<III", offset)
                    if not begin or end <= begin or begin % 16 or end % 16 or begin < previous_end:
                        raise ValueError("IA64 exception regions are unaligned, unordered, overlapping or empty")
                    code = exception_at_rva(begin, end - begin, executable=True)
                    if not unwind or unwind % 8:
                        raise ValueError("IA64 unwind information RVA is absent or unaligned")
                    unwind_offset = exception_at_rva(unwind, 8)
                    regions.append({"index": index, "table_file_offset": offset,
                        "entry_hex": raw[offset:offset + 12].hex(),
                        "begin_rva": hex(begin), "end_rva": hex(end), "bytes": end - begin,
                        "file_offset": code, "sha256": hashlib.sha256(raw[code:code + end - begin]).hexdigest(),
                        "unwind_rva": hex(unwind), "unwind_file_offset": unwind_offset,
                        "unwind_header_hex": raw[unwind_offset:unwind_offset + 8].hex(),
                        "unwind_semantics_verified": False})
                    previous_end = end
                exception["sha256"] = hashlib.sha256(raw[table:table + exception_size]).hexdigest()
                exception["regions"] = regions
            exception["validated"] = True
        except (ValueError, struct.error) as exc:
            result["unresolved"].append({"stage": "ia64_exception_directory", "error": str(exc)})
    for key, index, stride in (("imports", 1, 20), ("delay_imports", 13, 32)):
        rva, size = directory(index)
        if not rva or not size:
            continue
        try:
            for offset in range(0, size, stride):
                values = read("<" + "I" * (stride // 4), at_rva(rva + offset, stride))
                if not any(values):
                    break
                if key == "imports":
                    table, _, _, name, address_table = values
                    table = table or address_table
                else:
                    attrs, name, _, address_table, table, _, _, _ = values
                    if not attrs & 1:
                        name, table = name - image_base, table - image_base
                symbols = thunks(table)
                result[key].append({"library": string(name), "symbols": symbols})
            else:
                raise ValueError("Import directory lacks a null descriptor")
        except (ValueError, struct.error) as exc:
            result["unresolved"].append({"stage": key, "error": str(exc)})
    export_rva, export_size = directory(0)
    if export_rva and export_size:
        try:
            fields = read("<IIHHIIIIIII", at_rva(export_rva, 40))
            _, _, _, _, _, ordinal_base, function_count, name_count, functions, names, ordinals = fields
            named = {}
            for i in range(name_count):
                name_rva, = read("<I", at_rva(names + i * 4, 4))
                ordinal, = read("<H", at_rva(ordinals + i * 2, 2))
                if ordinal >= function_count:
                    raise ValueError("Export name ordinal is out of range")
                named.setdefault(ordinal, []).append(string(name_rva))
            for i in range(function_count):
                function_rva, = read("<I", at_rva(functions + i * 4, 4))
                if function_rva:
                    row = {"ordinal": ordinal_base + i, "names": named.get(i, []), "rva": hex(function_rva)}
                    if export_rva <= function_rva < export_rva + export_size:
                        row["forwarder"] = string(function_rva)
                    result["exports"].append(row)
        except (ValueError, struct.error) as exc:
            result["unresolved"].append({"stage": "exports", "error": str(exc)})
    clr_rva, clr_size = directory(14)
    if clr_rva and clr_size:
        header = at_rva(clr_rva, 24)
        cb, major, minor, metadata_rva, metadata_size, flags, entrypoint = read("<IHHIIII", header)
        metadata_offset = at_rva(metadata_rva, metadata_size)
        if raw[metadata_offset:metadata_offset + 4] != b"BSJB":
            raise ValueError("CLR header does not point to valid metadata")
        result["kind"] = "managed" if flags & 1 else "mixed_managed_native"
        result["clr"] = {"header_bytes": cb, "runtime_version": [major, minor], "flags": hex(flags),
                         "metadata_rva": hex(metadata_rva), "metadata_bytes": metadata_size,
                         "entrypoint_token_or_rva": hex(entrypoint)}
        if cb >= 72 and clr_size >= 72:
            native_rva, native_size = read("<II", header + 64)
            result["clr"]["managed_native_header"] = {"rva": hex(native_rva), "bytes": native_size}
            if native_rva and native_size >= 16:
                signature, major, minor, native_flags, count = read("<IHHII", at_rva(native_rva, 16))
                if signature == 0x00525452:
                    result["kind"] = "managed"
                    result["clr"]["ready_to_run"] = {"version": [major, minor], "flags": hex(native_flags), "sections": count}
    resource_rva, resource_size = directory(2)
    if resource_rva and resource_size:
        try:
            def resource_entries(offset):
                base = at_rva(resource_rva + offset, 16)
                named, numbered = read("<HH", base + 12)
                for i in range(named + numbered):
                    name, child = read("<II", at_rva(resource_rva + offset + 16 + i * 8, 8))
                    yield name, child

            def version_block(blob, begin=0, end=None, depth=0):
                end = len(blob) if end is None else end
                if depth > 8 or begin + 6 > end:
                    raise ValueError("Invalid version resource nesting")
                length, value_length, kind = struct.unpack_from("<HHH", blob, begin)
                block_end = begin + length
                if length < 6 or block_end > end:
                    raise ValueError("Version resource block exceeds parent")
                cursor = begin + 6
                while cursor + 2 <= block_end and blob[cursor:cursor + 2] != b"\0\0":
                    cursor += 2
                if cursor + 2 > block_end:
                    raise ValueError("Version resource key is unterminated")
                name = blob[begin + 6:cursor].decode("utf-16-le")
                value_start = (cursor + 2 + 3) & ~3
                value_end = value_start + value_length * (2 if kind == 1 else 1)
                if value_end > block_end:
                    raise ValueError("Version resource value exceeds parent")
                record = {"key": name}
                if kind == 1 and value_length:
                    record["value"] = blob[value_start:value_end].decode("utf-16-le").rstrip("\0")
                elif name == "VS_VERSION_INFO" and value_length >= 52:
                    fixed = struct.unpack_from("<13I", blob, value_start)
                    record["file_version"] = ".".join(str(v) for n in fixed[2:4] for v in (n >> 16, n & 0xffff))
                    record["product_version"] = ".".join(str(v) for n in fixed[4:6] for v in (n >> 16, n & 0xffff))
                children = []
                child_start = (value_end + 3) & ~3
                while child_start + 6 <= block_end:
                    child, child_end = version_block(blob, child_start, block_end, depth + 1)
                    children.append(child)
                    child_start = (child_end + 3) & ~3
                if children:
                    record["children"] = children
                return record, block_end

            def visit(offset, route, depth):
                if depth > 3:
                    raise ValueError("Version resource directory is cyclic or too deep")
                for name, child in resource_entries(offset):
                    if child & 0x80000000:
                        visit(child & 0x7fffffff, [*route, name], depth + 1)
                    else:
                        data_rva, data_size, codepage, _ = read("<IIII", at_rva(resource_rva + child, 16))
                        data_offset = at_rva(data_rva, data_size)
                        blob = raw[data_offset:data_offset + data_size]
                        parsed, _ = version_block(blob)
                        result["version_resources"].append({"route": [*route, name], "rva": hex(data_rva),
                            "bytes": data_size, "sha256": hashlib.sha256(blob).hexdigest(), "codepage": codepage, "tree": parsed})

            for name, child in resource_entries(0):
                if name == 16 and child & 0x80000000:
                    visit(child & 0x7fffffff, [name], 1)
        except (ValueError, struct.error, UnicodeDecodeError) as exc:
            result["unresolved"].append({"stage": "version_resources", "error": str(exc)})
    return result


def native_exception_region_coverage(exception, functions, image_base):
    """Compare audited exception regions with captured chunks without inventing owners."""
    if exception.get("validated") is not True:
        raise ValueError("Exception coverage requires a validated original directory")
    base = int(image_base, 0) if isinstance(image_base, str) else image_base
    if type(base) is not int or base < 0:
        raise ValueError("Exception coverage image base is invalid")
    chunks, entries = [], set()
    for function in functions:
        address = int(function["address"], 0)
        if address < base or int(function["rva"], 0) != address - base:
            raise ValueError("Captured function address differs from its RVA/image base")
        entries.add(address - base)
        for chunk in function["chunks"]:
            begin, end = int(chunk["start"], 0) - base, int(chunk["end"], 0) - base
            if begin < 0 or end <= begin:
                raise ValueError("Captured function chunk has an invalid interval")
            chunks.append((begin, end, function["rva"]))
    result = []
    for region in exception["regions"]:
        begin, end = int(region["begin_rva"], 0), int(region["end_rva"], 0)
        overlaps = sorted((max(begin, a), min(end, b), owner)
                          for a, b, owner in chunks if a < end and begin < b)
        cursor, missing = begin, []
        for a, b, _ in overlaps:
            if a > cursor:
                missing.append({"begin_rva": hex(cursor), "end_rva": hex(a), "bytes": a - cursor})
            cursor = max(cursor, b)
        if cursor < end:
            missing.append({"begin_rva": hex(cursor), "end_rva": hex(end), "bytes": end - cursor})
        result.append({"index": region["index"], "begin_rva": region["begin_rva"],
            "end_rva": region["end_rva"], "bytes": end - begin,
            "matches_captured_entry": begin in entries,
            "captured_chunk_owners": sorted({owner for _, _, owner in overlaps}, key=lambda x: int(x, 0)),
            "uncovered": missing, "covered_bytes": end - begin - sum(x["bytes"] for x in missing),
            "all_bytes_in_captured_chunks": not missing})
    return {"regions": result, "region_count": len(result),
        "uncovered_region_count": sum(bool(row["uncovered"]) for row in result),
        "uncovered_bytes": sum(x["bytes"] for row in result for x in row["uncovered"]),
        "all_regions_in_captured_chunks": all(row["all_bytes_in_captured_chunks"] for row in result),
        "function_discovery_complete": False,
        "boundary": "Chunk membership proves only byte coverage; exception fragments, leaf functions, indirect entries and unwind semantics require separate review"}


def inspect_embedded_pe_ranges(raw, reviewed_ranges):
    """Validate explicit, evidence-bound PE slices without discovering or executing them.

    The publisher verifies the parent and evidence file identities before calling
    this pure parser. Offsets, complete lengths, machine and hashes must come from
    the reviewed owning chain; an MZ hit or SizeOfImage is not a file boundary.
    """
    if not isinstance(raw, (bytes, bytearray, memoryview)) or not isinstance(reviewed_ranges, list):
        raise ValueError("Embedded PE review requires bytes and a list of explicit ranges")
    raw = bytes(raw)
    fields = {"name", "offset", "bytes", "sha256", "machine", "evidence", "basis"}
    names, spans = set(), []

    def digest(value):
        return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None

    for row in reviewed_ranges:
        if not isinstance(row, dict) or set(row) != fields:
            raise ValueError("Embedded PE review fields differ from the explicit range schema")
        name, offset, size = row["name"], row["offset"], row["bytes"]
        if (not isinstance(name, str) or not name or name in {".", ".."}
                or any(ord(c) < 32 or c in '<>:"/\\|?*' for c in name) or name[-1] in ". "
                or re.fullmatch(r"(?i:con|prn|aux|nul|com[1-9]|lpt[1-9])", name.split(".")[0])):
            raise ValueError("Embedded PE output name must be a safe basename")
        if name.casefold() in names:
            raise ValueError("Embedded PE output names collide")
        names.add(name.casefold())
        if (type(offset) is not int or type(size) is not int or offset <= 0 or size < 64
                or offset + size > len(raw)):
            raise ValueError("Embedded PE reviewed range is outside parent bytes")
        if (not digest(row["sha256"]) or not isinstance(row["machine"], str)
                or re.fullmatch(r"0x[0-9a-f]{1,4}", row["machine"]) is None
                or hex(int(row["machine"], 16)) != row["machine"]
                or not isinstance(row["basis"], str) or not row["basis"].strip()):
            raise ValueError("Embedded PE review needs an exact hash, machine and boundary basis")
        evidence, seen = row["evidence"], set()
        if not isinstance(evidence, list) or not evidence:
            raise ValueError("Embedded PE review has no evidence identities")
        for original in evidence:
            if not isinstance(original, dict) or set(original) != {"path", "bytes", "sha256"}:
                raise ValueError("Embedded PE evidence identity fields are invalid")
            path = original["path"]
            if (not isinstance(path, str) or not path.startswith(".analysis/") or "\\" in path
                    or ":" in path or any(part in {"", ".", ".."} for part in path.split("/"))
                    or path in seen or type(original["bytes"]) is not int or original["bytes"] <= 0
                    or not digest(original["sha256"])):
                raise ValueError("Embedded PE evidence needs unique project-relative identities")
            seen.add(path)
        spans.append((offset, offset + size))
    ordered = sorted(spans)
    if any(left[1] > right[0] for left, right in zip(ordered, ordered[1:])):
        raise ValueError("Embedded PE reviewed ranges overlap")

    results = []
    for row in reviewed_ranges:
        body = raw[row["offset"]:row["offset"] + row["bytes"]]
        actual = hashlib.sha256(body).hexdigest()
        if actual != row["sha256"]:
            raise ValueError("Embedded PE reviewed hash differs from actual parent bytes")

        def read(fmt, offset):
            if offset < 0 or offset + struct.calcsize(fmt) > len(body):
                raise ValueError("Embedded PE structure exceeds reviewed bytes")
            return struct.unpack_from(fmt, body, offset)

        if body[:2] != b"MZ":
            raise ValueError("Embedded PE reviewed range has no DOS header")
        pe, = read("<I", 0x3c)
        if pe < 64 or body[pe:pe + 4] != b"PE\0\0":
            raise ValueError("Embedded PE reviewed range has no valid PE signature")
        machine, count, _, symbol_offset, symbol_count, optional_size, flags = read("<HHIIIHH", pe + 4)
        optional = pe + 24
        magic, = read("<H", optional)
        directory_start = {0x10b: 96, 0x20b: 112}.get(magic)
        if (not machine or hex(machine) != row["machine"] or not flags & 2 or not count
                or directory_start is None or optional_size < directory_start):
            raise ValueError("Embedded PE machine or executable headers disagree with review")
        expected_magic = {0x14c: 0x10b, 0x1c0: 0x10b, 0x1c4: 0x10b,
                          0x200: 0x20b, 0x8664: 0x20b, 0xaa64: 0x20b}.get(machine)
        if expected_magic is not None and expected_magic != magic:
            raise ValueError("Embedded PE machine and optional header width disagree")
        section_table = optional + optional_size
        table_end = section_table + count * 40
        entry, = read("<I", optional + 16)
        section_alignment, file_alignment = read("<II", optional + 32)
        image_size, header_size = read("<II", optional + 56)
        directory_count, = read("<I", optional + directory_start - 4)
        if (not section_alignment or section_alignment & (section_alignment - 1)
                or not file_alignment or file_alignment & (file_alignment - 1)
                or section_alignment < file_alignment or header_size % file_alignment
                or not table_end <= header_size <= len(body)
                or image_size < header_size or image_size % section_alignment
                or directory_count * 8 > optional_size - directory_start):
            raise ValueError("Embedded PE header, alignment or directory bounds are invalid")

        raw_spans = [{"kind": "headers", "offset": 0, "bytes": header_size}]
        virtual_spans, sections = [(0, header_size)], []
        for index in range(count):
            section = section_table + index * 40
            name, virtual_size, rva, size, offset = read("<8sIIII", section)
            characteristics, = read("<I", section + 36)
            mapped_size = max(virtual_size, size)
            if (rva % section_alignment or rva + mapped_size > image_size
                    or size and (offset % file_alignment or size % file_alignment
                                 or offset + size > len(body))):
                raise ValueError("Embedded PE section exceeds file/image bounds or alignment")
            if mapped_size:
                virtual_spans.append((rva, rva + mapped_size))
            if size:
                raw_spans.append({"kind": "section", "index": index, "offset": offset, "bytes": size})
            sections.append({"index": index, "name": name.rstrip(b"\0").decode("ascii", errors="replace"),
                "rva": rva, "virtual_bytes": virtual_size, "file_offset": offset, "bytes": size,
                "characteristics": hex(characteristics),
                "sha256": hashlib.sha256(body[offset:offset + size]).hexdigest()})

        def reject_overlaps(spans, label):
            values = sorted(spans)
            if any(a[1] > b[0] for a, b in zip(values, values[1:])):
                raise ValueError("Embedded PE " + label + " ranges overlap")

        reject_overlaps([(s["offset"], s["offset"] + s["bytes"]) for s in raw_spans], "raw section/header")
        reject_overlaps(virtual_spans, "virtual section/header")

        def at_rva(rva, size):
            if 0 <= rva < header_size and rva + size <= header_size:
                return rva
            for section in sections:
                delta = rva - section["rva"]
                if 0 <= delta < section["bytes"] and delta + size <= section["bytes"]:
                    return section["file_offset"] + delta
            raise ValueError("Embedded PE RVA range has no complete original byte backing")

        if entry:
            at_rva(entry, 16 if machine == 0x200 else 1)
            if machine in {0x14c, 0x8664, 0xaa64} and not any(
                    s["rva"] <= entry < s["rva"] + s["bytes"]
                    and int(s["characteristics"], 16) & 0x20000000 for s in sections):
                raise ValueError("Embedded PE entrypoint is not in executable bytes")

        directories, security, debug = [], None, []
        for index in range(directory_count):
            address, size = read("<II", optional + directory_start + index * 8)
            record = {"index": index, "address": address, "bytes": size, "address_kind": "rva"}
            if index == 4:
                record["address_kind"] = "file_offset"
                if address or size:
                    if not address or size < 8 or address % 8 or address + size > len(body):
                        raise ValueError("Embedded PE security directory exceeds reviewed file bytes")
                    security = {"offset": address, "bytes": size,
                        "sha256": hashlib.sha256(body[address:address + size]).hexdigest(),
                        "certificates": [], "cryptographic_signature_verified": False}
                    position = address
                    while position < address + size:
                        length, revision, kind = read("<IHH", position)
                        aligned = (length + 7) & ~7
                        if (length < 8 or revision not in {0x100, 0x200} or kind not in {1, 2, 3, 4}
                                or position + aligned > address + size):
                            raise ValueError("Embedded PE WIN_CERTIFICATE range is invalid or truncated")
                        security["certificates"].append({"offset": position, "bytes": length,
                            "aligned_bytes": aligned, "revision": hex(revision), "type": kind,
                            "sha256": hashlib.sha256(body[position:position + length]).hexdigest()})
                        position += aligned
            elif index == 8:
                # IA64 uses this zero-sized entry as GP metadata, not a raw span.
                record["address_kind"] = "global_pointer"
                if size:
                    raise ValueError("Embedded PE global-pointer directory must have zero size")
            elif address or size:
                if not address or not size:
                    raise ValueError("Embedded PE data directory has an incomplete address/size pair")
                record["file_offset"] = at_rva(address, size)
                if index == 6:
                    if size % 28:
                        raise ValueError("Embedded PE debug directory has a partial record")
                    for position in range(record["file_offset"], record["file_offset"] + size, 28):
                        _, _, _, _, kind, data_size, data_rva, data_offset = read("<IIHHIIII", position)
                        if data_size:
                            if not data_offset or data_offset + data_size > len(body):
                                raise ValueError("Embedded PE debug data exceeds reviewed file bytes")
                            if data_rva and at_rva(data_rva, data_size) != data_offset:
                                raise ValueError("Embedded PE debug RVA and file offset disagree")
                            debug.append({"kind": "debug_data", "type": kind,
                                "offset": data_offset, "bytes": data_size})
            directories.append(record)

        if symbol_offset or symbol_count:
            if not symbol_offset:
                raise ValueError("Embedded PE COFF symbols have no file offset")
            strings = symbol_offset + symbol_count * 18
            string_bytes, = read("<I", strings)
            if string_bytes < 4 or strings + string_bytes > len(body):
                raise ValueError("Embedded PE COFF symbols/string table exceed reviewed file bytes")
            symbol_span = {"kind": "coff_symbols", "offset": symbol_offset,
                "bytes": symbol_count * 18 + string_bytes}
            reject_overlaps([(s["offset"], s["offset"] + s["bytes"])
                             for s in [*raw_spans, symbol_span]], "COFF symbol/image")
            raw_spans.append(symbol_span)
        for span in debug:
            if not any(s["offset"] <= span["offset"] and span["offset"] + span["bytes"]
                       <= s["offset"] + s["bytes"] for s in raw_spans):
                reject_overlaps([(s["offset"], s["offset"] + s["bytes"])
                                 for s in [*raw_spans, span]], "debug/image")
                raw_spans.append(span)
        if security:
            reject_overlaps([(s["offset"], s["offset"] + s["bytes"])
                             for s in [*raw_spans, security]], "security/image")
            raw_spans.append({"kind": "security", "offset": security["offset"], "bytes": security["bytes"]})
        complete_size = max(s["offset"] + s["bytes"] for s in raw_spans)
        if complete_size != len(body):
            raise ValueError("Embedded PE reviewed length differs from complete declared file extent")
        metadata = inspect_pe(body)
        if metadata.get("format") not in {"pe32", "pe32+"} or metadata["unresolved"]:
            raise ValueError("Embedded PE has unresolved or invalid metadata")
        results.append({"name": row["name"], "offset": row["offset"], "bytes": len(body), "sha256": actual,
            "pe": metadata, "layout": {"header_bytes": header_size, "image_bytes": image_size,
                "entrypoint_rva": hex(entry),
                "entrypoint_kind": "ia64_function_descriptor" if machine == 0x200 else "code_rva",
                "subsystem": read("<H", optional + 68)[0], "sections": sections,
                "directories": directories, "raw_ranges": raw_spans, "security_directory": security,
                "complete_file_bytes": complete_size},
            "review": {**row, "evidence": [dict(original) for original in row["evidence"]]}})
    return results


def pe_resource_entries(raw):
    """Read complete PE resource leaves and byte ranges without loading the image."""
    metadata = inspect_pe(raw)
    if metadata["format"] not in ("pe32", "pe32+"):
        raise ValueError("Resource input is not a PE image")

    def read(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(raw):
            raise ValueError("PE resource structure exceeds input bytes")
        return struct.unpack_from(fmt, raw, offset)

    pe, = read("<I", 0x3c)
    optional = pe + 24
    directory = optional + (112 if metadata["format"] == "pe32+" else 96)
    count, = read("<I", directory - 4)
    if count <= 2:
        return []
    resource_rva, resource_size = read("<II", directory + 16)
    if not resource_rva or not resource_size:
        return []
    header_size, = read("<I", optional + 60)

    def at_rva(rva, size):
        if size < 0:
            raise ValueError("Invalid PE resource length")
        if 0 <= rva and rva + size <= min(header_size, len(raw)):
            return rva
        for section in metadata["sections"]:
            delta = rva - section["rva"]
            if 0 <= delta and delta + size <= section["bytes"]:
                return section["file_offset"] + delta
        raise ValueError("PE resource RVA is not backed by input bytes: " + hex(rva))

    def relative(offset, size):
        if offset < 0 or offset + size > resource_size:
            raise ValueError("PE resource directory exceeds declared bounds")
        return at_rva(resource_rva + offset, size)

    def identifier(value):
        if value & 0x80000000:
            offset = value & 0x7fffffff
            length, = read("<H", relative(offset, 2))
            begin = relative(offset + 2, length * 2)
            return raw[begin:begin + length * 2].decode("utf-16-le")
        if value > 0xffff:
            raise ValueError("PE resource numeric identifier uses reserved bits")
        return value

    leaves, routes = [], set()

    def visit(offset, route, parents):
        if offset in parents or len(route) > 2:
            raise ValueError("PE resource directory is cyclic or too deep")
        base = relative(offset, 16)
        named, numbered = read("<HH", base + 12)
        relative(offset + 16, (named + numbered) * 8)
        siblings = set()
        for index in range(named + numbered):
            name, child = read("<II", relative(offset + 16 + index * 8, 8))
            key = identifier(name)
            if key in siblings:
                raise ValueError("PE resource directory repeats an identifier")
            siblings.add(key)
            branch = (*route, key)
            if child & 0x80000000:
                visit(child & 0x7fffffff, branch, (*parents, offset))
                continue
            if len(branch) != 3 or branch in routes:
                raise ValueError("PE resource leaf has an invalid type/name/language route")
            routes.add(branch)
            data_rva, size, codepage, reserved = read("<IIII", relative(child, 16))
            if reserved:
                raise ValueError("PE resource data uses reserved fields")
            data_offset = at_rva(data_rva, size)
            leaves.append({"type": branch[0], "name": branch[1], "language": branch[2],
                           "rva": hex(data_rva), "file_offset": data_offset, "bytes": size,
                           "sha256": hashlib.sha256(raw[data_offset:data_offset + size]).hexdigest(),
                           "codepage": codepage})

    visit(0, (), ())
    return leaves


def pe_cursor_groups(raw, group_id=None):
    """Reconstruct CUR directories while preserving original hotspots and DIB bytes."""
    entries = pe_resource_entries(raw)
    groups = [row for row in entries if row["type"] == 12 and (group_id is None or row["name"] == group_id)]
    if group_id is not None and not groups:
        raise ValueError("Requested PE cursor group is absent")

    def data(row):
        return raw[row["file_offset"]:row["file_offset"] + row["bytes"]]

    results = []
    for group in groups:
        directory = data(group)
        if len(directory) < 6:
            raise ValueError("PE cursor group header is truncated")
        reserved, kind, count = struct.unpack_from("<HHH", directory)
        if reserved or kind != 2 or not count or len(directory) != 6 + count * 14:
            raise ValueError("Invalid PE cursor group directory")
        frames, payloads = [], []
        offset = 6 + count * 16
        cur = bytearray(struct.pack("<HHH", 0, 2, count))
        for index in range(count):
            width, combined_height, planes, bit_count, size, identifier = struct.unpack_from("<HHHHIH", directory, 6 + index * 14)
            matches = [row for row in entries if row["type"] == 1 and row["name"] == identifier
                       and row["language"] == group["language"]]
            if not matches:
                matches = [row for row in entries if row["type"] == 1 and row["name"] == identifier and row["language"] == 0]
            if len(matches) != 1:
                raise ValueError("PE cursor image language/identifier is missing or ambiguous")
            original = matches[0]
            image = data(original)
            if len(image) != size or len(image) < 44:
                raise ValueError("PE cursor group and image lengths differ")
            hotspot_x, hotspot_y = struct.unpack_from("<HH", image)
            dib = image[4:]
            header, dib_width, dib_height, dib_planes, dib_bits, compression, image_size, _, _, colors, _ = struct.unpack_from("<IiiHHIIiiII", dib)
            if (header != 40 or compression != 0 or dib_planes != 1 or dib_bits not in (1, 4, 8, 16, 24, 32)
                    or dib_width != width or dib_height != combined_height or dib_height <= 0 or dib_height % 2
                    or planes != dib_planes or bit_count != dib_bits or not 1 <= width <= 256):
                raise ValueError("Cursor extraction supports only matching uncompressed BITMAPINFOHEADER frames")
            height = dib_height // 2
            if not 1 <= height <= 256 or hotspot_x >= width or hotspot_y >= height:
                raise ValueError("PE cursor dimensions or hotspot are out of range")
            palette = colors or (1 << dib_bits if dib_bits <= 8 else 0)
            if (dib_bits <= 8 and palette > 1 << dib_bits) or (dib_bits > 8 and palette):
                raise ValueError("Unsupported PE cursor palette")
            xor_bytes = ((width * dib_bits + 31) // 32) * 4 * height
            and_bytes = ((width + 31) // 32) * 4 * height
            if len(dib) != header + palette * 4 + xor_bytes + and_bytes or image_size not in (0, xor_bytes):
                raise ValueError("PE cursor bitmap or mask length differs from its dimensions")
            color_count = palette if palette < 256 else 0
            cur.extend(struct.pack("<BBBBHHII", width % 256, height % 256, color_count, 0,
                                   hotspot_x, hotspot_y, len(dib), offset))
            frames.append({"ordinal": index, "width": width, "height": height, "hotspot_x": hotspot_x,
                           "hotspot_y": hotspot_y, "planes": planes, "bit_count": bit_count,
                           "group_height": combined_height, "group_image_bytes": size, "resource_id": identifier,
                           "original": original, "cur_image_offset": offset, "cur_image_bytes": len(dib),
                           "cur_image_sha256": hashlib.sha256(dib).hexdigest()})
            offset += len(dib)
            if offset > 0xffffffff:
                raise ValueError("CUR directory offset exceeds its 32-bit field")
            payloads.append(dib)
        cur.extend(b"".join(payloads))
        results.append({"group": group, "frames": frames, "data": bytes(cur)})
    return results


def prepare_managed_analyzer():
    """Restore only exact pinned ILSpy bytes declared by the existing receipt."""
    import analysis as a
    root = a.ROOT / "tools/analyzers/ilspycmd-9.1.0.7988"
    receipt = json.loads((root / "acquisition.json").read_bytes())
    missing = []
    for row in receipt["files"]:
        path = a.resolve(row["path"], a.rel(root))
        if not path.is_file() or path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            missing.append((path, row))
    restored = []
    if missing:
        request = urllib.request.Request(receipt["url"], headers={"User-Agent": "razer-static-evidence/1"})
        with urllib.request.urlopen(request, timeout=90) as response:
            raw = response.read()
        if len(raw) != receipt["bytes"] or hashlib.sha256(raw).hexdigest() != receipt["sha256"]:
            raise ValueError("ILSpy archive differs from pinned acquisition receipt")
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            pending = []
            for path, row in missing:
                member = "tools/net8.0/any/" + path.relative_to(root).as_posix()
                data = archive.read(member)
                if len(data) != row["bytes"] or hashlib.sha256(data).hexdigest() != row["sha256"]:
                    raise ValueError("ILSpy member differs from pinned acquisition receipt: " + member)
                pending.append((path, data, row))
            for path, data, row in pending:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
                restored.append(row["path"])
    for row in receipt["files"]:
        path = a.ROOT / row["path"]
        if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            raise ValueError("ILSpy analyzer identity differs: " + row["path"])
    return root, receipt, restored


def prepare_inno_analyzer():
    """Acquire/verify the pinned static unpacker; never launch an installer."""
    import analysis as a
    root = a.ROOT / "tools/analyzers/innounp-2.71.2"
    receipt_path = root / "acquisition.json"
    url = "https://github.com/jrathlev/InnoUnpacker-Windows-GUI/releases/download/ui_2_2_13/innounp-2.zip"
    expected_sha = "10fce27b6344b7674d5625cc34afca1c88c12183d957c6d93e401097fa2556cd"
    expected_bytes = 618221
    if receipt_path.is_file():
        receipt = json.loads(receipt_path.read_bytes())
        if (receipt.get("sha256"), receipt.get("bytes"), receipt.get("url")) != (expected_sha, expected_bytes, url):
            raise ValueError("Inno unpacker acquisition differs from pinned release")
        missing = []
        for row in receipt["files"]:
            path = a.resolve(row["path"], a.rel(root))
            if not path.is_file() or path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
                missing.append((path, row))
        if missing:
            restoration = {"recorded_at_utc": a.stamp(), "acquisition": {
                "path": a.rel(receipt_path), "sha256": a.sha(receipt_path)},
                "missing_or_different": [row for _, row in missing], "downloads": [],
                "restored": [], "complete": False, "target_execution": False}
            restoration_path = root / "restoration.json"
            try:
                def retained_download(source):
                    request = urllib.request.Request(source, headers={"User-Agent": "razer-static-evidence/1"})
                    with urllib.request.urlopen(request, timeout=90) as response:
                        raw = response.read()
                        final = response.geturl().split("?", 1)[0]
                        parsed = urllib.parse.urlsplit(final)
                        if parsed.scheme != "https" or parsed.netloc not in {
                                "github.com", "api.github.com", "codeload.github.com", "release-assets.githubusercontent.com"}:
                            raise ValueError("Unexpected Inno analyzer acquisition redirect")
                        restoration["downloads"].append({"url": source, "final_url": final,
                            "http_status": response.status, "headers": dict(response.headers),
                            "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
                        return raw
                archive_raw = retained_download(url)
                if len(archive_raw) != expected_bytes or hashlib.sha256(archive_raw).hexdigest() != expected_sha:
                    raise ValueError("Inno restoration archive differs from pinned acquisition")
                with zipfile.ZipFile(io.BytesIO(archive_raw)) as archive:
                    pending = []
                    for path, row in missing:
                        name = path.relative_to(root).as_posix()
                        if name == "innounp-2.zip":
                            raw = archive_raw
                        elif name == "release.json":
                            raw = retained_download(receipt["release"]["url"])
                        elif name == "source.zip":
                            raw = retained_download(receipt["source"]["url"])
                        else:
                            raw = archive.read(name)
                        if len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]:
                            raise ValueError("Inno restoration bytes differ from pinned acquisition: " + row["path"])
                        pending.append((path, raw, row))
                    for path, raw, row in pending:
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes(raw)
                        restoration["restored"].append(row)
                restoration["complete"] = True
            except Exception as error:
                restoration["error"] = str(error)
                raise
            finally:
                a.save(restoration_path, restoration)
        for row in receipt["files"]:
            path = a.resolve(row["path"], a.rel(root))
            if not path.is_file() or path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
                raise ValueError("Inno static analyzer identity differs: " + row["path"])
        return root / "innounp.exe", receipt

    def fetch(source):
        request = urllib.request.Request(source, headers={"User-Agent": "razer-static-evidence/1"})
        with urllib.request.urlopen(request, timeout=45) as response:
            return response.read(), {"url": source, "http_status": response.status,
                                     "final_url": response.geturl().split("?", 1)[0],
                                     "headers": dict(response.headers.items())}

    release_raw, release_origin = fetch("https://api.github.com/repos/jrathlev/InnoUnpacker-Windows-GUI/releases/tags/ui_2_2_13")
    release = json.loads(release_raw)
    asset = next(row for row in release["assets"] if row["name"] == "innounp-2.zip")
    if release["tag_name"] != "ui_2_2_13" or (asset["digest"], asset["size"], asset["browser_download_url"]) != (
            "sha256:" + expected_sha, expected_bytes, url):
        raise ValueError("Inno unpacker release declaration changed")
    raw, origin = fetch(url)
    if len(raw) != expected_bytes or hashlib.sha256(raw).hexdigest() != expected_sha:
        raise ValueError("Inno unpacker archive differs from declared release digest")
    source_commit = "032d64c782efc6b36766d8f9c3b1b2f042144a58"
    source_url = "https://codeload.github.com/jrathlev/InnoUnpacker-Windows-GUI/zip/" + source_commit
    source_raw, source_origin = fetch(source_url)
    pending = {"innounp-2.zip": raw, "release.json": release_raw, "source.zip": source_raw}
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        for member in archive.infolist():
            if member.is_dir():
                continue
            if member.filename != Path(member.filename).name or re.search(r'[\\/:*?"<>|]', member.filename):
                raise ValueError("Unsafe static analyzer member")
            pending[member.filename] = archive.read(member)
    if "innounp.exe" not in pending or b"2.71.2" not in pending.get("innounp-cl.txt", b""):
        raise ValueError("Inno unpacker package does not contain the declared console version")
    root.mkdir(parents=True, exist_ok=True)
    files = []
    for name, content in pending.items():
        path = root / name
        if path.exists() and path.read_bytes() != content:
            raise ValueError("Partial static analyzer acquisition differs: " + a.rel(path))
        path.write_bytes(content)
        files.append({"path": a.rel(path), "bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()})
    receipt = {"schema_version": 1, "recorded_at_utc": a.stamp(), "name": "Inno Setup Unpacker",
               "version": "2.71.2", **origin, "bytes": len(raw), "sha256": expected_sha,
               "release": {**release_origin, "path": a.rel(root / "release.json"), "asset_id": asset["id"],
                           "declared_digest": asset["digest"]},
               "source": {**source_origin, "path": a.rel(root / "source.zip"), "commit": source_commit,
                          "bytes": len(source_raw), "sha256": hashlib.sha256(source_raw).hexdigest()},
               "files": files, "target_execution": False, "real_device_operations": False,
               "scope": "Static Inno Setup archive metadata/payload extraction only; reconstructed scripts are analyzer outputs"}
    a.save(receipt_path, receipt)
    return root / "innounp.exe", receipt


def inno_archive_entries(raw):
    """Parse the unpacker's UTF-8 short listing before allowing extraction."""
    names = []
    for line in raw.decode("utf-8-sig").replace("\r\n", "\n").splitlines():
        if not line or line.startswith(("Inno Setup archive:", "Inno Setup version detected:")):
            continue
        if not line.startswith("  "):
            raise ValueError("Unrecognized Inno archive listing output: " + line)
        name = line[2:]
        parts = name.replace("\\", "/").split("/")
        if name.startswith(("/", "\\")) or any(not part or part in {".", ".."} or ":" in part for part in parts):
            raise ValueError("Unsafe Inno archive entry: " + name)
        names.append(name)
    if not names:
        raise ValueError("Inno static listing contains no archive entries")
    return names


def recheck_installed(args):
    """Compare live code files with retained originals without reading user state."""
    import analysis as a
    baseline_path = a.ROOT / ".analysis/inputs/installed-current/acquisition.json"
    baseline_raw = baseline_path.read_bytes()
    baseline = json.loads(baseline_raw)
    roots = {"app-engine": Path(args.app_engine_root), "apps": Path(args.user_data_root) / "Apps"}
    if args.services_root:
        roots["services"] = Path(args.services_root)
    rows, failures = [], []
    for old in baseline["files"]:
        if old["source_root"] == "cache-storage":
            continue
        record = {"source_root": old["source_root"], "source_relative_path": old["source_relative_path"],
                  "baseline": {key: old[key] for key in ("path", "bytes", "sha256")}}
        rows.append(record)
        try:
            root = roots[old["source_root"]].resolve()
            relative = Path(old["source_relative_path"])
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError("Installed source path is not relative")
            source = root / relative
            if source.is_symlink() or source.is_junction() or not source.resolve().is_relative_to(root):
                raise ValueError("Installed source path escaped its declared root")
            before = source.stat()
            raw = source.read_bytes()
            current_sha = hashlib.sha256(raw).hexdigest()
            after = source.stat()
            if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns) or a.sha(source) != current_sha:
                raise ValueError("Installed source changed during verification")
            copy = a.ROOT / old["path"]
            if copy.stat().st_size != old["bytes"] or a.sha(copy) != old["sha256"]:
                raise ValueError("Retained installed original differs from its receipt")
            record.update(bytes=len(raw), sha256=current_sha,
                          matches_retained_original=len(raw) == old["bytes"] and current_sha == old["sha256"])
            if old["source_root"] == "app-engine" and relative.name == "RazerAppEngine.exe":
                record["version_resources"] = inspect_pe(raw)["version_resources"]
            if not record["matches_retained_original"]:
                raise ValueError("Live installed code differs from the retained original")
        except (OSError, ValueError, KeyError) as error:
            record["error_kind"] = type(error).__name__
            failures.append({"source_root": record["source_root"], "source_relative_path": record["source_relative_path"],
                             "error_kind": type(error).__name__})
    metadata_path = a.ROOT / ".analysis/inputs/host-current/metadata-recheck.json"
    metadata_raw = metadata_path.read_bytes()
    metadata = json.loads(metadata_raw)
    acquisition_path = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    acquisition_raw = acquisition_path.read_bytes()
    acquisition = json.loads(acquisition_raw)
    if (not metadata.get("deployment_metadata_stable") or not metadata.get("matches_previous_metadata")
            or metadata["baseline"]["sha256"] != hashlib.sha256(acquisition_raw).hexdigest()):
        raise ValueError("Current official host deployment does not match the retained package acquisition")
    for row in metadata["files"]:
        path = a.ROOT / row["path"]
        if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            raise ValueError("Current official metadata original differs")
    inspection_path = a.ROOT / ".analysis/results/host-current/inspection.json"
    inspection_raw = inspection_path.read_bytes()
    inspection = json.loads(inspection_raw)
    packages, matched = [], 0
    for package in acquisition["files"]:
        if package["kind"] != "official_stable_host_package" or package["declaration"]["name"] != "Razer App Engine":
            continue
        declaration = package["declaration"]
        if not any(module == declaration for product in metadata["stable_products"] for module in product["modules"]):
            raise ValueError("Retained host package is not declared by the current official stable metadata")
        package_raw = (a.ROOT / package["path"]).read_bytes()
        if (len(package_raw) != declaration["file_size"] or hashlib.sha256(package_raw).hexdigest() != package["sha256"]
                or hashlib.md5(package_raw).hexdigest() != declaration["download_file_checksum"]):
            raise ValueError("Official host package byte identity differs")
        container = next(row for row in inspection["containers"] if row["input"] == package["path"])
        if not container.get("extracted") or container.get("extraction_exit_code") != 0 or container["sha256"] != package["sha256"]:
            raise ValueError("Official host package has no successful static extraction")
        official = {}
        for row in container["files"]:
            path = a.ROOT / row["path"]
            if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
                raise ValueError("Official static extraction original differs")
            official.setdefault((row["bytes"], row["sha256"]), []).append(row["path"])
        for row in rows:
            if row["source_root"] != "app-engine" or row.get("error_kind"):
                continue
            row["current_official_package_matches"] = official.get((row["bytes"], row["sha256"]), [])
            matched += bool(row["current_official_package_matches"])
        packages.append({"path": package["path"], "sha256": package["sha256"], "version": declaration["version"],
                         "static_extraction_files_verified": len(container["files"])})
    if not packages:
        raise ValueError("Current stable metadata lacks a retained App Engine package")
    report = {"schema_version": 1, "recorded_at_utc": a.stamp(),
        "tool": {"path": "tools/host.py", "sha256": a.sha(Path(__file__))},
        "baseline": {"path": a.rel(baseline_path), "bytes": len(baseline_raw), "sha256": hashlib.sha256(baseline_raw).hexdigest()},
        "metadata_recheck": {"path": a.rel(metadata_path), "sha256": hashlib.sha256(metadata_raw).hexdigest()},
        "static_extraction_receipt": {"path": a.rel(inspection_path), "sha256": hashlib.sha256(inspection_raw).hexdigest()},
        "official_host_packages": packages, "files": rows, "failures": failures,
        "scope": "Live installed static code versus retained originals and current official host package bytes; excludes caches, user state and runtime acceptance",
        "target_execution": False, "real_device_operations": False,
        "summary": {"files": len(rows), "matches_retained_original": sum(row.get("matches_retained_original", False) for row in rows),
                    "app_engine_current_official_matches": matched, "failures": len(failures)},
        "host_package_closure_complete": False}
    for path, raw in ((baseline_path, baseline_raw), (metadata_path, metadata_raw), (inspection_path, inspection_raw), (acquisition_path, acquisition_raw)):
        if path.read_bytes() != raw:
            raise ValueError("Installed recheck evidence changed during verification")
    receipt = a.ROOT / ".analysis/inputs/installed-current/recheck.json"
    a.save(receipt, report)
    a.index_files([{"path": a.rel(receipt), "bytes": receipt.stat().st_size, "sha256": a.sha(receipt), "kind": "installed_static_recheck"}])
    print(json.dumps({"receipt": a.rel(receipt), **report["summary"]}), flush=True)
    if failures:
        raise SystemExit(1)


def acquire_installed(args):
    """Snapshot installed files and public static Service Worker resources as data.

    Cache provenance is kept separate from a live official deployment audit.
    No browser, vendor process, JavaScript, native export or device is invoked.
    """
    if getattr(args, "verify_only", False):
        return recheck_installed(args)
    import analysis as a
    tool_root = a.ROOT / "tools/analyzers/ccl-chromium-reader"
    tool_receipt = tool_root / "acquisition.json"
    tool_data = json.loads(tool_receipt.read_bytes())
    for row in tool_data["files"]:
        path = a.ROOT / row["path"]
        if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            raise ValueError("Chromium cache analyzer byte identity differs")
    # Load only the pinned cache module and its type definitions. The upstream
    # package initializer imports unrelated browser-history/cookie readers.
    package = types.ModuleType("ccl_chromium_reader")
    package.__path__ = [str(tool_root / "ccl_chromium_reader")]
    sys.modules[package.__name__] = package
    from ccl_chromium_reader import ccl_chromium_cache as cache

    output = a.ROOT / ".analysis/inputs/installed-current"
    result = a.ROOT / ".analysis/results/installed-current"
    output.mkdir(parents=True, exist_ok=True)
    result.mkdir(parents=True, exist_ok=True)
    indexed = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
    identities = {}
    for row in indexed["inputs"]:
        if not row["path"].startswith(a.rel(output) + "/"):
            identities.setdefault(row["sha256"], []).append(row)
    files, resources, failures, excluded = [], [], [], 0
    excluded_installed = {}
    report = {"schema_version": 1, "started_at_utc": a.stamp(), "files": files,
              "resources": resources, "failures": failures, "target_execution": False,
              "real_device_operations": False,
              "tool": {"path": "tools/host.py", "sha256": a.sha(Path(__file__))},
              "source_current_during_acquisition": False,
              "scope": "installed host/native files and cached public official resources; cache age and complete resource closure require independent deployment audit"}

    def snapshot(source, root, label, category="files"):
        relative = source.relative_to(root)
        if source.is_symlink() or source.is_junction() or not source.resolve().is_relative_to(root.resolve()):
            raise ValueError("Linked installed input is outside its declared root")
        before = source.stat()
        raw = source.read_bytes()
        after = source.stat()
        identity = hashlib.sha256(raw).hexdigest()
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns) or a.sha(source) != identity:
            raise ValueError("Installed input changed during read")
        path = output / category / label / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        if a.sha(path) != identity:
            raise ValueError("Private installed evidence copy differs")
        matches = [row["path"] for row in identities.get(identity, [])
                   if (a.ROOT / row["path"]).is_file() and a.sha(a.ROOT / row["path"]) == identity]
        record = {"path": a.rel(path), "bytes": len(raw), "sha256": identity,
                  "kind": "chromium_cache_container" if label == "cache-storage" else "installed_static_evidence", "source_root": label,
                  "source_relative_path": relative.as_posix(), "acquired_at_utc": a.stamp(),
                  "source_unchanged": True, "verified_existing_originals": matches}
        files.append(record)
        return path, raw, record

    def failure(label, source, root, stage, error):
        failures.append({"source_root": label, "source_relative_path": source.relative_to(root).as_posix(),
                         "stage": stage, "error_kind": type(error).__name__,
                         "errno": getattr(error, "errno", None)})

    roots = [("app-engine", Path(args.app_engine_root)), ("apps", Path(args.user_data_root) / "Apps")]
    if args.services_root:
        roots.append(("services", Path(args.services_root)))
    for label, root in roots:
        if not root.is_dir():
            failures.append({"source_root": label, "stage": "inventory", "error_kind": "root_absent"})
            continue
        for source in sorted(root.rglob("*")):
            if source.is_file():
                # Service directories also contain live logs/databases and
                # per-user state. Retain code and explicit .NET deployment
                # manifests only; no account or activity data is evidence.
                if label == "services" and not (
                    source.suffix.lower() in {".dll", ".exe", ".node"}
                    or source.name.lower().endswith((".deps.json", ".runtimeconfig.json"))
                ):
                    extension = source.suffix.lower() or "<none>"
                    excluded_installed[extension] = excluded_installed.get(extension, 0) + 1
                    continue
                try:
                    snapshot(source, root, label)
                except (OSError, ValueError) as error:
                    failure(label, source, root, "snapshot", error)
        print(json.dumps({"installed_root": label, "captured_files": len(files), "failures": len(failures)}), flush=True)

    report["cache_analyzer"] = {"name": "ccl_chromium_reader", "version": cache.__version__,
                                "commit": tool_data["commit"], "acquisition": a.rel(tool_receipt),
                                "acquisition_sha256": a.sha(tool_receipt)}
    cache_root = Path(args.user_data_root) / "Default/Service Worker/CacheStorage"
    grouped = {}
    for source in sorted(cache_root.rglob("*_0")):
        try:
            # The key alone selects public static official origins before any
            # response data is copied; account/profile storage is not parsed.
            with source.open("rb") as stream:
                header = cache.SimpleCacheHeader.from_reader(cache.BinaryReader(stream))
                key = stream.read(header.key_length).decode("latin-1")
            url = urllib.parse.urlsplit(key)
            if url.scheme != "https" or url.netloc not in {"apps.razer.com", "app-assets.razer.com"}:
                excluded += 1
                continue
            private, raw, original = snapshot(source, cache_root, "cache-storage", "cache/entries")
            with cache.SimpleCacheFile(private) as entry:
                body, metadata = entry.get_stream_1(), entry.get_stream_0()
                if not entry._has_data:
                    raise ValueError("Cache entry has no response body")
                for data, eof in [(body, entry._stream_1_eof), (metadata, entry._stream_0_eof)]:
                    if eof.has_crc and zlib.crc32(data) != eof.data_crc:
                        raise ValueError("Cache stream CRC differs")
                if entry._stream_0_eof.has_key_sha256:
                    position = len(raw) - cache.SIMPLE_EOF_SIZE - 32
                    if raw[position:position + 32] != hashlib.sha256(key.encode("latin-1")).digest():
                        raise ValueError("Cache key SHA-256 differs")
                offset = entry.data_start_offset
                if raw[offset:offset + len(body)] != body:
                    raise ValueError("Cache response span differs from original bytes")
            identity = hashlib.sha256(body).hexdigest()
            name = Path(urllib.parse.unquote(url.path)).name or "index.html"
            if re.search(r'[\\:*?"<>|]', name) or name in {".", ".."}:
                raise ValueError("Unsafe cached resource filename")
            resource = output / "cache/resources" / identity[:12] / name
            resource.parent.mkdir(parents=True, exist_ok=True)
            if resource.exists() and a.sha(resource) != identity:
                raise ValueError("Cached resource identity collision")
            resource.write_bytes(body)
            row = {"path": a.rel(resource), "bytes": len(body), "sha256": identity,
                   "kind": "installed_cached_official_resource", "url": key,
                   "canonical_url": urllib.parse.urlunsplit((url.scheme, url.netloc, url.path, "", "")),
                   "acquired_at_utc": original["acquired_at_utc"],
                   "container": original["path"], "container_sha256": original["sha256"],
                   "body_offset": offset, "body_bytes": len(body), "cache_crc_verified": True,
                   "deployment_current_verified": False}
            resources.append(row)
            match = re.match(r"/synapse/products/(\d+)/(ui|mw)/", url.path)
            group = f"installed-product-{match[1]}-{match[2]}" if match else None
            for prefix, label in [("/synapse/dashboard/", "installed-synapse-dashboard"),
                                  ("/chroma-app/dashboard/", "installed-chroma-dashboard")]:
                if url.path.startswith(prefix):
                    group = label
            if group:
                grouped.setdefault(group, []).append(row)
        except (OSError, ValueError, struct.error) as error:
            failure("cache-storage", source, cache_root, "cache_static_extraction", error)
    report["excluded_nonofficial_cache_entries"] = excluded
    report["excluded_installed_files"] = {
        "source_root": "services", "by_extension": excluded_installed,
        "policy": "only executable code and explicit .NET deployment manifests; logs, databases, configuration and activity data excluded before reading",
    }
    report["frontend_groups"] = []
    shared = [row for row in resources if urllib.parse.urlsplit(row["url"]).path.startswith("/synapse/assets/")]
    for label, rows in grouped.items():
        destination = a.ROOT / ".analysis/inputs/frontend" / label
        by_url = {}
        for row in [*rows, *shared]:
            by_url.setdefault(row["canonical_url"], {})[row["sha256"]] = row
        artifacts, conflicts = [], []
        for canonical, variants in sorted(by_url.items()):
            if len(variants) != 1:
                conflicts.append({"kind": "cached_resource_versions_conflict", "url": canonical,
                                  "variants": list(variants.values())})
                continue
            row = next(iter(variants.values()))
            parts = urllib.parse.urlsplit(canonical)
            path = destination / "raw" / parts.netloc / parts.path.lstrip("/")
            if not path.resolve().is_relative_to((destination / "raw").resolve()):
                raise ValueError("Cached frontend resource path escaped")
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((a.ROOT / row["path"]).read_bytes())
            artifacts.append({**row, "path": a.rel(path), "url": canonical, "cached_url": row["url"]})
        acquisition = {"schema_version": 1, "acquired_at_utc": a.stamp(), "files": artifacts,
                       "unresolved": [*conflicts, {"kind": "installed_cache_deployment_audit_pending",
                                                  "scope": "compare cached manifests/entry bytes with current official deployment; a cache is not proof of current release or dependency closure"}],
                       "source_current_during_acquisition": False, "declared_references_fetched": False,
                       "target_execution": False, "source": a.rel(output / "acquisition.json")}
        receipt = destination / "acquisition.json"
        a.save(receipt, acquisition)
        a.index_files([*artifacts, {"path": a.rel(receipt), "bytes": receipt.stat().st_size,
                                   "sha256": a.sha(receipt), "kind": "acquisition_receipt"}])
        report["frontend_groups"].append({"input": a.rel(destination), "files": len(artifacts), "conflicts": len(conflicts)})
    report["finished_at_utc"] = a.stamp()
    report["summary"] = {"installed_files": sum(row["source_root"] != "cache-storage" for row in files),
                         "cache_containers": sum(row["source_root"] == "cache-storage" for row in files),
                         "cached_resources": len(resources),
                         "failures": len(failures), "frontend_groups": len(grouped)}
    receipt = output / "acquisition.json"
    a.save(receipt, report)
    a.index_files([*files, *resources, {"path": a.rel(receipt), "bytes": receipt.stat().st_size,
                                       "sha256": a.sha(receipt), "kind": "acquisition_receipt"}])
    print(json.dumps({"installed_acquisition": a.rel(receipt), **report["summary"]}), flush=True)


def repair_host_metadata_index():
    import analysis as a
    source_name = ".analysis/results/host-current/metadata-recheck.json"
    destination_name = ".analysis/inputs/host-current/metadata-recheck.json"
    source = a.resolve(source_name, ".analysis/results/host-current")
    destination = a.resolve(destination_name, ".analysis/inputs/host-current")
    index_path = a.ROOT / ".analysis/inputs.json"
    with a.index_lock():
        index_raw = index_path.read_bytes()
        index = json.loads(index_raw)
        originals = [row for row in index["inputs"] if row["path"] == source_name]
        retained = [row for row in index["inputs"] if row["path"] == destination_name]
        if len(originals) > 1 or len(retained) > 1:
            raise ValueError("Duplicate host metadata receipt index entries")
        if not originals:
            if retained and (destination.stat().st_size != retained[0]["bytes"]
                             or a.sha(destination) != retained[0]["sha256"]):
                raise ValueError("Retained host metadata receipt differs from its index")
            return {"repaired": False, "source": source_name, "destination": destination_name,
                    "reason": "no_misplaced_receipt_index_entry"}
        original = originals[0]
        if original.get("kind") != "deployment_metadata_recheck":
            raise ValueError("Unexpected metadata receipt index kind")
        raw = source.read_bytes()
        identity = hashlib.sha256(raw).hexdigest()
        if len(raw) != original["bytes"] or identity != original["sha256"]:
            raise ValueError("Misplaced metadata receipt differs from its retained identity")
        replacement = {**original, "path": destination_name}
        if retained and retained[0] != replacement:
            raise ValueError("Conflicting host metadata receipt index destination")
        if destination.exists():
            if destination.read_bytes() != raw:
                raise ValueError("Conflicting host metadata receipt destination bytes")
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("xb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
        if destination.read_bytes() != raw or source.read_bytes() != raw:
            raise ValueError("Metadata receipt changed during index migration")
        proof = {"kind": "host_metadata_receipt_boundary_repair", "at_utc": a.stamp(),
                 "source": source_name, "destination": destination_name,
                 "source_preserved": True, "bytes": len(raw), "sha256": identity,
                 "original_index_entry": original,
                 "index_before_sha256": hashlib.sha256(index_raw).hexdigest(),
                 "tool": {"path": "tools/host.py", "sha256": a.sha(Path(__file__))}}
        index["inputs"] = sorted([row for row in index["inputs"]
                                  if row["path"] not in {source_name, destination_name}] + [replacement],
                                 key=lambda row: row["path"])
        index.setdefault("migrations", []).append(proof)
        a.save(index_path, index)
        return {"repaired": True, **proof}


def recheck_host_metadata(fetch, results):
    import analysis as a
    repair_host_metadata_index()
    baseline_path = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    baseline_raw = baseline_path.read_bytes()
    baseline = json.loads(baseline_raw)
    kinds = {"official_host_release_manifest", "official_host_resource_manifest",
             "official_host_discovery", "official_host_product_release"}
    previous = {row["url"]: row for row in baseline["files"] if row["kind"] in kinds}
    if {row["kind"] for row in previous.values()} != kinds:
        raise ValueError("Host acquisition lacks complete baseline deployment metadata")
    for row in previous.values():
        path = a.ROOT / row["path"]
        if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            raise ValueError("Baseline host metadata byte identity differs: " + row["path"])
    rows, failures, comparisons, stability = [], [], [], []
    report = {"schema_version": 1, "started_at_utc": a.stamp(), "files": rows,
              "failures": failures, "comparisons": comparisons, "stability": stability,
              "baseline": {"path": a.rel(baseline_path), "sha256": hashlib.sha256(baseline_raw).hexdigest()},
              "tool": {"path": "tools/host.py", "sha256": a.sha(Path(__file__))},
              "scope": "Official deployment JSON only; no package downloads, target execution or dependency-closure claim",
              "metadata_only": True, "target_execution": False, "real_device_operations": False,
              "host_package_closure_complete": False}
    pending = [(url, row["kind"]) for url, row in previous.items()]
    visited, documents = {}, {}
    current_products_url = None
    while pending:
        url, kind = pending.pop(0)
        if url in visited:
            continue
        visited[url] = None
        try:
            raw, row = fetch(url, kind=kind)
            rows.append(row)
            document = json.loads(raw)
            if not isinstance(document, dict):
                raise ValueError("Deployment metadata is not a JSON object")
            documents[url] = document
            visited[url] = row
            old = previous.get(url)
            comparisons.append({"url": url, "before_sha256": old["sha256"] if old else None,
                                "after_sha256": row["sha256"], "unchanged": old is not None and old["sha256"] == row["sha256"]})
            if kind == "official_host_release_manifest":
                latest_url = "https://apps.razer.com/" + document["baseURL"].strip("/") + "/" + document["latest"]["url"]
                if not urllib.parse.urlsplit(latest_url).path.endswith(".json"):
                    raise ValueError("Latest resource manifest is not a JSON endpoint")
                pending.append((latest_url, "official_host_resource_manifest"))
            elif kind == "official_host_discovery":
                endpoint = next(entry for entry in document["data"]["items"] if entry["name"] == "prod")
                if not re.fullmatch(r"[A-Za-z0-9]+", endpoint["hash"]):
                    raise ValueError("Unexpected production endpoint hash")
                query = urllib.parse.urlencode({"os": "WINDOWS", "osver": "11", "arch": "64", "mfr": "Generic-MFR",
                                               "model": "Generic-MDL0", "sku": "Generic-SKU", "l": "en-US"})
                current_products_url = f"https://manifest3.razerapi.com/api/v1/releases/{endpoint['hash']}/tags/prod/products?{query}"
                pending.append((current_products_url, "official_host_product_release"))
        except Exception as error:
            failures.append({"stage": "metadata", "url": url, "error_kind": type(error).__name__})
    current_products = documents.get(current_products_url, {})
    products = [entry for entry in current_products.get("data", {}).get("items", []) if entry["name"] in {"Anne", "Chroma"}]
    if {entry["name"] for entry in products} != {"Anne", "Chroma"}:
        failures.append({"stage": "stable_products", "error_kind": "missing_current_release_entries"})
    report["stable_products"] = products
    report["release_profile"] = baseline.get("release_profile")
    report["release_source"] = baseline.get("release_source")
    for url, before in visited.items():
        if before is None:
            continue
        try:
            raw, after = fetch(url, kind=before["kind"])
            rows.append(after)
            json.loads(raw)
            stability.append({"url": url, "before_sha256": before["sha256"], "after_sha256": after["sha256"],
                              "unchanged": before["sha256"] == after["sha256"]})
        except Exception as error:
            failures.append({"stage": "stability", "url": url, "error_kind": type(error).__name__})
    report["finished_at_utc"] = a.stamp()
    report["deployment_metadata_stable"] = not failures and bool(stability) and all(row["unchanged"] for row in stability)
    report["matches_previous_metadata"] = not failures and bool(comparisons) and all(row["unchanged"] for row in comparisons)
    receipt = a.ROOT / ".analysis/inputs/host-current/metadata-recheck.json"
    a.save(receipt, report)
    a.save(results / "metadata-recheck.json", report)
    a.index_files([{key: row[key] for key in ("path", "bytes", "sha256", "kind", "url")} for row in rows] + [
        {"path": a.rel(receipt), "bytes": receipt.stat().st_size, "sha256": a.sha(receipt), "kind": "deployment_metadata_recheck"}])
    print(json.dumps({"receipt": a.rel(receipt), "metadata_endpoints": len(visited), "failures": len(failures),
                      "deployment_metadata_stable": report["deployment_metadata_stable"],
                      "matches_previous_metadata": report["matches_previous_metadata"],
                      "versions": [{"name": entry["name"], "version": entry["version"]} for entry in products]}), flush=True)
    if not report["deployment_metadata_stable"]:
        raise SystemExit(1)


def recheck_host_bootstrap(fetch, results):
    """Recheck the current official download entry as inert archive evidence."""
    import analysis as a
    receipt = a.ROOT / ".analysis/inputs/host-current/bootstrap-recheck.json"
    previous_raw = receipt.read_bytes() if receipt.is_file() else None
    previous = json.loads(previous_raw) if previous_raw is not None else {}
    rows, failures = [], []
    report = {"schema_version": 1, "started_at_utc": a.stamp(), "files": rows, "failures": failures,
              "tool": {"path": "tools/host.py", "sha256": a.sha(Path(__file__))},
              "scope": "Current official download page and bootstrap bytes only; static inspection, dependencies and installer lifecycle remain separate",
              "target_execution": False, "real_device_operations": False, "host_package_closure_complete": False}
    if previous_raw is not None:
        report["previous_receipt"] = {"bytes": len(previous_raw), "sha256": hashlib.sha256(previous_raw).hexdigest()}
        report["previous_observations"] = list(previous.get("previous_observations", [])) + [
            {key: value for key, value in previous.items() if key not in {"previous_observations", "previous_receipt"}}]
    samples = []
    for phase in ("before", "after"):
        sample = {}
        samples.append(sample)
        try:
            raw, page = fetch(STABLE_DOWNLOAD_PAGE, kind="official_stable_download_page")
            rows.append(page)
            sample[STABLE_DOWNLOAD_PAGE] = page
            if STABLE_BOOTSTRAP not in raw.decode("utf-8", errors="strict"):
                raise ValueError("Stable download entry absent from the current official page")
            raw, bootstrap = fetch(STABLE_BOOTSTRAP, kind="official_stable_bootstrap_installer")
            rows.append(bootstrap)
            sample[STABLE_BOOTSTRAP] = bootstrap
            if raw[:2] != b"MZ":
                raise ValueError("Official bootstrap response is not a PE candidate")
        except Exception as error:
            failures.append({"stage": phase, "error_kind": type(error).__name__, "error": str(error)})
    report["stability"] = []
    for url in (STABLE_DOWNLOAD_PAGE, STABLE_BOOTSTRAP):
        before, after = (sample.get(url) for sample in samples)
        report["stability"].append({"url": url, "before_sha256": before.get("sha256") if before else None,
            "after_sha256": after.get("sha256") if after else None,
            "unchanged": bool(before and after and before["bytes"] == after["bytes"]
                              and before["sha256"] == after["sha256"] and before["final_url"] == after["final_url"])})
    baseline = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    if baseline.is_file():
        raw = baseline.read_bytes()
        report["baseline"] = {"path": a.rel(baseline), "sha256": hashlib.sha256(raw).hexdigest()}
        known = [row for row in json.loads(raw).get("files", []) if row.get("kind") == "official_stable_bootstrap_installer"]
        current = samples[-1].get(STABLE_BOOTSTRAP)
        report["matches_previous_bootstrap"] = bool(current and any(all(row.get(key) == current[key]
            for key in ("url", "bytes", "sha256")) for row in known))
    report["finished_at_utc"] = a.stamp()
    report["bootstrap_stable"] = not failures and all(row["unchanged"] for row in report["stability"])
    a.save(receipt, report)
    a.save(results / "bootstrap-recheck.json", report)
    a.index_files([{key: row[key] for key in ("path", "bytes", "sha256", "kind", "url")} for row in rows] + [
        {"path": a.rel(receipt), "bytes": receipt.stat().st_size, "sha256": a.sha(receipt), "kind": "official_bootstrap_recheck"}])
    print(json.dumps({"receipt": a.rel(receipt), "files": len(rows), "failures": len(failures),
                      "bootstrap_stable": report["bootstrap_stable"],
                      "matches_previous_bootstrap": report.get("matches_previous_bootstrap")}), flush=True)
    if not report["bootstrap_stable"]:
        raise SystemExit(1)
    return report


def acquire_host(args):
    import analysis as a
    if getattr(args, "repair_metadata_index", False):
        if any(getattr(args, name, False) for name in ("metadata_only", "release", "extract", "inspect", "revalidate_bootstrap")):
            raise ValueError("Metadata index repair is local-only and cannot be combined with acquisition options")
        print(json.dumps(repair_host_metadata_index()), flush=True)
        return
    if getattr(args, "revalidate_bootstrap", False) and any(getattr(args, name, False)
            for name in ("metadata_only", "release", "extract", "inspect")):
        raise ValueError("Bootstrap revalidation cannot be combined with other acquisition modes")
    output = a.ROOT / ".analysis/inputs/host-current"
    results = a.ROOT / ".analysis/results/host-current"
    output.mkdir(parents=True, exist_ok=True)
    results.mkdir(parents=True, exist_ok=True)
    rows = []
    failures = []

    class OfficialRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):
            p = urllib.parse.urlsplit(newurl)
            if p.scheme != "https" or p.netloc not in OFFICIAL:
                raise ValueError("Nonofficial redirect: " + newurl)
            return super().redirect_request(req, fp, code, msg, headers, newurl)

    opener = urllib.request.build_opener(OfficialRedirect)

    def fetch(url, expected=None, kind="official_host_artifact"):
        p = urllib.parse.urlsplit(url)
        if p.scheme != "https" or p.netloc not in OFFICIAL:
            raise ValueError("Nonofficial input URL: " + url)
        headers = {"User-Agent": "razer-ui-static-evidence", "Accept-Encoding": "identity",
                   "Cache-Control": "no-cache", "Pragma": "no-cache"}
        request = urllib.request.Request(url, headers=headers)
        with opener.open(request, timeout=90) as response:
            raw = response.read()
            final = urllib.parse.urlsplit(response.geturl())
            if final.scheme != "https" or final.netloc not in OFFICIAL or response.status != 200:
                raise ValueError("Host evidence response is not an official HTTPS success")
            row = {"url": url, "final_url": response.geturl(), "http_status": response.status,
                   "headers": dict(response.headers), "request_headers": headers, "acquired_at_utc": a.stamp(),
                   "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), "kind": kind}
        if expected:
            if len(raw) != expected["size"] or row["sha256"] != expected["sha256"].lower():
                raise ValueError("Resource byte identity differs from manifest: " + url)
            if expected.get("integrity") != "sha256-" + base64.b64encode(bytes.fromhex(row["sha256"])).decode():
                raise ValueError("Resource integrity differs: " + url)
            row["declaration"] = expected
        advertised = row["headers"].get("x-amz-meta-checksum-sha256") or row["headers"].get("x-amz-meta-sha256")
        if advertised and advertised.lower() != row["sha256"]:
            raise ValueError("Server SHA-256 header differs: " + url)
        filename = Path(urllib.parse.urlsplit(row["final_url"]).path).name or "index.html"
        if re.search(r'[\\:*?"<>|]', filename):
            raise ValueError("Unsafe resource filename")
        path = output / "blobs" / row["sha256"] / filename
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.is_file():
            if path.stat().st_size != len(raw) or a.sha(path) != row["sha256"]:
                raise ValueError("Existing host evidence blob differs from its content address")
        else:
            path.write_bytes(raw)
        row["path"] = a.rel(path)
        return raw, row

    if getattr(args, "metadata_only", False):
        return recheck_host_metadata(fetch, results)
    if getattr(args, "revalidate_bootstrap", False):
        return recheck_host_bootstrap(fetch, results)

    data = {"schema_version": 1, "started_at_utc": a.stamp(), "files": rows, "failures": failures,
            "target_execution": False, "real_device_operations": False,
            "scope": "official stable download entry and source-declared host resource manifests; full host/device closure pending"}
    receipt = output / "acquisition.json"
    page_raw, page = fetch(STABLE_DOWNLOAD_PAGE, kind="official_stable_download_page")
    rows.append(page)
    stable_url = STABLE_BOOTSTRAP
    if stable_url not in page_raw.decode("utf-8", errors="replace"):
        raise ValueError("Stable download entry absent from current official page")
    _, installer = fetch(stable_url, kind="official_stable_bootstrap_installer")
    rows.append(installer)
    manifests = []
    for location in ["background-manager", "synapse/dashboard"]:
        try:
            raw, index = fetch(f"https://apps.razer.com/{location}/installer-manifest.json", kind="official_host_release_manifest")
            rows.append(index)
            index_data = json.loads(raw)
            url = "https://apps.razer.com/" + index_data["baseURL"].strip("/") + "/" + index_data["latest"]["url"]
            raw, manifest = fetch(url, kind="official_host_resource_manifest")
            rows.append(manifest)
            manifest["release"] = index_data["latest"]
            manifest["release_manifest"] = index["path"]
            manifests.append((index, manifest, json.loads(raw)))
        except Exception as exc:
            failures.append({"stage": "manifest", "location": location, "error": str(exc)})
    resources = {}
    for index, _, manifest in manifests:
        for item in manifest.get("resources", []):
            resources.setdefault(item["url"], (item, []))[1].append(index["path"])

    def resource(task):
        item, parents = task
        try:
            _, row = fetch(item["url"], item)
            row["release_manifests"] = parents
            return row
        except Exception as exc:
            return {"url": item["url"], "stage": "resource", "error": str(exc)}

    a.save(receipt, data)
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        for row in pool.map(resource, resources.values()):
            (failures if "error" in row else rows).append(row)
            a.save(receipt, data)
            print(json.dumps({"host_files": len(rows), "failures": len(failures)}), flush=True)
    data["stability"] = []
    for index in [item for group in manifests for item in group[:2]]:
        try:
            _, after = fetch(index["url"], kind=index["kind"])
            data["stability"].append({"url": index["url"], "before_sha256": index["sha256"],
                                      "after_sha256": after["sha256"], "unchanged": index["sha256"] == after["sha256"]})
        except Exception as exc:
            data["stability"].append({"url": index["url"], "unchanged": False, "error": str(exc)})
    if getattr(args, "release", False):
        # Proven by complete ILSpy bodies: Config, ManifestDownloader.GetManifestFromWebAsync,
        # GetEndpointHashAsync, EnvironmentVar and RazerLaptopInfoHelper.GetDeviceInfo.
        # Generic profile deliberately avoids exporting workstation hardware identity.
        try:
            raw, discovery = fetch("https://discovery3.razerapi.com/api/v1/endpoints", kind="official_host_discovery")
            rows.append(discovery)
            endpoint = next(x for x in json.loads(raw)["data"]["items"] if x["name"] == "prod")
            if not re.fullmatch(r"[A-Za-z0-9]+", endpoint["hash"]):
                raise ValueError("Unexpected production endpoint hash")
            query = urllib.parse.urlencode({"os": "WINDOWS", "osver": "11", "arch": "64", "mfr": "Generic-MFR",
                                           "model": "Generic-MDL0", "sku": "Generic-SKU", "l": "en-US"})
            url = f"https://manifest3.razerapi.com/api/v1/releases/{endpoint['hash']}/tags/prod/products?{query}"
            raw, release = fetch(url, kind="official_host_product_release")
            rows.append(release)
            apps = [x for x in json.loads(raw)["data"]["items"] if x["name"] in {"Anne", "Chroma"}]
            if {x["name"] for x in apps} != {"Anne", "Chroma"}:
                raise ValueError("Stable Synapse/Chroma release entries absent")
            data["stable_products"] = apps
            data["release_profile"] = {"os": "WINDOWS", "osver": "11", "arch": "64", "hardware": "source-defined generic fallback", "locale": "en-US"}
            data["release_source"] = ".analysis/results/host-current/managed/bdcb81a0545a/source/Razer/Installer/Core/Downloader/ManifestDownloader.cs"
            packages = {item["download_url"]: item for app in apps for item in app["modules"]}
            for url, item in packages.items():
                raw, package = fetch(url, kind="official_stable_host_package")
                actual_md5 = hashlib.md5(raw).hexdigest()
                package["declaration"] = item
                package["release_manifest"] = release["path"]
                package["md5"] = actual_md5
                package["manifest_identity_verified"] = len(raw) == item["file_size"] and actual_md5.lower() == item["download_file_checksum"].lower()
                rows.append(package)
                if not package["manifest_identity_verified"]:
                    failures.append({"stage": "package_identity", "path": package["path"], "error": "Downloaded package differs from source-defined length/MD5 contract"})
                a.save(receipt, data)
                print(json.dumps({"host_package": item["original_file_name"], "bytes": len(raw), "manifest_identity_verified": package["manifest_identity_verified"]}), flush=True)
            for before in [discovery, release]:
                _, after = fetch(before["url"], kind=before["kind"])
                data["stability"].append({"url": before["url"], "before_sha256": before["sha256"], "after_sha256": after["sha256"],
                                          "unchanged": before["sha256"] == after["sha256"]})
        except Exception as exc:
            failures.append({"stage": "stable_release", "error": str(exc)})
    data["finished_at_utc"] = a.stamp()
    data["declared_host_resources_verified"] = len(manifests) == 2 and bool(resources) and not failures and all(x["unchanged"] for x in data["stability"])
    data["host_package_closure_complete"] = False
    a.save(receipt, data)
    a.index_files([{k: row[k] for k in ("path", "bytes", "sha256", "kind", "url")} for row in rows] + [
        {"path": a.rel(receipt), "bytes": receipt.stat().st_size, "sha256": a.sha(receipt), "kind": "acquisition_receipt"}])
    print(json.dumps({"receipt": a.rel(receipt), "files": len(rows), "failures": len(failures)}), flush=True)
    if getattr(args, "inspect", False):
        inspect_host(args)


def retained_host_archive_integrity(tool, source, output, inno=False):
    """Test archive data with the static analyzer, never execute the installer."""
    import analysis as a
    argv = ([str(tool), "-t", "-b", "-q", "-m", "-a", "-o", "-u", str(source)] if inno else
            [str(tool), "t", "-bd", str(source)])
    before = {"bytes": source.stat().st_size, "sha256": a.sha(source)}
    process = subprocess.run(argv, capture_output=True, check=False)
    stdout, stderr = output / "retained-integrity.stdout.txt", output / "retained-integrity.stderr.txt"
    stdout.write_bytes(process.stdout)
    stderr.write_bytes(process.stderr)
    unchanged = source.stat().st_size == before["bytes"] and a.sha(source) == before["sha256"]
    return {"exit_code": process.returncode, "input_bytes_unchanged": unchanged,
            "input": {"path": a.rel(source), **before},
            "tool": {"name": "innounp" if inno else "7-Zip", "sha256": a.sha(tool)},
            "argv": [a.rel(tool) if inno else "7-Zip", *argv[1:-1], a.rel(source)],
            "outputs": [{"path": a.rel(path), "bytes": path.stat().st_size, "sha256": a.sha(path)}
                        for path in (stdout, stderr)],
            "target_execution": False, "scope": "Static archive integrity test; no installer actions"}


def repair_host_extraction(args):
    """Re-extract to scratch, verify every retained identity, then restore missing originals."""
    import analysis as a

    if not args.input:
        raise ValueError("Static extraction repair requires one explicit indexed input")
    source = a.resolve(args.input, ".analysis/inputs/host-current")
    index = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
    source_relative = a.rel(source)
    row = next(r for r in index["inputs"] if r["path"] == source_relative)
    if source.stat().st_size != row["bytes"] or a.sha(source) != row["sha256"]:
        raise ValueError("Host repair source identity differs")
    tool = Path(args.seven_zip).resolve()
    output = a.ROOT / ".analysis/results/host-current" / row["sha256"] / "restoration"
    output.mkdir(parents=True, exist_ok=True)
    destination = a.ROOT / ".analysis/inputs/host-current/extracted" / row["sha256"]
    destination_prefix = a.rel(destination) + "/"
    expected = {r["path"]: r for r in index["inputs"] if r["path"].startswith(destination_prefix)}
    inspection_path = a.ROOT / ".analysis/results/host-current/inspection.json"
    inspection = json.loads(inspection_path.read_bytes())
    prior = next(r for r in inspection["containers"] if r["sha256"] == row["sha256"])
    record = restore_static_archive(row, tool, destination, expected, prior, output, "static_extracted_host_input")
    replacement = {**prior, "files": record["files"], "extracted": True,
                   "extraction_exit_code": record["extraction_exit_code"],
                   "restoration": a.rel(output / "capture.json"), "restoration_tool": record["tool"]}
    inspection["containers"] = [replacement if r["sha256"] == row["sha256"] else r for r in inspection["containers"]]
    a.save(inspection_path, inspection)
    a.index_files(record["files"])
    print(json.dumps({"restoration": a.rel(output / "capture.json"), "files": len(record["files"]), "restored": len(record["restored"])}), flush=True)


def restore_static_archive(row, tool, destination, expected, prior, output, kind):
    """Preserve old receipts and promote fresh static archive bytes only after identity checks."""
    import analysis as a
    import shutil
    import tempfile

    source = a.resolve(row["path"], ".analysis/inputs")
    if source.stat().st_size != row["bytes"] or a.sha(source) != row["sha256"]:
        raise ValueError("Archive restoration source identity differs")
    output.mkdir(parents=True, exist_ok=True)
    def identity(path):
        return {"path": a.rel(path), "bytes": path.stat().st_size, "sha256": a.sha(path)}
    record = {"recorded_at_utc": a.stamp(), "input": row, "target_execution": False,
              "tool": {"sha256": a.sha(tool), "library_sha256": a.sha(tool.with_name("7z.dll"))},
              "previous_container": prior, "files": [], "restored": [], "complete": False}
    a.save(output / "capture.json", record)
    try:
        listing = subprocess.run([str(tool), "l", "-slt", str(source)], capture_output=True, check=False)
        (output / "listing.stdout.txt").write_bytes(listing.stdout)
        (output / "listing.stderr.txt").write_bytes(listing.stderr)
        record["listing_exit_code"] = listing.returncode
        text = listing.stdout.decode("utf-8", errors="replace").replace("\r", "")
        if listing.returncode or "BadCmd=" in text or re.search(r"^(?:Symbolic Link|Hard Link) =", text, re.M):
            raise ValueError("Repair requires a complete static archive listing without unresolved commands or links")
        payload = text.split("----------\n", 1)
        if len(payload) != 2:
            raise ValueError("No static archive entry listing")
        names = re.findall(r"^Path = (.+)$", payload[1], re.M)
        record["archive_entries"] = names
        for name in names:
            if name.startswith(("/", "\\")) or any(p in {".", ".."} or ":" in p for p in name.replace("\\", "/").split("/")):
                raise ValueError("Unsafe static archive entry")
        with tempfile.TemporaryDirectory(prefix="static-restore-", dir=a.ROOT / ".work") as scratch:
            scratch = Path(scratch)
            process = subprocess.run([str(tool), "x", "-y", "-aou", "-o" + str(scratch), str(source)], capture_output=True, check=False)
            (output / "extraction.stdout.txt").write_bytes(process.stdout)
            (output / "extraction.stderr.txt").write_bytes(process.stderr)
            record["extraction_exit_code"] = process.returncode
            if process.returncode or source.stat().st_size != row["bytes"] or a.sha(source) != row["sha256"]:
                raise ValueError("Static repair extraction failed or source changed")
            originals = {}
            for item in scratch.rglob("*"):
                if item.is_symlink() or item.is_junction() or not item.resolve().is_relative_to(scratch.resolve()):
                    raise ValueError("Static extraction escaped scratch")
                if item.is_file():
                    originals[a.rel(destination / item.relative_to(scratch))] = item
            if not set(expected).issubset(originals):
                raise ValueError("Static extraction still misses indexed originals: " + str(sorted(set(expected) - set(originals))))
            for target, item in originals.items():
                actual = {"bytes": item.stat().st_size, "sha256": a.sha(item)}
                if target in expected and any(expected[target][k] != actual[k] for k in actual):
                    raise ValueError("Fresh extraction differs from indexed original: " + target)
                local = a.ROOT / target
                if local.exists() and (not local.is_file() or local.stat().st_size != actual["bytes"] or a.sha(local) != actual["sha256"]):
                    raise ValueError("Fresh extraction differs from retained bytes: " + target)
                record["files"].append({**expected.get(target, {}), "path": target, **actual,
                    "kind": kind, "container": row["path"], "container_sha256": row["sha256"]})
            for target, item in originals.items():
                local = a.ROOT / target
                if not local.exists():
                    local.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(item, local)
                    record["restored"].append(target)
            record["complete"] = True
    except Exception as exc:
        record["error"] = str(exc)
        raise
    finally:
        record["outputs"] = [identity(p) for p in sorted(output.glob("*.txt"))]
        a.save(output / "capture.json", record)
    return record


def setup_factory_static_extraction(raw):
    """Decode the observed single-segment LZMA container without running it.

    The launcher owns the marker, 64-bit sizes and first-2000-byte XOR. The
    setup engine owns the 280-byte auxiliary headers and sequential payloads.
    Both complete function sets are retained by the local installation audit.
    This accepts only those observed branches; another compression/segment
    format remains an explicit acquisition failure.
    """
    marker = bytes(value for value in range(0xe0, 0xe8) for _ in range(2))
    hits = [match.start() for match in re.finditer(re.escape(marker), raw)]
    if not hits:
        return None
    if len(hits) != 1:
        raise ValueError("Setup Factory data marker is ambiguous")
    inspect_pe(raw)
    start = hits[0]
    if start + 34 > len(raw):
        raise ValueError("Setup Factory header exceeds input")
    multi, compression, end, engine_size = struct.unpack_from("<BBQQ", raw, start + 16)
    if multi != 0 or compression != 1:
        raise ValueError("Unresolved Setup Factory segment/compression branch")
    if not start + 34 < end <= len(raw):
        raise ValueError("Setup Factory total size exceeds input")
    pe = struct.unpack_from("<I", raw, 0x3c)[0]
    optional = pe + 24
    magic = struct.unpack_from("<H", raw, optional)[0]
    directory = optional + (112 if magic == 0x20b else 96)
    cert_offset, cert_bytes = struct.unpack_from("<II", raw, directory + 4 * 8)
    if cert_offset or cert_bytes:
        if (cert_offset != (end + 7) & ~7 or not cert_bytes
                or cert_offset + cert_bytes != len(raw) or any(raw[end:cert_offset])):
            raise ValueError("Setup Factory payload/certificate boundary differs")
        cursor = cert_offset
        while cursor < len(raw):
            if cursor + 8 > len(raw):
                raise ValueError("Truncated Setup Factory certificate")
            length, revision, kind = struct.unpack_from("<IHH", raw, cursor)
            if length < 8 or cursor + length > len(raw) or revision != 0x200 or kind != 2:
                raise ValueError("Invalid Setup Factory certificate boundary")
            aligned = (cursor + length + 7) & ~7
            if aligned > len(raw) or any(raw[cursor + length:aligned]):
                raise ValueError("Invalid Setup Factory certificate padding")
            cursor = aligned
    elif end != len(raw):
        raise ValueError("Unexpected bytes after Setup Factory payload")
    files, names = [], set()
    cursor = start + 34

    def retain(name, offset, compressed, data, transform):
        parts = name.replace("\\", "/").split("/")
        if (not name or name.startswith(("/", "\\"))
                or any(not part or part in {".", ".."} or ":" in part for part in parts)
                or name.casefold() in names):
            raise ValueError("Unsafe or duplicate Setup Factory output name")
        names.add(name.casefold())
        files.append({"name": name, "container_offset": offset, "container_bytes": len(compressed),
                      "container_sha256": hashlib.sha256(compressed).hexdigest(),
                      "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                      "transform": transform, "data": bytes(data)})

    def bounded(size):
        nonlocal cursor
        if not 0 < size <= end - cursor:
            raise ValueError("Setup Factory component size exceeds input")
        offset = cursor
        cursor += size
        return offset, raw[offset:cursor]

    offset, encoded = bounded(engine_size)
    engine = bytearray(encoded)
    for index in range(min(2000, len(engine))):
        engine[index] ^= 7
    if inspect_pe(engine).get("kind") != "native":
        raise ValueError("Setup Factory engine is not the declared PE")
    retain("engine/irsetup.exe", offset, encoded, engine, "xor_7_first_2000_bytes")
    if cursor + 8 > end:
        raise ValueError("Missing Setup Factory Lua length")
    lua_size = struct.unpack_from("<Q", raw, cursor)[0]
    cursor += 8
    offset, lua = bounded(lua_size)
    if inspect_pe(lua).get("kind") != "native":
        raise ValueError("Setup Factory Lua library is not a PE")
    retain("engine/lua5.3.dll", offset, lua, lua, "exact_range")
    if cursor + 4 > end:
        raise ValueError("Missing Setup Factory auxiliary count")
    count = struct.unpack_from("<I", raw, cursor)[0]
    cursor += 4
    if not 0 < count <= (end - cursor) // 280:
        raise ValueError("Invalid Setup Factory auxiliary count")

    def decode(encoded):
        if len(encoded) < 13:
            raise ValueError("Truncated Setup Factory LZMA header")
        size = struct.unpack_from("<Q", encoded, 5)[0]
        if not 0 < size <= 256 * 1024 * 1024:
            raise ValueError("Unresolved Setup Factory LZMA output length")
        decoder = lzma.LZMADecompressor(format=lzma.FORMAT_ALONE, memlimit=512 * 1024 * 1024)
        try:
            data = decoder.decompress(encoded, max_length=size + 1)
        except lzma.LZMAError as error:
            raise ValueError("Invalid Setup Factory LZMA component") from error
        if not decoder.eof or len(data) != size:
            raise ValueError("Incomplete Setup Factory LZMA output")
        consumed = len(encoded) - len(decoder.unused_data)
        return data, consumed

    for _ in range(count):
        offset, header = bounded(280)
        name_bytes, attributes, length, timestamp = struct.unpack("<260sIQQ", header)
        name_raw, terminator, padding = name_bytes.partition(b"\0")
        if not terminator or any(padding):
            raise ValueError("Invalid Setup Factory auxiliary name padding")
        try:
            name = name_raw.decode("ascii")
        except UnicodeDecodeError as error:
            raise ValueError("Unresolved Setup Factory auxiliary name encoding") from error
        if "/" in name or "\\" in name:
            raise ValueError("Unsafe Setup Factory auxiliary basename")
        offset, encoded = bounded(length)
        data, consumed = decode(encoded)
        if consumed != length:
            raise ValueError("Trailing bytes in Setup Factory auxiliary component")
        retain("auxiliary/" + name, offset, encoded, data, "lzma_alone")
        files[-1].update(attributes=attributes, timestamp=timestamp)
    payloads = 0
    while cursor < end:
        offset = cursor
        data, consumed = decode(raw[cursor:end])
        cursor += consumed
        suffix = ".bin"
        if data[:2] == b"MZ":
            inspect_pe(data)
            image = struct.unpack_from("<I", data, 0x3c)[0]
            characteristics = struct.unpack_from("<H", data, image + 22)[0]
            suffix = ".dll" if characteristics & 0x2000 else ".exe"
        elif data[:4] == b"PK\x03\x04":
            with zipfile.ZipFile(io.BytesIO(data)) as archive:
                if archive.testzip() is not None:
                    raise ValueError("Setup Factory ZIP payload integrity failed")
            suffix = ".zip"
        elif data[:4] == b"\0\0\x01\0":
            suffix = ".ico"
        retain(f"payload/{payloads:04}{suffix}", offset, raw[offset:cursor], data, "lzma_alone")
        payloads += 1
    if not payloads:
        raise ValueError("Setup Factory contains no local file payload")
    return {"format": "Setup Factory", "marker_offset": start,
            "single_segment": True, "compression": "LZMA alone", "payload_end": end,
            "auxiliary_count": count, "payload_count": payloads, "files": files,
            "target_execution": False, "complete": cursor == end}


def inspect_host(args):
    if getattr(args, "repair_extraction", False):
        return repair_host_extraction(args)
    workers = getattr(args, "workers", 4)
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Host inspection workers must be between two and sixteen")
    import analysis as a
    receipt_path = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    receipt = json.loads(receipt_path.read_bytes())
    indexed = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
    installed_by_identity = {}
    for artifact in indexed["inputs"]:
        if artifact["path"].startswith(".analysis/inputs/installed-current/files/"):
            installed_by_identity.setdefault(artifact["sha256"], []).append(artifact)
    selected = getattr(args, "input", None)
    if selected:
        original = a.resolve(selected, ".analysis/inputs/host-current")
        original_relative = a.rel(original)
        row = next((row for row in indexed["inputs"] if row["path"] == original_relative), None)
        if row is None or original.stat().st_size != row["bytes"] or a.sha(original) != row["sha256"]:
            raise ValueError("Nested host container is not a verified indexed original")
        if original.suffix.lower() not in {".exe", ".msix", ".zip", ".7z", ".msi"}:
            raise ValueError("Expected a statically extractable host evidence container")
        rows = [row]
    else:
        rows = receipt["files"]
    results = a.ROOT / ".analysis/results/host-current"
    previous_path = results / "inspection.json"
    previous = json.loads(previous_path.read_bytes()) if previous_path.is_file() else {}
    previous_containers = {x["sha256"]: x for x in previous.get("containers", [])}
    tool = Path(getattr(args, "seven_zip", "C:/Program Files/7-Zip/7z.exe"))
    if not tool.is_file():
        raise ValueError("7-Zip static extractor is unavailable")
    version = subprocess.run([str(tool), "i"], capture_output=True, check=False)
    (results / "7zip-version.stdout.txt").write_bytes(version.stdout)
    (results / "7zip-version.stderr.txt").write_bytes(version.stderr)
    report = {"schema_version": 1, "recorded_at_utc": a.stamp(), "target_execution": False,
              "tool": {"name": "7-Zip", "sha256": a.sha(tool), "library_sha256": a.sha(tool.with_name("7z.dll")),
                       "version": version.stdout.decode("utf-8", errors="replace").strip().splitlines()[0],
                       "version_output": a.rel(results / "7zip-version.stdout.txt")},
              "acquisition": a.rel(receipt_path), "containers": []}
    if selected:
        report["selected_input"] = rows[0]
    # Container rows are independent: workers only write their content-addressed
    # output directories and return immutable records.  The coordinator below
    # performs the ordered receipt/index publication.
    inno_lock = threading.Lock()
    inno_artifact = None

    def inno_analyzer():
        nonlocal inno_artifact
        if inno_artifact is None:
            with inno_lock:
                if inno_artifact is None:
                    inno_artifact = prepare_inno_analyzer()
        return inno_artifact

    def inspect_row(row):
        row_extracted_rows = []
        path = a.ROOT / row["path"]
        if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            raise ValueError("Changed host input: " + row["path"])
        out = results / row["sha256"]
        out.mkdir(parents=True, exist_ok=True)
        listing = subprocess.run([str(tool), "l", "-slt", str(path)], capture_output=True, check=False)
        (out / "listing.txt").write_bytes(listing.stdout)
        (out / "listing.stderr.txt").write_bytes(listing.stderr)
        record = {"input": row["path"], "sha256": row["sha256"], "list_exit_code": listing.returncode,
                  "listing": a.rel(out / "listing.txt"), "extracted": False}
        text = listing.stdout.decode("utf-8", errors="replace").replace("\r\n", "\n")
        formats = re.findall(r"^Type = (.*)$", text, re.M)
        record["formats"] = formats
        inno = None
        setup = None
        if path.suffix.lower() == ".exe" and not any(f in {"zip", "Nsis", "7z"} for f in formats):
            setup = setup_factory_static_extraction(path.read_bytes())
            if setup is not None:
                formats.append("Setup Factory")
                record["setup_factory"] = {key: value for key, value in setup.items() if key != "files"}
                record["setup_factory"]["components"] = [
                    {key: value for key, value in component.items() if key != "data"}
                    for component in setup["files"]]
            marker = re.search(rb"Inno Setup Setup Data \(([^)\0]+)\)", path.read_bytes())
            if marker:
                inno, inno_receipt = inno_analyzer()
                argv = [str(inno), "-s", "-h", "-o", "-u", "-b", "-m", "-a", str(path)]
                inno_listing = subprocess.run(argv, capture_output=True, check=False)
                (out / "inno-listing.stdout.txt").write_bytes(inno_listing.stdout)
                (out / "inno-listing.stderr.txt").write_bytes(inno_listing.stderr)
                record["inno_setup"] = {"declared_setup_data_version": marker.group(1).decode("ascii"),
                                        "list_exit_code": inno_listing.returncode,
                                        "listing": a.rel(out / "inno-listing.stdout.txt"),
                                        "listing_stderr": a.rel(out / "inno-listing.stderr.txt"),
                                        "analyzer": {"name": inno_receipt["name"], "version": inno_receipt["version"],
                                                     "path": a.rel(inno), "sha256": a.sha(inno),
                                                     "acquisition": a.rel(inno.parent / "acquisition.json"),
                                                     "acquisition_sha256": a.sha(inno.parent / "acquisition.json")},
                                        "argv": [a.rel(inno), *argv[1:-1], row["path"]]}
                if inno_listing.returncode == 0:
                    formats.append("Inno Setup")
        supported = (setup is not None or listing.returncode == 0 and
                     any(f in {"zip", "Nsis", "7z", "Inno Setup"} for f in formats))
        if getattr(args, "extract", False) and supported:
            if setup:
                names = [component["name"] for component in setup["files"]]
            elif inno:
                names = inno_archive_entries(inno_listing.stdout)
                record["inno_setup"]["archive_entries"] = names
            else:
                entries = text.split("----------", 1)[-1]
                names = re.findall(r"^Path = (.*)$", entries, re.M)
            for name in names:
                parts = name.replace("\\", "/").split("/")
                if name.startswith(("/", "\\")) or any(part in {".", ".."} or ":" in part for part in parts):
                    raise ValueError("Unsafe archive entry: " + name)
            destination = a.ROOT / ".analysis/inputs/host-current/extracted" / row["sha256"]
            destination.mkdir(parents=True, exist_ok=True)
            if any(destination.iterdir()):
                prior = previous_containers.get(row["sha256"], {})
                if not prior.get("extracted") or not prior.get("files"):
                    raise ValueError("Existing extraction has no successful receipt; inspect partial output: " + a.rel(destination))
                for artifact in prior["files"] + prior.get("reconstructed_outputs", []):
                    local = a.ROOT / artifact["path"]
                    if not local.is_file() or local.stat().st_size != artifact["bytes"] or a.sha(local) != artifact["sha256"]:
                        raise ValueError("Retained extracted bytes changed: " + artifact["path"])
                if {a.rel(x) for x in destination.rglob("*") if x.is_file()} != {x["path"] for x in prior["files"]}:
                    raise ValueError("Retained extraction file set changed")
                # Reuse must retain the observed analyzer result. Older repeat
                # receipts dropped it; obtain a fresh static archive test in
                # that case instead of inventing a successful extraction exit.
                if "extraction_exit_code" in prior:
                    record["extraction_exit_code"] = prior["extraction_exit_code"]
                    if "extraction_argv" in prior:
                        record["extraction_argv"] = prior["extraction_argv"]
                else:
                    integrity = prior.get("retained_archive_integrity", {})
                    reusable = (integrity.get("exit_code") == 0 and integrity.get("input_bytes_unchanged") is True
                                and all(integrity.get("input", {}).get(key) == row[key] for key in ("path", "bytes", "sha256"))
                                and len(integrity.get("outputs", [])) == 2)
                    if reusable:
                        for artifact in integrity["outputs"]:
                            local = a.resolve(artifact["path"], ".analysis/results/host-current")
                            if not local.is_file() or local.stat().st_size != artifact["bytes"] or a.sha(local) != artifact["sha256"]:
                                reusable = False
                                break
                    if not reusable:
                        integrity = retained_host_archive_integrity(inno or tool, path, out, bool(inno))
                    record["retained_archive_integrity"] = integrity
                    if integrity["exit_code"] != 0 or not integrity["input_bytes_unchanged"]:
                        raise ValueError("Retained host archive static integrity verification failed")
                record["retained_outputs_verified"] = True
                record["extraction_status"] = "retained; identities recorded below"
            elif setup:
                for component in setup["files"]:
                    target = destination / component["name"]
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(component["data"])
                    if target.stat().st_size != component["bytes"] or a.sha(target) != component["sha256"]:
                        raise ValueError("Setup Factory static extraction readback differs")
                record["extraction_exit_code"] = 0
                record["extraction_status"] = "complete; source-derived static byte decoder"
                record["extraction_argv"] = ["tools/workspace.py", "inspect-host", "--input", row["path"], "--extract"]
            else:
                argv = ([str(inno), "-x", "-b", "-q", "-m", "-a", "-o", "-u", "-d" + str(destination), str(path)] if inno else
                        [str(tool), "x", "-y", "-aou", "-o" + str(destination), str(path)])
                process = subprocess.run(argv, capture_output=True, check=False)
                (out / "extraction.stdout.txt").write_bytes(process.stdout)
                (out / "extraction.stderr.txt").write_bytes(process.stderr)
                record["extraction_exit_code"] = process.returncode
                record["extraction_status"] = "complete" if process.returncode == 0 else "failed; partial output retained"
                record["extraction_argv"] = ([a.rel(inno), *argv[1:-2], "-d" + a.rel(destination), row["path"]] if inno else
                                             [report["tool"]["name"], *argv[1:-2], "-o" + a.rel(destination), row["path"]])
            if inno:
                generated = destination / "install_script.iss"
                script = out / "install_script.iss"
                if generated.is_file():
                    if script.exists() and script.read_bytes() != generated.read_bytes():
                        raise ValueError("Reconstructed Inno script changed during repeat extraction")
                    generated.replace(script)
                record["reconstructed_outputs"] = ([{"path": a.rel(script), "bytes": script.stat().st_size,
                                                      "sha256": a.sha(script), "kind": "analyzer_reconstructed_installer_script",
                                                      "is_original_input": False}] if script.is_file() else [])
            record["files"] = []
            for item in sorted(destination.rglob("*")):
                if item.is_symlink() or item.is_junction() or not item.resolve().is_relative_to(destination.resolve()):
                    raise ValueError("Archive output escaped extraction directory")
                if not item.is_file():
                    continue
                artifact = {"path": a.rel(item), "bytes": item.stat().st_size, "sha256": a.sha(item),
                            "kind": "static_extracted_host_input", "container": row["path"], "container_sha256": row["sha256"]}
                verified_aliases = []
                for alias in installed_by_identity.get(artifact["sha256"], []):
                    installed = a.ROOT / alias["path"]
                    if not installed.is_file() or installed.stat().st_size != alias["bytes"] or a.sha(installed) != alias["sha256"]:
                        raise ValueError("Retained installed evidence changed during official byte comparison: " + alias["path"])
                    if alias["bytes"] != artifact["bytes"]:
                        raise ValueError("Content-identity alias has inconsistent byte length")
                    verified_aliases.append(alias["path"])
                if verified_aliases:
                    artifact["verified_installed_aliases"] = sorted(verified_aliases)
                record["files"].append(artifact)
                row_extracted_rows.append(artifact)
            record["verified_installed_input_matches"] = sorted({alias for artifact in record["files"]
                                                                 for alias in artifact.get("verified_installed_aliases", [])})
            if inno:
                expected_entries = {name.replace("\\", "/") for name in names}
                captured_entries = {Path(artifact["path"]).relative_to(destination.relative_to(a.ROOT)).as_posix()
                                    for artifact in record["files"]}
                if record.get("reconstructed_outputs"):
                    captured_entries.add("install_script.iss")
                record["inno_setup"]["archive_entry_closure"] = {
                    "declared_entries": len(expected_entries), "captured_entries": len(captured_entries),
                    "missing": sorted(expected_entries - captured_entries), "unexpected": sorted(captured_entries - expected_entries),
                    "complete": expected_entries == captured_entries}
            if setup:
                actual_names = {Path(artifact["path"]).relative_to(destination.relative_to(a.ROOT)).as_posix()
                                for artifact in record["files"]}
                if actual_names != set(names):
                    raise ValueError("Setup Factory extraction file closure differs")
                expected = {component["name"]: component for component in setup["files"]}
                for artifact in record["files"]:
                    name = Path(artifact["path"]).relative_to(destination.relative_to(a.ROOT)).as_posix()
                    if any(artifact[key] != expected[name][key] for key in ("bytes", "sha256")):
                        raise ValueError("Setup Factory extracted bytes differ from decoded source: " + name)
                record["setup_factory"]["archive_entry_closure"] = {
                    "declared_entries": len(names), "captured_entries": len(actual_names), "complete": True}
            record["input_bytes_unchanged"] = path.stat().st_size == row["bytes"] and a.sha(path) == row["sha256"]
            if not record["input_bytes_unchanged"]:
                raise ValueError("Host container changed during static extraction")
            record["extracted"] = (record.get("extraction_exit_code", 0) == 0 and
                                   (not inno or record["inno_setup"]["archive_entry_closure"]["complete"]))
        elif getattr(args, "extract", False):
            record["extraction_status"] = "no supported embedded archive detected; PE sections alone do not prove payload extraction"
        record["input_record"] = row
        return row["sha256"], record, row_extracted_rows

    containers = dict(previous_containers)
    extracted_rows = []
    unique_rows = sorted({row["sha256"]: row for row in rows
                          if Path(row["path"]).suffix.lower() in {".exe", ".msix", ".zip", ".7z", ".msi"}}.values(),
                         key=lambda item: item["sha256"])
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        for key, record, extracted in pool.map(inspect_row, unique_rows):
            containers[key] = record
            extracted_rows.extend(extracted)
    report["containers"] = [containers[key] for key in sorted(containers)]
    report["outputs"] = [{"path": a.rel(p), "bytes": p.stat().st_size, "sha256": a.sha(p)}
                         for p in sorted(results.rglob("*.txt"))]
    a.save(results / "inspection.json", report)
    if extracted_rows:
        a.index_files(extracted_rows)
    if getattr(args, "extract", False) and not selected:
        extract_asar()
    print(json.dumps({"inspection": a.rel(results / "inspection.json"), "containers": len(report["containers"])}), flush=True)


def extract_asar():
    """Read Electron ASAR Pickle header/byte spans; no JS is evaluated.

    Layout and unpacked semantics follow electron/asar src/disk.ts
    readArchiveHeaderSync/readFileSync; original header is retained in full.
    """
    import analysis as a
    indexed = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
    for row in indexed["inputs"]:
        if not row["path"].startswith((".analysis/inputs/host-current/extracted/",
                                       ".analysis/inputs/installed-current/files/app-engine/")) or not row["path"].endswith(".asar"):
            continue
        source = a.ROOT / row["path"]
        raw = source.read_bytes()
        if len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError("ASAR source identity differs")
        size_payload, header_length, header_payload, json_length = struct.unpack_from("<IIII", raw)
        if size_payload != 4 or header_length < 8 or header_payload + 4 != header_length or json_length + 8 > header_length or 8 + header_length > len(raw):
            raise ValueError("Malformed ASAR Pickle header")
        header_raw = raw[16:16 + json_length]
        header = json.loads(header_raw)
        output = a.ROOT / ".analysis/inputs/host-current/asar" / row["sha256"][:12]
        previous_output = output.with_name(row["sha256"])
        if previous_output.is_dir() and not output.exists():
            # Keep this content tree below Windows MAX_PATH; identity remains the full SHA in receipts.
            old_prefix, new_prefix = a.rel(previous_output) + "/", a.rel(output) + "/"
            with a.index_lock():
                previous_output.rename(output)
                index_path = a.ROOT / ".analysis/inputs.json"
                index = json.loads(index_path.read_bytes())
                for entry in index["inputs"]:
                    if entry["path"].startswith(old_prefix):
                        entry["path"] = new_prefix + entry["path"][len(old_prefix):]
                index["inputs"].sort(key=lambda entry: entry["path"])
                a.save(index_path, index)
        results = a.ROOT / ".analysis/results/host-current/asar" / row["sha256"]
        results.mkdir(parents=True, exist_ok=True)
        (results / "header.json").write_bytes(header_raw)
        capture = {"schema_version": 1, "recorded_at_utc": a.stamp(), "input": row["path"], "input_sha256": row["sha256"],
                   "input_bytes": row["bytes"], "header_bytes": header_length, "payload_offset": 8 + header_length,
                   "tool": {"name": "workspace ASAR static parser", "path": "tools/host.py", "sha256": a.sha(Path(__file__))},
                   "format_reference": "https://github.com/electron/asar/blob/main/src/disk.ts",
                   "files": [], "unresolved": [], "target_execution": False}

        def visit(node, parts):
            for name, item in node["files"].items():
                if name in {"", ".", ".."} or re.search(r'[\\/:*?"<>|]', name):
                    raise ValueError("Unsafe ASAR entry")
                nested = [*parts, name]
                if "files" in item:
                    visit(item, nested)
                    continue
                if "link" in item:
                    capture["unresolved"].append({"path": "/".join(nested), "kind": "asar_link", "declaration": item})
                    continue
                size = item["size"]
                if not isinstance(size, int) or size < 0:
                    raise ValueError("Invalid ASAR size")
                location = None
                if item.get("unpacked"):
                    unpacked = source.with_name(source.name + ".unpacked")
                    sibling = unpacked.joinpath(*nested)
                    if not sibling.resolve().is_relative_to(unpacked.resolve()):
                        raise ValueError("ASAR unpacked path escaped")
                    content = sibling.read_bytes()
                else:
                    offset = int(item.get("offset", "0"))
                    location = 8 + header_length + offset
                    if offset < 0 or location + size > len(raw):
                        raise ValueError("ASAR byte range outside input")
                    content = raw[location:location + size]
                integrity_content = content
                signing = None
                if len(content) != size:
                    # The official packager signs unpacked PE files after ASAR header construction.
                    # Accept that relation only when the exact pre-signing bytes reproduce the header hash.
                    if item.get("unpacked") and len(content) > size and content[:2] == b"MZ" and item.get("integrity"):
                        pe = struct.unpack_from("<I", content, 0x3c)[0]
                        optional = pe + 24
                        magic = struct.unpack_from("<H", content, optional)[0]
                        security = optional + (112 if magic == 0x20b else 96) + 8 * 4
                        certificate_offset, certificate_size = struct.unpack_from("<II", content, security)
                        if content[pe:pe + 4] == b"PE\0\0" and certificate_offset == size and certificate_offset + certificate_size == len(content):
                            unsigned = bytearray(content[:size])
                            unsigned[optional + 64:optional + 68] = bytes(4)
                            unsigned[security:security + 8] = bytes(8)
                            candidate = bytes(unsigned)
                            candidate_sha = hashlib.sha256(candidate).hexdigest()
                            if candidate_sha == item["integrity"]["hash"].lower():
                                integrity_content = candidate
                                signing = {"declared_unsigned_bytes": size, "unsigned_sha256": candidate_sha,
                                           "certificate_offset": certificate_offset, "certificate_bytes": certificate_size,
                                           "zeroed_fields": [{"offset": optional + 64, "bytes": 4}, {"offset": security, "bytes": 8}],
                                           "authenticode_signature_validation": "not performed; exact unsigned-byte reconstruction only"}
                    if signing is None:
                        capture["unresolved"].append({"kind": "asar_size_mismatch", "path": "/".join(nested),
                                                      "expected_bytes": size, "actual_bytes": len(content), "unpacked": bool(item.get("unpacked"))})
                        continue
                identity = hashlib.sha256(content).hexdigest()
                integrity = item.get("integrity")
                if integrity:
                    if integrity["algorithm"] != "SHA256" or integrity["hash"].lower() != hashlib.sha256(integrity_content).hexdigest():
                        raise ValueError("ASAR file integrity differs")
                    block = integrity["blockSize"]
                    if not isinstance(block, int) or block <= 0:
                        raise ValueError("ASAR block size invalid")
                    blocks = [hashlib.sha256(integrity_content[n:n + block]).hexdigest() for n in range(0, len(integrity_content), block)]
                    if blocks != integrity["blocks"]:
                        raise ValueError("ASAR block integrity differs")
                target = output.joinpath(*nested)
                target.parent.mkdir(parents=True, exist_ok=True)
                if target.exists() and target.read_bytes() != content:
                    raise ValueError("Previously extracted ASAR bytes changed")
                target.write_bytes(content)
                capture["files"].append({"path": a.rel(target), "bytes": len(content), "sha256": identity, "kind": "static_extracted_host_asar_input",
                                         "container": row["path"], "container_sha256": row["sha256"], "offset": location,
                                         "unpacked": bool(item.get("unpacked")), "integrity_verified": bool(integrity) and signing is None,
                                         "post_packaging_signing": signing})

        try:
            visit(header, [])
        except Exception as exc:
            capture["unresolved"].append({"kind": "asar_extraction_failure", "error": str(exc)})
        capture["outputs"] = [{"path": a.rel(results / "header.json"), "bytes": len(header_raw), "sha256": hashlib.sha256(header_raw).hexdigest()}]
        capture["complete"] = not capture["unresolved"]
        a.save(results / "capture.json", capture)
        a.index_files(capture["files"])
        print(json.dumps({"asar": row["path"], "extracted_files": len(capture["files"]), "unresolved": len(capture["unresolved"])}), flush=True)


def add_commands(parsers):
    local = parsers.add_parser("audit-host-local", help="Trace current local host UI, storage, service and native source chains without executing targets")
    local.add_argument("--query", required=True, choices=["dynamic-lighting", "macro-recorder", "screen-color", "help-system-info", "product-chroma-observation", "window-lifecycle", "profiles", "keypad-mapping", "mapping-descriptors"])
    local.add_argument("--verify-only", action="store_true", help="Independently verify the retained profiles source receipt without collecting or publishing it")
    installed = parsers.add_parser("acquire-installed", help="Read-only snapshot of installed Razer files and public official Service Worker resources")
    installed.add_argument("--app-engine-root", required=True)
    installed.add_argument("--user-data-root", required=True)
    installed.add_argument("--services-root")
    installed.add_argument("--verify-only", action="store_true", help="Recheck installed code against retained bytes and current official host package without copying cache or user state")
    fetch = parsers.add_parser("acquire-host", help="Acquire stable official host installer and declared local resources as inert evidence")
    fetch.add_argument("--inspect", action="store_true", help="List containers with 7-Zip without running target code")
    fetch.add_argument("--seven-zip", default="C:/Program Files/7-Zip/7z.exe")
    fetch.add_argument("--extract", action="store_true", help="Statically extract supported ZIP/NSIS/7z payloads; never run installer actions")
    fetch.add_argument("--release", action="store_true", help="Follow statically verified production discovery to Synapse/Chroma host packages")
    fetch.add_argument("--metadata-only", action="store_true", help="Recheck retained official deployment JSON and current manifest references without package downloads")
    fetch.add_argument("--revalidate-bootstrap", action="store_true", help="Double-fetch the current official download page and bootstrap as inert evidence without executing it")
    fetch.add_argument("--repair-metadata-index", action="store_true", help="Preserve and relocate a wrongly indexed metadata receipt into inputs without network access")
    fetch.add_argument("--workers", type=int, choices=range(2, 17), default=4,
                       help="bounded worker threads for independent host container inspections")
    inspect = parsers.add_parser("inspect-host", help="Statically list verified host evidence containers")
    inspect.add_argument("--input", help="Inspect one verified nested host archive while retaining prior container receipts")
    inspect.add_argument("--seven-zip", default="C:/Program Files/7-Zip/7z.exe")
    inspect.add_argument("--extract", action="store_true", help="Statically extract supported payloads")
    inspect.add_argument("--repair-extraction", action="store_true", help="Fresh static extraction with exact retained-byte checks before restoring missing originals")
    inspect.add_argument("--workers", type=int, choices=range(2, 17), default=4,
                         help="bounded worker threads for independent host container inspections")
    analyze = parsers.add_parser("analyze-host", help="Statically restore verified host managed assemblies or host JavaScript")
    mode = analyze.add_mutually_exclusive_group()
    mode.add_argument("--javascript-only", action="store_true", help="Restore current Electron host JS/CJS and packaged Electron integration dependencies with Wakaru")
    mode.add_argument("--extract-asar-only", action="store_true", help="Statically extract indexed installed or official host ASAR bytes without executing JavaScript")
    mode.add_argument("--resolve-managed-dependencies", action="store_true", help="Supplement unresolved C# using byte-verified same-directory assemblies under their captured AssemblyName")
    analyze.add_argument("--managed-scope", choices=["vendor", "all"], default="vendor",
                         help="Capture all Razer version-resource assemblies first, or include every retained CLR dependency")
    analyze.add_argument("--workers", type=int, choices=range(2, 17), default=4,
                         help="bounded worker threads for independent managed assembly captures")


def analyze_host(args):
    import analysis as a

    def filesystem_path(path):
        # ILSpy preserves full WinRT type names, which can exceed MAX_PATH.
        # Keep portable evidence paths unchanged and use extended Windows paths
        # only at the filesystem boundary rather than dropping those outputs.
        if os.name == "nt":
            value = str(path.resolve())
            if not value.startswith("\\\\?\\"):
                return Path("\\\\?\\UNC\\" + value[2:] if value.startswith("\\\\") else "\\\\?\\" + value)
        return path

    if getattr(args, "resolve_managed_dependencies", False):
        return resolve_managed_dependencies(workers=getattr(args, "workers", 4))
    if getattr(args, "extract_asar_only", False):
        return extract_asar()
    if getattr(args, "javascript_only", False):
        return analyze_host_javascript()
    workers = getattr(args, "workers", 4)
    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Managed host analysis workers must be between two and sixteen")
    tool_dir, tool_receipt, restored = prepare_managed_analyzer()
    indexed = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
    capture = {"schema_version": 2, "started_at_utc": a.stamp(), "target_execution": False,
               "tool": {"name": "ILSpyCmd", "version": "9.1.0.7988", "acquisition": a.rel(tool_dir / "acquisition.json"),
                        "acquisition_sha256": a.sha(tool_dir / "acquisition.json"), "restored_files": restored},
               "workflow": {"path": "tools/host.py", "sha256": a.sha(Path(__file__))},
               "assemblies": [], "pending_dependencies": [], "classification_failures": []}
    output = a.ROOT / ".analysis/results/host-current/managed"
    output.mkdir(parents=True, exist_ok=True)
    previous_path = output / "capture.json"
    previous = json.loads(previous_path.read_bytes()) if previous_path.is_file() else {}
    previous_records = {row["input_sha256"]: row for row in previous.get("assemblies", [])}
    candidates = {}
    for row in indexed["inputs"]:
        path = a.ROOT / row["path"]
        if not row["path"].startswith((".analysis/inputs/host-current/", ".analysis/inputs/installed-current/files/",
                                        ".analysis/inputs/official-native-current/")):
            continue
        if row.get("container_extraction_complete") is False or row.get("source_bytes_verified") is False:
            capture.setdefault("excluded_partial_extractions", []).append(row)
            continue
        if path.suffix.lower() not in {".dll", ".exe"}:
            if row.get("kind") not in {"static_extracted_native_input", "static_embedded_native_input"}:
                continue
            with path.open("rb") as stream:
                if stream.read(2) != b"MZ":
                    continue
        raw = path.read_bytes()
        if len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError("Changed managed input")
        if row["sha256"] in candidates:
            candidates[row["sha256"]]["aliases"].append(row["path"])
            candidates[row["sha256"]]["acquisition_bindings"].append(row)
            continue
        try:
            pe = inspect_pe(raw)
        except Exception as exc:
            capture["classification_failures"].append({"input": row["path"], "error": str(exc)})
            continue
        if "clr" not in pe:
            continue
        candidates[row["sha256"]] = {**row, "aliases": [row["path"]], "pe": pe, "acquisition_bindings": [row]}

    def version_values(tree):
        return ([tree["value"]] if "value" in tree else []) + [v for child in tree.get("children", []) for v in version_values(child)]

    ordered_candidates = sorted(candidates.values(), key=lambda r: (r["bytes"], r["sha256"]))
    work_rows = []
    for row in ordered_candidates:
        versions = [v for resource in row["pe"]["version_resources"] for v in version_values(resource["tree"])]
        vendor = any("razer" in value.lower() for value in versions)
        if args.managed_scope == "vendor" and not vendor:
            capture["pending_dependencies"].append({"input": row["path"], "sha256": row["sha256"], "aliases": row["aliases"],
                                                    "reason": "CLR dependency available for --managed-scope all; no Razer version-resource ownership declaration"})
            continue
        work_rows.append(row)

    # ILSpy invocations are independent per content-addressed assembly.  Each
    # worker owns its output directory; the coordinator publishes the receipt
    # in stable input order after every result has completed.
    def analyze_row(row):
        path = a.ROOT / row["path"]
        dest = output / row["sha256"][:12]
        previous_dest = output / row["sha256"]
        if previous_dest.is_dir() and not dest.exists():
            previous_dest.rename(dest)
        dest.mkdir(parents=True, exist_ok=True)
        record = {"input": row["path"], "input_sha256": row["sha256"], "input_bytes": row["bytes"], "aliases": row["aliases"],
                  "acquisition_bindings": row["acquisition_bindings"], "pe": row["pe"], "commands": []}
        retained = previous_records.get(row["sha256"])
        if retained and retained.get("capture_complete") and previous.get("tool", {}).get("acquisition_sha256") == capture["tool"]["acquisition_sha256"]:
            for item in retained["outputs"]:
                artifact = a.ROOT / item["path"]
                native_path = filesystem_path(artifact)
                if not native_path.is_file() or native_path.stat().st_size != item["bytes"] or a.sha(native_path) != item["sha256"]:
                    raise ValueError("Retained managed output changed: " + item["path"])
            record = {**retained, "aliases": row["aliases"], "acquisition_bindings": row["acquisition_bindings"],
                      "verified_reuse_at_utc": a.stamp()}
            return record
        for name, options in [("csharp", ["-p", "--nested-directories", "-o", str(dest / "source")]), ("il", ["--ilcode"])]:
            stdout, stderr = dest / (name + ".stdout.txt"), dest / (name + ".stderr.txt")
            command = ["dotnet", str(tool_dir / "ilspycmd.dll"), "--disable-updatecheck", *options, "-r", str(path.parent), str(path)]
            with stdout.open("wb") as out, stderr.open("wb") as err:
                process = subprocess.run(command, stdout=out, stderr=err, check=False, cwd=a.ROOT)
            record["commands"].append({"stage": name, "argv": [a.rel(Path(v)) if Path(v).is_absolute() and Path(v).is_relative_to(a.ROOT) else v for v in command],
                                       "exit_code": process.returncode, "stdout": a.rel(stdout), "stderr": a.rel(stderr)})
        record["input_bytes_unchanged"] = a.sha(path) == row["sha256"]
        record["diagnostics"] = []
        for generated in sorted((dest / "source").rglob("*.cs")):
            for line_number, line in enumerate(filesystem_path(generated).read_text(encoding="utf-8-sig").splitlines(), 1):
                if re.match(r"\s*//.*(?:Unknown result type|Expected .*got|Error decompiling|Invalid IL|Could not resolve)", line):
                    record["diagnostics"].append({"path": a.rel(generated), "line": line_number,
                                                  "kind": "managed_decompiler_unresolved", "text": line.strip()})
        record["outputs"] = [{"path": a.rel(p), "bytes": filesystem_path(p).stat().st_size,
                              "sha256": a.sha(filesystem_path(p))}
                             for p in sorted(dest.rglob("*")) if filesystem_path(p).is_file()]
        record["capture_complete"] = record["input_bytes_unchanged"] and all(c["exit_code"] == 0 for c in record["commands"])
        record["all_csharp_resolved"] = record["capture_complete"] and not record["diagnostics"]
        return record
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        for row, record in zip(work_rows, pool.map(analyze_row, work_rows)):
            capture["assemblies"].append(record)
            a.save(output / "capture.json", capture)
            print(json.dumps({"managed_input": row["path"], "reused": bool(record.get("verified_reuse_at_utc")),
                              "outputs": len(record.get("outputs", [])),
                              "failed_stages": sum(c["exit_code"] != 0 for c in record.get("commands", []))}), flush=True)
    capture["finished_at_utc"] = a.stamp()
    capture["scope"] = "Verified official and installed CLR assemblies deduplicated by actual bytes; " + ("all retained CLR dependencies included" if args.managed_scope == "all" else "Razer version-resource ownership declarations selected; remaining CLR dependencies explicitly queued")
    capture["selected_capture_complete"] = bool(capture["assemblies"]) and all(r["capture_complete"] for r in capture["assemblies"]) and not capture["classification_failures"]
    capture["full_managed_corpus_captured"] = capture["selected_capture_complete"] and not capture["pending_dependencies"]
    capture["runtime_acceptance"] = "unverified"
    a.save(output / "capture.json", capture)
    print(json.dumps({"receipt": a.rel(output / "capture.json"), "assemblies": len(capture["assemblies"]),
                      "pending_dependencies": len(capture["pending_dependencies"]), "selected_capture_complete": capture["selected_capture_complete"]}), flush=True)


def resolve_managed_dependencies(*, workers=4):
    """Supplement unresolved assemblies with verified same-directory references."""
    import analysis as a
    import shutil
    import xml.etree.ElementTree as ET

    if type(workers) is not int or not 2 <= workers <= 16:
        raise ValueError("Managed dependency workers must be between two and sixteen")
    capture_path = a.ROOT / ".analysis/results/host-current/managed/capture.json"
    capture = json.loads(capture_path.read_bytes())
    if not capture.get("finished_at_utc"):
        raise ValueError("Complete the initial managed capture before dependency supplementation")
    tool_dir, tool_receipt, _ = prepare_managed_analyzer()
    if a.sha(tool_dir / "acquisition.json") != capture["tool"]["acquisition_sha256"]:
        raise ValueError("Managed dependency supplement requires the original pinned analyzer")
    index = {r["path"]: r for r in json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())["inputs"]}
    assemblies_by_directory, assembly_name_sources = {}, {}

    def identity(path):
        return {"path": a.rel(path), "bytes": path.stat().st_size, "sha256": a.sha(path)}

    def verified(row):
        path = a.resolve(row["path"], ".analysis")
        if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            raise ValueError("Managed dependency evidence identity differs: " + row["path"])
        return path

    # Build the reference views on the coordinator.  Workers only consume
    # these immutable, byte-verified files and write to their own SHA output.
    for assembly in capture["assemblies"]:
        projects = [r for r in assembly.get("outputs", []) if r["path"].endswith(".csproj")]
        if not assembly.get("capture_complete") or len(projects) != 1:
            continue
        name = ET.fromstring(verified(projects[0]).read_bytes()).findtext(".//AssemblyName")
        if not name or name in {".", ".."} or any(c in name for c in '/\\:'):
            raise ValueError("Captured assembly name is absent or unsafe")
        assembly_name_sources[assembly["input_sha256"], name] = projects[0]
        for alias in assembly.get("aliases", [assembly["input"]]):
            original = index[alias]
            if original["sha256"] != assembly["input_sha256"]:
                raise ValueError("Captured managed assembly alias changed")
            assemblies_by_directory.setdefault(str(Path(alias).parent), {}).setdefault(name, {})[original["sha256"]] = original

    jobs = []
    for assembly_index, assembly in enumerate(capture["assemblies"]):
        if not assembly.get("capture_complete") or not assembly.get("diagnostics"):
            continue
        directory = assemblies_by_directory.get(str(Path(assembly["input"]).parent), {})
        references = [{"assembly_name": name, "original": next(iter(rows.values())),
                       "assembly_name_evidence": assembly_name_sources[next(iter(rows)), name]}
                      for name, rows in sorted(directory.items()) if len(rows) == 1]
        conflicts = [{"assembly_name": name, "originals": list(rows.values())}
                     for name, rows in sorted(directory.items()) if len(rows) != 1]
        binding = hashlib.sha256(json.dumps(references, sort_keys=True).encode()).hexdigest()
        previous = assembly.get("dependency_resolution")
        if previous and previous.get("reference_binding_sha256") == binding and previous.get("capture_complete"):
            for output in previous["outputs"]:
                verified(output)
            jobs.append({"assembly_index": assembly_index, "input": assembly["input"],
                         "initial_diagnostics": len(assembly["diagnostics"]), "binding": binding,
                         "references": references, "conflicts": conflicts, "reuse": True,
                         "record": previous})
            continue
        if previous:
            raise ValueError("Changed managed dependency supplement requires explicit evidence review")
        view = a.ROOT / ".work/managed-reference-views" / binding
        view.mkdir(parents=True, exist_ok=True)
        for reference in references:
            source = verified(reference["original"])
            destination = view / (reference["assembly_name"] + ".dll")
            if destination.exists() and a.sha(destination) != reference["original"]["sha256"]:
                raise ValueError("Private managed dependency view changed")
            if not destination.exists():
                shutil.copyfile(source, destination)
        source = verified(index[assembly["input"]])
        output = a.ROOT / ".analysis/results/host-current/managed" / assembly["input_sha256"][:12] / "dependency-resolution"
        output.mkdir(parents=True, exist_ok=True)
        command = ["dotnet", str(tool_dir / "ilspycmd.dll"), "--disable-updatecheck", "-p", "--nested-directories",
                   "-o", str(output / "source"), "-r", str(view), str(source)]
        jobs.append({"assembly_index": assembly_index, "input": assembly["input"],
                     "input_sha256": assembly["input_sha256"], "source": source, "output": output,
                     "initial_diagnostics": len(assembly["diagnostics"]), "binding": binding,
                     "references": references, "conflicts": conflicts, "view": view,
                     "command": command, "reuse": False})

    def supplement_job(job):
        if job["reuse"]:
            return job["record"], True, None
        output = job["output"]
        command = job["command"]
        process_returncode, failure = None, None
        stdout, stderr = output / "csharp.stdout.txt", output / "csharp.stderr.txt"
        try:
            with stdout.open("wb") as out, stderr.open("wb") as err:
                process = subprocess.run(command, stdout=out, stderr=err, check=False, cwd=a.ROOT)
            process_returncode = process.returncode
        except Exception as error:  # Preserve a worker failure in the receipt.
            failure = str(error)
        diagnostics = []
        try:
            for generated in sorted((output / "source").rglob("*.cs")):
                for number, line in enumerate(generated.read_text(encoding="utf-8-sig").splitlines(), 1):
                    if re.match(r"\s*//.*(?:Unknown result type|Expected .*got|Error decompiling|Invalid IL|Could not resolve)", line):
                        diagnostics.append({"path": a.rel(generated), "line": number,
                                            "kind": "managed_decompiler_unresolved", "text": line.strip()})
        except Exception as error:
            failure = failure or str(error)
        outputs = []
        for path in sorted(output.rglob("*")):
            if path.is_file():
                try:
                    outputs.append(identity(path))
                except Exception as error:
                    failure = failure or str(error)
        input_unchanged = False
        try:
            input_unchanged = a.sha(job["source"]) == job["input_sha256"]
        except Exception as error:
            failure = failure or str(error)
        record = {"recorded_at_utc": a.stamp(), "reference_binding_sha256": job["binding"],
                  "references": job["references"], "conflicting_names": job["conflicts"],
                  "workflow": identity(Path(__file__)), "analyzer": capture["tool"],
                  "target_execution": False, "reference_scope": "same original input directory only",
                  "command": {"argv": [a.rel(Path(value)) if Path(value).is_absolute() and Path(value).is_relative_to(a.ROOT) else value
                                      for value in command], "exit_code": process_returncode},
                  "diagnostics": diagnostics, "original_diagnostics_retained": job["initial_diagnostics"],
                  "input_bytes_unchanged": input_unchanged, "outputs": outputs}
        if failure:
            record["error"] = failure
        record["capture_complete"] = process_returncode == 0 and input_unchanged and not failure
        record["all_csharp_resolved"] = record["capture_complete"] and not diagnostics and not job["conflicts"]
        return record, False, failure

    results = []
    if jobs:
        pool_workers = max(2, min(workers, len(jobs)))
        with concurrent.futures.ThreadPoolExecutor(max_workers=pool_workers) as pool:
            results = list(pool.map(supplement_job, jobs))
    for job, (record, reused, failure) in zip(jobs, results):
        assembly = capture["assemblies"][job["assembly_index"]]
        assembly["dependency_resolution"] = record
        if not reused and record["capture_complete"] and len(record["diagnostics"]) <= len(assembly["diagnostics"]):
            assembly["preferred_csharp_source"] = a.rel(job["output"] / "source")
        print(json.dumps({"managed_input": job["input"], "initial_diagnostics": job["initial_diagnostics"],
                          "supplement_diagnostics": len(record.get("diagnostics", [])),
                          "exit_code": record["command"].get("exit_code"), "error": failure}, ensure_ascii=False), flush=True)
    capture["dependency_supplement_finished_at_utc"] = a.stamp()
    a.save(capture_path, capture)


def analyze_host_javascript():
    import analysis as a
    tool = a.wakaru()
    indexed = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
    rows = [r for r in indexed["inputs"] if r["path"].startswith(".analysis/inputs/host-current/asar/")
            and "/electron/" in r["path"] and Path(r["path"]).suffix in {".js", ".cjs"}]
    if not rows:
        raise ValueError("No current extracted Electron host JS inputs")
    output = a.ROOT / ".analysis/results/host-current/javascript"
    unpacked = output / "modules"
    application_count = sum(Path(row["path"]).parts[5] == "electron" for row in rows)
    scope = "all current Electron application JS/CJS plus packaged Electron integration dependency subtrees; other dependency raw bytes retained separately"
    if unpacked.exists() and any(unpacked.iterdir()):
        capture_path = output / "capture.json"
        retained = json.loads(capture_path.read_bytes())
        if retained["inputs"] != rows:
            raise ValueError("Host JS capture inputs changed; existing outputs cannot be reused")
        for item in [*rows, *retained["outputs"]]:
            artifact = a.ROOT / item["path"]
            if artifact.stat().st_size != item["bytes"] or a.sha(artifact) != item["sha256"]:
                raise ValueError("Host JS retained artifact changed: " + item["path"])
        retained.update({"scope": scope, "application_js_inputs": application_count,
                         "integration_dependency_js_inputs": len(rows) - application_count, "verified_reuse_at_utc": a.stamp()})
        a.save(capture_path, retained)
        print(json.dumps({"host_js_capture_reused": True, "application_js_inputs": application_count,
                          "integration_dependency_js_inputs": len(rows) - application_count}), flush=True)
        return
    output.mkdir(parents=True, exist_ok=True)
    capture = {"schema_version": 1, "started_at_utc": a.stamp(), "target_execution": False,
               "tool": {"name": "Wakaru", "version": "1.14.0", "sha256": a.sha(tool)}, "inputs": rows,
               "commands": [], "unresolved": [], "scope": scope, "application_js_inputs": application_count,
               "integration_dependency_js_inputs": len(rows) - application_count}
    for row in rows:
        path = a.ROOT / row["path"]
        if path.stat().st_size != row["bytes"] or a.sha(path) != row["sha256"]:
            raise ValueError("Host JS input changed")
        for reference in re.findall(r"sourceMappingURL=([^\s*]+)", path.read_text(encoding="utf-8")):
            if reference.startswith("data:"):
                continue
            if not (path.parent / reference).is_file():
                capture["unresolved"].append({"kind": "referenced_source_map_absent_from_package", "input": row["path"], "reference": reference})
    commands = [("unpack", [str(tool), *[str(a.ROOT / row["path"]) for row in rows], "--unpack", "--level", "minimal", "--provenance", "--emit-source-map", "--diagnostics", "--json", "-o", str(unpacked)]),
                ("validate", [str(tool), "debug", "validate", str(unpacked), "--json"])]
    for stage, argv in commands:
        stdout, stderr = output / (stage + ".stdout.txt"), output / (stage + ".stderr.txt")
        with stdout.open("wb") as out, stderr.open("wb") as err:
            process = subprocess.run(argv, stdout=out, stderr=err, check=False, cwd=a.ROOT)
        capture["commands"].append({"stage": stage, "argv": [a.rel(Path(v)) if Path(v).is_absolute() and Path(v).is_relative_to(a.ROOT) else v for v in argv],
                                   "exit_code": process.returncode, "stdout": a.rel(stdout), "stderr": a.rel(stderr)})
    capture["input_bytes_unchanged"] = all(a.sha(a.ROOT / row["path"]) == row["sha256"] for row in rows)
    capture["outputs"] = [{"path": a.rel(p), "bytes": p.stat().st_size, "sha256": a.sha(p)} for p in sorted(output.rglob("*")) if p.is_file() and p.name != "capture.json"]
    capture["finished_at_utc"] = a.stamp()
    a.save(output / "capture.json", capture)
    print(json.dumps({"host_js_inputs": len(rows), "outputs": len(capture["outputs"]), "missing_maps": len(capture["unresolved"]),
                      "failed_stages": sum(c["exit_code"] != 0 for c in capture["commands"])}), flush=True)


def native_pseudocode_failures(retained, failures, image_base=0):
    """Report missing selected bodies using the capture's actual stage identity."""
    result = []
    for row in retained:
        function = row["capture"]
        if function.get("pseudocode") is not None:
            continue
        address = function["address"]
        rva = function.get("rva")
        if rva is None:
            rva = hex(int(address, 16) - image_base)
        stage = format(int(rva, 16), "x") + ":hexrays"
        matching = [failure for failure in failures
                    if failure.get("stage") == stage or failure.get("address") == address
                    or failure.get("stage") == "hexrays" and failure.get("rva") == rva]
        result.append({"address": address, "rva": rva, "artifact": row["artifact"],
                       "pseudocode_captured": False, "capture_failures": matching})
    return result


class HostLocalAudit:
    """Reusable byte-verified source/function store for local host chain audits."""
    def __init__(self):
        import analysis as a
        import importlib.metadata
        import families
        sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        self.a, self.families = a, families
        self.parser = Parser(Language(tree_sitter_javascript.language()))
        self.parser_versions = {name: importlib.metadata.version(name) for name in ("tree-sitter", "tree-sitter-javascript")}
        self.raw, self.parsed, self.sources, self.owners = {}, {}, {}, {}
        self.artifacts, self.module_cache, self.binding_indexes = {}, {}, {}

    def artifact(self, path, expected=None):
        name = self.a.rel(path)
        boundary = next((root for root in (".analysis/inputs/", ".analysis/results/", "tools/")
                         if name.startswith(root)), None)
        if name == ".analysis/inputs.json":
            boundary = ".analysis/"
        if boundary is None:
            raise ValueError("Local host audit artifact is outside maintained evidence: " + name)
        path = self.a.resolve(name, boundary.rstrip("/"))
        raw = path.read_bytes()
        actual = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        if expected is not None and any(actual[key] != expected[key] for key in ("bytes", "sha256") if key in expected):
            raise ValueError("Local host audit artifact identity differs: " + name)
        if name in self.artifacts and self.artifacts[name] != actual:
            raise ValueError("Local host audit artifact changed during query: " + name)
        self.artifacts[name] = actual
        return raw

    def final_snapshot(self):
        changed = []
        for name, expected in self.artifacts.items():
            try:
                path = self.a.resolve(name, ".analysis" if name == ".analysis/inputs.json" else
                                      name.split("/", 2)[0] if name.startswith("tools/")
                                      else "/".join(name.split("/")[:2]))
                actual = path.read_bytes()
                if len(actual) != expected["bytes"] or hashlib.sha256(actual).hexdigest() != expected["sha256"]:
                    changed.append(name)
            except (OSError, ValueError):
                changed.append(name)
        return changed

    def read(self, item):
        name = item["path"]
        if name in self.sources and any(self.sources[name][key] != item[key] for key in ("bytes", "sha256")):
            raise ValueError("Conflicting local host original identity claims: " + name)
        if name not in self.raw:
            if not name.startswith(".analysis/inputs/") or ".." in Path(name).parts:
                raise ValueError("Local host original is outside current inputs")
            path = self.a.resolve(name, ".analysis/inputs")
            raw = self.artifact(path, item)
            self.raw[name] = raw
            self.sources[name] = {key: item[key] for key in ("path", "bytes", "sha256")}
        return self.raw[name]

    def source_set(self, name):
        import evidence
        receipt = self.a.ROOT / ".analysis/inputs/frontend" / name / "acquisition.json"
        raw = self.artifact(receipt)
        acquired = json.loads(raw)
        deployment = evidence.frontend_deployment_sources(acquired)
        if not acquired.get("source_current_during_acquisition"):
            raise ValueError("Current local host deployment is unverified: " + name)
        originals = [row["original"] for row in deployment["sources"] if row["identity_verified"]]
        if deployment.get("entry_original") is not None:
            self.read(deployment["entry_original"])
        for manifest in deployment["manifests"]:
            if manifest.get("original") is not None:
                self.read(manifest["original"])
        for item in originals:
            self.read(item)
        capture_path = self.a.ROOT / ".analysis/results/frontend" / name / "analysis.json"
        capture = json.loads(self.artifact(capture_path)) if capture_path.is_file() else None
        return {"name": name, "acquisition": {"path": self.a.rel(receipt), "bytes": len(raw),
                    "sha256": hashlib.sha256(raw).hexdigest()}, "deployment": deployment,
                "capture": self.families.identity(capture_path) if capture is not None else None,
                "capture_failures": [row for row in capture.get("commands", []) if row.get("exit_code")] if capture else ["complete_capture_unavailable"],
                "unresolved_acquisition": acquired.get("unresolved", []),
                "failed_acquisition": [row for row in acquired["files"] if not row.get("path")]}, originals

    def bounded_observation_history(self, acquired, identity):
        """Verify appended provider references without expanding the HTTP observation."""
        extensions = []
        while acquired.get("dependency_repairs"):
            repairs = acquired["dependency_repairs"]
            if not isinstance(repairs, list) or not isinstance(repairs[-1], dict):
                raise ValueError("Bounded observation has invalid dependency repair history")
            repair = repairs[-1]
            predecessor = repair.get("previous_acquisition", {})
            digest = predecessor.get("sha256", "")
            if (not re.fullmatch(r"[a-f0-9]{64}", digest)
                    or predecessor.get("path") != ".analysis/results/reverse-pending/review-sources/" + digest + ".json"
                    or type(predecessor.get("bytes")) is not int):
                raise ValueError("Bounded observation requires its immutable predecessor identity")
            previous = json.loads(self.artifact(self.a.ROOT / predecessor["path"], predecessor))
            immutable = lambda value: {key: item for key, item in value.items()
                                       if key not in {"files", "edges", "dependency_repairs"}}
            if (previous.get("dependency_repairs", []) != repairs[:-1]
                    or immutable(acquired) != immutable(previous)
                    or acquired.get("files", [])[:len(previous.get("files", []))] != previous.get("files")
                    or acquired.get("edges", [])[:len(previous.get("edges", []))] != previous.get("edges", [])
                    or repair.get("target_execution") is not False
                    or repair.get("copied_originals") != 0
                    or repair.get("repeated_factory_capture") is not False):
                raise ValueError("Dependency repair changed the bounded observation or its original owners")
            provider_identity, capture_identity = repair.get("provider_acquisition", {}), repair.get("provider_capture", {})
            if any(not isinstance(claim, dict) or type(claim.get("bytes")) is not int
                   or claim["bytes"] < 0 or not re.fullmatch(r"[a-f0-9]{64}", claim.get("sha256", ""))
                   for claim in (provider_identity, capture_identity)):
                raise ValueError("Dependency provider requires complete byte identities")
            provider_path = provider_identity.get("path", "")
            provider_directory = Path(provider_path).parent.as_posix()
            if (not re.fullmatch(r"\.analysis/inputs/frontend/[a-z0-9-]+/acquisition\.json", provider_path)
                    or capture_identity.get("path") != ".analysis/results/frontend/" + Path(provider_directory).name + "/analysis.json"):
                raise ValueError("Dependency provider is outside its frontend acquisition and capture owners")
            provider = json.loads(self.artifact(self.a.ROOT / provider_path, provider_identity))
            capture = json.loads(self.artifact(self.a.ROOT / capture_identity["path"], capture_identity))
            parsed = urllib.parse.urlsplit(provider.get("entry_url", ""))
            if (provider.get("target_execution") is not False
                    or provider.get("source_current_during_acquisition") is not True
                    or parsed.scheme != "https" or parsed.netloc != "apps.razer.com"
                    or capture.get("input") != provider_directory
                    or capture.get("input_bytes_unchanged") is not True
                    or capture.get("target_execution") is not False):
                raise ValueError("Dependency provider has no verified static acquisition and capture")
            provider_files = {row["url"]: row for row in provider["files"]}
            captured = {row["path"]: row for row in capture["original_inputs"]}
            additions = acquired["files"][len(previous["files"]):]
            declared = repair.get("added_originals")
            if (len(provider_files) != len(provider["files"]) or not additions
                    or not isinstance(declared, list) or len(declared) != len(additions)
                    or len({row["url"] for row in additions}) != len(additions)):
                raise ValueError("Dependency repair does not enumerate its exact additional originals")
            for row, claim in zip(additions, declared):
                proof = row.get("retained_dependency", {})
                original = provider_files.get(row["url"])
                captured_original = captured.get(row["path"], {})
                if (proof.get("acquisition") != provider_identity or proof.get("capture") != capture_identity
                        or original != {key: value for key, value in row.items() if key != "retained_dependency"}
                        or any(row.get(key) != claim.get(key) for key in ("url", "path", "bytes", "sha256"))
                        or any(row.get(key) != captured_original.get(key) for key in ("path", "bytes", "sha256"))):
                    raise ValueError("Additional dependency differs from its actual provider or capture")
                self.read(row)
            edges = acquired.get("edges", [])[len(previous.get("edges", [])):]
            if (len(edges) != len(additions) or {edge.get("to") for edge in edges} != {row["url"] for row in additions}
                    or any(edge.get("retained_provider") != provider_identity for edge in edges)):
                raise ValueError("Dependency repair edges differ from the additional originals")
            extensions.append({"acquisition": identity, "previous_acquisition": predecessor,
                               "provider_acquisition": provider_identity, "provider_capture": capture_identity,
                               "additional_originals_verified": len(additions), "additional_http_rounds_claimed": 0})
            acquired, identity = previous, predecessor
        if not isinstance(acquired.get("dependency_repairs", []), list):
            raise ValueError("Bounded observation has invalid dependency repair history")
        return acquired, identity, list(reversed(extensions))

    def observed_sources(self, name, entry_url):
        """Verify bounded HTTP observations without claiming a full deployment closure."""
        receipt = self.a.ROOT / ".analysis/inputs/frontend" / name / "acquisition.json"
        acquired = json.loads(self.artifact(receipt))
        if acquired.get("entry_url") != entry_url or acquired.get("target_execution") is not False \
                or acquired.get("source_current_during_acquisition") is not True:
            raise ValueError("Bounded source observation has an unverified identity: " + name)
        indexed = json.loads(self.artifact(self.a.ROOT / ".analysis/inputs.json"))["inputs"]
        receipt_rows = [row for row in indexed if row["path"] == self.a.rel(receipt)]
        if len(receipt_rows) != 1:
            raise ValueError("Bounded source observation receipt is absent from the input index")
        self.artifact(receipt, receipt_rows[0])
        current_acquisition = self.artifacts[self.a.rel(receipt)]
        if (len({row.get("url") for row in acquired["files"]}) != len(acquired["files"])
                or len({row.get("path") for row in acquired["files"]}) != len(acquired["files"])):
            raise ValueError("Bounded source observation repeats an identity")
        for row in acquired["files"]:
            matches = [item for item in indexed if item["path"] == row.get("path")]
            if len(matches) != 1 or any(matches[0].get(key) != row.get(key) for key in ("bytes", "sha256")):
                raise ValueError("Observed original differs from the input index: " + str(row.get("path")))
        acquired, observation_identity, extensions = self.bounded_observation_history(acquired, current_acquisition)
        originals, verified, urls, paths, request_count = [], [], set(), set(), 0
        observations = acquired["http_observations"]
        stability = acquired["stability"]
        for row in acquired["files"]:
            url, path = row["url"], row["path"]
            if url in urls or path in paths:
                raise ValueError("Bounded source observation repeats an identity: " + url)
            urls.add(url)
            paths.add(path)
            matches = [item for item in indexed if item["path"] == path]
            if len(matches) != 1 or any(matches[0][key] != row[key] for key in ("bytes", "sha256")):
                raise ValueError("Observed original differs from the input index: " + path)
            self.read(row)
            requests = [item for item in observations if item["url"] == url]
            pair = [item for item in requests if item["phase"] != "entry_midcheck"]
            stable = [item for item in stability if item["url"] == url]
            if len(pair) != 2 or pair[0]["phase"] not in {"before", "sources"} \
                    or pair[1]["phase"] != "after" or len(stable) != 1:
                raise ValueError("Bounded source observation lacks both HTTP rounds: " + url)
            for item in [row, *requests]:
                if item["http_status"] != 200 or item["final_url"] != url or item["redirects"] \
                        or any(item[key] != row[key] for key in ("path", "bytes", "sha256")):
                    raise ValueError("Observed HTTP identity changed or failed: " + url)
            if stable[0].get("unchanged") is not True or any(stable[0][key] != row["sha256"]
                    for key in ("before_sha256", "after_sha256")):
                raise ValueError("Observed HTTP stability claim differs from its rounds: " + url)
            originals.append(row)
            request_count += len(requests)
            times = [item.get("acquired_at_utc", item.get("finished_at_utc")) for item in pair]
            if not all(isinstance(value, str) and value for value in times):
                raise ValueError("Observed HTTP rounds have no recorded completion time")
            verified.append({"url": url, "original": self.sources[path],
                "first_acquired_at_utc": times[0], "second_acquired_at_utc": times[1],
                "additional_entry_observations": len(requests) - 2, "same_bytes": True})
        if not originals or entry_url not in urls or len(observations) != request_count \
                or len(stability) != len(urls):
            raise ValueError("Bounded source observation has incomplete or unrelated HTTP rows")
        differences = []
        for row in acquired.get("retained_mismatches", []):
            current = next((item for item in originals if item["url"] == row["url"]), None)
            if current is None or row["live_path"] != current["path"] \
                    or row["live_sha256"] != current["sha256"]:
                raise ValueError("Observed changed-source identity is not retained")
            old = row["retained"]
            self.read(old)
            if old["sha256"] == current["sha256"]:
                raise ValueError("Observed changed-source claim contains identical bytes")
            differences.append({"url": row["url"], "current": self.sources[current["path"]],
                                "previous": self.sources[old["path"]], "same_bytes": False})
        return {"name": name, "acquisition": current_acquisition,
                "bounded_http_acquisition": observation_identity, "retained_dependency_extensions": extensions,
                "scope": acquired["scope"], "observations": verified,
                "http_observations_verified": len(observations), "retained_differences": differences,
                "full_dependency_closure": False, "target_execution": False}, originals

    def frontend_capture(self, name, deployment_files):
        """Verify the reusable full capture, without promoting failed stages."""
        directory = ".analysis/results/frontend/" + name + "/"
        path = self.a.ROOT / directory / "analysis.json"
        capture = json.loads(self.artifact(path))
        if capture["input"] != ".analysis/inputs/frontend/" + name:
            raise ValueError("Frontend capture belongs to another input set: " + name)
        originals, repeated = {}, []
        for row in capture["original_inputs"]:
            self.read(row)
            if row["path"] in originals:
                repeated.append(row["path"])
            originals[row["path"]] = row
        for row in deployment_files:
            expected = originals.get(row["path"])
            if expected is None or any(expected[key] != row[key] for key in ("bytes", "sha256")):
                raise ValueError("Frontend capture does not cover current deployment bytes: " + row["path"])
        declared_js = capture["original_js_inputs"]
        if len(set(declared_js)) != len(declared_js) or set(declared_js) != {
                row["path"] for row in originals.values() if Path(row["path"]).suffix.lower() in {".js", ".mjs"}}:
            raise ValueError("Frontend capture JavaScript input list differs from retained originals")
        recipe = capture.get("module_recovery", {}).get("recipe")
        shared_recipe = None
        if recipe and not recipe["path"].startswith(directory):
            if (not re.fullmatch(r"[a-f0-9]{64}", recipe.get("sha256", ""))
                    or recipe["path"] != ".analysis/results/frontend/recovery-tools/" + recipe["sha256"] + ".py"
                    or not isinstance(recipe.get("parser_versions"), dict) or not recipe["parser_versions"]):
                raise ValueError("Frontend capture shared recovery recipe identity differs")
            shared_recipe = recipe
        outputs = set()
        for row in capture["outputs"]:
            is_shared_recipe = shared_recipe is not None and row["path"] == shared_recipe["path"]
            if is_shared_recipe and row["sha256"] != shared_recipe["sha256"]:
                raise ValueError("Frontend capture shared recovery recipe output differs")
            if (not row["path"].startswith(directory) and not is_shared_recipe) or row["path"] in outputs:
                raise ValueError("Frontend capture output boundary is absent or repeated")
            self.artifact(self.a.ROOT / row["path"], row)
            outputs.add(row["path"])
        if shared_recipe is not None and shared_recipe["path"] not in outputs:
            raise ValueError("Frontend capture shared recovery recipe has no verified output")
        if not outputs:
            raise ValueError("Frontend full capture has no retained output")
        version = capture["tool"]["version"]
        if not re.fullmatch(r"[0-9.]+", version):
            raise ValueError("Frontend analyzer version has an invalid path component")
        executable = self.a.ROOT / "tools/analyzers" / ("wakaru-" + version) / "wakaru.exe"
        self.artifact(executable, {"sha256": capture["tool"]["sha256"]})
        failures = [row for row in capture["commands"] if row.get("exit_code")]
        return {"manifest": self.artifacts[self.a.rel(path)], "tool": capture["tool"],
                "originals_verified": len(originals), "outputs_verified": len(outputs),
                "repeated_identical_original_entries": repeated,
                "commands": capture["commands"], "capture_failures": failures,
                "source_maps": capture.get("source_maps", []), "target_execution": False,
                "successful_full_capture": not failures,
                "semantic_audit": capture.get("semantic_audit", "unverified"),
                "implementation_acceptance": capture.get("implementation_acceptance", "unverified")}

    def tree(self, source):
        name = source["path"]
        if name not in self.parsed:
            tree = self.parser.parse(self.read(source))
            if tree.root_node.has_error:
                raise ValueError("Local host source has unresolved AST errors: " + name)
            self.parsed[name] = tree
        return self.parsed[name]

    def retain(self, role, source, needle, kinds, *, unique=True):
        raw = self.read(source)
        hits = list(re.finditer(re.escape(needle.encode("utf-8")), raw))
        if not hits or unique and len(hits) != 1:
            raise ValueError("Local host owner anchor is absent or ambiguous: " + role)
        result = []
        for hit in hits:
            node = self.tree(source).root_node.descendant_for_byte_range(hit.start(), hit.end() - 1)
            while node is not None and node.type not in kinds:
                node = node.parent
            if node is None:
                raise ValueError("Complete local host owner boundary is absent: " + role)
            key = self.retain_range(role, source, node.start_byte, node.end_byte, node.type)
            if key not in result:
                result.append(key)
        return result

    def retain_range(self, role, source, start, end, syntax):
        raw = self.read(source)
        if not 0 <= start < end <= len(raw):
            raise ValueError("Local host source range exceeds original bytes")
        key = source["sha256"] + ":" + str(start) + ":" + str(end)
        if key not in self.owners:
            body = raw[start:end]
            self.owners[key] = {"id": key, "roles": [], "original": self.sources[source["path"]],
                "syntax": syntax, "byte_start": start, "byte_end": end,
                "body_sha256": hashlib.sha256(body).hexdigest(), "body": body.decode("utf-8")}
        if role not in self.owners[key]["roles"]:
            self.owners[key]["roles"].append(role)
        return key

    def callable_member(self, role, source, root, name, *, receiver=None):
        """Retain one actual callable member inside its lexical owner.

        This handles constructor assignments, class fields, class methods and
        object callbacks. A same-named nested owner remains ambiguous until
        the caller supplies its real enclosing class/object.
        """
        def walk(node):
            yield node
            for child in node.named_children:
                yield from walk(child)
        matches = []
        for node in walk(root):
            key, value, target = None, None, None
            if node.type == "assignment_expression":
                left = node.child_by_field_name("left")
                if left.type == "member_expression":
                    key = left.child_by_field_name("property")
                    target = left.child_by_field_name("object")
                    value = node.child_by_field_name("right")
            elif node.type == "field_definition":
                key = node.child_by_field_name("property")
                value = node.child_by_field_name("value")
            elif node.type == "method_definition":
                key, value = node.child_by_field_name("name"), node
            elif node.type == "pair":
                key, value = node.child_by_field_name("key"), node.child_by_field_name("value")
            if key is None or value is None or key.text != name.encode():
                continue
            if receiver is not None and (target is None or target.text != receiver.encode()):
                continue
            if value.type not in {"arrow_function", "function_expression", "method_definition"}:
                continue
            matches.append(node)
        if len(matches) != 1:
            raise ValueError("Complete callable member is absent or ambiguous: " + role)
        node = matches[0]
        owner = self.retain_range(role, source, node.start_byte, node.end_byte, node.type)
        return node, owner

    def retain_class_field(self, role, source, name):
        """Retain an exact AST field, never a suffix of another member's name."""
        pending, found = [self.tree(source).root_node], []
        while pending:
            node = pending.pop()
            if node.type == "field_definition":
                property_name = node.child_by_field_name("property")
                if property_name is not None and property_name.text == name.encode("utf-8"):
                    found.append(node)
            pending.extend(node.named_children)
        if len(found) != 1:
            raise ValueError("Current exact host class field is absent or ambiguous: " + role)
        node = found[0]
        return self.retain_range(role, source, node.start_byte, node.end_byte, node.type)

    def binding(self, role, source, name, reference_byte):
        import evidence
        if source["path"] not in self.binding_indexes:
            self.binding_indexes[source["path"]] = evidence.JavascriptBindings(self.read(source), self.tree(source))
        result = self.binding_indexes[source["path"]].resolve(name, reference_byte)
        if result["resolution"] == "declaration":
            result["owner_id"] = self.retain_range(role, source, result["start_byte"], result["end_byte"], result["syntax"])
        return result

    def webpack_export(self, role, files, module_id, export_name, *, namespace=None):
        """Resolve an observed current factory and its static export getter.

        Cross-chunk factories are searched only in the caller's verified
        deployment source set. Missing/ambiguous IDs and dynamic getters are
        rejected instead of borrowing similarly named minified bindings.
        """
        matches = []
        for source in files:
            if Path(source["path"]).suffix != ".js":
                continue
            name = source["path"]
            if name not in self.module_cache:
                modules, pending = {}, [self.tree(source).root_node]
                while pending:
                    node = pending.pop()
                    if node.type in {"pair", "method_definition"}:
                        key = node.child_by_field_name("key" if node.type == "pair" else "name")
                        value = node.child_by_field_name("value") if node.type == "pair" else node
                        if key is not None and key.type == "number" and value is not None and value.type in {
                                "arrow_function", "function_expression", "method_definition"}:
                            identifier = self.families.javascript_integer_literal(key.text.decode("ascii"))
                            modules.setdefault(identifier, []).append((node, value))
                            continue
                    pending.extend(reversed(node.named_children))
                self.module_cache[name] = modules
            matches.extend((source, node, value) for node, value in self.module_cache[name].get(module_id, []))
        if len(matches) != 1:
            raise ValueError(f"Current host webpack factory {module_id} is absent or ambiguous: {len(matches)}")
        source, node, factory = matches[0]
        module_owner = self.retain_range(role + "_complete_factory", source, node.start_byte, node.end_byte, node.type)
        parameters = factory.child_by_field_name("parameters")
        params = parameters.named_children if parameters is not None else []
        if len(params) != 3 or any(param.type != "identifier" for param in params):
            raise ValueError("Current host webpack factory parameters require caller analysis")
        exports_name, require_name = params[1].text, params[2].text
        definitions, pending = [], [factory]
        while pending:
            candidate = pending.pop()
            pending.extend(reversed(candidate.named_children))
            if candidate.type != "call_expression":
                continue
            callee = candidate.child_by_field_name("function")
            if callee is None or callee.type != "member_expression":
                continue
            obj, prop = callee.child_by_field_name("object"), callee.child_by_field_name("property")
            args = candidate.child_by_field_name("arguments").named_children
            if obj is None or prop is None or obj.text != require_name or prop.text != b"d" or len(args) != 2:
                continue
            if args[0].text != (namespace.encode("utf-8") if namespace is not None else exports_name) or args[1].type != "object":
                continue
            for pair in args[1].named_children:
                if pair.type != "pair":
                    continue
                key, getter = pair.child_by_field_name("key"), pair.child_by_field_name("value")
                if key is None or getter is None:
                    continue
                name = key.text.decode("utf-8")
                if key.type == "string":
                    name = name[1:-1]
                if name == export_name:
                    definitions.append((candidate, getter))
        if len(definitions) != 1 or definitions[0][1].type not in {"arrow_function", "function_expression"}:
            raise ValueError(f"Current host export {module_id}.{export_name} is absent, ambiguous or dynamic")
        call, getter = definitions[0]
        body = getter.child_by_field_name("body")
        parameters = getter.child_by_field_name("parameters")
        if getter.child_by_field_name("parameter") is not None or parameters is not None and parameters.named_child_count:
            raise ValueError(f"Current host export {module_id}.{export_name} getter has parameters")
        if body is not None and body.type == "statement_block":
            statements = body.named_children
            if len(statements) == 1 and statements[0].type == "return_statement" and statements[0].named_child_count == 1:
                body = statements[0].named_children[0]
        if body is None or body.type != "identifier":
            raise ValueError(f"Current host export {module_id}.{export_name} requires expression analysis")
        binding = self.binding(role + "_exported_owner", source, body.text.decode("utf-8"), body.start_byte)
        if binding["resolution"] != "declaration":
            raise ValueError(f"Current host export {module_id}.{export_name} has no unique lexical declaration")
        return {"module_id": module_id, "export": export_name, "original": self.sources[source["path"]],
                "complete_factory_owner": module_owner,
                "export_definition_owner": self.retain_range(role + "_export_definition", source,
                    call.start_byte, call.end_byte, call.type), "binding": binding}

    def rpc_contract(self, original, *, stub_rva, interface_rva, role, server_contract=None,
                     callback_offsets_rva=None, callback_count=0):
        """Decode the observed x64 MIDL/Oi2 contract as static PE bytes.

        This preserves raw descriptors, procedure/type bytes and every server
        slot. It deliberately rejects other formats and does not execute RPC.
        """
        import uuid
        raw = self.read(original)
        pe = inspect_pe(raw)
        if pe["architecture"] != "x64":
            raise ValueError("Current MIDL query only decodes observed x64 descriptors")
        base = int(pe["image_base"], 16)
        regions = []

        def region(label, rva, size):
            section = next((part for part in pe["sections"] if part["rva"] <= rva
                            and rva + size <= part["rva"] + part["bytes"]), None)
            if size <= 0 or section is None:
                raise ValueError("MIDL contract bytes are outside file-backed PE sections")
            offset = section["file_offset"] + rva - section["rva"]
            body = raw[offset:offset + size]
            regions.append({"role": label, "rva": hex(rva), "file_offset": offset,
                "bytes": size, "raw_hex": body.hex(), "sha256": hashlib.sha256(body).hexdigest()})
            return body

        def pointer(data, offset):
            address = struct.unpack_from("<Q", data, offset)[0]
            if address < base:
                raise ValueError("MIDL contract pointer is not image-relative")
            return address - base

        stub = region("MIDL_STUB_DESC", stub_rva, 152)
        interface = region("RPC_INTERFACE", interface_rva, 96)
        if struct.unpack_from("<I", interface)[0] != 96 or pointer(stub, 0) != interface_rva:
            raise ValueError("MIDL interface length or stub binding differs")
        iid = str(uuid.UUID(bytes_le=interface[4:20]))
        iid_version = struct.unpack_from("<HH", interface, 20)
        syntax = str(uuid.UUID(bytes_le=interface[24:40]))
        syntax_version = struct.unpack_from("<HH", interface, 40)
        if syntax != "8a885d04-1ceb-11c9-9fe8-08002b104860" or syntax_version != (2, 0):
            raise ValueError("MIDL transfer syntax requires a different decoder")
        info_rva = pointer(interface, 80)
        # Both endpoints expose incoming RPC through MIDL_SERVER_INFO. On the
        # DLL endpoint this is the reverse callback interface, not a proxy
        # descriptor for the seven outgoing service procedures.
        info = region("MIDL_SERVER_INFO", info_rva, 64)
        if pointer(info, 0) != stub_rva:
            raise ValueError("MIDL proxy/server info does not reference this stub")
        dispatch = region("RPC_DISPATCH_TABLE", pointer(interface, 48), 24)
        interface_dispatch_count = struct.unpack_from("<I", dispatch)[0]
        offset_evidence = None
        if role == "server":
            count = interface_dispatch_count
            if not 0 < count <= 256:
                raise ValueError("MIDL procedure count is outside the observed contract")
            offsets_rva = pointer(info, 24)
            offsets_raw = region("procedure_offset_table", offsets_rva, count * 2)
            offsets = struct.unpack("<" + str(count) + "H", offsets_raw)
        elif role == "client":
            # The client-side interface dispatch table describes callbacks,
            # not the outgoing service opnums. Obtain the latter from the
            # verified server table and require byte equality below.
            if not server_contract or server_contract["role"] != "server" or server_contract["interface_uuid"] != iid:
                raise ValueError("Client service procedure boundaries require the matching verified server contract")
            offsets = tuple(row["offset"] for row in server_contract["procedures"])
            count = len(offsets)
            offset_evidence = {"original": server_contract["original"], "procedure_offsets": list(offsets)}
        else:
            raise ValueError("MIDL endpoint role is unresolved")
        if offsets[0] != 0 or list(offsets) != sorted(set(offsets)):
            raise ValueError("MIDL procedure offsets are not ordered unique boundaries")
        proc_rva, type_rva = pointer(info, 16), pointer(stub, 64)
        # The observed Oi2 header has no explicit handle arguments beyond its
        # primitive implicit handle. Extended headers and six-byte parameters
        # determine its complete size, including the final procedure.
        probe = region("last_procedure_header", proc_rva + offsets[-1], 26)
        if probe[0:2] != b"\x32\x48" or probe[16] < 8:
            raise ValueError("MIDL procedure header requires an unobserved decoder")
        last_size = 16 + probe[16] + 6 * probe[15]
        procedures_raw = region("complete_procedure_format", proc_rva, offsets[-1] + last_size)
        types_raw = region("observed_type_format_with_terminator", type_rva, proc_rva - type_rva)
        def procedure(number, start, value, handle):
            if len(value) < 24 or value[0:2] != bytes((handle, 0x48)) or value[16] < 8:
                raise ValueError("MIDL procedure is not the observed Oi2 handle format")
            opnum, stack_size = struct.unpack_from("<HH", value, 6)
            parameter_count, extension_size = value[15], value[16]
            begin = 16 + extension_size
            if opnum != number or begin + 6 * parameter_count != len(value):
                raise ValueError("MIDL procedure ordinal or parameter boundary differs")
            parameters = []
            for index in range(parameter_count):
                flags, stack, kind = struct.unpack_from("<HHH", value, begin + index * 6)
                parameter = {"ordinal": index, "flags": hex(flags), "stack_offset": stack,
                    "in": bool(flags & 0x8), "out": bool(flags & 0x10), "return": bool(flags & 0x20),
                    "base_type": bool(flags & 0x40), "simple_ref": bool(flags & 0x100),
                    "must_size": bool(flags & 1), "must_free": bool(flags & 2), "server_alloc_units_8_bytes": flags >> 13}
                if flags & 0x40:
                    parameter.update(type_code=hex(kind), decoded_base_type={3: "FC_SMALL", 8: "FC_LONG"}.get(kind, "unresolved"))
                else:
                    if not 0 <= kind < len(types_raw):
                        raise ValueError("MIDL parameter type offset exceeds retained format")
                    parameter.update(type_offset=kind, type_code=hex(types_raw[kind]))
                    if types_raw[kind] == 0x22:
                        parameter["decoded_type"] = "FC_C_CSTRING"
                    elif types_raw[kind:kind + 4] == b"\x12\x00\x02\x00":
                        parameter["decoded_type"] = "FC_UP -> FC_CARRAY"
                        if types_raw[kind + 4] != 0x1b:
                            raise ValueError("MIDL unique pointer target differs from observed array")
                    elif types_raw[kind:kind + 8] == b"\x11\x14\x02\x00\x12\x00\x02\x00":
                        if kind + 8 >= len(types_raw) or types_raw[kind + 8] != 0x1b:
                            raise ValueError("MIDL reference/unique pointer target differs from observed array")
                        parameter["decoded_type"] = "FC_RP(pointer_deref) -> FC_UP -> FC_CARRAY"
                    else:
                        parameter["decoded_type"] = "requires_type_analysis"
                parameters.append(parameter)
            return {"opnum": number, "offset": start, "bytes": len(value), "raw_hex": value.hex(),
                "sha256": hashlib.sha256(value).hexdigest(), "stack_bytes": stack_size,
                "handle_type": "FC_CALLBACK_HANDLE" if handle == 0x34 else "FC_BIND_PRIMITIVE",
                "parameters": parameters}

        procedures = []
        for number, start in enumerate(offsets):
            end = offsets[number + 1] if number + 1 < count else len(procedures_raw)
            procedures.append(procedure(number, start, procedures_raw[start:end], 0x32))
        if role == "client" and any(left["raw_hex"] != right["raw_hex"] for left, right in zip(procedures, server_contract["procedures"])):
            raise ValueError("Client and server procedure format bytes differ")
        slots = []
        if role == "server":
            table_rva = pointer(info, 8)
            table = region("server_routine_table", table_rva, count * 8)
            for number, target in enumerate(struct.unpack("<" + str(count) + "Q", table)):
                slots.append({"opnum": number, "slot_rva": hex(table_rva + number * 8), "target": hex(target),
                              "target_rva": hex(target - base)})
        callbacks, callback_slots = [], []
        if callback_count:
            if not 0 < callback_count <= 256 or callback_offsets_rva is None:
                raise ValueError("Callback procedure boundaries require an explicit observed table")
            if role == "client" and (callback_count != interface_dispatch_count
                                     or callback_offsets_rva != pointer(info, 24)):
                raise ValueError("Reverse callback table differs from the incoming DLL interface")
            callback_offsets_raw = region("callback_procedure_offset_table", callback_offsets_rva, callback_count * 2)
            callback_offsets = struct.unpack("<" + str(callback_count) + "H", callback_offsets_raw)
            if list(callback_offsets) != sorted(set(callback_offsets)) or callback_offsets[0] < len(procedures_raw):
                raise ValueError("Callback format overlaps service procedures or has repeated boundaries")
            for number, start in enumerate(callback_offsets):
                header = region("callback_procedure_header", proc_rva + start, 26)
                size = 16 + header[16] + 6 * header[15]
                body = region("complete_callback_procedure_format", proc_rva + start, size)
                callbacks.append(procedure(number, start, body, 0x34))
            if role == "client":
                if not server_contract or [row["raw_hex"] for row in callbacks] != [row["raw_hex"] for row in server_contract.get("callbacks", [])]:
                    raise ValueError("Reverse callback procedure format differs between server and DLL")
                table_rva = pointer(info, 8)
                table = region("callback_routine_table", table_rva, callback_count * 8)
                for number, target in enumerate(struct.unpack("<" + str(callback_count) + "Q", table)):
                    callback_slots.append({"opnum": number, "slot_rva": hex(table_rva + number * 8),
                                           "target": hex(target), "target_rva": hex(target - base)})
        return {"original": self.sources[original["path"]], "role": role, "interface_uuid": iid,
            "interface_version": list(iid_version), "transfer_syntax_uuid": syntax,
            "transfer_syntax_version": list(syntax_version), "procedure_count": count,
            "interface_dispatch_count": interface_dispatch_count, "service_offset_evidence": offset_evidence,
            "service_procedure_bytes_match_server": True if role == "client" else None,
            "procedures": procedures, "server_routines": slots,
            "incoming_descriptor_kind": "MIDL_SERVER_INFO",
            "incoming_dispatch_role": "service" if role == "server" else "reverse_callback",
            "callbacks": callbacks, "callback_routines": callback_slots,
            "callback_format_bytes_match_server": True if role == "client" and callbacks else None,
            "verified_regions": regions,
            "contract_abi_complete": False, "target_execution": False}

    def native_slice(self, original, exports, needles=(), depth=2, *, rvas=(), regions=()):
        """Retain full functions; follow explicit direct edges for a stated depth.

        Imports, indirect calls and unexpanded edges remain visible. This does
        not turn a bounded native slice into a semantic-closure claim.
        """
        import bisect
        raw = self.read(original)
        pe = inspect_pe(raw)
        directory = self.a.ROOT / ".analysis/results/native" / original["sha256"]
        capture_path = directory / "capture.json"
        capture = json.loads(self.artifact(capture_path))
        image_base = int(pe["image_base"], 16)
        if capture.get("input_sha256") != original["sha256"] or int(capture["image_base"], 16) != image_base:
            raise ValueError("Native capture input or image base differs")
        outputs = {row["path"]: row for row in capture["outputs"]}

        def dataset(name):
            item = outputs[name + ".json"]
            path = directory / item["path"]
            return json.loads(self.artifact(path, item))

        entries = {row["name"]: row for row in dataset("entrypoints")}
        functions = {int(row["address"], 16): row for row in capture["functions"]}
        function_directories = {address: directory for address in functions}
        supplements = []
        decompiler_records = list(capture.get("failures", []))
        for path in sorted(directory.glob("supplements/*/capture.json")):
            supplement = json.loads(self.artifact(path))
            decompiler_records.extend(supplement.get("failures", []))
            if supplement.get("input_sha256") != original["sha256"] or int(supplement["image_base"], 16) != image_base:
                raise ValueError("Native supplemental capture input or image base differs")
            blocking_failures = [failure for failure in supplement.get("failures", [])
                if failure.get("stage") != "hexrays" or not re.fullmatch(r"0x[0-9a-fA-F]+", str(failure.get("rva", "")))]
            if not supplement.get("capture_complete") or blocking_failures:
                raise ValueError("Native supplemental capture is incomplete: " + self.a.rel(path))
            supplements.append({"artifact": self.families.identity(path),
                "base_capture": supplement.get("base_capture"),
                "ida_version": supplement.get("ida_version"),
                "hexrays_version": supplement.get("hexrays_version"),
                "capture_failures": supplement.get("failures", []),
                "all_pseudocode_captured": supplement.get("all_pseudocode_captured"),
                "chains": supplement.get("chains", []),
                "pointer_table": supplement.get("pointer_table")})
            for row in supplement.get("functions", []):
                address = int(row["address"], 16)
                previous = functions.get(address)
                if previous is not None and previous["sha256"] == row["sha256"]:
                    continue
                if previous is None or not previous.get("pseudocode_captured") and row.get("pseudocode_captured"):
                    functions[address] = row
                    function_directories[address] = path.parent
        starts, retained = sorted(functions), {}

        def function(address):
            if address not in retained:
                item = functions[address]
                path = function_directories[address] / item["path"]
                data = json.loads(self.artifact(path, item))
                if int(data["address"], 16) != address:
                    raise ValueError("Native function address differs from its capture entry")
                for chunk in data["chunks"]:
                    expected = bytes.fromhex(chunk["raw_hex"])
                    if int(chunk["end"], 16) - int(chunk["start"], 16) != len(expected):
                        raise ValueError("Native function chunk boundary differs from its complete bytes")
                    rva = int(chunk["start"], 16) - image_base
                    section = next((part for part in pe["sections"] if part["rva"] <= rva
                                    and rva + len(expected) <= part["rva"] + part["bytes"]), None)
                    if section is None:
                        raise ValueError("Native function chunk is outside file-backed PE sections")
                    offset = section["file_offset"] + rva - section["rva"]
                    if raw[offset:offset + len(expected)] != expected or hashlib.sha256(expected).hexdigest() != chunk["sha256"]:
                        raise ValueError("Native function chunk differs from actual PE bytes")
                retained[address] = {"artifact": self.families.identity(path), "capture": data, "input_chunks_verified": True}
            return retained[address]

        def owner(address):
            index = bisect.bisect_right(starts, address) - 1
            if index < 0:
                return None
            candidate = starts[index]
            data = function(candidate)["capture"]
            return candidate if any(int(row["start"], 16) <= address < int(row["end"], 16) for row in data["chunks"]) else None

        seeds, missing = set(), []
        for rva in rvas:
            address = image_base + rva
            if address not in functions:
                missing.append({"kind": "native_seed_function_unavailable", "rva": hex(rva)})
            else:
                seeds.add(address)
        retained_regions = []
        for spec in regions:
            rva, size = spec["rva"], spec["bytes"]
            section = next((part for part in pe["sections"] if part["rva"] <= rva
                            and rva + size <= part["rva"] + part["bytes"]), None)
            if section is None or size <= 0:
                raise ValueError("Native data region is outside file-backed PE sections")
            offset = section["file_offset"] + rva - section["rva"]
            body = raw[offset:offset + size]
            region = {**spec, "rva": hex(rva), "raw_hex": body.hex(), "sha256": hashlib.sha256(body).hexdigest(),
                      "file_offset": offset}
            if spec.get("decode") == "pointer_pairs":
                if pe["architecture"] not in {"x64", "arm64", "x86"}:
                    raise ValueError("Native dispatch pointer width is unresolved")
                width = 8 if pe["architecture"] in {"x64", "arm64"} else 4
                if size % (width * 2):
                    raise ValueError("Native pointer pair table has an incomplete row")
                pairs = []
                for start in range(0, size, width * 2):
                    key, target = struct.unpack_from("<QQ" if width == 8 else "<II", body, start)
                    key_rva = key - image_base
                    key_section = next((part for part in pe["sections"] if part["rva"] <= key_rva
                                        < part["rva"] + part["bytes"]), None)
                    if key_section is None:
                        raise ValueError("Native dispatch string is outside file-backed PE sections")
                    key_offset = key_section["file_offset"] + key_rva - key_section["rva"]
                    key_end = raw.find(b"\0", key_offset, key_section["file_offset"] + key_section["bytes"])
                    if key_end < 0 or target not in functions:
                        raise ValueError("Native dispatch key or function boundary is unresolved")
                    key_bytes = raw[key_offset:key_end + 1]
                    pairs.append({"slot_rva": hex(rva + start), "key_rva": hex(key_rva),
                                  "key": key_bytes[:-1].decode("utf-8"), "key_raw_hex": key_bytes.hex(),
                                  "key_sha256": hashlib.sha256(key_bytes).hexdigest(), "target": hex(target),
                                  "target_rva": hex(target - image_base)})
                    seeds.add(target)
                region["pointer_pairs"] = pairs
            retained_regions.append(region)
        for name in exports:
            if name not in entries:
                missing.append({"kind": "native_export_unavailable", "name": name})
            elif int(entries[name]["address"], 16) not in functions:
                missing.append({"kind": "native_export_function_unavailable", "name": name})
            else:
                seeds.add(int(entries[name]["address"], 16))
        string_refs = []
        if needles:
            for row in dataset("strings"):
                if not any(needle in row["text"] for needle in needles):
                    continue
                string_refs.append(row)
                for ref in row["xrefs"]:
                    address = owner(int(ref["from"], 16))
                    if address is not None:
                        seeds.add(address)
        selected, edges, indirect, pending = set(), [], [], [(address, 0) for address in sorted(seeds)]
        while pending:
            address, level = pending.pop(0)
            if address in selected:
                continue
            selected.add(address)
            data = function(address)["capture"]
            for instruction in data["assembly"]:
                mnemonic = instruction["text"].strip().split(" ", 1)[0].lower()
                if mnemonic not in {"call", "jmp"}:
                    continue
                targets = [ref for ref in instruction["xrefs_from"] if ref.get("code") and ref["type"] in {17, 18, 19, 20}]
                if not targets:
                    indirect.append({"function": data["address"], "instruction": instruction,
                                     "status": "requires_import_or_indirect_target_analysis"})
                for ref in targets:
                    target = owner(int(ref["to"], 16))
                    if target == address:
                        continue
                    edges.append({"from": data["address"], "instruction": instruction["address"], "target": ref["to"],
                                  "function": hex(target) if target is not None else None, "depth": level + 1})
                    if target is not None and level < depth:
                        pending.append((target, level + 1))
        selected_functions = [retained[address] for address in sorted(selected)]
        decompiler_failures = native_pseudocode_failures(
            selected_functions, decompiler_records, image_base=int(capture["image_base"], 16))
        return {"original": self.sources[original["path"]], "capture": self.families.identity(capture_path),
            "ida_version": capture["ida_version"], "hexrays_version": capture["hexrays_version"],
            "supplemental_captures": supplements,
            "capture_failures": capture.get("failures", []),
            "image_base": capture["image_base"], "exports": [entries[name] for name in exports if name in entries],
            "explicit_seed_rvas": [hex(rva) for rva in rvas], "verified_data_regions": retained_regions,
            "string_references": string_refs, "complete_functions": selected_functions,
            "direct_edges": edges, "indirect_or_import_calls": indirect,
            "unexpanded_direct_edges": [row for row in edges if row["function"] and int(row["function"], 16) not in selected],
            "direct_expansion_depth": depth, "failures": missing,
            "decompiler_failures": decompiler_failures,
            "all_selected_pseudocode_captured": not decompiler_failures,
            "native_semantics_complete": False, "target_execution": False}


def verified_host_owners(audit, suffixes):
    """Byte-verify complete ASAR members and their current official package."""
    indexed = json.loads(audit.artifact(audit.a.ROOT / ".analysis/inputs.json"))
    originals = {row["path"]: row for row in indexed["inputs"]}
    receipt_path = audit.a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    receipt = json.loads(audit.artifact(receipt_path))
    retained = {}
    for suffix in suffixes:
        matches = [row for row in indexed["inputs"] if row["path"].startswith(".analysis/inputs/host-current/asar/")
                   and row["path"].endswith("/" + suffix)]
        if len(matches) != 1:
            raise ValueError("Current complete host source is absent or ambiguous: " + suffix)
        row = matches[0]
        parent = originals[row["container"]]
        body, archive = audit.read(row), audit.read(parent)
        if (parent["sha256"] != row["container_sha256"] or row.get("unpacked") or row.get("offset") is None
                or archive[row["offset"]:row["offset"] + len(body)] != body):
            raise ValueError("Current host member differs from its retained ASAR bytes")
        aliases = [item for item in indexed["inputs"]
                   if item["path"].startswith(".analysis/inputs/host-current/extracted/")
                   and item["sha256"] == parent["sha256"] and item["bytes"] == parent["bytes"]]
        if len(aliases) != 1 or audit.read(aliases[0]) != archive:
            raise ValueError("Current host ASAR official byte identity is unresolved")
        official = aliases[0]
        packages = [item for item in receipt["files"] if item.get("path") == official.get("container")
                    and item.get("sha256") == official.get("container_sha256") and item.get("manifest_identity_verified")]
        if len(packages) != 1:
            raise ValueError("Current complete host source package provenance is unresolved")
        audit.artifact(audit.a.ROOT / packages[0]["path"], packages[0])
        retained[suffix] = {"original": row, "container": parent, "official_container": official,
                            "official_package": packages[0], "acquisition": audit.families.identity(receipt_path),
                            "container_bytes_verified": True, "body": body.decode("utf-8")}
    return retained


def retain_no_browser_input_handler(audit):
    """Retain the host shortcut gate without inferring suppression of DOM keys."""
    common = "electron/components/Tab/common.js"
    store = "electron/components/Tab/TabStore.js"
    main = "electron/main.js"
    sources = verified_host_owners(audit, (common, store, main))
    original = sources[common]["original"]
    role = "host_no_browser_input_handler"
    key = audit.retain(role, original, "ie=(e,o)=>", {"variable_declarator"})[0]
    handler = audit.owners[key]["body"]
    if ('if(o&&!B.noBrowserInputHandlerView.includes(e.webContents.id))' not in handler
            or 'B.noBrowserInputHandlerView.includes(o.webContents?.id)' not in handler
            or 'else console.log("handleBeforeInput ~ blocked")' not in handler):
        raise ValueError("Current host browser-input gate differs from its sender/view branches")
    def walk(node):
        yield node
        for child in node.named_children:
            yield from walk(child)
    node = audit.tree(original).root_node.descendant_for_byte_range(
        audit.owners[key]["byte_start"], audit.owners[key]["byte_end"] - 1)
    methods = {child.child_by_field_name("property").text.decode() for child in walk(node)
        if child.type == "member_expression"}
    if methods & {"preventDefault", "stopPropagation", "stopImmediatePropagation"}:
        raise ValueError("Current host browser-input gate now suppresses input delivery")
    owners = {"handle_before_input": key}
    for name in ("registerNoBrowserInputHandler", "unRegisterNoBrowserInputHandler"):
        _, owners[name] = audit.callable_member("host_" + name, original, audit.tree(original).root_node, name)
        owners["dispatch_" + name] = audit.retain("host_dispatch_" + name, sources[main]["original"],
            'case"' + name + '":', {"switch_case"})[0]
    register = audit.owners[owners["registerNoBrowserInputHandler"]]["body"]
    unregister = audit.owners[owners["unRegisterNoBrowserInputHandler"]]["body"]
    if ('if(e?.sender?.id)' not in register or 'const o=e.sender.id' not in register
            or 'B.noBrowserInputHandlerView.includes(o)||B.noBrowserInputHandlerView.push(o)' not in register
            or 'if(e?.sender?.id)' not in unregister or 'const o=e.sender.id' not in unregister
            or 'B.noBrowserInputHandlerView=B.noBrowserInputHandlerView.filter(e=>e!==o)' not in unregister):
        raise ValueError("Current host browser-input registration no longer retains sender identity")
    owners["state_initialization"] = audit.retain("host_no_browser_input_state", sources[store]["original"],
        "this.noBrowserInputHandlerView=[]", {"method_definition"})[0]
    owners["handler_export"] = audit.retain("host_no_browser_input_export", original,
        "handleBeforeInput:ie", {"pair"})[0]
    if not all(('Ro.' + name + '(n)') in audit.owners[owners["dispatch_" + name]]["body"]
               for name in ("registerNoBrowserInputHandler", "unRegisterNoBrowserInputHandler")):
        raise ValueError("Current host browser-input IPC dispatch differs from its registration owner")
    return {"sources": sources, "owners": owners,
        "semantics": "Registration deduplicates the IPC sender.id in TabStore's noBrowserInputHandlerView and unregistration removes that identity. handleBeforeInput skips its host shortcut actions for a registered webContents or the matching active view; the complete handler does not preventDefault or stop propagation. This gate does not establish suppression of DOM keydown/keyup and cannot justify disabling the mounted browser capture fallback.",
        "target_execution": False, "runtime_acceptance": "unverified"}


def verified_host_native(audit, suffix):
    indexed = json.loads(audit.artifact(audit.a.ROOT / ".analysis/inputs.json"))
    matches = [row for row in indexed["inputs"] if row["path"].startswith(".analysis/inputs/host-current/extracted/")
               and row["path"].endswith("/" + suffix)]
    if len(matches) != 1:
        raise ValueError("Current host native input is absent or ambiguous: " + suffix)
    row = matches[0]
    audit.read(row)
    receipt_path = audit.a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    receipt = json.loads(audit.artifact(receipt_path))
    packages = [item for item in receipt["files"] if item.get("path") == row.get("container")
                and item.get("sha256") == row.get("container_sha256") and item.get("manifest_identity_verified")]
    if len(packages) != 1:
        raise ValueError("Current host native package provenance is unresolved")
    audit.artifact(audit.a.ROOT / packages[0]["path"], packages[0])
    for name in row.get("verified_installed_aliases", []):
        alias = next((item for item in indexed["inputs"] if item["path"] == name), None)
        if alias is None or audit.read(alias) != audit.read(row):
            raise ValueError("Current installed native alias differs from official bytes")
    return row, {"original": row, "official_package": packages[0], "acquisition": audit.families.identity(receipt_path)}


def receiver_hardware_dispatch_proof(retained):
    """Resolve the queued callback's adjusted this pointer against verified PE bytes."""
    functions = {int(row["capture"]["rva"], 16): row["capture"]
                 for row in retained["complete_functions"]}
    instructions = {int(row["rva"], 16): bytes.fromhex(row["bytes"])
                    for function in functions.values() for row in function["assembly"]}
    for rva, expected in ((0x9d7f4, "488b4620"), (0x9d7f8, "488b4e28"),
                          (0x9d7fc, "4883e980"), (0x6a470, "488b01"),
                          (0x6a473, "488b4018")):
        if instructions.get(rva) != bytes.fromhex(expected):
            raise ValueError("Receiver queued hardware callback this adjustment or slot differs: " + hex(rva))
    base = int(retained["image_base"], 16)
    regions = {row["role"]: bytes.fromhex(row["raw_hex"])
               for row in retained["verified_data_regions"]}
    dispatch = []
    for name, constructor, lea, table, target in (
        ("input_base", 0x64e8c, 0x64ef0, 0x2e5ac8, 0x2a4ac0),
        ("usb", 0xa89f0, 0xa8a5a, 0x2e7e38, 0xa8de0),
        ("ble", 0x52158, 0x521c2, 0x2e3fd8, 0x53c70),
        ("external", 0x6319c, 0x63206, 0x2e5668, 0x64a60),
        ("os_hook", 0xa6b74, 0xa6bde, 0x2e7958, 0x549d0),
    ):
        encoded = instructions.get(lea, b"")
        if (constructor not in functions or len(encoded) != 7 or encoded[:3] != b"\x48\x8d\x05"
                or lea + 7 + struct.unpack_from("<i", encoded, 3)[0] != table
                or instructions.get(lea + 7) != bytes.fromhex("48898680000000")):
            raise ValueError("Receiver hardware callback constructor subobject differs: " + name)
        raw = regions.get(name + "_hardware_vtable", b"")
        if len(raw) != 32:
            raise ValueError("Receiver hardware callback table is truncated: " + name)
        if (struct.unpack_from("<Q", raw, 16)[0] - base != 0x6a330
                or struct.unpack_from("<Q", raw, 24)[0] - base != target):
            raise ValueError("Receiver hardware callback virtual dispatch differs: " + name)
        dispatch.append({"device_type": name, "constructor_rva": hex(constructor),
            "subobject_offset": 128, "vtable_rva": hex(table), "slot_byte_offset": 24,
            "queued_target_rva": hex(target), "actual_constructor_and_pe_bytes_verified": True})
    return {"bound_object_adjustment": 128, "queued_thunk_rva": "0x9d7d0",
        "virtual_thunk_rva": "0x6a470", "virtual_slot_bytes": 24, "dispatch": dispatch}


def retain_receiver_hardware_producer(audit):
    """Retain the exact collection, adjusted callback, report classifier and serializer."""
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    retained = audit.native_slice(original, ("registerHardwareEvent", "unregisterHardwareEvent",
        "setHardwareEventCallback"), depth=0,
        rvas=(0x7610, 0x7800, 0x58790, 0x64e8c, 0x52158, 0x6319c, 0xa6b74, 0xa89f0,
              0x6b7c0, 0x6b9e0, 0x6bbd0, 0x6bde0, 0xab7f0, 0x11a702, 0x119de6,
              0xac850, 0x123720, 0x122fd8, 0x1232f8, 0x123600, 0x1244e0,
              0x6a330, 0x9d7d0, 0x6a470, 0xa8de0, 0x53c70, 0x64a60, 0x549d0,
              0xe5a74, 0xe5b6c, 0x7f43e, 0x9e490, 0x18db0, 0x51ac0, 0x2b770,
              0x7f9fe, 0x7fc20, 0x2b8b0, 0x7bb0, 0x7de0),
        regions=tuple({"role": role + "_hardware_vtable", "rva": rva, "bytes": 32}
            for role, rva in (("input_base", 0x2e5ac8), ("usb", 0x2e7e38),
                             ("ble", 0x2e3fd8), ("external", 0x2e5668), ("os_hook", 0x2e7958))))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Receiver hardware producer native functions are unavailable")
    dispatch = receiver_hardware_dispatch_proof(retained)
    sources = verified_host_owners(audit, ("electron/modules/mapping_engine/win/index.js",))
    source = sources["electron/modules/mapping_engine/win/index.js"]["original"]
    owners = {}
    for name in ("createCbHardwareEvent", "createCbDeviceEvent", "registerHardwareEvent",
                 "unregisterHardwareEvent", "setHardwareEventCallback", "createDeviceKey",
                 "getUSBDeviceInfoFromDeviceMap"):
        _, owners[name] = audit.callable_member("receiver_host_" + name, source,
            audit.tree(source).root_node, name)
    callback = audit.owners[owners["createCbHardwareEvent"]]["body"]
    if any(token not in callback for token in ('0===+t', 'hardwareEventList.includes(e.getURL())',
            'data:{buffer:JSON.parse(s)}', 'e.send("hardwareevent"')):
        raise ValueError("Receiver host hardware callback type/identity/raw-byte route differs")
    return {**provenance, "native": retained, "verified_callback_dispatch": dispatch,
        "host_sources": sources, "host_owners": owners, "complete_host_owners": audit.owners,
        "semantics": {
            "collections": "ab7f0 -> 11a702 -> 119de6 enumerates matching physical ContainerId/PID across HID collections. ac850 -> 123720 creates the reader; 1232f8 uses capacity100, 123600 passes actual completion length, 1244e0 copies exactly those bytes.",
            "adjusted_dispatch": "6a330 binds the base object; 9d7d0 adds128 before 6a470 invokes virtual slot24. The callback table is the +128 subobject: USB A8DE0, BLE53C70, external64A60, OS-hook549D0. Base table2e5a18 slot24/662b0 is not this hardware dispatch.",
            "classification": "USB A8DE0 selects raw[bluetoothLE] using E5B6C; class0..6 follows the respective input handler, class7 stream-controller80738, class8 hardware7f43e. Hardware bytes are never stripped. Missing descriptor/report binding selects class8; BLE and external have their separately retained dispatch branches.",
            "registration": "InputDevice registration uses the +136 interface: 6b7c0 stores registered at base640; 6bbd0 queues6bde0 to store callbacks at base648/704. 7f43e requires the global enabled bit, registered bit and callback, then posts an owned copy or the explicitly configured interval batch.",
            "callback": "7f43e -> 9e490 -> 18db0 invokes the callback bound by2ba40/51ac0 to2b770. 2b770 serializes every unsigned byte, supplies type0 and the captured timestamp. Host createCbHardwareEvent accepts type0, resolves physical identity and current subscriber URL, then publishes the raw buffer. createCbDeviceEvent accepts type5 and is a separate notification path.",
            "product_join": "Legacy product default reportIDs map4 to razerKey and5 to hardwareEvent. Published DualLink descriptors also merge slave reportIDs and add a slave key report. Defaults alone do not establish the live descriptor, parser override or product/version initialization state."},
        "limitations": ["The selected native producer is statically resolved; live product descriptors and current registration/parser branches must still be joined before changing a generic event adapter.",
            "No vendor export, device I/O or window was executed; full native and page acceptance remains unverified."],
        "target_execution": False, "runtime_acceptance": "unverified"}


def retain_global_shortcut_clock(audit):
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    retained = audit.native_slice(original, [], depth=0,
        rvas=(0xc94c0, 0xc760a, 0x189a0, 0x18db0, 0x1b7f50, 0x1a8970, 0x1a8c30, 0x1a8d30,
              0x1a9160, 0x1a9170, 0x1f1000, 0x1f0d40,
              0x4ca80, 0x141f0, 0x143e0, 0xc82b2, 0xc84d0, 0xc6ba4, 0xc9364, 0xc9790, 0xc97c0),
        regions=({"role": "timer_dispatch_slot_and_adjacent_data", "rva": 0x387248, "bytes": 16},
                 {"role": "timer_dispatch_slot_address", "rva": 0x3872a8, "bytes": 8},
                 {"role": "main_register_vtable_dispatch", "rva": 0x2df010 + 856, "bytes": 8},
                 {"role": "hotkey_backend_vtable", "rva": 0x2e97c0, "bytes": 48}))
    if retained["failures"]:
        raise ValueError("Current global shortcut clock functions are unavailable")
    regions = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in retained["verified_data_regions"]}
    base = int(retained["image_base"], 16)
    slots = (("main_register_vtable_dispatch", 0, 0x141f0),
             ("hotkey_backend_vtable", 16, 0xc9790),
             ("hotkey_backend_vtable", 32, 0xc97c0))
    dispatch = []
    for role, offset, target in slots:
        actual, = struct.unpack_from("<Q", regions[role], offset)
        if actual - base != target:
            raise ValueError("Current shortcut register/enable virtual dispatch differs: " + role)
        dispatch.append({"region": role, "offset": offset, "target_rva": hex(target),
                         "actual_file_pointer": hex(actual)})
    return {**provenance, "native": retained,
        "clock_chain": ["c94c0: WM_HOTKEY(786) -> 1b7f50 before key modifier processing",
            "1b7f50 -> *(off3872a8); initial file pointer -> 1a8970 -> off387250",
            "1a8c30 selects 1a8d30 when QPF>0 and CPU gate is set, otherwise 1a9170",
            "1f1000 -> 1f0d40 CPU object offset41: invariant TSC CPUID80000007.EDX.bit8, or hypervisor CPUID1.ECX.bit31 plus Microsoft Hv signature at CPUID40000000",
            "1a8d30 returns integer QPC microseconds, split quotient/remainder above overflow threshold",
            "off387248 points to 1a9160 __imp_timeGetTime thunk; 1a9170 extends its unsigned millisecond counter across rollover and multiplies by 1000",
            "c760a -> 189a0(a4), JSON virtualKey/modifiers/argument -> 18db0(callback,string,tick)"],
        "timestamp_capture": "Native WM_HOTKEY callback entry, not a later service drain or UI clock",
        "verified_register_enable_dispatch": dispatch,
        "registration_chain": [
            "registerGlobalShortcut 4ca80 -> main vtable2df010+856 -> 141f0 queued RegisterHotkey -> 143e0",
            "143e0 -> c82b2 queued RegisterAcceleratorOnAnyThread -> c84d0 -> c6ba4",
            "c9364 installs backend vtable2e97c0; its +32 slot is c97c0 and +16 slot is c9790",
            "c97c0 calls OS RegisterHotKey and retains the actual key/id on success; OS errors produce failure",
            "c6ba4 invokes backend +16 Enable after successful backend +32 Register; c9790 sets byte96=1 when bit0 is clear and key tree at offset80 is nonempty",
            "StreamMixer therefore needs no separate invented enableGlobalShortcut JS call after successful registration"],
        "target_execution": False, "runtime_acceptance": "unverified"}


def retain_global_shortcut_storage_lifetime(audit):
    """Verify the native storage acknowledgement and owning DLL lifetime."""
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    retained = audit.native_slice(original, ("localStorageSetItem", "mappingEngineShutdown",
        "disableMapping", "enableMapping", "disableGlobalShortcut", "enableGlobalShortcut"), depth=0,
        rvas=(0x1e454, 0x1000, 0x1060, 0x2090, 0x2510, 0x1fdd0, 0x20680,
              0x2fa0, 0x3160, 0xd46ec, 0xd4900, 0xd52d2, 0xd539c, 0x562c8,
              0xe95e0, 0xe9720, 0xe9a2c, 0xdb930, 0xeb281, 0xea410, 0xea7de,
              0x77550, 0xaa588, 0x19c30, 0x19cb0, 0x19d30, 0x19ed0,
              0x1a010, 0x1a150, 0x1a290, 0x1a3c0, 0x1a440, 0x1a4c0, 0x1a620,
              0x56b40, 0x56e70, 0x56f10, 0x57090, 0x571e0, 0x57e60,
              0x6b90, 0x6900, 0x6d50, 0x6ac0, 0x19180, 0x55de4,
              0x57f70, 0x5804c, 0x580b0, 0x58192, 0x66870, 0x66d10,
              0x14fb0, 0x15280, 0x15170, 0x15440, 0xc8cd2, 0xc6fb8,
              0xc9246, 0xc9282, 0x1ae04),
        regions=({"role": "main_storage_and_shutdown_vtable", "rva": 0x2df010, "bytes": 896},
                 {"role": "global_shortcut_data_vtable", "rva": 0x2ec538, "bytes": 40},
                 {"role": "device_manager_active_mapping_vtable", "rva": 0x2e4a88, "bytes": 32},
                 {"role": "active_mapping_initial_value", "rva": 0x3811d8, "bytes": 1}))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Current shortcut storage/lifetime native owners are unavailable")
    base = int(retained["image_base"], 16)
    regions = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in retained["verified_data_regions"]}
    if regions["active_mapping_initial_value"] != b"\x01":
        raise ValueError("Current native active mapping initial byte differs")
    slots = (("main_storage_and_shutdown_vtable", 16, 0x2090),
             ("main_storage_and_shutdown_vtable", 32, 0x2fa0),
             ("global_shortcut_data_vtable", 8, 0xe9720),
             ("main_storage_and_shutdown_vtable", 216, 0x6b90),
             ("main_storage_and_shutdown_vtable", 224, 0x6900),
             ("main_storage_and_shutdown_vtable", 880, 0x14fb0),
             ("main_storage_and_shutdown_vtable", 888, 0x15280),
             ("device_manager_active_mapping_vtable", 16, 0x57f70),
             ("device_manager_active_mapping_vtable", 24, 0x580b0))
    dispatch = []
    for role, offset, target in slots:
        actual, = struct.unpack_from("<Q", regions[role], offset)
        if actual - base != target:
            raise ValueError("Current shortcut storage/lifetime dispatch differs: " + role)
        dispatch.append({"region": role, "offset": offset, "target_rva": hex(target),
                         "actual_file_pointer": hex(actual)})
    return {**provenance, "native": retained, "verified_dispatch": dispatch,
        "storage_chain": [
            "20920 localStorageSetItem -> 1e454 -> 1000 module-static MappingEngine singleton -> vtable+32 2fa0",
            "2fa0 posts 3160 on that engine's thread -> d46ec posts d4900 on its local-storage worker",
            "d4900 changes its in-memory key map and notifies observers, then separately posts callback true",
            "562c8 owns key-to-parser registration: synapseDefaultTurbos, synapseGlobalShortcuts, synapseMacros, synapsePhasedMacros, synapseTurbos",
            "GlobalShortcutData ctor e95e0 -> verified vtable+8 e9720 -> e9a2c JSON parser -> ea410 hash/mappings validation",
            "e9a2c initializes version=1 and globalHooking=false; db930/eb281 overwrite only when their optional reads succeed",
            "ea410 validates the source hash after removing hash/gamemode and parses the complete mappings array",
            "77550 publishes parsed shortcut data to its input device; aa588 applies source data and schedules affected outputs"],
        "shutdown_chain": [
            "1fed0 mappingEngineShutdown resolves the same module-static singleton; 1060 installs vtable2df010",
            "vtable+16 2090 marks that singleton uninitialized, posts 2510 and waits before invoking void completion 1fdd0",
            "2510 composes cleanup of that engine's owned service threads and device manager",
            "1a620 -> 56b40 sequences CleanupDevices571e0, Stop57090, ShutdownExecutors56f10, then CleanupLocalStorage56e70; storage owners are released after the stop/executor stages",
            "571e0 uses per-device cleanup completions and 57e60 removes the corresponding owned entry"],
        "capture_suspension_chain": [
            "28c20 disableMapping -> main vtable+216 6b90 -> engine-thread 6d50 -> instance+152 device manager vtable+24 580b0",
            "19180 constructs that instance's device manager with 55de4 and verified vtable2e4a88",
            "580b0 changes module-static byte3811d8 to false, then 58192 cancels owned input mappings and schedules 57090 pending-output cleanup",
            "66870 and 66d10 gate their global-shortcut EA2C6 lookup on byte3811d8; disabling active mapping also excludes the native appEngine global-shortcut branch",
            "28910 enableMapping -> main vtable+224 6900 -> 6ac0 -> device-manager+16 57f70 sets byte3811d8 true and invokes 5804c",
            "4d8c0 disableGlobalShortcut -> main+888 15280 -> 15440 retains its +224 HotkeyService disable lease at instance+232",
            "4d5b0 enableGlobalShortcut -> main+880 14fb0 -> 15170 releases only that instance's +232 lease",
            "These objects and the active-mapping byte belong to the loaded module, so changing them in one process does not suspend another process's mapping engine"],
        "switch_observations": {"initial_active_mapping": True,
            "enableMapping_already_target": {"result": False, "reason": "already enabled"},
            "disableMapping_already_target": {"result": False, "reason": "already disabled"},
            "enableGlobalShortcut_no_lease": {"result": False, "reason": "hotkey was not disabled"},
            "disableGlobalShortcut_existing_lease": {"result": True, "reason": "Warning: Already disabled"}},
        "acknowledgement_scope": "Successful native storage publication; parser acceptance, OS effects and device writes are separate observations",
        "process_scope": "MappingEngine state and cleanup ownership reside in the loaded DLL module; each independent service process owns its own instance",
        "limitations": ["The void shutdown completion has no boolean device-write result",
            "The storage acknowledgement does not confirm every parsed output or OS effect",
            "Bounded native ownership evidence does not prove every external device/service callback queue has drained"],
        "target_execution": False, "runtime_acceptance": "unverified"}


def native_ascii_pointer_table(table, strings, *, image_base, strings_rva, count):
    """Decode bounded x64 string pointers from already byte-verified regions."""
    if not 1 <= count <= 4096 or len(table) != count * 8:
        raise ValueError("Native ASCII pointer table length differs")
    values = []
    for index, (pointer,) in enumerate(struct.iter_unpack("<Q", table)):
        rva = pointer - image_base
        offset = rva - strings_rva
        if not 0 <= offset < len(strings):
            raise ValueError("Native ASCII pointer is outside its retained string region")
        end = strings.find(b"\0", offset, min(offset + 256, len(strings)))
        if end <= offset:
            raise ValueError("Native ASCII string is empty, unterminated or overlong")
        try:
            name = strings[offset:end].decode("ascii")
        except UnicodeDecodeError as error:
            raise ValueError("Native ASCII string has non-ASCII bytes") from error
        values.append({"index": index, "pointer": hex(pointer), "string_rva": hex(rva), "name": name})
    if len({row["name"] for row in values}) != count:
        raise ValueError("Native ASCII table contains duplicate names")
    return values


def retain_mapping_execution_dependencies(audit):
    """Retain actual type identities and reviewed dispatch/lookup dependencies."""
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    identities = (
        ("keyboard_identity_vtable", 0x2f0438, (0x101a40, 0x1007c0, 0x10cc20, 0x10cc50,
            0x10ccc0, 0x10cd90, 0xfb7a0, 0x10cfe0, 0x10d180,
            0x100530, 0x10d320, 0x10d330, 0x10d340)),
        ("mouse_identity_vtable", 0x2f05d8, (0x102730, 0x1007c0, 0x10d900, 0x10d930,
            0x10d990, 0x10da10, 0xfb7a0, 0x10dbf0, 0x10dd90,
            0x10dfd0, 0x10e030, 0x10e0a0, 0x10e110)),
        ("macro_data_vtable", 0x2ecbe8, (0xa6f80, 0xf6a70, 0xf6f60, 0xf7000, 0xf8890)),
    )
    functions = (0xfbb74, 0xfbc28, 0xee0f1, 0xee5a1, 0xef470,
        0xe72fa, 0xe75a0, 0xe8518, 0xe871e, 0xe8888, 0xe9a2c,
        0xf8e3e, 0xf8f74, 0xf9118, 0xf92bc, 0xf9460, 0xf9604,
        0xf9c96, 0xfa4d2, 0xfa742, 0xfa90e,
        0x10bc0d, 0x10bfed, 0x10cbc4, 0x10d8a8, 0x10d3d2, 0x10d3f8,
        0xed10b, 0xed512, 0xed8d2, 0xf2ac9, 0xf2f76, 0xf1dc1, 0xf38eb, 0xf36f2,
        0xf49f8, 0x1020a8, 0x10215c,
        0x797a8, 0xad38c, 0xae7e2, 0x858fe, 0x85704, 0x8ab6a,
        0x893dc, 0x8ca44, 0xae010, 0x85ea8, 0x12ee96, 0xa2f1a, 0xa32e6,
        0xf68c0, 0xf69c6, 0xf7066, 0x5cf60, 0x562c8,
        0x57f70, 0x5804c, 0x580b0, 0x58192, 0x582e0, 0x583bc,
        0xf6bd0, 0xf7106, 0xf86ae, 0xf8bd4, 0xb0ed0, 0xecee3,
        0x127060, 0x1272ea, 0x127490, 0x127574, 0x12772a, 0x127760,
        0x127892, 0x127aa0, 0x1283a0, 0x128692,
        0x12a8ca, 0x12aad4, 0x12ab1e, 0x12ab8e, 0x12af54,
        0x12b4d8, 0x12b55a, 0x12b5b0, 0x12bde4, 0x12be6e,
        0x12c8f0, 0x12ca5c, 0x12cb32, 0x12d770, 0x12dcf0,
        0x12ea6c, 0x12eb04, 0x12eba2, 0x12ed24, 0x12edae, 0x12ee42,
        0x12f292, 0x12f312, 0x12f3ce, 0x12fb30, 0x12fd66, 0x12fe22, 0x12ff70,
        0x130b68, 0x130bba, 0x130c08, 0x130c1a, 0x130cf8, 0x130dc6, 0x1314d0,
        0x126902, 0x126a80, 0x126b3e, 0x126c80, 0x1345aa, 0x134640, 0x134674,
        0x11a950, 0x11a922, 0x11aa78, 0x11c510, 0x11cbd3,
        0x1000, 0x1060, 0x2fa0, 0x3160, 0x17ee0, 0x18010, 0x18130, 0x182a0,
        0x19a40, 0x1a4c0, 0x1a8e0, 0x1d190, 0x1d1a4, 0x1d38e, 0x1e454, 0x1fb30,
        0x1fdd0, 0x20680, 0x20920, 0x519fc, 0x56850, 0x56b40, 0x56e70, 0x56f10,
        0x57090, 0x571e0, 0x57c64, 0x57cd4, 0x5cb50, 0x5cc20, 0x5ccb0, 0x5ce40,
        0x5d020, 0x5d600, 0x5d770, 0x5d890, 0x5dc90, 0x5de50, 0x5e4e0, 0x5e600,
        0x5e710, 0x5eb10, 0x5eeec, 0x5f086, 0x61aa0, 0x6237c, 0x62510, 0x89968,
        0x8d1b8, 0x8dc00, 0x8edfc, 0x8f0e4, 0x8f5e0, 0xa3910, 0xa9500, 0xb0ab0,
        0xb0c20, 0xb0cd0, 0xb0ef0, 0xb10d0, 0xb1150, 0xb11d0, 0xb1286, 0xb1480,
        0xb1640, 0xb215c, 0xb2164, 0xb24b8, 0xb2640, 0xb2732, 0xb28c0, 0xb2986,
        0xb2994, 0xb29f8, 0xb2ca0, 0xb3030, 0xb3180, 0xb33d0, 0xb351a, 0xb3760,
        0xb3af0, 0xb3e80, 0xb3fd2, 0xb4220, 0xb436c, 0xb45b0, 0xb4a42, 0xb4b2c,
        0xb4bfc, 0xb4c70, 0xb4cf0, 0xb4d40, 0xb4e80, 0xb4fd0, 0xb5110, 0xb5250,
        0xb5390, 0xb54e0, 0xb5620, 0xb5788, 0xb598e, 0xb59c0, 0xb5cd2, 0xb5e9a,
        0xb6100, 0xb61d0, 0xb6210, 0xb6220, 0xb6230, 0xb6280, 0xb62c0, 0xb62f0,
        0xb6320, 0xb63f0, 0xb74fc, 0xb7850, 0xb79e0, 0xb82e0, 0xb8490, 0xb8c00,
        0xb9050, 0xb90e0, 0xb91a0, 0xb9560, 0xb9800, 0xb9f3c, 0xba1b8, 0xba770,
        0xba910, 0xbb6b6, 0xbbc50, 0xbc2e0, 0xbc390, 0xbc400, 0xbc5e0, 0xbc6de,
        0xbc970, 0xbcdd0, 0xbd090, 0xbd190, 0xbd2f0, 0xbd980, 0xbd9d0, 0xbda00,
        0xbda90, 0xbdb50, 0xbe4b0, 0xbe646, 0xcf650, 0xd3c70, 0xd3d1e, 0xd46ec,
        0xd4900, 0xd52d2, 0xd539c, 0xd570c, 0xdb873, 0xdb930, 0xedada, 0xedce6,
        0xfcab2, 0xfd33a, 0xfd72c, 0x101ff0, 0x102eb0, 0x107a24, 0x107b2a, 0x107bd0,
        0x107d30, 0x1080b2, 0x10be63, 0x10e270, 0x10e2f0, 0x10f582, 0x110b5a, 0x112c60,
        0x114850, 0x115da0, 0x1186c0, 0x118840, 0x11ae38, 0x11b849, 0x11bc70, 0x11be52,
        0x11c460, 0x1219dc, 0x125da8, 0x128e76, 0x1295ca, 0x1297d8, 0x129998, 0x134af0,
        0x134b64, 0x134c80, 0x13579a, 0x135880, 0x135c20, 0x1a7cf0, 0x1a9710, 0x1aba20,
        0x1abf10, 0x1b1430, 0x1b1720, 0x1b2dd0, 0x1b3a30, 0x1b4940, 0x1b8590, 0x1b8d40,
        0x1b9fa0, 0x1dc35e, 0x1dc768, 0x1ddbae, 0x1ef7e0, 0x1ef8e0, 0x1ef9c0, 0x1efa60,
        0x1efc70, 0x1f3600, 0x22beaa, 0x22c704, 0x22c85a, 0x22e640, 0x22e730, 0x22e7ad,
        0x22e830)
    regions = ({"role": "mapping_type_pointers", "rva": 0x2ed830, "bytes": 43 * 8},
        {"role": "mapping_type_strings", "rva": 0x2e2b54, "bytes": 0xa90},
        {"role": "macro_movement_pointers", "rva": 0x2ecd20, "bytes": 4 * 8},
        {"role": "macro_movement_strings", "rva": 0x2e2b03, "bytes": 0x820},
        {"role": "macro_movement_driver_flags", "rva": 0x2f28e0, "bytes": 12},
        {"role": "macro_movement_coordinate_scale", "rva": 0x2f2840, "bytes": 4},
        {"role": "macro_storage_observer_vtable", "rva": 0x2e4c88, "bytes": 40},
        {"role": "macro_changed_observer_vtable", "rva": 0x2e4da8, "bytes": 16},
        {"role": "phased_macro_changed_observer_vtable", "rva": 0x2e4dc8, "bytes": 16},
        {"role": "phased_macro_data_observer_vtable", "rva": 0x2ef688, "bytes": 16},
        {"role": "audio_engine_observer_vtable", "rva": 0x2df430, "bytes": 40},
        {"role": "audio_service_vtable", "rva": 0x2e8aa0, "bytes": 152},
        {"role": "audio_monitor_vtable", "rva": 0x2e8b40, "bytes": 152},
        {"role": "audio_volume_callback_vtable", "rva": 0x2e8bf0, "bytes": 32},
        {"role": "audio_device_callback_vtable", "rva": 0x2e8c10, "bytes": 64},
        {"role": "audio_endpoint_volume_iid", "rva": 0x2e8d00, "bytes": 16},
        {"role": "audio_volume_percent_divisor", "rva": 0x2e8a98, "bytes": 4},
        {"role": "audio_action_id_pointers", "rva": 0x2edb50, "bytes": 56},
        {"role": "clipboard_action_id_pointers", "rva": 0x2edcf0, "bytes": 16},
        {"role": "clipboard_endpoint_paste_slot", "rva": 0x2e8e50 + 32, "bytes": 8},
        {"role": "clipboard_windows_write_slots", "rva": 0x33ce70 + 256, "bytes": 16},
        {"role": "launch_shell_open_verb_utf16", "rva": 0x2f112a, "bytes": 10})
    retained = audit.native_slice(original, (), depth=0,
        rvas=tuple(sorted(set(functions) | {target for _, _, targets in identities for target in targets})),
        regions=regions + tuple({"role": name, "rva": rva, "bytes": 8 * len(targets)}
            for name, rva, targets in identities))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Current mapping execution dependencies are unavailable")
    data = {row["role"]: bytes.fromhex(row["raw_hex"]) for row in retained["verified_data_regions"]}
    base = int(retained["image_base"], 16)
    types = native_ascii_pointer_table(data["mapping_type_pointers"], data["mapping_type_strings"],
        image_base=base, strings_rva=0x2e2b54, count=43)
    expected = ("analogJoystick", "analogJoystickV2", "analogKey", "analogStick", "analogXboxGamepad",
        "analogXboxGamepadV2", "audio", "cascade", "clipboard", "controlPod", "coolingPad", "copyText",
        "delay", "disabled", "display", "doubleMapping", "dpad", "external", "hypershift", "joystick",
        "joystickInput", "keyboard", "launch", "loop", "macro", "mode", "modtap", "mouse", "mouseMove",
        "multi", "otfs", "phasedMacro", "randomDelay", "rapidTrigger", "razerEvent", "razerKey",
        "streamController", "system", "triggers", "turbo", "unsupported", "xboxGamepad", "xboxInput")
    if tuple(row["name"] for row in types) != expected:
        raise ValueError("Current mapping type enum differs from reviewed dispatch")
    movement_types = native_ascii_pointer_table(data["macro_movement_pointers"],
        data["macro_movement_strings"], image_base=base, strings_rva=0x2e2b03, count=4)
    if tuple(row["name"] for row in movement_types) != ("none", "absolute", "foreground", "relative"):
        raise ValueError("Current macro movement type enum differs from reviewed execution")
    movement_flags = struct.unpack("<III", data["macro_movement_driver_flags"])
    movement_scale, = struct.unpack("<f", data["macro_movement_coordinate_scale"])
    if movement_flags != (1, 1, 3) or movement_scale != 65535.0:
        raise ValueError("Current macro movement constants differ from reviewed execution")
    dispatch = []
    for name, rva, targets in identities:
        actual = tuple(pointer - base for pointer, in struct.iter_unpack("<Q", data[name]))
        if actual != targets:
            raise ValueError("Current mapping identity virtual dispatch differs: " + name)
        dispatch.append({"role": name, "table_rva": hex(rva),
            "targets": [{"offset": index * 8, "rva": hex(target)} for index, target in enumerate(actual)]})
    assembly_proofs = (
        (0x12a8ca, ((0x12aa3a, "41807e2800"), (0x12aa4d, "01ed"),
            (0x12aa54, "bd01000000"), (0x12aa95, "896c2428")),
            "Final callback status is 1 for failed execution, otherwise twice the stop-event state: 0 completed or 2 cancelled. Hex-Rays omits these call arguments"),
        (0x130dc6, ((0x131333, "440fafcd"), (0x131354, "0fafcb"),
            (0x131375, "f30f5bd2"), (0x131387, "f3440f2cc4"),
            (0x13138c, "f30f5bc3"), (0x1313a2, "f3440f2cc9")),
            "Foreground coordinates use wrapping int32 multiplication, float32 scaling and two truncating float-to-int32 conversions. The complete assembly resolves the local-variable-allocation warning"),
    )
    captured = {int(row["capture"]["rva"], 16): row["capture"] for row in retained["complete_functions"]}
    verified_assembly = []
    for owner, instructions, semantics in assembly_proofs:
        actual = {int(row["rva"], 16): row for row in captured[owner]["assembly"]}
        if any(actual.get(rva, {}).get("bytes") != raw for rva, raw in instructions):
            raise ValueError("Current macro execution assembly differs at owner " + hex(owner))
        verified_assembly.append({"owner_rva": hex(owner), "semantics": semantics,
            "instructions": [actual[rva] for rva, _ in instructions]})
    return {**provenance, "native": retained, "types": types, "verified_identity_dispatch": dispatch,
        "movement_types": movement_types, "movement_driver_flags": movement_flags,
        "movement_coordinate_scale": movement_scale, "verified_execution_assembly": verified_assembly,
        "reviewed_semantics": [
            "fbb74 compares exactly 43 case-sensitive type names; unmatched text produces index43. Captured bytes, not a presumed enum order, bind parser and runtime dispatch",
            "ee5a1 accepts input types2,3,9,10,17,21,27,34,35,36 and validates the constructed object's virtual validity predicate",
            "ee0f1 adds hypershift/gamemode/otfs, DKP or SnapTap metadata and validates interrupt input objects; break-form and duplicate interrupt identities are rejected",
            "e8888 parses required input/output objects, rejects duplicate canonical input serialization, checks phasedMacro make/break compatibility and can remove source-equal input/output mappings",
            "f8e3e chooses the lookup bucket in gamemode, otfs, dkp, snaptap, ordinary priority, always including hypershift. Bucket identity is vtable+56, not raw input JSON",
            "Ordinary macro storage observers5cb50/5cc20/5ccb0 publish raw data independently of f6bd0 candidate parsing. Same GUID/hash reuses old events; successful batches swap maps atomically, deletion omits changed-GUID cancellation and clear does not cancel pending parses.56b40 cleanup orders devices, executions, executors, then storage owners",
            "Clipboard text requests queue an independent paste sequence; wait completion precedes the500ms prioritized single-format restoration. Failures of clipboard writes or paste keys do not become ordinary macro frame errors. Macro cancellation does not cancel the queued paste/recovery timer",
            "Launch checks optional pathToCheck, uses CreateProcessW then ShellExecuteExW(open), and closes owned handles without waiting for or terminating the new process. Macro status reflects launch request acceptance, not command completion",
            "Audio seven-ID parser validates volume0..100 and mute0..2. COM setters run asynchronously on the shared service; ordinary macro does not await their bool/reason. Initial monitor HRESULT and reinitialization HRESULT may be discarded; role is ignored in default-device notification. Initialized cache/default-change event does not establish an observed endpoint value",
            "Audio observer identity/serial rejects undelivered stale tasks, but17ee0/18010 capture hooks into a second queue with no repeated engine-initialized check. Global1a4c0 cleanup removes hooks/subscriptions and stops monitors, which is distinct from stopping one macro",
            "Keyboard vtable+40 emits type/scancode/flag/extraInformation/modifiers plus present delayInMs/releaseAfterMs/unitId; mouse emits type/id/data/extraInformation/modifiers and present unitId",
            "Keyboard vtable+56 preserves the complete flag while vtable+64 clears break bit0; mouse vtable+64 combines each button make/break pair into one mask, while wheel identities use vtable+56 unchanged",
            "Source equality vtable+32 compares keyboard timing presence and values, modifiers and optional unit; mouse compares packed id/data, modifiers and optional unit. It is distinct from full serialized identity",
            "797a8 selects ae7e2 only for a global mapping with global-data version>=2; otherwise ad38c dispatches. Device cases cascade,dpad,joystick,phasedMacro,xboxGamepad are absent from the global>=2 dispatcher",
            "The source unsupported output parser f49f8 retains the serialized output object. Only the source dispatch default invokes 858fe/85704; unresolved Rust handlers cannot be reclassified as unsupported",
            "Multi executes outputs in list order; keyboard goes through ae010, other outputs clone the mapping, replace its output and re-enter797a8. It stops on mapping allocation failure",
            "893dc macro flags0,2,4 insert by input vtable+64 identity, flag1 erases that key, and flag3 inserts then invokes89968 after clearing byte912; actual macro scheduling continues through the device delegate",
            "ae010 routes keyboard through KeyboardExecutor12ee96 when present, otherwise through driver-thread10f582 with fallback callback a3910. Mouse output85ea8 uses that driver thread path",
            "f68c0 owns MappingEngineMacroDataThread and synchronized macro maps; f7066 queries the current compiled map by GUID. Publication, parser state and runtime execution are separate boundaries",
            "f6a70 only queues publication. f6bd0 swaps staging into the live map under its exclusive lock and queues changed-GUID notification only after all of f7106 succeeds; failure preserves the prior live map",
            "f7106 requires an array of unique nonempty GUIDs with appEngine and hash, reuses the live object for an equal hash, otherwise verifies the hash and movement enum, and copies the required events array. Individual events are parsed during execution, not publication",
            "128692 rejects a missing or empty macro and rejects recursion at 100 live macro frames. Repeat=0 marks indefinite repetition; nested completion retains the outer state. 12b5b0 checks cancellation and prior output failure before every step",
            "ecee3 parses audio,clipboard,delay,keyboard,launch,loop,macro,mouse,mouseMove,randomDelay events; nested macro flags>=2 fail. 12be6e forwards the actual accepted boolean to12c8f0; a consecutive keyboard event waits10000 microseconds before enqueueing",
            "12cb32 owns one optional loop frame: a start with nonzero repeat replaces its start/count; an end records the continuation and advances to the array end. 12b5b0 repeats that range or resumes its continuation. It does not implement a general nested-loop stack",
            "12ca5c tracks successful make outputs by release-neutral identity and removes matching breaks. At top-level completion12a8ca clones each remaining held output, calls verified vtable+96 to form a break and sends it through130c1a/130cf8 before clearing held state",
            "12a8ca ignores release-send return values, clears held state, requests endpoint close, releases the relative-motion lease and requests keyboard stop. Its final delegate status is0 for completed,1 for failed,2 for cancelled; this status does not prove cleanup delivery. Complete assembly supplies arguments omitted by Hex-Rays",
            "127574 signals the stop event before queueing shutdown. 127760/12d770 wait for endpoint close completion, destroy that endpoint and request keyboard executor shutdown before posting owner completion;127060 joins the execution thread before releasing fields",
            "12f312 derives repeat delay from SPI_GETKEYBOARDDELAY as250*n+250ms and repeat interval from SPI_GETKEYBOARDSPEED as truncated1000/(n*0.887+2.5), with source defaults250ms/30ms on unavailable settings",
            "12f3ce retains pressed keys, timing option presence, active output and generation tick. 12fb30 subtracts250ms from auto-release counters and releases only below zero. 12ff70 rejects stale output/tick callbacks; smart-stop ends repeating without inventing a break event",
            "130c1a queues through the dedicated keyboard executor when present; otherwise130cf8 uses a ready driver endpoint or11c510 SendInput fallback. Queue acceptance is distinct from final driver/OS delivery and cleanup results",
            "130dc6 uses the active macro movement enum: absolute passes coordinates, foreground captures the actual foreground rectangle and monitor metrics for a movement run, relative first captures an origin/lease and later sends deltas. Each macro repetition resets the relative origin",
            "1314d0 uses actual driver flag bytes1,1,3 for absolute,foreground,relative and forwards asynchronous completion. The11cbd3 OS fallback requires SendInput==1; its absolute/foreground flags are0x8001 and relative flags1, without an invented VIRTUALDESK flag"],
        "decompiler_limitations": [
            "130dc6 reports local variable allocation failure; coordinate arithmetic was checked against the complete assembly and its actual constant bytes",
            "12a8ca omits final delegate call arguments in pseudocode; retained instruction proofs establish status computation and stack argument placement",
            "Some other captured helper signatures and callback arguments remain incomplete in Hex-Rays; their full assembly is retained and unresolved call chains remain below"],
        "remaining_closure": [
            "Full per-device/global input lookup priority, all product/accessory constructors and matching predicates",
            "Macro storage transaction/broadcast, nested macro/loop/turbo/phased-macro scheduling, interruption and complete release of held outputs",
            "Every output executor, keyboard timing/smart-stop state, driver discovery/capability wiring and completion/fallback propagation",
            "Enable/disable manager virtual calls, live mode producers and mode observers across all service/device owners",
            "The captured profile/global-shortcut storage bodies e75a0/e9a2c still require full semantic review; capture alone is not implementation"],
        "source_chain_complete": False, "independent_execution_connected": False,
        "target_execution": False, "runtime_acceptance": "unverified"}


KEYPAD_MAPPING_TABLES = (
    ("analog_stick_input", 0x2ed2a8, (0xa8dd0, 0x1007c0, 0xfc1d0, 0xfc1e0,
        0xfc230, 0xfc290, 0xfb7a0, 0xfc400, 0xfc560, 0xfc6a0, 0xfc6b0, 0xfc6c0, 0xfc6d0)),
    ("dpad_output", 0x2ee338, (0xff390, 0x52530, 0xff3a0, 0xff3f0, 0xfcf70, 0xfd050, 0xff650)),
    ("joystick_output", 0x2ee608, (0x1009d0, 0x52530, 0x1009e0, 0x1009f0, 0x100a60, 0x100ae0, 0xe2b10)),
    ("joystick_state", 0x2ee6e8, (0x100dd0, 0x52530, 0x100de0, 0x100e20, 0x100ed0, 0x100f30, 0x101790)),
    ("input_device_mapping_delegate", 0x2e5a78, (0x55680, 0x665e0, 0x66870, 0x66d10, 0x69530)),
)


def verify_retained_native_pe(raw, retained, label):
    """Bind complete retained owners and data to their file-backed PE bytes."""
    pe = inspect_pe(raw)
    base = int(pe["image_base"], 16)
    if base != int(retained["image_base"], 16):
        raise ValueError(label + " image base differs")

    def actual(rva, size):
        section = next((row for row in pe["sections"] if row["rva"] <= rva
            and rva + size <= row["rva"] + row["bytes"]), None)
        if section is None or size <= 0:
            raise ValueError(label + " evidence is outside file-backed PE sections")
        offset = section["file_offset"] + rva - section["rva"]
        body = raw[offset:offset + size]
        if len(body) != size:
            raise ValueError(label + " evidence exceeds the PE file")
        return body

    functions, instruction_count = {}, 0
    for row in retained["complete_functions"]:
        function = row["capture"]
        rva = int(function["rva"], 16)
        if rva in functions or int(function["address"], 16) != base + rva:
            raise ValueError(label + " function identity is repeated or inconsistent")
        if not function["chunks"] or not function["assembly"]:
            raise ValueError(label + " complete machine-code owner is absent")
        if not function["pseudocode"].get("lines"):
            raise ValueError(label + " complete pseudocode is absent")
        for chunk in function["chunks"]:
            body = bytes.fromhex(chunk["raw_hex"])
            start, end = int(chunk["start"], 16), int(chunk["end"], 16)
            if (end - start != len(body) or actual(start - base, len(body)) != body
                    or hashlib.sha256(body).hexdigest() != chunk["sha256"]):
                raise ValueError(label + " function chunk differs from PE bytes")
        for instruction in function["assembly"]:
            body = bytes.fromhex(instruction["bytes"])
            address = int(instruction["address"], 16)
            if (address - base != int(instruction["rva"], 16)
                    or not any(int(chunk["start"], 16) <= address
                        and address + len(body) <= int(chunk["end"], 16) for chunk in function["chunks"])
                    or actual(address - base, len(body)) != body):
                raise ValueError(label + " instruction differs from PE bytes or owner")
        functions[rva] = function
        instruction_count += len(function["assembly"])

    regions = {}
    for row in retained["verified_data_regions"]:
        body = bytes.fromhex(row["raw_hex"])
        if (row["role"] in regions or len(body) != row["bytes"]
                or actual(int(row["rva"], 16), len(body)) != body
                or hashlib.sha256(body).hexdigest() != row["sha256"]):
            raise ValueError(label + " data region differs from PE bytes")
        regions[row["role"]] = body
    return base, functions, regions, instruction_count, actual


def keypad_mapping_constants(raw, retained):
    """Bind reviewed dispatch and arithmetic to full captures and actual PE bytes."""
    base, functions, regions, instruction_count, actual = verify_retained_native_pe(raw, retained, "Keypad mapping")
    dispatch = []
    for role, rva, targets in KEYPAD_MAPPING_TABLES:
        body = regions[role]
        values = tuple(value - base for value, in struct.iter_unpack("<Q", body))
        if values != targets or any(target not in functions for target in targets):
            raise ValueError("Keypad mapping virtual dispatch differs: " + role)
        dispatch.append({"role": role, "rva": hex(rva),
            "targets": [{"slot_bytes": slot * 8, "rva": hex(target)} for slot, target in enumerate(values)]})

    def strings(role, expected):
        values = []
        for pointer, in struct.iter_unpack("<Q", regions[role]):
            rva = pointer - base
            value = bytearray()
            for offset in range(64):
                byte = actual(rva + offset, 1)[0]
                if byte == 0:
                    break
                value.append(byte)
            else:
                raise ValueError("Keypad mapping string is unterminated")
            values.append(value.decode("ascii"))
        if tuple(values) != expected:
            raise ValueError("Keypad mapping string table differs: " + role)
        return values

    axes = strings("analog_stick_axis_names", ("left", "right", "up", "down", "invalid"))
    joystick = strings("joystick_id_names", ("button", "axes", "raxes", "pov"))
    proofs = (
        (0x7eb92, ((0x7ebe5, "b8204e0000"), (0x7ebf5, "66f7d9"),
            (0x7ebfc, "0fb7d9"), (0x7ec0b, "39dd"), (0x7ec0d, "766d"),
            (0x7ecf7, "80f203"), (0x7ecff, "41b001"), (0x7ed12, "4531c0")),
            "The native threshold fallback is 20000; signed int16 magnitude uses unsigned16, including -32768. Equality activates. Make/break axis is derived from the current sample sign."),
        (0xf27ed, ((0xf28c6, "83f87f"), (0xf28cb, "b87f000000"),
            (0xf28d0, "83f881"), (0xf28d5, "b880ffffff")),
            "Joystick xyz clamps values >=127 to127 and values <-127 to-128; -127 is retained. This asymmetric lower branch is not a conventional -127 clamp."),
        (0xfc6d6, ((0xfc6d6, "885138"), (0xfc6d9, "44884139")),
            "AnalogStick stores axis at byte56 and flag at byte57."),
    )
    verified = []
    for owner, rows, semantics in proofs:
        instructions = {int(item["rva"], 16): item for item in functions[owner]["assembly"]}
        if any(instructions.get(rva, {}).get("bytes") != body for rva, body in rows):
            raise ValueError("Keypad mapping reviewed arithmetic differs: " + hex(owner))
        verified.append({"owner_rva": hex(owner), "instructions": [instructions[rva] for rva, _ in rows],
            "semantics": semantics})
    return {"dispatch": dispatch, "axis_names": axes, "joystick_ids": joystick,
        "instruction_proofs": verified, "functions_verified": len(functions),
        "instructions_verified": instruction_count,
        "verification_scope": "All retained chunk and instruction bytes equal the official PE; mnemonic/operand interpretation uses the retained IDA capture, without an independent x86 decoder"}


def collect_keypad_mapping_static(audit):
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    functions = (0x64e8c, 0x79e9e, 0x7eb92, 0xa4a36, 0xa4d6a, 0xe64e0,
        0xe381e, 0xe383a, 0xebef2, 0xee98a, 0xee0f1, 0xee5a1, 0xef470,
        0xfa90e, 0xfa54c, 0xfbc50, 0xfc6d6, 0xf2243, 0xf5027, 0xff48c,
        0xff5ae, 0xff676, 0x861ca, 0x87b6e, 0x88914, 0x88872, 0x87b02,
        0x88e20, 0x872c6, 0xf27ed, 0x100910, 0x100d0e, 0x100d1e,
        0x1011f4, 0x10132c, 0xaf2ae, 0x797a8, 0xad38c,
        0x677ba, 0x69728, 0x68d0c)
    retained = audit.native_slice(original, (), depth=0,
        rvas=tuple(sorted(set(functions) | {value for _, _, values in KEYPAD_MAPPING_TABLES for value in values})),
        regions=tuple({"role": role, "rva": rva, "bytes": len(values) * 8}
            for role, rva, values in KEYPAD_MAPPING_TABLES) + (
                {"role": "analog_stick_axis_names", "rva": 0x2edaa0, "bytes": 40},
                {"role": "joystick_id_names", "rva": 0x2ee640, "bytes": 32}))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Keypad mapping complete native owners remain unavailable")
    constants = keypad_mapping_constants(audit.read(original), retained)
    return {"schema_version": 1, "scope": "Current shared keypad mapping native consumers; product747 frontend and OBM are separate source joins",
        "provenance": provenance, "native": retained, "verified_constants": constants,
        "semantics": {
            "input_parser": "ee98a accepts exact axis strings left/right/up/down with enum0/1/2/3 and uint8 flag0/1. It allocates type3, then writes bytes56/57. fc400 identity contains type,axis,flag; fc560 stateless identity omits flag. vtable+72/+80 identify make/break.",
            "threshold": "Optional analogStick.threshold is parsed by ebef2 as signed16 and must be1..32766. Missing analogStick leaves the native AnalogStick constructor/reset default20000; this is a native mapping threshold, not a device read or permission to write device defaults.",
            "input_samples": "79e9e parses report triples starting at byte1: inputID plus signed big-endian16 value, stopping on inputID0. When mapping is enabled, IDs251/252 reach7eb92 unless already excluded by the active-input tree; absent previously held entries reach7eb92 with0. Other IDs use ordinary analog-key handling.",
            "edge_state": "7eb92 compares unsigned magnitude with abs(threshold), including equality. It tracks held state by axis inputID alone: repeated above-threshold values and direct sign reversal do not emit another edge. Only insertion emits flag0; only removal below threshold emits flag1. ID251 maps positive to right and nonpositive to left; ID252 maps positive to up and nonpositive to down. The break axis uses the current sample sign, including zero; no previous-sign substitution is present.",
            "mapping_delegate": "Constructor64e8c installs the +120 subobject table2e5a78. Slots24/32 resolve to66d10/69530. Break lookup uses the active map and stateless identity; make lookup follows live priority, then the normal mapped-key path.69530 checks677ba, dispatches69728 then68d0c and otherwise797a8; native acceptance is distinct from final output delivery.",
            "dpad_parser": "f2243 requires a nonempty dpad array of objects. Each may have a parsable input; missing or nonobject input is treated as no condition by the exact optional EB468 guard. output is required and parsed with restricted f5027. Any element failure fails the candidate. ff48c retains list order and indexes outputs by input stateless identity, using an empty identity for unconditional entries.",
            "dpad_execution": "861ca snapshots active inputs under the owning lock, walks source dpad order and skips conditional entries absent from that snapshot. First matching candidate stops the search; it does not execute every candidate. Joystick19 and Xbox41 use aggregated controller state; other outputs clone the mapped key, substitute output and re-enter ad38c. The input make/break predicates control the pending dpad pair and release/re-selection chain88914/87b6e; lookup/allocation failure does not prove a successful output.",
            "joystick_parser": "f27ed selects exact id button/axes/raxes/pov. Button requires uint32 button plus uint8 flag0/1; runtime1011f4 applies only buttons1..24. Three-axis branches require exactly three JSON integers and clamp upper127/lower-128 as shown by the exact instruction proof.100ae0 serializes a button under value although the parser expects button; this source asymmetry is retained.",
            "joystick_aggregation": "872c6 mutates the live joystick state only when present.1011f4 maintains byte counters for buttons and keyed contributions for axes/raxes/pov.10132c removes the previous contribution for the same stateless input, inserts only a nonzero vector, then sums each axis strongest negative and positive values; the -128/+127 pair explicitly adjusts to0. af2ae sends changed button/axis fields through the driver queue and publishes via the actual controller consumer. Queueing is not transport ACK.",
            "failure_cleanup": "The retained native owners release reference-counted mappings, inputs, controller snapshots and owned strings on their explicit paths. No new device command, rollback, cancellation acknowledgment or hardware readback is inferred. Full device shutdown and all downstream executors remain separate lifecycles."},
        "remaining_closure": [
            "Join product747 Fe mapping compiler and OBM command/ACK owners to these native consumers",
            "Finish every dpad deferred pair branch, mode-priority branch and native output executor for every product and transport",
            "Independent Rust implementation, production integration, official actual-function comparison and real-device/window acceptance"],
        "source_chain_complete": False, "target_execution": False, "actual_dll_comparison": "unverified",
        "independent_execution_connected": False, "runtime_acceptance": "unverified"}


def audit_host_keypad_mapping(args):
    audit = HostLocalAudit()
    report = collect_keypad_mapping_static(audit)
    report.update(recorded_at_utc=audit.a.stamp(), tool=audit.families.identity(Path(__file__)),
        complete_originals=list(audit.sources.values()), verified_artifacts=list(audit.artifacts.values()))
    changed = audit.final_snapshot()
    if changed:
        raise ValueError("Keypad mapping inputs changed during query: " + ", ".join(changed))
    publish_host_static_query(report, audit.a.ROOT / ".analysis/results/host-current/local-audit.json", audit.a, "keypad_mapping")
    print(json.dumps({"query": args.query, "functions": report["verified_constants"]["functions_verified"],
        "instructions": report["verified_constants"]["instructions_verified"], "target_execution": False,
        "actual_dll_comparison": "unverified"}), flush=True)


MAPPING_DESCRIPTOR_OBSERVERS = (
    ("input_base", 0x64e8c, 0x64edb, 0x2e5a18, 0x55680),
    ("usb", 0xa89f0, 0xa8a45, 0x2e7d88, 0xa92a0),
    ("ble", 0x52158, 0x521ad, 0x2e3f28, 0x54000),
    ("external", 0x6319c, 0x631f1, 0x2e55b8, 0x64b90),
    ("os_hook", 0xa6b74, 0xa6bc9, 0x2e78a8, 0xa8930),
)

MAPPING_DESCRIPTOR_TABLES = (
    ("engine_storage_vtable", 0x2df010, (0x1b380, 0x1350, 0x2090, 0x2cc0, 0x2fa0, 0x3220, 0x3490)),
    ("device_data_vtable", 0x2ec2e8, (0x52530, 0xe3c90, 0xe5930, 0xe59d0, 0xe90c0)),
) + tuple((role + "_storage_vtable", table, (destructor, 0x65cc0, 0x65cc0, 0x662b0, 0x66590))
    for role, _, _, table, destructor in MAPPING_DESCRIPTOR_OBSERVERS)

MAPPING_DESCRIPTOR_REPORT_NAMES = (
    "analogKeyboardReportID", "analogKeyboardV2ReportID", "analogKeypadReportID",
    "controlPodReportID", "coolingPadReportID", "razerEventReportID", "razerKeyReportID",
    "streamControllerReportID", "hardwareEventReportID",
)


def mapping_descriptor_proof(raw, retained):
    """Resolve storage observers, publication gates and report classes from PE bytes."""
    base, functions, regions, instruction_count, actual = verify_retained_native_pe(raw, retained, "Mapping descriptor")
    region_rows = {row["role"]: row for row in retained["verified_data_regions"]}
    dispatch = []
    for role, rva, targets in MAPPING_DESCRIPTOR_TABLES:
        row, body = region_rows.get(role), regions.get(role, b"")
        if (row is None or int(row["rva"], 16) != rva or len(body) != len(targets) * 8
                or tuple(value - base for value, in struct.iter_unpack("<Q", body)) != targets
                or any(target not in functions for target in targets)):
            raise ValueError("Mapping descriptor virtual dispatch differs: " + role)
        dispatch.append({"role": role, "rva": hex(rva), "targets": [
            {"slot_bytes": slot * 8, "rva": hex(target)} for slot, target in enumerate(targets)]})

    def instructions(owner):
        if owner not in functions:
            raise ValueError("Mapping descriptor reviewed owner is absent: " + hex(owner))
        return {int(row["rva"], 16): row for row in functions[owner]["assembly"]}

    constructors = []
    for role, owner, lea, table in (("engine", 0x1060, 0x106d, 0x2df010),
            ("device_data", 0xe3b10, 0xe3b1f, 0x2ec2e8)):
        rows = instructions(owner)
        encoded = bytes.fromhex(rows.get(lea, {}).get("bytes", ""))
        if (len(encoded) != 7 or encoded[:3] != b"\x48\x8d\x05"
                or lea + 7 + struct.unpack_from("<i", encoded, 3)[0] != table
                or rows.get(lea + 7, {}).get("bytes") != "488901"):
            raise ValueError("Mapping descriptor primary constructor differs: " + role)
        constructors.append({"role": role, "owner_rva": hex(owner), "vtable_rva": hex(table),
            "this_offset": 0, "instructions": [rows[lea], rows[lea + 7]]})
    for role, owner, lea, table, _ in MAPPING_DESCRIPTOR_OBSERVERS:
        rows = instructions(owner)
        encoded = bytes.fromhex(rows.get(lea, {}).get("bytes", ""))
        if (len(encoded) != 7 or encoded[:3] != b"\x48\x8d\x05"
                or lea + 7 + struct.unpack_from("<i", encoded, 3)[0] != table
                or rows.get(lea + 7, {}).get("bytes") != "488906"):
            raise ValueError("Mapping descriptor storage observer this or constructor differs: " + role)
        constructors.append({"role": role, "owner_rva": hex(owner), "vtable_rva": hex(table),
            "this_offset": 0, "instructions": [rows[lea], rows[lea + 7]]})

    names_row = region_rows.get("report_class_names")
    if names_row is None or int(names_row["rva"], 16) != 0x2ec310 or len(regions.get("report_class_names", b"")) != 72:
        raise ValueError("Mapping descriptor report-name table identity differs")
    names = []
    for pointer, in struct.iter_unpack("<Q", regions["report_class_names"]):
        value, rva = bytearray(), pointer - base
        for offset in range(64):
            byte = actual(rva + offset, 1)[0]
            if not byte:
                break
            value.append(byte)
        else:
            raise ValueError("Mapping descriptor report name is unterminated")
        names.append({"table_index": len(names), "class": min(len(names), 8),
            "rva": hex(rva), "name": value.decode("ascii"),
            "bytes": len(value) + 1, "raw_hex": (value + b"\0").hex()})
    if tuple(row["name"] for row in names) != MAPPING_DESCRIPTOR_REPORT_NAMES:
        raise ValueError("Mapping descriptor report-name values differ")

    proofs = (
        (0xb0ed0, ((0xb0ed0, "488b01"), (0xb0ed3, "488b4008")), "added_observer_slot8"),
        (0xb0fb0, ((0xb0fb0, "488b01"), (0xb0fb3, "488b4010")), "changed_observer_slot16"),
        (0x6a470, ((0x6a470, "488b01"), (0x6a473, "488b4018")), "deleted_observer_slot24"),
        (0x8dc00, ((0x8dc00, "488b01"), (0x8dc03, "488b4020")), "clear_observer_slot32"),
        (0x65cc0, ((0x65d2d, "488b4018"), (0x65d54, "0f88f9010000"),
            (0x65d98, "488b4008")), "current_key_compare_and_set_data"),
        (0xe3c90, ((0xe3cc4, "488b4038"), (0xe3ce7, "e832020000"),
            (0xe3cee, "0f8403020000"), (0xe3cf4, "418a442441"), (0xe3cfc, "a801"),
            (0xe3d23, "498b542430"), (0xe3d28, "49c744243000000000"),
            (0xe3d31, "e86c1c0000")), "ready_parse_changed_flag_then_publish"),
        (0xe7080, ((0xe71b2, "80792004"), (0xe71d4, "8a44246c"),
            (0xe71d8, "8844243f"), (0xe7215, "44897814")), "report_id_string_and_uint8_key"),
        (0xe3bc8, ((0xe3c60, "4183ff08"), (0xe3c64, "be08000000"),
            (0xe3c69, "410f42f7")), "eight_special_classes_else_hardware8"),
        (0xe5b6c, ((0xe5b99, "be08000000"), (0xe5ba2, "488b4f38"),
            (0xe5bbd, "4883c120"), (0xe5bd6, "8b7014")), "classifier_reads_published_report_map"),
        (0xdb630, ((0xdb72c, "4c8d4622"), (0xdb74d, "4c8d4624")), "physical_pid_and_edition_id_fields"),
    )
    verified = []
    for owner, expected, role in proofs:
        rows = instructions(owner)
        if any(rows.get(rva, {}).get("bytes") != body for rva, body in expected):
            raise ValueError("Mapping descriptor reviewed instruction differs: " + role)
        verified.append({"role": role, "owner_rva": hex(owner),
            "instructions": [rows[rva] for rva, _ in expected]})
    return {"dispatch": dispatch, "constructors": constructors, "report_names": names,
        "default_report_class": 8, "report_id_key_bits": 8,
        "instruction_proofs": verified, "functions_verified": len(functions),
        "instructions_verified": instruction_count,
        "verification_scope": "All retained chunks, instructions and data equal the official PE. Reviewed machine-code branches and constructor writes bind the interpretation; no target execution or independent x86 decoder."}


def collect_mapping_descriptors_static(audit):
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    functions = (
        0x1000, 0x1060, 0x1e454, 0x20680, 0x20920, 0x20f00, 0x21460,
        0x3160, 0x33d0, 0x3650, 0xd42d8, 0xd46ec, 0xd4900, 0xd4b72,
        0xd4d60, 0xd4fb4, 0xd51b0, 0xd52d2, 0xd539c, 0xd545a, 0xd5518,
        0xd565c, 0xd570c, 0xd58f4, 0xd0030, 0xce9ec, 0xd6c60,
        0x5546c, 0x55484, 0x555b0, 0xd3ff2, 0xb07d8, 0xb1640, 0xb1b90,
        0xb1e3a, 0xb1fb0, 0xc21b0, 0xb0ed0, 0xb0fb0, 0x6a470, 0x8dc00,
        0x549e0, 0x54a50, 0x54a60, 0x5597d, 0x65cc0, 0x662b0, 0x66590,
        0xdb630, 0x77f10, 0x780b0, 0x78790, 0x78be0, 0xaa588,
        0xe3b10, 0xe3b6c, 0xe3c90, 0xe3f1e, 0xe3878, 0xe59a2,
        0xe7080, 0xe3bc8, 0xe5b6c, 0xe5a74, 0xe72fa, 0xe75a0,
        0xeb468, 0xeb32e, 0xeb3cb, 0xeb281, 0xdb930, 0xeb505, 0xebef2,
        0x1b4af0, 0x1b4940, 0x1b48a0, 0x1b49e0, 0x1b43d0, 0x1bfd50,
        0xa8de0, 0x53c70, 0x64a60, 0x549d0, 0x7f43e,
    )
    retained = audit.native_slice(original, ("localStorageGetItem", "localStorageSetItem",
        "localStorageDeleteItem", "localStorageDeleteAllItems"), depth=0,
        rvas=tuple(sorted(set(functions)
            | {value for _, _, values in MAPPING_DESCRIPTOR_TABLES for value in values}
            | {owner for _, owner, _, _, _ in MAPPING_DESCRIPTOR_OBSERVERS})),
        regions=tuple({"role": role, "rva": rva, "bytes": len(values) * 8}
            for role, rva, values in MAPPING_DESCRIPTOR_TABLES) + (
                {"role": "report_class_names", "rva": 0x2ec310, "bytes": 72},))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Mapping descriptor complete native owners remain unavailable")
    proof = mapping_descriptor_proof(audit.read(original), retained)
    sources = verified_host_owners(audit, ("electron/modules/mapping_engine/win/index.js",))
    source = sources["electron/modules/mapping_engine/win/index.js"]["original"]
    owners = {}
    for name in ("localStorageSetItem", "localStorageDeleteItem", "localStorageDeleteAllItems"):
        _, owners[name] = audit.callable_member("mapping_descriptor_host_" + name,
            source, audit.tree(source).root_node, name)
        body = audit.owners[owners[name]]["body"]
        if any(token not in body for token in ("if(!this.isInit)", 'e.Callback("void",["bool","string"]',
                "this.cbMap.set(", "this.cbMap.delete(", "this.libFFI[")):
            raise ValueError("Mapping descriptor host callback ownership differs: " + name)
    return {"schema_version": 1,
        "scope": "Current shared descriptor storage, identity, report classification and publication gates; complete product callers and all mapping executors are separate joins",
        "provenance": provenance, "native": retained, "verified_contract": proof,
        "host_sources": sources, "host_owners": owners, "complete_host_owners": audit.owners,
        "semantics": {
            "storage_set": "20920 rejects empty key/value and uses the engine primary slot32/2fa0.3160 -> d46ec queues d4900 on the storage-service thread. A missing key is inserted and notified; equal bytes do not notify; changed bytes replace the old value and queue both values. Completion reports storage success on the origin runner, not parse/publication or hardware acknowledgment.",
            "completion_lifetime": "20680 invokes the retained bool/string callback only while global394748 bit0 remains set. Host fields gate isInit, retain the FFI callback in cbMap until the Promise completes, and remove it afterward. If native shutdown suppresses the callback, this source does not supply a cancellation completion or successful fallback.",
            "observer_dispatch": "5546c -> d3ff2 -> b07d8 registers the unadjusted input object with a token. Added d539c/ce9ec/b0ed0 calls primary slot8; changed d52d2/d570c/b0fb0 calls slot16. All five concrete primary tables resolve both to65cc0. b1640 checks the current registration token under its owning lock before dispatching queued work. Hardware callback subobject+128 is a different table, even where6a470 is shared.",
            "physical_identity": "db630 parses vendorId at+32(u16), productId at+34(u16), editionId at+36(u32), serialNumber at+88 and containerId at+112.780b0 registers then reads synapse_PID_container when present, synapse_PID_edition_serial when present, synapse_PID_edition, then synapse_PID. PID is the native physical device identity, not a guessed displayed model.",
            "initial_read": "77f10 queues780b0 to the device runner.55484 -> d42d8 performs asyncGet;555b0 caches only a successful nonempty value at input+80. The final base-key callback78790 schedules initialization.78be0 creates DeviceData and walks the ordered cache until the first successful SetData. These reads do not initialize hardware from local defaults.",
            "update_selection": "65cc0 updates the cache first via549e0, obtains the published key, and compares incoming bytes using5597d. An incoming key smaller than the current key is skipped; a nonempty value otherwise reaches SetData. This is the actual lexical comparison and initial ordered traversal, not an invented specificity priority.",
            "candidate_parse": "e3f1e resets change bytes64/65 and creates a separate candidate at+48. version(int32) and dualLinked/bluetoothLE/turnOffAnalogToKey(bool) reads are optional: ignored getter failure leaves the candidate constructor values. reportIDs is processed only for an object; absent or nonobject is ignored by eb468/1b4af0. defaultMappings object, activeProfile string and profiles array are required; analogData/analogStick and mapping parsers have their own branches. These internal parser defaults are not observed device values.",
            "report_ids": "e7080 parses reportIDs property names through signed-int conversion1bfd50, then stores only the low uint8 key. Each present value must be JSON string type4. The nine name table has special classes0..7; hardwareEventReportID and unknown strings both classify8. A missing reportID also classifies8. Classes0..2 additionally update the candidate special selector at+144; no capability is inferred from count or integer value.",
            "publication": "e3c90 requires the engine ready gate and successful full FormatData. It returns true on successful parse even if change byte65 bit0 is clear. Only a set bit locks+72, updates key+24 and moves candidate+48 into published+56.65cc0 refreshes viaaa588 only on that same change bit. reportIDs alone does not set it; default/profile hashes, active selection and analog changes must be followed through their actual owners. Parse failure preserves the old published object.",
            "classification": "e5b6c reads the published+56 report map at+32 and defaults to class8. USB a8de0 obtains the reportID from raw[bluetoothLE]; BLE53c70 and the other concrete input branches remain separately retained. Class8 reaches the existing hardware event path7f43e. A cached candidate or storage success cannot change the live classifier by itself.",
            "deletion": "d545a/ce9ec binds6a470 on the primary object, so slot24 resolves to662b0. It removes the cached key, clears live DeviceData only when deleting the current key, then tries remaining cache entries in order until SetData succeeds. d5518/d58f4 binds8dc00, primary slot32/66590, which clears cache and DeviceData then refreshes. DeviceData slot16/e5930 clears its key and published data; destructors and reference releases are retained without claiming all engine shutdown paths closed."},
        "remaining_closure": [
            "Join every actual product publisher and merged descriptor to physical identity, hash gates, active mapping and transport consumers",
            "Finish all default/profile mapping parsers and executors, notification unregister and full engine stop/cancel/drain lifecycles",
            "Independent Rust implementation, production/UI integration and official actual-function/device acceptance"],
        "source_chain_complete": False, "target_execution": False, "actual_dll_comparison": "unverified",
        "independent_execution_connected": False, "runtime_acceptance": "unverified"}


def freeze_host_static_review(report, a):
    """Retain one deterministic, content-addressed complete static review."""
    import os
    frozen = (json.dumps(report, ensure_ascii=False, indent=2) + "\n").replace("\n", os.linesep).encode("utf-8")
    digest = hashlib.sha256(frozen).hexdigest()
    path = a.ROOT / ".analysis/results/reverse-pending/review-sources" / (digest + ".json")
    if path.exists():
        if path.read_bytes() != frozen:
            raise ValueError("Host frozen review collides with different retained bytes")
    else:
        a.save(path, report)
        if path.read_bytes() != frozen:
            raise ValueError("Host frozen review serialization differs")
    return {"path": a.rel(path), "bytes": len(frozen), "sha256": digest}


MAPPING_DESCRIPTOR_LIFECYCLE_FUNCTIONS = (0xa8c2c, 0x52394, 0x633d8, 0xa6db8, 0x656ca, 0x547e0,
    0xd3c70, 0xd3d1e, 0xb0ab0, 0xb1ebc, 0xb1f26, 0xd3ef2,
    0x1b8670, 0x1b88e0, 0x1a9710, 0x1b8a20, 0x200430, 0x65c90, 0x77f00)


def mapping_descriptor_lifecycle_proof(raw, retained):
    base, functions, regions, instruction_count, _ = verify_retained_native_pe(raw, retained, "Mapping observer lifecycle")
    if set(MAPPING_DESCRIPTOR_LIFECYCLE_FUNCTIONS) != set(functions):
        raise ValueError("Mapping observer lifecycle complete owners differ")
    table = regions["input_base_lifetime_vtable"]
    if len(table) != 80 or struct.unpack_from("<Q", table, 72)[0] - base != 0x77f00:
        raise ValueError("Mapping observer lifecycle destructor slot72 differs")
    proof = []
    for owner, expected in (
        (0x65c90, ((0x65c90, "488b01"), (0x65c93, "488b4048"))),
        (0x547e0, ((0x54894, "e9d7f30700"),)),
        (0xd3c70, ((0xd3ccb, "e84e000000"),)),
        (0xb0ab0, ((0xb0af0, "e8c7130000"),)),
        (0x1b8670, ((0x1b86a3, "e86810ffff"),)),
        (0xd3ef2, ((0xd3f2a, "48c7054b082c0000000000"), (0xd3f39, "e832470e00"))),
    ):
        rows = {int(row["rva"], 16): row for row in functions[owner]["assembly"]}
        if any(rows.get(rva, {}).get("bytes") != encoded for rva, encoded in expected):
            raise ValueError("Mapping observer lifecycle reviewed instruction differs: " + hex(owner))
        proof.append({"owner_rva": hex(owner), "instructions": [rows[rva] for rva, _ in expected]})
    return {"functions_verified": len(functions), "instructions_verified": instruction_count,
        "input_base_slot72_target_rva": "0x77f00", "instruction_proofs": proof}


def collect_mapping_descriptor_lifecycle(audit, source_review):
    """Join observer destruction and its thread stop to the existing descriptor review."""
    baseline = json.loads(audit.artifact(audit.a.ROOT / source_review["path"], source_review))
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    if baseline["provenance"]["original"] != original:
        raise ValueError("Mapping observer lifecycle belongs to another descriptor input")
    retained = audit.native_slice(original, (), depth=0, rvas=MAPPING_DESCRIPTOR_LIFECYCLE_FUNCTIONS,
        regions=({"role": "input_base_lifetime_vtable", "rva": 0x2e5a18, "bytes": 80},))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Mapping observer lifecycle complete owners remain unavailable")
    proof = mapping_descriptor_lifecycle_proof(audit.read(original), retained)
    return {"schema_version": 1, "scope": "Shared descriptor observer destruction and owned-thread stop boundaries",
        "descriptor_review": source_review, "provenance": provenance, "native": retained,
        "verified_contract": proof,
        "semantics": {
            "concrete_destructors": "USB a8c2c, BLE52394, external633d8 and OS-hooka6db8 all reach common656ca after restoring their own tables; OS-hook separately releases its owned pointer+1936.656ca then installs base input tables before posting65c90 to its owned device runner.65c90 reads primary slot72, now77f00, a no-op thunk; this queued call is not a derived restore or hardware confirmation.",
            "device_runner": "656ca calls1b8670 on the owned thread before freeing mapping/callback/container fields.1b8670 holds the thread lock, calls StopSoon1b88e0, joins with1a9710 when a handle exists, and resets its handle/stopping byte.1b8a20 reaches RunLoop::QuitWhenIdle200430; its actual thread guard or re-post is retained. Join waits INFINITE and closes the handle; unexpected wait results go through the native assertion path. No per-request hardware ACK is inferred.",
            "unregister": "After owned fields and references are released,547e0 ends with d3c70 on the same primary object. d3c70 installs Observer's base table, leases global394780 if present, then d3d1e snapshots all key-specific observer lists under the service lock. For each list b0ab0 removes this identity via b1ebc under that list's lock. Baseline b1640 rejects a later queued callback if the registration is absent or its token changed; unregister itself does not expose a separate drain acknowledgment.",
            "service_destroy": "d3ef2 checks its caller runner affinity, clears global394780, stops/joins its storage thread, then destroys observer lists and stored strings and releases caller/thread owners. An observer destroyed after the global is cleared skips service removal. Service teardown, connection stop and all external producers remain separate owners."},
        "remaining_closure": ["Verify every engine/device shutdown initiator, all external producers and per-request cancellation/restore consumers",
            "Independent Rust ownership, integration and official actual-function/device comparisons"],
        "source_chain_complete": False, "target_execution": False, "actual_dll_comparison": "unverified",
        "cleanup": {"temporary_files_retained": [], "private_databases_created": []}}


MAPPING_DESCRIPTOR_PARSER_FUNCTIONS = (0xe5bf6, 0xe2f94, 0xe8518, 0xe871e, 0xe8888,
    0x1b3a30, 0x1f3600, 0x1f3710, 0x251cc0, 0x1c0030, 0x1bcb40, 0x1bcb80,
    0x1c9d80, 0x1c9dd0, 0x109b12, 0x109c08, 0xfb6ac, 0xfa828)


def mapping_descriptor_parser_proof(raw, baseline, retained):
    _, old, _, _, _ = verify_retained_native_pe(raw, baseline, "Mapping descriptor parser baseline")
    _, new, regions, instruction_count, _ = verify_retained_native_pe(raw, retained, "Mapping descriptor parser")
    required = (0xe3f1e, 0xe72fa, 0xe75a0, 0xeb468, 0xeb32e, 0xeb3cb, 0xeb281)
    if any(rva not in old for rva in required) or set(new) != set(MAPPING_DESCRIPTOR_PARSER_FUNCTIONS):
        raise ValueError("Mapping descriptor parser complete owners differ")
    if regions.get("mapping_descriptor_md5_initial_state") != bytes.fromhex("0123456789abcdeffedcba9876543210"):
        raise ValueError("Mapping descriptor parser MD5 initial state differs")
    functions = {**old, **new}
    proofs = []
    for owner, expected, role in (
        (0xe72fa, ((0xe7342, "e821410000"), (0xe7366, "e8fd400000"),
            (0xe7477, "e89c100000"), (0xe7515, "864641"),
            (0xe7567, "e88c59f7ff"), (0xe759b, "e97cffffff")), "default_hash_reuses_old_mapping_without_change_bit"),
        (0xe75a0, ((0xe79be, "e8a53a0000"), (0xe79c5, "0f84d8050000"),
            (0xe79f0, "41c60000"), (0xe7a03, "e879380000"),
            (0xe7e86, "888198000000"), (0xe7eb9, "e83a50f7ff"),
            (0xe7ebe, "31ed"), (0xe7ef5, "0f84f9040000"),
            (0xe7f08, "e80b060000"), (0xe7f9f, "41864641")), "same_hash_mutates_shared_gamemode_and_skips_reparse"),
        (0xe8518, ((0xe8561, "e83ac00c00"), (0xe8587, "e824cb0c00"),
            (0xe85ad, "e8feca0c00"), (0xe85e0, "e89b170e00"),
            (0xe861d, "e80eb40c00")), "hash_removes_hash_and_gamemode_before_serialization"),
    ):
        rows = {int(row["rva"], 16): row for row in functions[owner]["assembly"]}
        if any(rows.get(rva, {}).get("bytes") != body for rva, body in expected):
            raise ValueError("Mapping descriptor parser reviewed instruction differs: " + role)
        proofs.append({"role": role, "owner_rva": hex(owner),
            "instructions": [rows[rva] for rva, _ in expected]})
    return {"additional_functions_verified": len(new), "additional_instructions_verified": instruction_count,
        "baseline_owner_rvas": [hex(rva) for rva in required], "instruction_proofs": proofs,
        "same_hash_profile_shared_mutation": True, "same_hash_rechecks_mapping_payload": False,
        "hash_format": "lowercase hexadecimal MD5 of the native serialized object without hash/gamemode",
        "serialization_boundary": "Exact JSON writer ordering, number and string formatting remain separately auditable; no generic JSON serializer equivalence is inferred"}


def collect_mapping_descriptor_parser(audit, source_review):
    baseline = json.loads(audit.artifact(audit.a.ROOT / source_review["path"], source_review))
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    if baseline["provenance"]["original"] != original:
        raise ValueError("Mapping descriptor parser belongs to another descriptor input")
    retained = audit.native_slice(original, (), depth=0, rvas=MAPPING_DESCRIPTOR_PARSER_FUNCTIONS,
        regions=({"role": "mapping_descriptor_md5_initial_state", "rva": 0x2f3740, "bytes": 16},))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Mapping descriptor parser complete owners remain unavailable")
    proof = mapping_descriptor_parser_proof(audit.read(original), baseline["native"], retained)
    return {"schema_version": 1, "scope": "Default/profile hash reuse, shared gamemode mutation and hash-validation dispatch",
        "descriptor_review": source_review, "provenance": provenance, "native": retained,
        "verified_contract": proof,
        "baseline_owners": [row["artifact"] for row in baseline["native"]["complete_functions"]
            if int(row["capture"]["rva"], 16) in {int(rva, 16) for rva in proof["baseline_owner_rvas"]}],
        "semantics": {
            "default_mapping": "e72fa requires defaultMappings object. Its appEngine is optional: absent or wrong-type skips that subparser successfully. A present object requires string hash. Under the DeviceData lock e5bf6 leases the published default map; identical hash strings reuse that reference without validating the incoming payload or setting change byte65. A changed/first hash must pass e8518 then e871e before change byte65 is set. A fresh candidate default set is not an observed device configuration.",
            "profile_selection": "e75a0 requires profiles array and allocates/reset a Profile. It validates each visited element as object with string guid, skips unequal guids, and stops at the first activeProfile match. It does not validate every later inactive profile. No match returns parser success without publishing an active profile. A matching profile without an appEngine object accepts the new empty Profile; a present appEngine requires string hash and treats a missing/wrong-type gamemode as false.",
            "shared_profile": "The candidate and published descriptor hold reference-counted Profile objects. On an equal active profile appEngine hash, e7e86 writes the parsed gamemode directly to the existing published Profile+152, then e7eb9 shares it with candidate+24. e7ebe clears the reparse flag and e7ef5 bypasses hash validation and mapping parsing, without setting descriptor change byte65. This shared-object mutation occurs before later descriptor validation; a retained published pointer on parse failure does not imply immutable contents.",
            "changed_profile": "On a different/first hash, e8518 validates first, then the new Profile receives gamemode/hash and e871e parses optional profile-only actuations/rapidTriggers/snaptaps plus required mappings. Only successful parsing sets descriptor change byte65 at e7f9f. These paths do not issue a device command or provide a hardware acknowledgment.",
            "hash_validator": "e8518 copies the appEngine object, removes hash and gamemode, serializes through1c9d80/1c9dd0, and computes1b3a30. That helper uses1f3600/1f3710/251cc0 with the retained standard MD5 initial state and complete 64-round body;1c0030 hex-encodes16 digest bytes and1bcb40/1bcb80 lowercases ASCII. The supplied hash is compared byte-for-byte; differing case is not normalized on the supplied string. Equal stored hash bypasses this validator entirely."},
        "remaining_closure": ["Close exact native JSON writer ordering/escaping/number behavior and every input/output mapping parser/executor branch",
            "Join shared gamemode consumers, complete descriptor failure/cancellation lifetimes and all applicable product publishers",
            "Independent Rust implementation, UI/backend integration and actual official-function/device comparisons"],
        "source_chain_complete": False, "target_execution": False, "actual_dll_comparison": "unverified",
        "cleanup": {"temporary_files_retained": [], "private_databases_created": []}}


def collect_mapping_descriptor_gamemode(audit, source_review, parser_review):
    baseline = json.loads(audit.artifact(audit.a.ROOT / source_review["path"], source_review))
    audit.artifact(audit.a.ROOT / parser_review["path"], parser_review)
    original = baseline["provenance"]["original"]
    retained = audit.native_slice(original, (), depth=0, rvas=(0xe5a36, 0x8eb70, 0xc438a, 0xc45b0, 0xc0e76))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Mapping gamemode consumer complete owners remain unavailable")
    _, functions, _, instructions, _ = verify_retained_native_pe(audit.read(original), retained, "Mapping gamemode consumer")
    if set(functions) != {0xe5a36, 0x8eb70, 0xc438a, 0xc45b0, 0xc0e76}:
        raise ValueError("Mapping gamemode consumer complete owners differ")
    _, old, _, _, _ = verify_retained_native_pe(audit.read(original), baseline["native"], "Mapping gamemode baseline")
    proofs = []
    for owner, rows, expected in ((0xe3f1e, old, ((0xe4c77, "8a8098000000"), (0xe4c80, "88818c000000"))),
            (0xe5a36, functions, ((0xe5a4b, "488b4738"), (0xe5a54, "80b88c00000000"))),
            (0x8eb70, functions, ((0x8eba1, "e8906e0500"), (0x8ebcb, "868720030000"), (0x8ec28, "e85d570300")))):
        assembly = {int(row["rva"], 16): row for row in rows[owner]["assembly"]}
        if any(assembly.get(rva, {}).get("bytes") != body for rva, body in expected):
            raise ValueError("Mapping gamemode consumer instruction differs: " + hex(owner))
        proofs.append({"owner_rva": hex(owner), "instructions": [assembly[rva] for rva, _ in expected]})
    return {"schema_version": 1, "descriptor_review": source_review, "parser_review": parser_review,
        "scope": "Shared Profile gamemode versus published descriptor cache and its device-mode caller",
        "native": retained, "verified_contract": {"additional_functions_verified": len(functions),
            "additional_instructions_verified": instructions, "instruction_proofs": proofs},
        "semantics": "e3f1e copies Profile+152 into candidate descriptor+140 at e4c77/e4c80. The effective getter e5a36 locks DeviceData and reads published+56 -> byte140, not the shared Profile byte. Thus an equal-hash Profile mutation alone does not update the effective published gamemode cache; a later accepted candidate can copy it.8eb70 compares that getter with its own observed byte800, marks byte801 and atomically updates800 before queuing c438a NotifyGamemodeStatusOnAnyThread. Its c45b0 task calls c0e76 on the device-mode service and separately posts completion to the origin runner. c0e76 looks up the actual identity; a miss logs without creating a device. A hit writes byte48 of the registered mode record and invokes c13d8 to notify observers, including the equal-state case (its check only changes logging). That service notification is not a hardware write/readback, and its complete downstream event/observer consumers remain separate.",
        "remaining_closure": ["Close all device-mode service event recipients, descriptor refresh callers and applicable product consumers"],
        "source_chain_complete": False, "target_execution": False, "actual_dll_comparison": "unverified",
        "cleanup": {"temporary_files_retained": [], "private_databases_created": []}}


def audit_host_mapping_descriptors(args):
    audit = HostLocalAudit()
    report = collect_mapping_descriptors_static(audit)
    report.update(complete_originals=list(audit.sources.values()), verified_artifacts=list(audit.artifacts.values()),
        cleanup={"temporary_files_retained": [], "private_databases_created": [],
            "capture_policy": "Reused complete byte-verified native and host captures; no recapture or temporary copies"})
    changed = audit.final_snapshot()
    if changed:
        raise ValueError("Mapping descriptor inputs changed during query: " + ", ".join(changed))
    source_review = freeze_host_static_review(report, audit.a)
    lifecycle = collect_mapping_descriptor_lifecycle(audit, source_review)
    lifecycle["verified_artifacts"] = list(audit.artifacts.values())
    changed = audit.final_snapshot()
    if changed:
        raise ValueError("Mapping observer inputs changed during query: " + ", ".join(changed))
    lifecycle_review = freeze_host_static_review(lifecycle, audit.a)
    parser = collect_mapping_descriptor_parser(audit, source_review)
    parser["verified_artifacts"] = list(audit.artifacts.values())
    changed = audit.final_snapshot()
    if changed:
        raise ValueError("Mapping parser inputs changed during query: " + ", ".join(changed))
    parser_review = freeze_host_static_review(parser, audit.a)
    gamemode = collect_mapping_descriptor_gamemode(audit, source_review, parser_review)
    gamemode["verified_artifacts"] = list(audit.artifacts.values())
    changed = audit.final_snapshot()
    if changed:
        raise ValueError("Mapping gamemode inputs changed during query: " + ", ".join(changed))
    gamemode_review = freeze_host_static_review(gamemode, audit.a)
    report.update(recorded_at_utc=audit.a.stamp(), tool=audit.families.identity(Path(__file__)),
        source_review=source_review, observer_lifecycle={key: value for key, value in lifecycle.items()
            if key not in {"native", "verified_artifacts"}})
    report["observer_lifecycle"]["source_review"] = lifecycle_review
    report["parser_reuse"] = {key: value for key, value in parser.items()
        if key not in {"native", "verified_artifacts"}}
    report["parser_reuse"]["source_review"] = parser_review
    report["gamemode_consumer"] = {key: value for key, value in gamemode.items()
        if key not in {"native", "verified_artifacts"}}
    report["gamemode_consumer"]["source_review"] = gamemode_review
    published = {**report, "native": {"complete_evidence": report["source_review"], "pointer": "/native",
        "original": report["provenance"]["original"], "image_base": report["native"]["image_base"],
        "functions_verified": report["verified_contract"]["functions_verified"],
        "instructions_verified": report["verified_contract"]["instructions_verified"],
        "full_function_records_in_review": True, "target_execution": False}}
    publish_host_static_query(published, audit.a.ROOT / ".analysis/results/host-current/local-audit.json",
        audit.a, "mapping_descriptors")
    print(json.dumps({"query": args.query, "functions": report["verified_contract"]["functions_verified"],
        "instructions": report["verified_contract"]["instructions_verified"], "source_review": report["source_review"], "target_execution": False,
        "actual_dll_comparison": "unverified"}), flush=True)


def retain_mapping_global_mode(audit):
    """Retain the live mode service, its query, producers and owned cleanup."""
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    retained = audit.native_slice(original, ("getGlobalMode",), depth=0,
        rvas=(0x1e454, 0x1000, 0x1060, 0x13340, 0x13530, 0x4b740,
              0xc352a, 0xc3710, 0xc50c0, 0xc0124, 0xbfe62, 0xbec9a,
              0xbeb94, 0xbec0a, 0xbf868, 0xbf8bc, 0xbf92e, 0xbf9dc,
              0xc23f4, 0xc249c, 0xc263a, 0xc27c0, 0xc29f0, 0xc4e80,
              0xc033e, 0xc05e2, 0xc07ce, 0xc0c22, 0xc0e76, 0xc1602,
              0xc1c10, 0xc1a0a, 0xc1d90, 0xc234e,
              0xbf2c6, 0xbf3f4, 0xbf4c8, 0xbf575, 0xbf602, 0xbf642,
              0xbf676, 0xbf716, 0xbf784, 0xbf82c, 0xbf33a, 0xbf37e,
              0x5597d, 0x55a1a, 0x55a5e,
              0xc3e44, 0xc4030, 0xc40ea, 0xc42d0, 0xc4a58, 0xc4cd0,
              0x5e230, 0x5e310, 0x5e3e0, 0x5e460, 0x8ab6a,
              0x1bf9f0, 0x1b44f0, 0x1b4db0, 0x1b4f10, 0x1b5020),
        regions=({"role": "mapping_engine_global_mode_dispatch", "rva": 0x2df010, "bytes": 832},))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Current mapping global-mode native owners are unavailable")
    table, = retained["verified_data_regions"]
    pointer, = struct.unpack_from("<Q", bytes.fromhex(table["raw_hex"]), 824)
    if pointer - int(retained["image_base"], 16) != 0x13340:
        raise ValueError("Current getGlobalMode virtual dispatch differs")
    return {**provenance, "native": retained,
        "verified_dispatch": {"vtable_rva": "0x2df010", "offset": 824,
            "target_rva": "0x13340", "actual_file_pointer": hex(pointer)},
        "query_chain": [
            "getGlobalMode -> 1e454/1000 module-static MappingEngine singleton -> vtable+824 13340 -> engine-thread 13530",
            "13530 requires its DeviceModeServiceThread at instance+200 -> c352a queues c3710 -> c0124 reads that service instance",
            "Missing engine thread, mode thread or mode service returns failure; no empty/default success is synthesized",
            "c0124 emits hypershift:true only for service byte+88 and otfs:true only for byte+89; absent false keys are source output behavior",
            "bfe62 iterates the owned global action map, emitting globalmode only when nonempty; bec9a writes guid/scope/input/output strings and timeTick as a decimal string",
            "c50c0 returns through the retained owner thread and export callback adapter 4b740"],
        "ownership_and_producers": [
            "c263a -> c27c0 allocates a 96-byte DeviceModeService; bf868 initializes the two maps and sets both mode bytes false",
            "5e230/5e310 own mapping-engine hypershift activation/deactivation; c3e44/c4030 publish through c07ce on the mode thread",
            "5e3e0/5e460 own OTFS activation/deactivation; c40ea/c42d0 publish through c0c22 on the mode thread",
            "Repeated same-value boolean notifications log a warning but still update and notify observers",
            "8ab6a HandleModeOnDeviceThread accepts source scope<3 and flag<2, obtains a pooled action, copies device GUID, scope and polymorphic input/output strings and uses the actual tick or source clock when zero",
            "8ab6a sends scope0 through c4a58/c4cd0 to global c1c10; nonzero scope goes through device-mode notification instead",
            "c1c10 keys global actions by device GUID, input identity and the current hypershift byte; enabling inserts without replacing an existing equal key, disabling removes the matching key",
            "bf575 compares the two string byte sequences and then the hypershift byte; globalmode iteration preserves this ordered-map traversal",
            "c29f0 clears its service before shutdown completion; c4e80/bf8bc release global actions, device entries, observers and the action pool; c249c stops the owning thread and clears its singleton"],
        "replacement_boundary": "GlobalMode belongs to the live mapping execution service. OS properties, persisted macros and an unpopulated Rust map cannot replace the source query without wiring actual event producers and cleanup.",
        "limitations": [
            "This bounded slice does not close all polymorphic input/output formatters or every product mapping action producer",
            "Several Hex-Rays helper signatures infer truncated pointer parameters; complete captured assembly, not those inferred C prototypes, governs any future port",
            "No independent Rust mapping-execution owner is connected by this evidence update; the production DLL dependency is not removed",
            "No native export, installed service or real device was executed"],
        "source_chain_complete": False, "target_execution": False, "runtime_acceptance": "unverified"}


def retain_mapping_suppression(audit):
    """Retain distinct mapping and hotkey suppression owners and their consumers."""
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    tables = (
        ("engine_control_dispatch", 0x2df010, 896,
         ((208, 0x68f0), (216, 0x6b90), (224, 0x6900),
          (880, 0x14fb0), (888, 0x15280))),
        ("device_manager_controls", 0x2e4a88, 32,
         ((16, 0x57f70), (24, 0x580b0))),
        ("input_device_mode_notifications", 0x2e5af8, 24,
         ((8, 0x6a490), (16, 0x6ab90))),
        ("ble_device_mode_notifications", 0x2e4008, 24,
         ((8, 0x6a490), (16, 0x6ab90))),
        ("external_device_mode_notifications", 0x2e5698, 24,
         ((8, 0x6a490), (16, 0x6ab90))),
        ("os_hook_device_mode_notifications", 0x2e7988, 24,
         ((8, 0x6a490), (16, 0x6ab90))),
        ("usb_device_mode_notifications", 0x2e7e68, 24,
         ((8, 0x6a490), (16, 0x6ab90))),
        ("hotkey_service_dispatch", 0x2e97c0, 48,
         ((8, 0xc6ff0), (16, 0xc9790), (24, 0xc97b0),
          (32, 0xc97c0), (40, 0xc9aa0))),
    )
    retained = audit.native_slice(original, (
        "isMappingEnabled", "enableMapping", "disableMapping",
        "enableGlobalShortcut", "disableGlobalShortcut",
        "mappingEngineInitialize", "mappingEngineShutdown"), depth=0,
        rvas=(0x1000, 0x1e454, 0x68f0, 0x6900, 0x6ac0, 0x6b90, 0x6d50,
              0x14fb0, 0x15170, 0x15280, 0x15440, 0x1ae04,
              0x19180, 0x19590, 0x1a9d0, 0x1ad48, 0x1fb30, 0x1fdd0,
              0x28740, 0x28a50, 0x4d3e0, 0x4d6f0,
              0x55de4, 0x56128, 0x57f70, 0x5804c, 0x580b0, 0x58192, 0x57090,
              0x52158, 0x6319c, 0x64e8c, 0xa6b74, 0xa89f0,
              0x6a490, 0x6ab90, 0x6a5e0, 0x66870, 0x66d10, 0x909da,
              0xc6690, 0xc671c, 0xc6782, 0xc6fb8, 0xc6ff0, 0xc741a,
              0xc7b80, 0xb0ed0, 0xb0fb0,
              0xc7bac, 0xc7c4c, 0xc7e00, 0xc7f90, 0xc8cd2,
              0xc91f0, 0xc9246, 0xc9282, 0xc92e0, 0xc9338, 0xc9364,
              0xc94c0, 0xc9688, 0xc9790, 0xc97b0, 0xc97c0, 0xc9aa0),
        regions=tuple({"role": role, "rva": rva, "bytes": length}
                      for role, rva, length, _ in tables))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Current mapping suppression owner capture is unavailable")
    base = int(retained["image_base"], 16)
    dispatch = []
    for region, (role, _, _, slots) in zip(retained["verified_data_regions"], tables, strict=True):
        raw = bytes.fromhex(region["raw_hex"])
        for offset, expected in slots:
            pointer, = struct.unpack_from("<Q", raw, offset)
            if pointer - base != expected:
                raise ValueError("Current mapping suppression dispatch differs: " + role)
            dispatch.append({"table": role, "offset": offset,
                             "target_rva": hex(expected), "actual_file_pointer": hex(pointer)})
    return {**provenance, "native": retained, "verified_dispatch": dispatch,
        "mapping": {
            "owner": "One process-global atomic flag at RVA3811d8, queried by286a0 -> engine+208 ->68f0; not a per-request suspension counter",
            "disable": "28c20 -> engine+216 ->6b90 ->6d50 -> device-manager+24 ->580b0 changes true to false, visits input-device notification slots and posts57090 to cancel active/exclusive/pending execution. It invokes completion after posting, without waiting for every device notification or macro cleanup to finish",
            "enable": "28910 -> engine+224 ->6900 ->6ac0 -> device-manager+16 ->57f70 changes false to true and posts each input-device enabled notification before successful completion",
            "repeat": "Already disabled/enabled completes false with exact reason already disabled/already enabled. Exports have no owner identity or matching-release token, and a successful enable can release a prior disable from another caller",
            "consumers": "InputDevice, BLE, USB, external and OS-hook constructors install mode-notification tables whose +8/+16 slots share6a490/6ab90; both queue6a5e0 on the device thread.66870/66d10/909da read the same atomic flag during endpoint/action selection. Queued notifications and physical/OS effects are separate from the export bool"},
        "hotkeys": {
            "owner": "Engine+232 holds at most one lease on HotkeyService.15440 disables by borrowing via c8cd2/c6fb8; duplicate disable returns true with Warning: Already disabled without another lease.15170 enables by clearing/releasing this lease through1ae04/c9282; absent lease returns false with hotkey was not disabled",
            "count": "HotkeyService constructor c6690/c91f0 starts lease count+24 at0 and acquires its base hold to1. A borrow crossing1->2 invokes SuspendHotkeyHandling(true); the release crossing2->1 invokes it(false). Additional internal borrowers retain suspension. The exported disable itself is not a balanced per-caller counter",
            "actual_backend": "c7f90/c9338/c9364 constructs the RegisterHotKey/WM_HOTKEY service with vtable2e97c0. c6ff0 removes each real registration through+40 while suspended, and on release attempts each +32 registration then calls+16. c9790 leaves an already enabled flag unchanged; when disabled it enables only if the actual registration tree+64 has nonzero size+80. c97c0 inserts only after RegisterHotKey succeeds, so an initially disabled service stays disabled with an empty set or all failed re-registrations. Suspension itself unregisters entries without unconditionally clearing that flag. c9aa0 removes registrations through UnregisterHotKey. This export path does not disable the separate GlobalShortcutWin low-level hooks",
            "confirmation": "15170/15440 return their lease-operation success without awaiting the posted service-thread work or collecting all OS registration results. c6ff0 logs individual registration failures and continues. c94c0 accepts WM_HOTKEY786 only while service enabled. c741a publishes observer notifications separately"},
        "callback": "Exports are void APIs with completion callback(bool,const char* reason). 28740/28a50/4d3e0/4d6f0 forward only when callback is nonnull and DLL callback gate394748 is set.1fb30 sets the gate before initialize callback;1fdd0 clears it after shutdown callback. Missing completion is not operation cancellation",
        "independent_implementation": [audit.families.identity(audit.a.ROOT / name) for name in (
            "crates/razer-ipc/src/mapping_control.rs",
            "crates/razer-service/src/runtime/windows/mapping_runtime/backend.rs",
            "crates/razer-service/src/runtime/windows/mapping_runtime/backend/tests.rs",
            "crates/razer-service/src/runtime/windows/mapping_runtime/device.rs",
            "crates/razer-service/src/runtime/windows/mapping_runtime/capture/recorder.rs")],
        "comparison_boundary": "Current Rust scope sets coordinate its own actors; do not equate their aggregate ActionReply with the raw official DLL result. Preserve repeat-call result data, service-thread notifications, macro cancellation completion and OS registration outcomes separately",
        "source_chain_complete": False, "target_execution": False,
        "runtime_acceptance": "unverified"}


def retain_mapping_macro_input(audit):
    """Current recorder hook bodies, dispatch tables, clocks and joined teardown."""
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    dispatch = (
        ("recorder_service_vtable", 0x2ea570,
         (0xd9680, 0xd7040, 0xd7170, 0xd72b0, 0xd7020, 0xd7030, 0xd94e0, 0xd9500, 0xd95c0)),
        ("window_hook_destructor", 0x2ea5b8, (0xdaba0,)),
        ("recorder_wall_clock", 0x3872a0, (0x1a8750,)),
        ("recorder_interpolation_clock_initializer", 0x387250, (0x1a8c00,)),
    )
    retained = audit.native_slice(original, (
        "hasMacroRecordingStarted", "startMacroRecording", "stopMacroRecording",
        "getMacroRecordingMode", "registerMacroRecorderEvent", "unregisterMacroRecorderEvent",
        "setMacroRecorderEventCallback"), depth=0,
        rvas=(0x112e0, 0x114a0, 0x11560, 0x11710, 0x117d0, 0x11990, 0x11a50, 0x11c40,
              0xd8000, 0xd81c0, 0xd833c, 0xd84e0, 0xd8654, 0xd8810, 0xd897a, 0xd8b60,
              0xd77cc, 0xd7af8, 0xd7c80, 0xd7ea0, 0xd8d90, 0xd93f4,
              0xd943e, 0xd947c, 0xd9680, 0xd96a8, 0xd97e8, 0xdaba0,
              0xd6e5c, 0xd6f8a, 0xd7020, 0xd7030, 0xd703a,
              0xd7040, 0xd70e0, 0xd7170, 0xd7210, 0xd72b0, 0xd7350, 0xd73ca,
              0xd94e0, 0xd9500, 0xd95c0, 0xd9baa, 0xd9d60, 0xd99e8, 0xda110,
              0xda3e0, 0xda640, 0xda9c0, 0xdab00, 0x498b0, 0x49aa0, 0x49ca0,
              0x119b14, 0x1b8670, 0x1b88e0, 0x1a9710, 0x121890,
              0x1b7f30, 0x1b7f50, 0x1a8750, 0x1a8c00, 0x1a8c30, 0x1a8d30,
              0x1a9170, 0x1f1000, 0x1f0d40, 0x1b4e20),
        regions=tuple({"role": name, "rva": rva, "bytes": 8 * len(targets)}
                      for name, rva, targets in dispatch))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Current macro input owner capture is unavailable")
    base = int(retained["image_base"], 16)
    for region, (name, _, expected) in zip(retained["verified_data_regions"], dispatch, strict=True):
        actual = tuple(pointer - base for pointer in struct.unpack(
            "<" + "Q" * len(expected), bytes.fromhex(region["raw_hex"])))
        if actual != expected:
            raise ValueError("Current macro input dispatch differs: " + name)
    return {**provenance, "native": retained,
        "actual_input": "d9d60 installs WH_KEYBOARD_LL13/WH_MOUSE_LL14 on the owned Windows message thread; debugger-present source branch installs keyboard only",
        "lifecycle": [
            "Start rejects duplicate mode, resets the shared previous timestamp, retains supplied mode only after hook installation succeeds, then publishes started kind0",
            "Partial hook failure captures actual GetLastError, formats source errorCode/errorString and releases acquired hooks before rejected completion",
            "hasMacroRecordingStarted returns the actual active flag; false is a valid state. getMacroRecordingMode fails while inactive rather than returning a default mode",
            "da110 unhooks both inputs, flushes the remaining array, publishes stopped kind1 and clears active mode before completion",
            "d97e8 clears the callback singleton, posts disable and a queue barrier, then 1b8670/1b88e0/1a9710 stop and join the hook thread before releasing weak context and buffers"],
        "input_semantics": [
            "da3e0 accepts ordinary/system key down and up; unknown messages remain signed32 numeric action values, and injected/repeated input is not filtered",
            "da640 names move, left/right/middle down/up, wheel and horizontal wheel only; XBUTTON messages remain numeric. Coordinates, mouseData, key fields and flags are signed32 JSON integers",
            "Both hooks use the same previous wall microsecond timestamp; first delta is0, subsequent deltas retain source signed32 truncation",
            "da9c0 flushes when item_count exceeds threshold; mousemove threshold1000 gives1001 items, all other input uses0 and flushes preceding buffered moves",
            "Normal item events retain FILETIME-epoch wall microseconds from1a8750, interpolated through the same monotonic clock and resampled after60 million microseconds",
            "Start/stop and the final stop-flush batch retain monotonic1b7f50 ticks instead. Arrival Unix milliseconds are separate Rust delivery metadata",
            "49ca0 assembly passes kind2 in ecx, JSON array pointer in rdx and actual tick in r8; decompiler-inferred wrapper signatures are not used as ABI evidence"],
        "independent_implementation": [audit.families.identity(audit.a.ROOT / name) for name in (
            "crates/razer-platform/src/input/macro_recorder.rs",
            "crates/razer-platform/src/platform/windows/macro_recorder.rs",
            "crates/razer-platform/src/platform/windows/input_clock.rs",
            "crates/razer-service/src/runtime/windows/mapping_runtime/capture.rs",
            "crates/razer-service/src/runtime/windows/mapping_runtime/capture/recorder.rs",
            "crates/razer-service/src/mapping/recorder.rs",
            "crates/razer-service/src/mapping/recorder_owner.rs",
            "crates/razer-shell/src/shell/keyboard/keyboard_latest_restore_publisher.rs")],
        "shared_query_owner": [
            "The recording actor retains its one ServiceClient through stop, restoration and child teardown; a weak registry exposes only that process's real MacroRecordingMode query",
            "PID756 restore publication queries the retained recording owner instead of constructing or querying a recorder in its separate mapping worker",
            "Each observation requests the live input thread mode; there is no copied mode cache or invented inactive default",
            "Shutdown rejects new borrowed observations before cleanup; an in-flight query serializes with teardown, and its synchronization-cell borrow cannot retain the child beyond the owning actor",
            "Registry locks do not span transport calls or suppression acknowledgement waits; only the matching owner can retire its registry slot"],
        "production_input_exports_removed": ["hasMacroRecordingStarted", "startMacroRecording",
            "stopMacroRecording", "getMacroRecordingMode", "registerMacroRecorderEvent",
            "unregisterMacroRecorderEvent", "setMacroRecorderEventCallback"],
        "cleanup_semantics": "Rust reports UnhookWindowsHookEx failure and retains failed handles on ordinary Stop; owner shutdown joins the hook thread and reports cleanup failure instead of claiming success",
        "limitations": [
            "The independent input recorder does not replace mapping suppression, GlobalMode producers, macro publication/execution, product OTFM input or device write-back",
            "Those remaining production mapping DLL calls are unfinished implementation; no complete DLL or macro-page replacement is claimed",
            "Generic native scheduling and JSON allocator helpers are not reimplemented byte-for-byte; retained complete owners define input/state/event/lifecycle semantics",
            "No actual hook installation, injected input, vendor export, application window or device was executed"],
        "target_execution": False, "runtime_acceptance": "unverified"}


def retain_philips_measurement_lifetime(audit):
    """Preserve current callback ABIs and the owned device removal/join chain."""
    original, provenance = verified_host_native(audit, "CommonDLL/mapping_engine.dll")
    retained = audit.native_slice(original, ("addUsbDevice", "removeUsbDevice",
        "setHardwareEventCallback", "registerHardwareEvent", "unregisterHardwareEvent",
        "setInputNotificationCallback", "registerInputNotification", "unregisterInputNotification",
        "enableMouseMoveRedirect", "disableMouseMoveRedirect", "setMouseMoveRedirectCallback",
        "registerGlobalShortcut", "unregisterGlobalShortcut"), depth=1,
        rvas=(0x57ac0, 0x53090, 0x54930, 0x57e60, 0x6269e, 0x62708, 0x6127c,
              0x60dfa, 0x5f917, 0xa92a0, 0xa8c2c, 0x656ca, 0x65c90, 0x1b8670, 0x1a9710))
    if retained["failures"] or retained["decompiler_failures"]:
        raise ValueError("Current Philips callback/lifetime native owners are unavailable")
    return {**provenance, "native": retained,
        "callback_abi": {
            "hardware_input_device": "void(const char*, DeviceEventType/int32, const char*, uint64)",
            "mouse_move": "void(const char*, DeviceEventType/int32, uint8, uint8, uint64)",
            "completion": "void(bool, const char*, const char*)"},
        "removal_chain": [
            "57ac0 -> 53090/54930 -> 57e60 -> 6269e/62708/6127c/60dfa/5f917",
            "a92a0/a8c2c/656ca -> 65c90 -> 1b8670 StopSoon -> 1a9710 WaitForSingleObject(INFINITE) and CloseHandle",
            "Device worker completion clears callback slot+768 after its join, before the successful removeUsbDevice callback"],
        "cancellation_semantics": [
            "disableMouseMoveRedirect clears only the enabled flag+760; it does not erase callback slot+768",
            "Already posted callback functors may arrive after redirect is disabled; retained callback context must survive owned device removal and worker completion",
            "Source per-engine Escape registration, timers and event forwarding are separate from native callback/resource ownership"],
        "limitations": ["The complete retained functions form a bounded native slice, not proof that all external callback queues drain",
            "No vendor code, native export, application window, real device or installed service was executed"],
        "target_execution": False, "runtime_acceptance": "unverified"}


def audit_stream_mixer_profile_notification(prepare=False):
    """Existing Seiren query's complete local profile notification subchain."""
    import workspace
    audit = HostLocalAudit()
    a = audit.a
    for name in ("tools/host.py", "tools/families.py", "tools/evidence.py", "tools/analysis.py", "tools/workspace.py"):
        audit.artifact(a.ROOT / name)
    source_sets, files = {}, {}
    for name in ("audio-mw-1422-current", "notification-manager-current", "background-manager-current"):
        source_sets[name], files[name] = audit.source_set(name)
        source_sets[name]["verified_complete_capture"] = audit.frontend_capture(name, files[name])
    def original(name, token):
        found = [row for row in files[name] if row["path"].endswith(".js") and token.encode() in audit.read(row)]
        if len(found) != 1:
            raise ValueError("Current notification source owner is absent or ambiguous: " + token)
        return found[0]
    mw = files["audio-mw-1422-current"]
    ui = files["notification-manager-current"]
    producer = original("audio-mw-1422-current", 'this.presetChangedByDeviceHandler=')
    background = original("background-manager-current", 'ot=[{name:"notification-manager"')
    root = original("notification-manager-current", 'const P=new C,U=')
    selected = {}
    for role, source, token, kinds in (
        ("stream_mixer_actual_preset_change_producer", producer, 'this.presetChangedByDeviceHandler=', {"assignment_expression"}),
        ("stream_mixer_actual_globalshortcut_event", producer, 'this.initGlobalShortcutEvent=', {"assignment_expression"}),
        ("stream_mixer_preset_and_audio_owner", producer, 'this.presetChangedByDeviceHandler=', {"class", "class_declaration"}),
        ("notification_consumer_windowstorage_owner", root, 'class C extends j.EventEmitter', {"class_declaration"}),
        ("notification_storage_diff_initialization_and_subscription", root, 'J=async()=>{P.on("windowstorageevent"', {"variable_declarator"}),
        ("notification_card_complete_mounted_owner", root, 'R=e=>{let t=e.notifications', {"variable_declarator"}),
        ("notification_card_hook_and_dismissal", root, 'x=e=>{let t=e.notifications', {"variable_declarator"}),
        ("background_notification_window_descriptor", background, 'ot=[{name:"notification-manager"', {"variable_declarator"}),
        ("background_actual_startup_and_lifecycle", background, 'qC=new class{numberOfStart=0', {"variable_declarator"}),
        ("background_complete_window_tab_opener", background, 'Mt=new class', {"variable_declarator"}),
    ):
        selected[role] = audit.retain(role, source, token, kinds)
    exports = [{"role": role, **audit.webpack_export(role, rows, module, export)} for role, rows, module, export in (
        ("stream_mixer_windowstorage_notification_producer", mw, 44857, "Ds"),
        ("stream_mixer_windowstorage_notification_remove", mw, 44857, "ww"),
        ("stream_mixer_source_constants_and_url_container", mw, 26502, "wI"),
        ("stream_mixer_source_host_adapter", mw, 91902, "A"),
        ("notification_complete_root_and_consumer", ui, 5568, "default"),
        ("notification_geometry_and_empty_click", ui, 2864, "Ru"),
        ("notification_host_bridge", ui, 7217, "A"),
        ("notification_redux_store", ui, 2724, "A"),
        ("notification_transition_and_default_constants", ui, 5596, "xt"),
        ("notification_action_constants", ui, 7358, "wy"),
        ("notification_display_setting_local_storage", ui, 8531, "A"),
        ("notification_local_translation", ui, 7384, "JN"),
        ("notification_runtime_positions", ui, 9749, "z"))]
    constants = next(item for item in exports if item["role"] == "stream_mixer_source_constants_and_url_container")
    constants_owner = audit.owners[constants["binding"]["owner_id"]]
    constants_source = next(row for row in mw if row["path"] == constants_owner["original"]["path"])
    container = audit.binding("source_container_query_parameter", constants_source, "_", constants_owner["byte_start"])
    styles = []
    for row in ui:
        if Path(row["path"]).suffix == ".css" and Path(row["path"]).name in ("Root.ece81d56.chunk.css", "main.505ec086.css"):
            styles.append({"original": row, "complete_body": audit.read(row).decode("utf-8")})
    if len(styles) != 2:
        raise ValueError("Current notification main/Root CSS closure is unavailable")
    host = verified_host_owners(audit, ("electron/modules/window_storage/index.js", "electron/components/Tab/Tab.js",
        "electron/main.js", "electron/preload.js", "electron/constants.js", "electron/modules/sysutil/win/index.js"))
    host_selected = {}
    for suffix, token, role, kinds in (
        ("electron/modules/window_storage/index.js", 'this.windowStorageMap=new Map', "host_windowstorage_complete_owner", {"class", "class_declaration"}),
        ("electron/components/Tab/Tab.js", 'initEventHandler(){', "host_tab_lifetime_storage_quirk", {"class", "class_declaration"}),
        ("electron/main.js", 'case D.GET_DISPLAY_NEAREST_POINT:', "host_actual_display_lookup", {"switch_case"}),
        ("electron/main.js", 'case D.SET_BOUNDS:', "host_actual_window_bounds", {"switch_case"}),
        ("electron/main.js", 'case D.SET_BROWSER_VISIBLE:', "host_actual_browser_visibility", {"switch_case"}),
        ("electron/main.js", 'case D.SET_BROWSER_ON_TOP:', "host_actual_topmost", {"switch_case"}),
        ("electron/main.js", 'case"isFullScreenMode":', "host_actual_fullscreen_dispatch", {"switch_case"}),
    ):
        host_selected[role] = audit.retain(role, host[suffix]["original"], token, kinds)
    native_input, native_provenance = verified_host_native(audit, "CommonDLL/SysUtilsNative.dll")
    fullscreen = audit.native_slice(native_input, ["isFullScreenMode"], depth=0)
    if fullscreen["failures"]:
        raise ValueError("Current native fullscreen owner is unavailable")
    notification_acquisition = json.loads(audit.artifact(a.ROOT / source_sets["notification-manager-current"]["acquisition"]["path"]))
    icons = [row for row in notification_acquisition["files"] if row.get("path", "").endswith("/static/media/razer-ths.d6985a25.svg")]
    if len(icons) != 1:
        raise ValueError("Current notification brand resource is unavailable")
    audit.read(icons[0])
    icon = {"original": icons[0], "runtime_key": "synapse/notification-profile-switch-brand.svg"}
    acquisition_path = source_sets["notification-manager-current"]["acquisition"]["path"]
    requests = {icons[0]["path"]: icon["runtime_key"]}
    prepared = (workspace.prepare_resources(acquisition_path, requests) if prepare
                else audit.families.pinned_resources(acquisition_path, requests))
    if len(prepared) != 1:
        raise ValueError("Current notification resource preparation is ambiguous")
    icon.update({"preparation": prepared[0],
                 "physical": {"path": prepared[0]["output"], "sha256": prepared[0]["output_sha256"]}})
    result = {"source_sets": list(source_sets.values()), "selected_owners": selected, "current_exports": exports,
        "source_container_parameter": container, "complete_styles": styles, "complete_host_sources": list(host.values()),
        "selected_host_owners": host_selected, "native_fullscreen": {**native_provenance, "slice": fullscreen},
        "resource": icon, "semantics": {
            "actual_trigger": "globalshortcutevent.event -> getTargetPreset -> presetChangedByDeviceHandler(event.timeTick,guid); cache active GUID save and initActiveStreamMixerPreset precede notification",
            "producer": "44857.w writes the exact persistent switch-profile record to its own WindowStorage URL with actual container query parameter, translated current edition product footer, raw preset title and callback timeTick",
            "two_timeouts": "w replaces its own id-type 3000ms timeout; the caller schedules an independent P timeout after awaiting w. An earlier caller timeout survives a newer notification",
            "consumer": "5568.J uses timerTick/id/type diff, updates only increasing tick, H schedules a separate 3000ms consumer timeout; fullscreen applies only to new SHOW, UPDATE bypasses it",
            "removal": "Shortened-array path parses aggregate row itself and catches to absent; explicit removed/cleared reads row.value and compares tick across owners. Both timer-map/orphan quirks are retained",
            "window": "BackgroundStation.start opens one hidden, topmost, taskbar-hidden, shadowless and nonresizable resident window. Empty notifications hide it rather than destroy it",
            "geometry": "Card width360, full-monitor lower right margin20/bottom80, min window380x250, actual measured stacks with gap6; nearest point comes from first record.screen, absent->(0,0). payload.avaiLeft is not substituted",
            "ui": "Exact Root/main CSS cascade: #111/#ccc/#44d62c, body14Roboto, header uppercase with margin-bottom11 plus padding-bottom10, preset20RazerF5, footer14/17, outline1 and 32px blank pointer close area",
            "motion_and_click": "Mount waits100ms then enters over200ms, opacity linear/translate15 CSS ease-in-out. Pointer close exits200ms then O; whole-card xP is empty. No source keyboard handlers or focus transfer are added",
            "local_storage": "2864.d/h mutate the measured source notification array, then assign r=f(r). f rereads displayNotifications on each visibility effect but returns undefined, so 8531.set stores the raw string undefined for notifications. 8531.get catches JSON parse failure and preserves its original raw string; only null/missing displayNotifications initializes true",
            "source_cleanup": "Producer owner destruction cancels its JS timers; manager independently unregisters storage listeners before unload. Current host Tab clear sender-method quirk does not establish source URL removal/broadcast",
            "fullscreen_native": "SysUtilsNative.isFullScreenMode gets foreground RECT, excludes shell/desktop, uses MonitorFromPoint(rect.left,rect.top,nearest), and requires exact equality of all rcMonitor edges"},
        "rust_implementation": [audit.families.identity(a.ROOT / name) for name in (
            "crates/razer-storage/src/profile_notification.rs", "crates/razer-shell/src/shell/windows/profile_switch_notification.rs")],
        "static_source_query": "complete_for_mounted_stream_mixer_switch_profile_branch",
        "full_notification_manager_feature_implementation": False,
        "pure_and_simulated_verification": "separately recorded in .analysis/validation.json",
        "target_execution": False, "runtime_acceptance": "unverified", "complete_originals": list(audit.sources.values()),
        "complete_source_owners": list(audit.owners.values()), "verified_artifacts": list(audit.artifacts.values())}
    result["changed_during_query"] = audit.final_snapshot()
    result["snapshot_stable"] = not result["changed_during_query"]
    if result["changed_during_query"]:
        raise ValueError("Current notification artifact changed during query: " + ", ".join(result["changed_during_query"]))
    return result


def retain_otfm_product_triggers(audit):
    """Verify current entry relationships and full AST owners for every retained trigger."""
    import evidence
    path = audit.a.ROOT / ".analysis/results/frontend/background-manager-current/otfm-trigger-owners.json"
    retained = json.loads(audit.artifact(path))
    if retained["target_execution"] is not False or retained["current_conditions_unchanged"] is not True:
        raise ValueError("OTFM trigger analysis has an invalid evidence boundary")
    indexed = {row["path"]: row for row in json.loads(
        audit.artifact(audit.a.ROOT / ".analysis/inputs.json"))["inputs"]}
    deployments, referenced = {}, {}
    for name, row in retained["deployments"].items():
        acquired = json.loads(audit.artifact(audit.a.ROOT / row["acquisition"]))
        if row["accepted"]:
            if acquired.get("source_current_during_acquisition") is not True:
                raise ValueError("OTFM trigger source is not a verified current deployment")
            deployment = evidence.frontend_deployment_sources(acquired)
            if deployment != row["deployment"]:
                raise ValueError("OTFM trigger entry references differ from retained analysis")
            deployments[name] = deployment
            referenced[name] = {item["original"]["path"] for item in deployment["sources"]
                                if item["identity_verified"]}
            for identity in [deployment["entry_original"], *[item["original"] for item in deployment["manifests"]
                                                          if item.get("original") is not None]]:
                audit.read(identity)
    grouped, verified = {}, {}
    for owner in retained["retained_owners"]:
        grouped.setdefault(owner["original"]["path"], []).append(owner)
    existing_trees, existing_raw = set(audit.parsed), set(audit.raw)
    for name, owners in grouped.items():
        original = owners[0]["original"]
        identity = indexed.get(name)
        deployment = "/".join(name.split("/")[:4])
        if identity is None or any(identity[key] != original[key] for key in ("bytes", "sha256")) \
                or name not in referenced.get(deployment, set()):
            raise ValueError("OTFM owner is outside the verified current entry closure: " + name)
        raw, tree = audit.read(original), audit.tree(original)
        for owner in owners:
            start, end = owner["byte_start"], owner["byte_end"]
            body = raw[start:end]
            node = tree.root_node.descendant_for_byte_range(start, end - 1)
            if owner["original"] != original or (node.start_byte, node.end_byte, node.type) != (
                    start, end, owner["syntax"]) or body.decode("utf-8") != owner["body"] \
                    or hashlib.sha256(body).hexdigest() != owner["body_sha256"]:
                raise ValueError("OTFM trigger owner differs from its complete source AST")
            identifier = audit.retain_range("otfm_trigger_current_owner", original, start, end, node.type)
            if identifier != owner["id"]:
                raise ValueError("OTFM trigger owner identity differs")
            verified[identifier] = audit.owners[identifier]
        if name not in existing_trees:
            audit.parsed.pop(name)
        if name not in existing_raw:
            audit.raw.pop(name)
    products, fields, imports = set(), {"eventType", "containerId", "productId", "layoutId", "deviceId",
                                       "macro_recorder_mode"}, 0
    for row in retained["semantic"]:
        if row["unresolved"] or type(row["product_key"]) is not int or row["product_key"] in products \
                or set(row["payload"]) != fields or row["owner"] not in verified \
                or row["postmessage_owner"] not in verified or not row["entry_imports"]:
            raise ValueError("OTFM producer has missing or ambiguous argument/entry bindings")
        products.add(row["product_key"])
        for item in row["entry_imports"]:
            if item["owner"] not in verified or verified[item["owner"]]["original"] != item["main"] \
                    or verified[item["owner"]]["body"] != item["body"]:
                raise ValueError("OTFM producer main entry import is not a retained owner")
            imports += 1
    if len(products) != len(retained["owners"]):
        raise ValueError("OTFM producer and semantic lists do not cover the same products")
    return {"analysis": audit.artifacts[audit.a.rel(path)], "products": sorted(products),
            "producer_count": len(retained["owners"]), "owner_ids": sorted(verified),
            "semantic": retained["semantic"], "excluded": retained["excluded"],
            "entry_imports_verified": imports, "source_sets_verified": len(deployments),
            "scope": "Current retained entry-to-broadcast producers; device eligibility and all downstream product mappings require separate verification",
            "target_execution": False, "actual_dll_comparison": "unverified"}


def retain_otfm_eligibility(audit, triggers):
    """Verify each reviewed participant identity, product guard and save owner."""
    path = audit.a.ROOT / ".analysis/results/frontend/background-manager-current/otfm-eligibility.json"
    retained = json.loads(audit.artifact(path))
    if retained["target_execution"] is not False or retained["current_conditions_unchanged"] is not True:
        raise ValueError("OTFM eligibility review has an invalid evidence boundary")
    previous = retained["previous_producer_analysis"]
    if any(previous[key] != triggers["analysis"][key] for key in ("path", "bytes", "sha256")):
        raise ValueError("OTFM eligibility review refers to a different trigger analysis")
    indexed = {row["path"]: row for row in json.loads(
        audit.artifact(audit.a.ROOT / ".analysis/inputs.json"))["inputs"]}
    existing_trees, existing_raw = set(audit.parsed), set(audit.raw)
    for row in retained["source_identities"]:
        original = indexed[row["path"]]
        if any(row[key] != original[key] for key in ("bytes", "sha256")):
            raise ValueError("OTFM eligibility source differs from current indexed bytes")
        audit.read(original)
        if row["path"] not in existing_raw:
            audit.raw.pop(row["path"])
    for row in retained["acquisition_receipts"]:
        receipt = json.loads(audit.artifact(audit.a.ROOT / row["path"], row))
        if receipt.get("source_current_during_acquisition") is not True:
            raise ValueError("OTFM eligibility source has no current deployment receipt")
    grouped, verified, reviewed = {}, {}, {}
    for owner in retained["retained_owners"]:
        grouped.setdefault(owner["original"]["path"], []).append(owner)
    for name, owners in grouped.items():
        original = indexed[name]
        raw = audit.read(original)
        for owner in owners:
            start, end = owner["byte_start"], owner["byte_end"]
            body = raw[start:end]
            if any(owner["original"][key] != original[key] for key in ("bytes", "sha256")) \
                    or body.decode("utf-8") != owner["body"] \
                    or hashlib.sha256(body).hexdigest() != owner["body_sha256"]:
                raise ValueError("OTFM eligibility owner differs from original bytes")
            existing = audit.owners.get(owner["id"])
            if existing is not None:
                if any(existing[key] != owner[key] for key in (
                        "byte_start", "byte_end", "syntax", "body", "body_sha256")):
                    raise ValueError("OTFM shared owner differs between the two reviewed chains")
                identifier = owner["id"]
            else:
                node = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
                if (node.start_byte, node.end_byte, node.type) != (start, end, owner["syntax"]):
                    raise ValueError("OTFM eligibility owner is not its complete original AST")
                identifier = audit.retain_range("otfm_eligibility_owner", original, start, end, node.type)
            if identifier != owner["id"] or identifier in reviewed:
                raise ValueError("OTFM eligibility owner has an invalid or duplicate identity")
            reviewed[identifier], verified[identifier] = owner, audit.owners[identifier]
        if name not in existing_trees:
            audit.parsed.pop(name, None)
        if name not in existing_raw:
            audit.raw.pop(name, None)
    keys = {row["product_id"] for row in retained["private_lookup"]}
    products = set(triggers["products"])
    summary = retained["summary"]
    if (len(keys), len(products), sorted(keys - products), sorted(products - keys)) != (
            summary["private_lookup_keys"], summary["current_producer_keys"],
            summary["lookup_only_keys"], summary["producer_only_keys"]):
        raise ValueError("OTFM participant and producer differences are inconsistent")
    catalog = retained["current_catalog"]["original"]
    rows = json.loads(audit.read(indexed[catalog["path"]]))
    if len(rows) != len(retained["catalog_rows"]) or any(
            row["value"] != rows[row["row_index"]] or row["owner"] not in verified
            for row in retained["catalog_rows"]):
        raise ValueError("OTFM catalog relationships differ from the complete original row order")
    for row in retained["private_lookup"]:
        owner = reviewed[row["owner"]]
        if not re.match(r"Wt\.set\(" + str(row["product_id"]) + r",", owner["body"]):
            raise ValueError("OTFM participant key differs from its retained lookup call")
    categories = {}
    for row in retained["producer_only"]:
        guard = row["input_guard"]
        if guard["unresolved"] or row["current_producer_sends_primary_product_id"] != row["product_id"]:
            raise ValueError("OTFM product guard or primary PID binding remains unresolved")
        for name in ("input_case_owner", "state_owner", "state_guard_owner", "allow_action_owner",
                     "task_init_call_owner", "task_static_init_owner"):
            if guard[name] not in verified:
                raise ValueError("OTFM product input/guard/task has no complete retained owner")
        categories.setdefault(row["device_info_category"], []).append(row["product_id"])
    if {key: sorted(value) for key, value in categories.items()} != summary["producer_only_categories"]:
        raise ValueError("OTFM product categories differ from their reviewed identity fields")
    return {"analysis": audit.artifacts[audit.a.rel(path)], "summary": summary,
            "background_semantics": retained["background_semantics"],
            "owner_ids": sorted(verified), "remaining": retained["unresolved"],
            "target_execution": False, "actual_dll_comparison": "unverified"}


def audit_host_macro_recorder(args):
    audit = HostLocalAudit()
    a, families = audit.a, audit.families
    report = {"recorded_at_utc": a.stamp(), "tool": families.identity(Path(__file__)),
              "parser": audit.parser_versions, "source_sets": [], "failures": [],
              "source_chain_complete": False, "product_code_modified": False,
              "target_execution": False, "real_device_operations": False, "runtime_acceptance": "unverified"}
    try:
        for tool in ("tools/host.py", "tools/evidence.py", "tools/families.py", "tools/analysis.py", "tools/workspace.py"):
            audit.artifact(a.ROOT / tool)
        mouse, mw = audit.source_set("mouse-mw-192-current")
        macro, ui = audit.source_set("synapse-macro-current")
        background, bg = audit.source_set("background-manager-current")
        keyboard, keyboard_mw = audit.source_set("keyboard-mw-756-current")
        audio, audio_mw = audit.source_set("audio-mw-1422-current")
        report["source_sets"] = [mouse, macro, background, keyboard, audio]
        report["reused_complete_captures"] = [audit.frontend_capture(name, files) for name, files in (
            ("mouse-mw-192-current", mw), ("synapse-macro-current", ui), ("background-manager-current", bg),
            ("keyboard-mw-756-current", keyboard_mw), ("audio-mw-1422-current", audio_mw))]

        def source(files, needle):
            matches = [row for row in files if Path(row["path"]).suffix == ".js" and needle.encode() in audit.read(row)]
            if len(matches) != 1:
                raise ValueError("Local macro source owner is absent or ambiguous: " + needle)
            return matches[0]

        mouse_main = source(mw, 'RZ_WINDOWSTORAGE_MACRO_RECORDER:"RzWinStorage_macro_recorder"')
        mouse_chunk = source(mw, 'function An(e=y.default.runtimeData,t)')
        macro_main = source(ui, 'no="RzWinStorage_macro_recorder"')
        macro_recording_ui = source(ui, 'xa=e=>{const t=(0,Se.useDispatch)()')
        macro_xstate = source(ui, 't.prototype.next=function(t,e){var n,o,a,s=this,c=e.name')
        otfm = source(bg, 'gt="RzWinStorage_macro_recorder"')
        background_main = source(bg, 'gC(()=>te(()=>import("./otfm-d56d4f50.js").then(e=>e.ad)')
        keyboard_otfm = source(keyboard_mw, 'eventType:"start-otfm"')
        audio_router = source(audio_mw, 'routeExternalDevice(e,t){')
        selected = {}
        for role, original, anchor, kinds in (
            ("mouse_macro_storage_key", mouse_main, 'RZ_WINDOWSTORAGE_MACRO_RECORDER:"RzWinStorage_macro_recorder"', {"pair"}),
            ("mouse_initial_lock_and_local_state", mouse_chunk, 'function An(e=y.default.runtimeData,t)', {"function_declaration"}),
            ("mouse_windowstorage_event_consumer", mouse_chunk, 'q=e=>V(void 0,void 0,void 0,(function*(){var t,n,o,i,u,p,v;const{newValue:', {"variable_declarator"}),
            ("mouse_storage_event_subscribe", mouse_chunk, 'S.Z.on("windowstorageevent",q)', {"variable_declarator"}),
            ("mouse_storage_event_cleanup", mouse_chunk, 'S.Z.off("windowstorageevent",q)', {"variable_declarator"}),
            ("mouse_storage_event_activation", mouse_chunk, 'J=()=>{Z=!0}', {"variable_declarator"}),
            ("mouse_device_otfm_event", mouse_chunk, 'G=e=>{const{timerTick:t}=e,n=new o.bM("ON_OTFM"', {"variable_declarator"}),
            ("macro_literal_storage_key", macro_main, 'no="RzWinStorage_macro_recorder"', {"variable_declarator"}),
            ("macro_complete_recording_machine", macro_main, 'id:"RECORDING",initial:"idle"', {"arrow_function"}),
            ("macro_copy_first_existing_owner_value", macro_main, 'Er=async()=>{try{var e,t;const n=JSON.parse', {"variable_declarator"}),
            ("macro_recording_stop_guard", macro_main, 'dr=async()=>{if(pr)return;', {"variable_declarator"}),
            ("macro_startup_owner", macro_main, 'hr=async()=>{kr(),await gr()', {"variable_declarator"}),
            ("macro_start_and_otfm_event", macro_main, 'Sr=e=>{Dr.onMacroStarted', {"variable_declarator"}),
            ("macro_stopped_event", macro_main, 'Ar=()=>{Do.j.reduxStore.dispatch({type:h.GR', {"variable_declarator"}),
            ("macro_globalshortcut_stop", macro_main, 'br=(0,G.sg)(async e=>', {"variable_declarator"}),
            ("macro_postrecording_and_close_tab_owner", macro_main, 'Nr=()=>{window.addEventListener("postRecordingEvent"', {"variable_declarator"}),
            ("macro_complete_reducer_and_saved_callback", macro_main, 'Zs=(e,t)=>{var n;let o=null,r=!0,a=!1,s=!0;', {"variable_declarator"}),
            ("macro_recording_ui_start_stop_cancel_and_cleanup", macro_recording_ui, 'xa=e=>{const t=(0,Se.useDispatch)()', {"variable_declarator"}),
            ("macro_keyboard_editor_shortcut_suspension", macro_recording_ui, 'yn=e=>{const{item:t,itemIndex:n}=e', {"variable_declarator"}),
            ("macro_record_shortcut_capture_lifetime", macro_recording_ui, 'Rr=e=>{const{inputIds:t=Nr', {"variable_declarator"}),
            ("macro_record_shortcut_mounted_caller", macro_recording_ui, 'Lr=(0,Se.connect)(e=>({macroList:', {"variable_declarator"}),
            ("macro_mapping_engine_callback_results_are_data", macro_main, 'this.callMappingEngineAction=async e=>', {"assignment_expression"}),
            ("macro_xstate_first_matching_transition", macro_xstate, 't.prototype.next=function(t,e){var n,o,a,s=this,c=e.name', {"assignment_expression"}),
            ("macro_xstate_candidate_order", macro_xstate, 't.prototype.getCandidates=function(t)', {"assignment_expression"}),
            ("macro_xstate_transition_array_order", macro_xstate, 't.prototype.formatTransitions=function()', {"assignment_expression"}),
            ("macro_recorder_event_subscription", macro_main, 'Or=async()=>(vo.A.on("macroitem"', {"variable_declarator"}),
            ("macro_unload_stop", macro_main, 'window.onbeforeunload=()=>{console.log("window.onbeforeunload"),dr()}', {"assignment_expression"}),
            ("macro_windowstorage_set_adapter", macro_main, 'this.setWindowStorageItem=(e,t)=>', {"assignment_expression"}),
            ("macro_windowstorage_get_adapter", macro_main, 'this.getWindowStorageItem=e=>', {"assignment_expression"}),
            ("macro_windowstorage_remove_adapter", macro_main, 'this.removeWindowStorageItem=e=>', {"assignment_expression"}),
            ("macro_start_recording_adapter", macro_main, 'this.startMacroRecording=async e=>', {"assignment_expression"}),
            ("macro_stop_recording_adapter", macro_main, 'this.stopMacroRecording=async()=>', {"assignment_expression"}),
            ("mouse_receiver_connected_projection", mouse_chunk, 'he=()=>L(void 0,void 0,void 0,(function*(){var e;const t=g.razerWindow.rzDevice', {"variable_declarator"}),
            ("mouse_receiver_layout_resolver", mouse_chunk, '_e=(e,t,n)=>L(void 0,void 0,void 0,(function*(){try{if(!e)return console.warn("[getMouseLayoutId]', {"variable_declarator"}),
            ("mouse_receiver_multiple_device_gate", mouse_chunk, 'V=()=>{const{canPairMultipleDevices:e}', {"variable_declarator"}),
            ("mouse_receiver_serial_cache_selection", mouse_chunk, 'Ie=e=>{var t;const n=JSON.parse', {"variable_declarator"}),
            ("mouse_complete_receiver_module", mouse_chunk, '34340:', {"pair"}),
            ("mouse_default_injector_factory", mouse_main, '95414:', {"pair"}),
            ("mouse_default_runtime_factory", mouse_main, '11579:', {"pair"}),
            ("mouse_bootstrap_feature_mutations", mouse_main, 'useFeature("calibrationInterfaces","orca5PlusCalibrationEngine")', {"arrow_function"}),
            ("mouse_complete_decision_task_listener", mouse_chunk, 'ze=()=>Ve(void 0,void 0,void 0,(function*(){p.razerWindow.addEventListener("rz_ui_action"', {"variable_declarator"}),
            ("mouse_brightness_actual_task_branch", mouse_chunk, 'case"ON_BRIGHTNESS_MACRO_RECORDING_START":case"ON_BRIGHTNESS_OTFM_START"', {"switch_statement"}),
            ("mouse_decision_descriptor_emitter", mouse_chunk, 'R=(e,t)=>{var n,o,i,r;const a=/done.invoke.get[a-z]*Locks/i.test(e.type)', {"variable_declarator"}),
            ("mouse_brightness_stop_descriptors", mouse_chunk, 'M=[{target:"disabled",actions:["updateSwitchOffSettingContext"', {"variable_declarator"}),
            ("mouse_complete_machine_guards", mouse_chunk, 'brightnessOTFMGuard:(e,t,n)=>"OTFMStarted"===n.state.value.OTFM', {"variable_declarator"}),
            ("otfm_storage_key", otfm, 'gt="RzWinStorage_macro_recorder"', {"variable_declarator"}),
            ("otfm_complete_state_machine", otfm, 'Ct=e=>{const o={...$t,...e};return m({id:"OTFM"', {"variable_declarator"}),
            ("otfm_exclusive_storage_producer", otfm, 'lo=({containerId:e,productId:t,layoutId:o})=>', {"variable_declarator"}),
            ("otfm_start_order", otfm, 'co=async({containerId:e,productId:t,layoutId:o})=>', {"variable_declarator"}),
            ("otfm_global_recorder_lock", otfm, 'po=()=>new Promise((e,t)=>{navigator.locks.request("RzGlobalLock_macro_recorder"', {"variable_declarator"}),
            ("otfm_lock_release_and_storage_remove", otfm, 'ko=()=>{Jt&&Jt(),Me.removeWindowStorageItem(gt)}', {"variable_declarator"}),
            ("otfm_post_recording_persistence_and_events", otfm, 'Ro=()=>{const e=[];return window.addEventListener("storage"', {"variable_declarator"}),
            ("otfm_recorder_listeners", otfm, 'Mo={startListen:async()', {"variable_declarator"}),
            ("otfm_system_listeners", otfm, 'so={startListen(){Ft?', {"variable_declarator"}),
            ("otfm_brightness_then_initial_press", otfm, 'Lo=async()=>{const e=await $o.serviceGetDeviceBrightness(Yt)', {"variable_declarator"}),
            ("otfm_init_export", otfm, 'To=Object.freeze(Object.defineProperty({__proto__:null,initOTFM:', {"variable_declarator"}),
            ("otfm_complete_local_services", otfm, '$o={serviceTurningOnMacroLED:', {"variable_declarator"}),
            ("otfm_current_background_import", background_main, 'import("./otfm-d56d4f50.js").then(e=>e.ad)', {"call_expression"}),
            ("otfm_current_background_init", background_main, '({initOTFM:e})=>{e().then', {"arrow_function"}),
            ("keyboard_otfm_complete_caller_module", keyboard_otfm, '26568(e,t,i){', {"method_definition"}),
            ("keyboard_otfm_complete_action_listener", keyboard_otfm, 'eventType:"start-otfm"', {"variable_declarator"}),
            ("keyboard_otfm_start_action_branch", keyboard_otfm, 'eventType:"start-otfm"', {"switch_case"}),
            ("audio_route_external_device", audio_router, 'routeExternalDevice(e,t){', {"method_definition"}),
            ("audio_util_dispatch_boolean", audio_router, 'dispatch(e,t,i={},o){', {"method_definition"}),
            ("audio_util_initialization_owner", audio_router, 'this.audioUtil=new T.A(', {"method_definition"}),
            ("audio_playback_mix_route_consumer", audio_router, 'changePlaybackMixDevice(e){', {"method_definition"}),
            ("audio_update_mix_input_route_consumer", audio_router, 'updateMixInput(e){', {"method_definition"}),
        ):
            selected[role] = audit.retain(role, original, anchor, kinds)
        report["selected_owners"] = selected
        machine = audit.owners[selected["macro_complete_recording_machine"][0]]
        report["macro_machine_dependencies"] = [audit.binding("macro_dependency_" + name, macro_main, name,
            machine["byte_start"]) for name in ("no", "vo", "m", "dr", "Io", "To")]
        report["global_shortcut_capture_lifecycle"] = {
            "keyboard_editor": "yn click disables mapping and global shortcuts; its blur/click exit enables both after the edit lock",
            "record_shortcut": "Lr mounts Rr; browser keycodes and inputredirect share click/focus disable and blur/outside-click/unmount enable calls",
            "software_recording": "RECORDING.disablingMapping calls only disableMapping; enablingMapping calls Promise.all(enableMapping,enableGlobalShortcut)",
            "response_semantics": "callMappingEngineAction preserves callback {result,reason}; false is data, transport rejection remains an error",
            "rust_owner": "MacroPage capture lifetimes -> AppShell retained global intent -> all mounted1422/1446 Audio_StreamMixer worker FIFOs; new workers inherit intent before first preset registration",
            "runtime_acceptance": "unverified"}
        receiver = audit.owners[selected["mouse_receiver_serial_cache_selection"][0]]
        report["receiver_cache_dependencies"] = [audit.binding("mouse_receiver_cache_" + name, mouse_chunk, name,
            receiver["byte_start"]) for name in ("De", "X", "Ue", "N")]
        otfm_owner = audit.owners[selected["otfm_init_export"][0]]
        report["otfm_dependencies"] = [audit.binding("otfm_dependency_" + name, otfm, name,
            otfm_owner["byte_start"]) for name in ("At", "Me", "Se", "Je", "Ge", "No", "bo", "go", "fo", "uo", "So",
                "$t", "Rt", "Mt", "bt", "kt", "_t", "Ot", "Pt", "Kt", "Bt", "Dt", "ho", "wo", "Io", "Ao", "Wt")]
        for role, anchor, kinds in (
                ("otfm_assign_input_validation", 'collectInputAndCheckValid:mo,setTriggerKey:Eo}=', {"variable_declarator"}),
                ("otfm_assign_input_subscription", 'window.addEventListener("assignkey"', {"expression_statement"}),
                ("otfm_complete_device_manifest", 'Wt.set(679,', {"expression_statement"})):
            selected[role] = audit.retain(role, otfm, anchor, kinds)
        utilities = source(bg, 'export{r as K,qr as L,gr as M,Ho as R,rr as a,Na as b,Zr as c,qo as d,La as e,Pr as f,Xt as g,')
        report["otfm_utility_dependencies"] = [audit.binding("otfm_utility_" + name, utilities, name,
            len(audit.read(utilities)) - 100) for name in ("rr", "er", "ar", "or", "Zr", "zr", "Na", "La", "qo", "Pr", "Ho", "r", "Sa", "Ca", "Ma", "yr")]
        report["otfm_manifest_rows"] = [audit.owners[owner] for owner in audit.retain(
            "otfm_device_manifest_rows", otfm, 'Wt.set(', {"call_expression"}, unique=False)]
        mapping_host = verified_host_owners(audit, ("electron/modules/mapping_engine/win/index.js",))
        report["otfm_complete_mapping_host"] = list(mapping_host.values())
        mapping_source = mapping_host["electron/modules/mapping_engine/win/index.js"]["original"]
        report["otfm_native_callback_owners"] = {}
        for name in ("hasMacroRecordingStarted", "startMacroRecording", "stopMacroRecording", "registerMacroRecorderEvent",
                "unregisterMacroRecorderEvent", "setMacroRecorderEventCallback", "createCbMacroRecorderStarted",
                "createCbMacroRecorderStopped", "createCbMacroItem", "createCbInputNotification",
                "createCbDeviceModeChanged", "createCbGlobalShortcutEnableEvent", "disableGlobalShortcut",
                "enableGlobalShortcut", "disableMapping", "enableMapping", "registerInputNotification", "unregisterInputNotification",
                "setInputNotificationCallback", "getGlobalMode"):
            report["otfm_native_callback_owners"][name] = audit.retain_class_field(
                "otfm_native_" + name, mapping_source, name)
        keyboard_owner = audit.owners[selected["keyboard_otfm_start_action_branch"][0]]
        report["keyboard_otfm_dependencies"] = [audit.binding("keyboard_otfm_" + name, keyboard_otfm, name,
            keyboard_owner["byte_start"]) for name in ("pt", "dt", "G", "H", "U")]
        report["current_cross_chunk_exports"] = [audit.webpack_export(role, files, identifier, exported)
            for role, files, identifier, exported in (
                ("mouse_derived_feature_defaults", mw, 88443, "G"),
                ("mouse_effective_feature_lookup", mw, 88443, "N$"),
                ("mouse_runtime_layout_lookup", mw, 20236, "oP"),
                ("mouse_receiver_device_factory", mw, 20236, "f2"),
                ("mouse_hardware_module_adapter", mw, 96204, "v5"),
                ("mouse_runtime_storage_keys", mw, 76886, "rt"),
                ("mouse_complete_state_machine_and_descriptors", mw, 41374, "g5"),
                ("mouse_task_queue_and_feedback", mw, 18863, "Q"),
                ("mouse_recorder_brightness_start_execution", mw, 25947, "lk_"),
                ("mouse_display_power_subscription", mw, 64707, "Yu"),
                ("mouse_display_power_unsubscription", mw, 64707, "Hq"),
                ("mouse_system_idle_unsubscription", mw, 64707, "_P"),
                ("mouse_generic_event_registration_and_execution", mw, 48163, "RzGenericFeatureManager"),
                ("mouse_static_generic_event_configuration", mw, 92678, "FEATURE_EVENT_HANDLER"),
                ("macro_mapping_engine_adapter", ui, 39356, "A"),
                ("macro_windowstorage_adapter", ui, 37226, "A"),
                ("keyboard_windowstorage_adapter", keyboard_mw, 88558, "A"))]
        # Preserve every occurrence used to distinguish an active feature from
        # shared dormant branches. Only the verified current deployment is scanned.
        report["current_multiple_device_feature_occurrences"] = [
            {"original": row, "byte_start": hit.start(), "byte_end": hit.end()}
            for row in mw if Path(row["path"]).suffix == ".js"
            for hit in re.finditer(b"canPairMultipleDevices", audit.read(row))]
        report["macro_stop_cleanup_occurrences"] = {
            token: [{"original": row, "byte_start": hit.start(), "byte_end": hit.end()}
                    for row in ui if Path(row["path"]).suffix == ".js"
                    for hit in re.finditer(re.escape(token.encode("utf-8")), audit.read(row))]
            for token in ('"aborted"', "'aborted'", 'ON_STOP_RECORDING', 'removeWindowStorageItem(no)')}

        # Retain the actual host owners and verify their embedded bytes against
        # the indexed ASAR container, including the source's sender-method quirk.
        indexed = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
        originals = {row["path"]: row for row in indexed["inputs"]}
        host_receipt_path = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
        host_receipt = json.loads(audit.artifact(host_receipt_path))
        report["complete_host_sources"] = []
        for suffix in ("electron/modules/window_storage/index.js", "electron/components/Tab/Tab.js"):
            found = [row for row in indexed["inputs"] if row["path"].startswith(".analysis/inputs/host-current/asar/")
                     and row["path"].endswith("/" + suffix)]
            if len(found) != 1:
                raise ValueError("Current WindowStorage host owner is absent or ambiguous: " + suffix)
            row = found[0]
            container = originals[row["container"]]
            if container["sha256"] != row["container_sha256"] or row.get("unpacked") or row.get("offset") is None:
                raise ValueError("Current WindowStorage host container relation is unresolved")
            body, parent = audit.read(row), audit.read(container)
            if parent[row["offset"]:row["offset"] + len(body)] != body:
                raise ValueError("Current WindowStorage host owner differs from ASAR bytes")
            official_aliases = [item for item in indexed["inputs"]
                                if item["path"].startswith(".analysis/inputs/host-current/extracted/")
                                and item["sha256"] == container["sha256"] and item["bytes"] == container["bytes"]]
            if len(official_aliases) != 1 or audit.read(official_aliases[0]) != parent:
                raise ValueError("Current WindowStorage ASAR official byte identity is unresolved")
            official = official_aliases[0]
            packages = [item for item in host_receipt["files"] if item.get("path") == official.get("container")
                        and item.get("sha256") == official.get("container_sha256")
                        and item.get("manifest_identity_verified")]
            if len(packages) != 1:
                raise ValueError("Current WindowStorage ASAR package provenance is unresolved")
            audit.artifact(a.ROOT / packages[0]["path"], packages[0])
            report["complete_host_sources"].append({"original": row, "container": container,
                "official_container": official, "official_package": packages[0],
                "acquisition": families.identity(host_receipt_path),
                "container_bytes_verified": True, "body": body.decode("utf-8")})
            if suffix.endswith("window_storage/index.js"):
                audit.retain("host_complete_windowstorage_owner", row, 'this.windowStorageMap=new Map', {"class", "class_declaration"})
                audit.retain("host_clear_windowstorage_actual_branch", row, 'case"clearWindowStorage"', {"switch_case"})
            else:
                audit.retain("host_complete_tab_lifecycle", row, 'initEventHandler(){', {"class", "class_declaration"})
                audit.retain("host_tab_event_lifetime_and_storage_cleanup", row, 'initEventHandler(){', {"method_definition"})

        native_receipt_path = a.ROOT / ".analysis/inputs/official-native-current/acquisition.json"
        native_receipt = json.loads(audit.artifact(native_receipt_path))
        # Acquisition may reuse an equal-byte retained original in another
        # input namespace. The current manifest/resource relation selects the
        # evidence; a workstation's chosen copy directory cannot select it.
        acquired = [row for row in native_receipt["resources"] if row.get("status") == "verified"
                    and row.get("path", "").endswith("/RzAudioUtil_v1.0.3.1.dll")
                    and any(manifest["url"] == "https://apps.razer.com/synapse/products/1422/mw/manifest.json"
                            and manifest.get("resource_name") == "RzAudioUtil_PID058E"
                            for manifest in row.get("manifests", []))]
        if len(acquired) != 1:
            raise ValueError("Current AudioRouter native resource declaration is absent or ambiguous")
        native_input = originals.get(acquired[0]["path"])
        if (native_input is None or not native_receipt.get("source_current_during_acquisition")
                or any(acquired[0].get(key) != native_input[key] for key in ("bytes", "sha256"))
                or acquired[0].get("declared_sha256") != native_input["sha256"]
                or acquired[0].get("declared_bytes") != native_input["bytes"]):
            raise ValueError("Current AudioRouter native acquisition identity is unresolved")
        native_manifests = [row for row in acquired[0].get("manifests", [])
                            if row["url"] == "https://apps.razer.com/synapse/products/1422/mw/manifest.json"]
        if len(native_manifests) != 1:
            raise ValueError("PID1422 current AudioRouter resource declaration is unresolved")
        audit.artifact(a.ROOT / native_manifests[0]["path"], native_manifests[0])
        report["native_audio_route_status"] = {
            "acquisition": families.identity(native_receipt_path), "resource": acquired[0],
            "current_manifest": native_manifests[0],
            **audit.native_slice(native_input, ["Dispatch", "dispatchAudioRouter"], depth=0,
                rvas=(0x19760, 0xc130, 0xcde0, 0xd1b0),
                regions=({"role": "route_device_handler_vtable", "rva": 0x995c0, "bytes": 48},))}
        report["native_global_shortcut_clock"] = retain_global_shortcut_clock(audit)
        report["native_mapping_global_mode"] = retain_mapping_global_mode(audit)
        report["native_mapping_suppression"] = retain_mapping_suppression(audit)
        report["native_mapping_execution_dependencies"] = retain_mapping_execution_dependencies(audit)
        report["native_macro_input"] = retain_mapping_macro_input(audit)
        shortcut_host = verified_host_owners(audit, ("electron/modules/mapping_engine/win/index.js",))
        report["complete_host_sources"].extend(shortcut_host.values())
        shortcut_original = shortcut_host["electron/modules/mapping_engine/win/index.js"]["original"]
        audit.retain("host_complete_globalshortcut_callback_and_lifetime", shortcut_original,
                     'createCbGlobalShortcutEvent', {"class", "class_declaration"}, unique=False)
        report["otfm_product_triggers"] = retain_otfm_product_triggers(audit)
        report["otfm_eligibility"] = retain_otfm_eligibility(audit, report["otfm_product_triggers"])
        report["semantics"] = {
            "storage_key": "RzWinStorage_macro_recorder",
            "software_set": {"state": "disablingMapping", "value": {"mode": "exclusive", "payload": {"macro_recorder_mode": "kSoftware"}},
                "ordering": "setWindowStorageItem is invoked without awaiting; Promise.all([disableMapping()]) is returned. Both onDone and onError enter checkingRunningMacros."},
            "software_remove": "stoppingRecorder invokes removeWindowStorageItem(no) without awaiting, then returns dr(). dr checks hasMacroRecordingStarted; a separate pr guard protects stopMacroRecording and is reset in finally. The native stop event drives the next state.",
            "software_native_stop_and_postrecording": "Ar clears the source UI flags, dispatches its completed notification and sends only ON_RECORDING_STOPPED. The recording state's native-stop transition enters monitor query/postRecording directly, bypassing stoppingRecorder and its remove. Nr processes recorded events and monitor/stopBy adjustments, then dispatches h.nu; the complete reducer's callback sends ON_MACRO_SAVED. Neither complete owner sends ON_STOP_RECORDING or removes the recorder storage item.",
            "software_escape_and_abort": "The current xstate@4.38.3 formatTransitions/getCandidates preserve transition order and next stops at the first matching candidate. ON_MACRO_ITEM's first candidate is unconditional recordingHandler, so its later keyup/vkCode27 abort candidate is shadowed. abortingRecorder dispatches literal aborted but has no outgoing transition; stoppingRecorderAfterAborted has no inbound transition. The verified current Macro deployment has no aborted event listener; the retained literal occurrences distinguish event names from AbortSignal.aborted and diagnostic status values.",
            "software_explicit_stop_and_unload": "The complete br globalshortcut and xa UI stop owners send ON_STOP_RECORDING. xa retains countdown timer cancellation and hook cleanup. window.onbeforeunload invokes only dr(), without recorder WindowStorage removal. No native-stop/abort-to-explicit-stop bridge is inferred.",
            "host_tab_storage_cleanup": "The current official-byte-identical Tab class invokes clearWindowStorage on destroyed and render-process-gone using a new sender object whose getURL method returns this.url. That method's this is the new sender, which has no url property; it does not capture the Tab's this.url. WindowStorage clear only deletes the map entry selected by sender.getURL(), and broadcasts no removed/cleared event. These complete owners therefore do not establish removal or notification for the actual Tab URL.",
            "software_start": "Button trigger invokes startMacroRecording(kSoftware) then waits for RECORDING. The hotkey branch invokes the same mode on RECORDING. Starting alone does not write the exclusive storage entry until disablingMapping is entered.",
            "macro_startup": "hr creates the machine, migrates/reads local macros, stops and unregisters an existing recorder if running, initializes local state, then Er reads the first WindowStorage owner value and writes that exact value under this owner before subscribing recorder events.",
            "macro_otfm_event": "Sr tests mode against kOTFM. Other modes initialize local recorder state and send RECORDING; kOTFM sends OTFM. The shared machine moves to startedByOther; this source owner does not fabricate a software-recording lock for OTFM.",
            "otfm_current_owner": "The verified current background-manager loader imports otfm-d56d4f50.js export ad, which is To, and invokes initOTFM. init creates the OTFM state machine, installs local completion/abortion/persistence handlers and subscribes At start-otfm to co. The identity broadcast portion of this source is excluded from the local product scope.",
            "keyboard_otfm_pretrigger": "Current PID756 module 26568, OTFM/ON_OTFM branch, first invokes setWindowStorageItem with the same recorder key and an OBJECT {macro_recorder_mode:kOTFM,deviceId:usb_5426_PRODUCT_CONTAINER,containerId:CONTAINER}. It has no mode:exclusive. It then executes the event task, posting eventType:start-otfm with containerId/productId/layoutId/deviceId/macro_recorder_mode through pt's global_broadcast_channel. This preliminary record is distinct from the background's later stringified exclusive payload.",
            "otfm_set_and_global_lock": "co calls lo before awaiting po. lo invokes setWindowStorageItem(RzWinStorage_macro_recorder, JSON.stringify({payload:{macro_recorder_mode:kOTFM},mode:exclusive})) without awaiting, then records containerId/productId/layoutId and resets Qt. It catches only synchronous exceptions. po requests RzGlobalLock_macro_recorder with steal:true, retaining the pending lock Promise's resolve as Jt. No failure rollback is added to co.",
            "otfm_start_order": "After acquiring the lock, co starts recorder listeners, awaits local device enumeration, selects the default mouse-move type, registers the input device, starts system listeners, reads brightness and sends ON_OTFM_PRESS. go defines other local-storage choices but actually calls only otfmDefault; those unused getters are not fabricated reads. The machine queries an already-running recorder, disables mapping/global shortcuts, obtains monitor info and only then starts kOTFM. Native result.result determines whether to proceed with LEDs or error lighting.",
            "otfm_remove": "ko invokes Jt if present before removeWindowStorageItem(gt); remove is not awaited and this owner does not reset Jt. The completed handler sets Zt=false and eo=true, calls ko when its local pending array is empty, then stops system/recorder listeners. The aborted handler calls ko only when eo is already true. The complete Ro owner retains the pending work and local persistence transitions.",
            "otfm_abort_and_finish": "The complete machine preserves escape/system/unplug/timeout branches. Mouse-mode-selection Escape only skips that selection. Ro checks geometry after numpad normalization and before key-release cleanup; a geometry rejection restores lighting/error/enables/completes without stopMacroRecording. Accepted save stops only after ON_MACRO_SAVED. Completed cleanup is not fully awaited.",
            "initial_consumer": "An clears read-held counts, scans every returned owner value for mode===exclusive, sets macroRecorderExclusiveHeld and dispatches held_lock_updated. It reads macro_recorder_mode from the parsed value TOP LEVEL, then enqueues ON_OTFM_STARTED_BY_OTHER or ON_MACRO_RECORDER_STARTED only for its matching value. A parse/query failure clears the held flag.",
            "event_consumer": "q is gated by Z. It parses newValue/oldValue separately with empty-object fallback. Any new exclusive value sets macroRecorderExclusiveHeld. It reads payload.macro_recorder_mode, queues OTFM brightness/start or software start, and dispatches held_lock_updated.",
            "event_removal": "Only removed/cleared plus an old exclusive value triggers an asynchronous re-read of all owners. A remaining exclusive owner keeps the held flag. Re-read/parse rejection clears it. finally queues the old payload mode's stop events only when no exclusive owner remains, and always dispatches held_lock_updated.",
            "field_difference": "The current initial scanner uses value.macro_recorder_mode; the software producer and live consumer use value.payload.macro_recorder_mode. Both complete source owners are retained; they are not silently normalized.",
            "mouse_mode_state_execution": "The complete PID192 machine moves macroRecorder idle/macroRecorderStarted and OTFM idle/otfmStarted, emitting descriptors without deviceAccess actions. The actual ze decision listener has no macroRecorder/OTFM cases and falls back to the retained static/generic registries. The current default injector/runtime/bootstrap owners activate no genericFeature config for these events; no mapping or brightness write is inferred from mode state changes.",
            "mouse_brightness_start_execution": "ze handles ON_BRIGHTNESS_MACRO_RECORDING_START and ON_BRIGHTNESS_OTFM_START through 25947.lk_ (ct). Its complete body only registers or unregisters display-power callbacks according to the descriptor flag, then unregisters system-idle callbacks; it ignores the deviceAccess argument and performs no brightness or mapping write. The complete 64707 export owners retain subscription bookkeeping and source cleanup.",
            "mouse_brightness_source_guards": "The OTFM START transition from enabled/disabled requires brightnessOTFMGuard, which compares literal OTFMStarted while the actual OTFM target is otfmStarted. The q consumer queues brightness START before mode START. macroRecorderUnknown's START has no such guard. The original state and ordering are retained without case normalization or invented transitions.",
            "mouse_brightness_stop_descriptor_without_executor": "M's STOP descriptors list setBrightnessOn/Off and optional handler restoration, and R preserves the original eventType. However ze's brightness event switch has no ON_BRIGHTNESS_OTFM_STOP, ON_BRIGHTNESS_MACRO_RECORDING_STOP or connection-error case, and no default. With no registered generic override it neither executes those listed writes nor emits task feedback/removes the current event. Descriptor presence is not device write-back evidence.",
            "event_lifetime": "j registers the WindowStorage q listener and memory initialization; J sets Z=true. Y removes q and its recorded memory listener, and replaces onstorage with an empty function. This records actual source cleanup rather than inventing a getter refresh.",
            "receiver_active_feature": "PID192's 95414 derives defaults from 11579.Z through 88443.G and the absent 11579.injectorDefaults export. Neither this complete factory, the default runtime, derived defaults nor boot feature mutations enable canPairMultipleDevices. V therefore selects the false branch in this current boot configuration.",
            "receiver_plain_record": "he obtains connectedInfo through JSON.parse(connectedDeviceInfo). Its V=true path calls Ie, which also selects a plain JSON record by productId and serialNumber. Neither path attaches getHWModule. The conditional hardware getter in _e cannot be invented for these records.",
            "receiver_serial_selection": "Ie finds the first De() device matching productId, computes X(device), requires Ue.exists(key), reads Ue.getSerialNumber(key), then finds the first cached JSON record whose productId and serialNumber strictly equal those values.",
            "runtime_layout_lookup": "20236.oP reads DEVICE_RUNTIME_DATA (literal connectedDevices) WindowStorage [{value:string}], JSON-parses every value, and reduces using strict deviceContainerId/productId equality. Each match replaces the accumulator, so the LAST matching value wins. It returns only sidePanelName/sidepadLayout.",
            "receiver_layout_order": "_e first checks mouse category, then container plus actual productId runtime data, then container plus dongleId. Only truthy sidepadLayout returns. If an actual object has a getHWModule function, 96204.v5 is awaited; otherwise layoutId||0 is returned. Any lookup/parse failure reaches catch and returns layoutId||0.",
            "audio_route_boolean": "PID1422 routeExternalDevice awaits audioUtil.dispatch(AudioRouter,RouteDevice,{primaryDevice,routedDevice}) without an output object. dispatch returns true only for a string containing JSON with top-level status===success; status error, unknown/non-string replies, parse failure and uninitialized dispatch return false. response content or truthiness does not determine this boolean.",
            "audio_native_status_envelope": "The current declared RzAudioUtil input's Dispatch RVA0x1af70 calls the cmdClass handler, sets top-level status to success when the returned signed HRESULT is nonnegative, otherwise error, serializes the envelope and returns an allocated UTF-8 string. dispatchAudioRouter RVA0xae90 registers RouteDevice's lambda at vtable RVA0x995c0; its invoke RVA0x19760 forwards to 0xc130, then 0xcde0. SetRouterState starts with S_OK, removes an old route, returns S_OK for an empty routed name, otherwise propagates init/start 0xd1b0 HRESULT. The RouteDevice handler adds only an error on failure and need not add a response field for success.",
            "audio_route_consumers": "changePlaybackMixDevice applies setPlaybackExternal only if routing returns true, then returns the routing boolean even if the later native setter returns false; its exception path returns false. updateMixInput combines routing and native results according to each add/remove/external branch, preserving the source ordering and existing writes rather than treating service request completion as route success.",
        }
        report["remaining_chains"] = [
            "The 114 current entry producers, complete background participant table and 25/37 identity/guard/save differences are retained separately. Continue remaining product mapping writers, native transport/parser branches, local profile persistence and failure/cleanup semantics; these static subsets do not mean all products are independently implemented.",
            "Finish native recorder callback ownership, ABI and device write-back chains through the full current captures; complete frontend/host WindowStorage owners and the bounded AudioRouter status slice do not establish native semantic closure or runtime acceptance.",
            "Complete the mounted macro UI, dialogs, local macro persistence/mappings, cancellation and cleanup against its entire current source-to-Rust/backend checklist; selected complete owners do not mean a complete page."]
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        report["failures"].append(str(error))
    report["complete_originals"] = list(audit.sources.values())
    report["complete_source_owners"] = list(audit.owners.values())
    report["verified_artifacts"] = list(audit.artifacts.values())
    report["changed_during_query"] = audit.final_snapshot()
    report["snapshot_stable"] = not report["changed_during_query"]
    report["failures"].extend("Audit artifact changed during query: " + name for name in report["changed_during_query"])
    destination = a.ROOT / ".analysis/results/host-current/local-audit.json"
    with a.index_lock(destination.with_suffix(".lock")):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous[args.query.replace("-", "_")] = report
        a.save(destination, previous)
    print(json.dumps({"receipt": a.rel(destination), "query": args.query,
                      "complete_source_owners": len(audit.owners), "failures": report["failures"]}), flush=True)
    if report["failures"]:
        raise ValueError("Local macro recorder source query remains unresolved")


def audit_host_screen_color(args):
    import frontend
    audit = HostLocalAudit()
    a, families = audit.a, audit.families
    report = {"recorded_at_utc": a.stamp(), "tool": families.identity(Path(__file__)),
              "parser": audit.parser_versions, "source_sets": [], "failures": [],
              "source_chain_complete": False, "product_code_modified": False,
              "target_execution": False, "real_device_operations": False, "runtime_acceptance": "unverified"}
    try:
        for name in ("tools/host.py", "tools/evidence.py", "tools/families.py", "tools/analysis.py", "tools/frontend.py"):
            audit.artifact(a.ROOT / name)
        mouse_set, mouse_files = audit.source_set("mouse-70-current")
        studio_selection = frontend.current_source_selection("https://apps.razer.com/synapse/chroma-studio/", audit,
            candidates=[".analysis/inputs/frontend/" + name + "/acquisition.json" for name in (
                "synapse-chroma-studio-current", "synapse-chroma-studio-build-2609080829-current")])
        studio_files = studio_selection["originals"]
        report["source_sets"] = [mouse_set, {"name": studio_selection["name"],
            "acquisition": studio_selection["acquisition"], "deployment": studio_selection["deployment"]}]
        report["studio_source_selection"] = {key: value for key, value in studio_selection.items()
            if key not in {"originals", "capture", "deployment"}}
        report["reused_complete_captures"] = [audit.frontend_capture("mouse-70-current", mouse_files)]
        if studio_selection["capture"] is not None:
            report["reused_complete_captures"].append(studio_selection["capture"])

        def source(files, needle):
            matches = [row for row in files if Path(row["path"]).suffix.lower() in {".js", ".mjs"} and needle.encode() in audit.read(row)]
            if len(matches) != 1:
                raise ValueError("Screen color source owner is absent or ambiguous: " + needle)
            return matches[0]

        ordinary = source(mouse_files, 'this.onColorPickerDoneHandler=()=>{console.log("onColorPickerDone")}')
        studio = source(studio_files, 'case"onColorPickerDone":me(W.current)')
        studio_native = source(studio_files, 'this.channelName="chromaStudioNativeAction"')
        for role, original, needle, kinds in (
            ("ordinary_complete_color_picker", ordinary, 'this.onColorPickerHandler=', {"class", "class_declaration"}),
            ("ordinary_show_with_registration_gate", ordinary, 'this.showColorPicker=async()=>', {"assignment_expression"}),
            ("ordinary_close_with_registration_gate", ordinary, 'this.closeColorPicker=async()=>', {"assignment_expression"}),
            ("ordinary_native_color_dispatch", ordinary, 's.response("onColorPicker",e=>', {"call_expression"}),
            ("ordinary_native_done_dispatch", ordinary, 's.response("onColorPickerDone",()=>', {"call_expression"}),
            ("ordinary_shared_event_lifetime", ordinary, 'const iO=_O,oO="onColorPicker"', {"lexical_declaration"}),
            ("studio_complete_color_picker", studio, 'function y(e){let{canvasWidth:t,canvasHeight:n,mixedValues:o', {"function_declaration"}),
            ("studio_native_complete_owner", studio_native, 'async showColorPicker(){', {"class", "class_declaration"}),
            ("studio_native_show", studio_native, 'async showColorPicker(){', {"method_definition"}),
            ("studio_native_close", studio_native, 'async closeColorPicker(){', {"method_definition"}),
            ("studio_native_callback_done_close_before_emit", studio_native, 'this.handleEvent=e=>', {"assignment_expression"}),
            ("studio_native_terminate", studio_native, 'async terminate(){', {"method_definition"}),
        ):
            audit.retain(role, original, needle, kinds)
        host = verified_host_owners(audit, ("electron/modules/sysutil/win/index.js", "electron/main.js"))
        report["complete_host_sources"] = list(host.values())
        sysutil = host["electron/modules/sysutil/win/index.js"]["original"]
        main = host["electron/main.js"]["original"]
        for role, original, needle, kinds in (
            ("host_picker_ffi_declarations_and_callback_lifetime", sysutil, 'ShowColorPicker:["bool",[]]', {"class", "class_declaration"}),
            ("host_picker_command_dispatch", sysutil, 'callDLL=async', {"field_definition"}),
            ("host_picker_callback_to_living_webcontents", sysutil, 'handleFFIEvent=async', {"field_definition"}),
            ("host_shared_sysutils_dispatch_caller", main, 'case"ShowColorPicker":', {"switch_statement"}),
        ):
            audit.retain(role, original, needle, kinds)

        sysutil_native, host_provenance = verified_host_native(audit, "CommonDLL/SysUtilsNative.dll")
        native_receipt_path = a.ROOT / ".analysis/inputs/official-native-current/acquisition.json"
        native_receipt = json.loads(audit.artifact(native_receipt_path))
        candidates = [row for row in native_receipt["resources"] if row.get("status") == "verified"
                      and any(item.get("resource_name") == "ChromaStudioNativeDLL" for item in row.get("manifests", []))]
        if len(candidates) != 1:
            raise ValueError("Current declared Studio native picker is absent or ambiguous")
        studio_input = candidates[0]
        audit.read(studio_input)
        manifests = [item for item in studio_input["manifests"] if item.get("resource_name") == "ChromaStudioNativeDLL"]
        for manifest in manifests:
            audit.artifact(a.ROOT / manifest["path"], manifest)
        studio_provenance = {"original": studio_input, "acquisition": families.identity(native_receipt_path),
                             "current_manifests": manifests}
        roles = {
            "host": {0x73180: "show", 0x73330: "color_callback", 0x735a0: "done_callback",
                     0x737c0: "close_impl", 0x73850: "close_export", 0xa9e0: "create_callbacks_and_thread",
                     0xabd0: "close_wait_join", 0x7b830: "destructor", 0x9f30: "window_procedure",
                     0xa220: "window_and_snapshot_loop", 0xae10: "initial_and_button_down_sample",
                     0x87490: "window_base"},
            "studio": {0x2cd0: "show", 0x2e60: "color_callback", 0x30f0: "done_callback",
                       0x3360: "close", 0x5a20: "destructor", 0x8150: "window_procedure",
                       0x83d0: "window_and_snapshot_loop", 0x8b50: "create_callbacks_and_thread",
                       0x8ca0: "close_send_message_and_join", 0x8dc0: "initial_and_button_down_sample",
                       0x3a3b0: "window_base"},
        }
        report["native"] = []
        for name, original, provenance in (("host", sysutil_native, host_provenance),
                                            ("studio", studio_input, studio_provenance)):
            verified = families.verify_native_functions(original, roles[name])
            # Full bodies are retained once in native_slice; the verification
            # metadata binds every selected role to actual file-backed chunks.
            for function in verified["complete_selected_functions"]:
                function.pop("capture")
            regions = ({"role": "window_background_brush_constant", "rva": 0xe2c60, "bytes": 16},) if name == "host" else ()
            native = audit.native_slice(original, ["ShowColorPicker", "CloseColorPicker"], depth=0,
                                       rvas=tuple(roles[name]), regions=regions)
            if native["failures"] or native["decompiler_failures"]:
                raise ValueError("Selected native screen picker functions are incomplete: " + name)
            report["native"].append({"owner": name, **provenance, "verification": verified, "slice": native})

        cursor_path = a.ROOT / ".analysis/inputs/native-resources/acquisition.json"
        cursor_receipt = json.loads(audit.artifact(cursor_path))
        cursors = []
        for original in (sysutil_native, studio_input):
            groups = pe_cursor_groups(audit.read(original), 102)
            if len(groups) != 1:
                raise ValueError("Current picker cursor language is ambiguous")
            group = groups[0]
            matches = [row for row in cursor_receipt["derivations"]
                       if row["original"]["sha256"] == original["sha256"] and row["group"] == group["group"]]
            if len(matches) != 1 or matches[0]["frames"] != group["frames"]:
                raise ValueError("Current picker cursor derivation differs from actual PE bytes")
            output = matches[0]["output"]
            if audit.read(output) != group["data"]:
                raise ValueError("Delivered picker cursor differs from the source PE frames")
            cursors.append({"input": original, "group": group["group"], "frames": group["frames"], "output": output})
        if cursors[0]["output"]["sha256"] != cursors[1]["output"]["sha256"]:
            raise ValueError("Host and Studio picker cursor resources differ")
        report["cursor"] = {"acquisition": families.identity(cursor_path), "derivations": cursors,
                            "host_and_studio_frames_byte_identical": True}
        report["semantics"] = {
            "sampling": "Both current native owners create a layered/tool/topmost popup with alpha 1, capture the virtual desktop into a compatible memory DC, then sample that fixed snapshot with GetPixel; mouse moves do not capture a new live desktop.",
            "coordinates": "The initial WM_START_COLOR_PICKER calls GetCursorPos and passes global coordinates directly to the snapshot sample without subtracting virtual left/top. Later WM_MOUSEMOVE/WM_LBUTTONDOWN use signed 16-bit client coordinates. The source coordinate difference remains explicit; CLR_INVALID must not be reported as an observed white pixel.",
            "color": "Native COLORREF bytes are converted to RGB24 by swapping the red and blue positions. The custom source CUR group is 102; all hotspots, DIB pixels and AND masks are preserved.",
            "stacking": "Initial HandleLButtonDown selects the custom cursor, publishes one color and calls SetWindowPos HWND_NOTOPMOST. WM_MOUSEMOVE and WM_KILLFOCUS select the same cursor and restore HWND_TOPMOST with NOMOVE|NOSIZE.",
            "ordinary_picker": "Color calls onChangeHex and updates local selectedColor/fields/markers only. Done only logs. Only saveColor invokes the parent save callback; cancelColor invokes the parent cancel callback. Unmount removes mouse and picker event listeners.",
            "ordinary_wrapper": "show sets isRegisterColorPicker before invoking ShowColorPicker; close clears it before invoking CloseColorPicker. Callback delivery is gated by that flag; Done is emitted before the gate is cleared. The retained complete ordinary picker does not call closeColorPicker.",
            "studio_picker": "Color calls ae for onChange and local RGB/HSV/field preview. Done calls me(W.current), producing onAfterChange with rgbData and numeric color. The enclosing owner retains native event subscription and cleanup.",
            "studio_wrapper": "The native Done callback is deferred, then closeColorPicker is awaited before emitting to UI. show sets isOnColorPicker before its native request; close clears it first. Termination closes an existing region window, releases FFI and clears native initialization state.",
            "native_lifetime": "SysUtils Done posts WM_USER+0x67 before publishing its event. Studio Done publishes its event and relies on the JS close/send-message/join chain. Replacement destroys and joins the previous owner before creating another. The Rust adapter closes its worker-owned window/DC/bitmap/cursor and restores thread DPI/cursor state before emitting Done or Canceled.",
        }
        report["remaining_chains"] = [
            "Audit every product/version/accessory mounted color-picker caller and parent save/cancel/device write-back branch. The representative complete ordinary picker and Studio owner do not establish all-product or all-page closure.",
            "Resolve the retained native indirect/import and unexpanded helper edges where they change local behavior; unrelated whole-module decompiler failures are retained separately from the selected picker functions.",
            "Real window, multi-monitor/DPI, cancellation and device/service acceptance are unverified under the current execution limits."]
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        report["failures"].append(str(error))
    report["complete_originals"] = list(audit.sources.values())
    report["complete_source_owners"] = list(audit.owners.values())
    report["verified_artifacts"] = list(audit.artifacts.values())
    report["changed_during_query"] = audit.final_snapshot()
    report["snapshot_stable"] = not report["changed_during_query"]
    report["failures"].extend("Audit artifact changed during query: " + name for name in report["changed_during_query"])
    destination = a.ROOT / ".analysis/results/host-current/local-audit.json"
    publish_host_static_query(report, destination, a, "screen_color")
    print(json.dumps({"receipt": a.rel(destination), "query": args.query,
                      "complete_source_owners": len(audit.owners), "failures": report["failures"]}), flush=True)
    if report["failures"]:
        raise ValueError("Local screen color source query remains unresolved")


def help_system_mount_layout(props):
    """Classify the actual Help JSX props, rejecting dynamic/duplicate flags."""
    if props.type != "object":
        raise ValueError("System Help mount props are not a static object")
    flags = {}
    for pair in props.named_children:
        if pair.type != "pair":
            raise ValueError("System Help mount props require spread/member analysis")
        key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
        name = key.text.decode("utf-8")
        if key.type == "string":
            name = name[1:-1]
        if name not in {"hasSystemInfo", "hasSystemInfoImage"}:
            continue
        if name in flags:
            raise ValueError("System Help mount has a duplicate capability flag")
        if value.text in {b"true", b"!0"}:
            flags[name] = True
        elif value.text in {b"false", b"!1"}:
            flags[name] = False
        else:
            raise ValueError("System Help mount has a dynamic capability flag")
    if flags.get("hasSystemInfo") is not True:
        raise ValueError("System Help mount does not enable system information")
    return "image" if flags.get("hasSystemInfoImage", False) else "basic"


def help_system_native_declaration(manifest, original):
    """Bind the selected BladeDLL to its current manifest declaration."""
    entries = manifest["resourceManifest"]["resources"]
    matches = [row for row in entries if row.get("resourceName") == "BladeDLL"]
    if len(matches) != 1:
        raise ValueError("Current system manifest BladeDLL is absent or ambiguous")
    row = matches[0]
    if (not isinstance(row.get("resourceVersion"), str) or not row["resourceVersion"]
            or not isinstance(row.get("size"), int) or isinstance(row["size"], bool)
            or row["size"] != original["bytes"]
            or not isinstance(row.get("sha256"), str)
            or not re.fullmatch(r"[0-9a-fA-F]{64}", row["sha256"])
            or row["sha256"].lower() != original["sha256"]
            or row.get("url") != original["url"]):
        raise ValueError("Current system manifest BladeDLL differs from acquired bytes")
    return row


def help_system_report_constants(raw, native):
    """Retain referenced data bytes, not just names from a decompiler listing."""
    import uuid
    pe = inspect_pe(raw)
    base = int(pe["image_base"], 16)
    property_guid = uuid.UUID("a45c254e-df1c-4efd-8020-67d146a850e0").bytes_le
    expected = {
        "memory_scale": struct.pack("<d", 1.0 / 1024.0),
        "hardware_ids": property_guid + struct.pack("<I", 3),
        "device_driver": property_guid + struct.pack("<I", 11),
        "device_manufacturer": property_guid + struct.pack("<I", 13),
        "display_adapter_interface": uuid.UUID("5b45201d-f2f2-4f3b-85bb-30ff1f953599").bytes_le,
        "monitor_interface": uuid.UUID("e6f07b5f-ee97-4a90-b076-33f57bf4eaa7").bytes_le,
    }
    retained = {name: [] for name in expected}
    for function in native["complete_functions"]:
        for instruction in function["capture"]["assembly"]:
            for reference in instruction.get("xrefs_from", []):
                if reference.get("code"):
                    continue
                rva = int(reference["to"], 16) - base
                for name, value in expected.items():
                    section = next((part for part in pe["sections"] if part["rva"] <= rva
                        and rva + len(value) <= part["rva"] + part["bytes"]), None)
                    if section is None:
                        continue
                    offset = section["file_offset"] + rva - section["rva"]
                    actual = raw[offset:offset + len(value)]
                    if actual == value:
                        retained[name].append({"rva": hex(rva), "raw_hex": actual.hex(),
                            "sha256": hashlib.sha256(actual).hexdigest(),
                            "function": function["artifact"], "instruction": instruction})
    missing = [name for name, references in retained.items() if not references]
    if missing:
        raise ValueError("Report constants lack current instruction/data evidence: " + ", ".join(missing))
    return retained


def audit_host_help_system_info(args):
    audit = HostLocalAudit()
    a, families = audit.a, audit.families
    products = (694, 695, 696, 709, 710, 711, 736, 737)
    report = {"recorded_at_utc": a.stamp(), "tool": families.identity(Path(__file__)),
              "parser": audit.parser_versions, "source_sets": [], "products": [],
              "native": [], "failures": [], "reused_complete_captures": [],
              "source_chain_complete": False, "query_writes_product_code": False,
              "target_execution": False, "real_device_operations": False,
              "runtime_acceptance": "unverified"}

    def source(files, needle):
        matches = [row for row in files if Path(row["path"]).suffix == ".js"
                   and needle.encode("utf-8") in audit.read(row)]
        if len(matches) != 1:
            raise ValueError("System Help source owner is absent or ambiguous: " + needle)
        return matches[0]

    def enclosing(role, original, needle, predicate, *, unique=True):
        raw = audit.read(original)
        hits = list(re.finditer(re.escape(needle.encode("utf-8")), raw))
        if not hits or unique and len(hits) != 1:
            raise ValueError("System Help source anchor is absent or ambiguous: " + role)
        result = []
        for hit in hits:
            node = audit.tree(original).root_node.descendant_for_byte_range(hit.start(), hit.end() - 1)
            while node is not None and not predicate(node):
                node = node.parent
            if node is None:
                raise ValueError("System Help complete lexical owner is unavailable: " + role)
            key = audit.retain_range(role, original, node.start_byte, node.end_byte, node.type)
            if key not in result:
                result.append(key)
        return result

    def callable_owner(node):
        if node.type in {"function_declaration", "method_definition"}:
            return True
        value = node.child_by_field_name("value") if node.type == "variable_declarator" else None
        return value is not None and value.type in {"arrow_function", "function_expression"}

    try:
        for name in ("tools/host.py", "tools/evidence.py", "tools/families.py", "tools/analysis.py", "tools/workspace.py"):
            audit.artifact(a.ROOT / name)
        middleware = {}
        for product in products:
            deployment = {}
            for name, kind in ((f"system-{product}-current", "ui"), (f"system-mw-{product}-current", "mw")):
                receipt, files = audit.source_set(name)
                report["source_sets"].append(receipt)
                report["reused_complete_captures"].append(audit.frontend_capture(name, files))
                deployment[kind] = files
                if kind == "mw":
                    middleware[product] = json.loads(audit.artifact(a.ROOT / receipt["acquisition"]["path"]))["files"]
            ui, mw = deployment["ui"], deployment["mw"]
            page = source(ui, ".convertSystemInfoData(this.props.systemInfo)")
            state = source(ui, "systemInfoReducer:function")
            flow = source(mw, "taskRunnerStoreSystemDetailsData SUCCESS")
            wrapper = source(mw, 'GetAllBladeInfo:["pointer",[]]')
            row = {"product_id": product, "owners": {}, "exports": []}
            owners = row["owners"]
            owners["basic_component"] = audit.retain("system_help_basic_component", page,
                ".convertSystemInfoData(this.props.systemInfo)", {"class", "class_declaration"})
            owners["help_parent_and_branch_gates"] = audit.retain("system_help_parent_and_branch_gates", page,
                "this.props.hasSystemInfo", {"class", "class_declaration"}, unique=False)
            owners["native_observation_reducer_and_save_dispatch"] = audit.retain("system_help_reducer", state,
                "systemInfoReducer:function", {"pair"})
            owners["display_getter"] = enclosing("system_help_display_getter", flow,
                "e.getBladeDisplayData()", callable_owner)
            owners["display_initialization_and_observation"] = enclosing("system_help_display_initialization_and_observation", flow,
                "hasSystemDetails", callable_owner, unique=False)
            owners["full_save_runner_and_completion"] = audit.retain("system_help_full_save_runner_and_completion", flow,
                "taskRunnerStoreSystemDetailsData SUCCESS", {"function_declaration"}, unique=False)
            owners["full_save_task_maker"] = enclosing("system_help_full_save_task_maker", flow,
                'actionType:"taskMakerStoreSystemDetailsData"', callable_owner)
            owners["full_save_event_and_state_machine"] = audit.retain("system_help_full_save_event_and_state_machine", flow,
                "ON_STORE_SYSTEM_INFO_DATA", {"switch_case", "pair"}, unique=False)
            owners["full_save_state_machine_action"] = audit.retain("system_help_full_save_state_machine_action", flow,
                "systemInfoAction:", {"pair"})
            owners["native_complete_wrapper"] = audit.retain("system_help_native_complete_wrapper", wrapper,
                'GetAllBladeInfo:["pointer",[]]', {"class", "class_declaration"})
            native_owner = audit.owners[owners["native_complete_wrapper"][0]]
            native_class = audit.tree(wrapper).root_node.descendant_for_byte_range(
                native_owner["byte_start"], native_owner["byte_end"] - 1)
            for method in ("init", "terminate", "getBladeDisplayData", "getAllBladeInfo"):
                _, owner = audit.callable_member("system_help_native_" + method, wrapper, native_class, method)
                owners["native_" + method] = [owner]
            if b"NVIDIANvApp:" in audit.read(page):
                owners["image_component"] = enclosing("system_help_image_component", page, "NVIDIANvApp:",
                    lambda node: callable_owner(node) and b"systemInfoReducer.systemInfo" in node.text)
            # The JSX call's actual props establish which retained component is mounted.
            raw = audit.read(page)
            hits = list(re.finditer(rb"(?<![\w$])hasSystemInfo:", raw))
            if len(hits) != 1:
                raise ValueError("System Help actual mounted capability props are absent or ambiguous")
            hit = hits[0]
            props = audit.tree(page).root_node.descendant_for_byte_range(hit.start(), hit.end() - 1)
            while props is not None and props.type != "object":
                props = props.parent
            if props is None or props.parent.type != "arguments":
                raise ValueError("System Help capability props are not passed to an actual component")
            arguments = props.parent.named_children
            if len(arguments) != 2 or arguments[1] != props or arguments[0].type != "identifier":
                raise ValueError("System Help mounted component identity requires further analysis")
            row["layout"] = help_system_mount_layout(props)
            row["mounted_component"] = audit.binding("system_help_mounted_component", page,
                arguments[0].text.decode("utf-8"), arguments[0].start_byte)
            if row["mounted_component"]["resolution"] != "declaration":
                raise ValueError("System Help mounted component has no unique lexical owner")
            owners["mounted_product_page"] = enclosing("system_help_mounted_product_page", page,
                "hasSystemInfo:", lambda node: node.type in {"class", "class_declaration"})
            owners["mounted_props"] = [audit.retain_range("system_help_mounted_props", page,
                props.start_byte, props.end_byte, props.type)]
            if row["layout"] == "image" and "image_component" not in owners:
                raise ValueError("System Help mounted image component is unavailable")
            # Exact factory/export resolution preserves the inherited FFI route.
            for module_id, export in ((34938, "h3"), (77127, "default"), (19635, "oE"), (19635, "yL"), (19635, "I3")):
                row["exports"].append(audit.webpack_export("system_help_ffi_or_display_export", mw, module_id, export))
            report["products"].append(row)

        host = verified_host_owners(audit, ("electron/main.js", "electron/preload.js",
            "electron/modules/ffi/ffiMain.js", "electron/modules/ffi/FFIPreloadMain.js"))
        report["complete_host_sources"] = list(host.values())
        main, preload, legacy, ffi = [host[name]["original"] for name in ("electron/main.js", "electron/preload.js",
            "electron/modules/ffi/ffiMain.js", "electron/modules/ffi/FFIPreloadMain.js")]
        for role, original, needle, kinds in (
            ("system_help_host_save_dialog_and_file_write", main, 'case"showFileSaveDialog":', {"switch_case"}),
            ("system_help_host_ffi_async_registration", main, 'n.handle("ffiPreloadAsync",', {"call_expression"}),
            ("system_help_host_ffi_sync_registration", main, 'n.handle("ffiPreload",', {"call_expression"}),
            ("system_help_host_ffi_exit_caller", main, "await b.ffiPreloadMain.callApiWhenExit()", {"function_declaration", "variable_declarator"}),
            ("system_help_preload_ffi_async", preload, "doDLLMainActionAsync:", {"pair"}),
            ("system_help_preload_ffi_sync", preload, "doDLLMainAction:", {"pair"}),
            ("system_help_preload_legacy_deprecated_route", preload, "doDLLAction:", {"pair"}),
            ("system_help_current_ffi_async_pointer_release", ffi, "async _callDLLFunctionAsync(", {"method_definition"}),
            ("system_help_current_ffi_sync_pointer_release", ffi, "_handleAction_Default=", {"field_definition"}),
            ("system_help_current_ffi_async_dispatch", ffi, "callDLLAsync=", {"field_definition"}),
            ("system_help_current_ffi_sync_dispatch", ffi, "callDLL=", {"field_definition"}),
            ("system_help_current_ffi_exit_registration", ffi, "_handleAction_ConfigureFFI_APIToCallWhenExit=", {"field_definition"}),
            ("system_help_current_ffi_exit_consumer", ffi, "callApiWhenExit=", {"field_definition"}),
            ("system_help_legacy_ffi_async_pointer_release", legacy, "exports.callDLLMainAsync=", {"assignment_expression"}),
            ("system_help_legacy_ffi_sync_pointer_release", legacy, "exports.callDLLMain=", {"assignment_expression"}),
        ):
            audit.retain(role, original, needle, kinds)

        receipt_path = a.ROOT / ".analysis/inputs/official-native-current/acquisition.json"
        receipt = json.loads(audit.artifact(receipt_path))
        coverage = {pid: [] for pid in products}
        for original in receipt["resources"]:
            if original.get("status") != "verified" or Path(original.get("path", "")).suffix.lower() != ".dll":
                continue
            manifests = []
            for link in original.get("manifests", []):
                match = re.fullmatch(r"https://apps\.razer\.com/synapse/products/(\d+)/mw/manifest\.json", link["url"])
                if match is None or int(match[1]) not in coverage or link.get("resource_name") != "BladeDLL":
                    continue
                product = int(match[1])
                acquired = json.loads(audit.artifact(a.ROOT / link["path"], link))
                declaration = help_system_native_declaration(acquired, original)
                current = [row for row in middleware[product] if row.get("path") and row["url"] == link["url"]]
                if len(current) != 1 or audit.read(current[0]) != audit.artifact(a.ROOT / link["path"], link):
                    raise ValueError("System Help native manifest differs from current middleware deployment")
                if link.get("resource_version") != declaration["resourceVersion"]:
                    raise ValueError("System Help native version link differs from its current declaration")
                manifests.append({"product_id": product, "link": link, "declaration": declaration,
                                  "current_manifest": current[0]})
                coverage[product].append(original["sha256"])
            if not manifests:
                continue
            native = audit.native_slice(original, ["GetSerialNumber", "GetBladeDisplayData", "GetAllBladeInfo", "GetBladeFirmwareInfo",
                "GetBladeDriverInfo", "FreeMalloc", "Terminate", "TerminateWhenExit"],
                needles=("DLL_PROCESS_ATTACH", "ColorProfileStopNew",
                    "Series10NewFeature::GetBlade", "Series10NewFeature::EnumerateMonitor",
                    "Series10NewFeature::MonitorEnumProc", "Series10NewFeature::ParseEDIDData",
                    "Series10NewFeature::GetMonitorDeviceInfo", "Series10NewFeature::GetProcessorInfo",
                    "Series10NewFeature::GetGraphicsInfo", "Series10NewFeature::GetRAMInfo",
                    "Series10NewFeature::GetSystemVersionInfo", "Series10NewFeature::GetBiosECFirmwareInfo",
                    "Series10NewFeature::GetCameraFWInfo", "Series10NewFeature::GetVBIOSInfo",
                    "Series10NewFeature::GetDriverInfo", "Series10NewFeature::GetSynapse",
                    "Series10NewFeature::GetNVDIA", "Series10NewFeature::GetIntelGraphicsCommandCenterInfo",
                    "Series10NewFeature::GetAMDSoftwareInfo", "Series10NewFeature::GetRazer"), depth=2)
            incomplete_bodies = [row["capture"]["address"] for row in native["complete_functions"]
                if not (row["capture"].get("pseudocode") or {}).get("lines") or not row["capture"].get("assembly")]
            if native["failures"] or native["decompiler_failures"] or incomplete_bodies:
                raise ValueError("Selected Help system information native functions are incomplete")
            capture = json.loads(audit.artifact(a.ROOT / native["capture"]["path"]))
            report["native"].append({"products": sorted({row["product_id"] for row in manifests}),
                "acquisition": families.identity(receipt_path), "manifests": manifests, "slice": native,
                "report_constants": help_system_report_constants(audit.read(original), native),
                "whole_capture_failures": capture.get("failures", [])})
        if any(len(identities) != 1 for identities in coverage.values()):
            raise ValueError("System Help current native declaration coverage is absent or ambiguous")
        report["native_product_coverage"] = coverage
        report["semantics"] = {
            "display_observation": "The middleware capability gate calls the current h3 export, parses GetBladeDisplayData JSON, stores systemDetails and dispatches FNw; the UI reducer retains the native payload. Getter failure remains unobserved and is not evidence that an application is absent.",
            "mounted_layout": "The retained product-page JSX props select basic for 694/695/696 and image for 709/710/711/736/737. Bundles may retain both components; availability does not establish that both are mounted.",
            "display_schema": "Display is always inserted by each selected producer and can be native null when no monitor exists. Monitor Name and the native spelling Manufactuer are preserved. Optional software/system strings are not fabricated when absent; unknown external fields are retained at the JSON boundary.",
            "full_save": "The UI dispatches ON_STORE_SYSTEM_INFO_DATA. The task maker, queue, runner and completion owners are retained. The runner uses observed systemDetails.DeviceName and Date.now(), queries GetAllBladeInfo, pretty-prints parsed JSON and invokes Save as / Save file with the Synapse 4 .synapse4 filter.",
            "host_save": "The host owns the sender's BrowserWindow, chooses the remembered directory or Downloads, preserves cancellation, then updates the directory and calls fs.writeFile. Its callback is not awaited. The Rust worker waits for an actual atomic file publication and never treats dialog cancellation or draft state as a successful write.",
            "native_result": "Official reference getters return owned UTF-8 JSON and the host frees it with FreeMalloc. Production Rust builds typed Display/Firmware/Driver observations directly from Windows WMI, DXGI, SetupDi, display enumeration and registry APIs; it does not load blade2Native or use its FFI descriptors.",
            "native_cleanup": "Official Terminate/TerminateWhenExit and DllMain remain analysis evidence only. Rust owns COM apartment initialization, BSTR/VARIANT/interface release, registry keys, device lists and driver lists with scoped destructors. EnumDisplayMonitors callbacks are synchronous and unwind-contained; the existing request worker rejects canceled/old-mount results and owns process termination.",
            "actual_read_corrections": "GetBladeDisplayData copies the NVIDIAControlPanel temporaries into IntelGraphicCommandCenter fields in the current original; Rust publishes the independently queried Intel version/platform. Missing registry strings, EC words, camera revisions and driver entries remain unobserved instead of source zero/empty defaults. Earlier actual query values survive later enumeration errors with separate diagnostics. These are explicit actual-read-rule differences, not a claim of byte-equal DLL output. Diagnostics are separate from observed data and reach the existing shell log.",
            "rust_ownership": "Page mount and connection identity/generation guard delivery. A retained shell worker owns IPC, dialog and file I/O; SaveFileDialog has a separate STA and cancellation. Page replacement, connection change and exit cancel outstanding work. Exit waits for worker/service/dialog cleanup and canceled exit admits only new operations after old work drains.",
        }
        report["implementation_owners"] = [
            "crates/razer-device/src/host/system_info.rs", "crates/razer-ipc/src/device_tasks.rs",
            "crates/razer-service/src/native/help_system_info.rs", "crates/razer-service/src/runtime/mod.rs",
            "crates/razer-service/src/native/help_system_info/windows.rs",
            "crates/razer-service/src/native/help_system_info/wmi.rs",
            "crates/razer-service/src/native/help_system_info/registry.rs",
            "crates/razer-service/src/native/help_system_info/devices.rs",
            "crates/razer-service/src/native/help_system_info/semantics.rs",
            "crates/razer-platform/src/file_dialog.rs", "crates/razer-platform/src/platform/windows/file_dialog.rs",
            "crates/razer-pages/src/features/help/source_help_system.rs",
            "crates/razer-pages/src/features/workspace/source_workspace/help_system.rs",
            "crates/razer-shell/src/shell/host/help_system_info.rs"]
        report["remaining_chains"] = [
            "Unavailable source maps and failed acquisition/capture probes remain listed in each current deployment receipt. The observed minified sources are byte-verified; this query does not establish every Help/product page or dependency branch is complete.",
            "Selected native complete bodies are byte-verified. Their indirect/import and unexpanded direct helper edges are retained explicitly; bounded selection is not a claim of complete whole-module native semantics.",
            "The other local Help actions, support text/popups and product/accessory branches require their own full source-to-device/service and resource acceptance. This query covers system information observation and full local report saving only.",
            "Rust compile/unit/mock results are recorded separately in the unified validation record. The independent Windows report path still needs explicit local original-versus-Rust comparisons, including source anomalies, partial query failures and all deployed product branches. Real windows/file dialogs remain unverified under the execution limits."]
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        report["failures"].append(str(error))
    report["complete_originals"] = list(audit.sources.values())
    report["complete_source_owners"] = list(audit.owners.values())
    report["verified_artifacts"] = list(audit.artifacts.values())
    report["changed_during_query"] = audit.final_snapshot()
    report["snapshot_stable"] = not report["changed_during_query"]
    report["failures"].extend("Audit artifact changed during query: " + name for name in report["changed_during_query"])
    destination = a.ROOT / ".analysis/results/host-current/local-audit.json"
    with a.index_lock(destination.with_suffix(".lock")):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous["help_system_info"] = report
        a.save(destination, previous)
    print(json.dumps({"receipt": a.rel(destination), "query": args.query, "products": len(report["products"]),
        "complete_source_owners": len(audit.owners), "failures": report["failures"]}), flush=True)
    if report["failures"]:
        raise ValueError("Local Help system information source query remains unresolved")


def audit_product_chroma_observation(args):
    """Rebuild product host observation evidence from current original bytes."""
    audit = HostLocalAudit()
    a = audit.a
    report = {"recorded_at_utc": a.stamp(), "parser": audit.parser_versions,
              "source_sets": [], "selected": {}, "exports": [], "failures": [],
              "source_chain_complete": False, "target_execution": False,
              "real_device_operations": False, "runtime_acceptance": "unverified"}
    selected = report["selected"]
    try:
        for name in ("tools/host.py", "tools/workspace.py", "tools/families.py", "tools/evidence.py", "tools/analysis.py"):
            audit.artifact(a.ROOT / name)
        sources = {}
        for name in ("synapse-lighting-engine-current", "mouse-226-current", "keyboard-756-current", "mouse-162-current"):
            source_set, originals = audit.source_set(name)
            source_set["verified_complete_capture"] = audit.frontend_capture(name, originals)
            report["source_sets"].append(source_set)
            sources[name] = originals

        def source(name, anchor):
            matches = [row for row in sources[name] if Path(row["path"]).suffix == ".js"
                       and anchor.encode() in audit.read(row)]
            if len(matches) != 1:
                raise ValueError("Product Chroma source is absent or ambiguous: " + anchor)
            return matches[0]

        def retain(role, original, anchor, kinds, unique=True):
            selected[role] = audit.retain(role, original, anchor, set(kinds), unique=unique)
            return selected[role]

        engine = source("synapse-lighting-engine-current", "async updateChromaStudio(e)")
        for role, anchor, kinds in (
            ("engine_actual_chroma_devices_publisher", "async updateChromaStudio(e)", ("method_definition",)),
            ("engine_published_payload_actual_filter_and_order", "generateUpdateChromaStudioPayload(e){", ("method_definition",)),
            ("engine_update_caller", "this.updateChromaDevice=async e=>", ("assignment_expression",)),
            ("engine_refresh_caller", "this.handleRefreshDevices=async e=>", ("assignment_expression",)),
            ("engine_add_caller", "async addChromaDevice(e,t)", ("method_definition",)),
            ("engine_remove_caller", "async removeDeviceChromaEngine(e)", ("method_definition",)),
            ("engine_actual_host_memory_bridge", "this.getMemoryStorageItem=", ("assignment_expression",)),
            ("engine_actual_host_memory_publisher_bridge", "this.setMemoryStorageItem=", ("assignment_expression",))):
            retain(role, engine, anchor, kinds)
        report["exports"].append(audit.webpack_export("engine_actual_memory_key", sources["synapse-lighting-engine-current"], 1876, "D6"))
        publisher = audit.owners[selected["engine_actual_chroma_devices_publisher"][0]]
        for name in ("o", "l"):
            selected["engine_publisher_namespace_" + name] = audit.binding(
                "engine_publisher_namespace_" + name, engine, name, publisher["byte_start"])

        for family, name, reducer, projection in (
            ("mouse", "mouse-226-current", "pe=", "l={DEVICE_TYPE:"),
            ("keyboard", "keyboard-756-current", "re=", "D={DEVICE_TYPE:")):
            lighting = source(name, "checkSyncQuickEffect(e){")
            for role, anchor in (("quick_memory_consumers_complete_classes", "this.checkSyncQuickEffect("),
                                 ("installation_consumers_complete_classes", "e.checkIsFullyInstall().then(")):
                retain(family + "_" + role, lighting, anchor, ("class_declaration", "class"), False)
            for index, anchor in enumerate(("loadActiveProfileSettings(){", "this.loadActiveProfileSettings=")):
                root = source(name, anchor)
                retain(family + "_local_profiles_projection_branch_" + str(index), root, anchor,
                       ("method_definition", "assignment_expression"))
            main = source(name, "this.getMemoryStorageItem=")
            retain(family + "_actual_get_memory_storage_bridge", main, "this.getMemoryStorageItem=", ("assignment_expression",))
            retain(family + "_local_projection_keys", main, projection, ("variable_declarator",))
            retain(family + "_lighting_reducer_initial_state", main, reducer + "{selectedEffectId:3,effectSettings:[]", ("variable_declarator",))
            dispatch = "Te=function(e,t){" if family == "mouse" else "Ae=function(e,t){"
            retain(family + "_profiles_reducer_dispatch", main, dispatch, ("variable_declarator",))
            checker = source(name, "async checkIsFullyInstall(){")
            retain(family + "_fully_installed_complete_owner", checker, "async checkIsFullyInstall(){", ("class", "class_declaration"))

        babel = source("mouse-162-current", "r.loadActiveProfileSettings=function()")
        retain("babel_local_profiles_projection", babel, "r.loadActiveProfileSettings=function()", ("assignment_expression",))
        retain("babel_quick_memory_complete_component", babel, "uJ=function(e){", ("variable_declarator",))
        retain("babel_advanced_complete_component", babel, "_J=function(e){", ("variable_declarator",))
        retain("babel_advanced_connected_component", babel, "fJ=(0,L.connect)", ("variable_declarator",))
        retain("babel_effects_complete_component", babel, "OJ=function(e){", ("variable_declarator",))
        advanced = audit.owners[selected["babel_advanced_complete_component"][0]]["body"]
        if "checkIsFullyInstall" in advanced or "this.props.isChromaInstalled" not in advanced:
            raise ValueError("Current Babel installation branch changed")
        connected = audit.owners[selected["babel_advanced_connected_component"][0]]["body"]
        if 'commonReducer.installedModules||[]' not in connected or '"chroma-app"===e' not in connected:
            raise ValueError("Current Babel module-status projection changed")
        report["host_provenance"] = verified_host_owners(audit, (
            "electron/modules/memory_storage/index.js", "electron/modules/window_storage/index.js"))
        for name, item in report["host_provenance"].items():
            row = item["original"]
            key = audit.retain_range("host_" + name.split("/")[-2] + "_complete_owner", row, 0,
                                     len(audit.read(row)), "program")
            selected["host_" + name.split("/")[-2] + "_complete_owner"] = [key]
        report["semantics"] = {
            "independent_observations": "Local Chroma configuration, module state, dynamic-lighting state, installation probes and the lighting-engine memory publisher have independent producers and failure results.",
            "publisher": "updateChromaStudio publishes generateUpdateChromaStudioPayload through the verified D6 memory key. Preserve actual device filters, ordering and empty-payload distinction.",
            "memory_getter": "The host stores publisher URL entries in Map order; QuickEffects reads only the first returned value, subscribes to memorystorageevent, and removes its handler on unmount. Absent and published-empty remain separate observations.",
            "local_profiles": "Current product roots select the matching source-local Chroma record and dispatch enabled, profiles and currentProfile independently of installation and memory results. Missing or failed reads cannot establish default or old device state.",
            "babel": "PID162 loads local Chroma profiles in loadActiveProfileSettings. Its connected advanced component derives isChromaInstalled from commonReducer.installedModules. Its complete advanced component contains no checkIsFullyInstall call; this branch must not run the newer installation probe.",
            "installation": "Newer advanced components call the complete checker, which combines the actual app-engine product observation and local Apps folder existence. This proves host installation facts only.",
        }
        report["remaining_chains"] = [
            "Complete ordinary keyboard/mouse quick-effect persistence, lighting-engine device writeback, Chroma handoff and display/idle lifecycle paths for every product/accessory branch.",
            "These complete owners cover PID162, PID226 and PID756 host observation boundaries; they do not establish the entire mounted UI/backend checklist for all products.",
            "Frontend capture failures and missing source maps remain recorded in each source set; retained source owners are not complete reconstructed-source or runtime acceptance.",
        ]
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        report["failures"].append(str(error))
    report["complete_originals"] = list(audit.sources.values())
    report["complete_source_owners"] = list(audit.owners.values())
    report["verified_artifacts"] = list(audit.artifacts.values())
    report["changed_during_query"] = audit.final_snapshot()
    report["snapshot_stable"] = not report["changed_during_query"]
    report["failures"].extend("Audit artifact changed during query: " + name for name in report["changed_during_query"])
    destination = a.ROOT / ".analysis/results/host-current/local-audit.json"
    with a.index_lock(destination.with_suffix(".lock")):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous["product_chroma_observation"] = report
        a.save(destination, previous)
    print(json.dumps({"receipt": a.rel(destination), "query": args.query,
                      "complete_source_owners": len(audit.owners), "failures": report["failures"]}), flush=True)
    if report["failures"]:
        raise ValueError("Product Chroma source observation query remains unresolved")


def collect_chroma_dashboard_launch(audit, host_originals):
    """Join the current Chroma hidden-start/restore page to URL-keyed host storage."""
    source_set, originals = audit.source_set("chroma-app-dashboard-current")
    names = ("main.e5fe7086.js", "332.780bf0e1.chunk.js")
    files = [source for source in originals if Path(source["path"]).name in names]
    if len(files) != len(names):
        raise ValueError("Current Chroma launch originals are absent or ambiguous")
    main = next(source for source in files if Path(source["path"]).name == names[0])
    chunk = next(source for source in files if Path(source["path"]).name == names[1])
    selected, exports, bindings = {}, {}, {}

    def binding(role, source, name, reference):
        result = audit.binding("chroma_launch_" + role, source, name, reference)
        if result["resolution"] != "declaration":
            raise ValueError("Current Chroma launch has an unresolved binding: " + role)
        selected[role], bindings[role] = result["owner_id"], result
        return audit.tree(source).root_node.descendant_for_byte_range(result["start_byte"], result["end_byte"] - 1)

    def export(role, module, name):
        result = audit.webpack_export("chroma_launch_" + role, files, module, name)
        exports[role] = result
        selected[role] = result["binding"]["owner_id"]
        result_binding = result["binding"]
        return audit.tree(result["original"]).root_node.descendant_for_byte_range(
            result_binding["start_byte"], result_binding["end_byte"] - 1)

    selected["entry_mount"] = audit.retain("chroma_launch_entry_mount", main,
        "Promise.all([T.e(7714),T.e(3874),T.e(332)])", {"arrow_function"})[0]
    export("entry_export", 11078, "default")
    binding("language_wrapper", chunk, "Ne", 332540)
    page = binding("page_class", chunk, "ge", 332000)
    binding("install_gate", chunk, "Ce", 330800)
    binding("hidden_flag", chunk, "De", 330800)
    binding("auto_start_query", chunk, "Se", 328470)
    binding("force_hidden_query", chunk, "Ie", 328470)
    for name in ("handleAppHiddenStart", "handleOnUserRestoreWindow", "componentDidMount", "render"):
        _, selected[name] = audit.callable_member("chroma_launch_" + name, chunk, page, name)
    descriptors = export("hidden_descriptors", 60336, "Oc")
    for name in ("l", "d"):
        binding("descriptor_" + name, chunk, name, descriptors.start_byte)
    for role, module, name in (("tab_helper", 87893, "A"), ("tab_manager", 28648, "A"),
                              ("visualizer_key", 69937, "XrU"), ("host_facade", 37226, "A")):
        export(role, module, name)
    tab_helper = audit.owners[selected["tab_helper"]]
    tab_root = audit.tree(chunk).root_node.descendant_for_byte_range(tab_helper["byte_start"], tab_helper["byte_end"] - 1)
    _, selected["getTabInfo"] = audit.callable_member("chroma_launch_getTabInfo", chunk, tab_root, "getTabInfo")
    manager = audit.owners[selected["tab_manager"]]
    manager_root = audit.tree(chunk).root_node.descendant_for_byte_range(manager["byte_start"], manager["byte_end"] - 1)
    _, selected["forceOpenHiddenTab"] = audit.callable_member(
        "chroma_launch_forceOpenHiddenTab", chunk, manager_root, "forceOpenHiddenTab")
    facade = binding("facade_class", main, "R", exports["host_facade"]["binding"]["start_byte"])
    for name in ("callElectronAction", "getWindowStorageItem", "setWindowStorageItem", "getWindowServiceClients"):
        _, selected[name] = audit.callable_member("chroma_launch_" + name, main, facade, name)
    for role, relative, name in (("host_storage", "modules/window_storage/index.js", "callFunction"),):
        source = host_originals["electron/" + relative]
        _, selected[role] = audit.callable_member("chroma_launch_" + role, source, audit.tree(source).root_node, name)
    preload = host_originals["electron/preload.js"]
    for role, name in (("preload_response", "response"), ("preload_invoke", "doElectronAction")):
        _, selected[role] = audit.callable_member("chroma_launch_" + role, preload, audit.tree(preload).root_node, name)
    selected["preload_pagehide"] = audit.retain("chroma_launch_preload_pagehide", preload,
        'window.addEventListener("pagehide",', {"call_expression"})[0]
    dispatch = host_originals["electron/main.js"]
    selected["host_dispatch"] = audit.retain("chroma_launch_host_dispatch", dispatch,
        'n.handle("electronAction",', {"call_expression"})[0]
    report = {"source_set": source_set, "selected": selected, "exports": exports, "bindings": bindings,
        "semantics": {
            "hidden_start": "Only autoStart=1 together with forceHidden=1 initializes hidden. The page requests background-manager and notification-manager through the shared lock-based hidden-tab helper.",
            "visualizer": "Hidden start reads the saved synapseAudioVisualizerProfiles visualizerState and the actual window-client list. It opens the existing local Visualizer URL only when no matching tab was found and the saved state is truthy; this is not a device getter or confirmed engine start.",
            "restore": "visibleChange with isVisible, changeActiveTab for chroma-app, focus with isFocus and USER_CLICK_DESKTOP_ICON request restore. An empty status array or no value.windowName containing chroma-app returns before unhiding. The selected URL row with installing status blocks only the running publication; the page still clears its hidden state afterwards. A rejected initial storage await occurs before the try block and does not reach that clear.",
            "storage_result": "The Chroma facade forwards through preload electronAction into URL-keyed WindowStorage. GET returns JSON rows for truthy values; SET returns the previous value and emits change events. hidden/running publication is not awaited, and neither state acknowledges installation, a native worker or device settings.",
            "cleanup": "The hidden-tab helper removes its create listener on existing-tab, create-event and ten-second timeout paths; a rejection is caught and does not itself remove that listener. ge has no componentWillUnmount and discards the tab-message disposer and internal desktop-icon listener. Preload pagehide separately removes IPC listeners. Ce has an empty-dependency effect with an initial-hidden early return; prop changes alone do not re-run that same effect instance.",
        }, "selected_static_semantics_reviewed": True, "source_chain_complete": False,
        "target_execution": False, "actual_dll_comparison": "unverified",
        "rust_independent_implementation": "unverified", "ui_backend_wiring": "unverified",
        "runtime_acceptance": "unverified"}
    verify_chroma_dashboard_launch(audit, report)
    return report


def verify_chroma_dashboard_launch(audit, report):
    """Reject excerpts, rebinding and completion claims in this bounded launch review."""
    expected = {
        "entry_mount": "90bb700a7c06df95da413d2b8592dca7b95a47d627311d907ed090c09471b004",
        "entry_export": "3e7168b4a7489a6085678ac64f33f103ff5174dd6ed1c53d3590d536fecc70c9",
        "language_wrapper": "0a419718135e75aae77bde3b97acf626b4188958f774f61425e8232e473a9659",
        "page_class": "983bc2cd93cd7d3e2e6c4b716c75b700bd47536cfaf903e924cc070682f67424",
        "install_gate": "d02559d05d6e8ee433c86be089a7ea5f3cef14cab17fa1f3f9da22f139b79d57",
        "hidden_flag": "d0b3de235a33b37bdfec246ff7fe740bde2154da06d2aac9fc0cd1f4d1295bbd",
        "auto_start_query": "e21d4f06b3e5e23091547225d33dc1eeebc82a1ed018b7820d3185491428950c",
        "force_hidden_query": "55052080e983a72c53af3f88ccf8af2cada106419a3ee78253726afc21e50c41",
        "handleAppHiddenStart": "f6280cb5ba130011efe3fdeb47644a6d86d731dd2d7bd9fabf65ea8acca6683c",
        "handleOnUserRestoreWindow": "19d8bebafa851f64a4225e0723d7f99545ee8d4d113f77a4aa5ffdbef74ada3f",
        "componentDidMount": "4693d5af9b21ec39f3a8235122ceccc239beae3745c765e9608decd83ea4bef2",
        "render": "8e1134be5717f27e97c086dd9de0f6d8ad89c04a4f4b34388f417e539e445f5c",
        "hidden_descriptors": "09432d6a907d22a7fe1a25ab15f99b499960f59eff020f2883f62220f9334fa7",
        "descriptor_l": "1fcd8384d350ecef99fb9474c3c2a1aa43ca32ac9078f23a94081502c469d861",
        "descriptor_d": "8153f14f20c32c0cc7e1f517defe9e7d8db3f37e9cecc96cac5ed4be6946d024",
        "tab_helper": "d4daa416e3334ba7830aa2a3c8072e21355a45ea84498189e5bd80fb357b84e0",
        "tab_manager": "a50bbcd29b830060cda00fa3dc79dc6e046ebd2cbec2140df28ce114d1eeba9f",
        "visualizer_key": "12b2fdc1b4416a640dfe071be5a099d6eab1404e3b9e855d9a8ab634527cf8ff",
        "host_facade": "36cd8884b96cfd032ef9f934a2b5d9359da011d51cd9755f893950619aeb7daa",
        "getTabInfo": "4fdcd08552a441db5bd7c81a004452412fddfe4a01bcb8ac76b9f3c5909a1fd9",
        "forceOpenHiddenTab": "165a7f37534e09db9986a12cb69502354aecc107943c47e4f150483d9fb683fd",
        "facade_class": "d6e10a5078465bac01dad844aa811adf32f27bf415ebe93d9005bedcfad1bb67",
        "callElectronAction": "45e13eebe85b3e1b0b1723e8cdebd1f9a4c1fabcaaca6eb30e56dd9b9c3dbf40",
        "getWindowStorageItem": "8105476f22d349a6616ee930234afd07b7ec64be28ee1c472df7e9db30d31fe0",
        "setWindowStorageItem": "3b258a0b1c4fbc27177a6a6afe75d7a3c76b5748a7eff9af8ed1e84a8e915a00",
        "getWindowServiceClients": "b2e0b776878264e4a1bd36f3c44fac7eefa9434d86932c24aa30d945ab0fce15",
        "host_storage": "529bd8409fabb820ba0912a3dad9639cad0e0fa12f0e9f6d991844bb1295e793",
        "preload_response": "6c93d17865da59776ca87e9cb74e80c06fda9c6501e87219f18463af863666a4",
        "preload_invoke": "476db09d6e320b1519ae4ce21a92490ec363d073fbadda68b1a80b9f79149e59",
        "preload_pagehide": "4217c07812b24a6f26920870473590b42a4dbdce1c61331198f7c0c451c40213",
        "host_dispatch": "a957dbbaa2759162a1bc25601bcc5937e0b8ae5ab01041170cc088a022650b4d",
    }
    if (report.get("source_chain_complete") is not False or report.get("target_execution") is not False
            or report.get("selected_static_semantics_reviewed") is not True
            or any(report.get(name) != "unverified" for name in ("actual_dll_comparison",
                "rust_independent_implementation", "ui_backend_wiring", "runtime_acceptance"))):
        raise ValueError("Chroma launch review promotes static evidence to runtime acceptance")
    for role, owner_id in report["selected"].items():
        owner = audit.owners[owner_id]
        verify_profiles_owner(audit, owner)
        if role in expected and owner["body_sha256"] != expected[role]:
            raise ValueError("Chroma launch complete owner identity differs: " + role)
    if set(expected) != report["selected"].keys():
        raise ValueError("Chroma launch review omits a required result or state owner")
    if set(report["exports"]) != {"entry_export", "hidden_descriptors", "tab_helper", "tab_manager",
                                  "visualizer_key", "host_facade"}:
        raise ValueError("Chroma launch export graph is incomplete")
    for role, exported in report["exports"].items():
        if report["selected"].get(role) != exported["binding"]["owner_id"]:
            raise ValueError("Chroma launch export was rebound: " + role)
    return {"selected_owners": len(report["selected"]), "target_execution": False,
            "runtime_acceptance": "unverified"}


def audit_host_window_lifecycle(args):
    """Retain actual host window, storage, suspend and shutdown owners."""
    audit = HostLocalAudit()
    a = audit.a
    report = {"recorded_at_utc": a.stamp(), "parser": audit.parser_versions,
              "host_sources": [], "selected": {}, "failures": [],
              "source_chain_complete": False, "target_execution": False,
              "real_device_operations": False, "runtime_acceptance": "unverified"}
    try:
        for name in ("tools/host.py", "tools/workspace.py", "tools/families.py", "tools/evidence.py", "tools/analysis.py"):
            audit.artifact(a.ROOT / name)
        paths = ["electron/" + name for name in (
            "main.js", "preload.js", "constants.js", "mainSubFunction.js",
            "lib/globalNodeVar.js", "lib/createDebouncedSystrayRebuild.js",
            "components/Tab/Tab.js", "components/Tab/TabStore.js",
            "components/Tab/TabManager.js", "components/Tab/common.js",
            "components/Tab/LeftSystray.js", "modules/window_storage/index.js",
            "modules/memory_storage/index.js", "UsbRzDeviceAction.js",
            "modules/sysutil/index.js", "modules/sysutil/win/index.js",
            "modules/sysutil/mac/index.js", "modules/ffi/FFIPreloadMain.js",
            "modules/ffi_subprocess/index.js", "lib/common.js", "WssAction.js",
            "lib/useLogger.js", "modules/noble/index.js", "modules/noble/endpoint-detect/index.js")]
        paths.append("node_modules/node-rz-ble-endpoint-detection/index.js")
        retained = verified_host_owners(audit, paths)
        originals = {name: item["original"] for name, item in retained.items()}
        for name, item in retained.items():
            source = item["original"]
            tree = audit.tree(source)
            audit.retain_range("complete_host_file:" + name, source, 0,
                               len(audit.read(source)), tree.root_node.type)
            report["host_sources"].append({key: value for key, value in item.items() if key != "body"})
        selections = [
            ("host_electron_action_dispatch", "main.js", 'n.handle("electronAction",', {"call_expression"}),
            ("host_before_quit", "main.js", 'e.on("before-quit",', {"call_expression"}),
            ("host_will_quit", "main.js", 'e.on("will-quit",', {"call_expression"}),
            ("host_all_windows_closed", "main.js", 'e.on("window-all-closed",', {"call_expression"}),
            ("host_suspend_platform_branch", "main.js", 'c.on("suspend",', {"call_expression"}),
            ("host_resume_platform_branch", "main.js", 'c.on("resume",', {"call_expression"}),
            ("host_exit_admission_and_broadcast", "main.js", 'async function Dn(', {"function_declaration"}),
            ("host_final_shutdown", "main.js", 'async function Mn()', {"function_declaration"}),
            ("host_exit_window_countdown", "main.js", 'Un=o=>', {"variable_declarator"}),
            ("preload_pagehide_cleanup", "preload.js", 'window.addEventListener("pagehide",', {"call_expression"}),
            ("common_close_window_handler", "components/Tab/common.js", 'G=e=>o=>', {"variable_declarator"}),
            ("common_window_closed_handler", "components/Tab/common.js", 'Q=e=>()=>', {"variable_declarator"}),
            ("common_set_will_close", "components/Tab/common.js", 'Ne=(e,o)=>', {"variable_declarator"}),
        ]
        for role, relative, anchor, kinds in selections:
            report["selected"][role] = audit.retain(role, originals["electron/" + relative], anchor, kinds)
        for relative, names in {
            "components/Tab/Tab.js": ("createTab", "initEventHandler", "destroy", "beforeHibernate", "hibernate", "reInit"),
            "components/Tab/TabStore.js": ("setHibernateView", "removeHibernate", "getStopHibernateList", "setActiveTabOfWindow"),
            "components/Tab/TabManager.js": ("handleChangeActiveTab", "initHibernateProcess", "restoreFromHibernate", "hibernatePage"),
            "components/Tab/common.js": ("hibernateWindow", "hibernateAllTabs", "clearHibernate", "reCreateWindow", "handleCloseBtn", "closeWindow", "focusWindow"),
            "modules/window_storage/index.js": ("callFunction", "shouldFireEvent"),
            "modules/memory_storage/index.js": ("callFunction", "shouldFireEvent"),
            "modules/sysutil/win/index.js": ("initDll", "callDLL", "callDLLAsync", "handleFFIEvent"),
            "modules/sysutil/mac/index.js": ("initDll", "callDLL", "callDLLAsync", "handleFFIEvent"),
            "modules/ffi/FFIPreloadMain.js": (
                "_handleAction_ConfigureFFI_APIToCallWhenExit", "generateKey_APIToCallWithPID",
                "_handleAction_SetAPIToCallWhenExitDevice", "_handleAction_RemoveAPIToCallWhenExitDevice",
                "_handleAction_SetAPIToCallWhenSuspend", "_handleAction_RemoveAPIToCallWhenSuspend",
                "_handleAction_SetAPIToCallWhenShutdown", "_handleAction_RemoveAPIToCallWhenShutdown",
                "callApiWhenExitDevice", "callApiWhenShutdown", "callApiWhenSuspend", "callApiWhenExit",
                "_callDLLNoAsync", "callDLL", "callDLLAsync", "_handleAction_FreeFFI"),
            "modules/ffi_subprocess/index.js": ("callDLLAsync", "batchCallFFIProcesses"),
            "WssAction.js": ("startServer", "start", "stop"),
            "lib/useLogger.js": ("saveLoggerBeforeQuit", "consumer", "close"),
            "modules/noble/index.js": ("startBLEEndpointMonitoring", "stopBLEEndpointMonitoring"),
            "modules/noble/endpoint-detect/index.js": ("start", "stop"),
            "mainSubFunction.js": ("createExitEvent", "setExitEvent"),
            "lib/common.js": ("promiseWithTimeout", "sleepWithWhileLoop"),
            "UsbRzDeviceAction.js": (
                "constructor", "stop", "handle_hid_setDevicesToDefaultNoAsync",
                "setDevicesToDefaultNoAsyncExec", "doDisableAllDevicesHooks", "doEnableAllDevicesHoooks",
                "handle_hid_addDeviceCmdDataSuspend", "handle_hid_removeDeviceCmdDataSuspend",
                "handle_hid_addDeviceCmdDataExit", "handle_hid_removeDeviceCmdDataExit",
                "setDevicesSuspendNoAsync", "handle_setDevicesSuspendNoAsync",
                "setDevicesExitNoAsync", "handle_setDevicesExitNoAsync"),
        }.items():
            source = originals["electron/" + relative]
            for name in names:
                role = relative + ":" + name
                _, owner = audit.callable_member(role, source, audit.tree(source).root_node, name)
                report["selected"][role] = [owner]
        report["chroma_dashboard_launch"] = collect_chroma_dashboard_launch(audit, originals)
        native, lineage = verified_host_native(audit,
            "win-unpacked/resources/app.asar.unpacked/node_modules/"
            "node-rz-ble-endpoint-detection/build/Release/detection.node")
        native_result = a.ROOT / ".analysis/results/native" / native["sha256"]
        capture, capture_ref = a.verify_native_capture(native_result, native)
        selected = audit.families.verify_native_functions(native, {
            0x3af0: "addon_registration_and_initial_inventory", 0x39d0: "start_monitoring",
            0x3aa0: "stop_monitoring", 0x5a00: "monitor_work_wait",
            0x5a60: "monitor_after_work", 0x5cc0: "device_window_thread",
            0x5c60: "device_window_procedure", 0x72e0: "device_change_producer",
            0x1c00: "register_added_callback", 0x2480: "register_removed_callback",
            0x1d20: "added_callback_consumer", 0x25a0: "removed_callback_consumer",
            0x2d00: "find_queue", 0x5b10: "find_worker", 0x3020: "find_after_work",
            0x6ab0: "initial_inventory"})
        for row in selected["complete_selected_functions"]:
            audit.artifact(a.ROOT / row["artifact"]["path"], row["artifact"])
        audit.artifact(native_result / "capture.json", capture_ref)
        resolution = a.native_reanalysis_resolution(native_result, capture)
        report["ble_endpoint_native"] = {
            "lineage": lineage, "capture": capture_ref,
            "image_base": capture["image_base"], "ida_version": capture["ida_version"],
            "hexrays_version": capture["hexrays_version"],
            "functions_total": capture["functions_total"],
            "complete_selected_functions": selected["complete_selected_functions"],
            "base_decompilation_failures": len(capture["failures"]),
            "reanalysis": resolution, "target_execution": False,
            "semantics_complete": False, "runtime_acceptance": "unverified"}
        report["semantics"] = {
            "identity": "Window names/frameName and owning window IDs identify tabs; selected-tab, force-focus, native focus, visibility and hibernation are separate source facts.",
            "storage": "WindowStorage and MemoryStorage are URL-keyed host Maps. Gets return JSON arrays of truthy values; the complete owners retain actual return and event differences. No device write or disk persistence is implied.",
            "tab_cleanup": "Tab destroyed/render-process-gone constructs sender.getURL(){return this.url} on a fresh object without url. It does not capture the Tab URL. clearWindowStorage deletes only the selected URL map and sends no removal event.",
            "hibernate": "The default tab timeout is 120000ms. Ordinary beforeHibernate reads STOP_HIBERNATE and checks current active/visible state and reject rules. Explicit hibernatePage uses a separate zero-delay callback to hibernate directly. Restoring reinitializes the original Tab and then requests activation.",
            "preload": "pagehide removes tab-message, async-message, windowStorageEvent and window-list-change listeners and closes the local broadcast channel; the retained callback does not remove every IPC listener or acknowledge native cleanup.",
            "power": "Electron powerMonitor suspend/resume deliberately ignores Windows. Windows SysUtils handleFFIEvent parses the native JSON, emits global suspend/resume, optionally requests quit on suspend, then broadcasts to live webContents. Native callback production and release require a separate audit.",
            "exit_admission": "A named app is blocked while listed in cannotExitAppNameList. Whole-engine exit is blocked on its first two attempts while the list is nonempty; forceQuick bypasses this gate. The exitApp broadcast precedes a 200ms delay and exit-app emission; lighting-engine waits use distinct shutdown and exit-completed flags and expire after ten 200ms iterations.",
            "device_restore": "UsbRzDeviceAction suspend disables hooks and traverses registered default, suspend and exit command sequences. The default compatibility branch may issue asynchronous sendFeatureReport without waiting. Resume/unlock checks screen-lock state before requesting hook enablement. exit-app invokes default and exit sequences. stop only calls stopMonitoring; no complete HID or worker release is acknowledged.",
            "ffi_cleanup": "FFIPreloadMain keeps separate per-channel exit and per-channel/product/container command Maps. Device/shutdown/suspend loops call the synchronous no-mutex path and do not validate completion results. FreeFFI returns true without releasing the library or callback. Subprocess exit awaits each process call but the manager does not establish native worker or process termination.",
            "shutdown_dependencies": "Ln is WssAction: stop requests client/server close and returns true without awaiting the server callback. ae requests SysUtils SetExitEvent without awaiting. Qo stops native BLE endpoint monitoring and requests disconnect on cached peripherals without completion waits. N is saveLoggerBeforeQuit: it first awaits per-URL close promises and only then applies a 30000ms bound to the remaining logger-wait loop. promiseWithTimeout races rejection without canceling the original operation.",
            "ble_native_stop": "The endpoint addon starts an unretained Windows thread and a reusable uv work item. stopMonitoring clears its active byte, stops two uv signal handles and signals the work event; it neither posts window quit nor joins the window thread. monitor_after_work skips callback delivery, lpMem cleanup, the producer handshake signal and requeue after active becomes false. The retained selected bodies do not close the two events, uv handles or notification registrations. This is static behavior, not an observed runtime failure.",
            "shutdown": "Mn marks isQuit, cancels tray rebuild, awaits APIToCallWhenExit, invokes device/shutdown preload APIs, awaits exit preload, then attempts SysUtils, mapping-engine and simple-service shutdown with separate 3000ms waits before app.quit. Outer exit timers are not successful release acknowledgements.",
        }
        report["remaining_chains"] = [
            "The complete retained host owners do not establish native worker, callback, HID/COM and service release completion; follow each native target and failure branch independently.",
            "Review every launched app/product descriptor and its actual canHibernate/manual-close/STOP_HIBERNATE consumer; this host query does not establish every page lifecycle.",
            "Online fallback, account and remote-sync paths are retained source context only and remain excluded from the local-first product.",
        ]
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        report["failures"].append(str(error))
    report["complete_originals"] = list(audit.sources.values())
    report["complete_source_owners"] = list(audit.owners.values())
    report["verified_artifacts"] = list(audit.artifacts.values())
    report["changed_during_query"] = audit.final_snapshot()
    report["snapshot_stable"] = not report["changed_during_query"]
    report["failures"].extend("Audit artifact changed during query: " + name for name in report["changed_during_query"])
    destination = a.ROOT / ".analysis/results/host-current/local-audit.json"
    with a.index_lock(destination.with_suffix(".lock")):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous["window_lifecycle"] = report
        a.save(destination, previous)
    print(json.dumps({"receipt": a.rel(destination), "query": args.query,
                      "complete_source_owners": len(audit.owners), "failures": report["failures"]}), flush=True)
    if report["failures"]:
        raise ValueError("Host window lifecycle source query remains unresolved")


def profiles_static_boundaries():
    """Reviewed page semantics; none of these fields is a runtime result."""
    return {
        "installed_apps": {"icon_size": [80, 80], "join": "path_then_name_path_dedup",
            "refresh_end": "300ms_timer_after_nonempty_list_and_icon_join",
            "empty_or_failure_clears_refresh": False, "inflight_cancel_on_unmount": False},
        "browse": {"multi_selection": True, "windows_types": ["exe", "url"],
            "mac_types": ["app", "url"], "dispatch": "foreach_without_join",
            "electron_cancel": "JSON_string_of_empty_filePaths",
            "missing_host_reply": "array_not_JSON_string"},
        "add": {"waits_for": "memory_storage_action_promise",
            "publication_error_rethrown": False, "close_proves_service_success": False,
            "request_id_supplied": False, "remote_result_consumed": False},
        "drop": {"requires_file_and_target_class": True, "extension_filter": False,
            "dispatch": "each_file_path_without_join", "container_listener_cleanup": True,
            "overlay_drop_cleanup": "adds_listener_again"},
        "scan": {"request_result_correlated": False, "stop_action_in_page": False,
            "list_commit_clears_syncing": True, "start_failure_clears_isScanning": False,
            "finished_requires_success_flag": False},
        "relocate": {"selection": "first_path_case_sensitive_unanchored_exe_regex",
            "conflict": "exact_location", "selection_dispatches_add": False,
            "selected_action": "reducer_id_path_only", "replacement_input": "new_ids_in_later_list",
            "old_entry_removed_without_link": True, "clear_and_select_requires_link": True},
        "host": {"installed_apps_and_icons": "async_sysutil_FFI",
            "pointer_string_release": "readCString_then_FreeMalloc_in_finally",
            "file_dialog": "Electron_dialog_not_business_DLL",
            "async_error_is_reliable_rejection": False},
    }


def verify_profiles_owner(audit, owner):
    """Recheck a retained complete AST node against the actual original bytes."""
    original = owner["original"]
    raw = audit.read(original)
    start, end = owner["byte_start"], owner["byte_end"]
    if type(start) is not int or type(end) is not int or not 0 <= start < end <= len(raw):
        raise ValueError("Profiles owner byte boundary is invalid")
    node = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
    if (node.start_byte, node.end_byte, node.type) != (start, end, owner["syntax"]):
        raise ValueError("Profiles owner is not a complete original AST node")
    body = raw[start:end]
    if (owner["id"] != original["sha256"] + ":" + str(start) + ":" + str(end)
            or body.decode("utf-8") != owner["body"]
            or hashlib.sha256(body).hexdigest() != owner["body_sha256"]):
        raise ValueError("Profiles owner content differs from the actual original")
    if not isinstance(owner["roles"], list) or not owner["roles"] or len(set(owner["roles"])) != len(owner["roles"]):
        raise ValueError("Profiles owner roles are missing or repeated")
    return node


def verify_profiles_static_claims(report, bodies):
    """Check the reviewed failure/cancellation claims without collecting a report."""
    if report.get("boundaries") != profiles_static_boundaries():
        raise ValueError("Profiles failure, cancellation or result boundary was changed")
    if (report.get("schema_version") != 1 or report.get("query") != "profiles"
            or report.get("source_chain_complete") is not False
            or report.get("selected_static_semantics_reviewed") is not True
            or report.get("product_code_modified") is not False
            or report.get("target_execution") is not False
            or report.get("real_device_operations") is not False
            or report.get("actual_dll_comparison") != "unverified"
            or report.get("rust_independent_implementation") != "unverified"
            or report.get("ui_backend_wiring") != "unverified"
            or report.get("runtime_acceptance") != "unverified"
            or report.get("snapshot_stable") is not True
            or report.get("changed_during_query") != [] or report.get("failures") != []):
        raise ValueError("Profiles receipt promotes static evidence or hides a failed snapshot")
    required = {
        "get_installed_apps": ('if(!t)return;', 'if(!s.length)return;',
            'I.A.getApplicationIcons(JSON.stringify(s),80,80)', 'this.state.refresh&&', ',300)'),
        "browse_game": ('showFileOpenDialog("Open",!0,[v.U0?"app":"exe","url"])',
            'a.forEach(e=>{this.addApp(e)})'),
        "add_app_call": ('this.props.addGame(e).then(', 't.props.close()', '.catch('),
        "add_action": ('filePath:t', 'await r.A.setMemoryStorageItem(', 'catch(o){console.log('),
        "file_dialog_adapter": ('return a?JSON.stringify(a.filePaths):[]',
            'window.showFileOpenDialog(t,e?1:0,o)'),
        "unmount_add_app": ('window.removeEventListener("click",this.handleClick,!1)',
            'this.timeout&&clearTimeout(this.timeout)'),
        "drag_drop": ('e.dataTransfer.files.length>0', 't(e.dataTransfer.files)',
            'e.dataTransfer.clearData()', 'e.removeEventListener("drop",A)',
            'e.addEventListener("drop",A),e.removeEventListener("dragover",f)'),
        "games_add_drop": ('for(const t of e)this.props.addGame(t.path)',),
        "games_refresh": ('this.props.initiateGameScan(),this.props.setSyncing(!0)',),
        "proxy_api": ('case"scanGame":', 'await this.startScan()', 'this.hasPendingScan=!0'),
        "proxy_start_scan": ('this.isScanning=!0', 'n=await this.gmsClient.startScan()',
            'catch(o){', 'return void 0!==t&&t()'),
        "proxy_finished": ('this.isScanning=!1', 'await this.getLocalGameList()',
            'await this.getUserGameList()', 'updateGMSData_MemoryStorage("GMS_Custom_onScanCompleted")',
            'await this.addGameToUserGameList(e.args)'),
        "list_commit": ('isSyncing:!1',),
        "relocate_selection": ('/.*\\.exe/.test(s[0])', 'e.location===s[0]',
            'await this.props.updateRelocateGamePath(e),this.props.close()'),
        "relocate_action": ('e({type:o.BuN,payload:t})',),
        "relocate_reducer": ('relocatedPath:e.payload.path,relocateGameID:e.payload.id',),
        "replace_game": ('e.find(t=>t.location===this.props.path)', '-1!==o&&(',
            'this.props.selectedGameData(a)', 'updateRelocateGamePath({id:"",path:""})',
            't=t.filter(t=>t.id!==n.id)'),
        "bridge_events": ('case C.FB:', 'case C.Rp:', 'this.props.setSyncing(!0)'),
        "host_dialog_dispatch": ('u.showOpenDialog(', '"openFile"', '"multiSelections"'),
        "host_async_dispatch": ('return io.callDLLAsync(n,i)',),
        "host_async_ffi": ('n.readCString()', 'i.FreeMalloc(n)', 'finally{',
            'i[l.action].async(...l.payload.actionArgs,(e,a)=>{n(a)})'),
    }
    for role, fragments in required.items():
        body = bodies[role]
        if any(fragment not in body for fragment in fragments):
            raise ValueError("Profiles reviewed branch differs from its complete owner: " + role)
    forbidden = {
        "add_action": ("throw", "requestId"),
        "browse_game": ("Promise.all", "await this.addApp"),
        "unmount_add_app": ("abort", "cancel", "generation"),
        "drag_drop": (".exe", ".app", ".url"),
        "proxy_api": ('case"stopScan":',),
        "proxy_start_scan": ('isScanning=!1',),
        "relocate_selection": ('addGame', 'setMemoryStorageItem'),
        "relocate_action": ('await', 'addGame', 'setMemoryStorageItem'),
        "bridge_events": ('gms-remote-action-result',),
    }
    for role, fragments in forbidden.items():
        if any(fragment in bodies[role] for fragment in fragments):
            raise ValueError("Profiles recorded absence no longer matches its complete owner: " + role)


def collect_profiles_static(audit):
    """Capture current mounted Games branches and their actual host consumers."""
    report = {"schema_version": 1, "query": "profiles", "recorded_at_utc": audit.a.stamp(),
        "parser": audit.parser_versions, "source_sets": [], "host_sources": [],
        "selected": {}, "cross_chunk_exports": {}, "bindings": {}, "failures": [],
        "source_chain_complete": False, "selected_static_semantics_reviewed": True,
        "product_code_modified": False, "target_execution": False, "real_device_operations": False,
        "actual_dll_comparison": "unverified", "rust_independent_implementation": "unverified",
        "ui_backend_wiring": "unverified", "runtime_acceptance": "unverified",
        "boundaries": profiles_static_boundaries(),
        "remaining_chains": ["SysUtilsNative installed-app enumeration and icon extraction native bodies",
            "All game scanner strategies and every product automatic-profile writer",
            "Independent Rust implementation, mounted UI wiring and automated feature tests",
            "Actual corresponding DLL/service/device comparison and window acceptance"]}
    for name in ("tools/host.py", "tools/workspace.py", "tools/evidence.py", "tools/families.py"):
        audit.artifact(audit.a.ROOT / name)
    deployments = {}
    for name in ("synapse-profiles-current", "gms-proxy-current"):
        source_set, files = audit.source_set(name)
        capture = audit.frontend_capture(name, files)
        manifest = json.loads(audit.artifact(audit.a.ROOT / source_set["capture"]["path"]))
        source_set["complete_capture"] = capture
        source_set["capture_acquisition_relation"] = {
            "capture_recorded_acquisition_sha256": manifest.get("module_recovery", {}).get("original_acquisition_sha256"),
            "current_acquisition_sha256": source_set["acquisition"]["sha256"],
            "current_deployment_original_bytes_reverified": True,
            "capture_metadata_rewritten": False}
        report["source_sets"].append(source_set)
        deployments[name] = files

    def source(files, filename):
        matches = [row for row in files if Path(row["path"]).name == filename]
        if len(matches) != 1:
            raise ValueError("Profiles current source is absent or ambiguous: " + filename)
        return matches[0]

    ui, proxy = deployments["synapse-profiles-current"], deployments["gms-proxy-current"]
    main, page, common = (source(ui, name) for name in (
        "main.776df2b1.js", "9449.8f17b519.chunk.js", "9473.f9ed3133.chunk.js"))
    gms = source(proxy, "main.fd0c50a8.js")
    selected = report["selected"]
    for role, original, needle, kinds in (
        ("api_adapter", main, 'this.getInstalledApplicationList=', {"class_declaration", "class"}),
        ("add_app", common, 'this.handleGetInstalledApp=', {"class_declaration", "class"}),
        ("detail", page, 'this.relocateGame=async', {"class_declaration", "class"}),
        ("games", page, 'this.onRefresh=', {"class_declaration", "class"}),
        ("gms_bridge", main, 'this.gmsEventsListener=', {"class_declaration", "class"}),
        ("list_commit", main, 'pt=(t,e)=>', {"variable_declarator"}),
        ("relocate_reducer", main, 'case u.BuN:', {"switch_case"}),
        ("proxy_api", gms, 'this.handleAPIRequest=async', {"assignment_expression"}),
        ("proxy_start_scan", gms, 'this.startScan=async', {"assignment_expression"}),
        ("proxy_finished", gms, 'this._onGameScanFinishedEvent=async', {"assignment_expression"}),
    ):
        selected[role] = audit.retain("profiles_" + role, original, needle, kinds)[0]
    for role, parent, name in (
        ("get_installed_apps", "add_app", "getInstalledApps"),
        ("browse_game", "add_app", "browseGame"), ("add_app_call", "add_app", "addApp"),
        ("unmount_add_app", "add_app", "componentWillUnmount"),
        ("games_add_drop", "games", "addNewApp"), ("games_refresh", "games", "onRefresh"),
        ("relocate_selection", "detail", "relocateGame"),
        ("replace_game", "gms_bridge", "_replacingGame"),
        ("bridge_events", "gms_bridge", "handleMemoryStorageEvent"),
        ("file_dialog_adapter", "api_adapter", "showFileOpenDialog"),
        ("get_installed_adapter", "api_adapter", "getInstalledApplicationList"),
        ("icons_adapter", "api_adapter", "getApplicationIcons"),
    ):
        owner = audit.owners[selected[parent]]
        original = owner["original"]
        node = audit.tree(original).root_node.descendant_for_byte_range(owner["byte_start"], owner["byte_end"] - 1)
        kwargs = {} if name in {"componentWillUnmount", "showFileOpenDialog"} else {"receiver": "this"}
        _, selected[role] = audit.callable_member("profiles_" + role, original, node, name, **kwargs)
    for role, module, exported in (("add_action", 399, "addGame"),
            ("relocate_action", 399, "updateRelocateGamePath"),
            ("scan_action", 8301, "initiateGameScan"), ("drag_drop", 9902, "A")):
        item = audit.webpack_export("profiles_" + role, ui, module, exported)
        report["cross_chunk_exports"][role] = item
        selected[role] = item["binding"]["owner_id"]
    for role, original, name, parent in (
        ("add_app_api_import", common, "I", "add_app"),
        ("add_app_actions_import", common, "r", "add_app"),
        ("detail_api_import", page, "L", "detail"),
        ("games_drag_import", page, "H", "games"),
        ("bridge_api_import", main, "N", "gms_bridge")):
        reference = audit.owners[selected[parent]]["byte_start"]
        report["bindings"][role] = {"original": audit.sources[original["path"]],
            "reference_byte": reference, **audit.binding("profiles_" + role, original, name, reference)}

    host = verified_host_owners(audit, ("electron/main.js", "electron/modules/sysutil/index.js",
        "electron/modules/sysutil/win/index.js", "electron/modules/sysutil/mac/index.js",
        "electron/modules/ffi/ffiMain.js"))
    for suffix, item in host.items():
        report["host_sources"].append({key: value for key, value in item.items() if key != "body"})
        original = item["original"]
        if suffix == "electron/main.js":
            for role, anchor in (("host_dialog_dispatch", 'case"showFileOpenDialog":'),
                                 ("host_async_dispatch", 'case"getApplicationIconsAlt":')):
                selected[role] = audit.retain("profiles_" + role, original, anchor, {"switch_case"})[0]
        elif suffix.endswith("/ffiMain.js"):
            selected["host_async_ffi"] = audit.retain("profiles_host_async_ffi", original,
                "exports.callDLLMainAsync=", {"assignment_expression"})[0]
        else:
            selected["host_" + suffix] = audit.retain_range("profiles_host_" + suffix, original,
                0, len(audit.read(original)), audit.tree(original).root_node.type)
    report["complete_originals"] = list(audit.sources.values())
    report["complete_source_owners"] = list(audit.owners.values())
    report["verified_artifacts"] = list(audit.artifacts.values())
    report["changed_during_query"] = audit.final_snapshot()
    report["snapshot_stable"] = not report["changed_during_query"]
    report["failures"].extend("Profiles artifact changed during query: " + name
                              for name in report["changed_during_query"])
    return report


def verify_profiles_static_receipt(report, audit=None):
    """Independent read-only receipt check; never invokes the collector."""
    audit = audit or HostLocalAudit()
    if not isinstance(report, dict):
        raise ValueError("Profiles source receipt is absent")
    owners = {}
    for owner in report["complete_source_owners"]:
        verify_profiles_owner(audit, owner)
        if owner["id"] in owners:
            raise ValueError("Profiles source owner is repeated")
        owners[owner["id"]] = owner
    selected = report["selected"]
    bodies = {role: owners[key]["body"] for role, key in selected.items()}
    verify_profiles_static_claims(report, bodies)
    expected_selected = {"api_adapter", "add_app", "detail", "games", "gms_bridge", "list_commit",
        "relocate_reducer", "proxy_api", "proxy_start_scan", "proxy_finished", "get_installed_apps",
        "browse_game", "add_app_call", "unmount_add_app", "games_add_drop", "games_refresh",
        "relocate_selection", "replace_game", "bridge_events", "file_dialog_adapter", "get_installed_adapter",
        "icons_adapter", "add_action", "relocate_action", "scan_action", "drag_drop",
        "host_dialog_dispatch", "host_async_dispatch", "host_async_ffi", "host_electron/modules/sysutil/index.js",
        "host_electron/modules/sysutil/win/index.js", "host_electron/modules/sysutil/mac/index.js"}
    if set(selected) != expected_selected:
        raise ValueError("Profiles selected source owner set differs")
    reviewed = {role: {field: owners[key][field] for field in (
        "original", "syntax", "byte_start", "byte_end", "body_sha256")}
        for role, key in selected.items()}
    # This binds the manual static review to its complete source owners. It is
    # an analysis receipt constraint, never a product/runtime compatibility gate.
    reviewed_sha = hashlib.sha256(json.dumps(reviewed, ensure_ascii=False,
        sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()
    if reviewed_sha != "f4f74a263aec084f7a80a73b3dd1ac546872ab695b63a50543cc5ebccb7c0e91":
        raise ValueError("Profiles selected bytes require a new complete semantic review")
    for role, parent in {"get_installed_apps": "add_app", "browse_game": "add_app",
            "add_app_call": "add_app", "unmount_add_app": "add_app", "games_add_drop": "games",
            "games_refresh": "games", "relocate_selection": "detail", "replace_game": "gms_bridge",
            "bridge_events": "gms_bridge", "file_dialog_adapter": "api_adapter",
            "get_installed_adapter": "api_adapter", "icons_adapter": "api_adapter"}.items():
        child, root = owners[selected[role]], owners[selected[parent]]
        if (child["original"] != root["original"] or not root["byte_start"] <= child["byte_start"]
                < child["byte_end"] <= root["byte_end"]):
            raise ValueError("Profiles callable does not belong to its recorded complete owner")
    expected_sources = {}
    if [row["name"] for row in report["source_sets"]] != ["synapse-profiles-current", "gms-proxy-current"]:
        raise ValueError("Profiles current deployment source sets differ")
    for stored in report["source_sets"]:
        current, files = audit.source_set(stored["name"])
        for field in ("acquisition", "deployment", "capture", "capture_failures", "unresolved_acquisition", "failed_acquisition"):
            if stored[field] != current[field]:
                raise ValueError("Profiles current deployment receipt differs: " + field)
        if stored["complete_capture"] != audit.frontend_capture(stored["name"], files):
            raise ValueError("Profiles reusable full capture identity or failures differ")
        capture = json.loads(audit.artifact(audit.a.ROOT / current["capture"]["path"]))
        relation = stored["capture_acquisition_relation"]
        if relation != {"capture_recorded_acquisition_sha256": capture.get("module_recovery", {}).get("original_acquisition_sha256"),
                "current_acquisition_sha256": current["acquisition"]["sha256"],
                "current_deployment_original_bytes_reverified": True, "capture_metadata_rewritten": False}:
            raise ValueError("Profiles acquisition change was hidden or transferred to the old capture")
        expected_sources.update({row["path"]: row for row in files})
    host = verified_host_owners(audit, ("electron/main.js", "electron/modules/sysutil/index.js",
        "electron/modules/sysutil/win/index.js", "electron/modules/sysutil/mac/index.js",
        "electron/modules/ffi/ffiMain.js"))
    if report["host_sources"] != [{k: v for k, v in item.items() if k != "body"} for item in host.values()]:
        raise ValueError("Profiles host original or official package provenance differs")
    expected_sources.update({row["original"]["path"]: row["original"] for row in host.values()})
    for owner in owners.values():
        source = expected_sources.get(owner["original"]["path"])
        if source is None or any(source[k] != owner["original"][k] for k in ("bytes", "sha256")):
            raise ValueError("Profiles owner is outside its current deployment or official host")
    expected_exports = {"add_action": (399, "addGame"), "relocate_action": (399, "updateRelocateGamePath"),
        "scan_action": (8301, "initiateGameScan"), "drag_drop": (9902, "A")}
    if set(report["cross_chunk_exports"]) != set(expected_exports):
        raise ValueError("Profiles cross-chunk export set differs")
    for role, item in report["cross_chunk_exports"].items():
        original, binding = item["original"], item["binding"]
        factory, exported, target = (owners[item["complete_factory_owner"]],
            owners[item["export_definition_owner"]], owners[binding["owner_id"]])
        if (selected[role] != target["id"] or original != target["original"]
                or factory["original"] != original or exported["original"] != original
                or not factory["byte_start"] <= exported["byte_start"] < exported["byte_end"] <= factory["byte_end"]
                or not factory["byte_start"] <= target["byte_start"] < target["byte_end"] <= factory["byte_end"]):
            raise ValueError("Profiles cross-chunk export owner graph differs")
        resolved = audit.binding("profiles_verify_export_" + role, original, binding["name"], binding["reference_byte"])
        if resolved != binding:
            raise ValueError("Profiles cross-chunk export no longer has its recorded lexical binding")
        factory_node = verify_profiles_owner(audit, factory)
        module_key = factory_node.child_by_field_name("key")
        if ((item["module_id"], item["export"]) != expected_exports[role]
                or module_key is None or module_key.text.decode() != str(item["module_id"])
                or (item["export"] + ":()=>" + binding["name"]) not in exported["body"]):
            raise ValueError("Profiles source module or actual export getter differs")
    expected_imports = {"add_app_api_import": "I=a(7226)", "add_app_actions_import": "r=a(399)",
        "detail_api_import": "L=s(7226)", "games_drag_import": "H=s(9902)",
        "bridge_api_import": "N=n(7226)"}
    if set(report["bindings"]) != set(expected_imports):
        raise ValueError("Profiles producer import set differs")
    for role, binding in report["bindings"].items():
        original = binding["original"]
        expected = {"original": original, "reference_byte": binding["reference_byte"],
            **audit.binding("profiles_verify_" + role, original, binding["name"], binding["reference_byte"])}
        if binding != expected or binding["body"] != expected_imports[role]:
            raise ValueError("Profiles producer import binding differs")
    for artifact in report["verified_artifacts"]:
        audit.artifact(audit.a.ROOT / artifact["path"], artifact)
    recorded = {row["path"]: row for row in report["complete_originals"]}
    if len(recorded) != len(report["complete_originals"]) or recorded != audit.sources:
        raise ValueError("Profiles complete original inventory differs from verified current sources")
    if audit.final_snapshot():
        raise ValueError("Profiles input changed during independent verification")
    return {"complete_source_owners": len(owners), "selected_owners": len(selected),
        "source_sets": 2, "target_execution": False, "runtime_acceptance": "unverified"}


def publish_host_static_query(report, destination, a, query):
    """Update only this query while recording canonical identities of older fields."""
    def identity(value):
        return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
            separators=(",", ":")).encode("utf-8")).hexdigest()
    with a.index_lock(destination.with_suffix(".lock")):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        preserved = {key: identity(value) for key, value in previous.items() if key != query}
        report["preserved_top_level_canonical_sha256"] = preserved
        previous[query] = report
        a.save(destination, previous)
        del previous
        persisted = json.loads(destination.read_bytes())
        if ({key: identity(value) for key, value in persisted.items() if key != query} != preserved
                or persisted[query] != report):
            raise ValueError("Host query publication changed an unrelated top-level field")
    return preserved


def publish_profiles_static(report, destination, a):
    return publish_host_static_query(report, destination, a, "profiles")


def audit_host_profiles(args):
    audit = HostLocalAudit()
    destination = audit.a.ROOT / ".analysis/results/host-current/local-audit.json"
    if getattr(args, "verify_only", False):
        report = json.loads(destination.read_bytes())["profiles"]
    else:
        report = collect_profiles_static(audit)
    verification = verify_profiles_static_receipt(report)
    if not getattr(args, "verify_only", False):
        publish_profiles_static(report, destination, audit.a)
    print(json.dumps({"receipt": audit.a.rel(destination), "query": "profiles",
        "verify_only": bool(getattr(args, "verify_only", False)), **verification}), flush=True)


def lighting_catalog_projection(rows):
    """Project the ordered strict-equality helper over the catalog's full ID domain."""
    if not isinstance(rows, list) or not rows:
        raise ValueError("Device catalog is not a nonempty row array")
    shapes = {"productId": {int}, "dongleId": {int, list}, "bleId": {int},
              "wiredId": {list}, "psModeIds": {list}, "monitorPortIds": {list}, "xBoxId": {int, list}}
    declared, scalar = set(), set()
    for row in rows:
        if not isinstance(row, dict) or "productId" not in row:
            raise ValueError("Device catalog row has no product identity")
        for key, value in row.items():
            if key == "isNotChromaDevice" and type(value) is bool:
                continue
            if key not in shapes or type(value) not in shapes[key]:
                raise ValueError("Device catalog has an unverified field shape: " + key)
            values = value if type(value) is list else [value]
            if not values or any(type(item) is not int or not 0 < item <= 65535 for item in values):
                raise ValueError("Device catalog has an invalid numeric identity: " + key)
            declared.update(values)
            if key in {"productId", "dongleId", "bleId"} and type(value) is int:
                scalar.add(value)
    resolutions = []
    for pid in sorted(declared):
        match = {"input_pid": pid, "primary": pid, "index": None, "matched": "unchanged"}
        for index, row in enumerate(rows):
            field = next((key for key in ("productId", "dongleId", "bleId")
                          if type(row.get(key)) is int and row[key] == pid), None)
            if field is not None:
                match.update(primary=row["productId"], index=index, matched=field)
                break
        resolutions.append(match)
    return {"rows": len(rows), "distinct_primary_pids": len({row["productId"] for row in rows}),
            "all_declared_numeric_id_count": len(declared), "engine_scalar_input_count": len(scalar),
            "engine_changed_input_count": sum(row["input_pid"] in scalar and row["input_pid"] != row["primary"]
                                               for row in resolutions),
            "ordered_resolutions": resolutions}


def audit_lighting_catalog(audit, engine_main):
    observation, files = audit.observed_sources("synapse-device-catalog-current",
        "https://apps.razer.com/synapse/dashboard/AvailableDevices.json")
    main = next(row for row in files if Path(row["path"]).name == Path(engine_main["path"]).name)
    if audit.read(main) != audit.read(engine_main):
        raise ValueError("Observed lighting engine differs from the selected deployment")
    owner_path = audit.a.ROOT / ".analysis/results/frontend/synapse-device-catalog-current/source-owners.json"
    owners, owner_ids = json.loads(audit.artifact(owner_path)), []
    for owner in owners["owners"]:
        matches = [row for row in files if row["path"] == owner["source"]]
        if len(matches) != 1:
            raise ValueError("Catalog source owner is outside the bounded observations")
        original = matches[0]
        start, end = owner["start"], owner["end"]
        node = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
        raw = audit.read(original)[start:end]
        if (node.start_byte, node.end_byte, node.type) != (start, end, owner["type"]) \
                or hashlib.sha256(raw).hexdigest() != owner["body_sha256"] \
                or raw.decode("utf-8") != owner["body"]:
            raise ValueError("Catalog owner does not match its complete source AST")
        owner_ids.append(audit.retain_range("lighting_catalog_" + str(len(owner_ids)),
            original, start, end, node.type))
    original = next(row for row in files if row["url"] ==
        "https://apps.razer.com/synapse/dashboard/AvailableDevices.json")
    projection = lighting_catalog_projection(json.loads(audit.read(original)))
    analysis = json.loads(audit.artifact(audit.a.ROOT /
        ".analysis/results/frontend/synapse-device-catalog-current/catalog-analysis.json"))
    if analysis["input"] != original["path"] or analysis["input_bytes"] != original["bytes"] \
            or analysis["input_sha256"] != original["sha256"]:
        raise ValueError("Catalog projection belongs to different original bytes")
    for key, value in projection.items():
        if key == "ordered_resolutions":
            recorded = [{name: row[name] for name in ("input_pid", "primary", "index", "matched")}
                        for row in analysis["engine_all_declared_id_resolutions"]]
        else:
            recorded = analysis[key]
        if value != recorded:
            raise ValueError("Catalog projection differs from retained analysis: " + key)
    return {"observation": observation, "owner_ids": owner_ids, "catalog": audit.sources[original["path"]],
            **projection, "ordered_equality": "First row with strict scalar productId/dongleId/bleId equality; arrays and other ID fields do not match; no match preserves the input PID",
            "consumer": "DeviceInfo.configure uses vendor5426/1678 and feeds both manifests and middleware lighting SVG paths",
            "cache": "Getter may receive Service Worker DeviceDataJson stale cache and falls back to CacheAPI DeviceDataJson on failure/empty. This observation does not read a live browser cache",
            "runtime_browser_cache_observed": False, "target_execution": False,
            "actual_dll_comparison": "unverified"}


def audit_iot_reconnect(audit):
    import evidence
    observation, _ = audit.observed_sources("synapse-iot-reconnect-current",
        "https://apps.razer.com/synapse/products/780/mw/index.html")
    path = audit.a.ROOT / ".analysis/results/frontend/synapse-iot-reconnect-current/source-owners.json"
    retained = json.loads(audit.artifact(path))
    indexed = {row["path"]: row for row in json.loads(
        audit.artifact(audit.a.ROOT / ".analysis/inputs.json"))["inputs"]}
    sources, deployments, owner_ids = {}, {}, []
    for source in retained["sources"]:
        row = indexed.get(source["path"])
        if row is None or any(row[key] != source[key] for key in ("bytes", "sha256")):
            raise ValueError("IoT reconnect source differs from its indexed original")
        audit.read(row)
        sources[row["path"]] = row
        if row["path"].startswith(".analysis/inputs/frontend/"):
            receipt = source["acquisition"]
            if receipt not in deployments:
                acquired = json.loads(audit.artifact(audit.a.ROOT / receipt))
                if acquired.get("source_current_during_acquisition") is not True:
                    raise ValueError("IoT reconnect deployment has an unverified entry")
                deployments[receipt] = evidence.frontend_deployment_sources(acquired)
            if not any(item["identity_verified"] and item["original"]["path"] == row["path"]
                       for item in deployments[receipt]["sources"]):
                raise ValueError("IoT reconnect owner is not referenced by the current entry")
    host = verified_host_owners(audit, ("electron/modules/ffi/FFIPreloadMain.js", "electron/preload.js"))
    for owner in retained["owners"]:
        original = sources[owner["source"]]
        start, end = owner["start_byte"], owner["end_byte"]
        node = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
        body = audit.read(original)[start:end]
        if (node.start_byte, node.end_byte, node.type) != (start, end, owner["type"]) \
                or body.decode("utf-8") != owner["body"] \
                or hashlib.sha256(body).hexdigest() != owner["body_sha256"]:
            raise ValueError("IoT reconnect source owner differs from its complete AST")
        owner_ids.append(audit.retain_range("iot_reconnect_" + owner["label"], original, start, end, node.type))
    native = []
    for owner in retained["native_owners"]:
        raw = audit.artifact(audit.a.ROOT / owner["source"], {"sha256": owner["capture_sha256"]})
        function = json.loads(raw)
        if function["rva"] != owner["rva"]:
            raise ValueError("IoT reconnect native owner has a different RVA")
        native.append({"artifact": audit.artifacts[owner["source"]], "rva": owner["rva"]})
    return {"observation": observation, "owner_ids": owner_ids, "host_sources": list(host.values()),
            "native_owners": native, "products": retained["products"],
            "static_projections": retained["static_projections"], "findings": retained["findings"],
            "unverified": retained["unverified"], "target_execution": False,
            "actual_dll_comparison": "unverified"}


def audit_deviceinfo_value_sources(audit, main, manifests):
    """Recheck reviewed value sources against complete ASTs and retained manifests."""
    path = audit.a.ROOT / ".analysis/results/frontend/synapse-lighting-engine-current/deviceinfo-value-sources.json"
    retained = json.loads(audit.artifact(path))
    if retained["target_execution"] is not False or retained["real_device_operations"] is not False:
        raise ValueError("DeviceInfo value review has an invalid execution boundary")
    indexed = {row["path"]: row for row in json.loads(
        audit.artifact(audit.a.ROOT / ".analysis/inputs.json"))["inputs"]}
    sources = {}
    for row in retained["sources"]:
        original = indexed[row["path"]]
        if any(row[key] != original[key] for key in ("bytes", "sha256")):
            raise ValueError("DeviceInfo value source differs from current indexed bytes")
        sources[row["path"]] = original
    for row in retained["acquisitions"]:
        audit.artifact(audit.a.ROOT / row["path"], row)
    owners = {}
    for row in retained["owners"]:
        original = sources[row["source"]]
        start, end = row["start_byte"], row["end_byte"]
        body = audit.read(original)[start:end]
        node = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
        if row["source_sha256"] != original["sha256"] or (node.start_byte, node.end_byte, node.type) != (
                start, end, row["type"]) or body.decode("utf-8") != row["body"] \
                or hashlib.sha256(body).hexdigest() != row["body_sha256"]:
            raise ValueError("DeviceInfo value owner differs from its complete original AST")
        if row["id"] in owners:
            raise ValueError("DeviceInfo value owner has a duplicate identity")
        owners[row["id"]] = audit.retain_range(
            "lighting_deviceinfo_value_source", original, start, end, node.type)
    expected_manifests = {row["original"]["path"]: row for row in manifests}
    if {row["source"] for row in retained["manifest_branches"]} != set(expected_manifests):
        raise ValueError("DeviceInfo value review does not cover the retained manifest set")
    for row in retained["manifest_branches"]:
        expected = expected_manifests[row["source"]]
        original = expected["original"]
        body = json.loads(audit.read(original))
        if any(row[key] != original[key] for key in ("bytes", "sha256")) \
                or (row["filename_product_id"], row["edition_id"]) != (
                    expected["product_id"], expected["edition_id"]) \
                or {key: body[key] for key in row["fields"] if key in body} != row["fields"]:
            raise ValueError("DeviceInfo manifest value projection differs from original fields")
    gate_owner = [row for row in audit.owners.values()
                  if "lighting_default_restore_device_names" in row["roles"]]
    if len(gate_owner) != 1:
        raise ValueError("DeviceInfo restore name gate is absent or ambiguous")
    row = gate_owner[0]
    node = audit.tree(main).root_node.descendant_for_byte_range(row["byte_start"], row["byte_end"] - 1)
    value = node.child_by_field_name("right")
    if value is None or value.type != "array" or any(part.type != "string" for part in value.named_children):
        raise ValueError("DeviceInfo restore name gate requires dynamic expression analysis")
    gate = {json.loads(part.text.decode("utf-8")) for part in value.named_children}
    gated = [row for row in retained["manifest_branches"]
             if row["fields"].get("rzDeviceConfig", {}).get("rzDeviceName") in gate]
    counts = {}
    for row in gated:
        name = row["fields"]["rzDeviceConfig"]["rzDeviceName"]
        counts[name] = counts.get(name, 0) + 1
    projection = retained["manifest_projection"]
    if counts != projection["gated_by_protocol"] or len(gated) != projection["gated_originals"] \
            or len(manifests) != projection["retained_originals"]:
        raise ValueError("DeviceInfo per-protocol manifest projection differs")
    for row in retained["retained_engine_keyword_scope"]:
        original = indexed[row["source"]]
        if any(row[key] != original[key] for key in ("bytes", "sha256")):
            raise ValueError("DeviceInfo keyword scan original differs")
        raw = audit.read(original)
        counts = {key: raw.count(key.encode("utf-8")) for key in (
            "quickEffectOnExit", "userQuickEffectOnExit", "getQuickEffect") if key.encode("utf-8") in raw}
        if counts != row["occurrences"]:
            raise ValueError("DeviceInfo keyword scan does not match retained source bytes")
    return {"analysis": audit.artifacts[audit.a.rel(path)], "owner_ids": owners,
            "classes": retained["classes"], "manifest_projection": projection,
            "findings": retained["findings"], "remaining": retained["unresolved"],
            "target_execution": False, "actual_dll_comparison": "unverified"}


def audit_host_local(args):
    if getattr(args, "verify_only", False) and args.query != "profiles":
        raise ValueError("Host-local --verify-only currently applies only to the profiles query")
    if args.query == "profiles":
        return audit_host_profiles(args)
    if args.query == "keypad-mapping":
        return audit_host_keypad_mapping(args)
    if args.query == "mapping-descriptors":
        return audit_host_mapping_descriptors(args)
    if args.query == "window-lifecycle":
        return audit_host_window_lifecycle(args)
    if args.query == "product-chroma-observation":
        return audit_product_chroma_observation(args)
    if args.query == "help-system-info":
        return audit_host_help_system_info(args)
    if args.query == "macro-recorder":
        return audit_host_macro_recorder(args)
    if args.query == "screen-color":
        return audit_host_screen_color(args)
    audit = HostLocalAudit()
    a, families = audit.a, audit.families
    report = {"recorded_at_utc": a.stamp(), "tool": families.identity(Path(__file__)),
              "parser": audit.parser_versions, "source_sets": [], "native": [], "failures": [],
              "source_chain_complete": False, "product_code_modified": False,
              "target_execution": False, "real_device_operations": False, "runtime_acceptance": "unverified"}
    try:
        for tool in ("tools/host.py", "tools/evidence.py", "tools/families.py", "tools/analysis.py", "tools/workspace.py"):
            audit.artifact(a.ROOT / tool)
        settings, ui = audit.source_set("synapse-settings-current")
        engine, mw = audit.source_set("synapse-lighting-engine-current")
        report["source_sets"] = [settings, engine]

        def source(files, needle):
            found = [row for row in files if Path(row["path"]).suffix == ".js" and needle.encode("utf-8") in audit.read(row)]
            if len(found) != 1:
                raise ValueError("Local host source owner is not unique: " + needle)
            return found[0]

        page = source(ui, 'e[2]>=22631')
        main = source(mw, 'async updateRazerWDLDevices()')
        page_ids = audit.retain("dynamic_lighting_settings_widget", page, 'e[2]>=22631', {"variable_declarator"})
        columns_ids = audit.retain("settings_general_mounted_columns", page, 'jsx)(ia,{})', {"variable_declarator"})
        columns = audit.owners[columns_ids[0]]
        report["settings_mounted_parents"] = [audit.binding("settings_parent_" + name, page, name,
            columns["byte_start"]) for name in ("$", "Q", "te", "ee", "mo", "kr", "Vr", "Br", "Hr")]
        page_main = source(ui, '.e(720)')
        audit.retain("settings_current_lazy_loader_and_react_root", page_main, '.e(720)', {"call_expression"})
        audit.retain("settings_current_lazy_loader_complete_callback", page_main, '.e(720)', {"arrow_function"})
        audit.retain("dynamic_lighting_settings_storage_key", page, '="isDynamicLighting"', {"variable_declarator"})
        for role, anchor, kinds in [
            ("engine_dynamic_storage_consumer", "async onStorageChange_dynamicLighting", {"method_definition"}),
            ("engine_dynamic_storage_mutex", "this.onStorageChange_dynamicLighting", {"method_definition"}),
            ("engine_dynamic_device_ownership_transfer", "async updateRazerWDLDevices", {"method_definition"}),
            ("engine_complete_owner_and_lifecycle", "async updateRazerWDLDevices", {"class_declaration", "class"}),
            ("lamparray_complete_event_owner", 'this.setWDLModeToWindowStorage=', {"class_declaration", "class"}),
            ("lamparray_native_init_query_and_cleanup", 'this.isSystemWdlSupported=', {"class_declaration", "class"}),
            ("lamparray_device_protocol_and_result_consumers", 'action:"device_get_brightness"', {"class_declaration", "class"}),
            ("lighting_engine_native_owner_and_lifecycle", 'this.unInitRzLampArray=', {"class_declaration", "class"}),
            ("lighting_native_driver_wrapper", 'this.lightingDriverSkipWdlDevices=', {"class_declaration", "class"}),
            ("lighting_native_startup_lifecycle", 'Promise.resolve((async()=>{const e="LightingEngineModule.initRzLightingEngine()', {"variable_declarator"}),
        ]:
            audit.retain(role, main, anchor, kinds)
        for role, anchor in (("lighting_protocol_initialization", 'this.initLightingDriver=async function(){let t=arguments.length>0&&void 0!==arguments[0]?arguments[0]:"";e.lightingEngine'),
                             ("lighting_device_registration", "this.createLightingDeviceEx=async e=>{let{rzDevice"),
                             ("lighting_normal_device_metadata", "this.addDevice=async function(t){let n=arguments.l"),
                             ("lighting_multilink_device_metadata", "this.addMultiLinkSeqDevice=async function(t,n,i,o)")):
            audit.retain(role, main, anchor, {"assignment_expression"})
        for name in ("configureDevicesDefaultData", "setDeviceToDefaultOnExit"):
            audit.callable_member("lighting_default_restore_producer_" + name, main,
                                  audit.tree(main).root_node, name, receiver="this")
        for role, anchor in (
                ("lighting_default_restore_device_names", "this.resetDefaultDeviceNameList="),
                ("lighting_default_restore_mode_builder", "this.createSetDeviceModeData="),
                ("lighting_default_restore_effect_builder", "this.createSetChromaEffectCommonData="),
                ("lighting_default_restore_effect_parameters", "this.generateSetChromaEffectCommonData=")):
            audit.retain(role, main, anchor, {"assignment_expression"})
        report["lighting_default_restore_headers"] = [
            audit.webpack_export("lighting_default_restore_header_" + key, mw, module_id, key)
            for module_id, key in ((2968, "COMMAND_SET_DEVICE_MODE_HEADER"),
                                   (5668, "COMMAND_SET_CHROMA_EFFECT_HEADER"))]
        for name, start, end in (("checksum", 516122, 516191), ("transaction", 516191, 516287),
                                 ("frame", 516287, 516581)):
            node = audit.tree(main).root_node.descendant_for_byte_range(start, end - 1)
            if node.type != "method_definition" or (node.start_byte, node.end_byte) != (start, end):
                raise ValueError("Current lighting default frame AST boundary differs: " + name)
            audit.retain_range("lighting_default_restore_" + name, main, start, end, node.type)
        report["lighting_default_restore_classes"] = []
        for name, filename, start, end, base_name in (
                ("rzDevice25", "main.a383ea9e.js", 511609, 564815, "i"),
                ("rzDevice25LedMatrix", "3061.825e8b6b.chunk.js", 689, 8065, "a"),
                ("rzDevice25Oled", "7826.cb5be441.chunk.js", 3362, 23590, "n"),
                ("rzDevice25Linker", "4026.605cf6b7.chunk.js", 690, 7437, "r"),
                ("rzDevice25Linker", "3961.b993efcb.chunk.js", 2130, 8877, "r"),
                ("rzDevice25Linker", "2314.1b86b87a.chunk.js", 695, 7442, "n"),
                ("rzDevice25Linker", "8086.08213062.chunk.js", 193746, 200493, "n"),
                ("rzDevice25Glitter", "8648.80c7d0df.chunk.js", 689, 6856, "n"),
                ("rzDevice25Glitter", "1452.f8a38141.chunk.js", 6084, 12251, "r"),
                ("rzDevice25GlitterWireless", "1452.f8a38141.chunk.js", 635, 5935, "r"),
                ("rzDevice25DualLinkMouse", "8508.0c16c096.chunk.js", 700, 9258, "r"),
                ("rzDevice25DualLinkKeyboard", "986.87fa2f57.chunk.js", 698, 8732, "r"),
                ("rzDevice25LedMatrixBatchSender", "8086.08213062.chunk.js", 297537, 300692, "a"),
                ("rzDevice25LedMatrixSkipSetEffect", "269.63b2c328.chunk.js", 618, 1269, "a")):
            matches = [row for row in mw if Path(row["path"]).name == filename]
            if len(matches) != 1:
                raise ValueError("Current default-restore class original is absent or ambiguous: " + filename)
            original = matches[0]
            node = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
            if node.type not in {"class", "class_declaration"} or (node.start_byte, node.end_byte) != (start, end):
                raise ValueError("Current default-restore class AST boundary differs: " + name)
            if node.text.count(('this.name="' + name + '"').encode()) != 1:
                raise ValueError("Current default-restore class identity differs: " + name)
            owner = audit.retain_range("lighting_default_restore_class_" + name, original, start, end, node.type)
            report["lighting_default_restore_classes"].append({"name": name, "owner_id": owner,
                "base_binding": audit.binding("lighting_default_restore_base_" + name, original, base_name, start)})
        report["lighting_default_restore_matrix_contract"] = [
            audit.webpack_export("lighting_default_restore_matrix_" + key, mw, module_id, key)
            for module_id, key in ((1306, "NEW_CHROMA_EFFECT_ID"), (7017, "LED_MATRIX_RGB_EFFECT_ID"),
                                   (7909, "COMMAND_SET_LED_MATRIX_RGB_EFFECT_HEADER"))]
        report["lighting_protocol_constructors"] = []
        for role, filename, start, end in (
            ("base", "main.a383ea9e.js", 1475660, 1478620),
            ("rzDevice25", "main.a383ea9e.js", 511629, 516074),
            ("rzDevice25Chroma65", "334.68120187.chunk.js", 644, 1250),
            ("rzDevice25LedMatrix", "3061.825e8b6b.chunk.js", 715, 5064),
            ("rzDevice25LedMatrixSkipSetEffect", "269.63b2c328.chunk.js", 644, 1268),
            ("rzDevice25Glitter", "8648.80c7d0df.chunk.js", 715, 2649),
            ("rzDevice25GlitterWireless", "1452.f8a38141.chunk.js", 661, 5365),
            ("rzDevice25Glitter_in_wireless_chunk", "1452.f8a38141.chunk.js", 6110, 8044),
            ("rzDevice25MarciT1", "330.d43d2948.chunk.js", 878, 5093),
            ("rzDevice25AudioCamy", "2237.e8b29886.chunk.js", 746, 5727),
            ("rzDeviceIoT", "1794.13beaac0.chunk.js", 690, 3254),
            ("rzDevice25LedMatrixBatchSender", "8086.08213062.chunk.js", 297563, 298327),
            ("rzDeviceLampArray", "main.a383ea9e.js", 1118109, 1126709)):
            found = [row for row in mw if Path(row["path"]).name == filename]
            if len(found) != 1:
                raise ValueError("Current lighting constructor source is absent or ambiguous: " + filename)
            original = found[0]
            constructor = audit.tree(original).root_node.descendant_for_byte_range(start, end - 1)
            if constructor.type != "method_definition" or (constructor.start_byte, constructor.end_byte) != (start, end):
                raise ValueError("Current lighting constructor AST boundary differs: " + role)
            owner = audit.retain_range("lighting_protocol_constructor_" + role, original, start, end, constructor.type)
            report["lighting_protocol_constructors"].append({"protocol": role, "owner_id": owner})
        # Resolve actual local lexical bindings, retaining their complete values
        # instead of substituting another same-named minified module constant.
        widget = audit.owners[page_ids[0]]
        report["settings_dependencies"] = [audit.binding("settings_widget_dependency_" + name, page, name,
            widget["byte_start"]) for name in ("ye", "sa", "xe", "qs", "Oe", "me", "le", "na", "Js", "ue", "Qs", "$s", "re")]
        report["settings_cross_chunk_exports"] = [audit.webpack_export(role, ui, identifier, exported)
            for role, identifier, exported in (("settings_local_storage", 2478, "A"),
                ("settings_sysutil", 4518, "A"), ("settings_copy_keys", 4693, "AXr"),
                ("settings_localized_text", 264, "A"))]
        keys = ("AXr", "j4s", "jiX", "He3", "a5R", "k6W", "LRm", "doU", "SRS")
        report["settings_copy_exports"] = [audit.webpack_export("settings_copy_" + key, ui, 4693, key) for key in keys]
        report["settings_translation_lifecycle"] = [audit.webpack_export("settings_translation_" + key, ui, 1778, key)
            for key in ("getTextItem", "getLanguageItem", "swapLang", "updateLanguage")]
        report["settings_translation_importer"] = audit.webpack_export("settings_translation_importer", ui, 7248, "default")
        copy_keys = ("DEVICE_LIGHTING", "DEVICE_LIGHTING_TIPS", "CHROMA_RGB", "DYNAMIC_LIGHTING",
                     "DYNAMIC_LIGHTING_TITLE", "CHROMA_RGB_TITLE", "DYNAMIC_LIGHTING_MSG", "CHROMA_RGB_MSG",
                     "OPEN_WINDOWS_DYNAMIC_LIGHTING")
        report["settings_locale_exports"] = [{"locale": locale, "module_id": module_id,
            "entries": [audit.webpack_export("settings_locale_" + locale + "_" + key, ui, module_id, key)
                        for key in copy_keys]}
            for locale, module_id in (("en", 2221), ("de", 7493), ("es", 1418), ("fr", 4741),
                                      ("ja", 3227), ("kr", 3415), ("pt-BR", 9351), ("ru", 391),
                                      ("zh-CN", 6368), ("zh-TW", 2408))]
        report["lighting_ffi_exports"] = [audit.webpack_export("lighting_ffi_" + key, mw, 4407, key)
                                          for key in ("Qq", "oE", "yL", "Vj")]
        report["current_css"] = [{"original": row, "body": audit.read(row).decode("utf-8")}
            for row in ui if Path(row["path"]).suffix == ".css"]
        indexed = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
        report["lighting_restore_device_info_owners"] = []
        for role, start, end in (
                ("complete_module", 1281275, 1358467), ("complete_class", 1307479, 1358421),
                ("constructor", 1307487, 1328958), ("configure", 1307512, 1319984),
                ("prepare_device", 798037, 800387), ("create_device_info", 820622, 821045),
                ("manifest_selection", 1330761, 1333740), ("partner_manifest", 1328958, 1330761),
                ("manifest_fetch", 1333740, 1334329), ("manifest_idb_read", 1355596, 1356168),
                ("manifest_idb_write", 1356168, 1356677), ("available_devices", 796031, 796548),
                ("primary_product", 1072572, 1072716), ("exit_target_assignment", 1310886, 1310925),
                ("local_profile_read", 1337557, 1340073), ("ui_effect", 1334912, 1335556),
                ("software_effect_sync", 825229, 826331), ("region_effect_init", 821866, 823011),
                ("device_ready", 800388, 820621), ("snapshot_gate", 814211, 814505),
                ("snapshot_helper", 731819, 732880), ("snapshot_getter", 548800, 548901),
                ("hardware_effect_setter_exception_consumer", 730954, 731819),
                ("snapshot_encoder", 1091719, 1092237), ("snapshot_parser", 1091425, 1091718),
                ("snapshot_header", 760197, 760226), ("initial_snapshot", 1327965, 1327998),
                ("dynamic_context", 1186068, 1186476), ("unused_info_message", 913087, 913381)):
            node = audit.tree(main).root_node.descendant_for_byte_range(start, end - 1)
            if (node.start_byte, node.end_byte) != (start, end):
                raise ValueError("Current restore DeviceInfo AST boundary differs: " + role)
            report["lighting_restore_device_info_owners"].append(audit.retain_range(
                "lighting_restore_device_info_" + role, main, start, end, node.type))
        default_rows = [row for row in mw if Path(row["path"]).name == "4238.286b7bbe.chunk.js"]
        if len(default_rows) != 1:
            raise ValueError("Current PID zero manifest module is absent or ambiguous")
        audit.retain("lighting_pid_zero_manifest_only", default_rows[0], "quickEffectOnExit:",
                     {"variable_declarator"})
        manifest_rows = [row for row in indexed["inputs"] if row["path"].startswith(
            ".analysis/inputs/frontend/") and "-current/" in row["path"]
            and re.search(r"/DeviceManifest_\d+_\d+\.json$", row["path"])]
        if not manifest_rows:
            raise ValueError("Current retained product restore manifests are absent")
        manifest_receipts, restore_manifests = {}, []
        fields = ("version", "productId", "quickEffectOnExit", "rzDeviceConfig", "useUserQuickEffectOnExit",
                  "syncHWEffectWithSWEffectWhenExit", "skipQuickEffectOnExit", "keepSynapseQuickEffectOnExit",
                  "isHardwareEffectsOnly", "hasPowerButton", "supportBatteryLevelEffectOnExit")
        for original in manifest_rows:
            receipt_name = str(Path(*Path(original["path"]).parts[:4]) / "acquisition.json").replace("\\", "/")
            if receipt_name not in manifest_receipts:
                manifest_receipts[receipt_name] = json.loads(audit.artifact(a.ROOT / receipt_name))
            acquired = [row for row in manifest_receipts[receipt_name]["files"]
                        if row.get("path") == original["path"]]
            if len(acquired) != 1 or any(acquired[0].get(key) != original[key] for key in ("bytes", "sha256")):
                raise ValueError("Retained restore manifest differs from its acquisition: " + original["path"])
            body = json.loads(audit.read(original))
            pid, edition = map(int, re.search(r"/DeviceManifest_(\d+)_(\d+)\.json$", original["path"]).groups())
            if not isinstance(body, dict) or type(body.get("productId")) is not int:
                raise ValueError("Restore manifest has no integer product identity: " + original["path"])
            for field in fields[4:]:
                if field in body and type(body[field]) is not bool:
                    raise ValueError("Restore manifest branch has a non-boolean flag: " + field)
            restore_manifests.append({"original": original, "acquisition": receipt_name,
                "product_id": pid, "declared_product_id": body["productId"], "edition_id": edition,
                "filename_matches_declared_product": pid == body["productId"],
                "fields": {key: body[key] for key in fields if key in body}})
        report["lighting_restore_manifest_scope"] = {
            "retained_originals": restore_manifests,
            "physical_inputs": len(restore_manifests), "acquisition_sets": len(manifest_receipts),
            "product_editions": len({(row["product_id"], row["edition_id"]) for row in restore_manifests}),
            "products": len({row["product_id"] for row in restore_manifests}),
            "filename_declared_pid_differences": [{"original": row["original"],
                "filename_product_id": row["product_id"], "declared_product_id": row["declared_product_id"],
                "edition_id": row["edition_id"]} for row in restore_manifests
                if not row["filename_matches_declared_product"]],
            "device_identity_consumer": "DeviceInfo.configure retains the productId from discovered arguments and does not assign C.productId. A filename/declaration difference is retained explicitly; this query does not invent a filename-equality rule absent from that source owner",
            "all_official_products_covered": False, "latest_deployment_rechecked_this_query": False,
            "runtime_manifest_cache_read": False, "actual_dll_comparison": "unverified"}
        report["lighting_catalog"] = audit_lighting_catalog(audit, main)
        report["iot_reconnect"] = audit_iot_reconnect(audit)
        report["deviceinfo_value_sources"] = audit_deviceinfo_value_sources(audit, main, restore_manifests)
        report["settings_media"] = []
        for media in ("exclamation_icon.53d6d40b.svg", "wdl_large_icon.e610fd8c.svg",
                      "tooltip_questionmark.96138d2f.svg", "spinner.ef2d0235.svg"):
            found = [row for row in indexed["inputs"] if row["path"].startswith(
                ".analysis/inputs/frontend/synapse-settings-current/") and row["path"].endswith("/" + media)]
            if len(found) != 1:
                raise ValueError("Current settings media identity is absent or ambiguous: " + media)
            report["settings_media"].append({"original": found[0], "body": audit.read(found[0]).decode("utf-8")})
        host_rows = [row for row in indexed["inputs"] if row["path"].startswith(".analysis/inputs/host-current/asar/")]
        host_sources = []
        for suffix in ("electron/modules/LampArray/LampArrayAction.js", "electron/modules/ffi/FFIPreloadMain.js",
                       "electron/modules/ffi_subprocess/FFIPreloadSubProcess.js", "electron/modules/ffi/ffiMain.js",
                       "electron/modules/lighting/ffiLightingDriver.js",
                        "electron/modules/sysutil/win/index.js", "electron/main.js", "electron/UsbRzDeviceAction.js",
                        "electron/Protocol/protocol25.js", "electron/Protocol/protocol25Const.js",
                        "electron/modules/IoT/IoTNativeAction.js", "node_modules/node-rz-hid/nodehid.js",
                        "node_modules/node-rz-hid/package.json"):
            found = [row for row in host_rows if row["path"].endswith("/" + suffix)]
            if len(found) != 1:
                raise ValueError("Current host JS original is absent or ambiguous: " + suffix)
            row = found[0]
            host_sources.append({"original": row, "body": audit.read(row).decode("utf-8")})
            if suffix == "electron/main.js":
                audit.retain("lighting_host_final_writer_dispatch", row, "to.onWriteCallback=", {"assignment_expression"})
            elif suffix == "electron/UsbRzDeviceAction.js":
                for action in ("hid.removeDevice", "hid.sendFeatureReportInBatch", "hid.setDevicesToDefault",
                               "hid.configureDevicesDefaultData", "hid.setDevicesToDefaultNoAsync",
                               "hid.startLightingFramesInElectronMain", "hid.stopLightingFramesInElectronMain",
                               "hid.writeAsync"):
                    audit.retain("lighting_host_" + action, row, 'case"' + action + '":', {"switch_case"})
                audit.callable_member("lighting_host_writeAsync", row, audit.tree(row).root_node, "writeAsync")
                audit.retain_class_field("lighting_host_default_restore_on_exit", row,
                                         "handle_hid_setDevicesToDefaultNoAsync")
                audit.retain_class_field("lighting_host_default_restore_sender", row,
                                         "setDevicesToDefaultNoAsyncExec")
            elif suffix in ("electron/Protocol/protocol25.js", "electron/Protocol/protocol25Const.js"):
                audit.retain("lighting_default_restore_" + Path(suffix).stem, row,
                             "module.exports=", {"assignment_expression"})
        report["complete_host_sources"] = host_sources
        hid_rows = [row for row in host_rows if row["path"].endswith("/node-rz-hid/build/Release/HID.node")]
        if len(hid_rows) != 1:
            raise ValueError("Current lighting HID native input is absent or ambiguous")
        encoding_path = a.ROOT / ".analysis/results/native" / hid_rows[0]["sha256"] / "utf16-semantic-audit.json"
        encoding = json.loads(audit.artifact(encoding_path))
        if any(encoding["original"][key] != hid_rows[0][key] for key in ("bytes", "sha256")) \
                or encoding["byte_validation"]["errors"]:
            raise ValueError("HID encoding analysis is not bound to current original bytes")
        encoding_functions = [row for group in ("successful_supplement_functions", "supporting_base_callers")
                              for row in encoding["artifacts"][group]]
        for row in encoding_functions:
            audit.artifact(a.ROOT / row["artifact"]["path"], row["artifact"])
        encoding_rvas = tuple(int(row["rva"], 16) for row in encoding_functions)
        report["lighting_hid_encoding_review"] = {
            "analysis": audit.artifacts[a.rel(encoding_path)], "capture_states": encoding["capture_states"],
            "semantics": encoding["semantics"], "caller_proof": encoding["caller_proof"],
            "instruction_derived_cases": encoding["instruction_derived_cases"],
            "target_execution": False, "actual_dll_comparison": "unverified"}
        report["lighting_hid_native"] = audit.native_slice(hid_rows[0], (), depth=0,
            rvas=(0xc8d0, 0x5280, 0x45d0, 0x50e0, 0x4800, 0x16370, 0x13700, 0x165f0,
                  0x6170, 0x53a0, 0x94e0, 0x4710, 0xeaf0, 0x15ad0, 0x15fe0, 0x160e0,
                  0x17a10, 0x175e0, 0x17de0, 0x161e0, 0xcc90, 0x9750, 0x15d40, 0xc600) + encoding_rvas,
            regions=({"role": "hid_batch_feature_writer_vtable", "rva": 0x51c60, "bytes": 24},
                     {"role": "utf16_encoding_runtime_function", "rva": 0x6170c, "bytes": 12},
                     {"role": "utf16_encoding_unwind_info", "rva": 0x539a0, "bytes": 8}))
        acquisition = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
        host_acquired = json.loads(audit.artifact(acquisition))
        report["host_acquisition"] = families.identity(acquisition)
        packages = [row for row in host_acquired["files"]
                    if row.get("declaration", {}).get("resourceName") == "RazerWDLMsix"]
        if len(packages) != 1:
            raise ValueError("Current lighting package identity is absent or ambiguous")
        package = packages[0]
        declaration = package["declaration"]
        if (declaration.get("sha256", "").lower(), declaration.get("size")) != (package["sha256"], package["bytes"]):
            raise ValueError("Current lighting package differs from its resource declaration")
        import xml.etree.ElementTree as ET
        try:
            with zipfile.ZipFile(io.BytesIO(audit.read(package))) as archive:
                names = archive.namelist()
                if len(names) != len(set(names)):
                    raise ValueError("Lighting package has duplicate member identities")
                members = {name: archive.read(name) for name in names}
            manifest = ET.fromstring(members["AppxManifest.xml"])
            blockmap = ET.fromstring(members["AppxBlockMap.xml"])
        except (zipfile.BadZipFile, ET.ParseError) as error:
            raise ValueError("Lighting package container or XML is invalid") from error
        blockmap_ns = "{http://schemas.microsoft.com/appx/2010/blockmap}"
        if blockmap.get("HashMethod") != "http://www.w3.org/2001/04/xmlenc#sha256":
            raise ValueError("Lighting package uses an unreviewed block hash method")
        verified_blocks = []
        for item in blockmap.findall(blockmap_ns + "File"):
            name = item.attrib["Name"].replace("\\", "/")
            data = members[name]
            blocks = item.findall(blockmap_ns + "Block")
            if int(item.attrib["Size"]) != len(data) or len(blocks) != (len(data) + 65535) // 65536:
                raise ValueError("Lighting package block map differs from member boundaries")
            for ordinal, block in enumerate(blocks):
                part = data[ordinal * 65536:(ordinal + 1) * 65536]
                if base64.b64decode(block.attrib["Hash"], validate=True) != hashlib.sha256(part).digest():
                    raise ValueError("Lighting package block hash differs from member bytes")
                verified_blocks.append({"member": name, "ordinal": ordinal, "bytes": len(part),
                                        "sha256": hashlib.sha256(part).hexdigest()})
        ns = {"p": "http://schemas.microsoft.com/appx/manifest/foundation/windows10"}
        identity = manifest.find("p:Identity", ns)
        applications = manifest.findall("p:Applications/p:Application", ns)
        if identity is None or len(applications) != 1 or not verified_blocks:
            raise ValueError("Lighting package manifest lacks its reviewed identity or application")
        report["native_package_manifest"] = {
            "original": package, "members": [{"name": name, "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(), **({"body": data.decode("utf-8")}
                if name.lower().endswith(".xml") else {})} for name, data in members.items()],
            "identity": identity.attrib, "application": applications[0].attrib,
            "verified_blocks": verified_blocks, "signature_validation": "not_executed",
            "target_execution": False}
        rpc_originals = {}
        for resource, export_names, string_keys, extra in [
            ("IoTNative", [], [], {"depth": 0, "rvas": (
                0x346e0, 0x34170, 0x34c60, 0x37590, 0x36da0, 0x1250, 0x2dc0, 0x1900,
                0x1050, 0x2720, 0x3c400, 0x48fe0, 0x490e0, 0x48f90, 0x37140, 0x14550,
                0x35b40, 0x39870, 0x39bb0, 0x15b0, 0x1cc0, 0x6e1b0, 0x38690, 0x359c0,
                0x34e60, 0x37710, 0x59370, 0x3a350, 0x426e0,
                0x34f40, 0x35400, 0x35460, 0x35590, 0x35710, 0x35940, 0x3c900, 0x49190, 0x49220),
                "regions": ({"role": "sdk_load_filename_utf16", "rva": 0x74580, "bytes": 66},
                            {"role": "queued_connect_function_vtable", "rva": 0x767f0, "bytes": 40})}),
            ("Razer_IoT_SDK", [], [], {"depth": 0, "rvas": (
                0xad17, 0x26dd20, 0x6e97, 0x25cc60, 0x5812, 0x250de0, 0xc284, 0x36d5b0,
                0xb460, 0x247ef0, 0x3e45, 0x247920, 0xbf28, 0x227050, 0x5006, 0x264870,
                0xa46b, 0x2637d0, 0x1244d0, 0x2158, 0x6938, 0x476b0, 0x379c, 0x1f4dc0,
                0x74c8, 0x166620, 0x1d1110, 0x16c260, 0x29cb0, 0x1ef90, 0x263670, 0x242470,
                0x252420, 0x2532b0, 0x248c40, 0x259010, 0xb66d, 0xa6c3, 0x835a, 0x243e50,
                0x7f90, 0xaefc, 0x2676f0, 0x82e2, 0x247510, 0x8b89, 0x243060, 0x89c7,
                0x2438c0, 0x18e8, 0x24bac0, 0x5a1a, 0x24a9d0, 0x6d75, 0x26d720, 0x19ce,
                0x249b20, 0xbf73, 0x250fd0, 0x5551, 0x262fb0, 0x6bdb, 0x244830, 0xc1d0,
                0x244620, 0xa420, 0x251300, 0x5ac9, 0x2420c0, 0x2517c0, 0x24acf0, 0xb302,
                0x259190, 0x4ce1, 0x24bd30, 0x2923, 0x2410c0, 0x24ef70, 0x11ae, 0x13b830,
                0xb62c, 0x26d7a0,
                0x6181, 0xa678, 0x251490, 0xa010, 0x23f950, 0x5556, 0x2964ec, 0x76b7,
                0x242db0, 0xb7fd, 0x2414e0, 0xb753, 0x2968fc, 0xbdcf, 0x296840, 0x146a,
                0x251620, 0x19ec, 0x26d030, 0xbaaf, 0x26d830, 0x246ca0, 0x3d65d0,
                0x3ff30, 0x3ffe0, 0x2b8f, 0x25a100, 0x2374, 0x23f860, 0x6ab9,
                0x23e390, 0x6721, 0x4fc5, 0x255630, 0x3a03, 0x2541d0, 0x6230,
                0x22bcb0, 0xaeac, 0x2600d0, 0x956b, 0x261480, 0xae93, 0x2626a0,
                0xb29e, 0x13ba30, 0x7fd1, 0x23b950, 0x8076, 0x2442a0, 0x695b,
                0x244270, 0x419c, 0x244290, 0x1050, 0x244250, 0x58e4, 0x243e00,
                0xa0b5, 0x2430e0, 0x8909, 0x243ce0, 0x9f20, 0x243ca0, 0x13bfc0,
                0x13c210, 0x710d, 0xb3c5, 0x13bd70, 0x13bea0, 0x9a7a, 0x2d0b,
                0x1a96c0, 0x1eb380, 0x1742a0, 0x7f13, 0x1ff8e0, 0xb3d9,
                0x295ab0, 0xbe79, 0x32a208, 0x734c, 0x324df0,
                0x1573, 0x2315, 0x2a09, 0x2b4e, 0x2bcb, 0x317f, 0x398b, 0x3aad,
                0x3b57, 0x3bed, 0x4b06, 0x4ec6, 0x623f, 0x6244, 0x6f73, 0x8148,
                0x8666, 0x86f2, 0x87e2, 0x88c3, 0x8c5b, 0x8e0e, 0x8f3a, 0x93a9,
                0x98db, 0x9c5f, 0x9ddb, 0x9e44, 0xb3f2, 0xb497, 0xbccb, 0x47a70,
                0x48c00, 0xb4350, 0x239880, 0x23c0c0, 0x23d270, 0x23d390, 0x23e320,
                0x23ef40, 0x23f7e0, 0x23fa00, 0x240ea0, 0x242630, 0x242790, 0x2481c0,
                0x24b710, 0x24f1b0, 0x24fdf0, 0x250100, 0x2503e0, 0x2507a0, 0x2508b0,
                0x250ae0, 0x250cc0, 0x251260, 0x2536b0, 0x258610, 0x264b60, 0x2672f0,
                0x267700, 0x267910, 0x26d430, 0x26d7b0, 0x26df20, 0x26df40, 0x296294,
                0x296440, 0x2d3c40),
                "regions": ({"role": "socket_primary_vtable", "rva": 0x3ebb28, "bytes": 24},
                            {"role": "socket_secondary_vtable", "rva": 0x3ebb50, "bytes": 40},
                            {"role": "v25_frame_vtable", "rva": 0x3df6a0, "bytes": 24},
                            {"role": "frame_command_class_first", "rva": 0x45824c, "bytes": 4},
                            {"role": "frame_command_class_second", "rva": 0x4582d8, "bytes": 4},
                            {"role": "legacy_frame_command_name", "rva": 0x3e1450, "bytes": 16},
                            {"role": "v2_frame_command_name", "rva": 0x3e1530, "bytes": 17},
                            {"role": "sdk_version_literal", "rva": 0x3ed520, "bytes": 14},
                            {"role": "global_callback_object_vptr", "rva": 0x45e2a8, "bytes": 8},
                            {"role": "global_callback_vtable", "rva": 0x3eb8f8, "bytes": 8},
                            {"role": "socket_shared_control_vtable", "rva": 0x3ed5a8, "bytes": 24},
                            {"role": "socket_callback_shared_control_vtable", "rva": 0x3ed5d8, "bytes": 24},
                            {"role": "socket_callback_vtable", "rva": 0x3ec010, "bytes": 8},
                            {"role": "v25_keepalive_class", "rva": 0x4581e8, "bytes": 4},
                            {"role": "wled_keepalive_class", "rva": 0x4581f0, "bytes": 4},
                            {"role": "mdns_callback_object_vptr", "rva": 0x45da70, "bytes": 8},
                            {"role": "mdns_callback_vtable", "rva": 0x3eb8d8, "bytes": 8},
                            {"role": "mdns_record_callback_jump", "rva": 0x13c0, "bytes": 5},
                            {"role": "mdns_identity_markers", "rva": 0x3eb5d8, "bytes": 64},
                            {"role": "mdns_service_name", "rva": 0x3ec4f0, "bytes": 16},
                            {"role": "mdns_empty_substring", "rva": 0x3dcbf6, "bytes": 1})}),
            ("LightingDriverDLL", ["Configure", "ConfigureInMem", "Startup", "Shutdown", "FreeString"], ["skip_wdl_devices", "mode.set"],
                {"rvas": (0x5f40, 0x6b90, 0x5a00, 0xb790, 0xf4d0, 0x2e0d0, 0x30610,
                          0x2e500, 0x9240, 0x2edd0, 0x2ee70, 0x2ef10, 0x2efb0, 0x2f050, 0x2f0f0,
                          0x2f190, 0x2f230, 0x2f360, 0xa770, 0xdbd0, 0xdc80, 0x9920, 0xdc90, 0xdd40,
                          0x8b10, 0xc7b0, 0x2ce40, 0xdd90, 0x9730, 0xb310, 0xfed0, 0xefb0, 0xe250,
                          0xea90, 0x7feb0, 0x80460, 0x7fc30, 0x7fd70, 0x80a00, 0x80f90, 0x811b0,
                          0x81070, 0x85340, 0x85610, 0x85930, 0x85d20, 0x85fa0, 0xa100, 0x7ec90,
                          0x7ebf0, 0x7ec10, 0x7e220, 0x7e120, 0x7e2e0, 0x7e4e0, 0x7e430, 0x7e710,
                          0x2e0e0, 0x2ed70, 0x2f520, 0x2f570, 0x2f6c0, 0x83910, 0x2e4f0, 0x2550),
                 "regions": ({"role": "configure_type_dispatch", "rva": 0xafc30,
                    "bytes": 96, "decode": "pointer_pairs"},) + tuple(
                     {"role": role + "_translator_vtable", "rva": rva, "bytes": 16}
                     for role, rva in (("rzDevice25", 0xb3968), ("rzDevice25Chroma65", 0xb3b58),
                         ("rzDevice25LedMatrix", 0xb3c68), ("rzDevice25LedMatrixBatchSender", 0xb3d78),
                         ("rzDevice25Glitter", 0xb3e88), ("rzDevice25MarciT1", 0xb3f98),
                         ("rzDevice25AudioCamy", 0xb40a8), ("rzDeviceIoT", 0xb41b8),
                         ("rzDeviceLampArray", 0xb42c8))) + tuple(
                     {"role": role + "_report_length_u64_le", "rva": rva, "bytes": 8}
                     for role, rva in (("Glitter", 0xb3778), ("GlitterWireless", 0xb3798), ("MarciT1", 0xb37c0)))}),
            ("RzLampArrayDLL", ["SetupRPC", "Startup", "Shutdown", "JsonApi", "JsonApiNoReturn", "FreeMalloc", "GetDLLVersion",
                "StartWatcher", "StopWatcher", "SetNodeFFIEvent", "GetLightingSettings", "GetControllerFilePath", "SetDeviceInfoToFile"],
                ["device_get_all", "rpc::LampRpcClient::JsonApi", "rpc::LampRpcClient::InitRpc",
                 "rpc::LampRpcClient::ExecuteRpcServer", "rpc::LampRpcClient::StartRpcServer"],
                {"rvas": (0xa4340, 0x7d510, 0x7e580, 0x7e3e0, 0x7e720, 0x7dd70, 0x7c5e0,
                          0x7c820, 0x7c0f0, 0x7df10, 0x7c9b0, 0x7b4d0, 0x7d450, 0x4499, 0xa40a0, 0x7cf10,
                          0xa44a0, 0xa4530, 0xa4460, 0x7b220, 0x7b470, 0xa4410,
                          0x733a0, 0x96160, 0x2a180, 0x147a98, 0x147c34, 0x12f3,
                          0x36120, 0x377f0, 0x168790, 0x2a220, 0x2a6a0, 0x2ac60,
                          0x119360, 0x118ff0, 0x119300, 0x119610, 0x118d80, 0x118650,
                          0x118ad0, 0x113f10, 0x116eb0, 0x115100, 0x117c30,
                          0x116d30, 0x98f60),
                 "regions": ({"role": "controller_file_xor_key_and_string_layout", "rva": 0x1a42c0,
                              "bytes": 32},)}),
            ("RazerWDLExe", [], ["wdlrpc::WdlRpcServer::RunForever", "device_get_all", "device_close",
                "effect_set_colors_for_indices", "JsonApi data:{}", "JsonApi failed, error"],
                {"rvas": (0x79e40, 0x82c90, 0x83390, 0x83e90, 0x831c0, 0x85fe0, 0x82070, 0x88280,
                          0x874c0, 0x87900, 0x87eb0, 0x85a50, 0x84860, 0x158c60, 0x1585d0, 0x1584f0,
                          0x14f060, 0x14f990, 0x13a6c0, 0x1547b0, 0x154680, 0x88410, 0x883f0,
                          0x159be0, 0x159f00, 0x85cf0, 0x837a0, 0x84390, 0x83b00, 0x84630, 0x88600,
                          0x1530c0, 0x152bc0, 0x14f920, 0x157530, 0x156f60, 0x14f8c0,
                          0x1537a0, 0x153230, 0x153180, 0x156f30, 0x156a90, 0x157560,
                          0x74480, 0x75120, 0x75500, 0x74960, 0x74d40, 0x63ea0,
                          0x14fd90, 0x14ed70, 0x156760, 0x159b80, 0x159dd0, 0x159e40,
                          0x153d40, 0x3db60, 0x3dba0, 0x7dbd0, 0x14cbb0, 0x14c920,
                          0x13dc20, 0x14beb0, 0x22c00, 0x23cb20,
                          0x155750, 0x154d30, 0x152b30, 0x15f8c0, 0x148af0, 0x148b20,
                          0x153880, 0x152340, 0x159680, 0x14f4b0, 0x154580, 0x150990,
                          0x148780, 0x14ae70, 0x154820, 0x13fb50, 0x13f9c0,
                          0x14f830, 0x153930, 0x153990, 0x1539f0, 0x154c20,
                          0x1548c0, 0x154920, 0x1537f0, 0x1562f0, 0x156410,
                          0x3fbc0, 0x46480, 0x46580,
                          0x1318c0, 0xf5960, 0xf3d60, 0x138850, 0xf6b70, 0x138c40, 0x138d20,
                          0x1382f0, 0x1334d0, 0x133130, 0x1335a0, 0x1332a0, 0x1381a0,
                          0x135150, 0x137f30, 0x1352f0, 0x1351e0, 0x138220, 0x135030, 0x138680,
                          0x161d30, 0x161b20, 0x161c10, 0x160e80,
                          0xe3290, 0xf3190, 0xf71d0, 0x341f0, 0x37570, 0xad0f0,
                          0xf4730, 0xf2c20, 0xad140, 0xf31e0, 0xf7140, 0x35c70, 0x37a60,
                          0x14fe20, 0x150f10, 0x2329, 0xa0d3,
                          0x13b3f0, 0x13b430, 0x13b0c0, 0x13b0f0, 0x147860, 0x147910,
                          0x153ac0, 0x154ca0, 0x154740, 0x156380, 0x155c30, 0x1557a0,
                          0x1544c0, 0x154500, 0x13b840, 0x13b8c0, 0x14cb20,
                          0x153b50, 0x15f4b0, 0x15f5e0, 0x10023, 0x10a0a),
                 "regions": ({"role": "json_key_virtual_key", "rva": 0x24467c, "bytes": 4},
                             {"role": "json_key_brightness", "rva": 0x2446b4, "bytes": 7},
                             {"role": "brightness_unit_limit_f64_le", "rva": 0x246ca0, "bytes": 8},
                             {"role": "brightness_percent_scale_f64_le", "rva": 0x3052a0, "bytes": 8},
                             {"role": "lamp_availability_interface_iid", "rva": 0x304130, "bytes": 16},
                             {"role": "add_package_options_iid", "rva": 0x24b880, "bytes": 16},
                             {"role": "deployment_result2_iid", "rva": 0x24b898, "bytes": 16},
                             {"role": "package_manager_iid", "rva": 0x24b8b0, "bytes": 16},
                             {"role": "package_manager9_iid", "rva": 0x24b8c8, "bytes": 16},
                             {"role": "uri_activation_factory_pointer", "rva": 0x3864d8, "bytes": 8},
                             {"role": "package_options_activation_factory_pointer", "rva": 0x3864e8, "bytes": 8},
                             {"role": "package_manager_activation_factory_pointer", "rva": 0x3864f8, "bytes": 8},
                             {"role": "uri_runtime_class_utf16", "rva": 0x24b6a8, "bytes": 46},
                             {"role": "package_options_runtime_class_utf16", "rva": 0x24b790, "bytes": 96},
                             {"role": "package_manager_runtime_class_utf16", "rva": 0x24b810, "bytes": 90},
                             {"role": "session_mutex_prefix_ascii", "rva": 0x248d58, "bytes": 14},
                             {"role": "session_rpc_endpoint_format_ascii", "rva": 0x240328, "bytes": 15},
                             {"role": "hidden_window_identity_utf16", "rva": 0x246f80, "bytes": 18},
                             {"role": "watcher_added_delegate_vtable", "rva": 0x305190, "bytes": 32},
                             {"role": "watcher_removed_delegate_vtable", "rva": 0x3051f0, "bytes": 32},
                             {"role": "discovered_instance_id_property_utf16", "rva": 0x304bb0, "bytes": 62},
                             {"role": "discovered_vendor_marker_utf16", "rva": 0x304c30, "bytes": 10},
                             {"role": "discovered_product_marker_utf16", "rva": 0x304c40, "bytes": 10},
                             {"role": "discovered_container_property_utf16", "rva": 0x304cc0, "bytes": 54})}),
        ]:
            matches = [row for row in host_acquired["files"] if row.get("resourceName") == resource]
            if not matches:
                matches = [row for row in host_acquired["files"] if row.get("declaration", {}).get("resourceName") == resource]
            if len(matches) != 1:
                # Retained receipts keep the original resource under metadata;
                # never infer current ABI merely from a similar filename.
                raise ValueError("Current official native resource identity is unresolved: " + resource)
            declaration = matches[0].get("declaration", {})
            if declaration.get("sha256", "").lower() != matches[0]["sha256"] or declaration.get("size") != matches[0]["bytes"]:
                raise ValueError("Current native resource manifest identity differs: " + resource)
            native = audit.native_slice(matches[0], export_names, string_keys, **extra)
            if resource in {"IoTNative", "Razer_IoT_SDK"}:
                aliases = [row for row in indexed["inputs"] if row["path"].startswith(
                    ".analysis/inputs/installed-current/") and row["sha256"] == matches[0]["sha256"]]
                if len(aliases) != 1 or audit.read(aliases[0]) != audit.read(matches[0]):
                    raise ValueError("Current IoT installed alias differs from official original: " + resource)
                native["verified_installed_alias"] = aliases[0]
            report["native"].append({"resource": resource, **native})
            rpc_originals[resource] = matches[0]
        rpc_server = audit.rpc_contract(rpc_originals["RazerWDLExe"], stub_rva=0x24bc70,
            interface_rva=0x24bc10, role="server", callback_offsets_rva=0x24ba84, callback_count=1)
        rpc_client = audit.rpc_contract(rpc_originals["RzLampArrayDLL"], stub_rva=0x16f2b0,
            interface_rva=0x16f250, role="client", server_contract=rpc_server,
            callback_offsets_rva=0x16f0c4, callback_count=1)
        report["local_rpc_contract"] = {"server": rpc_server, "client": rpc_client}
        report["rpc_server_dispatch"] = audit.native_slice(rpc_originals["RazerWDLExe"], [],
            rvas=tuple(int(row["target_rva"], 16) for row in rpc_server["server_routines"]), depth=2)
        report["rpc_callback_dispatch"] = audit.native_slice(rpc_originals["RzLampArrayDLL"], [],
            rvas=tuple(int(row["target_rva"], 16) for row in rpc_client["callback_routines"]), depth=2)
        report["native_json_contract"] = {
            "original": rpc_originals["RazerWDLExe"],
            "dispatcher_rva": "0x82c90", "action_registration_rva": "0x79e40",
            "exception_owner_rva": "0x85cf0", "default_response": {"succeeded": True},
            "exception_response": {"succeeded": False, "reason": "std::exception::what()"},
            "actions": [
                {"action": name, "ordinal": ordinal, "handler_rva": hex(rva)}
                for name, ordinal, rva in (
                    ("device_get_all", 4, 0x83390), ("device_open", 5, 0x83e90),
                    ("device_close", 6, 0x831c0), ("device_indices_for_key", 7, 0x83b00),
                    ("device_get_brightness", 8, 0x837a0), ("device_set_brightness", 9, 0x84390),
                    ("effect_set_all_lamps_same_color", 10, 0x84630),
                    ("effect_set_colors_for_indices", 11, 0x84860),
                    ("system_get_vk_and_vsc_map", 12, 0x88600))],
            "device_serializer": {"rva": "0x85fe0",
                "base_fields": {"id": "string", "name": "string", "enabled": "bool",
                    "containerId": "string", "vendor_id": "u16", "product_id": "u16"},
                "optional_presence": "DeviceInfo byte +160 is nonzero; absent fields remain absent",
                "optional_fields": {"product_id": "u16 overwrites base field", "vendor_id": "u16 overwrites base field",
                    "connected": "bool", "available": "bool", "brightness": "u8", "type": "i32",
                    "bounding_box": {"x": "number", "y": "number", "z": "number"},
                    "lamps": [{"position": {"x": "number", "y": "number", "z": "number"},
                        "purposes": "u32 bitfield", "keys": "array of nonzero u32 virtual keys from eight native slots"}]},
                "geometry_rounding": "round(value * 1000000.0) / 1000000.0"},
            "device_open": {
                "handler_rva": "0x83e90", "manager_wrapper_rva": "0x155750", "owner_rva": "0x154d30",
                "factory_rva": "0x152b30", "factory_slot": "ILampArrayStatics+0x38 FromIdAsync",
                "sequence": "Resolve the exact discovered device id under Impl mutex+240, request FromIdAsync, wait for at most the source one-second interval via 0x15f8c0/0x148af0/0x148b20, require completed status 1, then GetResults 0x153880 and a nonnull LampArray. Missing id, timeout or null result yields no optional device; the complete owner does not call IAsyncInfo.Cancel on timeout",
                "metadata_owner_rva": "0x152340", "key_map_owner_rva": "0x159680",
                "metadata_sequence": "BoundingBox(+0x68), ProductId(+0x40), VendorId(+0x38), IsConnected(+0x90), IsEnabled(+0x70), Brightness(+0x80; multiply 100, truncate into a byte), optional IsAvailable, HardwareVersion(+0x48), MinUpdateInterval(+0x60; divide 10000), Kind(+0x50), LampCount(+0x58), then each GetLampInfo(+0xa0), position and purposes. Kind 1..11 stays identical and other kinds become 0; serializer omits optional fields when presence byte+160 is zero",
                "lamp_keys": "For virtual keys 1..254, query ILampArray+0xa8, group returned lamp indices and release each returned array with CoTaskMemFree. Assembly 0x1599f0..0x159a07 copies min(key_count,8) keys into each zeroed 48-byte lamp record. Serializer reads only these eight slots and omits zero keys",
                "availability_capability": "Impl constructor 0x14ae70 asks Windows.Foundation.Metadata ApiInformation IsEventPresent for Windows.Devices.Lights.LampArray and AvailabilityChanged via 0x154820/vtable+0x48. If absent, the metadata owner assigns available=true without a property query; this source default is not an observed availability result",
                "availability_subscription": "When supported and a LampArray was returned, read IsAvailable through 0x154580; if false, retry after Sleep(20ms) up to five times. Register the callback through 0x14f4b0 on the queried availability interface+0x38 and discard the local token. A catch logs subscription failure but still permits the already collected optional metadata to be returned; false availability is not an open-failure gate",
                "availability_callback_rva": "0x150990",
                "availability_callback": "Resolve id, lock the device map, update only the retained optional available field when its record exists, query additional properties for logging, unlock, mark state changed and invoke the retained availability consumer. The API owner 0x87900 emits DeviceAvailabilityChanged and removes the device's first-frame log marker; this is not a complete metadata reread",
                "result_consumer": "The handler sets succeeded:true and device only when the returned optional DeviceInfo is present, else succeeded:false. The JS connectDevice then awaits the keyboard map and immediately parses lamps/geometry; it does not add a separate required-field validator or map-success gate",
                "source_adapter_values": "The complete JS rzDeviceLampArray owner hashes deviceID to produce its pseudo serial, returns fixed edition/layout [0,0] and driver mode [3,0], and accepts setDeviceMode/setChromaEffectCommon without a native write. These adapter values must remain distinct from actual device reads or confirmed effects",
                "target_execution": False},
            "responses": {
                "device_get_all": "Adds devices array, serializing every retained map entry",
                "device_open": "Reads device_id with empty-string default, erases the per-device first-frame log marker; optional device produces succeeded:true plus device, absent device produces succeeded:false",
                "device_close": "Reads device_id with empty-string default; manager bool becomes succeeded. Missing device is false; an already-closed device still in the map is true"},
            "events": {"envelope_owner_rva": "0x85a50", "event": "OnRzLampArrayCallback",
                "variants": [
                    {"type": "DeviceAdded", "owner_rva": "0x874c0", "device": "device_serializer"},
                    {"type": "DeviceRemoved", "owner_rva": "0x87eb0", "device_id": "string"},
                    {"type": "DeviceAvailabilityChanged", "owner_rva": "0x87900", "device_id": "string", "available": "bool"}],
                "availability_followup": "After emitting, erases only the per-device first-frame log marker at owner +224 through RVA 0x88280"},
            "watcher_device_changes": {
                "registration_rva": "0x159be0",
                "callback_targets": "Added binds Impl and thunk 0x2329 -> 0x14fe20; Removed binds Impl and thunk 0xa0d3 -> 0x150f10. The latter has an unrelated decompiler-assigned CMFCRibbonPanel::Reposition name, which is not its semantic identity. Constructors 0x147860/0x147910 retain both bound words; raw delegate tables 0x305190/0x3051f0 resolve Invoke at +0x18 to 0x4d2c -> 0x1544c0 and 0x3de1 -> 0x154500. Complete Invoke assembly passes this+0x10 to 0x13b840/0x13b8c0, which AddRef both arguments and invokes the retained Impl callback. The two Invoke owners retain original Hex-Rays failures, not fabricated pseudocode",
                "added": "0x14fe20 reads DeviceInformation Id, Name, IsEnabled and Properties through wrappers 0x153ac0/0x154ca0/0x154740/0x156380. It derives VID/PID from System.Devices.DeviceInstanceId by finding VID_/PID_, taking up to four following characters and stoul(base16); missing property/markers gives zeros. ContainerId comes separately from System.Devices.ContainerId, formatted as a brace-delimited GUID and uppercased; missing/invalid property leaves an empty string. Under mutex+240 it inserts or replaces the exact-ID record with these base fields, clears optional metadata, releases an existing LampArray COM pointer and stores null. It unlocks before invoking the retained Added consumer",
                "removed": "0x150f10 reads DeviceInformationUpdate.Id through 0x153b50, removes the exact-ID map entry under mutex+240 and invokes record destruction 0x14cb20 to release its COM pointer, optional metadata and strings. It unlocks, marks changed and invokes the retained Removed consumer even when no matching map entry was found. Device close alone does not perform this removal",
                "api_consumers": "Startup 0x88410 binds Added/Removed to API thunks 0x10023/0x10a0a -> 0x874c0/0x87eb0; these serialize DeviceAdded{device} or DeviceRemoved{device_id} into the existing reverse RPC queue. Queueing and borrowed callback invocation do not establish that JS consumed the event",
                "source_consumers": "Complete RzWDL and LampArrayEventHandler classes handle additions/removals independently. An existing id in RzWDL emits its retained record without replacing it. Higher-level additions serialize by device-id mutex, skip Razer vendor devices while useWDL is false, remove matching native Chroma entries, prepare the LampArray device and save WindowStorage. Removal serializes by id, removes matching LampArray Chroma entries, conditionally removes the retained device and lamp-info, removes file cache and saves WindowStorage. Their caught errors only log; neither full owner has a connection-generation rejection rule",
                "unresolved_runtime": "Repeated Added replacing an open native record, removal before queued Added delivery, same-id readdition, callbacks after Stop and concurrent asynchronous JS listeners require pure/simulated tests and actual DLL/service comparison; static ownership facts are not runtime acceptance"},
            "colors_for_indices": {"handler_rva": "0x84860", "manager_rva": "0x1585d0",
                "validation": "indices and colors arrays; indices nonempty; colors.length == 3 * indices.length",
                "conversion": "indices to i32; RGB components to u8; source packs 0xFFBBGGRR, manager rotates the u32 left by eight bits to 0xBBGGRRFF, whose little-endian bytes are Windows Color{A,R,G,B}",
                "independent_static_example": {"rgb": [18, 52, 86], "packed_u32": "0xff563412",
                    "windows_color_u32": "0x563412ff", "windows_color_bytes": "ff123456",
                    "status": "derived from retained handler and manager instructions; not a DLL execution sample"},
                "write": "RVA 0x1584f0 calls ILampArray vtable +0xd0: HRESULT(this,u32 colorCount,Color*,u32 indexCount,i32*)",
                "logging_marker": "Handler owner +224 is a set of device-id strings protected by mutex +288. Missing entry logs the first frame and inserts its id through RVA 0x63ea0; both existing and new entries continue to conversion and COM write. It is not an indices cache or a frame-suppression result",
                "result": "Missing/unopened device or caught exception returns false; HRESULT failure throws into manager catch"},
            "brightness": {
                "get": {"handler_rva": "0x837a0", "facade_rva": "0x1530c0", "manager_rva": "0x152bc0",
                    "com_wrapper_rva": "0x14f920", "abi": "ILampArray vtable +0x80: HRESULT(this,double* out)",
                    "input": "device_id defaults to empty string",
                    "conversion": "COM double times 100.0, truncation to int and storage in u8",
                    "response": "succeeded is manager bool; nLevel is present only on success. Initialized zero out is not a successful observation"},
                "set": {"handler_rva": "0x84390", "facade_rva": "0x157530", "manager_rva": "0x156f60",
                    "json_value_helper_rva": "0x74480", "com_wrapper_rva": "0x14f8c0",
                    "input": "device_id defaults to empty string; nLevel defaults to 100 and is converted to u8",
                    "conversion": "u8 nLevel / 100.0; values outside [0.0,1.0] select 1.0",
                    "assembly_confirmation": "RVA 0x157344 replaces XMM6 with 1.0; RVA 0x15738a moves XMM6 to XMM1 before thunk 0x4c96, so the selected limit reaches COM rather than only the log",
                    "abi": "ILampArray vtable +0x88: HRESULT(this,double value), XMM1 contains value on Windows x64",
                    "response": "succeeded is manager bool; this setter does not read back brightness"},
                "failure": "Under the device-manager mutex, missing or unopened device returns false; failing HRESULT throws and the manager catches std::exception and returns false",
                "json_boundary": "Missing field uses the stated default. The brightness helper accepts boolean/numeric representations and throws type_error 302 for other values, 306 for non-object input; unverified out-of-range JSON conversion must not become a claimed stable protocol rule",
                "source_consumer": "The complete rzDeviceLampArray class reads nLevel into jsonData.brightness only on succeeded===true. Its setter reports the requested brightness on success, not a measured readback"},
            "indices_for_key": {"handler_rva": "0x83b00", "facade_rva": "0x1537a0",
                "manager_rva": "0x153230", "com_wrapper_rva": "0x153180",
                "input": {"device_id_default": "", "key_default": 0},
                "abi": "ILampArray vtable +0xa8: HRESULT(this,u32 virtualKey,u32* count,i32** indices); RCX/RDX/R8/R9 are confirmed in the complete wrapper assembly",
                "ownership": "Copy the returned i32 indices, then CoTaskMemFree the COM allocation",
                "response": "Adds indices. Missing/unopened device returns an empty vector without setting succeeded false; the dispatcher default true therefore does not establish a successful device observation",
                "exception": "This manager has no local catch; outer JSON owner RVA 0x85cf0 converts std::exception to succeeded:false with reason",
                "source_consumer": "The selected complete lighting-engine main and host LampArrayAction owners have no literal caller of this action. The device class instead derives WASD/arrows/functions/numpad/numbers groups from enumerated lamp.keys"},
            "single_color": {"handler_rva": "0x84630", "facade_rva": "0x156f30",
                "manager_rva": "0x156a90", "com_wrapper_rva": "0x157560",
                "input": {"device_id_default": "", "color_default": 0, "color_type": "u32"},
                "conversion": "For input 0xAABBGGRR, rotate left eight bits to 0xBBGGRRAA; little-endian Color bytes are A,R,G,B. No opaque alpha is added by this action",
                "independent_static_example": {"input_u32": "0xaa563412", "windows_color_u32": "0x563412aa",
                    "windows_color_bytes": "aa123456", "status": "instruction-derived, not a DLL execution sample"},
                "abi": "ILampArray vtable +0xb8: HRESULT(this,Color value); the four-byte Color value is loaded into EDX, not passed as a Color pointer",
                "response": "Missing/unopened device or caught std::exception returns false; otherwise true after COM success; no color readback",
                "source_consumer": "No literal caller in the selected complete lighting-engine main and host LampArrayAction owners; their actual frame route uses effect_set_colors_for_indices"},
            "keyboard_map": {"handler_rva": "0x88600",
                "input": {"map_type_default": 0, "id_thread_default": 0},
                "response_fields": {"vk_and_vsc": [{"key": "u32 virtual key", "scan_code": "u32 mapping result", "key_name": "UTF-8 key display name"}]},
                "calculation": "GetKeyboardLayout(id_thread), then virtual keys 1..254 in increasing order; keep nonzero MapVirtualKeyExA(vk,map_type,layout); GetKeyNameTextW(mapping<<16,buffer,128) supplies each display name",
                "cache": "Static map keyed by the complete parsed-input JSON dump; a hit copies the cached array without querying the current layout. This owner has no layout-change invalidation. Invalid layout emits an empty vk_and_vsc without changing the dispatcher default succeeded:true and is not inserted into the cache",
                "source_consumer": "On successful device_open, rzDeviceLampArray awaits getVKAndVSCMap with no extra parameters, then parseDeviceInfo. A succeeded map becomes Map(key,scan_code); it ignores key_name and does not treat that map call as a separate open-success gate"},
            "registry_settings": {
                "export_owner_rva": "0x2a6a0", "registry_owner_rva": "0x116eb0",
                "serialization_owner_rva": "0x115100", "value_reader_rva": "0x117c30",
                "identity_filter": "When the global RPC client exists, the export calls JsonApi with the literal device_get_all action and parseDeviceIds 0x98f60. A nonempty parsed id set filters enumerated registry device names by substring matching against those full ids; an empty set does not establish hardware identity",
                "roots": ["HKCU\\Software\\Microsoft\\Lighting", "HKCU\\Software\\Microsoft\\Lighting\\Providers",
                          "HKCU\\Software\\Microsoft\\Lighting\\Devices\\<device>"],
                "shape": {"globalSettings": [{"name": "string", "data": "string"}],
                          "globalProviders": ["string"],
                          "deviceSettings": [{"deviceId": "string", "values": [{"name": "string", "data": "string"}],
                                              "providers": ["string"]}]},
                "value_conversion": "REG_SZ is read as an ANSI string; REG_DWORD is converted to a string; other types become the literal (Unsupported type). The helper uses fixed 256-byte name/data buffers and stops its loop on the first nonzero RegEnumValueA result, including errors; the output has no separate per-key observation result",
                "source_consumer": "Complete RzWDL and LampArrayEventHandler owners require AmbientLightingEnabled data exactly equal to string 1, parse Brightness in decimal and warn at <=10, match devices by id substring, compare ordered provider arrays through JSON.stringify and test only the first provider for RazerDynamicLighting. Missing .find results throw into the owner's catch rather than providing a confirmed default"},
            "controller_file": {
                "get_path_owner_rva": "0x2a220", "path_owner_rvas": ["0x119360", "0x118ff0", "0x119300"],
                "path": "%localappdata%\\Razer\\RazerAppEngine\\User Data\\Apps\\Common\\LampArray\\RzLampArray.json",
                "path_side_effect": "0x118ff0 expands the local appdata environment path, conditionally attempts CreateDirectoryW for its LampArray directory, and does not promote directory creation to a checked installation result",
                "set_export_owner_rva": "0x2ac60", "write_owner_rva": "0x119610",
                "codec_owner_rvas": ["0x118d80", "0x118650", "0x118ad0"],
                "format": "For a nonempty payload other than exactly [], the helper opens wb, writes a four-byte native little-endian signed length and then length payload bytes XORed with the repeating eleven-byte ASCII key RzLampArray. No terminator or authentication field is written. Empty string or exactly [] instead attempts _wremove",
                "result": "The internal helper returns false on open/delete failure, but exported SetDeviceInfoToFile ignores that bool and returns true for a nonnull argument after normal helper return. The void codec only logs short writes/caught exceptions, and fclose is unchecked. Null argument or an exception reaching the export returns false; true is not confirmed durable publication",
                "source_consumer": "LampArrayEventHandler generates ledConfig/productName/productId/vendorId/enabled/type/category/deviceContainerId/keyIndices from cached devices, schedules a newly-created 300ms debounce wrapper, then sends JSON.stringify through the low-level writer. Its empty-cache branch only logs and does not invoke the native [] deletion path; the separate class facade setLampArrayDeviceInfoToFile(e) drops e when calling its wrapper",
                "target_execution": False},
            "callback_ffi": {"original": rpc_originals["RzLampArrayDLL"],
                "registration": "SetNodeFFIEvent is void(callback); RVA 0x7d450 stores pointer at LampRpcClient+0xd0",
                "invocation": "RVA 0x7cf10 invokes callback with RCX=const char*; void callback(const char*) on x64",
                "lifetime": "Copy UTF-8 JSON during callback; server serialized temporary and RPC incoming buffer are borrowed. No callback FreeMalloc ownership transfer",
                "source_retention": "FFIPreloadMain/SubProcess keep Callback(void,[string]) buffers in ffiCallbackMap; FreeFFI is a no-op in these originals"},
            "native_lifecycle": {
                "setup_rpc_rva": "0x7d510", "setup_rpc": "Rejects null/missing EXE or MSIX paths. Unless RPC is already connected, attempts KillProcessByName(razerwdl), warns on failure, retains both paths, then returns true; this step does not launch the helper",
                "startup": "RVA 0x7e580 calls InitRpc 0x7c0f0 if not connected, then service opnum 0. InitRpc invokes StartRpcServer 0x7df10, creates a worker thread if absent, composes local ncacn_np binding, marks connected, then flushes queued callbacks through opnum 6",
                "helper": "StartRpcServer invokes EXE install MSIX with wait=true; then EXE start -p currentProcessId -s session with wait=false; sleeps 400ms after launch. ExecuteRpcServer 0x7b4d0 validates Razer signing, builds quoted local command, calls CreateProcessW and optionally WaitForSingleObject, closes process/thread handles; process creation success does not inspect helper exit code",
                "initialization_failure": "Current RzWDL source returns false after ConfigureFFI/SetupRPC/Startup/StartWatcher failure. SetNodeFFIEvent is void and its returned value is ignored. No failure rollback is present in that source owner",
                "explicit_cleanup": "Low-level lighting owner unInitRzLampArray, only when initialized, awaits StopWatcher then Shutdown and clears its initialized flag. RzWDL.unregisterEvent separately removes response listener, cancels debounce, removes emitter listeners and clears devices/settings",
                "watcher_creation": "RVA 0x159be0 creates a retained DeviceWatcher through wrapper 0x14fd90 (factory vtable+0x70). It registers Added and Removed through wrappers 0x14ed70 (+0x30) and 0x156760 (+0x50); their returned event tokens are local temporaries. This complete start owner registers neither Stopped nor EnumerationCompleted",
                "watcher_start": "Impl byte+1 skips an already-marked start. It sets that byte before get_Status wrapper 0x159dd0 (+0x80), then calls Start wrapper 0x159b80 (+0x88). The status result is not a readiness gate; failed HRESULT throws without resetting the byte in this owner",
                "watcher_stop": "RPC opnum 5 reaches 0x3dba0 -> 0x159f00, which calls DeviceWatcher.Stop wrapper 0x159e40 (+0x90) only for a nonnull marked watcher, then clears byte+1. It retains the watcher pointer and does not remove event registrations, await a Stopped event or join callback work. A failing Stop HRESULT throws before the byte is cleared",
                "watcher_client_result": "Client StartWatcher/StopWatcher at 0x7e3e0/0x7e720 reinitializes RPC if necessary, returns false if still disconnected, otherwise invokes opnum 4/5 then returns true after normal RPC return. This does not independently confirm enumerator readiness or all in-flight callback completion",
                "cleanup_source_result": "The complete JS unInitRzLampArray awaits both calls but does not test their returned booleans before clearing its flag. Source flags, RPC completion, watcher Stop and full resource release remain different observations",
                "server_shutdown": "Service opnum 1 reaches WDL 0x883f0 and calls StopWatcher 0x159f00. The complete client Shutdown owner 0x7dd70 invokes this operation then returns true; it does not clear connected byte +40, callback pointer +208, join its worker or free the RPC binding",
                "rpc_worker": "InitRpc passes object plus thunk 0x5ed9 to the CRT thread wrapper 0x36120. The worker 0x7b220 computes a fresh ten-millisecond deadline per iteration (assembly 0x7b284 stores 0x0a), waits on condition variable +136 under mutex +48 and tests stop byte +128. After the deadline, 0x7b470 invokes RPC opnum 6 through stub 0xa4410 only when connected. A wake alone does not replace the deadline check",
                "rpc_worker_callback_flush": "Server opnum 6 reaches 0x3db20 -> 0x37470, locks its callback queue, calls the reverse callback for each front entry and removes entries after normal callback return. It is a callback-drain operation, not another device enumeration",
                "rpc_client_destruction": "Destructor 0x733a0 locks mutex +48, sets stop byte +128, unlocks, broadcasts condition variable +136 and joins thread +16 via 0x96160. On the normal path it then frees the retained RPC string, conditionally frees the binding, and destroys owned containers. A catch-all skips to member destruction after a failure; a remaining joinable thread calls terminate, so exceptional destruction is not a verified clean release",
                "dll_detach": "The PE entry 0x12f3 -> 0x147c34 -> 0x147a98 -> 0x2a180 reaches user DLL detach with reason zero. That owner exchanges the global client pointer with null, destroys/frees the old client, then closes its logging owner. FreeFFI in the current JS does not unload the library, and exported Shutdown does not invoke this detach path",
                "server_manager_destruction": "WDL registration 0x22c00 registers exit owner 0x23cb20, which destroys the API through 0x7dbd0. API destruction invokes Shutdown, destroys its containers/callback function and destroys the manager through 0x14cbb0 -> 0x14c920. Impl destruction conditionally stops the watcher again, destroys its device map through 0x13dc20 and retained function objects, then releases factory/watcher COM pointers through 0x13a6c0. These complete destructors contain no explicit Added/Removed token revocation, Stopped-event wait or callback join; Stop alone does not establish their completion",
                "device_close": "WDL 0x14f990 releases non-null LampArray COM pointer through 0x13a6c0 (set null, IUnknown::Release at vtable+16), retaining the discovered device record",
                "get_all_observation": "WDL 0x14f060 refreshes enabled via ILampArray get_IsEnabled(+0x70) and optional connected via get_IsConnected(+0x90); available is a separate retained field and is not silently replaced with either property"},
            "target_execution": False, "all_action_semantics_complete": False}
        report["native_process_contract"] = {
            "original": rpc_originals["RazerWDLExe"], "entry_rva": "0x1318c0",
            "cli_owner_rva": "0xf5960",
            "cli": "Exactly one of start [-p|--parent-pid UINT] [-s|--session STRING], install path, uninstall; global -v|--verbose. Only install path has the required flag. Parent PID defaults to zero and session to an empty string; session is not parsed as a Windows numeric session id",
            "package_identity": "0x1382f0 obtains the current process full package name with GetCurrentPackageFullName. APPMODEL_ERROR_NO_PACKAGE (15700) yields an empty name; other errors throw. It does not enumerate installed packages or check a particular product/version identity",
            "install": {"owner_rvas": ["0xf3d60", "0x138850"],
                "sequence": "An existing nonempty current-process package identity returns true before validating the supplied path. Otherwise obtain the current EXE path, validate the package signature, set IAddPackageOptions.ExternalLocationUri to the EXE parent directory via +0x68, obtain IPackageManager9 and call AddPackageByUriAsync at +0x38",
                "async_result": "0x1352f0 waits INFINITE through 0x1351e0 only while Started, throws hresult_canceled for Canceled, calls GetResults through 0x138220, queries IDeploymentResult2 via 0x135030 and reads IsRegistered at +0x30 through 0x138680. Caught exceptions produce false",
                "exit_code": "WinMain ignores the helper bool and returns 1 after parameter cleanup; this exit code is not the actual deployment result"},
            "uninstall": {"owner_rvas": ["0xf6b70", "0x138c40", "0x138d20"],
                "sequence": "No current-process package identity returns true. With identity, first require GetModuleFileNameW success, then call IPackageManager.RemovePackageAsync at +0x40 using the current full name",
                "async_result": "Releases the returned async operation immediately without waiting or GetResults. True only establishes absence of a synchronous exception in this owner",
                "exit_code": "WinMain ignores the helper bool and returns 0 after common cleanup; asynchronous uninstall completion is unobserved"},
            "package_signature": {"owner_rvas": ["0x161d30", "0x161b20", "0x161c10", "0x160e80"],
                "sequence": "WinVerifyTrust with CACHE_ONLY_URL_RETRIEVAL (0x1000) and STATEACTION_VERIFY, then require both signer Subject CN (OID 2.5.4.3) and O (OID 2.5.4.10) to equal the exact bytes Razer USA Ltd. Subject extraction accepts ValueType 4 (CERT_RDN_PRINTABLE_STRING); failed reads leave an empty subject",
                "cleanup": "The complete WinVerifyTrust owner has no subsequent STATEACTION_CLOSE; static trust verification calls are not evidence that trust-state cleanup completed"},
            "start": {"owner_rvas": ["0x1318c0", "0xf71d0"],
                "gates": "Check parent PID, require a nonempty current-process package identity, create session mutex and hidden window, then create RPC and optional parent-monitor workers. Invalid PID, missing identity or failed hidden window returns 1; normal message-loop completion and cleanup returns 0",
                "single_instance": "0xe3290 creates Global\\rzwdl_<session> with bInitialOwner=false and checks ERROR_ALREADY_EXISTS (183); it does not wait for ownership. CreateMutexA failure retains the new-instance flag. For an existing instance, 0xf3190 searches fixed RazerWDL class/title with no session key, posts message 0x40a/wParam 0x1337 if found; main sleeps 300ms and continues without confirming the old instance ended",
                "rpc": "0x341f0 retains the same session. 0x37570 forms \\pipe\\rzwdl_<session>, calls RpcServerUseProtseqEpA(ncacn_np,1234), RpcServerRegisterIf and RpcServerListen(1,1234,false). started is set before Listen and cleared on its error. Worker 0xad0f0 discards RunForever's result, so successful thread creation or process exit does not establish readiness",
                "parent": "0xf4730 opens the specified PID with 0x1000 PROCESS_QUERY_LIMITED_INFORMATION. Open failure is false; successful GetExitCodeProcess compares STILL_ACTIVE (259), but that API failing retains the initial true. It closes the handle without checking parent relationship, executable identity or session. 0xf2c20 checks every 300ms and posts message 0x12 to the hidden hwnd when inactive; wrapper 0xad140 sets the global exit flag before return",
                "messages": "WndProc 0xf31e0 handles WM_CLOSE or 0x40a with wParam 0x1337 by setting the exit flag and PostQuitMessage(0). Loop 0xf7140 uses GetMessageW/TranslateMessage/DispatchMessageW, then DestroyWindow",
                "cleanup": "0xf71d0 nulls and destroys/frees the global RPC server before joining its worker, then joins an existing parent-monitor worker. Main closes the session mutex and cleans log/arguments. Destructor 0x35c70 -> 0x37a60 calls RpcMgmtStopServerListening, only on success calls RpcServerUnregisterIf(nullptr,nullptr,false), and discards the cleanup bool"},
            "target_execution": False, "installation_execution": False,
            "actual_deployment_comparison": "unverified"}
        report["lighting_iot_contract"] = {
            "originals": {name: rpc_originals[name] for name in ("IoTNative", "Razer_IoT_SDK")},
            "native_queue": "IOT_RunCmdFw25 0x37590 bypasses its connection check for UDP, copies host/command/JSON via 0x36da0 into a per-host CommandQueue, then returns 1. Worker 0x2720 pops via 0x1050 and calls 0x48fe0 without consuming its result. UDP frames closer than 15000000ns to that host queue's last admitted frame are dropped before SDK execution; constructor 0x2dc0 initializes that timestamp, including the initial 15ms boundary",
            "payload": "0x35b40 parses the retained JSON array into u8 bytes, but passes the caller's length onward without comparing it to vector length. SDK ad17 -> 26dd20 -> 6e97 -> 25cc60 rejects non-UDP SetChromaFrame/V2; registered class is 0x0f, legacy id3, V2 id7",
            "selector": "SocketCallBack+540 starts at zero and is reset on open. Receive callback 0x2517c0 identifies header 02 00 as selector1 and AA 02 as selector2 after at least five buffered bytes. 0x24acf0 sends up to three identification probes using the callback's timeout/condition state. Selector is a received protocol identity, not a firmware-version or general capability declaration",
            "scan_native": "IOT_StartMdnsDeviceScan0x34f40 calls SDK Start(-1) only when SDK is loaded and Native scan flag903d8 is false, then schedules detached timer0x3c900. Native -1 becomes8 seconds; positive values are seconds, nonpositive values other than-1 expire at the initial deadline. Repeated Start while flag is true returns1 without starting another scan; unloaded SDK returns false. Timer invokes asynchronous SDK Stop then clears the flag. IOT_StopMdnsDeviceScan0x35400 only logs and returns1; it neither stops SDK nor clears that flag",
            "mdns_query": "SDK0x26df20 ->0x2672f0 ->0x267700 starts detached0x24f1b0 with _wled._tcp.local and2000ms per-select timeout. Duration is unsigned32 milliseconds at comparison, so-1 means4294967295ms; duration is checked only after a complete query.0x2536b0 enumerates up adapters with bounded reallocations, creates at most32 IPv4/IPv6 multicast UDP sockets bound to local10003, and0x2503e0 sends PTR/type12 to5353 with QCLASS0x8001.0x264b60 resets select's2000ms wait after received traffic, has no internal stop check, and closes sockets/frees buffers/WSACleanup per round. It has no whole-round hard deadline",
            "mdns_record": "Jump bytes atRVA13c0 resolve to0x258610, not the default printer. A single global168-byte record aggregates SRV/PTR/A/TXT mac/TXT exdata/port without a host-keyed map. A count>=4 in the additional section commits a record without proving one of each record type; AAAA is not projected.0x251260 publishes only nonempty PTR/SRV/IPv4/exdata, positive port and matching service prefix; connection still uses fixed10003/10005 rather than the SRV port",
            "mdns_identity": "0x251260 ->object45da70 ->vtable3eb8d8[0] ->0x8e0e ->0x239880 requires all six case-sensitive ProductType_/&VID_/&PID_/&EID_/&NAME_/backslash-MAC_ markers. It projects string identities/hostname/IPv4 and prefers separate TXT mac over the suffix; invalid ranges may be empty, bad hostname serialization becomes InvalidHostName.0x23d390 emits event2/cmdType0 through the global adapter and detached raw callback. Internal status8/13/10 empty records fail the marker gate. No GetMdnsDeviceList export exists in the selected current native interface",
            "scan_sdk_stop": "0x26df40 ->0x267910 creates detached0x23d270(wait=true) and returns immediately. The task sets stop and waits indefinitely for done only when holder exists and stop was previously false; already-stopped skips that wait. It frees the detached holder, not a joined worker. Worker signals done before final callbacks/capture destruction, so done does not establish thread/callback drain",
            "connection": "IOT_ConnectNetDevice0x35940 ->0x35710 queues a host-copied AddConnectTask and returns0;35710 uses strlen, so this does not establish a null-safe entry.0x49220 ->0x35590 returns-2 for unloaded SDK or NULL host pointer, otherwise executes SDK Connect; worker0x2720 discards its result. SDK0x26d430 ->0x2481c0 returns actual open status; already-connected emits16 and returns-1.0x24bac0 only tests stored TCP/UDP ready flags. Open0x252420 orders TCP10003, reader, selector probe, callback/TCP maps, keepalive, then UDP10005. TCP inet_addr/AF_INET uses one nonblocking connect plus select/SO_ERROR, then restores blocking. UDP failure does not replace the nonnegative selector return, so Status0 and IsConnected=false can coexist. Status0/1/16 carry Host/Status only",
            "probe_acceptance": "0x24acf0 encodes a KeepAlive time probe, mutates byte0=AA/byte4=02/checksum XOR02, sends before clearing+704, and waits on condition+552 up to10s for at most3 sends. Predicate is notification+704, not selector. Garbage wakes can exhaust all three iterations while selector remains0 and still return0. Failed identification closes TCP before map registration; map-based Disconnect is not evidence it can cancel this in-flight identification",
            "frame": "0x247ef0 -> 0x247920 -> vtable 0x3df6a0+0 -> 0x227050 encodes V25. Offsets0/1 are zero/transaction, 2..3 are total big-endian, 4 is zero, 5 is payload length low eight bits, 6/7 are class/id, payload begins8, XOR includes total length, reserved, payload length, class/id and encoded payload; final byte is zero. Selector1 uses payload+10 bytes; selector2 caps payload at80 in the serializer, emits a zero-filled fixed90-byte inner frame, then 0x264870 -> 0x1f4dc0 adds AA 00 totalBE16 00. Intermediate JSON/hex is decoded back to bytes before sending",
            "transport": "0x2637d0 selects that host's TCP/UDP vector element, requires ready byte+57, and invokes primary vtable+8 -> thunk0xa6c3 -> 0x263670. Open0x252420 uses TCP10003 and UDP10005; 0x2532b0 creates AF_INET/SOCK_DGRAM/IPPROTO_UDP for UDP. Actual sendto/send returns OS byte count or -1 and stores ready=(count>0). No per-frame ACK is read for UDP; a failed send causes later frames to return zero before send until another chain restores transport readiness. UDP open does not start a receive worker",
            "consumers": "SDK 0x25cc60 returns the actual send count/error to IoTNative 0x35b40, which also returns it and can log it. UDP skips the command-result event notification; it does not suppress the numeric return. Native queue worker 0x2720 ignores the execution result; 0x37590 returns1 for queue acceptance and JS ignores that immediate value. Queue acceptance, throttled drop, OS send count and device effect must be compared as distinct observations",
            "disconnect": "0x6d75 -> 0x26d720 -> 0x19ce -> 0x249b20 returns -1 immediately for a host absent from the transport map. Otherwise it sets callback stop+705, closes nonnull transports via secondary vtable+24 -> 0x82e2 -> 0x247510 (closesocket and WSACleanup), erases the transport map via0x244830, notifies conditions via0x250fd0, erases the callback map via0x244620, then passes Status14 to generic publisher0x262fb0. Return is the last nonnull transport close result. Native0x38690 invokes Node before queue removal0x15b0, requiring Node and payload pointers nonnull, original id/cmdType zero, exact Status14 text, parsed nonempty Host and existing Block; event parameter is not a removal gate. Empty C string is allowed; null pointer or the CommandName/KeepAlive string filter returns early. Stop does not interrupt an in-flight synchronous task; join waits. Detached reader/keepalive workers are not joined here",
            "reader": "TCP10003 open0x252420 starts reader0xb302 -> 0x259190 and keepalive0x4ce1 -> 0x24bd30; UDP10005 starts neither worker. Primary receive slot0 -> 0xb66d -> 0x259010 ignores the caller's10000 timeout; the audited chain restores blocking after connect and sets no SO_RCVTIMEO. It sets ready=(count>0), writes outcount only for positive receive, and uses recv/recvfrom. The TCP reader reports nonnegative count as status2, including EOF status2/len0 before exiting with ready=false; negative count logs WSA error, reports status3/zero length and closes via0x247510 without reconnecting. The lower0xa678 -> 0x251490 notification only dispatches when socket callback+424 exists",
            "receive_identification": "0x2517c0 ignores status4/5 except metadata cleanup. Status2 appends to identification buffer+712 until at least five bytes, then examines only the first two bytes for0200/aa02. It sets identified-notification byte+704 and signals even with selector0; garbage prefixes need not recover. Selector1/2 then dispatches the entire buffered input through V25/Wled and clears identification storage",
            "v25_parser": "0x2541d0 -> 0x6230 -> 0x22bcb0 appends to buffer+760. Extended selector1 checks XOR and uses totalBE16-10 payload, upper total4096+10; fixed selector2 uses90 bytes and size-byte payload with XOR check disabled. Eight equal header bytes skip8; status must be<=13; payload must be<4096. Fixed XOR covers all80 payload slots and status1/2 bypass checking even when requested. The terminal byte is read but not constrained to zero. Acceptance additionally requires skip+frameLength==whole input length; concatenated frames are not delivered as a conventional first-frame parser. A -21082 return may retain consumed from an earlier candidate;0x2541d0 removes consumed bytes before testing the return. Successful parse does not imply runV25 handler success or a device state confirmation",
            "wled_parser": "0x255630 scans AA framing in buffer+736, using5-byte header and BE16 total. Payload<0x400 limits accepted payload to1023 bytes/total1028 despite the outer total<=0x405 gate. There is no outer checksum. It can accept the first complete frame; cmd0 enters inner V25 and nonzero cmd enters0x261480. Its finite residual-buffer loop is not a proof of arbitrary-garbage recovery",
            "keepalive": "Every frame entering runV25/runWled signals callback+248/+88 before command dispatch. KeepAlive is not the only feed.0x24bd30 starts retry4/interval8s; timeout checks retry<1 before decrement, chooses interval0 while retry>=1 and4s after decrement to0, then closes on another timeout. Parse notification changes a local wait state but never replenishes retry. SetWifiConfiguration0x2626a0 sets30s by a20-byte command-prefix comparison; V25 class0x12/cmd5 or Wled class0x4e4554/cmd5 returns30s to8s. Callback stop+705 suppresses duplicate Status14",
            "decoded_events": "V25/Wled handlers0x2600d0/0x261480 each set and signal both+248 and+88 in that order, independently of business confirmation. V25 class16 updates firmware wait slots without this audit executing firmware operations. KeepAlive command2 decoders0x13bfc0/0x13c210 use package.status only on a match, otherwise5. Found V25 handlers publish event1 with cmdType1 for class0x12 or2 otherwise; Wled uses cmdType1. Missing handlers publish Status15/cmdType0 and return-20994, which the parser caller only logs",
            "callback_abi": "global47e120 -> object45e2a8 -> vtable3eb8f8[0] ->0x6ab9 ->0x23e390 ->0x6721 ->0x246ca0 copies each registered callback into a72-byte detached-thread capture.0x2374 ->0x23f860 ->0x2b8f ->0x25a100 invokes four arguments confirmed in RCX/RDX/R8/R9: rawFn(registrationId,event,cmdType,const char* JSON). Add/Remove0x26d030/0x26d830 lock mutex45e140 and modify map47e220; erase does not wait for captures already copied. Notification completion order need not match publication order",
            "node_projection": "Native0x38690 receives id0/event1/cmdType0 for Status14.0x59370 converts id and cmdType to decimal JSON strings; event0/1/2 names BLEEvent/NETEvent/MDNSEvent, others UnknowEvent. Parseable payload is deep-copied through0x3a350; unparseable payload remains a string through0x426e0. Typical envelope is eventId:string0,event:NETEvent,cmdType:string0,data:{Host,Status:14}. No VID/PID/Mac is supplied by that disconnect payload",
            "detached_lifecycle": "Each reader/keepalive worker retains40 bytes containing strong callback and socket references plus workerFn.0xa010 ->0x23f950 also passes weak references, but deleting host maps does not invalidate retained strong captures. Worker return0x5556 ->0x2964ec ->0x76b7 ->0x242db0 releases captures. SocketTransfer strong-zero0x8076 ->0x2442a0 ->0x835a ->0x243e50 ->0x8b89 ->0x243060 frees strings without a fallback closesocket; explicit0x247510 owns closing. SocketCallBack destruction0x419c ->0x244290 ->0x58e4 ->0x243e00 ->0xa0b5 ->0x2430e0 releases receive buffers/maps. Weak-zero releases control blocks separately. No worker join is present",
            "uninit": "IoTNative Init0x346e0 -> 0x34170 verifies and loads its fixed current SDK file and eleven required exports. UnInit0x34c60 calls FreeLibrary, ignores BOOL, clears hLibModule and returns true when a handle existed, false otherwise. It does not unregister callback, disconnect, stop discovery, clear export pointers or join queues. The separate CRT-owned manager cleanup0x6e1b0 -> 0x1cc0 joins queues and frees shared state; UnInit does not invoke it",
            "static_vectors": [
                {"command": "SetChromaFrameV2", "payload_hex": "008400000000112233",
                 "selector1_hex": "0000001300090f070084000000001122339600",
                 "selector2_parts": ["aa00005f00", "0000005a00090f07", "008400000000112233", "00 * 71", "df00"]},
                {"command": "SetChromaFrame", "payload_hex": "00000000001122337d",
                 "selector1_hex": "0000001300090f0300000000001122337d6b00",
                 "selector2_parts": ["aa00005f00", "0000005a00090f03", "00000000001122337d", "00 * 71", "2200"]}],
            "receive_static_vectors": {
                "source": "instruction-derived boundary calculations, not DLL execution samples",
                "tuple_fields": ["status", "skip", "consumed"],
                "extended_frame_hex": "0200000a000012021a00",
                "cases": [
                    {"input": "F", "check_xor": True, "result": [2, 0, 10]},
                    {"input": "F[:9]", "check_xor": True, "result": [-21082, 0, 0]},
                    {"input": "F+F", "check_xor": True, "result": [-21082, 0, 10]},
                    {"input": "F with XOR1a replaced by1b", "check_xor": True, "result": [-21082, 0, 10]},
                    {"input": "F with terminal00 replaced byff", "check_xor": True, "result": [2, 0, 10]},
                    {"input": "ff+F, directly at parser", "check_xor": True, "result": [2, 1, 11]},
                    {"input": "0200005a00001202 + 00*80 + 4b00", "check_xor": False, "result": [2, 0, 90]},
                    {"input": "same fixed frame/status2", "check_xor": True, "result": [2, 0, 90]}]},
            "target_execution": False, "actual_dll_comparison": "unverified"}
        report["lighting_default_restore_contract"] = {
            "producer": "configureDevicesDefaultData retains only ten explicit rzDevice names, a device-info entry outside Chroma Studio regions and a callable mode builder. It builds normal-mode first, then optional user/synchronized/default/special quick-effect data, and emits a device only when its mode frame exists. These are registered exit commands, not a factory-reset or read-failure initialization policy",
            "names": ["rzDevice25", "rzDevice25LedMatrix", "rzDevice25Oled", "rzDevice25Linker",
                      "rzDevice25Glitter", "rzDevice25GlitterWireless", "rzDevice25DualLinkMouse",
                      "rzDevice25DualLinkKeyboard", "rzDevice25LedMatrixBatchSender",
                      "rzDevice25LedMatrixSkipSetEffect"],
            "frame": "Base mode header is [2,0,4] with payload [0,0]; effect header is [80,15,2] with actual size 6+colorArray.length. _createDataSend returns 90 bytes, transaction at 1, size/class/id at 5/6/7, payload at 8 and XOR of bytes [2,88) at 88. The base transaction increments 0..30 then wraps; this is distinct from native frame translator counters",
            "class_overrides": "All fourteen current physical definitions of the ten gated names retain complete classes and actual base bindings. Linker's four copies OR transaction with 0xe0, DualLinkKeyboard with 0x80; DualLinkMouse retains untagged 0..30. Matrix, MatrixBatch and Skip inherit the matrix effect builder, mapping common effects to class03/id0a and matrix-specific lengths; Fire instead uses class0f/id02 with nine bytes. Other gated classes inherit the base effect builder. These findings do not prove every product's DeviceInfo default/restore policy",
            "registration": "hid.configureDevicesDefaultData clones the array and stores the PID list, sets useDefaultCmdNoAsync=true, and may immediately rerun restore when both quit flags are already set. Its retained producer has no USB instance/usage fields",
            "async": "hid.setDevicesToDefault sets frames=false, then returns without sending when useDefaultCmdNoAsync=true. Otherwise it acquires the selected device mutex and sends only protocol 25, with no ACK. Its 90-byte branch prefixes d.payload?.reportId ?? t.claimInterface, unlike the per-device reportId used by the sync branch. Errors are logged; the result is undefined",
            "sync": "Clear sentSetDeviceModeCommands and frames, clone only for maximum command count, busy-wait 10ms, then 3ms per command index and pass the original stored array. No device mutex is acquired; a repeated full key in one index waits 2ms. Protocol 25 or omitted protocol with a 90/91-byte frame enters send. Missing sync support invokes async then/catch without waiting",
            "retry": "At most three sync sends. Accepted send lengths 91/65 are not ACKs. Only [size2,class00,id04], [class03,id0a] and [class0f,id02] trigger GET: respectively five, three and three attempts. Delay is min(device.delay ?? 5,30), then delay+3. Default producer delay is 3. A set-mode identity/category is recorded before any GET succeeds",
            "ack": "getCommandSendStatus accepts only arrays of 90/91/65 bytes, reads status at 0 or 1 and class/id at 6/7 or 7/8. It checks class/id only, not transaction, checksum, payload or requested target value. Status 0/1/2/3/4/5 means new/busy/success/failed/timeout/notSupported; command mismatch is notSupported. GET busy/timeout keep reading, other non-success statuses request resend. Exhausted busy/timeout or read exceptions can leave resend=false and break without confirmation",
            "acceptance": "Log completion, undefined return, accepted send length and the pre-ACK sentSetDeviceModeCommands set must remain separate from observed device state. Full product DeviceInfo defaults, actual errors and device recovery still require per-product implementation and actual DLL/device comparison",
            "target_execution": False, "actual_dll_comparison": "unverified"}
        report["lighting_protocol_contract"] = {
            "original": rpc_originals["LightingDriverDLL"],
            "registry_owner_rvas": ["0x2e4f0", "0x2e500", "0x9240"],
            "metadata": {"category_u8": 0, "report_id_u8": 1, "device_handle_u32": 4,
                         "write_function_string": 8, "protocol_string": 40, "row_delay_u8": 72,
                         "max_rows_u8": 73, "batch_bool": 74, "wdl_bool": 75},
            "translators": [{"protocol": name, "constructor_rva": hex(ctor), "vtable_rva": hex(table),
                "translate_rva": hex(translate), "report_length": length}
                for name, ctor, table, translate, length in (
                    ("rzDevice25", 0x2edd0, 0xb3968, 0x7feb0, 91),
                    ("rzDevice25Chroma65", 0x2ee70, 0xb3b58, 0x80460, 65),
                    ("rzDevice25LedMatrix", 0x2ef10, 0xb3c68, 0x80a00, 91),
                    ("rzDevice25LedMatrixSkipSetEffect", 0x2ef10, 0xb3c68, 0x80a00, 91),
                    ("rzDevice25LedMatrixBatchSender", 0x2efb0, 0xb3d78, 0x811b0, 375),
                    ("rzDevice25Glitter", 0x2f050, 0xb3e88, 0x85340, 321),
                    ("rzDevice25GlitterWireless", 0x2f050, 0xb3e88, 0x85340, 381),
                    ("rzDevice25MarciT1", 0x2f0f0, 0xb3f98, 0x85610, 781),
                    ("rzDevice25AudioCamy", 0x2f190, 0xb40a8, 0x85930, 65),
                    ("rzDeviceIoT", 0x2f230, 0xb41b8, 0x85d20, "6 + RGB, maximum 726"),
                    ("rzDeviceLampArray", 0x2f360, 0xb42c8, 0x85fa0, "unframed RGB length"))],
            "shared_counters": "One translator instance is retained per protocol key, not per device. 25/65/Matrix/MatrixBatch start at u8=1, Camy at zero. Ordinary transaction is (q&31)|(category<<5); Camy uses its full u8 counter and includes it in its checksum",
            "return_boundaries": "25/65/Matrix/MatrixBatch consume writer bool, but their payload-too-large/insufficient-source branches can return true without writing. Glitter/Marci/Camy/IoT/LampArray ignore writer results. Batch flush 0x7e710 has its bool discarded by 0xf4d0. FFI writer 0x7ec90 invokes a void JSON callback and returns true; host callbacks do not await or return the final HID/IoT/LampArray Promise",
            "original_anomalies": "Glitter 0x8543b compares row RGB length <= sendLength+6, not minus six. Glitter and Marci fallback reuse the logging stack buffer without whole-report initialization; report byte zero retains 0x5b and trailing bytes are not deterministic. Matrix extra SetEffect leaves report byte zero at zero; MatrixBatch row byte is fixed zero. Preserve these facts without inventing zero padding or silently normalizing actual comparisons",
            "batch_parser": "Current HID 0x16370 scans from offset zero without checking 37 13 fe ca. f0 reads u8 report length, f2 u32LE, f1 u8 delay; unknown bytes are skipped. Truncated length fields or zero length fail, but report-body remaining length is not checked before writer. 0x13700 performs actual HidD_SetFeature, returns requested length or -1, with no ACK/readback. 0x4800 rejects -1, otherwise resolves the accumulated result",
            "marci_io": "HID 0x9750 queues request type4 to hid_write 0x175e0: pad short reports to OutputReportByteLength, overlapped WriteFile, and on ERROR_IO_PENDING wait1000ms then GetOverlappedResult. It is an OutputReport path, not HidD_SetFeature; its complete owner only assigns transferred count in the pending completion branch",
            "shutdown": "Export 0x2550 only clears the global callback pointer then calls TimerResWatcher stop 0x30610: set stop, notify, join timer thread. It does not invoke LightingDriver destructor 0x2e0e0 or drain host HID Promises. The separate destructor releases writer/protocol/metadata maps, FFI writer and retained function objects. Host shutdown only calls Shutdown and clears libFFI; device/callback maps are retained",
            "hid_cleanup": "StopLightingFrames only clears a source flag. Remove closes handles/cancels device mutexes without an explicit batch-Promise barrier. Native close 0x6170 cancels I/O and frees the HID allocation before shared worker release. Worker destructor 0x4710 sets stop, notifies, joins and destroys queue; its inner queue drain does not recheck stop for each request. TSFN finalizer 0x161e0 -> 0x15d40 releases request ownership after result consumption",
            "target_execution": False, "official_dll_comparison": "unverified"}
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        report["failures"].append(str(error))
    report["complete_originals"] = list(audit.sources.values())
    report["complete_source_owners"] = list(audit.owners.values())
    report["semantics"] = {
        "settings_mount": "The actual ia widget is hidden unless build >=22631 or build=22621/revision>=2506. It observes isDynamicLighting storage and wdl-devices WindowStorage switching state, and removes both subscriptions on unmount.",
        "settings_toggle": "The source flips its own state and writes isDynamicLighting immediately; no Apply or device-ACK rollback is present in this owner. Telemetry is excluded from the local product scope.",
        "settings_os_link": "The source invokes the local msSettings(personalization-lighting) adapter.",
        "switching": "The engine serializes storage events with its existing mutex, parses the value, awaits updateRazerWDLDevices and emits its mode event. Transfer sets wdl-devices.isSwitching true and clears it in finally.",
        "driver": "Configure receives {type:mode.set,skip_wdl_devices:useWDL}; this caller does not await its Promise.",
        "to_wdl": "Remove compatible Razer Chroma devices, then add enumerated Razer LampArray devices through their per-device mutex and local WindowStorage/cache owners.",
        "to_chroma": "Remove Razer LampArray Chroma entries, close and remove their LampArray devices, enumerate local USB devices, prepare matching containers with a one-second delay per device, then await readiness and the source two-second settling delay when there are readiness tasks.",
        "lamparray_gate": "The separate native-init source accepts Windows major>10, or major10/minor>0, or major10/minor0 with build>22621 or build22621/revision>=2506. This differs from the settings widget gate and must not be silently unified.",
        "native_lifecycle": "Source gates win32 and OS support, configures the explicit current FFI signature table, calls SetupRPC then Startup then void SetNodeFFIEvent then StartWatcher. It returns false on failed ConfigureFFI/SetupRPC/Startup/StartWatcher without introducing rollback. Returned strings require their matching native deallocator.",
        "driver_dispatch": "The actual PE pointer table at RVA 0xafc30 maps mode.set to RVA 0xb790. It accepts a JSON object, conditionally writes boolean pause at +672 and skip_wdl_devices at +673, and returns true even when neither field was changed.",
        "driver_skip_consumer": "ProcessDeviceFrame at RVA 0xf4d0 reads +673. If device flag +75 is exactly 1 and skip_wdl_devices bit 0 is set, it returns true without invoking the protocol translator. Other devices continue through their registered protocol's vtable +8; successful mode.set alone does not prove device frames were written.",
        "lamparray_rpc": "JsonApi reaches LampRpcClient::JsonApi RVA 0x7c5e0, then NdrClientCall2 through RVA 0xa4340. InitRpc composes ncacn_np with the local named pipe \\pipe\\rzwdl_%s; this is required local IPC. Both service and reverse-callback format bytes, server/callback routine tables and complete dispatch bodies are retained; bounded native expansion does not resolve all indirect calls.",
        "driver_response": "Configure 0x1460 -> 0x6b90 parses JSON, invokes type dispatch and returns malloc-owned UTF-8 JSON with succeeded boolean. The source ffiLightingDriver reads/parses this then uses FreeString. Startup and Shutdown are declared void in this actual source, despite inferred native residual return values.",
    }
    report["remaining_chains"] = [
        "Compare the retained settings root/parents, ten locale dictionaries, four SVG originals, tooltip positioning and actual hover/disabled/keyboard behavior against both Rust settings entries. Static source closure is not window or pixel acceptance.",
        "All eleven registered LightingDriver translators now have explicit vtable targets, report layouts and return consumers through host/HID/IoT SDK. IoT queue/drop, selector, V25/wrapper bytes, actual UDP writer, discarded results and disconnect/UnInit cleanup have selected retained owners. Default-restore producer, ten-name gate, all fourteen physical class definitions, matrix/base builders, both senders, ACK classifier and retry/cleanup are retained; separately finish each product's DeviceInfo defaults, other IoT command/product branches and actual DLL/device comparison. Selected static results do not establish full native or product coverage.",
        "Compare and implement the actual local storage/WindowStorage, LampArray and lighting-driver service consumers after source review; the existing Rust SettingsEvent handler only saves a draft and displays a status.",
        "Verify device removal races, callbacks already queued at unregister, exceptional native destruction and incomplete source cleanup through independent Rust tests and actual DLL comparison. Static Start/Stop, Shutdown and destructor owners have been traced; their existence does not establish runtime completion."]
    report["verified_artifacts"] = list(audit.artifacts.values())
    report["changed_during_query"] = audit.final_snapshot()
    report["snapshot_stable"] = not report["changed_during_query"]
    report["failures"].extend("Audit artifact changed during query: " + name for name in report["changed_during_query"])
    destination = a.ROOT / ".analysis/results/host-current/local-audit.json"
    with a.index_lock(destination.with_suffix(".lock")):
        previous = json.loads(destination.read_bytes()) if destination.is_file() else {"schema_version": 1}
        previous[args.query.replace("-", "_")] = report
        a.save(destination, previous)
    print(json.dumps({"receipt": a.rel(destination), "query": args.query,
                      "complete_source_owners": len(audit.owners), "failures": report["failures"]}), flush=True)
    if report["failures"]:
        raise ValueError("Local host source query remains unresolved")


def run(args):
    {"acquire-installed": acquire_installed, "acquire-host": acquire_host, "inspect-host": inspect_host, "analyze-host": analyze_host}[args.command](args)
