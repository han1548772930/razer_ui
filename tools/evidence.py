"""Join current acquisitions, complete captures, semantic audits and Rust checks.

This is an analysis index, not a product compatibility gate or a completion
heuristic. A reference edge means exactly that the owner receipt names a file.
Only the owning semantic audit can establish what a call or UI branch means.
"""
from __future__ import annotations

from collections import Counter
import hashlib
import html
import json
import os
from pathlib import Path, PureWindowsPath
import re
import sys
import urllib.parse
from html.parser import HTMLParser

import analysis as a


FORBIDDEN = (".ref/", ".work/latest-source-check/host-4.0.821/")
ROOTS = (".analysis/", "crates/", "tools/", "assets/", "app/", "docs/")


def _evidence_filesystem_path(path):
    """Check the current-tree boundary and support retained WinRT type names."""
    resolved = path.resolve()
    if not resolved.is_relative_to(a.ROOT) or resolved.relative_to(a.ROOT).as_posix().startswith(FORBIDDEN):
        raise ValueError("Evidence path redirects outside the permitted current tree")
    if os.name == "nt":
        value = str(resolved)
        if not value.startswith("\\\\?\\"):
            # Only filesystem calls receive this spelling. Node/claim/edge
            # paths remain the original portable project-relative names.
            return Path("\\\\?\\UNC\\" + value[2:] if value.startswith("\\\\") else "\\\\?\\" + value)
    return resolved


class EvidenceGraph:
    def __init__(self):
        self.nodes = {}
        self.edges = {}
        self.documents = {}
        self.failures = []
        self.snapshots = {}

    def artifact(self, name, claim=None, owner=None, pointer=None, relative_to_owner=False):
        if not isinstance(name, str) or not name:
            return None
        name = name.replace("\\", "/")
        if PureWindowsPath(name).anchor or "://" in name or ".." in Path(name).parts:
            return None
        if not name.startswith(ROOTS) and name not in {"Cargo.lock", "Cargo.toml", "AGENTS.md", "README.md"}:
            if not relative_to_owner or not owner:
                return None
            name = (Path(owner).parent / name).as_posix()
        if name not in self.nodes:
            node = {"path": name, "claims": []}
            self.nodes[name] = node
            if name.startswith(FORBIDDEN):
                node["status"] = "excluded_historical_reference_not_read"
            else:
                path = a.ROOT / name
                try:
                    io_path = _evidence_filesystem_path(path)
                    if io_path.is_file():
                        before = io_path.stat()
                        actual = a.sha(io_path)
                        after = io_path.stat()
                        node.update(bytes=after.st_size, sha256=actual, status="present_without_identity_claim")
                        self.snapshots[name] = (after.st_size, after.st_mtime_ns)
                        if (before.st_size, before.st_mtime_ns) != self.snapshots[name]:
                            node["status"] = "changed_during_hashing"
                    elif io_path.is_dir():
                        node["status"] = "directory"
                    else:
                        node["status"] = "missing"
                except (OSError, ValueError) as error:
                    node.update(status="error", error=str(error))
        node = self.nodes[name]
        if claim and (claim.get("sha256") is not None or claim.get("bytes") is not None):
            expected = {key: claim[key] for key in ("sha256", "bytes") if key in claim}
            matches = bool(node.get("sha256")) and all(node.get(key) == value for key, value in expected.items())
            item = {"owner": owner, "pointer": pointer, "expected": expected, "matches": matches}
            if item not in node["claims"]:
                node["claims"].append(item)
            if node["status"] in {"present_without_identity_claim", "verified", "identity_mismatch"}:
                node["status"] = "verified" if all(row["matches"] for row in node["claims"]) else "identity_mismatch"
        if owner and owner != name:
            key = (owner, name, pointer)
            self.edges[key] = {"from": owner, "to": name, "kind": "receipt_reference", "pointer": pointer}
        return name

    def document(self, path, *, required=False):
        name = a.rel(path)
        if name in self.documents:
            return self.documents[name]
        if self.artifact(name) is None:
            self.failures.append({"kind": "receipt_path_rejected", "path": name})
            return None
        node = self.nodes[name]
        if node["status"] in {"missing", "directory"}:
            if required:
                self.failures.append({"kind": "required_receipt_missing", "path": name})
            return None
        if node["status"] not in {"present_without_identity_claim", "verified", "identity_mismatch"}:
            self.failures.append({"kind": "receipt_read_prevented", "path": name, "status": node["status"]})
            return None
        try:
            raw = _evidence_filesystem_path(path).read_bytes()
            if hashlib.sha256(raw).hexdigest() != self.nodes[name].get("sha256"):
                raise ValueError("Receipt changed during the connected audit")
            data = json.loads(raw)
            self.documents[name] = data
            self.walk(data, name)
            return data
        except (OSError, ValueError) as error:
            self.failures.append({"kind": "receipt_parse_failed", "path": name, "error": str(error)})
            return None

    def walk(self, value, owner, pointer=""):
        if isinstance(value, dict):
            if isinstance(value.get("path"), str):
                self.artifact(value["path"], value, owner, pointer + "/path", relative_to_owner=True)
            if isinstance(value.get("input"), str) and isinstance(value.get("input_sha256"), str):
                claim = {"sha256": value["input_sha256"]}
                if type(value.get("input_bytes")) is int:
                    claim["bytes"] = value["input_bytes"]
                self.artifact(value["input"], claim, owner, pointer + "/input", relative_to_owner=True)
            for key, item in value.items():
                if key not in {"body", "pseudocode", "assembly", "disassembly", "complete_assignment", "path"}:
                    self.walk(item, owner, pointer + "/" + str(key).replace("~", "~0").replace("/", "~1"))
        elif isinstance(value, list):
            for index, item in enumerate(value):
                self.walk(item, owner, pointer + "/" + str(index))
        elif isinstance(value, str) and "\n" not in value:
            self.artifact(value, owner=owner, pointer=pointer)

    def final_snapshot(self):
        changed = []
        for name, expected in self.snapshots.items():
            try:
                current = _evidence_filesystem_path(a.ROOT / name).stat()
                stable = (current.st_size, current.st_mtime_ns) == expected
            except (OSError, ValueError):
                stable = False
            if not stable:
                self.nodes[name]["status"] = "changed_during_connected_audit"
                changed.append(name)
        return changed


