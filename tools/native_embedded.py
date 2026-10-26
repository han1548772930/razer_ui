"""Verify embedded-PE function ownership without executing or rewriting code.

Parent disassembly remains a historical interpretation in the parent's machine
mode. This validator proves a complete byte mapping to an independently captured
child function; it does not relabel that interpretation as a parent decompilation.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import struct
import tempfile


_IDENTITY = {"path", "bytes", "sha256"}
_SCHEMA = {"schema_version", "review_kind", "source_input", "base_capture", "child_input",
           "child_capture", "extraction_receipt", "mappings", "target_execution", "raw_artifacts_modified"}


def _integer(value, label="address"):
    if type(value) is int and value >= 0:
        return value
    if isinstance(value, str) and re.fullmatch(r"0x[0-9a-f]+", value) and hex(int(value, 16)) == value:
        return int(value, 16)
    raise ValueError("Embedded attribution has an invalid " + label)


def _path(a, name, boundary):
    if (not isinstance(name, str) or not name or "\\" in name or ":" in name
            or any(part in {"", ".", ".."} for part in name.split("/"))):
        raise ValueError("Embedded attribution requires a canonical project-relative path")
    candidate = a.ROOT / name
    current = candidate
    while current != a.ROOT:
        if current == current.parent or current.is_symlink() or (hasattr(current, "is_junction") and current.is_junction()):
            raise ValueError("Embedded attribution evidence follows a filesystem link")
        current = current.parent
    return a.resolve(name, boundary)


class _Evidence:
    def __init__(self, a):
        self.a = a
        self.identities = {}
        self.raw = {}

    def read(self, record, boundary=".analysis/results"):
        if (not isinstance(record, dict) or set(record) != _IDENTITY
                or type(record.get("bytes")) is not int or record["bytes"] <= 0
                or not isinstance(record.get("sha256"), str)
                or re.fullmatch(r"[0-9a-f]{64}", record["sha256"]) is None):
            raise ValueError("Embedded attribution requires a complete exact artifact identity")
        path = _path(self.a, record["path"], boundary)
        if record["path"] in self.identities:
            if self.identities[record["path"]] != record:
                raise ValueError("Embedded attribution contains conflicting artifact identities")
            return self.raw[record["path"]]
        raw = path.read_bytes()
        if len(raw) != record["bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
            raise ValueError("Embedded attribution artifact bytes changed: " + record["path"])
        self.identities[record["path"]] = dict(record)
        self.raw[record["path"]] = raw
        return raw

    def current(self, name, boundary=".analysis/results"):
        path = _path(self.a, name, boundary)
        raw = path.read_bytes()
        record = {"path": name, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}
        self.read(record, boundary)
        return raw, record

    def finish(self):
        for name, record in self.identities.items():
            raw = _path(self.a, name, ".analysis" if name.startswith(".analysis/") else "tools").read_bytes()
            if len(raw) != record["bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
                raise ValueError("Embedded attribution evidence changed during verification: " + name)


class _Image:
    def __init__(self, raw):
        from host import inspect_pe
        self.raw = raw
        self.metadata = inspect_pe(raw)
        if self.metadata.get("format") not in {"pe32", "pe32+"} or self.metadata.get("unresolved"):
            raise ValueError("Embedded attribution requires complete current PE metadata")
        self.base = _integer(self.metadata["image_base"])
        pe = struct.unpack_from("<I", raw, 0x3c)[0]
        count = struct.unpack_from("<H", raw, pe + 6)[0]
        optional = struct.unpack_from("<H", raw, pe + 20)[0]
        table = pe + 24 + optional
        if count != len(self.metadata["sections"]) or table + count * 40 > len(raw):
            raise ValueError("Embedded attribution PE section table is incomplete")
        self.sections = []
        for index, section in enumerate(self.metadata["sections"]):
            flags = struct.unpack_from("<I", raw, table + index * 40 + 36)[0]
            self.sections.append({**section, "index": index, "characteristics": hex(flags),
                                  "executable": bool(flags & 0x20000000)})
        for start_key, size_key in (("rva", "virtual_bytes"), ("file_offset", "bytes")):
            spans = sorted((s[start_key], s[start_key] + max(s["virtual_bytes"], s["bytes"]) if start_key == "rva"
                            else s[start_key] + s[size_key]) for s in self.sections if s[size_key])
            if any(a[1] > b[0] for a, b in zip(spans, spans[1:])):
                raise ValueError("Embedded attribution PE sections overlap")

    def at(self, rva, size, *, executable=None):
        if type(rva) is not int or type(size) is not int or rva < 0 or size <= 0:
            raise ValueError("Embedded attribution has an invalid machine range")
        sections = [s for s in self.sections if s["rva"] <= rva
                    and rva + size <= s["rva"] + min(s["bytes"], s["virtual_bytes"] or s["bytes"])]
        if len(sections) != 1:
            raise ValueError("Embedded attribution range is not uniquely file-backed in a PE section")
        section = sections[0]
        if executable is not None and section["executable"] != executable:
            raise ValueError("Embedded attribution range has the wrong section execution permission")
        offset = section["file_offset"] + rva - section["rva"]
        value = self.raw[offset:offset + size]
        if len(value) != size:
            raise ValueError("Embedded attribution original machine range is truncated")
        return offset, value, section

    def rva_at(self, offset, size):
        matches = [s for s in self.sections if s["file_offset"] <= offset
                   and offset + size <= s["file_offset"] + s["bytes"]]
        if len(matches) != 1:
            raise ValueError("Embedded attribution file range has an ambiguous parent RVA")
        section = matches[0]
        return section["rva"] + offset - section["file_offset"]


def _capture(evidence, reference, original, image):
    capture = json.loads(evidence.read(reference))
    if (capture.get("input") != original["path"] or capture.get("input_sha256") != original["sha256"]
            or capture.get("input_bytes") != original["bytes"] or _integer(capture.get("image_base")) != image.base
            or capture.get("target_execution") is not False or capture.get("real_device_operations") is not False
            or capture.get("capture_complete") is not True or capture.get("functions_complete") is not True
            or not isinstance(capture.get("functions"), list) or not capture["functions"]
            or capture.get("functions_captured") != len(capture["functions"])
            or capture.get("settled_functions_total") != len(capture["functions"])
            or capture.get("aggregate_functions_written") != len(capture["functions"])
            or not isinstance(capture.get("failures"), list)):
        raise ValueError("Embedded attribution capture does not cover the exact complete original")
    expected = ".analysis/results/native/" + original["sha256"] + "/capture.json"
    if reference["path"] != expected:
        raise ValueError("Embedded attribution capture belongs to another native input")
    functions = {}
    for row in capture["functions"]:
        rva = _integer(row.get("rva"))
        if (rva in functions or _integer(row.get("address")) != image.base + rva
                or row.get("path") != "functions/" + format(rva, "x") + ".json"):
            raise ValueError("Embedded attribution capture has duplicate or misbased function entries")
        functions[rva] = row
    return capture, functions


def _function(evidence, capture_reference, entry, image, *, executable, pseudocode):
    directory = str(Path(capture_reference["path"]).parent).replace("\\", "/")
    path = directory + "/" + entry["path"]
    raw, reference = evidence.current(path, directory)
    if hashlib.sha256(raw).hexdigest() != entry.get("sha256"):
        raise ValueError("Embedded attribution complete function artifact identity differs")
    body = json.loads(raw)
    rva = _integer(entry["rva"])
    if (body.get("address") != entry["address"] or _integer(body.get("rva")) != rva
            or body.get("name") != entry.get("name") or entry.get("assembly_captured") is not True
            or not isinstance(body.get("chunks"), list) or not body["chunks"]
            or not isinstance(body.get("assembly"), list) or not body["assembly"]):
        raise ValueError("Embedded attribution complete function boundary or assembly is absent")
    chunks, intervals = [], []
    for row in body["chunks"]:
        start, end = _integer(row["start"]), _integer(row["end"])
        offset, actual, section = image.at(start - image.base, end - start, executable=executable)
        if (row.get("raw_hex") != actual.hex() or row.get("sha256") != hashlib.sha256(actual).hexdigest()
                or any(start < old_end and old_start < end for old_start, old_end in intervals)):
            raise ValueError("Embedded attribution function chunks overlap or differ from current original bytes")
        intervals.append((start, end))
        chunks.append({"start": start, "end": end, "rva": start - image.base, "bytes": end - start,
                       "file_offset": offset, "sha256": row["sha256"], "section": section})
    if not any(start == image.base + rva for start, _ in intervals):
        raise ValueError("Embedded attribution function entry is not a complete chunk boundary")
    if _integer(body.get("end")) != next(end for start, end in intervals if start == image.base + rva):
        raise ValueError("Embedded attribution function end differs from the complete entry chunk")
    instructions, ranges = [], []
    for row in body["assembly"]:
        at = _integer(row["address"])
        if _integer(row.get("rva")) != at - image.base or not isinstance(row.get("text"), str) or not row["text"].strip():
            raise ValueError("Embedded attribution instruction has an invalid original coordinate or display")
        try:
            code = bytes.fromhex(row["bytes"])
        except (ValueError, TypeError, KeyError) as error:
            raise ValueError("Embedded attribution instruction bytes are invalid") from error
        offset, actual, _ = image.at(at - image.base, len(code), executable=executable)
        if actual != code or not any(start <= at < at + len(code) <= end for start, end in intervals):
            raise ValueError("Embedded attribution instruction differs from complete original function bytes")
        instructions.append({"address": at, "rva": at - image.base, "file_offset": offset,
                             "bytes": len(code), "raw_hex": code.hex(), "text": row["text"]})
        ranges.append((at, at + len(code)))
    ranges.sort()
    if any(a[1] > b[0] for a, b in zip(ranges, ranges[1:])):
        raise ValueError("Embedded attribution instruction coverage overlaps")
    for start, end in intervals:
        cursor = start
        for begin, finish in ranges:
            if begin < start or begin >= end:
                continue
            if begin != cursor:
                raise ValueError("Embedded attribution complete instruction coverage has a gap")
            cursor = finish
        if cursor != end:
            raise ValueError("Embedded attribution complete instruction coverage has a gap")
    if pseudocode:
        pseudo = body.get("pseudocode")
        if (entry.get("pseudocode_captured") is not True or entry.get("new_failures") != 0
                or not isinstance(pseudo, dict) or _integer(pseudo.get("entry")) != image.base + rva
                or not isinstance(pseudo.get("lines"), list) or len(pseudo["lines"]) < 3
                or not isinstance(pseudo.get("locals"), list)):
            raise ValueError("Embedded child selected function has no complete pseudocode")
        lines = pseudo["lines"]
        if (any(not isinstance(line, dict) or type(line.get("line")) is not int
                or line["line"] != index or not isinstance(line.get("text"), str)
                for index, line in enumerate(lines)) or lines[-1]["text"].strip() != "}"):
            raise ValueError("Embedded child pseudocode is partial or has missing original lines")
        declaration = next((index for index, line in enumerate(lines)
                            if not line["text"].lstrip().startswith("//")), len(lines))
        if (declaration >= len(lines) - 2 or not lines[declaration]["text"].strip()
                or lines[declaration + 1]["text"].strip() != "{"):
            raise ValueError("Embedded child pseudocode has no complete original declaration and body")
    return {"reference": reference, "body": body, "chunks": sorted(chunks, key=lambda row: row["file_offset"]),
            "instructions": sorted(instructions, key=lambda row: row["file_offset"])}


def _pointer(value, pointer):
    if not isinstance(pointer, str) or not pointer.startswith("/"):
        raise ValueError("Embedded attribution evidence pointer is not explicit")
    try:
        for token in pointer[1:].split("/"):
            if re.search(r"~(?![01])", token):
                raise ValueError("Invalid JSON pointer escape")
            token = token.replace("~1", "/").replace("~0", "~")
            if isinstance(value, list):
                if re.fullmatch(r"0|[1-9][0-9]*", token) is None:
                    raise ValueError("Invalid JSON pointer list index")
                value = value[int(token)]
            else:
                value = value[token]
    except (KeyError, TypeError, IndexError) as error:
        raise ValueError("Embedded attribution evidence pointer does not resolve") from error
    return value


def verify_function_attribution(a, review):
    """Strictly verify a reviewed parent-to-child mapping; preserve both failures."""
    from host import inspect_embedded_pe_ranges
    if (not isinstance(review, dict) or set(review) != _SCHEMA or review.get("schema_version") != 1
            or review.get("review_kind") != "embedded_pe_function_attribution"
            or review.get("target_execution") is not False or review.get("raw_artifacts_modified") is not False
            or not isinstance(review.get("mappings"), list) or not review["mappings"]):
        raise ValueError("Embedded attribution review differs from the exact static schema")
    evidence = _Evidence(a)
    parent_raw = evidence.read(review["source_input"], ".analysis/inputs")
    child_raw = evidence.read(review["child_input"], ".analysis/inputs")
    parent, child = _Image(parent_raw), _Image(child_raw)
    parent_capture, parent_functions = _capture(evidence, review["base_capture"], review["source_input"], parent)
    child_capture, child_functions = _capture(evidence, review["child_capture"], review["child_input"], child)
    index_raw, index_identity = evidence.current(".analysis/inputs.json", ".analysis")
    index = json.loads(index_raw)
    indexed_child = [row for row in index["inputs"] if row.get("path") == review["child_input"]["path"]]
    indexed_parent = [row for row in index["inputs"] if row.get("path") == review["source_input"]["path"]]
    if len(indexed_child) != 1 or len(indexed_parent) != 1:
        raise ValueError("Embedded attribution parent or child has no unique current input index entry")
    for actual, expected in ((indexed_child[0], review["child_input"]), (indexed_parent[0], review["source_input"])):
        if any(actual.get(key) != expected[key] for key in _IDENTITY):
            raise ValueError("Embedded attribution current input index identity differs")
        if actual.get("container_extraction_complete") is False or actual.get("source_bytes_verified") is False:
            raise ValueError("Embedded attribution source belongs to an incomplete extraction")
    extracted = json.loads(evidence.read(review["extraction_receipt"]))
    rows = [row for row in extracted["inputs"] if row.get("path") == review["child_input"]["path"]]
    if extracted.get("target_execution") is not False or len(rows) != 1 or rows[0] != indexed_child[0]:
        raise ValueError("Embedded attribution extraction receipt differs from the current indexed child")
    row = rows[0]
    extraction = row.get("extraction", {})
    offset = row.get("container_offset")
    if (row.get("kind") != "static_embedded_native_input" or row.get("byte_range_verified") is not True
            or row.get("container") != review["source_input"]["path"]
            or row.get("container_sha256") != review["source_input"]["sha256"]
            or type(offset) is not int or offset <= 0 or offset + len(child_raw) > len(parent_raw)
            or parent_raw[offset:offset + len(child_raw)] != child_raw
            or extraction.get("kind") != "reviewed_embedded_pe_range"):
        raise ValueError("Embedded attribution child is not the exact reviewed complete parent byte range")
    aliases = [item for item in row.get("container_aliases", [])
               if all(item.get(key) == review["source_input"][key] for key in _IDENTITY)
               and item.get("byte_identity_verified") is True]
    if len(aliases) != 1:
        raise ValueError("Embedded attribution parent has no exact verified container alias")
    group = _pointer(json.loads(evidence.read(extraction["review_source"])), extraction["review_pointer"])
    if group.get("schema_version") != 1 or group.get("source_input") != review["source_input"]:
        raise ValueError("Embedded attribution original range review belongs to another parent")
    ranges = inspect_embedded_pe_ranges(parent_raw, group["ranges"])
    matches = [item for item in ranges if item["name"] == Path(review["child_input"]["path"]).name
               and item["offset"] == offset and item["bytes"] == len(child_raw)
               and item["sha256"] == review["child_input"]["sha256"]]
    if len(matches) != 1:
        raise ValueError("Embedded attribution complete child extent has no unique original review")
    verified_range = matches[0]
    if any(verified_range[key] != extraction[key] for key in ("pe", "layout", "review")):
        raise ValueError("Embedded attribution extraction metadata differs from current full PE parsing")
    owning_evidence = []
    for reference in verified_range["review"]["evidence"]:
        body = json.loads(evidence.read(reference))
        rva = _integer(body.get("rva"))
        entry = parent_functions.get(rva)
        if entry is None:
            raise ValueError("Embedded extraction boundary evidence is absent from the parent capture")
        function = _function(evidence, review["base_capture"], entry, parent, executable=True, pseudocode=True)
        if function["reference"] != reference:
            raise ValueError("Embedded extraction evidence is not the complete current parent callable")
        owning_evidence.append(reference)
    child_directory = str(Path(review["child_capture"]["path"]).parent).replace("\\", "/")
    required_outputs = {"all-functions.asm", "all-functions.c", "entrypoints.json", "imports.json",
                        "local_types.json", "names.json", "segments.json", "strings.json", "structures.json"}
    outputs_seen = set()
    for output in child_capture.get("outputs", []):
        if set(output) != {"path", "bytes", "sha256"}:
            raise ValueError("Embedded child full capture has an incomplete output identity")
        if output["path"] in outputs_seen:
            raise ValueError("Embedded child full capture has duplicate output identities")
        outputs_seen.add(output["path"])
        evidence.read({**output, "path": child_directory + "/" + output["path"]}, child_directory)
    if not required_outputs.issubset(outputs_seen):
        raise ValueError("Embedded child full capture is missing complete retained output inventories")
    for entry in child_capture["functions"]:
        raw, _ = evidence.current(child_directory + "/" + entry["path"], child_directory)
        if hashlib.sha256(raw).hexdigest() != entry["sha256"]:
            raise ValueError("Embedded child full capture function inventory changed")
    mappings, resolved, parent_seen, child_seen, parent_spans = [], [], set(), set(), []
    for mapping in review["mappings"]:
        if (not isinstance(mapping, dict) or set(mapping) != {"parent_rva", "child_rva", "reason"}
                or not isinstance(mapping.get("reason"), str) or not mapping["reason"].strip()):
            raise ValueError("Embedded attribution mapping needs exact function entries and a reviewed reason")
        parent_rva, child_rva = _integer(mapping["parent_rva"]), _integer(mapping["child_rva"])
        if parent_rva in parent_seen or child_rva in child_seen or parent_rva not in parent_functions or child_rva not in child_functions:
            raise ValueError("Embedded attribution mapped function is duplicated or absent")
        parent_seen.add(parent_rva)
        child_seen.add(child_rva)
        selected_failures = [failure for failure in parent_capture["failures"]
                             if failure.get("stage", "").split(":", 1)[0] == format(parent_rva, "x")]
        if not selected_failures or parent_functions[parent_rva].get("pseudocode_captured") is not False:
            raise ValueError("Embedded attribution parent entry is not an original retained failure")
        if any(failure.get("stage", "").split(":", 1)[0] == format(child_rva, "x") for failure in child_capture["failures"]):
            raise ValueError("Embedded attribution selected child function still has a capture failure")
        before = _function(evidence, review["base_capture"], parent_functions[parent_rva], parent, executable=False, pseudocode=False)
        after = _function(evidence, review["child_capture"], child_functions[child_rva], child, executable=True, pseudocode=True)
        if len(before["chunks"]) != len(after["chunks"]):
            raise ValueError("Embedded attribution parent and child complete chunks differ")
        chunks = []
        for old, new in zip(before["chunks"], after["chunks"]):
            if (old["file_offset"] != offset + new["file_offset"] or old["bytes"] != new["bytes"]
                    or old["sha256"] != new["sha256"]
                    or any(old["file_offset"] < end and start < old["file_offset"] + old["bytes"] for start, end in parent_spans)):
                raise ValueError("Embedded attribution complete parent and child byte mapping differs or overlaps")
            parent_spans.append((old["file_offset"], old["file_offset"] + old["bytes"]))
            chunks.append({"parent_rva": hex(old["rva"]), "child_rva": hex(new["rva"]),
                "parent_file_offset": old["file_offset"], "child_file_offset": new["file_offset"],
                "bytes": old["bytes"], "sha256": old["sha256"],
                "parent_section": old["section"], "child_section": new["section"]})
        entry_parent_offset = parent.at(parent_rva, 1, executable=False)[0]
        entry_child_offset = child.at(child_rva, 1, executable=True)[0]
        if entry_parent_offset != offset + entry_child_offset:
            raise ValueError("Embedded attribution function entries map to different original bytes")
        instruction_map = []
        for instruction in after["instructions"]:
            start = offset + instruction["file_offset"]
            end = start + instruction["bytes"]
            overlaps = [row for row in before["instructions"] if row["file_offset"] < end
                        and start < row["file_offset"] + row["bytes"]]
            if not overlaps or sum(min(end, row["file_offset"] + row["bytes"]) - max(start, row["file_offset"]) for row in overlaps) != instruction["bytes"]:
                raise ValueError("Embedded attribution child instruction has no complete original parent coverage")
            instruction_map.append({"child_rva": hex(instruction["rva"]), "parent_rva": hex(parent.rva_at(start, instruction["bytes"])),
                "parent_file_offset": start, "child_file_offset": instruction["file_offset"],
                "bytes": instruction["bytes"], "raw_hex": instruction["raw_hex"], "child_text": instruction["text"],
                "parent_interpretation": [{"rva": hex(row["rva"]), "bytes": row["bytes"], "raw_hex": row["raw_hex"], "text": row["text"]} for row in overlaps]})
        entry_refs = []
        for xref in before["body"].get("xrefs_to_entry", []):
            at = _integer(xref["from"]) - parent.base
            source_offset, _, section = parent.at(at, 1)
            if xref.get("code") is True and section["executable"]:
                raise ValueError("Embedded attribution has an executable-parent code reference to the failed data entry")
            entry_refs.append({"historical_reference": xref, "source_section": section,
                               "source_parent_file_offset": source_offset,
                               "source_inside_embedded_child": offset <= source_offset < offset + len(child_raw)})
        pseudo = after["body"]["pseudocode"]
        mappings.append({"parent_rva": hex(parent_rva), "child_rva": hex(child_rva), "reason": mapping["reason"],
                         "parent_function": before["reference"], "child_function": after["reference"],
                         "complete_chunks": chunks, "instruction_mapping": instruction_map,
                         "parent_instruction_count": len(before["instructions"]), "child_instruction_count": len(after["instructions"]),
                         "parent_entry_references": entry_refs,
                         "complete_child_pseudocode": {"artifact": after["reference"], "lines": len(pseudo["lines"]),
                             "entry": pseudo["entry"], "sha256": hashlib.sha256(json.dumps(pseudo, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()},
                         "original_parent_failures": selected_failures,
                         "parent_decompilation_recovered": False, "child_typed_coordinates_verified": False})
        resolved.extend(selected_failures)
    evidence.finish()
    return {"schema_version": 1, "review_kind": "verified_embedded_pe_function_attribution", "status": "verified",
            "source_input": review["source_input"], "base_capture": review["base_capture"],
            "child_input": review["child_input"], "child_capture": review["child_capture"],
            "extraction_receipt": review["extraction_receipt"], "input_index": index_identity,
            "input_index_registration": {"source_input": indexed_parent[0], "child_input": indexed_child[0]},
            "range_review": extraction["review_source"], "range_review_pointer": extraction["review_pointer"],
            "range_owner_evidence": owning_evidence, "embedded_file_offset": offset, "embedded_file_bytes": len(child_raw),
            "parent_machine": parent.metadata["machine"], "child_machine": child.metadata["machine"],
            "architecture_mismatch": parent.metadata["machine"] != child.metadata["machine"],
            "resolved_parent_failures": resolved, "historical_parent_failures": parent_capture["failures"],
            "child_remaining_failures": child_capture["failures"], "child_failure_task_is_independent": True,
            "mappings": mappings, "verified_artifacts": list(evidence.identities.values()),
            "target_execution": False, "raw_artifacts_modified": False, "parent_decompilation_recovered": False,
            "semantics_complete": False, "runtime_acceptance": "unverified",
            "cleanup": {"temporary_paths_created": [], "deleted_paths": [], "released_bytes": 0}}


def _index_refresh_scope(previous, current):
    """Allow index refreshes only after a complete, otherwise identical current proof."""
    history = {"previous_audit_snapshot", "index_refresh"}
    before = {key: value for key, value in previous.items() if key not in history}
    after = {key: value for key, value in current.items() if key not in history}
    for proof in (before, after):
        index = proof.get("input_index", {})
        if (set(index) != _IDENTITY or index.get("path") != ".analysis/inputs.json"
                or type(index.get("bytes")) is not int or index["bytes"] <= 0
                or not isinstance(index.get("sha256"), str) or re.fullmatch(r"[a-f0-9]{64}", index["sha256"]) is None
                or not isinstance(proof.get("verified_artifacts"), list)
                or [row for row in proof["verified_artifacts"] if row.get("path") == ".analysis/inputs.json"] != [index]):
            raise ValueError("Embedded index refresh omitted an exact unique index identity")
    registration = after.get("input_index_registration")
    if not isinstance(registration, dict) or set(registration) != {"source_input", "child_input"}:
        raise ValueError("Embedded index refresh omitted complete current parent and child registrations")
    for key, row in registration.items():
        if (not isinstance(row, dict) or any(row.get(field) != after[key][field] for field in _IDENTITY)):
            raise ValueError("Embedded index refresh changed a registered original identity")
    previous_rows_available = "input_index_registration" in before
    if previous_rows_available and before["input_index_registration"] != registration:
        raise ValueError("Embedded index refresh changed a previously retained complete input row")
    before["input_index_registration"] = registration
    before["input_index"] = after["input_index"]
    before["verified_artifacts"] = [after["input_index"] if row.get("path") == ".analysis/inputs.json" else row
                                    for row in before["verified_artifacts"]]
    if before != after:
        raise ValueError("Embedded index refresh changed function evidence, failures, mappings or another artifact")
    return {"operation": "fresh_complete_current_input_index_revalidation",
            "historical_full_index_replay": False,
            "previous_complete_input_rows_retained": previous_rows_available,
            "current_complete_input_rows_verified": True,
            "unrecorded_historical_parent_row_metadata_equality": "unverified"}


def _audit_history(evidence, proof):
    """Validate every immutable predecessor without treating old index hashes as live files."""
    current, seen = proof, set()
    while "previous_audit_snapshot" in current:
        reference = current["previous_audit_snapshot"]
        if (not isinstance(reference, dict) or set(reference) != _IDENTITY
                or reference.get("sha256") in seen or reference.get("path") !=
                ".analysis/results/native-packages/review-sources/" + str(reference.get("sha256")) + ".json"):
            raise ValueError("Embedded index refresh history omitted a unique content-addressed predecessor")
        seen.add(reference["sha256"])
        previous = json.loads(evidence.read(reference))
        if current.get("index_refresh") != _index_refresh_scope(previous, current):
            raise ValueError("Embedded index refresh history changed its exact revalidation boundary")
        current = previous
    if "index_refresh" in current:
        raise ValueError("Embedded index refresh history has no exact predecessor")


def _verified_formal_audit(a, destination, evidence):
    raw, reference = evidence.current(a.rel(destination))
    proof = json.loads(raw)
    source = proof["review_source"]
    if not isinstance(source, dict) or set(source) != _IDENTITY | {"pointer"}:
        raise ValueError("Embedded attribution has no exact retained original review identity and pointer")
    source_identity = {key: source[key] for key in _IDENTITY}
    retained_path = ".analysis/results/native-packages/review-sources/" + source["sha256"] + ".json"
    if source["path"] != retained_path:
        raise ValueError("Embedded attribution original review is not content-addressed")
    review = json.loads(evidence.read(source_identity))
    if source["pointer"] is not None:
        review = _pointer(review, source["pointer"])
    current = verify_function_attribution(a, review)
    current["review_source"] = dict(source)
    if current != {key: value for key, value in proof.items()
                   if key not in {"previous_audit_snapshot", "index_refresh"}}:
        raise ValueError("Embedded attribution formal audit differs from freshly verified complete evidence")
    _audit_history(evidence, proof)
    return proof, reference, source


def retained_attribution_reference_valid(a, record, source_sha256):
    """Accept an exact old audit only through its freshly verified owner's predecessor chain."""
    try:
        if (not isinstance(source_sha256, str) or re.fullmatch(r"[a-f0-9]{64}", source_sha256) is None
                or not isinstance(record, dict) or set(record) != _IDENTITY
                or record.get("path") != ".analysis/results/native/" + source_sha256 + "/embedded-function-attribution.json"
                or type(record.get("bytes")) is not int or record["bytes"] <= 0
                or not isinstance(record.get("sha256"), str) or re.fullmatch(r"[a-f0-9]{64}", record["sha256"]) is None):
            return False
        evidence = _Evidence(a)
        destination = _path(a, record["path"], ".analysis/results/native")
        proof, current, _ = _verified_formal_audit(a, destination, evidence)
        if proof["source_input"]["sha256"] != source_sha256:
            return False
        found = current == record
        while "previous_audit_snapshot" in proof:
            reference = proof["previous_audit_snapshot"]
            if reference["sha256"] == record["sha256"] and reference["bytes"] == record["bytes"]:
                found = True
            proof = json.loads(evidence.read(reference))
        evidence.finish()
        return found
    except (OSError, ValueError, KeyError, TypeError):
        return False


