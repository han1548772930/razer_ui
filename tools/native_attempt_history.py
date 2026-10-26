"""Byte-bound proofs for superseded partial captures and explicit x86 ABI aliases."""
from __future__ import annotations

import copy
from dataclasses import asdict
from datetime import datetime
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re

import native_adjudication as evidence
import native_import_forwarders as forwarders


_SOURCE = Path(__file__).read_bytes()
_ALIAS_TYPES = ("ULONG(PCSTR Format, ...)", "unsigned __int32(const char *Format, ...)")
_DOCS = {
    "dbgprint": ("https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-dbgprint",
                 "ULONG DbgPrint( PCSTR Format, ... );"),
    "cdecl": ("https://learn.microsoft.com/en-us/cpp/cpp/cdecl?view=msvc-170",
              "Calling function pops the arguments from the stack."),
    "argument_passing": ("https://learn.microsoft.com/en-us/cpp/cpp/argument-passing-and-naming-conventions?view=msvc-170",
                         "Return values are also widened to 32 bits and returned in the EAX register"),
    "type_ranges": ("https://learn.microsoft.com/en-us/cpp/cpp/data-type-ranges?view=msvc-170",
                    "unsigned __int32"),
}


def source_identity(root):
    path = Path(__file__).resolve()
    if path.read_bytes() != _SOURCE:
        raise ValueError("Native attempt-history validator changed after import")
    return {"path": path.relative_to(root).as_posix(), "bytes": len(_SOURCE),
            "sha256": hashlib.sha256(_SOURCE).hexdigest()}


def alias_compiler_source():
    return '''/* SDK ntdef.h exact declarations; SAL annotation has no ABI effect. */
#define _Null_terminated_
#define CONST               const
typedef char CHAR;
typedef _Null_terminated_ CONST CHAR *LPCSTR, *PCSTR;
typedef unsigned long ULONG;
_Static_assert(sizeof(ULONG) == 4, "ULONG is exactly 32 bits");
_Static_assert(sizeof(PCSTR) == 4, "PCSTR is an x86 pointer");
_Static_assert(sizeof(unsigned __int32) == 4, "explicit result is exactly 32 bits");
_Static_assert((ULONG)-1 > 0, "ULONG is unsigned");
_Static_assert((unsigned __int32)-1 > 0, "explicit result is unsigned");
_Static_assert(__builtin_types_compatible_p(PCSTR, const char *), "PCSTR is const char pointer");
typedef ULONG (__cdecl *original_signature)(PCSTR Format, ...);
typedef unsigned __int32 (__cdecl *previous_signature)(const char *Format, ...);
ULONG __cdecl call_original(original_signature function, PCSTR format, unsigned int value) {
    return function(format, value);
}
unsigned __int32 __cdecl call_previous(previous_signature function, const char *format, unsigned int value) {
    return function(format, value);
}
'''