def product_references(data, pid, pointer=""):
    """Link explicit product IDs only; names/filenames never imply semantics."""
    found = []
    if isinstance(data, dict):
        if data.get("product_id") == pid or data.get("productId") == pid:
            return [pointer or "/"]
        for key, value in data.items():
            if key not in {"body", "pseudocode", "assembly", "disassembly"}:
                found.extend(product_references(value, pid, pointer + "/" + str(key)))
    elif isinstance(data, list):
        for index, value in enumerate(data):
            found.extend(product_references(value, pid, pointer + "/" + str(index)))
    return found


def frontend_status(graph, directory):
    source = a.ROOT / ".analysis/inputs/frontend" / directory / "acquisition.json"
    capture = a.ROOT / ".analysis/results/frontend" / directory / "analysis.json"
    acquired = graph.document(source)
    captured = graph.document(capture)
    result = {"acquisition": a.rel(source), "capture": a.rel(capture),
              "acquired": acquired is not None, "capture_available": captured is not None,
              "full_ui_semantics": "not_established_by_inventory", "runtime_acceptance": "unverified"}
    if acquired:
        files = acquired.get("files", [])
        result.update(original_files=sum("path" in row for row in files),
                      acquisition_failures=sum("error" in row for row in files),
                      deployment_stable=acquired.get("source_current_during_acquisition") is True,
                      declared_reference_closure=acquired.get("declared_references_fetched") is True,
                      unresolved_references=len(acquired.get("unresolved", [])),
                      original_identities_verified=all(graph.nodes.get(row["path"], {}).get("status") == "verified"
                                                       for row in files if "path" in row))
    if captured:
        result.update(js_inputs=sum(row.get("path", "").endswith(".js") for row in captured.get("original_inputs", [])),
                      generated_files=len(captured.get("outputs", [])),
                      failed_capture_stages=[row.get("stage") for row in captured.get("commands", []) if row.get("exit_code")],
                      original_inputs_unchanged=captured.get("original_inputs_unchanged", captured.get("input_bytes_unchanged")))
    return result


def connect_analysis():
    import families
    graph = EvidenceGraph()
    graph.document(a.ROOT / ".analysis/inputs.json", required=True)
    validation = graph.document(a.ROOT / ".analysis/validation.json", required=True)
    gaps = graph.document(a.ROOT / ".analysis/results/gaps.json", required=True)
    roots = [
        ".analysis/inputs/installed-current/acquisition.json", ".analysis/inputs/host-current/acquisition.json",
        ".analysis/inputs/official-native-current/acquisition.json", ".analysis/results/host-current/inspection.json",
        ".analysis/results/host-current/javascript/capture.json", ".analysis/results/native/chain-audit.json",
        ".analysis/results/input-audit.json", ".analysis/results/analysis-preparation.json",
    ]
    for name in roots:
        graph.document(a.ROOT / name)
    for pattern in ("native/*/capture.json", "host-current/asar/*/capture.json", "host-current/managed/capture.json"):
        for path in sorted((a.ROOT / ".analysis/results").glob(pattern)):
            graph.document(path)
    audits = {}
    for path in sorted((a.ROOT / ".analysis/results/frontend").glob("*-audit.json")):
        data = graph.document(path)
        if data is not None:
            audits[a.rel(path)] = data
    products = []
    for family in families.FAMILY_CANDIDATES:
        ids, scope = families.family_candidate_scope(family)
        for layer in ("ui", "mw"):
            graph.document(a.ROOT / ".analysis/results/families" / (family + ("-mw" if layer == "mw" else "") + ".json"))
        for pid in ids:
            row = {"family": family, "product_id": pid, "scope": "source endpoint locator; identity must be checked in DeviceInfo",
                   "ui": frontend_status(graph, f"{family}-{pid}-current"),
                   "mw": frontend_status(graph, f"{family}-mw-{pid}-current"),
                   "semantic_audits": [], "page_completion": "not_established", "runtime_acceptance": "unverified"}
            for name, data in audits.items():
                pointers = product_references(data, pid)
                if pointers:
                    row["semantic_audits"].append({"path": name, "pointers": pointers})
            products.append(row)
    dashboard = frontend_status(graph, "synapse-dashboard-current")
    applications = []
    for receipt in sorted((a.ROOT / ".analysis/inputs/frontend").glob("*/acquisition.json")):
        if re.fullmatch(r"(?:mouse|keyboard|audio|gamepad|system|accessories)-(?:mw-)?\d+-current", receipt.parent.name):
            continue
        applications.append({"name": receipt.parent.name, **frontend_status(graph, receipt.parent.name)})
    changed = graph.final_snapshot()
    statuses = Counter(row["status"] for row in graph.nodes.values())
    summary = {"product_endpoints": len(products), "ui_acquisitions": sum(row["ui"]["acquired"] for row in products),
               "mw_acquisitions": sum(row["mw"]["acquired"] for row in products),
               "ui_captures": sum(row["ui"]["capture_available"] for row in products),
               "mw_captures": sum(row["mw"]["capture_available"] for row in products),
               "products_with_semantic_audit_references": sum(bool(row["semantic_audits"]) for row in products),
               "artifact_nodes": len(graph.nodes), "reference_edges": len(graph.edges), "artifact_statuses": dict(statuses),
               "changed_during_audit": len(changed), "receipt_failures": len(graph.failures),
               "complete_pages": 0,
               "completion_policy": "Inventory/capture/hash/path matches never establish a complete page or behavior chain"}
    report = {"schema_version": 1, "recorded_at_utc": a.stamp(), "tool": families.identity(Path(__file__)),
              "scope": "All current product endpoints, Dashboard, installed host, native captures, semantic audits and Rust verification",
              "summary": summary, "dashboard": dashboard, "applications": applications, "products": products,
              "semantic_audits": list(audits), "nodes": sorted(graph.nodes.values(), key=lambda row: row["path"]),
              "edges": sorted(graph.edges.values(), key=lambda row: (row["from"], row["to"], row["pointer"] or "")),
              "failures": graph.failures, "changed_inputs": changed,
              "validation": {"path": ".analysis/validation.json", "recorded_passed": validation.get("passed") if validation else None,
                             "revalidated_by_this_command": False},
              "gaps": {"path": ".analysis/results/gaps.json", "owners": len(gaps.get("owners", {})) if gaps else None},
              "target_execution": False, "real_device_operations": False, "runtime_acceptance": "unverified"}
    output = a.ROOT / ".analysis/results/connected-analysis.json"
    a.save(output, report)
    write_view(report)
    print(json.dumps({"report": a.rel(output), "view": ".analysis/results/connected-analysis.html", **summary}), flush=True)
    return report


