"""Prove narrow roles of assigned factories without inferring page completion.

String exports must have no executable statements beyond literal declarations
and the actual webpack export calls. Locale runtimes retain executable owners
and remain behavioral dependencies, even when most bytes are dictionaries.
"""
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from contextlib import closing
import hashlib
import json
from pathlib import Path

import analysis as a
import code_identity as ci
import evidence

LANGUAGE_API = {"getLanguageItem", "getTextItem", "swapLang", "updateLanguage"}


def _expression_items(node):
    if node.type == "sequence_expression":
        for child in node.named_children:
            yield from _expression_items(child)
    else:
        yield node


def _owner(source, node):
    raw = source.raw[node.start_byte:node.end_byte]
    return {"start_byte": node.start_byte, "end_byte": node.end_byte,
            "syntax": node.type, "raw_sha256": hashlib.sha256(raw).hexdigest(),
            "normalized_sha256": ci.normalized_ast(node)}


def classify(source, factory):
    """Classify a complete original factory; failure to prove data stays open."""
    body = factory.child_by_field_name("body")
    parameters = factory.child_by_field_name("parameters")
    parameters = parameters.named_children if parameters is not None else []
    if body is None or len(parameters) != 3 or any(p.type != "identifier" for p in parameters):
        return {"role": "unclassified", "reason": "factory_parameter_or_body_shape"}
    exported, require = parameters[1].text, parameters[2].text
    providers = [(call, dictionary) for call, target, dictionary in source.exports
                 if source.scope(call) == body and target.text == exported
                 and call.child_by_field_name("function").child_by_field_name("object").text == require]
    if len(providers) != 1:
        return {"role": "unclassified", "reason": "export_provider_not_unique"}
    export_call, dictionary = providers[0]
    fields = {}
    try:
        for pair in dictionary.named_children:
            if pair.type != "pair":
                raise ValueError("Export map has a spread or shorthand")
            key = source.property_key(pair.child_by_field_name("key"), body)
            if key in fields:
                raise ValueError("Export map repeats a key")
            fields[key] = source.returned(pair.child_by_field_name("value"))
        if set(fields) == LANGUAGE_API:
            implementations = {}
            for name, value in fields.items():
                declaration = source.declaration(value, body)
                implementation = declaration.child_by_field_name("value")
                if implementation.type not in {"arrow_function", "function_expression"}:
                    raise ValueError("Language API is not a local callable")
                implementations[name] = _owner(source, declaration)
            return {"role": "locale_runtime_with_dictionaries", "export_provider": _owner(source, export_call),
                    "implementations": implementations, "pure_data": False,
                    "scope": "Actual language API exports and local callable owners; runtime semantics remain a shared dependency"}
        values = {key: source.literal(value, body) for key, value in fields.items()}
        for statement in body.named_children:
            if statement.type in {"comment", "empty_statement"}:
                continue
            if statement.type in {"variable_declaration", "lexical_declaration"}:
                for declaration in statement.named_children:
                    if declaration.type != "variable_declarator" or declaration.child_by_field_name("name").type != "identifier":
                        raise ValueError("Data factory has a nonliteral declaration")
                    source.literal(declaration.child_by_field_name("value"), body)
                continue
            if statement.type != "expression_statement" or len(statement.named_children) != 1:
                raise ValueError("Data factory has additional executable statements")
            for expression in _expression_items(statement.named_children[0]):
                if expression.type == "string":
                    continue
                if expression.type != "call_expression":
                    raise ValueError("Data factory has a non-export expression")
                function = expression.child_by_field_name("function")
                arguments = expression.child_by_field_name("arguments").named_children
                if function.type != "member_expression" or function.child_by_field_name("object").text != require:
                    raise ValueError("Data factory invokes another callable")
                method = function.child_by_field_name("property").text
                if method == b"r" and len(arguments) == 1 and arguments[0].text == exported:
                    continue
                if method == b"d" and expression == export_call:
                    continue
                raise ValueError("Data factory has additional runtime calls")
    except (ValueError, AttributeError) as error:
        return {"role": "unclassified", "reason": str(error)}
    return {"role": "static_string_exports", "pure_data": True, "export_count": len(values),
            "export_values_sha256": ci.digest(values), "export_provider": _owner(source, export_call),
            "lighting_named_export": values.get("TAB_LIGHTING"),
            "lighting_key_exports": sorted(key for key, value in values.items() if value == "TAB_LIGHTING"),
            "scope": "Complete factory contains only lexical string declarations, string getters and webpack export registration; no device/page behavior"}