class _Text(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts = []

    def handle_data(self, data):
        self.parts.append(data)


def _text(raw):
    parser = _Text()
    parser.feed(raw.decode("utf-8-sig"))
    return re.sub(r"\s+", " ", " ".join(parser.parts))


def _abi_sources(a, acquisition_identity, compiler_identity):
    acquisition = json.loads(evidence.checked(a, acquisition_identity))
    if (acquisition.get("schema_version") != 1
            or acquisition.get("operation") != "retain_windows_sdk_and_official_x86_abi_evidence"
            or acquisition.get("target_execution") is not False):
        raise ValueError("Alias ABI evidence lacks the retained SDK and official-document acquisition")
    header = acquisition.get("header", {})
    header_id = {key: header.get(key) for key in ("path", "bytes", "sha256")}
    sdk = evidence.checked(a, header_id, ".analysis/inputs").decode("utf-8-sig")
    receipt = header.get("acquisition_receipt", {})
    if (header.get("kind") != "installed_os_sdk_header" or header.get("target_execution") is not False
            or receipt.get("sdk_version") != "10.0.26100.0"
            or receipt.get("source_locator") != "Windows Kits/10/Include/10.0.26100.0/shared/ntdef.h"
            or receipt.get("installed_source_bytes_verified_before_retention") is not True
            or receipt.get("retained_original_header") is not True):
        raise ValueError("Alias SDK source identity or acquisition scope changed")
    declarations = ["typedef char CHAR;", "#define CONST               const",
                    "typedef _Null_terminated_ CONST CHAR *LPCSTR, *PCSTR;", "typedef unsigned long ULONG;"]
    for declaration in declarations:
        if declaration not in sdk:
            raise ValueError("Retained Windows SDK does not contain the exact reviewed alias declaration")
    documents = acquisition.get("documents", [])
    if (len(documents) != len(_DOCS) or {row.get("role") for row in documents} != set(_DOCS)):
        raise ValueError("Alias ABI review omitted a complete official signature or calling-convention source")
    verified_docs = []
    for document in documents:
        url, excerpt = _DOCS[document["role"]]
        identity = {key: document.get(key) for key in ("path", "bytes", "sha256")}
        text = _text(evidence.checked(a, identity, ".analysis/inputs"))
        if (document.get("url") != url or document.get("final_url") != url
                or document.get("http_status") != 200 or document.get("target_execution") is not False
                or document.get("kind") != "official_windows_abi_document" or excerpt not in text):
            raise ValueError("Retained Windows ABI document changed its source, exact declaration or scope")
        verified_docs.append({"role": document["role"], "document": identity, "url": url, "verified_excerpt": excerpt})
    compiler = json.loads(evidence.checked(a, compiler_identity))
    argv = ["<clang-static-compiler>", "-target", "i686-pc-windows-msvc", "-S", "-emit-llvm", "-O0", "-fno-ident",
            "-fdebug-compilation-dir=.", "-x", "c", "-o", "-", "-"]
    if (compiler.get("schema_version") != 1 or compiler.get("operation") != "static_i686_windows_variadic_alias_abi_compile"
            or compiler.get("argv") != argv or compiler.get("exit_code") != 0
            or compiler.get("target_execution") is not False or compiler.get("temporary_files_created") is not False
            or compiler.get("stderr") is not None or not compiler.get("compiler_version", "").startswith("clang version ")
            or compiler.get("compiler", {}).get("tool_locator") != "LLVM/bin/clang.exe"
            or type(compiler.get("compiler", {}).get("bytes")) is not int or compiler["compiler"]["bytes"] <= 0
            or not re.fullmatch(r"[0-9a-f]{64}", compiler.get("compiler", {}).get("sha256", ""))):
        raise ValueError("Alias ABI compile omitted its exact successful static i686 compiler receipt")
    if evidence.checked(a, compiler["stdin"]).decode() != alias_compiler_source():
        raise ValueError("Alias ABI compiler input differs from the retained exact SDK declarations and assertions")
    ir = evidence.checked(a, compiler["stdout"]).decode()
    definitions = re.findall(r"define dso_local i32 @(call_original|call_previous)\(ptr noundef %0, ptr noundef %1, i32 noundef %2\) #0 \{(.*?)\n\}", ir, re.S)
    if (not re.search(r'target triple = "i686-pc-windows-msvc[0-9.]*"', ir)
            or 'target datalayout = "e-m:x-p:32:32-' not in ir
            or [row[0] for row in definitions] != ["call_original", "call_previous"]
            or definitions[0][1] != definitions[1][1]
            or definitions[0][1].count("call i32 (ptr, ...)") != 1
            or "ret i32 %10" not in definitions[0][1]
            or any(value in ir for value in ("x86_stdcallcc", "x86_fastcallcc", "signext", "zeroext"))):
        raise ValueError("Static compiler did not retain identical full cdecl i32/const-pointer variadic ABI lowering")
    return {"sdk_header": header_id, "sdk_declarations": declarations,
            "official_documents": verified_docs, "static_compiler_receipt": compiler_identity,
            "sdk_installer_acquisition_chain_retained": header.get("acquisition_chain_retained") is True,
            "return_bits": 32, "return_signed": False, "return_register": "eax",
            "fixed_parameter": {"name": "Format", "type": "const char *", "pointer_bits": 32,
                                "pointee_const": True, "stack_offset_from_argument_base": 0},
            "calling_convention": "cdecl", "stack_cleanup": "caller", "variadic": True,
            "c_language_type_identity_claimed": False, "abi_equivalence_only": True}


def _current_forwarder(a, result, proof):
    current = forwarders.prove_forwarder(a, result, proof["rva"], proof["machine_evidence"], proof["abi_evidence"])
    if (any(proof.get(key) != value for key, value in current.items())
            or set(proof) - set(current) - {"review", "binding"}):
        raise ValueError("Alias forwarder differs from its complete independently recomputed current proof")
    return current


def _caller_cleanup(a, current):
    specifications = [("0x8593a", ["59"], 4), ("0x88c5f", ["59", "59"], 8),
                      ("0x89cc2", ["83c410"], 16)]
    result = []
    for rva, cleanup_bytes, adjustment in specifications:
        calls = [row for row in current["incoming_calls"] if row["call_rva"] == rva]
        if len(calls) != 1:
            raise ValueError("Explicit DbgPrint alias review lacks its independent original CALL cleanup witness")
        call = calls[0]
        body = json.loads(evidence.checked(a, call["owner_function"]))
        index = next(i for i, row in enumerate(body["assembly"]) if row["rva"] == rva)
        cleanup = body["assembly"][index + 1:index + 1 + len(cleanup_bytes)]
        at = int(body["assembly"][index]["address"], 16) + 5
        for row, expected in zip(cleanup, cleanup_bytes):
            if row.get("bytes") != expected or int(row["address"], 16) != at:
                raise ValueError("DbgPrint original caller cleanup bytes or contiguous boundaries differ")
            at += len(bytes.fromhex(expected))
        if len(cleanup) != len(cleanup_bytes):
            raise ValueError("DbgPrint original caller cleanup is truncated")
        result.append({"original_call": call, "cleanup_instructions": cleanup, "caller_releases_argument_bytes": adjustment})
    return result


def build_alias_review(a, result, proof, old_identity, acquisition_identity, compiler_identity):
    """Review only this original DbgPrint alias artifact; do not publish a binding."""
    current = _current_forwarder(a, result, proof)
    previous = json.loads(evidence.checked(a, old_identity, a.rel(result)))
    original = json.loads(evidence.checked(a, current["function"], a.rel(result)))
    raw = evidence.checked(a, current["source_input"], ".analysis/inputs")
    model, imported = forwarders.decode_import_jump(forwarders.Pe32(raw), previous)
    if (original.get("type") != _ALIAS_TYPES[0] or previous.get("type") != _ALIAS_TYPES[1]
            or imported != current["original_import"] or imported.get("name") != "DbgPrint"
            or asdict(model) != {key: current["machine"][key] for key in ("entry_virtual_address", "iat_virtual_address")}
            or set(previous) - set(original) - {"boundary_source"}
            or {key: value for key, value in previous.items() if key not in {"type", "boundary_source"}}
               != {key: value for key, value in original.items() if key != "type"}):
        raise ValueError("Explicit alias coverage cannot replace different original machine, callers, type or metadata")
    manifest_path = a.resolve(old_identity["path"], a.rel(result)).parent.parent / "capture.json"
    manifest = json.loads(manifest_path.read_bytes())
    plan_path = manifest_path.parent / "plan.json"
    plan = json.loads(plan_path.read_bytes())
    selection = manifest.get("function_selection", {})
    requests = selection.get("import_type_recoveries", [])
    applied = manifest.get("import_type_recoveries", [])
    thunks = manifest.get("import_thunk_type_recoveries", [])
    expected_declaration = "unsigned __int32 (__cdecl *DbgPrint)(const char *Format, ...);"
    if (plan.get("function_selection") != selection or len(requests) != 1 or len(applied) != 1 or len(thunks) != 1
            or requests[0].get("declaration") != expected_declaration
            or applied[0].get("declaration") != expected_declaration
            or applied[0].get("previous_type") != "ULONG (*)(PCSTR Format, ...)"
            or applied[0].get("applied_type") != "unsigned __int32 (*)(const char *Format, ...)"
            or thunks[0].get("previous_type") != _ALIAS_TYPES[0] or thunks[0].get("applied_type") != _ALIAS_TYPES[1]
            or thunks[0].get("source_function") != current["function"]
            or any(row.get("base_database_modified") is not False for row in [*applied, *thunks])):
        raise ValueError("Explicit alias review omitted original cdecl/ellipsis IAT and thunk application records")
    return {"schema_version": 1, "review_kind": "explicit_x86_dbgprint_unsigned_const_pointer_alias_abi",
        "validator_source": source_identity(a.ROOT), "source_input": current["source_input"],
        "base_capture": current["base_capture"], "original_function": current["function"],
        "previous_function": old_identity, "original_type": _ALIAS_TYPES[0], "previous_type": _ALIAS_TYPES[1],
        "previous_capture": evidence.identity(a, manifest_path), "previous_plan": evidence.identity(a, plan_path),
        "acquisition": acquisition_identity, "compiler_receipt": compiler_identity,
        "abi": _abi_sources(a, acquisition_identity, compiler_identity),
        "original_caller_cleanup": _caller_cleanup(a, current),
        "complete_field_differences": evidence.differences(original, previous),
        "machine_transfer_complete": True, "original_failure_preserved": True,
        "pseudocode_recovered": False, "provider_semantics_complete": False, "runtime_acceptance": False,
        "target_execution": False, "raw_artifacts_modified": False}


def previous_alias_forwarder_coverage(a, result, proof, old_identity):
    """Keep the default exact-type rule; separately verify an explicitly selected ABI-alias review."""
    proof = dict(proof)
    review_id = proof.pop("alias_abi_review")
    review = json.loads(evidence.checked(a, review_id))
    expected = build_alias_review(a, result, proof, old_identity, review["acquisition"], review["compiler_receipt"])
    if review != expected:
        raise ValueError("Alias review differs from its complete independently recomputed source-bound ABI evidence")
    return {"coverage_kind": "exact_previous_x86_variadic_import_jump_explicit_abi_alias_coverage",
        "previous_function": old_identity, "complete_forwarder_function": expected["original_function"],
        "source_input": expected["source_input"], "base_capture": expected["base_capture"],
        "rva": proof["rva"], "alias_abi_review": review_id, "explicit_abi": expected["abi"],
        "machine_transfer_complete": True, "original_failure_preserved": True,
        "original_hexrays_failure_reinterpreted_as_success": False, "whole_attempt_retired": False,
        "pseudocode_recovered": False, "provider_semantics_complete": False,
        "runtime_acceptance": False, "target_execution": False}


def _machine_scope(raw, body):
    """Keep entire raw chunks and identify bytes that the retained capture did not decode."""
    evidence.original_machine(raw, body)
    pe = forwarders.Pe32(raw)
    ranges = sorted((int(row["start"], 16), int(row["end"], 16)) for row in body["chunks"])
    if (len(set(ranges)) != len(ranges) or any(left[1] > right[0] for left, right in zip(ranges, ranges[1:]))
            or (int(body["address"], 16), int(body["end"], 16)) not in ranges
            or int(body["rva"], 16) + pe.base != int(body["address"], 16)):
        raise ValueError("Historical function chunks overlap, duplicate or omit their complete entry range")
    if any(not 1 <= len(bytes.fromhex(row["bytes"])) <= 15
           or int(row["rva"], 16) + pe.base != int(row["address"], 16) for row in body["assembly"]):
        raise ValueError("Historical instruction coordinate or length differs from the original x86 image")
    instructions = sorted((int(row["address"], 16), len(bytes.fromhex(row["bytes"]))) for row in body["assembly"])
    if (len({start for start, _ in instructions}) != len(instructions)
            or any(left[0] + left[1] > right[0] for left, right in zip(instructions, instructions[1:]))):
        raise ValueError("Historical function instructions overlap or duplicate")
    gaps = []
    for first, stop in ranges:
        pe.bytes(first - pe.base, stop - first, executable=True)
        at = first
        for start, size in instructions:
            if not first <= start < stop:
                continue
            if start > at:
                code = pe.bytes(at - pe.base, start - at)
                gaps.append({"start": hex(at), "end": hex(start), "raw_hex": code.hex(),
                             "sha256": hashlib.sha256(code).hexdigest()})
            at = start + size
        if at < stop:
            code = pe.bytes(at - pe.base, stop - at)
            gaps.append({"start": hex(at), "end": hex(stop), "raw_hex": code.hex(),
                         "sha256": hashlib.sha256(code).hexdigest()})
    return {"complete_original_chunks": body["chunks"], "retained_instruction_count": len(instructions),
            "bytes_not_marked_as_instructions": gaps, "undecoded_bytes_reinterpreted_as_code": False}


def _complete_replacement(a, result, resolution, base, raw, rva, previous):
    if rva in resolution.get("conflicting_functions", {}) or rva in resolution.get("conflicting_function_rvas", []):
        raise ValueError("Historical target still has a conflicting current replacement")
    selected = resolution.get("complete_functions", {}).get(rva)
    if selected is None:
        matches = [row for row in base["functions"] if row["rva"] == rva and row.get("pseudocode_captured") is True]
        if len(matches) != 1 or any(row.get("stage", "").lower().startswith(rva[2:].lower() + ":") for row in base["failures"]):
            raise ValueError("Historical target lacks a complete current typed replacement: " + rva)
        source = a.resolve(a.rel(result / matches[0]["path"]), a.rel(result))
        if a.sha(source) != matches[0]["sha256"]:
            raise ValueError("Historical target base replacement bytes changed")
        selected = {"capture": evidence.identity(a, source), **json.loads(source.read_bytes())}
    source = a.resolve(selected["capture"]["path"], a.rel(result))
    identity = evidence.identity(a, source)
    body = json.loads(evidence.checked(a, identity, a.rel(result)))
    if (identity["sha256"] != selected["capture"].get("sha256")
            or {key: value for key, value in selected.items() if key != "capture"} != body
            or body.get("rva") != rva or not isinstance(body.get("pseudocode"), dict)
            or not body["pseudocode"].get("lines")):
        raise ValueError("Current complete replacement differs from its retained exact typed function")
    manifest_path = source.parent.parent / "capture.json"
    manifest_id = evidence.identity(a, manifest_path)
    if manifest_id != resolution["base_capture"] and manifest_id not in [row["receipt"] for row in resolution.get("supplements", [])]:
        raise ValueError("Historical replacement does not belong to a currently verified supplement")
    manifest = json.loads(evidence.checked(a, manifest_id))
    matches = [row for row in manifest["functions"] if row["rva"] == rva]
    if (len(matches) != 1 or matches[0].get("path") != source.relative_to(manifest_path.parent).as_posix()
            or matches[0].get("sha256") != identity["sha256"] or matches[0].get("assembly_captured") is not True
            or matches[0].get("pseudocode_captured") is not True):
        raise ValueError("Historical replacement differs from its complete manifest inventory")
    evidence._typed_function(body)
    scope = _machine_scope(raw, body)
    if not evidence._same_machine(previous, body):
        raise ValueError("Partial target-history review cannot cover a different complete original machine scope")
    return {"replacement_function": identity, "replacement_supplement": manifest_id,
            "complete_original_machine_scope": scope}


def reviewed_partial_attempt_coverage(a, result, resolution, failure, failure_diagnostic=None):
    """Prove every target of an ended partial jump-walk failure; never turn that failure into success."""
    result = a.resolve(a.rel(result), ".analysis/results/native")
    path = a.resolve(failure["supplement"], a.rel(result / "supplements"))
    manifest_id = evidence.identity(a, path)
    manifest = json.loads(evidence.checked(a, manifest_id))
    plan_path, session_path = path.parent / "plan.json", path.parent / "session.json"
    plan, session = json.loads(plan_path.read_bytes()), json.loads(session_path.read_bytes())
    base_id = evidence.identity(a, result / "capture.json")
    base = json.loads(evidence.checked(a, base_id))
    source = evidence.identity(a, a.resolve(base["input"], ".analysis/inputs"))
    raw = evidence.checked(a, source, ".analysis/inputs")
    short_base = {key: base_id[key] for key in ("path", "sha256")}
    selection = manifest.get("function_selection", {})
    targets = selection.get("targets", [])
    rvas = [row.get("rva") for row in targets]
    records, failures = manifest.get("functions", []), manifest.get("failures", [])
    if (failure not in resolution.get("verification_failures", [])
            or resolution.get("base_capture") != base_id or resolution.get("target_execution") is not False
            or source["sha256"] != base.get("input_sha256") or source["bytes"] != base.get("input_bytes")
            or result.name != source["sha256"] or base.get("capture_complete") is not True
            or manifest.get("capture_complete") is not False or manifest.get("all_pseudocode_captured") is not False
            or manifest.get("input") != base["input"] or manifest.get("input_sha256") != source["sha256"]
            or manifest.get("base_capture") != short_base or manifest.get("database_integrity", {}).get("verified") is not True
            or manifest.get("base_capture_unchanged") is not True or manifest.get("base_database_unchanged") is not True
            or session.get("passed") is not False or type(session.get("exit_code")) is not int or session["exit_code"] != 0
            or plan.get("input") != base["input"] or plan.get("sha256") != source["sha256"]
            or plan.get("input_bytes") != source["bytes"] or plan.get("base_capture") != short_base
            or plan.get("output") != a.rel(path.parent) or plan.get("function_selection") != selection
            or any(row.get("target_execution") is not False or row.get("real_device_operations") is not False
                   for row in (manifest, plan, session))
            or plan.get("database") != manifest.get("database") or session.get("database") != manifest.get("database")
            or plan.get("source_database") != manifest.get("source_database")
            or session.get("capture_tool") != manifest.get("capture_tool")
            or not rvas or len(set(rvas)) != len(rvas) or len(records) != len(rvas)
            or [row.get("rva") for row in records] != rvas or not failures):
        raise ValueError("Partial history is not an exact ended failed target-chain capture with every selected original artifact")
    a.verify_native_supplement_request(plan, session)
    try:
        times = [datetime.fromisoformat(row[key]) for row, key in ((session, "started_at_utc"),
                 (manifest, "started_at_utc"), (manifest, "finished_at_utc"), (session, "finished_at_utc"))]
        if any(at.tzinfo is None for at in times) or times != sorted(times):
            raise ValueError("Partial history analyzer ending sequence differs")
    except (KeyError, TypeError) as error:
        raise ValueError("Partial history omitted its complete analyzer ending sequence") from error
    source_db = plan.get("source_database", {})
    source_path = a.resolve(source_db.get("path", ""), ".work/native")
    source_db_id = evidence.identity(a, source_path)
    private = a.resolve(manifest["database"].get("path", ""),
                        ".work/native/" + source["sha256"] + "/supplements/" + path.parent.name)
    expected_private = a.ROOT / ".work/native" / source["sha256"] / "supplements" / path.parent.name / source_path.name
    if (source_db_id["sha256"] != source_db.get("sha256")
            or manifest["database"].get("sha256") != source_db_id["sha256"] or private != expected_private.resolve()):
        raise ValueError("Partial history original source database changed")
    allowed = {"mode", "image_base", "architecture", "targets", "type_recoveries", "decompiler_limits",
               "static_observation", "function_shared_tails", "function_fresh_inlines"}
    if set(selection) - allowed or selection.get("mode") != "retained_function_reanalysis" or selection.get("architecture") != "x86":
        raise ValueError("Partial history selection contains an unreviewed operation")
    types = selection.get("type_recoveries", [])
    notes = {row["assembly_evidence"] for row in types}
    notes.update(row["assembly_evidence"] for row in selection.get("function_fresh_inlines", []))
    if len(notes) > 1:
        raise ValueError("Partial history selection has ambiguous original type/inline evidence")
    observation = selection.get("static_observation", {}).get("review_source")
    recomputed = a.native_function_selection(raw, base, [int(rva, 16) for rva in rvas],
        type_specs=[row["rva"] + "=" + row["declaration"] for row in types],
        type_note=next(iter(notes)) if notes else None,
        max_function_kib=selection.get("decompiler_limits", {}).get("max_function_kib"),
        shared_tail_specs=selection.get("function_shared_tails", []),
        fresh_inline_rvas=[int(row["rva"], 16) for row in selection.get("function_fresh_inlines", [])],
        observation_evidence=observation["path"] + "#" + observation["pointer"] if observation else None)
    if recomputed != selection:
        raise ValueError("Partial history intended scope differs from its independently recomputed original selection")
    functions, originals = [], {}
    for record in records:
        old_path = a.resolve(a.rel(path.parent / record["path"]), a.rel(path.parent / "functions"))
        old_id = evidence.identity(a, old_path)
        old = json.loads(evidence.checked(a, old_id, a.rel(path.parent)))
        rva = record["rva"]
        if (old_id["sha256"] != record.get("sha256") or record.get("assembly_captured") is not True
                or old.get("rva") != rva or record.get("address") != old.get("address")
                or record.get("pseudocode_captured") is not isinstance(old.get("pseudocode"), dict)):
            raise ValueError("Partial history original artifact differs from its complete selected inventory")
        original_scope = _machine_scope(raw, old)
        originals[rva] = old
        forwarder = resolution.get("complete_forwarders", {}).get(rva)
        if forwarder is not None:
            replacement = {"static_import_forwarder_coverage": forwarders.previous_function_coverage(a, result, forwarder, old_id)}
        else:
            replacement = _complete_replacement(a, result, resolution, base, raw, rva, old)
        functions.append({"rva": rva, "previous_function": old_id, "previous_machine_scope": original_scope, **replacement})
    chains = manifest.get("chains", [])
    if (len(chains) != len(rvas) or [row.get("requested_rva") for row in chains] != rvas
            or [row.get("index") for row in chains] != list(range(len(rvas)))):
        raise ValueError("Partial history omitted or reordered a selected target chain")
    incomplete = {}
    for chain in chains:
        rva = chain["requested_rva"]
        old = originals[rva]
        entries = chain.get("entries", [])
        if (len(entries) != 1 or entries[0].get("rva") != rva or entries[0].get("address") != old["address"]
                or type(chain.get("complete")) is not bool):
            raise ValueError("Partial history has an unreviewed additional target-chain entry")
        jump = entries[0].get("direct_jump_target_rva")
        if chain["complete"]:
            if jump is not None:
                raise ValueError("Partial history completed chain unexpectedly follows another target")
            continue
        first = old["assembly"][0]
        code = bytes.fromhex(first["bytes"])
        if first["address"] != old["address"] or not (len(code) == 2 and code[0] == 0xeb or len(code) == 5 and code[0] == 0xe9):
            raise ValueError("Partial target failure is not the retained original direct entry jump")
        destination = int(first["address"], 16) + len(code) + int.from_bytes(code[1:], "little", signed=True)
        if (jump != hex(destination - int(base["image_base"], 16)) or destination == int(old["address"], 16)
                or not any(int(chunk["start"], 16) < destination < int(chunk["end"], 16) for chunk in old["chunks"])
                or not any(row["address"] == hex(destination) for row in old["assembly"])
                or not any(edge.get("to") == hex(destination) and edge.get("type") == 19
                           and edge.get("code") is True for edge in first.get("xrefs_from", []))):
            raise ValueError("Partial target-chain jump is not an exact original interior block of the covered complete owner")
        incomplete[rva] = jump
    target_failures, hexrays_failures = set(), set()
    for item in failures:
        error = item.get("error", "")
        if not isinstance(error, str) or not error.startswith("Traceback (most recent call last):\n"):
            raise ValueError("Partial history omitted a complete original failure traceback")
        if item.get("stage") == "target" and item.get("requested_rva") in incomplete:
            if not error.endswith("ValueError: Requested entry lies inside a different function: " + incomplete[item["requested_rva"]] + "\n"):
                raise ValueError("Partial target-chain failure changed its exact original interior-block diagnostic")
            if item["requested_rva"] in target_failures:
                raise ValueError("Partial history repeats an original interior-target failure")
            target_failures.add(item["requested_rva"])
        elif item.get("stage") == "hexrays" and item.get("rva") in originals:
            if (records[rvas.index(item["rva"])].get("pseudocode_captured") is not False
                    or item["rva"] in hexrays_failures):
                raise ValueError("Partial history has a Hex-Rays failure for a purported successful original artifact")
            hexrays_failures.add(item["rva"])
        else:
            raise ValueError("Partial history contains an unreviewed failure outside original selected target chains")
    if (not incomplete or target_failures != set(incomplete)
            or hexrays_failures != {row["rva"] for row in records if row.get("pseudocode_captured") is False}):
        raise ValueError("Partial history did not preserve every original incomplete interior-target failure")
    if failure_diagnostic is not None:
        evidence.checked(a, failure_diagnostic)
    return {"capture": manifest_id, "session": evidence.identity(a, session_path), "plan": evidence.identity(a, plan_path),
        "verification_failure": copy.deepcopy(failure), "original_failures": copy.deepcopy(failures),
        "functions": functions, "source_database": source_db_id, "capture_outcome": "ended_partial_target_chain_failure",
        "original_target_chains": copy.deepcopy(chains), "partial_attempt_scope_independently_verified": True,
        "failed_attempt_success": False, "historical_failure_preserved": True,
        "raw_artifacts_modified": False, "target_execution": False,
        **({"failure_diagnostic": failure_diagnostic} if failure_diagnostic is not None else {})}