def write_view(report):
    """Self-contained, offline report; contains no vendor JavaScript."""
    output = a.ROOT / ".analysis/results/connected-analysis.html"
    def link(name, label):
        target = os.path.relpath(a.ROOT / name, output.parent).replace("\\", "/")
        return f'<a href="{html.escape(target, quote=True)}">{html.escape(str(label))}</a>'
    rows = []
    for product in report["products"]:
        cells = [str(product["product_id"]), html.escape(product["family"])]
        for layer in ("ui", "mw"):
            status = product[layer]
            if not status["acquired"]:
                cells.append("尚未获取")
                continue
            text = f'{status["original_files"]} 原件 · {status["acquisition_failures"]} 获取失败'
            if status["capture_available"]:
                text += f' · {status["js_inputs"]} JS'
            cells.append(link(status["acquisition"], text) + (" · " + link(status["capture"], "完整捕获") if status["capture_available"] else ""))
        audits = "<br>".join(link(row["path"], Path(row["path"]).name) for row in product["semantic_audits"])
        cells.extend([audits or "待逐页语义核对", "未完成整页验收"])
        rows.append("<tr>" + "".join("<td>" + cell + "</td>" for cell in cells) + "</tr>")
    summary = report["summary"]
    content = """<!doctype html><html lang="zh-CN"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Razer 本地逆向证据</title>
<style>body{font:15px/1.6 system-ui,sans-serif;margin:32px;color:#dce4e8;background:#121719}h1{font-size:26px;margin-bottom:6px}a{color:#91d59a}p{max-width:1040px}table{border-collapse:collapse;width:100%;font-size:14px}th,td{text-align:left;padding:12px;border-bottom:1px solid #354044;vertical-align:top}th{position:sticky;top:0;background:#20292c}input{padding:10px 14px;background:#20292c;color:inherit;border:1px solid #657478;border-radius:5px;width:400px;max-width:85%;font:inherit}small{color:#aab8bf}.stats{display:flex;gap:28px;margin:22px 0}.stats strong{font-size:24px;display:block}nav{margin:18px 0}tr[hidden]{display:none}</style>
<h1>Razer 本地逆向证据</h1><small>__TIME__</small>
<p>沿原始输入 → 完整源码／原生函数 → 语义审计 → Rust 实现 → 验证记录追踪。表中每一行是获取端点，不能把端点、捕获或哈希通过当作整页完成。源码行为、UI 接线、设备写回与真实运行分别验收。</p>
<div class="stats"><div><strong>__PRODUCTS__</strong>产品端点</div><div><strong>__CAPTURES__</strong>UI／MW 捕获</div><div><strong>__EDGES__</strong>可追溯引用</div><div><strong>__UNVERIFIED__</strong>真实窗口／设备验收</div></div>
<nav>__LINKS__</nav><label for="filter">筛选 PID／产品族／审计名称 </label><input id="filter" type="search" autocomplete="off">
<table><thead><tr><th>PID</th><th>族</th><th>UI</th><th>Middleware</th><th>语义与实现对照</th><th>整页状态</th></tr></thead><tbody>__ROWS__</tbody></table>
<script>const filter=document.getElementById('filter');filter.addEventListener('input',()=>{const term=filter.value.toLowerCase();for(const row of document.querySelectorAll('tbody tr'))row.hidden=!row.textContent.toLowerCase().includes(term)});</script></html>"""
    links = [link(".analysis/results/connected-analysis.json", "完整引用图 JSON"),
             link(report["dashboard"]["acquisition"], "Dashboard 原件"),
             link(report["dashboard"]["capture"], "Dashboard 完整捕获"),
             link(".analysis/results/native/chain-audit.json", "原生调用链"),
             link(".analysis/validation.json", "Rust 验证"), link(".analysis/results/gaps.json", "未解项")]
    links.extend(link(app["acquisition"], app["name"]) for app in report["applications"] if app["name"] != "synapse-dashboard-current")
    content = content.replace("__TIME__", html.escape(report["recorded_at_utc"]))
    content = content.replace("__PRODUCTS__", str(summary["product_endpoints"]))
    content = content.replace("__CAPTURES__", str(summary["ui_captures"] + summary["mw_captures"]))
    content = content.replace("__EDGES__", str(summary["reference_edges"]))
    content = content.replace("__UNVERIFIED__", "未执行").replace("__LINKS__", " · ".join(links)).replace("__ROWS__", "".join(rows))
    output.write_text(content, encoding="utf-8")


