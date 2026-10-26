"""Bind independently checked source and architecture contracts to native originals."""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile

import native_adjudication as evidence


KINDS = ("ia64", "source-assembly")
BINDING = "static-contract-review.json"


def _verified(a, kind, review):
    if kind == "ia64":
        import native_ia64_contracts
        verified = native_ia64_contracts.verify(a, review)
        rows = verified["complete_static_contracts"]
        source = verified["source_input"]
    elif kind == "source-assembly":
        import native_source_correspondence
        verified = native_source_correspondence.verify_review(a, review)
        grouped = verified["complete_sources"]
        if len(grouped) != 1:
            raise ValueError("Static source review must bind one exact original image")
        digest, rows = next(iter(grouped.items()))
        if not rows:
            raise ValueError("Static source review omitted its original functions")
        source = next(iter(rows.values()))["source_input"]
        if source["sha256"] != digest:
            raise ValueError("Static source review grouped a different original image")
    else:
        raise ValueError("Unknown independently maintained native static review kind")
    if (not isinstance(rows, dict) or not rows or verified.get("target_execution") is not False
            or any(verified.get(key, False) is not False for key in
                   ("pseudocode_recovered", "runtime_acceptance", "provider_semantics_complete"))):
        raise ValueError("Static review extended its original evidence or execution boundary")
    result = a.resolve(a.rel(a.ROOT / ".analysis/results/native" / source["sha256"]), ".analysis/results/native")
    capture_identity = evidence.identity(a, result / "capture.json")
    capture = json.loads(evidence.checked(a, capture_identity))
    actual_source = evidence.identity(a, a.resolve(capture["input"], ".analysis/inputs"))
    if source != actual_source or capture["input_sha256"] != source["sha256"] or capture["input_bytes"] != source["bytes"]:
        raise ValueError("Static contract original input changed")
    records = {row["rva"]: row for row in capture["functions"]}
    proofs = {}
    for rva, row in rows.items():
        if (not isinstance(row, dict) or row.get("rva") != rva or rva not in records
                or row.get("source_input") != source or row.get("base_capture") != capture_identity
                or not isinstance(row.get("coverage_kind"), str)):
            raise ValueError("Static contract omitted an exact original function identity")
        function = evidence.identity(a, a.resolve(a.rel(result / records[rva]["path"]), a.rel(result)))
        if function["sha256"] != records[rva]["sha256"] or row.get("function") != function:
            raise ValueError("Static contract function differs from the original capture")
        failures = [failure for failure in capture["failures"]
                    if failure.get("stage", "").lower() == rva[2:].lower() + ":hexrays"]
        if not failures:
            raise ValueError("Static contract has no original failure in its bound scope")
        proofs[rva] = {"review_kind": kind, "review": review, "proof": row,
            "source_input": source, "base_capture": capture_identity, "function": function, "rva": rva,
            "original_hexrays_failures": failures, "pseudocode_recovered": False,
            "target_execution": False, "rust_independent_implementation_complete": False,
            "ui_backend_wiring_complete": False, "runtime_acceptance": False}
    return source, capture_identity, proofs


def _checked_binding(a, result, binding, capture_identity):
    required = {"schema_version", "review_kind", "review", "source_input", "base_capture",
                "function_rvas", "target_execution", "pseudocode_recovered", "runtime_acceptance"}
    if (not isinstance(binding, dict) or not required.issubset(binding)
            or set(binding) - required - {"previous_binding_snapshot"} or binding.get("schema_version") != 1
            or binding.get("base_capture") != capture_identity
            or any(binding.get(key) is not False for key in
                   ("target_execution", "pseudocode_recovered", "runtime_acceptance"))):
        raise ValueError("Native static review binding changed its exact evidence scope")
    source, base, proofs = _verified(a, binding["review_kind"], binding["review"])
    if (source != binding["source_input"] or source["sha256"] != result.name or base != capture_identity
            or binding["function_rvas"] != sorted(proofs, key=lambda rva: int(rva, 16))):
        raise ValueError("Native static review binding omitted or changed its original targets")
    snapshot = binding.get("previous_binding_snapshot")
    seen = set()
    while snapshot is not None:
        if not isinstance(snapshot, dict):
            raise ValueError("Native static review history omitted an exact snapshot identity")
        if snapshot.get("sha256") in seen:
            raise ValueError("Native static review history contains a repeated binding")
        seen.add(snapshot.get("sha256"))
        previous = json.loads(evidence.checked(a, snapshot))
        if (not isinstance(previous, dict) or previous.get("source_input") != source or previous.get("base_capture") != base
                or previous.get("review_kind") != binding["review_kind"]
                or not set(previous.get("function_rvas", [])).issubset(proofs)
                or any(previous.get(key) is not False for key in
                       ("target_execution", "pseudocode_recovered", "runtime_acceptance"))):
            raise ValueError("Native static review lost a previous exact input or completion boundary")
        snapshot = previous.get("previous_binding_snapshot")
    return proofs


