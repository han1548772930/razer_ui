"""Recover webpack modules and source positions without executing target code."""
from __future__ import annotations

from bisect import bisect_right
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from types import SimpleNamespace


BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"


def decode_vlq(segment):
    values, value, shift = [], 0, 0
    for char in segment:
        digit = BASE64.find(char)
        if digit < 0 or shift > 60:
            raise ValueError("Invalid source-map VLQ")
        value |= (digit & 31) << shift
        if digit & 32:
            shift += 5
        else:
            values.append(-(value >> 1) if value & 1 else value >> 1)
            value, shift = 0, 0
    if shift:
        raise ValueError("Truncated source-map VLQ")
    return values


def encode_vlq(values):
    result = []
    for value in values:
        value = (-value << 1 | 1) if value < 0 else value << 1
        while value >= 32:
            result.append(BASE64[(value & 31) | 32])
            value >>= 5
        result.append(BASE64[value])
    return "".join(result)


def source_position(raw, byte_offset):
    """Source-map lines/columns are zero-based UTF-16, not UTF-8 bytes."""
    if not 0 <= byte_offset <= len(raw):
        raise ValueError("Source-map offset is outside the original")
    prefix = raw[:byte_offset].decode("utf-8")
    lines = re.split(r"\r\n|[\r\n\u2028\u2029]", prefix)
    return len(lines) - 1, len(lines[-1].encode("utf-16-le")) // 2






def nodes(root):
    pending = [root]
    while pending:
        node = pending.pop()
        yield node
        pending.extend(reversed(node.named_children))


def unparenthesized(node):
    while node is not None and node.type == "parenthesized_expression" and len(node.named_children) == 1:
        node = node.named_children[0]
    return node


def module_table(node):
    if node is None or node.type != "object" or not node.named_children:
        return None
    factories = []
    for pair in node.named_children:
        if pair.type == "comment":
            continue
        if pair.type == "pair":
            key, value = pair.child_by_field_name("key"), pair.child_by_field_name("value")
        elif pair.type == "method_definition":
            key, value = pair.child_by_field_name("name"), pair
        else:
            return None
        if key is None or not re.fullmatch(rb"\d+|[\"']\d+[\"']", key.text):
            return None
        value = unparenthesized(value)
        if value is None or value.type not in {"function_expression", "arrow_function", "method_definition"}:
            return None
        body = value.child_by_field_name("body")
        parameters = value.child_by_field_name("parameters")
        if body is None or body.type != "statement_block":
            return None
        if parameters is None:
            parameter = value.child_by_field_name("parameter")
            parameter_names = [parameter.text.decode()] if parameter is not None else []
        else:
            parameter_names = [part.text.decode() for part in parameters.named_children
                               if part.type != "comment"]
        factories.append({"module_id": int(key.text.strip(b"\"'")),
                          "factory_range": [value.start_byte, value.end_byte],
                          "body_range": [body.start_byte + 1, body.end_byte - 1],
                          "parameters": parameter_names, "syntax": value.type})
    if not factories:
        return None
    if len({row["module_id"] for row in factories}) != len(factories):
        raise ValueError("Webpack registration contains duplicate module identities")
    return factories


def webpack_receiver(node):
    node = unparenthesized(node)
    if node is not None and node.type == "assignment_expression":
        node = unparenthesized(node.child_by_field_name("left"))
    if node is None or node.type not in {"member_expression", "subscript_expression"}:
        return False
    key = node.child_by_field_name("property") or node.child_by_field_name("index")
    return key is not None and key.text.strip(b"\"'").startswith(b"webpackChunk")