def locale_keys(raw, keys):
    """Read literal strings from one complete original dictionary object."""
    sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    root = Parser(Language(tree_sitter_javascript.language())).parse(raw).root_node
    if root.has_error:
        raise ValueError("Locale source has JavaScript parse errors")
    wanted = set(keys)
    if not wanted or len(wanted) != len(keys):
        raise ValueError("Locale keys must be nonempty and unique")
    matches, pending = [], [root]
    while pending:
        node = pending.pop()
        if node.type == "object":
            entries = {}
            for pair in node.named_children:
                if pair.type != "pair":
                    continue
                key = pair.child_by_field_name("key")
                value = pair.child_by_field_name("value")
                key_text = raw[key.start_byte:key.end_byte].decode("utf-8")
                if key.type == "string":
                    key_text = json.loads(key_text)
                if key_text in wanted:
                    if key_text in entries or value.type != "string":
                        raise ValueError("Locale key has a duplicate or nonliteral value: " + key_text)
                    entries[key_text] = json.loads(raw[value.start_byte:value.end_byte])
            if wanted <= entries.keys():
                body = raw[node.start_byte:node.end_byte]
                matches.append({"values": entries, "owner": {
                    "start_byte": node.start_byte, "end_byte": node.end_byte,
                    "body": body.decode("utf-8"), "sha256": hashlib.sha256(body).hexdigest()}})
        pending.extend(node.named_children)
    if len(matches) != 1:
        raise ValueError(f"Locale dictionary owner is not unique: {len(matches)}")
    return matches[0]


class JavascriptBindings:
    """Index actual lexical owners once for repeated static source queries."""
    def __init__(self, raw, tree):
        if tree.root_node.has_error:
            raise ValueError("Binding source has invalid syntax")
        self.raw, self.tree = raw, tree
        scopes = {"program", "statement_block", "arrow_function", "function_expression",
                  "function_declaration", "method_definition", "catch_clause",
                  "for_statement", "for_in_statement", "switch_body"}
        functions = {"arrow_function", "function_expression", "function_declaration", "method_definition"}

        def owner(node, *, function_scoped=False):
            current = node.parent
            while current is not None:
                if current.type in scopes and (not function_scoped or current.type in functions | {"program"}):
                    return current
                current = current.parent
            return tree.root_node

        def key(node):
            return node.type, node.start_byte, node.end_byte

        definitions, pending = {}, [tree.root_node]
        while pending:
            node = pending.pop()
            pending.extend(reversed(node.named_children))
            if node.type == "variable_declarator":
                binding = node.child_by_field_name("name")
                if binding is None:
                    continue
                is_var = node.parent is not None and node.parent.type == "variable_declaration"
                target = owner(node, function_scoped=is_var)
                if binding.type == "identifier":
                    definitions.setdefault(key(target), []).append((binding.text.decode("utf-8"), node, "declaration"))
                else:
                    # Do not invent a value for a destructured binding.
                    descendants = [binding]
                    while descendants:
                        candidate = descendants.pop()
                        if candidate.type in {"identifier", "shorthand_property_identifier_pattern"}:
                            definitions.setdefault(key(target), []).append((candidate.text.decode("utf-8"), node, "destructuring_requires_value_analysis"))
                        descendants.extend(candidate.named_children)
            elif node.type in {"function_declaration", "class_declaration"}:
                binding = node.child_by_field_name("name")
                if binding is not None:
                    definitions.setdefault(key(owner(node)), []).append((binding.text.decode("utf-8"), node, "declaration"))
            if node.type in functions | {"catch_clause"}:
                parameters = node.child_by_field_name("parameters") or node.child_by_field_name("parameter")
                if parameters is not None:
                    descendants = [parameters]
                    while descendants:
                        candidate = descendants.pop()
                        if candidate.type in {"identifier", "shorthand_property_identifier_pattern"}:
                            definitions.setdefault(key(node), []).append((candidate.text.decode("utf-8"), candidate, "parameter_requires_caller_analysis"))
                        descendants.extend(candidate.named_children)
        self.definitions, self.key = definitions, key

    def resolve(self, name, reference_byte):
        raw, tree, definitions = self.raw, self.tree, self.definitions
        key = self.key
        if not 0 <= reference_byte < len(raw):
            raise ValueError("Binding reference has invalid byte boundary")
        current = tree.root_node.descendant_for_byte_range(reference_byte, reference_byte)
        while current is not None:
            candidates = [item for item in definitions.get(key(current), []) if item[0] == name]
            if candidates:
                if len(candidates) != 1:
                    return {"name": name, "reference_byte": reference_byte, "resolution": "ambiguous_same_scope_declarations",
                            "candidates": [{"start_byte": item[1].start_byte, "end_byte": item[1].end_byte} for item in candidates]}
                _, node, resolution = candidates[0]
                body = raw[node.start_byte:node.end_byte]
                return {"name": name, "reference_byte": reference_byte, "resolution": resolution,
                        "start_byte": node.start_byte, "end_byte": node.end_byte,
                        "syntax": node.type, "body": body.decode("utf-8"), "sha256": hashlib.sha256(body).hexdigest(),
                        "scope": {"syntax": current.type, "start_byte": current.start_byte, "end_byte": current.end_byte}}
            current = current.parent
        return {"name": name, "reference_byte": reference_byte, "resolution": "unresolved_identifier"}