def inspect_source(group):
    original, factories = group
    path = a.resolve(original["path"], ".analysis/inputs")
    before = path.stat()
    raw = path.read_bytes()
    if len(raw) != original["bytes"] or hashlib.sha256(raw).hexdigest() != original["sha256"]:
        raise ValueError("Role source byte identity differs")
    source = evidence._LocaleSource(raw)
    results = []
    for row in factories:
        node = source.tree.root_node.descendant_for_byte_range(row["start_byte"], row["end_byte"] - 1)
        if (node.start_byte, node.end_byte) != (row["start_byte"], row["end_byte"]):
            raise ValueError("Role source no longer owns a complete factory node")
        if ci.normalized_ast(node, factory=True) != row["normalized_sha"]:
            raise ValueError("Role source AST identity differs from its indexed variant")
        results.append({"normalized_sha256": row["normalized_sha"], "original": original,
            "module_id": row["module_id"], "start_byte": row["start_byte"], "end_byte": row["end_byte"],
            **classify(source, node)})
    after = path.stat()
    if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
        raise ValueError("Role source changed during query")
    return results


def build(db_path, owner, *, output=None, workers=2, variants=None):
    """Publish role evidence for one existing capability reservation."""
    if not 1 <= workers <= 4:
        raise ValueError("Role inspection requires one to four bounded workers")
    destination = Path(output) if output else a.ROOT / ".analysis/results/frontend" / (owner.replace("_", "-") + "-variant-roles.json")
    destination = a.resolve(a.rel(destination), ".analysis/results")
    if destination.exists():
        raise FileExistsError("Retain existing role evidence; use a new explicit destination")
    with closing(ci.connect(db_path)) as db:
        claim = db.execute("SELECT * FROM capability_assignments WHERE owner=?", (owner,)).fetchone()
        if claim is None:
            raise ValueError("Role inspection needs an existing capability assignment")
        rows = [dict(row) for row in db.execute("""SELECT f.*,s.path,s.bytes FROM variant_assignments va
            JOIN factories f USING(normalized_sha) JOIN sources s ON s.sha256=f.source_sha
            JOIN deployment_sources ds ON ds.path=s.path JOIN deployments d USING(product_id,layer)
            WHERE va.owner=? AND d.family=va.family AND d.failure IS NULL
            GROUP BY f.normalized_sha ORDER BY f.source_sha,f.start_byte""", (owner,))]
    reserved_count = len(rows)
    if variants is not None:
        selected = set(variants)
        if not selected or len(selected) != len(variants) or not selected <= {row["normalized_sha"] for row in rows}:
            raise ValueError("Selected role variants must be distinct members of the existing reservation")
        rows = [row for row in rows if row["normalized_sha"] in selected]
    grouped = {}
    for row in rows:
        if row["source_sha"] not in grouped:
            grouped[row["source_sha"]] = ({"path": row["path"], "sha256": row["source_sha"], "bytes": row["bytes"]}, [])
        grouped[row["source_sha"]][1].append(row)
    results, failures = [], []
    groups = list(grouped.values())
    with ThreadPoolExecutor(max_workers=workers) as pool:
        for start in range(0, len(groups), workers):
            batch = groups[start:start + workers]
            futures = [pool.submit(inspect_source, group) for group in batch]
            for group, future in zip(batch, futures):
                try:
                    results.extend(future.result())
                except (ValueError, OSError) as error:
                    failures.append({"original": group[0], "error": str(error)})
            if start % 20 == 0:
                print(json.dumps({"role_owner": owner, "sources_verified": min(start + workers, len(groups)), "sources": len(groups)}), flush=True)
    counts = defaultdict(int)
    for row in results:
        counts[row["role"]] += 1
    report = {"schema_version": 1, "kind": "assigned_code_variant_roles", "owner": owner,
        "recorded_at_utc": a.stamp(), "assignment": dict(claim), "variants": results,
        "variant_count": len(rows), "reserved_variant_count": reserved_count,
        "selected_variant_set_sha256": ci.digest(sorted(row["normalized_sha"] for row in rows)),
        "source_count": len(groups), "role_counts": dict(counts), "failures": failures,
        "target_execution": False, "source_chain_complete": False, "device_behavior_complete": False,
        "tool": {"path": a.rel(Path(__file__)), "sha256": a.sha(Path(__file__))}}
    a.save(destination, report)
    return {"receipt": a.rel(destination), "sha256": a.sha(destination), "variant_count": len(rows),
            "role_counts": dict(counts), "failures": failures}