def recovery_segments(root, *, include_standalone=False):
    """Select complete webpack expressions hidden by top-level comma sequences."""
    leaves = []
    for statement in root.named_children:
        if statement.type != "expression_statement" or not statement.named_children:
            continue
        pending = [statement.named_children[0]]
        while pending:
            expression = unparenthesized(pending.pop())
            if expression.type == "sequence_expression":
                pending.extend(reversed(expression.named_children))
            else:
                leaves.append(expression)
    result = []
    for expression in leaves:
        # A full standalone expression was already submitted to the analyzer.
        statement = expression.parent
        if statement.type == "expression_statement" and not include_standalone:
            continue
        callee = expression.child_by_field_name("function") if expression.type == "call_expression" else None
        if callee is not None and callee.type == "member_expression":
            prop = callee.child_by_field_name("property")
            receiver = callee.child_by_field_name("object")
            arguments = expression.child_by_field_name("arguments")
            values = arguments.named_children if arguments is not None else []
            if prop is not None and prop.text == b"push" and webpack_receiver(receiver) and len(values) == 1:
                array = values[0]
                parts = array.named_children if array.type == "array" else []
                factories = module_table(parts[1]) if len(parts) >= 2 else None
                if factories:
                    result.append({"kind": "webpack_chunk_registration",
                                   "range": [expression.start_byte, expression.end_byte],
                                   "factories": factories})
                    continue
        candidate = expression
        if candidate.type == "unary_expression":
            candidate = candidate.child_by_field_name("argument")
        if candidate is None or candidate.type != "call_expression":
            continue
        function = unparenthesized(candidate.child_by_field_name("function"))
        if function is None or function.type not in {"function_expression", "arrow_function"}:
            continue
        descendants = list(nodes(function))
        if not any(webpack_receiver(node) for node in descendants
                   if node.type in {"member_expression", "subscript_expression"}):
            continue
        tables = [table for node in descendants if node.type == "variable_declarator"
                  if (table := module_table(node.child_by_field_name("value")))]
        if tables:
            factories = max(tables, key=len)
            result.append({"kind": "webpack_bootstrap",
                           "range": [expression.start_byte, expression.end_byte], "factories": factories})
    return result


def factory_source_maps(document, raw, factories, sources):
    """Slice an existing readable bundle map, keeping complete factory bindings."""
    if document.get("version") != 3 or "sections" in document:
        raise ValueError("Factory slicing requires a flat version-3 source map")
    if len(sources) != len(document["sources"]):
        raise ValueError("Factory source-map identities are incomplete")
    line_starts = [0, *(match.end() for match in re.finditer(rb"\r\n|[\r\n]|\xe2\x80[\xa8\xa9]", raw))]
    def position(offset):
        line = bisect_right(line_starts, offset) - 1
        return line, len(raw[line_starts[line]:offset].decode().encode("utf-16-le")) // 2
    spans = []
    for factory in factories:
        start, end = map(position, factory["factory_range"])
        prefix = 2 if factory["syntax"] == "method_definition" else 1
        spans.append({"factory": factory, "start": start, "end": end, "prefix": prefix,
                      "lines": [[] for _ in range(end[0] - start[0] + 1)],
                      "last_source": 0, "last_line": 0, "last_column": 0, "last_name": 0,
                      "last_generated": {}, "mapped_segments": 0})
    spans.sort(key=lambda row: row["start"])
    if any(left["end"] > right["start"] for left, right in zip(spans, spans[1:])):
        raise ValueError("Readable webpack factories overlap")
    source = original_line = original_column = name = index = 0
    for generated_line, line in enumerate(document["mappings"].split(";")):
        generated_column = 0
        for segment in line.split(",") if line else []:
            values = decode_vlq(segment)
            if len(values) not in (1, 4, 5):
                raise ValueError("Invalid readable source-map segment width")
            generated_column += values[0]
            if len(values) > 1:
                source += values[1]
                original_line += values[2]
                original_column += values[3]
                if not 0 <= source < len(sources) or min(original_line, original_column) < 0:
                    raise ValueError("Readable source-map coordinate is invalid")
                if len(values) == 5:
                    name += values[4]
                    if not 0 <= name < len(document.get("names", [])):
                        raise ValueError("Readable source-map name is invalid")
            point = generated_line, generated_column
            while index < len(spans) and point >= spans[index]["end"]:
                index += 1
            if index == len(spans) or point < spans[index]["start"]:
                continue
            owner = spans[index]
            line_index = generated_line - owner["start"][0]
            column = generated_column - (owner["start"][1] if line_index == 0 else 0)
            column += owner["prefix"] if line_index == 0 else 0
            encoded = [column - owner["last_generated"].get(line_index, 0)]
            owner["last_generated"][line_index] = column
            if len(values) > 1:
                encoded.extend((source - owner["last_source"], original_line - owner["last_line"],
                                original_column - owner["last_column"]))
                owner.update(last_source=source, last_line=original_line, last_column=original_column)
                owner["mapped_segments"] += 1
                if len(values) == 5:
                    encoded.append(name - owner["last_name"])
                    owner["last_name"] = name
            owner["lines"][line_index].append(encode_vlq(encoded))
    result = {}
    for owner in spans:
        if not owner["mapped_segments"]:
            raise ValueError("Readable factory has no verified original source-map positions")
        mapping = {key: value for key, value in document.items()
                   if key not in {"sourcesContent", "sourceRoot", "mappings", "file"}}
        owner["lines"][0].insert(0, "A")  # The synthetic opening wrapper has no original position.
        mapping.update(sources=sources, mappings=";".join(",".join(line) for line in owner["lines"]),
                       x_recovery_kind="complete_lexical_factory_from_verified_readable_bundle")
        result[owner["factory"]["module_id"]] = mapping
    return result