def javascript_binding(raw, tree, name, reference_byte):
    """Resolve an identifier only in its actual enclosing lexical scopes."""
    return JavascriptBindings(raw, tree).resolve(name, reference_byte)


def ui_tree(raw):
    """Capture every JSX factory occurrence and its complete lexical owner.

    Calls and conditional contexts are syntax facts. A dynamic component or
    spread remains unresolved; this does not pretend to execute React mounts.
    """
    sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
    from tree_sitter import Language, Parser
    import tree_sitter_javascript
    tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
    if tree.root_node.has_error:
        failures, pending = [], [tree.root_node]
        while pending:
            node = pending.pop()
            if node.type == "ERROR" or node.is_missing:
                failures.append({"type": node.type, "start_byte": node.start_byte, "end_byte": node.end_byte})
            elif node.has_error:
                pending.extend(reversed(node.children))
        return {"parse_passed": False, "failures": failures, "owners": [], "elements": [], "actions": []}
    def span(node, *, body=False):
        result = {"start_byte": node.start_byte, "end_byte": node.end_byte, "syntax": node.type}
        if body:
            value = raw[node.start_byte:node.end_byte]
            result.update(body=value.decode("utf-8"), sha256=hashlib.sha256(value).hexdigest())
        return result
    def text(node):
        return raw[node.start_byte:node.end_byte].decode("utf-8") if node is not None else None
    function_types = {"function_declaration", "function_expression", "arrow_function", "method_definition"}
    owner_types = function_types | {"class_declaration", "class"}
    owners, elements, actions = {}, [], []
    pending = [tree.root_node]
    while pending:
        node = pending.pop()
        pending.extend(reversed(node.named_children))
        if node.type == "string":
            value = text(node)
            if re.fullmatch(r'''["']ON_[A-Z0-9_]+["']''', value):
                actions.append({**span(node), "action": value[1:-1]})
        if node.type != "call_expression":
            continue
        function = node.child_by_field_name("function")
        callee = function
        # Babel emits (0, runtime.jsx)(...) to discard the member receiver.
        # Read the final operand syntactically; never execute the comma chain.
        while callee is not None and callee.type in {"parenthesized_expression", "sequence_expression"}:
            callee = callee.named_children[-1] if callee.named_children else None
        if callee is None or callee.type != "member_expression":
            continue
        property_node = callee.child_by_field_name("property")
        factory = text(property_node)
        if factory not in {"jsx", "jsxs", "jsxDEV", "createElement"}:
            continue
        arguments = node.child_by_field_name("arguments")
        args = arguments.named_children if arguments else []
        if not args:
            continue
        ancestors, lexical_owners, current = [], [], node.parent
        while current is not None:
            if current.type in {"if_statement", "ternary_expression", "switch_case", "for_statement", "for_in_statement", "while_statement", "binary_expression"}:
                condition = current.child_by_field_name("condition") or current.child_by_field_name("value")
                ancestors.append({**span(current), "condition": span(condition, body=True) if condition is not None else None})
            if current.type in owner_types:
                lexical_owners.append(current)
            current = current.parent
        if not lexical_owners:
            lexical_owners = [tree.root_node]
        owner_ids = []
        for owner in lexical_owners:
            owner_id = f'{owner.start_byte}:{owner.end_byte}'
            owner_ids.append(owner_id)
            if owner_id not in owners:
                name = owner.child_by_field_name("name")
                if name is None and owner.parent is not None and owner.parent.type in {"pair", "variable_declarator"}:
                    name = owner.parent.child_by_field_name("key") or owner.parent.child_by_field_name("name")
                owners[owner_id] = {"id": owner_id, "name": text(name), **span(owner, body=True)}
        props, unresolved = [], []
        if len(args) > 1 and args[1].type == "object":
            for item in args[1].named_children:
                key, value = item.child_by_field_name("key"), item.child_by_field_name("value")
                if item.type == "pair" and key is not None and value is not None:
                    label = text(key).strip("\"'")
                    props.append({"name": label, **span(value, body=label != "children"),
                                  "role": "event_handler" if label.startswith("on") else "property"})
                else:
                    unresolved.append({"kind": "spread_or_shorthand_requires_binding_resolution", **span(item, body=True)})
        elif len(args) > 1 and args[1].type != "null":
            unresolved.append({"kind": "props_expression_requires_binding_resolution", **span(args[1], body=True)})
        component = text(args[0])
        if args[0].type != "string":
            unresolved.append({"kind": "component_binding_requires_module_resolution", "expression": component})
        elements.append({"id": str(node.start_byte), **span(node), "factory": text(function),
                         "component": component, "owner": owner_ids[0], "lexical_owners": owner_ids, "properties": props,
                         "conditional_context": ancestors, "unresolved": unresolved})
    return {"parse_passed": True, "owners": list(owners.values()), "elements": elements, "actions": actions,
            "failures": [], "mount_semantics": "Static occurrences; lexical owners/branches captured, dynamic binding requires semantic audit"}