def audit_function_attribution(args):
    """Maintained CLI; review is an exact JSON document or an explicit pointer."""
    import analysis as a
    name, separator, pointer = args.review.partition("#")
    path = _path(a, name, ".analysis/results")
    raw = path.read_bytes()
    review = json.loads(raw)
    if separator:
        review = _pointer(review, pointer)
    output = getattr(args, "output", None) or (
        ".analysis/results/native/" + review["source_input"]["sha256"] + "/embedded-function-attribution.json")
    destination = _path(a, output, ".analysis/results")
    expected_previous = getattr(args, "previous_audit_sha256", None)
    if expected_previous is not None and re.fullmatch(r"[a-f0-9]{64}", expected_previous) is None:
        raise ValueError("Embedded index refresh requires the exact previous audit SHA-256")
    with a.index_lock(a.ROOT / ".analysis/reverse-pending.lock"):
        result = verify_function_attribution(a, review)
        retained = a.retain_embedded_review_source(raw)
        result["review_source"] = {**retained, "pointer": pointer if separator else None}
        previous_raw = destination.read_bytes() if destination.exists() else None
        if previous_raw is None:
            if expected_previous is not None:
                raise ValueError("Embedded index refresh has no previous audit")
        else:
            previous = json.loads(previous_raw)
            historical = _Evidence(a)
            _audit_history(historical, previous)
            historical.finish()
            comparable = {key: value for key, value in previous.items()
                          if key not in {"previous_audit_snapshot", "index_refresh"}}
            if comparable == result:
                if expected_previous is not None and hashlib.sha256(previous_raw).hexdigest() != expected_previous:
                    raise ValueError("Embedded index refresh previous audit bytes changed")
                result = previous
            else:
                if expected_previous is None:
                    raise ValueError("Embedded attribution output already contains another immutable result")
                if hashlib.sha256(previous_raw).hexdigest() != expected_previous:
                    raise ValueError("Embedded index refresh previous audit bytes changed")
                refresh = _index_refresh_scope(previous, result)
                result["previous_audit_snapshot"] = a.retain_embedded_review_source(previous_raw)
                result["index_refresh"] = refresh
        serialized = (json.dumps(result, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
        if previous_raw != serialized:
            destination.parent.mkdir(parents=True, exist_ok=True)
            temporary = None
            try:
                with tempfile.NamedTemporaryFile(prefix=".embedded-attribution-", suffix=".tmp",
                                                 dir=destination.parent, delete=False) as stream:
                    temporary = Path(stream.name)
                    stream.write(serialized)
                    stream.flush()
                    os.fsync(stream.fileno())
                publication = _Evidence(a)
                for artifact in result["verified_artifacts"]:
                    publication.read(artifact, ".analysis")
                publication.read({key: result["review_source"][key] for key in _IDENTITY})
                publication.finish()
                current_index = a.ROOT / result["input_index"]["path"]
                if (current_index.stat().st_size != result["input_index"]["bytes"]
                        or a.sha(current_index) != result["input_index"]["sha256"]
                        or destination.exists() and destination.read_bytes() != previous_raw):
                    raise ValueError("Embedded index or previous audit changed before publication")
                _path(a, output, ".analysis/results")
                if previous_raw is None:
                    os.link(temporary, destination)
                else:
                    os.replace(temporary, destination)
                if destination.read_bytes() != serialized:
                    raise ValueError("Embedded index refresh publication bytes differ")
            finally:
                if temporary is not None:
                    temporary.unlink(missing_ok=True)
    print(json.dumps({"receipt": a.rel(destination), "status": result["status"],
                      "mapped_functions": len(result["mappings"]),
                      "resolved_parent_failures": len(result["resolved_parent_failures"]),
                      "child_remaining_failures": len(result["child_remaining_failures"])}), flush=True)
    return result


def apply_resolution(a, result, resolution, capture):
    """Reverify a formal byte-attribution proof without recovering parent types."""
    destination = result / "embedded-function-attribution.json"
    if not destination.is_file():
        return
    try:
        evidence = _Evidence(a)
        proof, reference, source = _verified_formal_audit(a, destination, evidence)
        base = json.loads(evidence.read(proof["base_capture"]))
        expected_folder = ".analysis/results/native/" + proof["source_input"]["sha256"]
        if (a.rel(result) != expected_folder or proof["base_capture"] != resolution["base_capture"]
                or base != capture):
            raise ValueError("Embedded attribution resolution belongs to another source or base capture")
        remaining = list(resolution["remaining_failures"])
        resolved = []
        for index, mapping in enumerate(proof["mappings"]):
            for failure in mapping["original_parent_failures"]:
                if remaining.count(failure) != 1:
                    raise ValueError("Embedded attribution original failure is absent or duplicated in the pending resolution")
                remaining.remove(failure)
                resolved.append({"original_failure": failure, "function_rva": mapping["parent_rva"],
                    "resolution_kind": "corrected_embedded_pe_function_attribution",
                    "proof": {"receipt": reference, "mapping_index": index}})
        if [item["original_failure"] for item in resolved] != proof["resolved_parent_failures"]:
            raise ValueError("Embedded attribution resolved failures differ from complete mapped original failures")
        evidence.finish()
    except (OSError, ValueError, KeyError, TypeError) as error:
        resolution["verification_failures"].append({"attribution": a.rel(destination), "error": str(error)})
        return
    resolution["remaining_failures"] = remaining
    resolution["resolved_failures"].extend(resolved)
    resolution["attribution_resolution"] = {
        "proof": reference, "review_source": source, "source_input": proof["source_input"],
        "child_input": proof["child_input"], "child_capture": proof["child_capture"],
        "mappings": [{key: mapping[key] for key in ("parent_rva", "child_rva")} for mapping in proof["mappings"]],
        "historical_parent_failures": proof["historical_parent_failures"],
        "child_remaining_failures": proof["child_remaining_failures"], "child_failure_task_is_independent": True,
        "parent_decompilation_recovered": False, "semantics_complete": False, "runtime_acceptance": "unverified"}