def recover_one(input_dir, tool, audit):
    import analysis as a
    output = a.ROOT / ".analysis/results/frontend" / input_dir.name
    receipt_path = output / "analysis.json"
    acquisition_path = input_dir / "acquisition.json"
    acquired = json.loads(acquisition_path.read_bytes())
    captured = json.loads(receipt_path.read_bytes())
    originals = {row["path"]: {key: row[key] for key in ("path", "bytes", "sha256")}
                 for row in acquired["files"] if row.get("path")}
    if originals != {row["path"]: row for row in captured["original_inputs"]}:
        raise ValueError("Recovery capture does not cover the current acquisition bytes")
    if captured["tool"]["sha256"] != a.sha(tool):
        raise ValueError("Recovery analyzer differs from the verified complete capture")
    for row in originals.values():
        audit.read(row)
    for row in captured["outputs"]:
        audit.artifact(a.ROOT / row["path"], row)
    previous = captured.get("module_recovery")
    if previous is not None:
        audit.artifact(a.ROOT / previous["recipe"]["path"], previous["recipe"])
        if audit.final_snapshot():
            raise ValueError("Recovery input or output changed during verification")
        return {"input": a.rel(input_dir), "status": "reused_verified_recovery",
                "registered_factories": previous["registered_factories"],
                "recovered_modules": previous["recovered_modules"],
                "missing_factories": len(previous["missing_factories"]),
                "scan_failures": len(previous["scan_failures"])}
    recipe_path = output / "factory-recovery-tool.py"
    recipe_path.write_bytes(Path(__file__).read_bytes())
    generated = {recipe_path}
    recipe = {"path": a.rel(recipe_path), "sha256": a.sha(recipe_path),
              "parser_versions": audit.parser_versions}
    provenance_path = output / "modules/provenance.json"
    provenance = json.loads(provenance_path.read_bytes())
    indexed = {}
    for name, owner in provenance["modules"].items():
        if owner["input"]:
            source = (input_dir / "raw" / owner["input"].replace("\\", "/")).resolve()
            indexed.setdefault(source, []).append((name, owner))
    selected, recovered, commands, scan_failures, readable_scripts = [], [], [], [], []
    def command(label, argv):
        stdout, stderr = output / (label + ".stdout.txt"), output / (label + ".stderr.txt")
        with stdout.open("wb") as out, stderr.open("wb") as err:
            completed = subprocess.run(argv, cwd=a.ROOT, stdout=out, stderr=err, check=False)
        row = {"stage": label, "argv": [a.rel(Path(arg)) if Path(arg).is_absolute() and Path(arg).is_relative_to(a.ROOT) else arg for arg in argv],
               "cwd": ".", "exit_code": completed.returncode, "stdout": a.rel(stdout), "stderr": a.rel(stderr)}
        commands.append(row)
        generated.update((stdout, stderr))
        return row
    for source in originals.values():
        if source["path"] not in captured["original_js_inputs"]:
            continue
        raw = audit.read(source)
        if b"webpackChunk" not in raw:
            continue
        try:
            segments = recovery_segments(audit.tree(source).root_node)
        except ValueError as error:
            scan_failures.append({"original": source, "error": str(error)})
            continue
        linked = indexed.get((a.ROOT / source["path"]).resolve(), [])
        pending = []
        for segment in segments:
            factories = [factory for factory in segment["factories"] if not any(
                any(factory["factory_range"][0] <= start < end <= factory["factory_range"][1]
                    for start, end in owner["ranges"]) for _, owner in linked)]
            if factories:
                pending.append({**segment, "factories": factories})
        if not pending:
            audit.parsed.pop(source["path"], None)
            continue
        full = [(name, owner) for name, owner in linked
                if owner["ranges"] == [[0, len(raw)]]]
        if len(full) > 1:
            raise ValueError("More than one complete readable script claims the original")
        if full:
            readable = output / "modules" / full[0][0]
        else:
            readable = output / "readable-scripts" / (source["sha256"] + ".js")
            readable.parent.mkdir(parents=True, exist_ok=True)
            completed = command("recover-format-" + source["sha256"][:16],
                [str(tool), str(a.ROOT / source["path"]), "--level", "minimal",
                 "--emit-source-map", "--diagnostics", "--json", "-o", str(readable)])
            if completed["exit_code"]:
                raise ValueError("Whole-script static formatting failed; diagnostics retained")
            generated.update((readable, readable.with_name(readable.name + ".map")))
        readable_raw = readable.read_bytes()
        tree = audit.parser.parse(readable_raw)
        if tree.root_node.has_error:
            raise ValueError("Verified readable script has unresolved syntax")
        readable_factories = {}
        for segment in recovery_segments(tree.root_node, include_standalone=True):
            for factory in segment["factories"]:
                if factory["module_id"] in readable_factories:
                    raise ValueError("Readable script has ambiguous webpack module identities")
                readable_factories[factory["module_id"]] = factory
        needed = []
        for segment in pending:
            for factory in segment["factories"]:
                readable_factory = readable_factories.get(factory["module_id"])
                if readable_factory is None or len(readable_factory["parameters"]) != len(factory["parameters"]):
                    raise ValueError("Readable factory lost its complete original parameter bindings")
                needed.append(readable_factory)
        map_path = readable.with_name(readable.name + ".map")
        document = json.loads(map_path.read_bytes())
        for literal in document["sources"]:
            if (map_path.parent / document.get("sourceRoot", "") / literal).resolve() != (a.ROOT / source["path"]).resolve():
                raise ValueError("Readable source map points outside its verified original")
        directory = output / "modules/factories" / source["sha256"]
        directory.mkdir(parents=True, exist_ok=True)
        sources = [os.path.relpath(a.ROOT / source["path"], directory).replace("\\", "/")
                   for _ in document["sources"]]
        mappings = factory_source_maps(document, readable_raw, needed, sources)
        for segment in pending:
            selected.append({"original": source, **segment})
            for factory in segment["factories"]:
                readable_factory = readable_factories[factory["module_id"]]
                start, end = readable_factory["factory_range"]
                value = readable_raw[start:end]
                wrapped = b"({" + value + b"});" if readable_factory["syntax"] == "method_definition" else b"(" + value + b");"
                if audit.parser.parse(wrapped).root_node.has_error:
                    raise ValueError("Complete recovered factory has invalid syntax")
                target = directory / ("module-" + str(factory["module_id"]) + ".factory.js")
                target.write_bytes(wrapped + b"\n")
                mapping_path = target.with_name(target.name + ".map")
                mapping = mappings[factory["module_id"]]
                mapping["file"] = target.name
                a.save(mapping_path, mapping)
                generated.update((target, mapping_path))
                key = target.relative_to(output / "modules").as_posix()
                if key in provenance["modules"]:
                    raise ValueError("Factory recovery would overwrite an existing module identity")
                provenance["modules"][key] = {
                    "input": os.path.relpath(a.ROOT / source["path"], input_dir / "raw").replace("\\", "/"),
                    "ranges": [factory["factory_range"]], "extraction": "complete_lexical_factory",
                    "factory_parameters": factory["parameters"], "module_id": factory["module_id"],
                    "readable_script": a.rel(readable), "readable_range": readable_factory["factory_range"]}
                recovered.append({"output": key, "original": source["path"], **factory})
        readable_scripts.append({"original": source, "readable_script": a.rel(readable),
                                 "runtime_and_non_factory_code_preserved": True})
        audit.parsed.pop(source["path"], None)
        del tree, readable_raw, document, mappings
    a.save(provenance_path, provenance)
    generated.add(provenance_path)
    changed = [name for name in audit.final_snapshot()
               if name not in {a.rel(provenance_path), a.rel(recipe_path)}]
    if changed:
        raise ValueError("Evidence changed during recovery: " + ", ".join(changed))
    captured["commands"].extend(commands)
    captured["module_recovery"] = {
        "strategy": "complete_lexical_factory_slices", "recipe": recipe, "target_execution": False,
        "registered_factories": sum(len(row["factories"]) for row in selected),
        "recovered_modules": len(recovered), "registrations": selected, "factories": recovered,
        "readable_scripts": readable_scripts, "missing_factories": [], "scan_failures": scan_failures,
        "original_acquisition_sha256": a.sha(acquisition_path),
        "source_map_kind": "composed_from_verified_readable_bundle",
        "official_original_names_recovered": False}
    outputs = {row["path"]: row for row in captured["outputs"]}
    for path in generated:
        outputs[a.rel(path)] = {"path": a.rel(path), "bytes": path.stat().st_size, "sha256": a.sha(path)}
    captured["outputs"] = [outputs[key] for key in sorted(outputs)]
    captured["recorded_at_utc"] = a.stamp()
    a.save(receipt_path, captured)
    return {"input": a.rel(input_dir), "status": "recovered_with_diagnostics" if scan_failures else "recovered",
            "registrations": len(selected), "registered_factories": captured["module_recovery"]["registered_factories"],
            "recovered_modules": len(recovered), "missing_factories": 0, "scan_failures": len(scan_failures),
            "failed_stages": sum(bool(row["exit_code"]) for row in commands)}