def frontend_deployment_sources(acquired):
    """Separate actual HTML includes from manifest candidates and retained files.

    This resolves deployment references, not React reachability. In particular,
    an old main bundle retained beside the current deployment must not acquire
    current-source status merely because its bytes still match its old receipt.
    """
    files = {}
    failures, selected, originals = [], {}, {}
    for item in acquired.get("files", []):
        if item.get("url") in files:
            failures.append({"kind": "duplicate_acquisition_url", "url": item["url"]})
        files[item.get("url")] = item

    def read(url, *, required=True):
        item = files.get(url, {})
        if not item.get("path"):
            if required:
                failures.append({"kind": "deployment_original_unavailable", "url": url,
                                 "acquisition_error": item.get("error")})
            return None
        try:
            name = item["path"].replace("\\", "/")
            if not name.startswith(".analysis/inputs/") or ".." in Path(name).parts:
                raise ValueError("Original is outside current input boundary")
            raw = _evidence_filesystem_path(a.ROOT / name).read_bytes()
            if len(raw) != item.get("bytes") or hashlib.sha256(raw).hexdigest() != item.get("sha256"):
                raise ValueError("Original byte identity differs")
            originals[url] = {key: item[key] for key in ("path", "bytes", "sha256")}
            return raw
        except (OSError, ValueError, KeyError) as error:
            failures.append({"kind": "deployment_original_invalid", "url": url, "error": str(error)})
            return None

    def select(url, role, owner, literal):
        if not url or Path(urllib.parse.urlsplit(url).path).suffix.lower() not in {".js", ".css"}:
            return
        if url not in selected:
            selected[url] = {"url": url, "references": []}
        reference = {"role": role, "from": owner, "literal": literal}
        if reference not in selected[url]["references"]:
            selected[url]["references"].append(reference)

    class EntryReferences(HTMLParser):
        def __init__(self, text):
            super().__init__()
            self.base, self.includes, self.inline = None, [], []
            self.text, self.active_inline = text, None
            self.lines = [0]
            self.lines.extend(match.end() for match in re.finditer("\n", text))

        def character_offset(self):
            line, column = self.getpos()
            return self.lines[line - 1] + column

        def handle_starttag(self, tag, attrs):
            attrs = dict(attrs)
            if tag == "base" and self.base is None:
                self.base = attrs.get("href")
            if tag == "script":
                if attrs.get("src"):
                    self.includes.append((attrs["src"], "html_script_include"))
                else:
                    start = self.character_offset() + len(self.get_starttag_text())
                    self.active_inline = {"kind": "inline_script_requires_owner_analysis", "attributes": attrs,
                                          "character_start": start}
            if tag == "link" and attrs.get("href"):
                rel = set(attrs.get("rel", "").lower().split())
                if "stylesheet" in rel:
                    self.includes.append((attrs["href"], "html_stylesheet_include"))
                elif rel & {"preload", "modulepreload"}:
                    self.includes.append((attrs["href"], "html_preload_candidate"))

        def handle_endtag(self, tag):
            if tag == "script" and self.active_inline is not None:
                start, end = self.active_inline["character_start"], self.character_offset()
                body = self.text[start:end].encode("utf-8")
                self.inline.append({**self.active_inline, "start_byte": len(self.text[:start].encode("utf-8")),
                    "end_byte": len(self.text[:end].encode("utf-8")), "body": body.decode("utf-8"),
                    "sha256": hashlib.sha256(body).hexdigest()})
                self.active_inline = None

    entry = acquired.get("entry_url")
    entry_raw = read(entry)
    entry_response = a.artifact_url(entry or "", files.get(entry, {}).get("final_url", entry or ""))
    if (entry_response is None or urllib.parse.urlsplit(entry_response).netloc != urllib.parse.urlsplit(entry or "").netloc):
        failures.append({"kind": "deployment_entry_redirect_outside_origin", "url": entry,
                         "final_url": files.get(entry, {}).get("final_url")})
        entry_response = entry
    parsed = EntryReferences(entry_raw.decode("utf-8") if entry_raw is not None else "")
    if entry_raw is not None:
        parsed.feed(parsed.text)
        base = urllib.parse.urljoin(entry_response, parsed.base) if parsed.base else entry_response
        for literal, role in parsed.includes:
            select(a.artifact_url(base, literal), role, entry, literal)
    manifests = []
    for filename in ("asset-manifest.json", "webpackManifest.json"):
        url = urllib.parse.urljoin(entry_response or "", filename)
        raw = read(url, required=False)
        if raw is None:
            continue
        try:
            manifest = json.loads(raw)
            if not isinstance(manifest, dict):
                raise ValueError("Manifest is not an object")
            manifests.append({"url": url, "original": originals[url]})
            values = manifest if filename == "webpackManifest.json" else manifest.get("files", {})
            if not isinstance(values, dict):
                raise ValueError("Manifest file table is not an object")
            for name, literal in values.items():
                if not isinstance(literal, str):
                    failures.append({"kind": "nonliteral_manifest_reference", "url": url, "entry": name})
                    continue
                select(a.artifact_url(url, literal), "manifest_declared_candidate", url, literal)
            entrypoints = manifest.get("entrypoints", []) if filename == "asset-manifest.json" else []
            if not isinstance(entrypoints, list):
                raise ValueError("Manifest entrypoints are not an array")
            for literal in entrypoints:
                if not isinstance(literal, str):
                    failures.append({"kind": "nonliteral_manifest_entrypoint", "url": url})
                    continue
                target = a.artifact_url(url, literal)
                select(target, "manifest_entrypoint_declaration", url, literal)
                if target in selected and not any(ref["role"] in {"html_script_include", "html_stylesheet_include"}
                                                  for ref in selected[target]["references"]):
                    failures.append({"kind": "manifest_entrypoint_missing_from_html", "url": target})
        except (ValueError, TypeError) as error:
            failures.append({"kind": "deployment_manifest_invalid", "url": url, "error": str(error)})

    # Acquisition seeds express what was requested, not what this deployment
    # imports. Follow only actual reference edges from selected source owners.
    reachable = {entry, entry_response, *(item["url"] for item in manifests), *selected}
    changed = True
    edges = acquired.get("edges", [])
    while changed:
        changed = False
        for edge in edges:
            if edge.get("from") not in reachable or edge.get("kind") == "explicit-retained-manifest":
                continue
            target = edge.get("to")
            if not isinstance(target, str) or target in reachable:
                continue
            reachable.add(target)
            changed = True
            select(target, "static_reference_candidate", edge["from"], edge.get("literal"))
    for url, item in selected.items():
        raw = read(url)
        item["identity_verified"] = raw is not None
        item["original"] = originals.get(url)
        roles = {ref["role"] for ref in item["references"]}
        item["deployment_role"] = ("html_include" if roles & {"html_script_include", "html_stylesheet_include"}
                                   else "conditional_or_declared_candidate")
        item["react_mount_semantics"] = "requires_complete_component_and_caller_audit"
    stable = acquired.get("source_current_during_acquisition") is True
    if not stable:
        failures.append({"kind": "current_deployment_stability_unverified"})
    for claim in acquired.get("stability", []):
        if claim.get("url") not in originals:
            continue
        digest = originals[claim["url"]]["sha256"]
        if claim.get("unchanged") is not True or claim.get("before_sha256") != digest or claim.get("after_sha256") != digest:
            failures.append({"kind": "deployment_stability_identity_mismatch", "url": claim.get("url")})
    if parsed.active_inline is not None:
        failures.append({"kind": "unterminated_inline_script", "url": entry})
    return {"entry_url": entry, "entry_response_url": entry_response,
            "entry_original": originals.get(entry), "manifests": manifests,
            "sources": list(selected.values()), "failures": failures, "inline_scripts": parsed.inline,
            "retained_without_current_reference": [{"url": url, "path": item["path"],
                "status": "not_current_implementation_evidence"} for url, item in files.items()
                if item.get("path") and url not in selected
                and Path(urllib.parse.urlsplit(url).path).suffix.lower() in {".js", ".css"}],
            "unresolved_acquisition_references": acquired.get("unresolved", []),
            "source_semantics_complete": False, "target_execution": False}


