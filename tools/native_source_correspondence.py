"""Reassemble an explicitly reviewed native source closure without executing it."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import tempfile

import native_adjudication as evidence


_SOURCE = Path(__file__).read_bytes()
_IMAGE_SHA256 = "f64c098b67eee3c769416564c4abc32e605eaa9a1804e41c03e74b4713d04333"
_SEMANTIC_REVIEW_SHA256 = "a669e5958f5aacaf4d7ea8936b93807d5969be8482c26277f5eca160a04e4051"
_ASSEMBLER_SHA256 = "a776cf32777d2df2e201a769bb5bd6502bb72bac98f60c6f53a4bfcecd0db8ed"
_ASSEMBLER_PATH = Path("C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/ml64.exe")
_SOURCE_FILES = {"SconvKernelAvx512F.asm", "SconvKernelCommon.inc", "mlasi.inc", "macamd64.inc"}
_OWNERS = {
    "0xa3ae20": ("MlasConvNchwFloatKernelAvx512F", "Nchw", "0x9e0b70", "0x9e0e41"),
    "0xa3bf00": ("MlasConvNchwcFloatKernelAvx512F", "Nchwc", "0x9e0610", "0x9e09a4"),
    "0xa41a20": ("MlasConvDepthwiseFloatKernelAvx512F", "Depthwise", "0x9e0210", "0x9e051c"),
}


def source_identity(root):
    path = Path(__file__).resolve()
    if path.read_bytes() != _SOURCE:
        raise ValueError("Source-correspondence validator changed after import")
    return {"path": path.relative_to(root).as_posix(), "bytes": len(_SOURCE),
            "sha256": hashlib.sha256(_SOURCE).hexdigest()}


def _range(raw, offset, size, label):
    if (type(offset) is not int or type(size) is not int or offset < 0 or size < 0
            or offset > len(raw) or size > len(raw) - offset):
        raise ValueError("Truncated or invalid " + label)
    return raw[offset:offset + size]


def _unpack(fmt, raw, offset, label):
    return struct.unpack(fmt, _range(raw, offset, struct.calcsize(fmt), label))


def _cstring(raw, start, label):
    if not 0 <= start < len(raw):
        raise ValueError("Invalid " + label)
    end = raw.find(b"\0", start)
    if end < 0:
        raise ValueError("Unterminated " + label)
    try:
        return raw[start:end].decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError("Non-ASCII " + label) from error


@dataclass(frozen=True)
class CoffSymbol:
    name: str
    value: int
    section: int
    type: int
    storage: int


@dataclass(frozen=True)
class CoffRelocation:
    offset: int
    symbol_index: int
    type: int


@dataclass(frozen=True)
class CoffSection:
    name: str
    data: bytes
    characteristics: int
    relocations: tuple[CoffRelocation, ...]


@dataclass(frozen=True)
class CoffObject:
    sections: tuple[CoffSection, ...]
    symbols: dict[str, CoffSymbol]
    symbol_indexes: dict[int, CoffSymbol]


def parse_coff(raw):
    """Parse ordinary AMD64 COFF, including actual symbol-indexed relocations."""
    machine, count, _timestamp, symoff, symcount, optional, _flags = _unpack(
        "<HHIIIHH", raw, 0, "COFF header")
    if machine != 0x8664 or optional != 0 or not 1 <= count <= 96:
        raise ValueError("Source correspondence requires an ordinary AMD64 COFF object")
    _range(raw, 20, 40 * count, "COFF section table")
    _range(raw, symoff, 18 * symcount, "COFF symbol table")
    string_offset = symoff + 18 * symcount
    string_bytes = _unpack("<I", raw, string_offset, "COFF string-table header")[0]
    if string_bytes < 4:
        raise ValueError("Invalid COFF string-table length")
    strings = _range(raw, string_offset, string_bytes, "COFF string table")

    def string_name(offset):
        if offset < 4:
            raise ValueError("COFF string offset aliases its length header")
        return _cstring(strings, offset, "COFF long name")

    def short_name(data):
        try:
            return data.split(b"\0", 1)[0].decode("ascii")
        except UnicodeDecodeError as error:
            raise ValueError("Non-ASCII COFF short name") from error

    symbols, indexes = {}, {}
    i = 0
    while i < symcount:
        entry = _range(raw, symoff + i * 18, 18, "COFF symbol")
        name = string_name(struct.unpack_from("<I", entry, 4)[0]) if entry[:4] == b"\0" * 4 else short_name(entry[:8])
        value, section, typ, storage, aux = struct.unpack_from("<IhHBB", entry, 8)
        if i + 1 + aux > symcount or section > count or section < -2:
            raise ValueError("Invalid COFF symbol section or auxiliary-record count")
        symbol = CoffSymbol(name, value, section, typ, storage)
        indexes[i] = symbol
        if name in symbols:
            raise ValueError("Ambiguous duplicate COFF symbol name: " + name)
        symbols[name] = symbol
        i += aux + 1

    sections = []
    for i in range(count):
        header = _range(raw, 20 + 40 * i, 40, "COFF section")
        name = short_name(header[:8])
        if name.startswith("/"):
            try:
                name = string_name(int(name[1:]))
            except ValueError as error:
                raise ValueError("Invalid COFF section long name") from error
        size, data_offset, relocation_offset, _line_offset, relocations, _lines, flags = struct.unpack_from("<IIIIHHI", header, 16)
        if flags & 0x1000000:
            raise ValueError("COFF relocation-overflow encoding is outside this reviewed source closure")
        data = _range(raw, data_offset, size, "COFF section bytes")
        _range(raw, relocation_offset, 10 * relocations, "COFF relocations")
        decoded = []
        for j in range(relocations):
            offset, symbol_index, typ = _unpack("<IIH", raw, relocation_offset + 10 * j, "COFF relocation")
            if offset >= size or symbol_index not in indexes:
                raise ValueError("COFF relocation points outside its section or to an auxiliary symbol")
            decoded.append(CoffRelocation(offset, symbol_index, typ))
        sections.append(CoffSection(name, data, flags, tuple(decoded)))
    return CoffObject(tuple(sections), symbols, indexes)


def relocate_function(obj, name, rva, size, addresses):
    """Resolve REL32 and REL32_1..5 using the real COFF symbol and signed addend."""
    symbol = obj.symbols.get(name)
    if symbol is None or not 1 <= symbol.section <= len(obj.sections):
        raise ValueError("Reviewed function has no defined COFF symbol: " + name)
    section = obj.sections[symbol.section - 1]
    if not section.characteristics & 0x20:
        raise ValueError("Reviewed source function is outside a COFF code section")
    code = bytearray(_range(section.data, symbol.value, size, "COFF function bytes"))
    relocations, occupied = [], set()
    for row in section.relocations:
        if row.offset >= symbol.value + size or row.offset + 4 <= symbol.value:
            continue
        local = row.offset - symbol.value
        if not 4 <= row.type <= 9 or local < 0 or local + 4 > size:
            raise ValueError("Unsupported or boundary-crossing function relocation")
        if occupied.intersection(range(local, local + 4)):
            raise ValueError("Overlapping function relocations")
        occupied.update(range(local, local + 4))
        target = obj.symbol_indexes[row.symbol_index]
        if target.name not in addresses:
            raise ValueError("Function relocation target is outside the reviewed original closure: " + target.name)
        addend = int.from_bytes(code[local:local + 4], "little", signed=True)
        location = rva + local
        value = addresses[target.name] + addend - location - 4 - (row.type - 4)
        if not -(1 << 31) <= value < (1 << 31):
            raise ValueError("COFF REL32 resolution overflows its signed displacement")
        code[local:local + 4] = value.to_bytes(4, "little", signed=True)
        relocations.append({"offset": local, "original_rva": hex(location), "target_name": target.name,
            "target_rva": hex(addresses[target.name]), "type": row.type, "addend": addend,
            "resolved_value": value})
    return bytes(code), relocations


@dataclass(frozen=True)
class Pe64:
    raw: bytes
    image_base: int
    sections: tuple[tuple[int, int, int], ...]

    @classmethod
    def parse(cls, raw):
        if _range(raw, 0, 2, "DOS header") != b"MZ":
            raise ValueError("Source image lacks the DOS signature")
        nt = _unpack("<I", raw, 0x3c, "DOS PE offset")[0]
        if _range(raw, nt, 4, "PE signature") != b"PE\0\0":
            raise ValueError("Source image lacks the PE signature")
        machine, count = _unpack("<HH", raw, nt + 4, "PE COFF header")
        optional = _unpack("<H", raw, nt + 20, "PE optional-header size")[0]
        _range(raw, nt + 24, optional, "PE optional header")
        if (machine != 0x8664 or not 1 <= count <= 96 or optional < 112
                or _unpack("<H", raw, nt + 24, "PE64 magic")[0] != 0x20b):
            raise ValueError("Source correspondence requires a complete AMD64 PE32+ image")
        image_base = _unpack("<Q", raw, nt + 48, "PE64 ImageBase")[0]
        sections = []
        for i in range(count):
            header = _range(raw, nt + 24 + optional + i * 40, 40, "PE section table")
            _virtual_size, va, raw_size, raw_offset = struct.unpack_from("<IIII", header, 8)
            _range(raw, raw_offset, raw_size, "PE section data")
            if any(va < old_va + old_size and old_va < va + raw_size for old_va, old_size, _ in sections):
                raise ValueError("Ambiguous overlapping PE raw RVA ranges")
            sections.append((va, raw_size, raw_offset))
        return cls(raw, image_base, tuple(sections))

    def at(self, rva, size):
        matches = [(start, count, offset) for start, count, offset in self.sections
                   if start <= rva and rva + size <= start + count]
        if len(matches) != 1 or size <= 0:
            raise ValueError("Original function is outside a unique complete PE raw range")
        start, _count, offset = matches[0]
        return _range(self.raw, offset + rva - start, size, "PE function bytes")


def _identity(record):
    if not isinstance(record, dict) or not {"path", "bytes", "sha256"}.issubset(record):
        raise ValueError("Source reference lacks an exact byte identity")
    return {key: record[key] for key in ("path", "bytes", "sha256")}


def _original_function(a, row, pe, capture_path, indexed):
    reference = _identity(row["function"])
    path = a.resolve(reference["path"], ".analysis/results/native")
    if path.parent != capture_path.parent / "functions":
        raise ValueError("Reviewed function is outside its exact base capture")
    body = json.loads(evidence.checked(a, reference, ".analysis/results/native"))
    rva = int(row["rva"], 0)
    if (body.get("rva") != hex(rva) or body.get("address") != hex(pe.image_base + rva)
            or indexed.get(path.name) != reference["sha256"]
            or not isinstance(body.get("chunks"), list) or len(body["chunks"]) != 1):
        raise ValueError("Reviewed function differs from its indexed original coordinates")
    chunk = body["chunks"][0]
    try:
        original = bytes.fromhex(chunk["raw_hex"])
    except (KeyError, ValueError) as error:
        raise ValueError("Reviewed function has invalid complete raw bytes") from error
    if (not original or int(chunk["start"], 0) != pe.image_base + rva
            or int(chunk["end"], 0) != pe.image_base + rva + len(original)
            or body.get("end") != chunk["end"] or len(original) != row["machine_bytes"]
            or hashlib.sha256(original).hexdigest() != chunk["sha256"]
            or pe.at(rva, len(original)) != original):
        raise ValueError("Reviewed function chunk differs from the complete original image bytes")
    cursor, calls = rva, []
    assembly = body.get("assembly")
    if not isinstance(assembly, list) or len(assembly) != row["instruction_count"]:
        raise ValueError("Reviewed function lost its original instruction inventory")
    for ins in assembly:
        data = bytes.fromhex(ins["bytes"])
        if (not data or int(ins["rva"], 0) != cursor or int(ins["address"], 0) != pe.image_base + cursor
                or pe.at(cursor, len(data)) != data):
            raise ValueError("Reviewed function instruction bytes or boundaries changed")
        if data[0] == 0xe8 and len(data) == 5:
            target = cursor + 5 + int.from_bytes(data[1:], "little", signed=True)
            edges = [int(x["to"], 0) - pe.image_base for x in ins["xrefs_from"] if x.get("code") and x.get("type") == 17]
            if edges != [target]:
                raise ValueError("Original CALL displacement and captured call edge differ")
            calls.append(target)
        cursor += len(data)
    if cursor != rva + len(original):
        raise ValueError("Reviewed instructions do not cover all original function bytes")
    return body, original, calls


def _source_closure(a, record, image):
    acquisition = json.loads(evidence.checked(a, record, ".analysis/inputs"))
    if (acquisition.get("source_image") != image or acquisition.get("dependency_closure_complete") is not True
            or acquisition.get("target_execution") is not False or acquisition.get("semantics_complete") is not False):
        raise ValueError("Source acquisition changed its original image or evidence-only boundary")
    rows = acquisition.get("files", [])
    if (len(rows) != 4 or {Path(x["path"]).name for x in rows} != _SOURCE_FILES
            or len({str(Path(x["path"]).parent) for x in rows}) != 1):
        raise ValueError("Source acquisition omitted the exact four-file include closure")
    result = {}
    for row in rows:
        raw = evidence.checked(a, _identity(row), ".analysis/inputs")
        name = Path(row["path"]).name
        includes = re.findall(r"^\s*include\s+([^;\s]+)", raw.decode("utf-8-sig"), re.M | re.I)
        if includes != row.get("includes") or any(x not in _SOURCE_FILES for x in includes):
            raise ValueError("Source include dependencies differ from the acquired complete closure")
        result[name] = raw
    return result


def _assemble(a, sources, assembler):
    """Own private source snapshots and all assembler output on every exit path."""
    compiler = _ASSEMBLER_PATH
    if (assembler.get("sha256") != _ASSEMBLER_SHA256 or not compiler.is_file()
            or compiler.stat().st_size != assembler.get("bytes")
            or hashlib.sha256(compiler.read_bytes()).hexdigest() != _ASSEMBLER_SHA256):
        raise ValueError("The exact reviewed ML64 assembler is not available")
    temp_root = a.resolve("target/verification-temp", "target")
    temp_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="source-assemble-", dir=temp_root) as directory:
        temporary = Path(directory)
        for name, raw in sources.items():
            if name not in _SOURCE_FILES:
                raise ValueError("Private assembly input is outside the owned include closure")
            (temporary / name).write_bytes(raw)
        output = temporary / "kernel.obj"
        environment = os.environ.copy()
        for variable in ("TEMP", "TMP", "TMPDIR"):
            environment[variable] = str(temporary)
        command = [str(compiler), "/nologo", "/c", "/I" + str(temporary), "/Fo" + str(output),
                   str(temporary / "SconvKernelAvx512F.asm")]
        completed = subprocess.run(command, capture_output=True, timeout=45, cwd=temporary, env=environment)
        if completed.returncode != 0 or not output.is_file():
            raise ValueError("Static ML64 source assembly failed: " + completed.stderr.decode(errors="replace"))
        raw = output.read_bytes()
    if temporary.exists():
        raise ValueError("Owned static assembly temporary directory was not released")
    return raw


def _semantic_boundary(review, identity):
    if (identity["sha256"] != _SEMANTIC_REVIEW_SHA256 or review.get("schema_version") != 1
            or review.get("review_kind") != "onnx_avx512_original_three_owner_complete_static_semantics"
            or review.get("static_semantics_review_complete") is not True
            or review.get("original_machine_bytes_and_current_cpp_callers_reviewed") is not True
            or any(review.get(key) is not False for key in ("semantics_complete", "rust_independent_implementation_complete",
                "dll_function_execution_verified", "target_execution", "debugger_used", "database_mutation"))):
        raise ValueError("Source review is not the explicitly approved manual semantic evidence boundary")
    coverage = review.get("instruction_coverage", {})
    if (len(coverage.get("inner_compute_blocks", [])) != 36 or len(coverage.get("postprocess_variants", [])) != 16
            or coverage.get("all_active_accumulator_clear_fma_store_sets_verified") is not True
            or len(review.get("actual_consumers", [])) != 3):
        raise ValueError("Manual semantic review omitted a complete original branch or consumer")
    for rva, (_name, kind, execute, call) in _OWNERS.items():
        consumers = [x for x in review["actual_consumers"] if x.get("kind") == kind]
        if (len(consumers) != 1 or consumers[0].get("execute_rva") != execute
                or consumers[0].get("actual_cfg_indirect_call_rva") != call):
            raise ValueError("Manual review changed an exact actual original consumer")
    failures = review.get("capture_integrity", {}).get("original_failures_preserved", {})
    if (failures.get("no_hexrays_success_claim") is not True or len(failures.get("pseudocode_failures", [])) != 3
            or any("code -12" not in x.get("error", "") for x in failures["pseudocode_failures"])):
        raise ValueError("Manual review omitted the three preserved original Hex-Rays failures")


def _review_captures(a, review, pe, capture_path, indexed):
    """Recheck the manual dispatch/consumer witnesses against current original bytes."""
    rows = list(review.get("capture_integrity", {}).get("byte_checks", []))
    rows += review.get("dispatch", {}).get("consumer_scan", {}).get("byte_verification", [])
    checked = {}
    for row in rows:
        reference = _identity(row["function"])
        previous = checked.get(reference["path"])
        if previous:
            if previous != reference:
                raise ValueError("Manual capture witnesses disagree about the same function")
            continue
        path = a.resolve(reference["path"], ".analysis/results/native")
        if path.parent != capture_path.parent / "functions" or indexed.get(path.name) != reference["sha256"]:
            raise ValueError("Manual caller witness is outside its exact indexed capture")
        body = json.loads(evidence.checked(a, reference, ".analysis/results/native"))
        if body.get("rva") != row["rva"] or len(body.get("assembly", [])) != row["instruction_count"]:
            raise ValueError("Manual caller witness changed its original function/instruction inventory")
        chunk_rows = []
        for chunk in body["chunks"]:
            data = bytes.fromhex(chunk["raw_hex"])
            rva = int(chunk["start"], 0) - pe.image_base
            if (int(chunk["end"], 0) - int(chunk["start"], 0) != len(data)
                    or hashlib.sha256(data).hexdigest() != chunk["sha256"] or pe.at(rva, len(data)) != data):
                raise ValueError("Manual caller chunk differs from original image bytes")
            chunk_rows.append({"rva": hex(rva), "bytes": len(data), "sha256": chunk["sha256"],
                               "original_bytes_equal": True})
        instruction_bytes = 0
        for instruction in body["assembly"]:
            data = bytes.fromhex(instruction["bytes"])
            if pe.at(int(instruction["rva"], 0), len(data)) != data:
                raise ValueError("Manual caller instruction differs from original image bytes")
            instruction_bytes += len(data)
        if (chunk_rows != row["chunks"] or instruction_bytes != row["instruction_bytes_verified"]
                or row.get("all_instruction_bytes_equal_original") is not True):
            raise ValueError("Manual caller witness changed its complete original byte accounting")
        checked[reference["path"]] = reference
    return tuple(checked.values())


def verify_review(a, review_identity):
    """Return only the three byte-proven source owners, keeping cfunc/runtime separate."""
    review = json.loads(evidence.checked(a, review_identity))
    _semantic_boundary(review, review_identity)
    image_id, capture_id = _identity(review["source_image"]), _identity(review["capture"])
    if image_id["sha256"] != _IMAGE_SHA256:
        raise ValueError("Manual source review belongs to a different original image")
    pe = Pe64.parse(evidence.checked(a, image_id, ".analysis/inputs"))
    capture_path = a.resolve(capture_id["path"], ".analysis/results/native")
    capture = json.loads(evidence.checked(a, capture_id, ".analysis/results/native"))
    if (capture.get("input") != image_id["path"] or capture.get("input_sha256") != image_id["sha256"]
            or capture.get("input_bytes") != image_id["bytes"] or capture.get("image_base") != hex(pe.image_base)
            or review.get("image_base") != hex(pe.image_base) or capture_path.parent.name != image_id["sha256"]):
        raise ValueError("Source review changed its exact image/base-capture binding")
    index = {Path(x["path"]).name: x["sha256"] for x in capture["functions"]}
    retained_sources = []
    input_index = json.loads((a.ROOT / ".analysis/inputs.json").read_bytes())
    indexed_inputs = {x["path"]: x for x in input_index["inputs"]}
    for row in review["input_sources"]:
        evidence.checked(a, row, ".analysis/inputs")
        if _identity(indexed_inputs.get(row["path"], {})) != row:
            raise ValueError("Reviewed source dependency is not registered with its actual bytes")
        retained_sources.append(row)
    correspondence_id = _identity(review["assembly_correspondence_receipt"])
    correspondence = json.loads(evidence.checked(a, correspondence_id))
    if (correspondence.get("review_kind") != "onnx_avx512_original_assembly_source_correspondence"
            or correspondence.get("source_image") != image_id or correspondence.get("capture") != capture_id
            or correspondence.get("all_28_closure_bytes_equal") is not True
            or any(correspondence.get(key) is not False for key in ("target_execution", "database_mutation", "semantics_complete"))):
        raise ValueError("Assembly correspondence changed its exact original closure or evidence boundary")
    sources = _source_closure(a, correspondence["source_acquisition"], image_id)
    original_object = parse_coff(evidence.checked(a, correspondence["object"]))
    closure = correspondence.get("closure")
    if (not isinstance(closure, list) or len(closure) != 28
            or len({x["rva"] for x in closure}) != 28 or len({x["symbol_name_lead"] for x in closure}) != 28
            or not set(_OWNERS).issubset(x["rva"] for x in closure)):
        raise ValueError("Source correspondence omitted or duplicated a reviewed closure function")
    addresses = {x["symbol_name_lead"]: int(x["rva"], 0) for x in closure}
    functions, calls, originals = {}, {}, {}
    for row in closure:
        name = row["symbol_name_lead"]
        body, original, targets = _original_function(a, row, pe, capture_path, index)
        code, relocations = relocate_function(original_object, name, int(row["rva"], 0), len(original), addresses)
        if (code != original or row.get("bytes_equal") is not True or row.get("byte_differences") != []
                or row.get("original_sha256") != hashlib.sha256(original).hexdigest()
                or row.get("relocated_object_sha256") != hashlib.sha256(code).hexdigest()
                or row.get("relocations") != relocations):
            raise ValueError("Retained source object or machine correspondence differs from original bytes")
        functions[row["rva"]], calls[int(row["rva"], 0)], originals[name] = body, targets, original
    reached, pending = set(), [int(x, 0) for x in _OWNERS]
    while pending:
        rva = pending.pop()
        if rva in reached:
            continue
        if rva not in calls:
            raise ValueError("Original source call graph leaves the reviewed closure")
        reached.add(rva)
        pending.extend(calls[rva])
    if reached != set(addresses.values()):
        raise ValueError("Source correspondence contains unreachable or missing original helpers")
    for rva, (name, _kind, _execute, _call) in _OWNERS.items():
        if addresses.get(name) != int(rva, 0) or functions[rva].get("pseudocode") is not None:
            raise ValueError("Source owner changed its exact name mapping or preserved cfunc failure")
    manual_captures = _review_captures(a, review, pe, capture_path, index)
    new_object = parse_coff(_assemble(a, sources, correspondence["assembler"]))
    checks = []
    for row in closure:
        name = row["symbol_name_lead"]
        original = originals[name]
        rebuilt, relocations = relocate_function(new_object, name, addresses[name], len(original), addresses)
        if rebuilt != original:
            raise ValueError("Reassembled original function machine bytes differ: " + name)
        symbol = new_object.symbols[name]
        checks.append({"rva": row["rva"], "name": name, "machine_bytes": len(original),
            "machine_sha256": hashlib.sha256(rebuilt).hexdigest(), "object_section": symbol.section,
            "object_offset": symbol.value, "relocations": relocations, "bytes_equal": True})
    # The assembler reads private snapshots. Reject concurrently changed retained evidence too.
    evidence.checked(a, image_id, ".analysis/inputs")
    evidence.checked(a, capture_id, ".analysis/results/native")
    for row in retained_sources:
        evidence.checked(a, row, ".analysis/inputs")
    for row in closure:
        evidence.checked(a, row["function"], ".analysis/results/native")
    for row in manual_captures:
        evidence.checked(a, row, ".analysis/results/native")
    evidence.checked(a, correspondence_id)
    evidence.checked(a, correspondence["object"])
    evidence.checked(a, review_identity)
    source_identity_record = source_identity(a.ROOT)
    common = {"assembly_correspondence_receipt": correspondence_id,
        "source_acquisition": correspondence["source_acquisition"], "assembler": correspondence["assembler"],
        "function_count": 28, "closure": checks, "reassembled_bytes_equal": True,
        "manual_caller_capture_identities_rechecked": len(manual_captures),
        "coff_timestamp_compared": False, "real_coff_symbol_relocations_resolved": True,
        "owned_temporary_directory_removed": True}
    proofs = {}
    for rva, (_name, kind, _execute, _call) in _OWNERS.items():
        row = next(x for x in closure if x["rva"] == rva)
        consumer = next(x for x in review["actual_consumers"] if x["kind"] == kind)
        proofs[rva] = {"review": review_identity, "validator_source": source_identity_record,
            "source_input": image_id, "base_capture": capture_id, "function": row["function"], "rva": rva,
            "coverage_kind": "exact_original_avx512_assembly_source_and_manual_semantics",
            "source_assembly": common,
            "manual_semantic_review": {"receipt": review_identity, "static_semantics_review_complete": True,
                "compute_blocks": 36, "postprocess_variants": 16, "actual_consumer": consumer,
                "original_hexrays_failure": next(x for x in review["capture_integrity"]["original_failures_preserved"]["pseudocode_failures"] if x["stage"] == rva[2:] + ":hexrays")},
            "pseudocode_recovered": False, "pseudocode_synthesized": False,
            "runtime_acceptance": False, "rust_implementation_acceptance": False,
            "target_execution": False, "raw_artifacts_modified": False, "queue_status_modified": False}
    return {"complete_sources": {image_id["sha256"]: proofs}, "source_assembly": common,
        "validator_source": source_identity_record, "pseudocode_recovered": False,
        "pseudocode_synthesized": False, "runtime_acceptance": False, "target_execution": False}