def recover_source_set(path, tool):
    import analysis as a
    from host import HostLocalAudit
    audit = HostLocalAudit()
    try:
        acquired = json.loads(path.read_bytes())
        if acquired.get("source_current_during_acquisition") is not True:
            return {"input": a.rel(path.parent), "status": "outside_current_deployment_scope"}
        if not any(row.get("path", "").endswith(".js") for row in acquired["files"]):
            return {"input": a.rel(path.parent), "status": "no_javascript_originals"}
        capture = a.ROOT / ".analysis/results/frontend" / path.parent.name / "analysis.json"
        if not capture.is_file():
            a.analyze_frontend(SimpleNamespace(input=a.rel(path.parent), no_refresh_gaps=True), refresh_ledger=False)
            restored = json.loads(capture.read_bytes())["module_recovery"]
            return {"input": a.rel(path.parent), "status": "captured_and_recovered",
                    "registered_factories": restored["registered_factories"], "recovered_modules": restored["recovered_modules"]}
        return recover_one(path.parent, tool, audit)
    except Exception as error:
        return {"input": a.rel(path.parent), "status": "failed", "error": str(error).replace(str(a.ROOT), "<project>")}


def recover_frontend(args):
    import analysis as a
    inputs = sorted((a.ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")) if getattr(args, "all_acquired", False) else [a.resolve(args.input, ".analysis/inputs") / "acquisition.json"]
    tool, rows = a.wakaru(), []
    workers = getattr(args, "workers", 2) if getattr(args, "all_acquired", False) else 1
    if not 1 <= workers <= 8:
        raise ValueError("Frontend recovery workers must be between 1 and 8")
    executor = concurrent.futures.ProcessPoolExecutor if workers > 1 else concurrent.futures.ThreadPoolExecutor
    with executor(max_workers=workers) as pool:
        futures = {pool.submit(recover_source_set, path, tool): path for path in inputs}
        for future in concurrent.futures.as_completed(futures):
            row = future.result()
            rows.append(row)
            print(json.dumps({"source_sets_done": len(rows), "source_sets_total": len(inputs), **row}), flush=True)
            if getattr(args, "all_acquired", False):
                a.save(a.ROOT / ".analysis/results/frontend/module-recovery.json",
                       {"recorded_at_utc": a.stamp(), "target_execution": False, "source_sets": rows})
    if getattr(args, "all_acquired", False):
        a.save(a.ROOT / ".analysis/results/frontend/module-recovery.json",
               {"recorded_at_utc": a.stamp(), "target_execution": False, "source_sets": rows})
    if not getattr(args, "no_refresh_gaps", False):
        a.refresh_gaps()
    if any(row["status"] == "failed" for row in rows):
        raise ValueError("Frontend recovery has failed source sets; inspect the retained diagnostics")
    return rows