def audit_ui_tree():
    """Index UI/MW syntax by verified deployment includes and candidates."""
    import families
    import importlib.metadata
    sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
    tool = families.identity(Path(__file__))
    parser = {name: importlib.metadata.version(name) for name in ("tree-sitter", "tree-sitter-javascript")}
    actions = {}
    for path in sorted((a.ROOT / "crates").rglob("*.rs")):
        if "src" not in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        for match in re.finditer(r'"(ON_[A-Z0-9_]+)"', text):
            actions.setdefault(match[1], []).append({"path": a.rel(path), "sha256": a.sha(path),
                "line": text.count("\n", 0, match.start()) + 1,
                "role": "literal_occurrence_candidate_only; not proof of dispatch or write-back"})
    products, unique, failures, snapshots = [], {}, [], {}
    root = a.ROOT / ".analysis/inputs/frontend"
    receipts = set(root.glob("*/acquisition.json"))
    for family in families.FAMILY_CANDIDATES:
        ids, _ = families.family_candidate_scope(family)
        for pid in ids:
            for layer in ("", "mw-"):
                receipts.add(root / f"{family}-{layer}{pid}-current/acquisition.json")
    for receipt in sorted(receipts):
        match = re.fullmatch(r"(mouse|keyboard|audio|gamepad|system|accessories)-(mw-)?(\d+)-current", receipt.parent.name)
        row = {"product_id": int(match[3]) if match else None, "family": match[1] if match else receipt.parent.name,
               "layer": "mw" if match and match[2] else "ui", "source_set": receipt.parent.name,
               "acquisition_path": a.rel(receipt), "bundles": [], "stylesheets": [], "actions": [], "failures": [],
               "page_completion": "not_established", "runtime_acceptance": "unverified"}
        products.append(row)
        if not receipt.is_file():
            row["failures"].append({"kind": "acquisition_receipt_unavailable"})
            failures.append({"kind": "acquisition_receipt_unavailable", "path": a.rel(receipt)})
            continue
        raw_receipt = receipt.read_bytes()
        row["acquisition"] = {"path": a.rel(receipt), "bytes": len(raw_receipt),
                              "sha256": hashlib.sha256(raw_receipt).hexdigest()}
        snapshots[a.rel(receipt)] = row["acquisition"]
        try:
            acquired = json.loads(raw_receipt)
        except ValueError as error:
            failure = {"kind": "acquisition_receipt_invalid", "error": str(error)}
            row["failures"].append(failure)
            failures.append({"path": a.rel(receipt), **failure})
            continue
        deployment = frontend_deployment_sources(acquired)
        row["deployment"] = deployment
        for original in [deployment["entry_original"], *(item["original"] for item in deployment["manifests"])]:
            if original is not None:
                snapshots[original["path"]] = original
        row["failures"].extend(deployment["failures"])
        failures.extend({"source_set": receipt.parent.name, **error} for error in deployment["failures"])
        if acquired.get("source_current_during_acquisition") is not True:
            continue
        action_names = set()
        for source in deployment["sources"]:
            if not source["identity_verified"]:
                continue
            identity = source["original"]
            name = identity["path"]
            snapshots[name] = identity
            if Path(urllib.parse.urlsplit(source["url"]).path).suffix.lower() == ".css":
                row["stylesheets"].append(source)
                continue
            digest = identity["sha256"]
            destination = a.ROOT / ".analysis/results/frontend/ui-trees" / (digest + ".json")
            if digest not in unique:
                parsed = None
                if destination.is_file():
                    previous = json.loads(destination.read_bytes())
                    if previous.get("tool") == tool and previous.get("parser") == parser and previous.get("input_sha256") == digest:
                        parsed = previous
                if parsed is None:
                    raw = (a.ROOT / name).read_bytes()
                    if hashlib.sha256(raw).hexdigest() != digest:
                        raise ValueError("Original changed before UI AST parse: " + name)
                    parsed = {"schema_version": 2, "input": name, "input_sha256": digest, "input_bytes": len(raw),
                              "tool": tool, "parser": parser, **ui_tree(raw), "target_execution": False}
                    a.save(destination, parsed)
                unique[digest] = {"report": families.identity(destination), "parse_passed": parsed["parse_passed"],
                                  "lexical_owners": len(parsed["owners"]), "elements": len(parsed["elements"]),
                                  "action_names": sorted({item["action"] for item in parsed["actions"]})}
            row["bundles"].append({**source, **unique[digest]})
            if not unique[digest]["parse_passed"]:
                error = {"kind": "javascript_parse_failed", "path": name}
                row["failures"].append(error)
                failures.append(error)
            action_names.update(unique[digest]["action_names"])
        row["actions"] = [{"action": name, "rust_literal_candidates": actions.get(name, []),
                           "source_to_backend_chain": "requires_semantic_audit"} for name in sorted(action_names)]
        print(json.dumps({"source_tree": receipt.parent.name, "layer": row["layer"], "bundles": len(row["bundles"]),
                          "stylesheets": len(row["stylesheets"]), "action_literals": len(action_names)}), flush=True)
    changed = []
    for name, original in snapshots.items():
        path = _evidence_filesystem_path(a.ROOT / name)
        if not path.is_file() or path.stat().st_size != original["bytes"] or a.sha(path) != original["sha256"]:
            changed.append(name)
    failures.extend({"kind": "input_changed_during_source_audit", "path": name} for name in changed)
    report = {"schema_version": 2, "recorded_at_utc": a.stamp(), "tool": tool, "parser": parser,
              "scope": "All registered UI/MW locators and retained application acquisitions; HTML includes separated from manifest/static-reference candidates; complete lexical JSX owners and branch contexts",
              "snapshot_stable": not changed, "changed_inputs": changed,
              "semantics_complete": False, "mount_tree_complete": False,
              "products": products, "failures": failures, "summary": {"source_sets": len(products), "unique_bundles": len(unique),
                  "missing_source_sets": sum(any(error["kind"] == "acquisition_receipt_unavailable" for error in row["failures"]) for row in products),
                  "ui_source_sets": sum(row["layer"] == "ui" for row in products),
                  "mw_source_sets": sum(row["layer"] == "mw" for row in products),
                  "jsx_occurrences": sum(row["elements"] for row in unique.values()),
                  "complete_lexical_owners": sum(row["lexical_owners"] for row in unique.values()),
                  "parse_failures": sum(not row["parse_passed"] for row in unique.values())},
              "target_execution": False, "real_device_operations": False, "runtime_acceptance": "unverified"}
    destination = a.ROOT / ".analysis/results/frontend/ui-tree-audit.json"
    a.save(destination, report)
    print(json.dumps({"report": a.rel(destination), **report["summary"]}), flush=True)
    return report


def add_commands(parsers):
    parsers.add_parser("connect-analysis", help="Join original bytes, complete captures, semantic audits, Rust and verification")
    parsers.add_parser("audit-ui-tree", help="Capture complete JSX owners, properties, handlers and branch contexts from current originals")
    parsers.add_parser("verify-analysis-tools", help="Pure provenance, byte-range and static UI-tree regression tests")