def publish_binding(a, args):
    if not re.fullmatch(r"[0-9a-f]{64}", args.review_sha256 or ""):
        raise ValueError("Native static publication requires the explicitly selected review SHA-256")
    review = evidence.identity(a, a.resolve(args.review, ".analysis/results"))
    if review["sha256"] != args.review_sha256:
        raise ValueError("Native static review bytes changed after explicit selection")
    original = evidence.identity(a, a.resolve(args.input, ".analysis/inputs"))
    with a.index_lock(a.ROOT / ".analysis/reverse-pending.lock"):
        source, base, proofs = _verified(a, args.kind, review)
        if source != original:
            raise ValueError("Native static publication selected a different original input")
        result = a.resolve(a.rel(a.ROOT / ".analysis/results/native" / source["sha256"]), ".analysis/results/native")
        destination = result / BINDING
        binding = {"schema_version": 1, "review_kind": args.kind, "review": review,
            "source_input": source, "base_capture": base,
            "function_rvas": sorted(proofs, key=lambda rva: int(rva, 16)),
            "target_execution": False, "pseudocode_recovered": False, "runtime_acceptance": False}
        previous_raw = None
        if destination.is_symlink():
            raise ValueError("Native static review cannot follow a binding link")
        previous_sha = getattr(args, "previous_binding_sha256", None)
        if destination.exists():
            previous_raw = destination.read_bytes()
            previous = json.loads(previous_raw)
            if not isinstance(previous, dict):
                raise ValueError("Previous native static binding is not a complete object")
            if all(previous.get(key) == value for key, value in binding.items()):
                binding = previous
            elif (previous_sha != hashlib.sha256(previous_raw).hexdigest()
                    or previous.get("source_input") != source or previous.get("base_capture") != base
                    or previous.get("review_kind") != args.kind
                    or not set(previous.get("function_rvas", [])).issubset(binding["function_rvas"])):
                raise ValueError("A different native static review requires its exact previous SHA and complete preserved scope")
            else:
                import reverse_pending
                binding["previous_binding_snapshot"] = reverse_pending.retain_review_json(raw=previous_raw)
        elif previous_sha is not None:
            raise ValueError("Native static review replacement has no previous binding")
        raw = (json.dumps(binding, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
        temporary = None
        try:
            if destination.is_symlink():
                raise ValueError("Native static review cannot follow a binding link")
            if destination.exists():
                if destination.read_bytes() != raw:
                    with tempfile.NamedTemporaryFile(prefix=".static-contract-", suffix=".tmp", dir=result, delete=False) as stream:
                        temporary = Path(stream.name)
                        stream.write(raw)
                        stream.flush()
                        os.fsync(stream.fileno())
                    if destination.read_bytes() != previous_raw or destination.is_symlink():
                        raise ValueError("Native static review binding changed before atomic replacement")
                    os.replace(temporary, destination)
            else:
                with tempfile.NamedTemporaryFile(prefix=".static-contract-", suffix=".tmp", dir=result, delete=False) as stream:
                    temporary = Path(stream.name)
                    stream.write(raw)
                    stream.flush()
                    os.fsync(stream.fileno())
                try:
                    os.link(temporary, destination)
                except FileExistsError:
                    if destination.is_symlink() or destination.read_bytes() != raw:
                        raise ValueError("Concurrent native static publication changed the binding")
            if destination.read_bytes() != raw:
                raise ValueError("Published native static review bytes differ")
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
        return {"binding": evidence.identity(a, destination), "review": review, "source_input": source,
            "base_capture": base, "reviewed_static_function_rvas": binding["function_rvas"],
            "target_execution": False, "pseudocode_recovered": False, "runtime_acceptance": False,
            "queue_status_modified": False}


def apply_resolution(a, result, resolution, capture):
    destination = result / BINDING
    if not destination.exists() and not destination.is_symlink():
        return
    try:
        if destination.is_symlink():
            raise ValueError("Native static resolution cannot follow a binding link")
        reference = evidence.identity(a, destination)
        binding = json.loads(evidence.checked(a, reference))
        if (json.loads(evidence.checked(a, resolution["base_capture"])) != capture
                or resolution.get("target_execution") is not False):
            raise ValueError("Native static resolution changed its original capture or static boundary")
        proofs = _checked_binding(a, result, binding, resolution["base_capture"])
        selected = {rva: {**proof, "binding": reference} for rva, proof in proofs.items()}
        if resolution.get("complete_static_contracts", {}) not in ({}, selected):
            raise ValueError("Native static contracts conflict with another selected review")
        resolved, remaining = list(resolution["resolved_failures"]), []
        for failure in resolution["remaining_failures"]:
            match = re.fullmatch(r"([0-9a-fA-F]+):hexrays", failure.get("stage", ""))
            rva = hex(int(match[1], 16)) if match else None
            if rva not in selected:
                remaining.append(failure)
                continue
            proof = selected[rva]
            if failure not in proof["original_hexrays_failures"]:
                raise ValueError("Native static resolution changed an original analyzer failure")
            resolved.append({"original_failure": copy.deepcopy(failure), "function_rva": rva,
                "resolved_by": "independently_verified_static_contract", "complete_static_contract": proof})
        resolution.update(complete_static_contracts=selected, resolved_failures=resolved, remaining_failures=remaining)
    except (OSError, ValueError, KeyError, TypeError) as error:
        resolution["verification_failures"].append({"static_contract_review": a.rel(destination), "error": str(error)})


def previous_function_coverage(a, result, contract, previous_function):
    """Cover old machine bytes without accepting failed decompiler type annotations."""
    reference = contract["binding"]
    binding = json.loads(evidence.checked(a, reference))
    if evidence.identity(a, result / BINDING) != reference:
        raise ValueError("Previous static function review binding changed")
    # The resolution owner has independently checked the binding once for this query.
    if (binding["review"] != contract["review"] or binding["base_capture"] != contract["base_capture"]
            or binding["source_input"] != contract["source_input"] or contract["rva"] not in binding["function_rvas"]):
        raise ValueError("Previous static function belongs to another bound review")
    old = json.loads(evidence.checked(a, previous_function, a.rel(result)))
    original = json.loads(evidence.checked(a, contract["function"], a.rel(result)))
    raw = evidence.checked(a, contract["source_input"], ".analysis/inputs")
    evidence.original_machine(raw, old)
    if not evidence._same_machine(old, original):
        raise ValueError("Previous static function omitted or changed complete original machine coverage")
    return {"coverage_kind": "reviewed_original_static_contract_covers_previous_machine",
        "rva": contract["rva"], "previous_function": previous_function,
        "replacement_static_contract": contract, "previous_type": old.get("type"),
        "original_type": original.get("type"), "previous_analyzer_type_accepted": False,
        "pseudocode_recovered": False, "target_execution": False, "runtime_acceptance": False}


def publish_operation(a, args):
    """Keep a content-addressed operation receipt and the exact maintenance sources."""
    import reverse_pending
    if args.command == "bind-native-import-forwarders":
        import native_import_forwarders as validator
    else:
        validator = __import__(__name__)
    outcome = validator.publish_binding(a, args)
    source_paths = [Path(__file__), Path(a.__file__), Path(evidence.__file__), Path(validator.__file__)]
    if args.command != "bind-native-import-forwarders":
        source_paths.append(Path(__import__("native_ia64_contracts" if args.kind == "ia64" else
                                            "native_source_correspondence").__file__))
    sources = []
    for source in source_paths:
        raw = source.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        target = a.ROOT / ".analysis/results/reverse-pending/tool-sources" / (digest + ".py")
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            with target.open("xb") as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
        except FileExistsError:
            if target.read_bytes() != raw:
                raise ValueError("Retained native static publication source bytes differ")
        record = evidence.identity(a, target)
        if record not in sources:
            sources.append(record)
    operation = {"schema_version": 1, "operation": args.command, "recorded_at_utc": a.stamp(),
                 **outcome, "tool_sources": sources}
    receipt = reverse_pending.retain_review_json(operation)
    print(json.dumps({"receipt": receipt, **outcome}), flush=True)
    return {"receipt": receipt, **operation}
